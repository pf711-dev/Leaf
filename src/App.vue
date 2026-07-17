<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useDocumentsStore } from "./stores/documents";
import { getDocumentPath, readDocumentContent } from "./api/client";
import { extractToc, preparePreviewHtml, type TocItem } from "./utils/html";
import DocumentListItem from "./components/DocumentListItem.vue";
import TocPanel from "./components/TocPanel.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ConflictDialog from "./components/ConflictDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import { enableModernWindowStyle } from "@cloudworxx/tauri-plugin-mac-rounded-corners";
import { PanelLeftOpen, PanelLeftClose } from "@lucide/vue";
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

function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value;
}

// 演示（沉浸式）模式
const presenting = ref(false);
const sidebarWasCollapsed = ref(false);
const presentHintVisible = ref(false);
let presentHintTimer: ReturnType<typeof setTimeout> | null = null;

function enterPresent() {
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

onMounted(() => {
  window.addEventListener("message", onIframeMessage);
  window.addEventListener("keydown", onKeydown);
  store.refresh();
  enableModernWindowStyle();
});

onUnmounted(() => {
  window.removeEventListener("message", onIframeMessage);
  window.removeEventListener("keydown", onKeydown);
});

/** 接收 iframe 内脚本回报的当前章节高亮 / Esc 按键 */
function onIframeMessage(e: MessageEvent) {
  if (e.data?.type === "toc-active") {
    activeTocId.value = e.data.id || "";
  } else if (e.data?.type === "esc" && presenting.value) {
    // iframe 获得焦点时 Esc 无法冒泡到父窗口，由注入脚本转发过来
    exitPresent();
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
    await navigator.clipboard.writeText(path);
  } catch (e) {
    console.error("复制路径失败:", e);
  }
}

async function selectDoc(doc: Document) {
  if (currentDoc.value?.id === doc.id) return;
  currentDoc.value = doc;
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
  <div class="app" :class="{ presenting: presenting }" @drop="onDrop" @dragover="onDragOver">
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
        <button
          class="btn"
          :disabled="!currentDoc"
          @click="enterPresent"
        >
          演示
        </button>
        <button class="btn btn-primary" @click="pickAndImport" :disabled="store.importing > 0">
          {{ store.importing > 0 ? "导入中…" : "导入" }}
        </button>
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
}
.topbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}
/* macOS 红黄绿按钮的结构性留白（含左侧窗口内边距） */
.topbar-traffic-pad {
  width: 60px;
  flex-shrink: 0;
  height: 100%;
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
