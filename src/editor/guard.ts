/**
 * 危险操作拦截（best-effort）。
 *
 * ⚠️ 技术现实：同域 iframe（allow-same-origin）内的文档脚本同源，一旦运行
 *    就无法被父窗口可靠拦截（能访问 parent.document）。这里只做 best-effort：
 *    拦截大多数文档会触发的导航 / 表单提交 / 弹窗，避免编辑时误触发跳转导致
 *    文档卸载、丢失改动。靠「入库文档皆可信」接受无法防恶意脚本这一权衡。
 *
 * 实现方式：在编辑文档的 contentDocument 上注入一段内联脚本（运行在文档内），
 * 覆盖 window.open / 拦截表单提交 / 拦截 a[href] 默认导航。
 */
import { EDITOR_GUARD_ID } from "./constants";

/**
 * 返回需要注入到编辑文档内的拦截脚本字符串。
 * 脚本以 IIFE 形式立即执行，无外部依赖。
 */
export function buildGuardScript(): string {
  return `<script id="${EDITOR_GUARD_ID}">
(function(){
  // 拦截 window.open：编辑时不应弹出新窗口
  try { window.open = function(){ return null; }; } catch(e){}

  // 拦截所有表单提交：避免文档内搜索框/按钮触发页面跳转
  document.addEventListener("submit", function(e){
    e.preventDefault();
    e.stopPropagation();
  }, true);

  // 拦截默认链接导航：编辑时点链接不应跳转/卸载文档
  document.addEventListener("click", function(e){
    var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
    if (a && a.getAttribute("target") !== "_blank") {
      e.preventDefault();
    }
  }, true);

  // 拦截 hash 变化导致的状态切换（沿用预览态的策略）
  // 注：此处不阻止 scrollIntoView，仅阻止默认 hash 导航副作用
})();
<\/script>`;
}

/**
 * 从文档中移除已注入的拦截脚本（保存序列化前调用，避免污染源文件）。
 */
export function removeGuard(doc: Document) {
  doc.getElementById(EDITOR_GUARD_ID)?.remove();
}
