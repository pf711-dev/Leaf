/**
 * 语义目标模型（Semantic Target Model）。
 *
 * 借鉴 tmustier/html-editor 的设计：把每个被点击的 DOM 元素解析成一种
 * 「能力类型」，统一驱动选择器 / 文字编辑 / 拖拽 / 属性面板的行为，
 * 比起「整个文档 contenteditable」健壮得多——能精确划出可编辑区边界，
 * 锁定页头 / 脚本 / 样式等非内容区。
 *
 * 三类目标：
 * - `text`：可直接改字的行内内容（段落 / 标题 / 链接 / 单元格…）
 * - `container`：容器型块，不可改字但可选 / 可移动 / 可调尺寸
 * - `locked`：脚本 / 样式 / 页头等，完全只读
 */
import {
  EDIT_ID_ATTR,
  LOCKED_ATTR,
  LOCKED_TAGS,
  CONTAINER_TAGS,
  INLINE_TEXT_TAGS,
  type TargetKind,
} from "./constants";

/** 一个被解析过的选中目标。 */
export interface Target {
  /** 对应的 DOM 元素（已打上稳定 edit-id）。 */
  el: HTMLElement;
  /** 能力分类。 */
  kind: TargetKind;
  /** 稳定引用 id（来自 data-leaf-edit-id）。 */
  editId: string;
}

/**
 * 自增计数器：为每个被标记元素分配稳定 id。
 * 用 `relPath` 维度隔离，切换文档时由 attachEditor 重置。
 */
let _editIdCounter = 0;

export function resetEditIdCounter() {
  _editIdCounter = 0;
}

/**
 * 给元素打上稳定 edit-id（若尚未有）。
 * 返回该 id。id 仅在内存中用于稳定引用，保存序列化时会被剥离。
 */
export function ensureEditId(el: HTMLElement): string {
  let id = el.getAttribute(EDIT_ID_ATTR);
  if (!id) {
    id = String(++_editIdCounter);
    el.setAttribute(EDIT_ID_ATTR, id);
  }
  return id;
}

/** 读取元素已有的 edit-id（不分配新的）。 */
export function readEditId(el: HTMLElement): string | null {
  return el.getAttribute(EDIT_ID_ATTR);
}

/** 判定元素是否被显式锁定（含 data-leaf-locked 标记）。 */
function isExplicitlyLocked(el: HTMLElement): boolean {
  return el.hasAttribute(LOCKED_ATTR);
}

/**
 * 解析一个元素的语义类型。
 *
 * 优先级：
 * 1. 显式锁定标记 / 锁定标签祖先 → `locked`
 * 2. 行内文本标签 → `text`
 * 3. 容器标签 → `container`
 * 4. 兜底：非锁定标签视为 `text`（允许就地改字），否则 `locked`
 */
export function classify(el: HTMLElement): TargetKind {
  // 显式锁定优先（作者标记或脚本/样式等）
  if (isExplicitlyLocked(el)) return "locked";

  const tag = el.tagName;

  // 锁定标签直接判 locked（含 <script>/<style>/<head> 等）
  if (LOCKED_TAGS.has(tag)) return "locked";

  // 行内文本优先：能改字的最小单元
  if (INLINE_TEXT_TAGS.has(tag)) return "text";

  // 容器型：可选 / 可移动 / 可调尺寸
  if (CONTAINER_TAGS.has(tag)) return "container";

  // 兜底：可渲染的非锁定元素（如 <img>/<svg>/<button>）当 container 处理——
  // 不可改字但可整体选中改属性；没有文字内容也不应进入 text 改字流程。
  return "container";
}

/**
 * 从一个点击点（event.target）解析出目标。
 *
 * 规则：
 * - 向上找到第一个「可交互」的元素节点（跳过纯文本节点）。
 * - 若该元素或其祖先处于锁定区（LOCKED_TAGS / 显式锁定），返回 null（点空）。
 * - 否则按 classify 分类，并分配 edit-id。
 *
 * 返回 null 表示该点击点没有可编辑目标（点到了锁定区或覆盖层）。
 */
export function resolveTarget(rawTarget: EventTarget | null): Target | null {
  // 注意：编辑事件来自 iframe 的 contentDocument，其 Element 实例属于 iframe 窗口的
  // Element 构造器，用 `instanceof Element`（父窗口构造器）会判 false。
  // 改用 nodeType === 1 判定元素节点，跨文档可靠。
  if (!rawTarget || (rawTarget as Node).nodeType !== 1) return null;
  const raw = rawTarget as HTMLElement;

  // 事件可能发自覆盖层自身——覆盖层元素一律视为「点空」
  if (raw.id === "leaf-editor-overlay") return null;
  if (raw.closest?.("#leaf-editor-overlay")) return null;

  // 找到最近的可交互元素（target 可能是文本节点，但其 parentNode 已在 Element 上）
  let el: HTMLElement | null = raw.closest
    ? (raw.closest(
        "a,abbr,b,blockquote,button,caption,code,div,em,figcaption,figure," +
          "footer,h1,h2,h3,h4,h5,h6,header,i,img,label,li,main,nav,ol,p," +
          "pre,section,span,strong,sub,sup,svg,table,td,th,tr,u,ul",
      ) as HTMLElement | null)
    : null;
  if (!el) {
    // 退而求其次：直接用 target 本身（若它是元素）
    el = raw;
  }
  if (!el || el.nodeType !== 1) return null;

  // 锁定区：向上检查是否有锁定标签祖先
  if (el.closest("script,style,head,meta,link,title,noscript")) return null;
  if (isExplicitlyLocked(el) || el.closest(`[${LOCKED_ATTR}]`)) return null;

  const kind = classify(el);
  if (kind === "locked") return null;

  const editId = ensureEditId(el);
  return { el, kind, editId };
}

/**
 * 标记整篇文档的可编辑边界：给锁定区的顶层容器打上 LOCKED_ATTR，
 * 这样点击其内部任何元素都会被 resolveTarget 视为点空。
 *
 * 在 attachEditor 时调用一次即可（锁定区本身不会变化）。
 */
export function markLockedRegions(root: Document) {
  // <head> 整体锁定
  const head = root.head;
  if (head) head.setAttribute(LOCKED_ATTR, "");
}
