/**
 * 拖拽移动（Drag）。
 *
 * 在选中框上提供一个移动手柄（move handle），mousedown 后跟随鼠标移动，
 * 通过修改元素的 inline `transform: translate(...)` 实现位移。
 *
 * 设计考量：
 * - 只在元素为 `container` 类（或已被定位的元素）时启用拖拽；text 类靠双击改字。
 * - 用 transform 而非 top/left，避免破坏文档原有定位布局，且性能更好。
 * - 拖拽结束后通知父组件标记 dirty，并让选择器重定位选中框。
 *
 * 借鉴 joyxiaofan-beep/html-visual-editor 的拖拽范式，但简化为「移动手柄」
 * 一种模式（不做四角缩放，缩放留给属性面板的 width/height 输入）。
 */
import type { Selector } from "./selector";

export interface DragOptions {
  /** 拖拽完成（松手）后回调，用于标记 dirty + 重定位选中框。 */
  onEnd: () => void;
}

export class DragController {
  private doc: Document;
  private selector: Selector;
  private opts: DragOptions;
  private handle: HTMLElement | null = null;
  private dragging = false;
  private startX = 0;
  private startY = 0;
  private startTx = 0;
  private startTy = 0;

  constructor(doc: Document, selector: Selector, opts: DragOptions) {
    this.doc = doc;
    this.selector = selector;
    this.opts = opts;
  }

  /**
   * 在选中框上挂载一个移动手柄。
   * 由 App / Selector 在选中变化时调用：传入承载手柄的父节点（覆盖层根）。
   * 每次选中变化前应先调用 removeHandle()。
   */
  attachHandle(parent: HTMLElement) {
    this.removeHandle();
    // 使用 Lucide Hand 图标的 SVG（内联，iframe 内无法使用 Vue 组件）
    const handSvg = `
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"
        viewBox="0 0 24 24" fill="none" stroke="currentColor"
        stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M18 11V6a2 2 0 0 0-4 0v0"/>
        <path d="M14 10V4a2 2 0 0 0-4 0v2"/>
        <path d="M10 10.5V6a2 2 0 0 0-4 0v8"/>
        <path d="M18 8a2 2 0 0 1 4 0v6a8 8 0 0 1-8 8h-2c-2.21 0-4.21-.9-5.66-2.34l-2.34-2.34A2 2 0 0 1 4 14.66V11a2 2 0 0 1 4 0v0"/>
      </svg>`;
    const handle = this.doc.createElement("div");
    handle.className = "leaf-handle leaf-handle-move";
    handle.title = "拖动移动";
    handle.innerHTML = handSvg;
    handle.style.position = "absolute";
    handle.style.pointerEvents = "auto";
    handle.addEventListener("mousedown", (e) => this.onDown(e));
    parent.appendChild(handle);
    this.handle = handle;
    console.log("[drag-debug] DragController.attachHandle", {
      parentId: parent.id,
      parentChildren: parent.children.length,
      handleInDOM: parent.contains(handle),
      handleRect: handle.getBoundingClientRect(),
    });
  }

  removeHandle() {
    this.handle?.remove();
    this.handle = null;
  }

  /** 根据当前选中元素定位手柄。 */
  positionHandle() {
    if (!this.handle || !this.selector.selected) {
      console.log("[drag-debug] positionHandle skipped", { hasHandle: !!this.handle, hasSelected: !!this.selector.selected });
      return;
    }
    const el = this.selector.selected.el;
    const rect = el.getBoundingClientRect();
    const scrollX = this.doc.defaultView?.scrollX ?? 0;
    const scrollY = this.doc.defaultView?.scrollY ?? 0;
    // 居中、贴顶上方（clamp 到 >=0，防止跑出视口）
    const left = rect.left + scrollX + rect.width / 2 - 14;
    const top = Math.max(0, rect.top + scrollY - 28);
    this.handle.style.left = left + "px";
    this.handle.style.top = top + "px";
    console.log("[drag-debug] positionHandle", { elRect: rect.toJSON(), scrollX, scrollY, handleLeft: left, handleTop: top });
  }

  private onDown(e: MouseEvent) {
    const target = this.selector.selected;
    if (!target) return;
    e.preventDefault();
    e.stopPropagation();
    this.dragging = true;
    this.startX = e.clientX;
    this.startY = e.clientY;
    // 读取已有 transform translate 作为起点
    const computed = this.doc.defaultView!.getComputedStyle(target.el).transform;
    const m = computed && computed !== "none" ? computed.match(/matrix\(([^)]+)\)/) : null;
    if (m) {
      const p = m[1].split(",").map(parseFloat);
      this.startTx = p[4] || 0;
      this.startTy = p[5] || 0;
    } else {
      this.startTx = 0;
      this.startTy = 0;
    }
    this.doc.addEventListener("mousemove", this.onMove);
    this.doc.addEventListener("mouseup", this.onUp);
  }

  private onMove = (e: MouseEvent) => {
    if (!this.dragging || !this.selector.selected) return;
    const dx = e.clientX - this.startX;
    const dy = e.clientY - this.startY;
    const tx = this.startTx + dx;
    const ty = this.startTy + dy;
    this.selector.selected.el.style.transform = `translate(${tx}px, ${ty}px)`;
    this.positionHandle();
    this.selector.reposition();
  };

  private onUp = () => {
    this.dragging = false;
    this.doc.removeEventListener("mousemove", this.onMove);
    this.doc.removeEventListener("mouseup", this.onUp);
    this.opts.onEnd();
  };

  detach() {
    this.removeHandle();
    this.doc.removeEventListener("mousemove", this.onMove);
    this.doc.removeEventListener("mouseup", this.onUp);
  }
}
