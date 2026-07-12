<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useDocumentsStore } from "./stores/documents";
import { readDocumentContent } from "./api/client";
import { extractToc, preparePreviewHtml, type TocItem } from "./utils/html";
import DocumentListItem from "./components/DocumentListItem.vue";
import TocPanel from "./components/TocPanel.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import { enableModernWindowStyle } from "@cloudworxx/tauri-plugin-mac-rounded-corners";
import type { Document } from "./types/document";

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

onMounted(() => {
  window.addEventListener("message", onIframeMessage);
  store.refresh();
  enableModernWindowStyle();
});

onUnmounted(() => {
  window.removeEventListener("message", onIframeMessage);
});

/** 接收 iframe 内脚本报回的当前章节高亮 */
function onIframeMessage(e: MessageEvent) {
  if (e.data?.type === "toc-active") {
    activeTocId.value = e.data.id || "";
  }
}

async function pickAndImport() {
  const selected = await open({
    multiple: true,
    filters: [{ name: "HTML", extensions: ["html", "htm"] }],
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await store.importPaths(paths);
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
  <div class="app" @drop="onDrop" @dragover="onDragOver">
    <!-- 顶部栏（data-tauri-drag-region 使整个顶栏可拖动窗口） -->
    <header class="topbar" data-tauri-drag-region>
      <div class="topbar-left">
        <span class="brand" data-tauri-drag-region>Leaf</span>
      </div>
      <div class="topbar-right">
        <span v-if="hasDocuments" class="count">{{ store.documents.length }} 篇</span>
        <button class="btn btn-primary" @click="pickAndImport" :disabled="store.importing > 0">
          {{ store.importing > 0 ? "导入中…" : "导入" }}
        </button>
      </div>
    </header>

    <!-- 主内容：左右分栏 -->
    <main class="content">
      <!-- 左侧：文档列表 -->
      <aside class="sidebar">
        <div class="sidebar-head">
          <span>文档</span>
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
          />
        </div>
      </aside>

      <!-- 右侧：预览 -->
      <section class="preview">
        <!-- 有文档选中 -->
        <template v-if="currentDoc">
          <div class="preview-head">
            <span class="preview-title" :title="currentDoc.title">{{ currentDoc.title }}</span>
            <span class="preview-path">{{ currentDoc.fileName }}</span>
            <button class="btn btn-ghost btn-danger" @click="removeDoc(currentDoc)">删除</button>
          </div>
          <div class="preview-body">
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
  </div>
</template>

<style scoped>
.app {
  display: flex;
  flex-direction: column;
  height: 100%;
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
.brand {
  color: var(--text);
  font-weight: 600;
  margin-left: 68px;
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
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
}
.sidebar-head {
  padding: 14px 14px 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--text-dim);
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
.preview-head {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 44px;
  padding: 0 16px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.preview-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.preview-path {
  font-size: 12px;
  color: var(--text-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.preview-head .btn {
  margin-left: auto;
}
.preview-body {
  flex: 1;
  min-height: 0;
  position: relative;
  background: #ffffff;
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
