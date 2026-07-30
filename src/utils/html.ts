/** 目录项：锚点 ID + 显示文本 */
export interface TocItem {
  id: string;
  text: string;
}

/**
 * 从 HTML 中提取目录项。
 *
 * 策略：查找 .toc 容器（兼容 nav.toc / aside.toc 等写法），
 * 收集其中所有 href="#anchor" 的链接。
 */
export function extractToc(html: string): TocItem[] {
  // 剥离前导 UTF-8 BOM（U+FEFF）。Windows WebView2 的 DOMParser 会把 BOM 当作
  // 文档根文本节点保留，导致 <html> 解析结构偏移、querySelector(".toc") 命中失败。
  // （Rust 端 read_file_inlined 已剥离一次，这里做防御性兜底。）
  const cleaned = html.charCodeAt(0) === 0xfeff ? html.slice(1) : html;
  const doc = new DOMParser().parseFromString(cleaned, "text/html");

  // 优先在 <body> 内查找（某些解析器可能将 .toc 放错了层级）
  let toc = doc.querySelector(".toc");
  if (!toc && doc.body) {
    // fallback：在 body 内查找所有含 toc 类的元素
    const allTocInBody = doc.body.querySelectorAll('[class]');
    for (const el of allTocInBody) {
      const cls = el.getAttribute("class") || "";
      if (/\btoc\b/i.test(cls)) {
        toc = el;
        break;
      }
    }
  }

  if (toc) {
    const links = Array.from(toc.querySelectorAll('a[href^="#"]'));
    const items = links
      .map((a) => ({
        id: (a.getAttribute("href") || "").substring(1),
        text: (a.textContent || "").trim(),
      }))
      .filter((item) => item.id && item.text);
    if (items.length > 0) return items;
  }

  return [];
}

/**
 * 处理 HTML 内容，使其在预览 iframe 中正确渲染。
 *
 * - 隐藏文档自带的 .toc 目录（由我们的浮动面板接管）
 * - 修复因隐藏 .toc 导致的两栏网格空列
 * - 设置 html 最小宽度，防止响应式折叠
 * - 注入脚本：响应父窗口的滚动指令 + 滚动时回报当前章节
 */
/**
 * 预览/编辑共用的文档修正样式。
 * 隐藏文档自带 .toc、修两栏 grid、隐藏滚动条、PDF 导出保留背景色。
 * 编辑态也需要这套样式，故抽出共享。
 */
const SHARED_FIX_STYLE = `<style id="_preview_fix">
html,body{margin:0;padding:0;}
/* min-width:auto：让文档按视口宽度自适应，避免窄窗口下强制出现
   横向滚动条（横向滚动条会在深色文档底部透出一条白线，并在左下/右下
   角形成白色矩形）。文档内容用 overflow-x:hidden 兜底防溢出。 */
html{min-width:auto;overflow-x:hidden;}
.toc{display:none !important;}
.layout{grid-template-columns:1fr !important;}
/* 滚动条：完全隐藏（纵向 + 横向 + 角块），消除预览/演示时右侧的竖条轨道。
   滚动能力保留——overflow 仍为默认 auto/scroll，滚轮、触控板、键盘均可滚动。
   scrollbar-width:none（标准/Firefox）+ ::-webkit-scrollbar{display:none}（WebKit/Chromium）
   双管齐下，覆盖 WKWebView 与 WebView2，并压制文档自定义的滚动条样式。 */
*{scrollbar-width:none !important;}
::-webkit-scrollbar{display:none !important;width:0 !important;height:0 !important;}
::-webkit-scrollbar-thumb,
::-webkit-scrollbar-track,
::-webkit-scrollbar-track-piece,
::-webkit-scrollbar-corner{display:none !important;}
/* PDF 导出：强制保留彩色背景。
   浏览器/WebView 默认省墨会丢弃 background-color/background-image，
   导致用户文档（Tailwind 彩色卡片、ECharts 主题等）打印后变白。
   print-color-adjust:exact（标准）+ -webkit-print-color-adjust:exact（Chromium）
   双保险，配合 Windows 端 PrintSettings.ShouldPrintBackgrounds=true。 */
@media print {
  * {
    -webkit-print-color-adjust: exact !important;
    print-color-adjust: exact !important;
  }
}
</style>`;

/**
 * 把注入内容插进 HTML 文档（优先 </head> 前，其次 <body 前，否则最前）。
 */
function injectIntoHtml(html: string, injected: string): string {
  if (/<\/head>/i.test(html)) {
    return html.replace(/<\/head>/i, `${injected}$&`);
  }
  if (/<body/i.test(html)) {
    return html.replace(/<body/i, `${injected}$&`);
  }
  return injected + html;
}

export function preparePreviewHtml(html: string, tocItems: TocItem[]): string {
  const anchorIds = JSON.stringify(tocItems.map((t) => t.id));

  const injected = `${SHARED_FIX_STYLE}
<script id="_preview_nav">
(function(){
  var ids=${anchorIds};
  // 字号增减。macOS WKWebView sandbox 允许 execCommand("insertHTML")，
  // Windows WebView2 sandbox 则允许 Range API。两个都试，自动 fallback。
  var STEP=2;
  var MIN_PX=10,MAX_PX=72;
  var _lastMethod=null; // 记录上次成功的方法，避免每次都试
  function bumpFontSize(dir){
    var sel=window.getSelection();
    if(!sel||sel.rangeCount===0||sel.isCollapsed)return;
    var range=sel.getRangeAt(0);
    var sc=range.startContainer;
    var refEl=(sc.nodeType===1)?sc:sc.parentElement;
    if(!refEl)return;
    var curPx=parseFloat(getComputedStyle(refEl).fontSize)||16;
    var nextPx=Math.max(MIN_PX,Math.min(MAX_PX,Math.round(curPx)+dir*STEP));
    if(nextPx===Math.round(curPx))nextPx+=dir;
    nextPx=Math.max(MIN_PX,Math.min(MAX_PX,nextPx));
    var frag=range.cloneContents();
    var tmp=document.createElement("div");
    tmp.appendChild(frag);
    var inner=tmp.innerHTML;
    var span=document.createElement("span");
    span.style.cssText="font-size:"+nextPx+"px !important;line-height:1.4 !important;";
    span.innerHTML=inner;
    // 方法 A: Range API（Windows WebView2 可用）
    function tryRange(){
      range.deleteContents();
      range.insertNode(span);
      var nr=document.createRange();
      nr.selectNodeContents(span);
      sel.removeAllRanges();
      sel.addRange(nr);
      return true;
    }
    // 方法 B: execCommand（macOS WKWebView 可用）
    function tryExec(){
      var mark="_lf"+Date.now();
      span.setAttribute("data-leaf-fs",mark);
      var html='<span data-leaf-fs="'+mark+'" style="'+span.style.cssText+'">'+inner+'</span>';
      document.execCommand("insertHTML",false,html);
      var el=document.querySelector('[data-leaf-fs="'+mark+'"]');
      if(el){
        var nr=document.createRange();
        nr.selectNodeContents(el);
        sel.removeAllRanges();
        sel.addRange(nr);
        el.removeAttribute("data-leaf-fs");
      }
      return true;
    }
    try{
      if(_lastMethod==="range"){tryRange();}
      else if(_lastMethod==="exec"){tryExec();}
      else{try{tryRange();_lastMethod="range";}catch(e){tryExec();_lastMethod="exec";}}
    }catch(err){
      try{parent.postMessage({type:"bump-error",msg:String(err&&err.message||err)},"*");}catch(e){}
    }
  }
  window.addEventListener("message",function(e){
    var d=e.data;
    if(!d)return;
    if(d.type==="scroll-to"){
      var el=document.getElementById(d.id);
      if(el)el.scrollIntoView({behavior:"smooth",block:"start"});
    }else if(d.type==="edit-mode"){
      // 开关整个文档的可编辑状态
      document.designMode=d.enabled?"on":"off";
    }else if(d.type==="exec"){
      // 执行格式化命令（bold/italic/fontSize/...）
      try{
        if(d.command==="increaseFontSize"||d.command==="decreaseFontSize"){
          bumpFontSize(d.command==="increaseFontSize"?1:-1);
        }else{
          document.execCommand(d.command,false,d.value||null);
        }
      }catch(err){}
    }else if(d.type==="get-html"){
      // 保存：先移除预览专用的注入节点，再序列化 outerHTML 回传父窗口
      var fix=document.getElementById("_preview_fix");
      var nav=document.getElementById("_preview_nav");
      if(fix)fix.remove();
      if(nav)nav.remove();
      var html=document.documentElement.outerHTML;
      // doctype 可能丢失，尽量补回
      var dt=document.doctype?"<!DOCTYPE "+(document.doctype.name||"html")+">":"";
      parent.postMessage({type:"html-content",html:dt+html},"*");
    }else if(d.type==="get-content-height"){
      // PDF 导出前获取文档完整高度，让打印面板能渲染整篇文档
      var h=Math.max(
        document.documentElement.scrollHeight||0,
        document.body?document.body.scrollHeight:0
      );
      parent.postMessage({type:"content-height",height:h},"*");
    }
  });
  // 拦截文档中所有 # 锚点链接点击（内置目录等），
  // 统一用 scrollIntoView 处理，避免 sandbox iframe 中原生 hash
  // 导航触发文档自带 JS 路由/状态导致页面空白。
  document.addEventListener("click",function(e){
    var a=e.target.closest('a[href^="#"]');
    if(a){
      var id=a.getAttribute("href").substring(1);
      if(id){
        var el=document.getElementById(id);
        if(el){
          e.preventDefault();
          el.scrollIntoView({behavior:"smooth",block:"start"});
        }
      }
    }
  },true);
  function report(){
    var pos=window.scrollY+100;
    var active="";
    ids.forEach(function(id){
      var el=document.getElementById(id);
      if(el){
        var top=el.getBoundingClientRect().top+window.scrollY;
        if(top<=pos)active=id;
      }
    });
    parent.postMessage({type:"toc-active",id:active},"*");
  }
  window.addEventListener("scroll",report,{passive:true});
  report();
  // iframe 获得焦点时，Esc 无法冒泡到父窗口，这里转发给父级处理
  window.addEventListener("keydown",function(e){
    if(e.key==="Escape"){parent.postMessage({type:"esc"},"*");}
  });
})();
<\/script>`;

  return injectIntoHtml(html, injected);
}

/**
 * 准备编辑模式的 HTML。
 *
 * 编辑态使用同域 iframe（allow-same-origin），父窗口通过 contentDocument 直接操作 DOM，
 * 因此**不注入**预览态的 postMessage 脚本（_preview_nav）——滚动定位、文字编辑、
 * 序列化保存全部由编辑引擎（src/editor/*）在父窗口侧完成。
 *
 * 仅注入：
 * 1. 共用修正样式（SHARED_FIX_STYLE：隐藏 .toc / 滚动条 / PDF 背景）
 * 2. 危险操作拦截脚本（best-effort：拦截导航/表单/弹窗，见 src/editor/guard.ts）
 *
 * 文档自带的 `<script>`（Tailwind/ECharts 等）会照常运行，保证渲染保真。
 */
export function prepareEditHtml(html: string): string {
  // guard 脚本延迟导入：避免 html.ts（工具模块）直接耦合编辑引擎常量
  // 这里直接内联构建，与 guard.ts 的 buildGuardScript 保持一致
  const guardScript = `<script id="_leaf_editor_guard">
(function(){
  try { window.open = function(){ return null; }; } catch(e){}
  document.addEventListener("submit", function(e){ e.preventDefault(); e.stopPropagation(); }, true);
  document.addEventListener("click", function(e){
    var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
    if (a && a.getAttribute("target") !== "_blank") { e.preventDefault(); }
  }, true);
})();
<\/script>`;
  return injectIntoHtml(html, SHARED_FIX_STYLE + guardScript);
}
