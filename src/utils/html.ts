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
  const doc = new DOMParser().parseFromString(html, "text/html");

  const toc = doc.querySelector(".toc");
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
export function preparePreviewHtml(html: string, tocItems: TocItem[]): string {
  const anchorIds = JSON.stringify(tocItems.map((t) => t.id));

  const injected = `<style id="_preview_fix">
html,body{margin:0;padding:0;}
html{min-width:1024px;}
.toc{display:none !important;}
.layout{grid-template-columns:1fr !important;}
/* 滚动条：恢复 macOS 原生 Overlay 行为（默认不显示，滚动时浮现、不占宽度）。
   清除被导入文档可能自定义的 ::-webkit-scrollbar 样式，横向纵向一致。 */
*{scrollbar-width:auto;}
::-webkit-scrollbar{width:auto;height:auto;background:transparent;}
::-webkit-scrollbar-thumb{background:initial;}
::-webkit-scrollbar-track{background:initial;}
::-webkit-scrollbar-corner{background:transparent;}
</style>
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

  if (/<\/head>/i.test(html)) {
    return html.replace(/<\/head>/i, `${injected}$&`);
  }
  if (/<body/i.test(html)) {
    return html.replace(/<body/i, `${injected}$&`);
  }
  return injected + html;
}
