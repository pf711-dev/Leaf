/**
 * 编辑引擎常量。
 *
 * 所有注入到文档 DOM 上的标记统一用 `leaf-` / `data-leaf-` 前缀，
 * 便于保存序列化时一次性识别并清理，避免污染用户原始 HTML。
 */

/** 注入标记统一前缀。用于：选区 id、覆盖层 class 等。 */
export const LEAF_PREFIX = "leaf";

/** 元素上的稳定引用 id 属性名（值是数字序号，保存时会被剥离）。 */
export const EDIT_ID_ATTR = "data-leaf-edit-id";

/** 元素上的「锁定」标记：锁定元素不可选中、不可编辑（如脚本/样式/页头容器）。 */
export const LOCKED_ATTR = "data-leaf-locked";

/** 覆盖层根容器 class（选中框、拖拽手柄等编辑 UI 都挂在此节点下）。 */
export const OVERLAY_ROOT_ID = "leaf-editor-overlay";

/** 编辑态注入的 `<style>` id（覆盖层样式 + 选中态样式）。 */
export const EDITOR_STYLE_ID = "_leaf_editor_style";

/** 编辑态注入的拦截 `<script>` id（guard.ts 注入）。 */
export const EDITOR_GUARD_ID = "_leaf_editor_guard";

/** 预览态已有的注入节点 id（沿用 html.ts，保存序列化时也要一并剥离）。 */
export const PREVIEW_FIX_ID = "_preview_fix";
export const PREVIEW_NAV_ID = "_preview_nav";

/** 暴露给目标模型的元素分类。借鉴 tmustier/html-editor 的语义目标模型。 */
export type TargetKind = "text" | "container" | "locked";

/**
 * 可直接编辑文本的行内标签。
 * 命中这些标签（或其内部文本节点）时，归类为 `text`：可改字、可走文字格式命令。
 */
export const INLINE_TEXT_TAGS = new Set([
  "P",
  "H1",
  "H2",
  "H3",
  "H4",
  "H5",
  "H6",
  "SPAN",
  "A",
  "B",
  "STRONG",
  "I",
  "EM",
  "U",
  "S",
  "STRIKE",
  "CODE",
  "PRE",
  "LI",
  "TD",
  "TH",
  "BLOCKQUOTE",
  "LABEL",
  "CAPTION",
]);

/**
 * 永远锁定的标签：不可选中、不可编辑。
 * 命中这些（或它们的祖先）时归类为 `locked`。
 */
export const LOCKED_TAGS = new Set([
  "SCRIPT",
  "STYLE",
  "HEAD",
  "META",
  "LINK",
  "TITLE",
  "NOSCRIPT",
]);

/**
 * 容器型标签：本身不可改字，但可整体选中、移动、调整尺寸、在属性面板改布局样式。
 * 命中这些时归类为 `container`。
 */
export const CONTAINER_TAGS = new Set([
  "DIV",
  "SECTION",
  "ARTICLE",
  "HEADER",
  "FOOTER",
  "MAIN",
  "ASIDE",
  "NAV",
  "FIGURE",
  "FIGCAPTION",
  "TABLE",
  "THEAD",
  "TBODY",
  "TFOOT",
  "TR",
  "UL",
  "OL",
  "FORM",
  "FIELDSET",
  "CARD", // 常见语义化自定义元素
]);
