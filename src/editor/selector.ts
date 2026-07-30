/**
 * 元素选择器（Selector）。
 *
 * 跑在编辑文档的 contentDocument 上（通过父窗口同域访问），负责：
 * - 点击选中目标元素 → 绘制选中框 + 手柄
 * - 双击 text 目标 → 进入 contenteditable 文字编辑
 * - 鼠标悬停 → 绘制淡色高亮预览
 * - Esc / 点空白 → 取消选中
 * - 滚动 / 窗口尺寸变化 → 重定位选中框
 *
 * 借鉴 joyxiaofan-beep/html-visual-editor 的选择器交互范式，
 * 但改用「语义目标模型」驱动（targets.ts），更健壮。
 *
 * 与属性面板（EditPanel）的协作：选中变化时通过 onChange 回调通知父组件，
 * 父组件读取选中元素的计算样式回填面板。
 */
import { resolveTarget, type Target } from "./targets";
import {
  injectOverlayStyle,
  ensureOverlay,
  positionBox,
} from "./overlay";

/** 选择器对外暴露的选中状态快照（供属性面板读取）。 */
export interface SelectionInfo {
  /** 选中元素（父窗口可直接访问，因同域）。 */
  el: HTMLElement;
  /** 能力分类。 */
  kind: Target["kind"];
  /** 标签名（大写）。 */
  tag: string;
}

/** 选择器配置。 */
export interface SelectorOptions {
  /** 选中目标变化时回调（target 为 null 表示取消选中）。 */
  onChange: (info: SelectionInfo | null) => void;
  /** 文档被改动时回调（用于标记 dirty）。 */
  onDirty: () => void;
  /** 文字编辑状态变化回调（进入/退出 contenteditable 局部编辑）。 */
  onEditingChange?: (editing: boolean) => void;
}

export class Selector {
  private doc: Document;
  private opts: SelectorOptions;
  private overlay: HTMLElement;
  private selectionBox: HTMLElement;
  private hoverBox: HTMLElement;
  private current: Target | null = null;
  // 文字编辑态：当前处于 contenteditable 的元素
  private editingEl: HTMLElement | null = null;
  // 进入文字编辑时的原始 innerHTML，用于退出时对比是否真正改动
  private editingSnapshot = "";
  private boundHandlers: Array<{ type: string; fn: EventListener; opts?: boolean | AddEventListenerOptions }> = [];

  constructor(doc: Document, opts: SelectorOptions) {
    this.doc = doc;
    this.opts = opts;
    injectOverlayStyle(doc);
    this.overlay = ensureOverlay(doc);
    console.log("[drag-debug] Selector 构造完成", {
      overlayId: this.overlay.id,
      overlayInDOM: doc.body?.contains(this.overlay),
      docBodyChildCount: doc.body?.children.length,
    });
    this.selectionBox = doc.createElement("div");
    this.selectionBox.className = "leaf-selection-box";
    this.selectionBox.style.display = "none";
    this.hoverBox = doc.createElement("div");
    this.hoverBox.className = "leaf-hover-box";
    this.hoverBox.style.display = "none";
    this.overlay.appendChild(this.hoverBox);
    this.overlay.appendChild(this.selectionBox);
    // 构造时不挂事件——由 enable() 显式启用（App.vue 在 select 模式下才启用）
  }

  /** 启用选择器：注册文档级事件监听。可重复调用（幂等）。 */
  enable() {
    if (this.boundHandlers.length > 0) return; // 已启用
    this.add("click", this.onClick, true);
    this.add("dblclick", this.onDblClick, true);
    this.add("mouseover", this.onMouseOver, true);
    this.add("mouseout", this.onMouseOut, true);
    this.add("scroll", this.reposition, true);
    this.add("keydown", this.onKeydown, true);
    this.add("resize", this.reposition);
  }

  /** 停用选择器：移除事件监听 + 清除选中态，但保留实例（可再 enable）。 */
  disable() {
    for (const h of this.boundHandlers) {
      this.doc.removeEventListener(h.type, h.fn, h.opts);
    }
    this.boundHandlers = [];
    this.exitTextEdit();
    this.select(null);
    this.hoverBox.style.display = "none";
  }

  /** 注销所有监听并移除覆盖层元素（彻底销毁）。 */
  detach() {
    this.disable();
    this.selectionBox.remove();
    this.hoverBox.remove();
  }

  private add(type: string, fn: EventListener, opts?: boolean | AddEventListenerOptions) {
    this.doc.addEventListener(type, fn, opts);
    this.boundHandlers.push({ type, fn, opts });
  }

  /** 点击：选中目标。 */
  private onClick = (e: Event) => {
    // 文字编辑态下，点击交由浏览器原生处理（光标定位）
    if (this.editingEl) {
      // 点击文字编辑元素外部 → 退出文字编辑
      const target = e.target as HTMLElement;
      if (this.editingEl.contains(target)) return;
      this.exitTextEdit();
    }
    const t = resolveTarget(e.target);
    console.log("[drag-debug] onClick", { hasTarget: !!t, kind: t?.kind, tag: t?.el?.tagName });
    if (!t) {
      this.select(null);
      return;
    }
    // 阻止默认（如链接导航，虽 guard 已拦截，双保险）
    e.preventDefault();
    this.select(t);
  };

  /** 双击：text 目标进入文字编辑。 */
  private onDblClick = (e: Event) => {
    const t = resolveTarget(e.target);
    if (!t || t.kind !== "text") return;
    e.preventDefault();
    this.enterTextEdit(t.el);
  };

  /** 悬停：绘制高亮预览。 */
  private onMouseOver = (e: Event) => {
    if (this.editingEl) return; // 文字编辑态不显示悬停高亮
    const t = resolveTarget(e.target);
    if (!t) {
      this.hoverBox.style.display = "none";
      return;
    }
    this.hoverBox.style.display = "block";
    positionBox(this.hoverBox, t.el, this.doc);
  };

  private onMouseOut = () => {
    this.hoverBox.style.display = "none";
  };

  /** 键盘：Esc 取消选中 / 退出文字编辑。 */
  private onKeydown = ((e: Event) => {
    const ke = e as KeyboardEvent;
    if (ke.key !== "Escape") return;
    if (this.editingEl) {
      this.exitTextEdit();
    } else if (this.current) {
      this.select(null);
    }
    ke.preventDefault();
  }) as EventListener;

  /** 进入文字编辑：给元素加 contenteditable + 聚焦。 */
  private enterTextEdit(el: HTMLElement) {
    // 先退出旧选中的文字编辑
    this.exitTextEdit();
    el.setAttribute("contenteditable", "true");
    el.focus();
    this.editingEl = el;
    // 记录原始内容，退出时对比判断是否真正改动
    this.editingSnapshot = el.innerHTML;
    // 隐藏选中框（编辑时光标即反馈）
    this.selectionBox.style.display = "none";
    // 通知外部：进入文字编辑
    this.opts.onEditingChange?.(true);
  }

  /** 退出文字编辑：移除 contenteditable，恢复选中框。 */
  private exitTextEdit() {
    if (!this.editingEl) return;
    const changed = this.editingEl.innerHTML !== this.editingSnapshot;
    this.editingEl.removeAttribute("contenteditable");
    this.editingEl = null;
    if (this.current) {
      this.selectionBox.style.display = "block";
      this.reposition();
      // 仅在内容真正变化时标记 dirty，避免双击后不改字也误报改动
      if (changed) this.opts.onDirty();
    }
    // 通知外部：退出文字编辑
    this.opts.onEditingChange?.(false);
  }

  /** 设置当前选中目标（null 表示取消选中）。 */
  select(t: Target | null) {
    if (this.editingEl) this.exitTextEdit();
    this.current = t;
    if (!t) {
      this.selectionBox.style.display = "none";
      this.opts.onChange(null);
      return;
    }
    this.selectionBox.style.display = "block";
    this.reposition();
    this.opts.onChange({ el: t.el, kind: t.kind, tag: t.el.tagName });
  }

  /** 重定位选中框（滚动 / resize 后调用）。 */
  reposition = () => {
    if (this.current && this.selectionBox.style.display !== "none") {
      positionBox(this.selectionBox, this.current.el, this.doc);
    }
  };

  /** 当前选中元素（供 drag / 属性面板使用）。 */
  get selected(): Target | null {
    return this.current;
  }

  /** 是否处于文字编辑状态（有元素在 contenteditable 中）。 */
  get isEditing(): boolean {
    return !!this.editingEl;
  }

  /** 当前正在编辑的 contenteditable 元素。 */
  get editingElement(): HTMLElement | null {
    return this.editingEl;
  }

  /** 直接修改选中元素的某条 inline 样式（属性面板调用）。 */
  updateStyle(prop: string, value: string) {
    if (!this.current) return;
    this.current.el.style.setProperty(prop, value);
    this.reposition();
    this.opts.onDirty();
  }
}
