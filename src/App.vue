<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useDocumentsStore } from "./stores/documents";
import { getDocumentPath, readDocumentContent, writeDocumentContent } from "./api/client";
import { extractToc, preparePreviewHtml, type TocItem } from "./utils/html";
import DocumentListItem from "./components/DocumentListItem.vue";
import TocPanel from "./components/TocPanel.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ConflictDialog from "./components/ConflictDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import { enableModernWindowStyle } from "@cloudworxx/tauri-plugin-mac-rounded-corners";
import { PanelLeftOpen, PanelLeftClose, Bold, Italic, Underline, Strikethrough, AlignLeft, AlignCenter, AlignRight, Undo2, Redo2, RotateCcw, ChevronDown, Baseline, AArrowUp, AArrowDown } from "@lucide/vue";
import type { ConflictInfo, Document } from "./types/document";

const store = useDocumentsStore();

// 当前选中的文档（用于右侧预览）
const currentDoc = ref<Document | null>(null);
const currentHtml = ref("");
const loadingContent = ref(false);

// 目录
const tocItems = ref<TocItem[]>([]);
const activeTocId = ref("");

// iframe 引用（用于 postMessage 导航）
const iframeRef = ref<HTMLIFrameElement | null>(null);

const hasDocuments = computed(() => store.documents.length > 0);

// 侧边栏展开/收起（默认展开，不持久化）
const sidebarCollapsed = ref(false);

// 窗口最大化/全屏状态：为 true 时收起红黄绿避让留白，让切换按钮左移与文档标题对齐
const windowMaximized = ref(false);
const appWindow = getCurrentWindow();

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value;
}

// 演示（沉浸式）模式
const presenting = ref(false);
const sidebarWasCollapsed = ref(false);
const presentHintVisible = ref(false);
let presentHintTimer: ReturnType<typeof setTimeout> | null = null;

// 编辑模式：整页可写 + 顶部工具栏。与演示模式互斥。
const editing = ref(false);
const saving = ref(false);

// 工具栏下拉面板状态：'color' | null
const openDropdown = ref<"color" | null>(null);
// 预设文字颜色（飞书风格，8 色 1 行）
const textColors = [
  { value: "#212121", label: "黑色" },
  { value: "#757575", label: "灰色" },
  { value: "#E03440", label: "红色" },
  { value: "#FF7A45", label: "橙色" },
  { value: "#FFC400", label: "黄色" },
  { value: "#00B42A", label: "绿色" },
  { value: "#006AFA", label: "蓝色" },
  { value: "#722ED1", label: "紫色" },
];
// 预设背景色（飞书风格，16 色 2 行 × 8 列；第一项为无填充）
const bgColors = [
  // 第一行：浅色
  { value: "transparent", label: "无填充" },
  { value: "#F2F2F2", label: "浅灰" },
  { value: "#FFECE8", label: "浅红" },
  { value: "#FFF3E8", label: "浅橙" },
  { value: "#FFF7E8", label: "浅黄" },
  { value: "#E8FFEA", label: "浅绿" },
  { value: "#E8F3FF", label: "浅蓝" },
  { value: "#F5E8FF", label: "浅紫" },
  // 第二行：中色
  { value: "#E5E5E5", label: "中灰" },
  { value: "#BFBFBF", label: "深灰" },
  { value: "#FFCCC7", label: "红" },
  { value: "#FFD591", label: "橙" },
  { value: "#FFFB8F", label: "黄" },
  { value: "#B7EB8F", label: "绿" },
  { value: "#ADC6FF", label: "蓝" },
  { value: "#D3ADF7", label: "紫" },
];

function enterPresent() {
  if (editing.value) exitEdit();   // 与编辑模式互斥
  sidebarWasCollapsed.value = sidebarCollapsed.value;
  sidebarCollapsed.value = true;
  presenting.value = true;
  // 短暂提示「按 Esc 退出」，2.5 秒后淡出
  showPresentHint();
}

function exitPresent() {
  presenting.value = false;
  sidebarCollapsed.value = sidebarWasCollapsed.value;
  hidePresentHint();
}

// ---------- 编辑模式 ----------
// 工具栏 → iframe 桥接：所有格式化都通过 postMessage 让 iframe 内执行 execCommand。

/** 进入编辑模式：先退出演示（互斥），再通知 iframe 开 designMode */
function enterEdit() {
  if (presenting.value) exitPresent();
  editing.value = true;
  postToIframe({ type: "edit-mode", enabled: true });
}

/** 退出编辑模式（不保存）：关 designMode */
function exitEdit() {
  postToIframe({ type: "edit-mode", enabled: false });
  editing.value = false;
  openDropdown.value = null;
}

/** 取消编辑：放弃改动，重新加载原始内容 */
function cancelEdit() {
  exitEdit();
  if (currentDoc.value) reloadCurrentDoc();
}

/** 执行一条格式化命令（bold/italic/fontSize/foreColor/...） */
function runFormat(command: string, value?: string) {
  postToIframe({ type: "exec", command, value });
}

/** 切换下拉面板（同一点击关闭，不同则切换） */
function toggleDropdown() {
  openDropdown.value = openDropdown.value === "color" ? null : "color";
}

/** 恢复默认颜色：文字回黑色、背景清除 */
function resetColor() {
  postToIframe({ type: "exec", command: "foreColor", value: "#212121" });
  postToIframe({ type: "exec", command: "hiliteColor", value: "transparent" });
  openDropdown.value = null;
}

/** 保存：向 iframe 请求当前 HTML，回执在 onIframeMessage 里处理 */
async function saveEdit() {
  if (!currentDoc.value || saving.value) return;
  saving.value = true;
  postToIframe({ type: "get-html" });
}

/** 向 iframe 发消息（iframe 未就绪时静默忽略） */
function postToIframe(msg: object) {
  iframeRef.value?.contentWindow?.postMessage(msg, "*");
}

function showPresentHint() {
  presentHintVisible.value = true;
  if (presentHintTimer) clearTimeout(presentHintTimer);
  presentHintTimer = setTimeout(() => {
    presentHintVisible.value = false;
  }, 2500);
}

function hidePresentHint() {
  presentHintVisible.value = false;
  if (presentHintTimer) {
    clearTimeout(presentHintTimer);
    presentHintTimer = null;
  }
}

/** Esc 退出演示 / Cmd+B 切换侧边栏 */
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && presenting.value) {
    exitPresent();
    return;
  }
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "b") {
    e.preventDefault();
    toggleSidebar();
  }
}

/** 更新窗口最大化状态（最大化或全屏任一为真即视为占满屏幕） */
async function refreshMaximized() {
  try {
    const [maximized, fullscreen] = await Promise.all([
      appWindow.isMaximized(),
      appWindow.isFullscreen(),
    ]);
    windowMaximized.value = maximized || fullscreen;
  } catch {
    // 在非 Tauri 环境（如纯浏览器开发）下忽略
  }
}

/** 窗口尺寸/状态变化时刷新最大化标记 */
function onWindowChanged() {
  refreshMaximized();
}

onMounted(() => {
  window.addEventListener("message", onIframeMessage);
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("mousedown", onGlobalMouseDown);
  store.refresh();
  enableModernWindowStyle();
  refreshMaximized();
  const unlistenPromise = appWindow.onResized(onWindowChanged);
  onUnmountedCleanup.push(() => {
    unlistenPromise.then((unlisten) => unlisten());
  });
});

// 收集需要在卸载时清理的副作用（如 Tauri 事件 unlisten）
const onUnmountedCleanup: Array<() => void> = [];

onUnmounted(() => {
  window.removeEventListener("message", onIframeMessage);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("mousedown", onGlobalMouseDown);
  onUnmountedCleanup.forEach((fn) => fn());
});

/** 点击工具栏外部时关闭下拉面板 */
function onGlobalMouseDown(e: MouseEvent) {
  if (!openDropdown.value) return;
  const target = e.target as HTMLElement;
  if (!target.closest(".fmt-dropdown")) {
    openDropdown.value = null;
  }
}

/** 接收 iframe 内脚本回报：章节高亮 / Esc / 编辑保存的 HTML 回执 */
function onIframeMessage(e: MessageEvent) {
  const d = e.data;
  if (!d) return;
  if (d.type === "toc-active") {
    activeTocId.value = d.id || "";
  } else if (d.type === "esc") {
    // iframe 获得焦点时 Esc 无法冒泡到父窗口，由注入脚本转发过来
    if (editing.value) exitEdit();
    else if (presenting.value) exitPresent();
  } else if (d.type === "html-content" && saving.value) {
    // 保存回执：写回库文件
    onHtmlContentForSave(d.html);
  }
}

/** 处理 iframe 返回的编辑后 HTML：写库 → 刷新预览 → 退出编辑 */
async function onHtmlContentForSave(html: string) {
  const doc = currentDoc.value;
  if (!doc) {
    saving.value = false;
    return;
  }
  try {
    await writeDocumentContent(doc.libraryPath, html);
    exitEdit();
    await reloadCurrentDoc();
  } catch (err) {
    console.error("保存失败:", err);
  } finally {
    saving.value = false;
  }
}

// 导入冲突弹窗
const conflictVisible = ref(false);
const conflictList = ref<ConflictInfo[]>([]);
const pendingPaths = ref<string[]>([]);

async function pickAndImport() {
  const selected = await open({
    multiple: true,
    filters: [{ name: "HTML", extensions: ["html", "htm"] }],
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await tryImport(paths);
}

/** 预检冲突 → 无冲突直接导入，有冲突弹窗等用户选择 */
async function tryImport(paths: string[]) {
  const conflicts = await store.checkConflicts(paths);
  if (conflicts.length > 0) {
    pendingPaths.value = paths;
    conflictList.value = conflicts;
    conflictVisible.value = true;
  } else {
    await store.importWithResolution(paths);
  }
}

/** 用户在冲突弹窗选「全部跳过」 */
async function onConflictSkip() {
  conflictVisible.value = false;
  // 跳过所有撞名文件，只导入非冲突项
  await store.importWithResolution(pendingPaths.value, "skip");
  pendingPaths.value = [];
  conflictList.value = [];
}

/** 用户在冲突弹窗选「全部覆盖」 */
async function onConflictOverwrite() {
  conflictVisible.value = false;
  await store.importWithResolution(pendingPaths.value, "overwrite");
  pendingPaths.value = [];
  conflictList.value = [];
}

/** 用户在冲突弹窗选「取消」 */
function onConflictCancel() {
  conflictVisible.value = false;
  pendingPaths.value = [];
  conflictList.value = [];
}

/** 复制当前文档的库内绝对路径到剪贴板 */
async function copyPath(doc: Document) {
  try {
    const path = await getDocumentPath(doc.libraryPath);
    await writeText(path);
  } catch (e) {
    console.error("复制路径失败:", e);
  }
}

async function selectDoc(doc: Document) {
  if (currentDoc.value?.id === doc.id) return;
  // 切换文档前若在编辑，先退出编辑（不自动保存）
  if (editing.value) exitEdit();
  currentDoc.value = doc;
  await reloadCurrentDoc();
}

/** 重新加载当前文档内容到预览（首次选中、取消编辑、保存后都用） */
async function reloadCurrentDoc() {
  const doc = currentDoc.value;
  if (!doc) return;
  loadingContent.value = true;
  currentHtml.value = "";
  activeTocId.value = "";
  try {
    const raw = await readDocumentContent(doc.libraryPath);
    tocItems.value = extractToc(raw);
    currentHtml.value = preparePreviewHtml(raw, tocItems.value);
  } finally {
    loadingContent.value = false;
  }
}

/** 点击目录项 → 通知 iframe 滚动到对应锚点 */
function navigateToc(id: string) {
  iframeRef.value?.contentWindow?.postMessage({ type: "scroll-to", id }, "*");
}

// 删除确认弹窗
const confirmVisible = ref(false);
const pendingDelete = ref<Document | null>(null);

function removeDoc(doc: Document) {
  pendingDelete.value = doc;
  confirmVisible.value = true;
}

async function doDelete() {
  const doc = pendingDelete.value;
  if (!doc) return;
  confirmVisible.value = false;
  await store.remove(doc.id, doc.libraryPath);
  if (currentDoc.value?.id === doc.id) {
    currentDoc.value = null;
    currentHtml.value = "";
    tocItems.value = [];
  }
  pendingDelete.value = null;
}

// 右键菜单
const contextMenuVisible = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const contextMenuDoc = ref<Document | null>(null);

/** 列表项右键 → 弹出菜单 */
function onItemContextMenu(doc: Document, e: MouseEvent) {
  e.preventDefault();
  contextMenuDoc.value = doc;
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuVisible.value = true;
}

function onContextCopyPath() {
  contextMenuVisible.value = false;
  const doc = contextMenuDoc.value;
  if (doc) copyPath(doc);
  contextMenuDoc.value = null;
}

function onContextDelete() {
  contextMenuVisible.value = false;
  const doc = contextMenuDoc.value;
  if (doc) removeDoc(doc);
  contextMenuDoc.value = null;
}

// 拖拽导入：拖到窗口任意位置
function onDrop(e: DragEvent) {
  e.preventDefault();
  const files = e.dataTransfer?.files;
  if (!files) return;
  // 浏览器拖拽拿不到绝对路径，MVP 阶段提示用户用按钮导入
}
function onDragOver(e: DragEvent) {
  e.preventDefault();
}
</script>

<template>
  <div
    class="app"
    :class="{ presenting: presenting, maximized: windowMaximized }"
    @drop="onDrop"
    @dragover="onDragOver"
  >
    <!-- 顶部栏（data-tauri-drag-region 使整个顶栏可拖动窗口） -->
    <header class="topbar" data-tauri-drag-region>
      <div class="topbar-left">
        <div class="topbar-traffic-pad" aria-hidden="true" data-tauri-drag-region></div>
        <button
          class="icon-btn"
          :title="sidebarCollapsed ? '展开侧边栏' : '收起侧边栏'"
          :aria-expanded="!sidebarCollapsed"
          :aria-label="sidebarCollapsed ? '展开侧边栏' : '收起侧边栏'"
          aria-controls="sidebar"
          @click="toggleSidebar"
        >
          <Transition name="toggle-icon" mode="out-in">
            <PanelLeftClose v-if="!sidebarCollapsed" key="close" :size="14" :stroke-width="1.5" />
            <PanelLeftOpen v-else key="open" :size="14" :stroke-width="1.5" />
          </Transition>
        </button>
      </div>
      <div class="topbar-right">
        <!-- 编辑模式：常驻工具栏（格式化按钮 + 保存/取消） -->
        <template v-if="editing">
          <div class="edit-toolbar" @mousedown.prevent>
            <!-- 1. 字号：增大 / 减小 -->
            <button class="fmt-btn" title="增大字号" @click="runFormat('increaseFontSize')"><AArrowUp :size="16" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="减小字号" @click="runFormat('decreaseFontSize')"><AArrowDown :size="16" :stroke-width="1.8" /></button>

            <!-- 2. 加粗 -->
            <button class="fmt-btn" title="加粗" @click="runFormat('bold')"><Bold :size="15" :stroke-width="1.8" /></button>
            <!-- 3. 删除线 -->
            <button class="fmt-btn" title="删除线" @click="runFormat('strikeThrough')"><Strikethrough :size="15" :stroke-width="1.8" /></button>
            <!-- 4. 倾斜 -->
            <button class="fmt-btn" title="倾斜" @click="runFormat('italic')"><Italic :size="15" :stroke-width="1.8" /></button>
            <!-- 5. 下划线 -->
            <button class="fmt-btn" title="下划线" @click="runFormat('underline')"><Underline :size="15" :stroke-width="1.8" /></button>

            <span class="fmt-sep"></span>

            <!-- 6. 颜色样式：飞书风预设色板（文字色 + 背景色） -->
            <div class="fmt-dropdown">
              <button class="fmt-trigger" title="颜色" @click="toggleDropdown()">
                <Baseline :size="15" :stroke-width="1.8" />
                <ChevronDown :size="12" :stroke-width="1.8" />
              </button>
              <div v-if="openDropdown === 'color'" class="fmt-menu fmt-color-menu">
                <div class="fmt-color-section">字体颜色</div>
                <div class="fmt-color-grid">
                  <button v-for="c in textColors" :key="'t'+c.value"
                    class="fmt-color-swatch fmt-text-swatch" :style="{ color: c.value }"
                    :title="c.label" @click="runFormat('foreColor', c.value)"
                  >A</button>
                </div>
                <div class="fmt-color-section">背景颜色</div>
                <div class="fmt-color-grid">
                  <button v-for="c in bgColors" :key="'b'+c.value"
                    class="fmt-color-swatch fmt-bg-swatch"
                    :class="{ 'is-none': c.value === 'transparent' }"
                    :style="{ background: c.value === 'transparent' ? undefined : c.value }"
                    :title="c.label" @click="runFormat('hiliteColor', c.value)"
                  >
                    <svg v-if="c.value === 'transparent'" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="5" y1="5" x2="19" y2="19"/><line x1="19" y1="5" x2="5" y2="19"/></svg>
                    <span v-else>A</span>
                  </button>
                </div>
                <div class="fmt-color-footer">
                  <button class="fmt-reset-btn" @click="resetColor">恢复默认</button>
                </div>
              </div>
            </div>

            <span class="fmt-sep"></span>

            <!-- 7. 对齐 -->
            <button class="fmt-btn" title="左对齐" @click="runFormat('justifyLeft')"><AlignLeft :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="居中" @click="runFormat('justifyCenter')"><AlignCenter :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="右对齐" @click="runFormat('justifyRight')"><AlignRight :size="15" :stroke-width="1.8" /></button>

            <span class="fmt-sep"></span>

            <!-- 8-10. 撤销 / 重做 / 重置 -->
            <button class="fmt-btn" title="撤销" @click="runFormat('undo')"><Undo2 :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="重做" @click="runFormat('redo')"><Redo2 :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="清空格式" @click="runFormat('removeFormat')"><RotateCcw :size="15" :stroke-width="1.8" /></button>
          </div>
          <button class="btn" :disabled="saving" @click="cancelEdit">取消</button>
          <button class="btn btn-primary" :disabled="saving" @click="saveEdit">
            {{ saving ? "保存中…" : "保存" }}
          </button>
        </template>

        <!-- 非编辑模式：演示 / 编辑 / 导入 -->
        <template v-else>
          <button class="btn" :disabled="!currentDoc || loadingContent" @click="enterEdit">
            编辑
          </button>
          <button class="btn" :disabled="!currentDoc" @click="enterPresent">
            演示
          </button>
          <button class="btn btn-primary" @click="pickAndImport" :disabled="store.importing > 0">
            {{ store.importing > 0 ? "导入中…" : "导入" }}
          </button>
        </template>
      </div>
    </header>

    <!-- 主内容：左右分栏 -->
    <main class="content">
      <!-- 左侧：文档列表 -->
      <aside id="sidebar" class="sidebar" :class="{ collapsed: sidebarCollapsed }">
        <div class="sidebar-inner">
          <div class="sidebar-head">
            <span>文档</span>
            <span v-if="hasDocuments" class="sidebar-count">{{ store.documents.length }} 篇</span>
          </div>

          <div class="sidebar-body">
            <p v-if="store.loading" class="status">加载中…</p>
            <p v-else-if="store.error" class="status error">{{ store.error }}</p>

            <!-- 空状态 -->
            <div v-else-if="!hasDocuments" class="empty-list">
              <p class="empty-line">暂无文档</p>
              <p class="empty-hint">点击右上角「+ 导入」添加 HTML 文件</p>
            </div>

          <!-- 列表 -->
          <DocumentListItem
            v-else
            v-for="doc in store.documents"
            :key="doc.id"
            :doc="doc"
            :active="currentDoc?.id === doc.id"
            @click="selectDoc(doc)"
            @contextmenu="(doc, event) => onItemContextMenu(doc, event)"
          />
          </div>
        </div>
      </aside>

      <!-- 右侧：预览 -->
      <section class="preview">
        <!-- 有文档选中 -->
        <template v-if="currentDoc">
          <div class="preview-body">
            <!-- 沉浸模式：进入时短暂提示 -->
            <transition name="fade">
              <div v-if="presentHintVisible" class="present-hint">
                按 <kbd>Esc</kbd> 退出演示
              </div>
            </transition>
            <p v-if="loadingContent" class="status">加载中…</p>
            <template v-else-if="currentHtml">
              <iframe
                ref="iframeRef"
                class="preview-iframe"
                :class="{ editing }"
                :srcdoc="currentHtml"
                sandbox="allow-scripts"
              />
              <!-- 浮动目录面板 -->
              <TocPanel
                :items="tocItems"
                :active-id="activeTocId"
                @navigate="navigateToc"
              />
            </template>
          </div>
        </template>

        <!-- 无选中：占位 -->
        <div v-else class="preview-empty">
          <svg class="preview-empty-icon" width="48" height="48" viewBox="0 0 48 48" fill="none">
            <rect x="12" y="8" width="20" height="30" rx="3" stroke="currentColor" stroke-width="1.5"/>
            <path d="M17 18h10M17 23h10M17 28h7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          </svg>
          <p class="preview-empty-text">选择左侧文档以预览</p>
        </div>
      </section>
    </main>

    <!-- 删除确认弹窗 -->
    <ConfirmDialog
      :visible="confirmVisible"
      :title="`删除「${pendingDelete?.title ?? ''}」？`"
      message="该操作不可撤销，确认删除这篇文档吗？"
      confirm-text="删除"
      :danger="true"
      @confirm="doDelete"
      @cancel="confirmVisible = false; pendingDelete = null"
    />

    <!-- 导入冲突弹窗 -->
    <ConflictDialog
      :visible="conflictVisible"
      :conflicts="conflictList"
      @skip="onConflictSkip"
      @overwrite="onConflictOverwrite"
      @cancel="onConflictCancel"
    />

    <!-- 列表项右键菜单 -->
    <ContextMenu
      :visible="contextMenuVisible"
      :x="contextMenuX"
      :y="contextMenuY"
      @copy-path="onContextCopyPath"
      @delete="onContextDelete"
      @close="contextMenuVisible = false"
    />
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--bg-sidebar);
}

/* ---------- 顶部栏 ---------- */
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  height: 44px;
  background: var(--bg-sidebar);
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
  /* 最大化时左 padding 与侧边栏标题对齐：补偿 .topbar-left 的 gap(10px)，
     使按钮最终左边距 = padding + gap = 14px，与文档标题一致 */
  transition: padding 0.12s cubic-bezier(0.4, 0, 0.2, 1);
}
.app.maximized .topbar {
  padding-left: 0px;
}
.topbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}
/* macOS 红黄绿按钮的结构性留白（含左侧窗口内边距）。
   最大化/全屏时红黄绿不再需要避让，留白收缩为 0，切换按钮随之左移与文档标题对齐。 */
.topbar-traffic-pad {
  width: 60px;
  flex-shrink: 0;
  height: 100%;
  transition: width 0.12s cubic-bezier(0.4, 0, 0.2, 1);
}
.app.maximized .topbar-traffic-pad {
  width: 0;
}
/* 最大化时按钮再往左微调 1px，与文档标题精确对齐 */
.app.maximized .topbar-left .icon-btn {
  margin-left: -1px;
}
.topbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}
.count {
  font-size: 12px;
  color: var(--text-faint);
}

/* ---------- 按钮（Notion 风） ---------- */
.btn {
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 500;
  color: var(--text-dim);
  background: transparent;
  border: 1px solid var(--border-strong);
  cursor: pointer;
  transition: background 0.1s;
}
.btn:hover {
  background: var(--bg-hover);
}
.btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.btn-primary {
  color: #ffffff;
  background: var(--accent-blue);
  border-color: var(--accent-blue);
}
.btn-primary:hover {
  background: var(--accent-blue-hover);
  border-color: var(--accent-blue-hover);
}
.btn-ghost {
  border-color: transparent;
}
.btn-danger:hover {
  color: var(--danger);
  background: var(--bg-hover);
}

/* ---------- 编辑工具栏 ---------- */
.edit-toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
}
.fmt-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 26px;
  height: 26px;
  padding: 0 5px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.1s;
}
.fmt-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.fmt-sep {
  width: 1px;
  height: 16px;
  margin: 0 4px;
  background: var(--border);
}
/* 下拉触发按钮（字号、颜色） */
.fmt-dropdown {
  position: relative;
}
.fmt-trigger {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 26px;
  padding: 0 6px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.1s;
}
.fmt-trigger:hover {
  background: var(--bg-hover);
  color: var(--text);
}
/* 下拉菜单浮层 */
.fmt-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: 50;
  min-width: 92px;
  padding: 4px;
  background: var(--bg);
  border-radius: 8px;
  box-shadow: rgba(15, 15, 15, 0.05) 0 0 0 1px,
    rgba(15, 15, 15, 0.1) 0 3px 6px, rgba(15, 15, 15, 0.2) 0 9px 24px;
}
.fmt-menu-item {
  display: block;
  width: 100%;
  padding: 5px 10px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12px;
  text-align: left;
  cursor: pointer;
}
.fmt-menu-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.fmt-menu-item.active {
  color: var(--accent-blue);
  font-weight: 500;
}
/* 颜色面板 */
.fmt-color-menu {
  min-width: 240px;
}
.fmt-color-section {
  padding: 8px 6px 4px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-dim);
}
.fmt-color-section:first-child {
  padding-top: 4px;
}
.fmt-color-grid {
  display: grid;
  grid-template-columns: repeat(8, 1fr);
  gap: 3px;
  padding: 0 6px;
}
/* 文字色块：只显示带颜色的 A */
.fmt-text-swatch {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 24px;
  border-radius: 4px;
  border: none;
  background: transparent;
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
  transition: background 0.1s, transform 0.1s;
}
.fmt-text-swatch:hover {
  background: var(--bg-hover);
  transform: scale(1.1);
}
/* 背景色块：填充背景 + A */
.fmt-bg-swatch {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 24px;
  border-radius: 4px;
  border: 1px solid var(--border);
  font-size: 13px;
  font-weight: 700;
  color: #555;
  cursor: pointer;
  transition: transform 0.1s;
}
.fmt-bg-swatch:hover {
  transform: scale(1.1);
}
.fmt-bg-swatch.is-none {
  color: var(--text-faint);
  background: var(--bg);
}
/* 面板底部恢复默认 */
.fmt-color-footer {
  padding: 8px 6px 6px;
  margin-top: 4px;
  border-top: 1px solid var(--border);
}
.fmt-reset-btn {
  width: 100%;
  padding: 5px 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.1s;
}
.fmt-reset-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}

/* ---------- 主内容分栏 ---------- */
.content {
  flex: 1;
  display: flex;
  min-height: 0;
}

/* 左侧列表（Notion 暖米白侧栏） */
.sidebar {
  width: 260px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;            /* 收起时裁切内容，配合 .sidebar-inner 固定宽度 */
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  transition: width 0.25s ease, border-color 0.25s ease;
}
.sidebar.collapsed {
  width: 0;
  border-right-color: transparent;
}
.sidebar-inner {
  width: 260px;                /* 内容保持 260px，外层收缩时被裁切而非被挤压 */
  flex: 1;
  display: flex;
  flex-direction: column;
}
/* 通用图标按钮（侧边栏切换、演示等共用） */
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.1s;
}
.icon-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.icon-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
/* 切换按钮图标淡入淡出（out-in：旧图标先淡出，再淡入新图标） */
.toggle-icon-enter-active,
.toggle-icon-leave-active {
  transition: opacity 0.15s ease;
}
.toggle-icon-enter-from,
.toggle-icon-leave-to {
  opacity: 0;
}
.sidebar-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 14px 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-dim);
}
.sidebar-count {
  font-weight: 400;
  color: var(--text-faint);
}
.sidebar-body {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px 8px;
}

.status {
  padding: 20px 8px;
  color: var(--text-faint);
  font-size: 13px;
}
.status.error {
  color: var(--danger);
}

.empty-list {
  padding: 24px 8px;
}
.empty-line {
  margin: 0 0 6px;
  color: var(--text-dim);
  font-size: 14px;
}
.empty-hint {
  margin: 0;
  color: var(--text-faint);
  font-size: 12px;
}

/* 右侧预览 */
.preview {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg);
}
.preview-body {
  flex: 1;
  min-height: 0;
  position: relative;
  background: #ffffff;
}

/* 沉浸模式：隐藏顶栏（侧栏已由 sidebarCollapsed 收起） */
.app.presenting .topbar {
  display: none;
}
/* 沉浸模式：进入时的短暂提示（toast） */
.present-hint {
  position: absolute;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 16px;
  border-radius: 8px;
  background: var(--bg-sidebar);
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
  font-size: 13px;
  color: var(--text-dim);
  z-index: 20;
  pointer-events: none;
  white-space: nowrap;
}
.present-hint kbd {
  display: inline-block;
  padding: 1px 6px;
  margin: 0 2px;
  border-radius: 3px;
  background: var(--bg-active);
  font-family: var(--font-sans);
  font-size: 11px;
  color: var(--text);
}
.preview-iframe {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: none;
  transition: box-shadow 0.2s ease;
}
/* 编辑模式：iframe 外框蓝色细线，提示当前可编辑 */
.preview-iframe.editing {
  box-shadow: inset 0 0 0 2px var(--accent-blue);
}

/* 预览空状态 */
.preview-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
}
.preview-empty-icon {
  color: var(--text-faint);
  opacity: 0.6;
}
.preview-empty-text {
  margin: 0;
  font-size: 14px;
  color: var(--text-faint);
}
</style>
