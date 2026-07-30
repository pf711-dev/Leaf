/**
 * 编辑覆盖层（Overlay）。
 *
 * 在编辑文档内注入一个绝对定位的 `<div id="leaf-editor-overlay">`，
 * 承载所有编辑态视觉元素：选中框、悬停高亮、拖拽手柄。
 *
 * 设计规范 B 层：iframe 内引用不到 CSS 变量，用规范对应色值硬编码：
 * - 选中框描边：--accent-blue #0b6e99 → rgba(11,110,153,0.5)
 * - 选中框填充：hover 叠层理念 → rgba(11,110,153,0.06)
 * - 悬停高亮：--bg-hover → rgba(55,53,47,0.06)
 * - 拖拽手柄：--accent 暖近黑 → rgb(55,53,47)
 *
 * 覆盖层用 position:absolute 跟随滚动，pointer-events 由各子元素自行控制，
 * 让点击能穿透到文档元素（选择器靠文档上的事件监听命中目标，而非覆盖层）。
 */
import { OVERLAY_ROOT_ID, EDITOR_STYLE_ID } from "./constants";

/**
 * 注入覆盖层样式（选中框 / 手柄）。仅注入一次。
 */
export function injectOverlayStyle(doc: Document) {
  if (doc.getElementById(EDITOR_STYLE_ID)) return;
  const style = doc.createElement("style");
  style.id = EDITOR_STYLE_ID;
  style.textContent = `
#${OVERLAY_ROOT_ID} {
  position: absolute;
  top: 0; left: 0;
  pointer-events: none;
  z-index: 2147483646;
  width: 0; height: 0;
}
/* 选中框：跟随选中元素的描边框 */
.leaf-selection-box {
  position: absolute;
  border: 1px solid rgba(11,110,153,0.5);
  background: rgba(11,110,153,0.06);
  pointer-events: none;
  transition: all 0.08s ease;
  box-sizing: border-box;
}
/* 悬停高亮：鼠标悬停目标时的淡色叠层 */
.leaf-hover-box {
  position: absolute;
  background: rgba(55,53,47,0.06);
  pointer-events: none;
  transition: all 0.05s ease;
  box-sizing: border-box;
}
/* 拖拽手柄：选中框四角/四边的小圆点（--accent 暖近黑） */
.leaf-handle {
  position: absolute;
  width: 8px; height: 8px;
  background: rgb(55,53,47);
  border: 1px solid #fff;
  border-radius: 50%;
  pointer-events: auto;
  cursor: nwse-resize;
  transition: transform 0.1s;
}
.leaf-handle:hover { transform: scale(1.25); }
.leaf-handle.leaf-handle-e, .leaf-handle.leaf-handle-w { cursor: ew-resize; }
.leaf-handle.leaf-handle-n, .leaf-handle.leaf-handle-s { cursor: ns-resize; }
.leaf-handle.leaf-handle-nesw { cursor: nesw-resize; }
/* 移动手柄（手形图标，白色圆形浮动按钮，悬停在选中框上方中央） */
.leaf-handle.leaf-handle-move {
  cursor: grab;
  width: 28px; height: 28px;
  border-radius: 50%;
  background: #fff;
  color: rgb(55,53,47);
  display: flex; align-items: center; justify-content: center;
  box-shadow: rgba(15,15,15,0.05) 0 0 0 1px,
              rgba(15,15,15,0.1) 0 2px 4px,
              rgba(15,15,15,0.15) 0 6px 12px;
  transition: box-shadow 0.15s, transform 0.15s;
}
.leaf-handle.leaf-handle-move:hover {
  box-shadow: rgba(15,15,15,0.08) 0 0 0 1px,
              rgba(15,15,15,0.15) 0 4px 8px,
              rgba(15,15,15,0.2) 0 10px 20px;
}
.leaf-handle.leaf-handle-move:active {
  cursor: grabbing;
}
`;
  doc.head?.appendChild(style);
}

/**
 * 创建覆盖层根节点并插入 body。返回根节点。
 * 若已存在则直接返回。
 */
export function ensureOverlay(doc: Document): HTMLElement {
  let root = doc.getElementById(OVERLAY_ROOT_ID);
  if (root) return root;
  root = doc.createElement("div");
  root.id = OVERLAY_ROOT_ID;
  doc.body.appendChild(root);
  return root;
}

/** 移除整个覆盖层（保存 / 退出编辑时调用）。 */
export function removeOverlay(doc: Document) {
  doc.getElementById(OVERLAY_ROOT_ID)?.remove();
  doc.getElementById(EDITOR_STYLE_ID)?.remove();
}

/** 根据 doc 内某元素的 getBoundingClientRect 重定位一个 box 元素。 */
export function positionBox(box: HTMLElement, el: HTMLElement, doc: Document) {
  const rect = el.getBoundingClientRect();
  // 覆盖层 position:absolute 定位基于 body，需加上滚动偏移
  const scrollX = doc.defaultView?.scrollX ?? 0;
  const scrollY = doc.defaultView?.scrollY ?? 0;
  box.style.left = rect.left + scrollX + "px";
  box.style.top = rect.top + scrollY + "px";
  box.style.width = rect.width + "px";
  box.style.height = rect.height + "px";
}
