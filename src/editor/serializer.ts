/**
 * 保存序列化（Serializer）。
 *
 * 借鉴 joyxiaofan-beep/html-visual-editor 的 html-serializer.js：
 * 克隆 documentElement → 清理所有编辑器注入痕迹 → 补回 doctype → 输出 outerHTML。
 *
 * 需要剥离的「注入痕迹」：
 * - 预览态注入节点：_preview_fix / _preview_nav（沿用 html.ts）
 * - 编辑态注入节点：leaf-editor-overlay / _leaf_editor_style / _leaf_editor_guard
 * - 元素上的标记属性：data-leaf-edit-id / data-leaf-locked
 * - contenteditable 标记（编辑态临时加的）
 *
 * ⚠️ 已知限制：outerHTML 序列化会丢失原文件的原始缩进/注释/属性顺序。
 *    这是 DOM 序列化的固有特性。保留原始格式的「定向 patch」作为后续优化。
 */
import {
  EDIT_ID_ATTR,
  LOCKED_ATTR,
  OVERLAY_ROOT_ID,
  EDITOR_STYLE_ID,
  EDITOR_GUARD_ID,
  PREVIEW_FIX_ID,
  PREVIEW_NAV_ID,
} from "./constants";

/** 所有需要从文档中移除的「注入节点」id。 */
const INJECTED_NODE_IDS = [
  OVERLAY_ROOT_ID,
  EDITOR_STYLE_ID,
  EDITOR_GUARD_ID,
  PREVIEW_FIX_ID,
  PREVIEW_NAV_ID,
];

/** 所有需要从元素上剥离的「注入属性」。 */
const INJECTED_ATTRS = [EDIT_ID_ATTR, LOCKED_ATTR, "contenteditable"];

/**
 * 序列化当前编辑文档为可保存的 HTML 字符串。
 *
 * @param doc 编辑 iframe 的 contentDocument（同域可访问）
 * @returns 干净的 HTML 字符串（含 doctype）
 */
export function serializeDocument(doc: Document): string {
  // 克隆整篇文档，避免改动内存中的真实 DOM
  const cloneRoot = doc.documentElement.cloneNode(true) as HTMLElement;

  // 1. 移除所有注入节点
  for (const id of INJECTED_NODE_IDS) {
    cloneRoot.querySelector(`#${id}`)?.remove();
  }

  // 2. 剥离所有元素上的注入属性
  const all = cloneRoot.querySelectorAll(`[${INJECTED_ATTRS.join("],[")}]`);
  all.forEach((el) => {
    for (const attr of INJECTED_ATTRS) {
      el.removeAttribute(attr);
    }
  });

  // 3. 补回 doctype（DOMParser 序列化常丢失）
  const dt = doc.doctype
    ? `<!DOCTYPE ${doc.doctype.name || "html"}>`
    : "<!DOCTYPE html>";

  return `${dt}${cloneRoot.outerHTML}`;
}
