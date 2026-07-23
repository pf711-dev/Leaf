<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useDocumentsStore } from "./stores/documents";
import {
  setVaultRoot,
  getVaultInfo,
  readFileInlined,
  writeFileContent,
  revealInFinder,
} from "./api/client";
import { extractToc, preparePreviewHtml, type TocItem } from "./utils/html";
import DocumentListItem from "./components/DocumentListItem.vue";
import FolderTree from "./components/FolderTree.vue";
import TocPanel from "./components/TocPanel.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ContextMenu, { type MenuItem } from "./components/ContextMenu.vue";
import VaultSetup from "./components/VaultSetup.vue";
import FolderPickerDialog from "./components/FolderPickerDialog.vue";
import { enableModernWindowStyle } from "@cloudworxx/tauri-plugin-mac-rounded-corners";
import {
  PanelLeftOpen,
  PanelLeftClose,
  Bold,
  Italic,
  Underline,
  Strikethrough,
  AlignLeft,
  AlignCenter,
  AlignRight,
  Undo2,
  Redo2,
  RotateCcw,
  ChevronDown,
  Baseline,
  AArrowUp,
  AArrowDown,
  Plus,
  ListTodo,
  X,
} from "@lucide/vue";
import type { VaultFile } from "./types/document";
import { formatDate, formatSize } from "./utils/format";

const store = useDocumentsStore();

// 仓库状态
const vaultReady = ref(false);
const vaultRoot = ref("");
const vaultName = ref("");

// 当前选中的文件
const currentFile = ref<VaultFile | null>(null);
const currentHtml = ref("");
const loadingContent = ref(false);

// 目录
const tocItems = ref<TocItem[]>([]);
const activeTocId = ref("");

// iframe
const iframeRef = ref<HTMLIFrameElement | null>(null);

// 展开的文件夹 id 集合
const expandedFolderIds = ref<Set<string>>(new Set());
// 正在重命名的文件夹 id（null = 无）
const renamingFolderId = ref<string | null>(null);
// 正在重命名的文件 id（null = 无）
const renamingFileId = ref<string | null>(null);

// 多选模式
const selectMode = ref(false);
const selectedIds = ref<Set<string>>(new Set());

function enterSelectMode() {
  if (editing.value) exitEdit();
  if (presenting.value) exitPresent();
  selectedIds.value = new Set();
  selectMode.value = true;
}

function exitSelectMode() {
  selectMode.value = false;
  selectedIds.value = new Set();
}

function toggleSelect(fileId: string) {
  const next = new Set(selectedIds.value);
  if (next.has(fileId)) next.delete(fileId);
  else next.add(fileId);
  selectedIds.value = next;
}

function selectAll() {
  selectedIds.value = new Set(store.files.map((f) => f.id));
}

function deselectAll() {
  selectedIds.value = new Set();
}

// 批量删除
const batchDeleteVisible = ref(false);

// 移动文件
const moveFileVisible = ref(false);
const pendingMoveFile = ref<VaultFile | null>(null);

async function onMoveFileConfirm(dirRel: string) {
  const file = pendingMoveFile.value;
  if (!file) return;
  moveFileVisible.value = false;
  const ok = await store.moveFile(file.relPath, dirRel);
  if (!ok) {
    // 错误已在 store.folderError 中，watch 会显示 toast
  } else {
    showToast(`已移动到目标文件夹`, "info");
  }
  syncCurrentFile();
  pendingMoveFile.value = null;
}

function cancelMoveFile() {
  moveFileVisible.value = false;
  pendingMoveFile.value = null;
}

// 移动文件夹
const folderMovePickerVisible = ref(false);
const pendingMoveFolderRel = ref("");

async function onMoveFolderConfirm(targetDirRel: string) {
  const dirRel = pendingMoveFolderRel.value;
  if (!dirRel) return;
  folderMovePickerVisible.value = false;
  const ok = await store.moveDir(dirRel, targetDirRel);
  if (ok) {
    showToast(`已移动到目标文件夹`, "info");
  }
  pendingMoveFolderRel.value = "";
}

function cancelMoveFolder() {
  folderMovePickerVisible.value = false;
  pendingMoveFolderRel.value = "";
}

function onBatchDelete() {
  if (selectedIds.value.size === 0) return;
  batchDeleteVisible.value = true;
}

async function onConfirmBatchDelete() {
  batchDeleteVisible.value = false;
  const ids = Array.from(selectedIds.value);
  const targetFiles = store.files.filter((f) => ids.includes(f.id));
  let ok = 0;
  let fail = 0;
  for (const f of targetFiles) {
    const success = await store.removeItem(f.relPath, false);
    if (success) ok += 1;
    else fail += 1;
  }
  if (currentFile.value && ids.includes(currentFile.value.id)) {
    currentFile.value = null;
    currentHtml.value = "";
    tocItems.value = [];
  }
  if (fail > 0) {
    showToast(`已删除 ${ok} 篇，${fail} 篇失败`, "error");
  } else {
    showToast(`已删除 ${ok} 篇文档`, "info");
  }
  exitSelectMode();
}

// 批量移动
const batchMovePickerVisible = ref(false);

function onBatchMove() {
  if (selectedIds.value.size === 0) return;
  batchMovePickerVisible.value = true;
}

async function onBatchMoveConfirm(targetDirRel: string) {
  batchMovePickerVisible.value = false;
  const ids = Array.from(selectedIds.value);
  const targetFiles = store.files.filter((f) => ids.includes(f.id));
  let ok = 0;
  let fail = 0;
  for (const f of targetFiles) {
    const success = await store.moveFile(f.relPath, targetDirRel);
    if (success) ok += 1;
    else fail += 1;
  }
  // 如果移动的文件中有当前预览的文件，更新引用
  if (currentFile.value && ids.includes(currentFile.value.id)) {
    syncCurrentFile();
  }
  if (fail > 0) {
    showToast(`已移动 ${ok} 篇，${fail} 篇失败`, "error");
  } else {
    showToast(`已移动 ${ok} 篇文档`, "info");
  }
  exitSelectMode();
}

function cancelBatchMove() {
  batchMovePickerVisible.value = false;
}

// 平台检测：macOS 需要为红黄绿按钮预留空间
const isMac = ref(false);

// 侧边栏
const SIDEBAR_DEFAULT = 260;
const SIDEBAR_MAX = 500;
const SIDEBAR_SNAP = 80;
const sidebarWidth = ref(SIDEBAR_DEFAULT);
const sidebarCollapsed = computed(() => sidebarWidth.value <= 0);

watch(sidebarCollapsed, (collapsed) => {
  if (collapsed && selectMode.value) exitSelectMode();
});

const isResizing = ref(false);
let resizeStartX = 0;
let resizeStartW = 0;

function onResizeStart(e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  isResizing.value = true;
  resizeStartX = e.clientX;
  resizeStartW = sidebarWidth.value;
  document.body.style.cursor = "col-resize";
  document.body.style.userSelect = "none";
  document.addEventListener("mousemove", onResizeMove);
  document.addEventListener("mouseup", onResizeEnd);
}

function onResizeMove(e: MouseEvent) {
  if (!isResizing.value) return;
  const delta = e.clientX - resizeStartX;
  let w = resizeStartW + delta;
  w = Math.max(SIDEBAR_DEFAULT, Math.min(SIDEBAR_MAX, w));
  sidebarWidth.value = w;
}

function onResizeEnd() {
  if (!isResizing.value) return;
  let target = sidebarWidth.value;
  if (target > 0 && Math.abs(target - SIDEBAR_DEFAULT) < SIDEBAR_SNAP) {
    target = SIDEBAR_DEFAULT;
  }
  const needsSnap = target !== sidebarWidth.value;
  document.body.style.cursor = "";
  document.body.style.userSelect = "";
  document.removeEventListener("mousemove", onResizeMove);
  document.removeEventListener("mouseup", onResizeEnd);
  isResizing.value = false;
  if (needsSnap) {
    requestAnimationFrame(() => {
      sidebarWidth.value = target;
    });
  }
}

function toggleSidebar() {
  sidebarWidth.value = sidebarCollapsed.value ? SIDEBAR_DEFAULT : 0;
}

// 窗口状态
const windowMaximized = ref(false);
const appWindow = getCurrentWindow();

// 演示模式
const presenting = ref(false);
const sidebarWasWidth = ref(SIDEBAR_DEFAULT);

// Toast
const toastMessage = ref("");
const toastType = ref<"info" | "error">("info");
const toastVisible = ref(false);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

function showToast(msg: string, type: "info" | "error" = "info", duration = 3500) {
  if (toastTimer) clearTimeout(toastTimer);
  toastMessage.value = msg;
  toastType.value = type;
  toastVisible.value = true;
  toastTimer = setTimeout(() => {
    toastVisible.value = false;
  }, duration);
}

// 编辑模式
const editing = ref(false);
const saving = ref(false);

const openDropdown = ref<"color" | null>(null);

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
const bgColors = [
  { value: "transparent", label: "无填充" },
  { value: "#F2F2F2", label: "浅灰" },
  { value: "#FFECE8", label: "浅红" },
  { value: "#FFF3E8", label: "浅橙" },
  { value: "#FFF7E8", label: "浅黄" },
  { value: "#E8FFEA", label: "浅绿" },
  { value: "#E8F3FF", label: "浅蓝" },
  { value: "#F5E8FF", label: "浅紫" },
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
  if (editing.value) exitEdit();
  sidebarWasWidth.value = sidebarWidth.value;
  sidebarWidth.value = 0;
  presenting.value = true;
  showToast("按 Esc 退出演示", "info", 2500);
}

function exitPresent() {
  presenting.value = false;
  sidebarWidth.value = sidebarWasWidth.value;
}

function enterEdit() {
  if (presenting.value) exitPresent();
  editing.value = true;
  postToIframe({ type: "edit-mode", enabled: true });
}

function exitEdit() {
  postToIframe({ type: "edit-mode", enabled: false });
  editing.value = false;
  openDropdown.value = null;
}

function cancelEdit() {
  exitEdit();
  if (currentFile.value) reloadCurrentFile();
}

function runFormat(command: string, value?: string) {
  postToIframe({ type: "exec", command, value });
}

function toggleDropdown() {
  openDropdown.value = openDropdown.value === "color" ? null : "color";
}

function resetColor() {
  postToIframe({ type: "exec", command: "foreColor", value: "#212121" });
  postToIframe({ type: "exec", command: "hiliteColor", value: "transparent" });
  openDropdown.value = null;
}

async function saveEdit() {
  if (!currentFile.value || saving.value) return;
  saving.value = true;
  postToIframe({ type: "get-html" });
}

function postToIframe(msg: object) {
  iframeRef.value?.contentWindow?.postMessage(msg, "*");
}

// 监听 folderError
watch(
  () => store.folderError,
  (msg) => {
    if (!msg) return;
    showToast(msg, "error", 3000);
    setTimeout(() => {
      store.folderError = "";
    }, 3100);
  },
);

// 键盘事件
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

async function refreshMaximized() {
  try {
    const [maximized, fullscreen] = await Promise.all([
      appWindow.isMaximized(),
      appWindow.isFullscreen(),
    ]);
    windowMaximized.value = maximized || fullscreen;
  } catch {
    // 非 Tauri 环境忽略
  }
}

function onWindowChanged() {
  refreshMaximized();
}

// ─── 仓库初始化 ───

async function initVault() {
  try {
    const info = await getVaultInfo();
    if (info && info.rootPath) {
      vaultRoot.value = info.rootPath;
      vaultName.value = info.name;
      await store.refresh();
      vaultReady.value = true;
    }
  } catch {
    // 首次使用，留在欢迎页
  }
}

async function onVaultSelected(rootPath: string) {
  try {
    await setVaultRoot(rootPath);
    vaultRoot.value = rootPath;
    const parts = rootPath.replace(/\\/g, "/").split("/");
    vaultName.value = parts[parts.length - 1] || "我的仓库";
    await store.refresh();
    vaultReady.value = true;
  } catch (e) {
    showToast(String(e), "error");
  }
}

// ─── 文件变更监听 ───

let vaultUpdatedUnlisten: UnlistenFn | null = null;

async function startVaultListener() {
  vaultUpdatedUnlisten = await listen("vault-updated", () => {
    // 文件变更：刷新列表
    store.refresh();
    // 如果当前预览的文件恰好在变更中，不主动重载（编辑模式时避免覆盖用户改动）
  });
}

// ─── 生命周期 ───

onMounted(async () => {
  window.addEventListener("message", onIframeMessage);
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("mousedown", onGlobalMouseDown);

  await initVault();
  await startVaultListener();
  try {
    enableModernWindowStyle();
    isMac.value = true;
  } catch {
    isMac.value = false;
  }
  refreshMaximized();

  const unlistenPromise = appWindow.onResized(onWindowChanged);
  onUnmountedCleanup.push(() => {
    unlistenPromise.then((unlisten) => unlisten());
  });
});

const onUnmountedCleanup: Array<() => void> = [];

onUnmounted(() => {
  window.removeEventListener("message", onIframeMessage);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("mousedown", onGlobalMouseDown);
  if (vaultUpdatedUnlisten) vaultUpdatedUnlisten();
  onUnmountedCleanup.forEach((fn) => fn());
});

function onGlobalMouseDown(e: MouseEvent) {
  if (openDropdown.value) {
    const target = e.target as HTMLElement;
    if (!target.closest(".fmt-dropdown")) {
      openDropdown.value = null;
    }
  }
}

// ─── iframe 消息处理 ───

function onIframeMessage(e: MessageEvent) {
  const d = e.data;
  if (!d) return;
  if (d.type === "toc-active") {
    activeTocId.value = d.id || "";
  } else if (d.type === "esc") {
    if (editing.value) exitEdit();
    else if (presenting.value) exitPresent();
  } else if (d.type === "html-content" && saving.value) {
    onHtmlContentForSave(d.html);
  }
}

async function onHtmlContentForSave(html: string) {
  const f = currentFile.value;
  if (!f) {
    saving.value = false;
    return;
  }
  try {
    await writeFileContent(f.relPath, html);
    exitEdit();
    await reloadCurrentFile();
  } catch (err) {
    console.error("保存失败:", err);
  } finally {
    saving.value = false;
  }
}

// ─── 文件选择与预览 ───

async function selectFile(file: VaultFile) {
  if (currentFile.value?.id === file.id) return;
  if (editing.value) exitEdit();
  currentFile.value = file;
  await reloadCurrentFile();
}

async function reloadCurrentFile() {
  const f = currentFile.value;
  if (!f) return;
  loadingContent.value = true;
  currentHtml.value = "";
  activeTocId.value = "";
  try {
    const raw = await readFileInlined(f.relPath);
    tocItems.value = extractToc(raw);
    currentHtml.value = preparePreviewHtml(raw, tocItems.value);
  } finally {
    loadingContent.value = false;
  }
}

function navigateToc(id: string) {
  iframeRef.value?.contentWindow?.postMessage({ type: "scroll-to", id }, "*");
}

// ─── 删除确认 ───

const confirmVisible = ref(false);
const pendingDelete = ref<VaultFile | null>(null);

function removeFile(file: VaultFile) {
  pendingDelete.value = file;
  confirmVisible.value = true;
}

async function doDelete() {
  const f = pendingDelete.value;
  if (!f) return;
  confirmVisible.value = false;
  await store.removeItem(f.relPath, false);
  if (currentFile.value?.id === f.id) {
    currentFile.value = null;
    currentHtml.value = "";
    tocItems.value = [];
  }
  pendingDelete.value = null;
}

// ─── 文件夹操作 ───

const newFolderVisible = ref(false);
const newFolderParentRel = ref<string>("");
const newFolderName = ref("");
const newFolderInputEl = ref<HTMLInputElement | null>(null);

watch(newFolderVisible, (v) => {
  if (v) {
    nextTick(() => {
      newFolderInputEl.value?.focus();
    });
  }
});

function onNewFolderAtRoot() {
  newFolderParentRel.value = "";
  newFolderName.value = "";
  newFolderVisible.value = true;
}

function onNewSubfolder(parentRel: string) {
  newFolderParentRel.value = parentRel;
  newFolderName.value = "";
  newFolderVisible.value = true;
}

async function confirmNewFolder() {
  const name = newFolderName.value.trim();
  if (!name) {
    newFolderVisible.value = false;
    return;
  }
  newFolderVisible.value = false;
  const parentRel = newFolderParentRel.value || null;
  await store.createDir(parentRel, name);
}

// ─── 文件夹展开 ───

function onToggleFolder(dirRel: string) {
  const next = new Set(expandedFolderIds.value);
  if (next.has(dirRel)) next.delete(dirRel);
  else next.add(dirRel);
  expandedFolderIds.value = next;
}

async function onCommitRename(folderRel: string, newName: string) {
  renamingFolderId.value = null;
  await store.renameItem(folderRel, newName, true);
}

function onCancelRename() {
  renamingFolderId.value = null;
}

async function onCommitFileRename(fileId: string, newName: string) {
  const f = store.files.find((d) => d.id === fileId);
  if (!f) {
    renamingFileId.value = null;
    return;
  }
  renamingFileId.value = null;
  await store.renameItem(f.relPath, newName, false);
  syncCurrentFile();
}

function onCancelFileRename() {
  renamingFileId.value = null;
}

function syncCurrentFile() {
  if (!currentFile.value) return;
  currentFile.value =
    store.files.find((f) => f.id === currentFile.value!.id) ?? null;
}

const deleteFolderVisible = ref(false);
const pendingDeleteFolder = ref<{ relPath: string; name: string } | null>(null);

async function onConfirmDeleteFolder() {
  const target = pendingDeleteFolder.value;
  if (!target) return;
  deleteFolderVisible.value = false;
  await store.removeItem(target.relPath, true);
  if (currentFile.value && currentFile.value.relPath.startsWith(target.relPath + "/")) {
    currentFile.value = null;
    currentHtml.value = "";
    tocItems.value = [];
  }
  pendingDeleteFolder.value = null;
}

// ─── 右键菜单 ───

const contextMenuVisible = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const contextMenuItems = ref<MenuItem[]>([]);
const contextMenuFooter = ref("");
let contextMenuTarget:
  | { type: "file"; file: VaultFile }
  | { type: "folder"; dirRel: string; level: number }
  | null = null;

function onFileContextMenu(file: VaultFile, e: MouseEvent) {
  e.preventDefault();
  contextMenuTarget = { type: "file", file };
  const items: MenuItem[] = [
    { key: "rename", label: "重命名" },
    { key: "move", label: "移动到..." },
    { key: "reveal", label: "在 Finder 中显示" },
    { key: "delete", label: "删除", danger: true },
  ];
  contextMenuItems.value = items;
  contextMenuFooter.value = `${formatSize(file.fileSize)} · ${formatDate(file.lastModified)}`;
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuVisible.value = true;
}

function onFolderContextMenu(dirRel: string, e: MouseEvent) {
  e.preventDefault();
  const dir = store.dirs.find((d) => d.relPath === dirRel);
  const level = dir?.level ?? 1;
  contextMenuTarget = { type: "folder", dirRel, level };
  contextMenuFooter.value = "";
  const items: MenuItem[] = [
    { key: "rename", label: "重命名" },
  ];
  items.push({ key: "moveFolder", label: "移动到…" });
  if (!(level >= 3)) {
    items.push({ key: "newSub", label: "新建子文件夹" });
  }
  items.push({ key: "delete", label: "删除", danger: true });
  contextMenuItems.value = items;
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuVisible.value = true;
}

function onContextSelect(key: string) {
  contextMenuVisible.value = false;
  const target = contextMenuTarget;
  contextMenuTarget = null;
  if (!target) return;

  if (target.type === "file") {
    const file = target.file;
    if (key === "rename") {
      renamingFileId.value = file.id;
    } else if (key === "move") {
      pendingMoveFile.value = file;
      moveFileVisible.value = true;
      return; // 让 moveFileVisible 接管，不执行下面的 contextMenuTarget 置空逻辑会导致问题，所以提前 return 保留 target
    } else if (key === "reveal") {
      revealInFinder(file.relPath).catch((e) =>
        console.error("在 Finder 中显示失败:", e),
      );
    } else if (key === "delete") {
      removeFile(file);
    }
  } else if (target.type === "folder") {
    const dirRel = target.dirRel;
    if (key === "rename") {
      renamingFolderId.value = dirRel;
    } else if (key === "moveFolder") {
      pendingMoveFolderRel.value = dirRel;
      folderMovePickerVisible.value = true;
      return;
    } else if (key === "newSub") {
      onNewSubfolder(dirRel);
    } else if (key === "delete") {
      const dir = store.dirs.find((d) => d.relPath === dirRel);
      pendingDeleteFolder.value = {
        relPath: dirRel,
        name: dir?.name ?? "",
      };
      deleteFolderVisible.value = true;
    }
  }
}
</script>

<template>
  <!-- 没有仓库 → 欢迎页 -->
  <div v-if="!vaultReady" class="app">
    <VaultSetup @selected="onVaultSelected" />
  </div>

  <!-- 已有仓库 → 主界面 -->
  <div
    v-else
    class="app"
    :class="{ presenting: presenting, maximized: windowMaximized, 'platform-mac': isMac }"
  >
    <!-- 顶部栏 -->
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
        <!-- 编辑模式工具栏 -->
        <template v-if="editing">
          <div class="edit-toolbar" @mousedown.prevent>
            <button class="fmt-btn" title="增大字号" @click="runFormat('increaseFontSize')"><AArrowUp :size="16" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="减小字号" @click="runFormat('decreaseFontSize')"><AArrowDown :size="16" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="加粗" @click="runFormat('bold')"><Bold :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="删除线" @click="runFormat('strikeThrough')"><Strikethrough :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="倾斜" @click="runFormat('italic')"><Italic :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="下划线" @click="runFormat('underline')"><Underline :size="15" :stroke-width="1.8" /></button>

            <span class="fmt-sep"></span>

            <div class="fmt-dropdown">
              <button class="fmt-trigger" title="颜色" @click="toggleDropdown()">
                <Baseline :size="15" :stroke-width="1.8" />
                <ChevronDown :size="12" :stroke-width="1.8" />
              </button>
              <div v-if="openDropdown === 'color'" class="fmt-menu fmt-color-menu">
                <div class="fmt-color-section">字体颜色</div>
                <div class="fmt-color-grid">
                  <button v-for="c in textColors" :key="'t' + c.value"
                    class="fmt-color-swatch fmt-text-swatch" :style="{ color: c.value }"
                    :title="c.label" @click="runFormat('foreColor', c.value)"
                  >A</button>
                </div>
                <div class="fmt-color-section">背景颜色</div>
                <div class="fmt-color-grid">
                  <button v-for="c in bgColors" :key="'b' + c.value"
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

            <button class="fmt-btn" title="左对齐" @click="runFormat('justifyLeft')"><AlignLeft :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="居中" @click="runFormat('justifyCenter')"><AlignCenter :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="右对齐" @click="runFormat('justifyRight')"><AlignRight :size="15" :stroke-width="1.8" /></button>

            <span class="fmt-sep"></span>

            <button class="fmt-btn" title="撤销" @click="runFormat('undo')"><Undo2 :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="重做" @click="runFormat('redo')"><Redo2 :size="15" :stroke-width="1.8" /></button>
            <button class="fmt-btn" title="清空格式" @click="runFormat('removeFormat')"><RotateCcw :size="15" :stroke-width="1.8" /></button>
          </div>
          <button class="btn" :disabled="saving" @click="cancelEdit">取消</button>
          <button class="btn btn-primary" :disabled="saving" @click="saveEdit">
            {{ saving ? "保存中…" : "保存" }}
          </button>
        </template>

        <!-- 非编辑模式 -->
        <template v-else>
          <button class="btn" :disabled="!currentFile || loadingContent" @click="enterEdit">
            编辑
          </button>
          <button class="btn" :disabled="!currentFile" @click="enterPresent">
            演示
          </button>
        </template>
      </div>
      <!-- Windows 窗口控制按钮（tauri-plugin-frame 自动注入） -->
      <div data-tauri-frame-tb></div>
    </header>

    <!-- 主内容 -->
    <main class="content">
      <!-- 侧边栏 -->
      <aside id="sidebar" class="sidebar" :class="{ collapsed: sidebarCollapsed, resizing: isResizing }" :style="{ width: sidebarWidth + 'px' }">
        <div class="sidebar-inner">
          <div class="sidebar-head">
            <span class="sidebar-title">{{ vaultName }}</span>
            <div class="sidebar-head-right">
              <button
                v-if="store.files.length > 0"
                class="icon-btn"
                :title="selectMode ? '完成多选' : '多选'"
                :aria-label="selectMode ? '完成多选' : '多选'"
                @click="selectMode ? exitSelectMode() : enterSelectMode()"
              >
                <ListTodo v-if="!selectMode" :size="14" :stroke-width="1.5" />
                <X v-else :size="14" :stroke-width="1.5" />
              </button>
              <button
                class="icon-btn"
                title="新建文件夹"
                aria-label="新建文件夹"
                @click="onNewFolderAtRoot"
              >
                <Plus :size="14" :stroke-width="1.5" />
              </button>
            </div>
          </div>

          <div class="sidebar-body">
            <p v-if="store.loading" class="status">加载中…</p>
            <p v-else-if="store.error" class="status error">{{ store.error }}</p>

            <template v-else>
              <!-- 仓库根目录下的散落文件 -->
              <DocumentListItem
                v-for="f in store.rootFiles"
                :key="f.id"
                :doc="{
                  id: f.id,
                  title: f.title,
                  fileName: f.fileName,
                  libraryPath: f.relPath,
                  fileSize: f.fileSize,
                  summary: f.summary,
                  importedAt: f.lastModified,
                  sourceCreatedAt: f.lastModified,
                }"
                :active="currentFile?.id === f.id"
                :select-mode="selectMode"
                :selected="selectMode ? selectedIds.has(f.id) : false"
                :renaming="renamingFileId === f.id"
                @click="selectFile(f)"
                @contextmenu="(_d, ev) => onFileContextMenu(f, ev)"
                @toggle="toggleSelect"
                @commit-rename="(_did: string, n: string) => onCommitFileRename(f.id, n)"
                @cancel-rename="onCancelFileRename"
              />
              <!-- 子文件夹目录树（最多 3 级） -->
              <FolderTree
                :nodes="store.tree"
                :level="0"
                :active-file-id="currentFile?.id"
                :expanded-ids="expandedFolderIds"
                :renaming-id="renamingFolderId"
                :renaming-file-id="renamingFileId"
                :select-mode="selectMode"
                :selected-ids="selectedIds"
                @select-file="selectFile"
                @file-contextmenu="onFileContextMenu"
                @folder-contextmenu="onFolderContextMenu"
                @toggle="onToggleFolder"
                @commit-rename="onCommitRename"
                @cancel-rename="onCancelRename"
                @commit-file-rename="onCommitFileRename"
                @cancel-file-rename="onCancelFileRename"
                @toggle-select="toggleSelect"
              />
            </template>
          </div>

          <!-- 多选操作栏 -->
          <div v-if="selectMode" class="select-actionbar">
            <div class="select-actionbar-left">
              <span class="select-count">已选 {{ selectedIds.size }} 篇</span>
              <button
                class="select-link-btn"
                @click="selectedIds.size === store.files.length ? deselectAll() : selectAll()"
              >
                {{ selectedIds.size === store.files.length ? '取消全选' : '全选' }}
              </button>
            </div>
            <div class="select-actionbar-right">
              <button
                class="btn btn-primary select-action-btn"
                :disabled="selectedIds.size === 0"
                @click="onBatchMove"
              >
                移动到…
              </button>
              <button
                class="btn btn-danger select-action-btn"
                :disabled="selectedIds.size === 0"
                @click="onBatchDelete"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      </aside>

      <!-- 拖拽手柄 -->
      <div
        class="sidebar-handle"
        :class="{ active: isResizing }"
        @mousedown="onResizeStart"
      />

      <!-- 预览区 -->
      <section class="preview">
        <template v-if="currentFile">
          <div class="preview-body">
            <p v-if="loadingContent" class="status">加载中…</p>
            <template v-else-if="currentHtml">
              <iframe
                ref="iframeRef"
                class="preview-iframe"
                :class="{ editing }"
                :srcdoc="currentHtml"
                sandbox="allow-scripts"
              />
              <TocPanel
                :items="tocItems"
                :active-id="activeTocId"
                @navigate="navigateToc"
              />
            </template>
          </div>
        </template>

        <div v-else class="preview-empty">
          <template v-if="!store.hasItems && !store.loading">
            <svg class="preview-empty-icon" width="48" height="48" viewBox="0 0 48 48" fill="none">
              <path d="M6 8h16l4 4h16v28a2 2 0 0 1-2 2H6z" stroke="currentColor" stroke-width="1.5" fill="none"/>
            </svg>
            <p class="preview-empty-text">仓库中暂无 HTML 文件</p>
            <p style="margin: 0; font-size: 12px; color: var(--text-faint);">
              把文件放入 {{ vaultName }} 文件夹即可自动识别
            </p>
          </template>
          <template v-else>
            <svg class="preview-empty-icon" width="48" height="48" viewBox="0 0 48 48" fill="none">
              <rect x="12" y="8" width="20" height="30" rx="3" stroke="currentColor" stroke-width="1.5"/>
              <path d="M17 18h10M17 23h10M17 28h7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
            <p class="preview-empty-text">选择左侧文档以预览</p>
          </template>
        </div>
      </section>
    </main>

    <!-- 删除确认 -->
    <ConfirmDialog
      :visible="confirmVisible"
      :title="`删除「${pendingDelete?.title ?? ''}」？`"
      message="将同时删除磁盘上的文件，该操作不可撤销。"
      confirm-text="删除"
      :danger="true"
      @confirm="doDelete"
      @cancel="confirmVisible = false; pendingDelete = null"
    />

    <!-- 右键菜单 -->
    <ContextMenu
      :visible="contextMenuVisible"
      :x="contextMenuX"
      :y="contextMenuY"
      :items="contextMenuItems"
      :footer="contextMenuFooter"
      @select="onContextSelect"
      @close="contextMenuVisible = false"
    />

    <!-- Toast -->
    <transition name="fade">
      <div v-if="toastVisible" class="toast" :class="toastType">
        {{ toastMessage }}
      </div>
    </transition>

    <!-- 新建文件夹 -->
    <transition name="fade">
      <div v-if="newFolderVisible" class="overlay" @click.self="newFolderVisible = false">
        <transition name="pop" appear>
          <div v-if="newFolderVisible" class="dialog new-folder-dialog">
            <div class="dialog-title">新建文件夹</div>
            <input
              ref="newFolderInputEl"
              v-model="newFolderName"
              class="new-folder-input"
              type="text"
              placeholder="文件夹名称"
              @keydown.enter="confirmNewFolder"
              @keydown.escape="newFolderVisible = false"
            />
            <div class="dialog-actions">
              <button class="btn btn-cancel" @click="newFolderVisible = false">取消</button>
              <button class="btn btn-confirm" @click="confirmNewFolder">创建</button>
            </div>
          </div>
        </transition>
      </div>
    </transition>

    <!-- 移动文件到文件夹 -->
    <FolderPickerDialog
      :visible="moveFileVisible"
      :folders="store.dirs"
      :root-name="vaultName"
      :default-selected="pendingMoveFile?.dirPath ?? ''"
      :exclude-dir-rel="pendingMoveFile?.dirPath || null"
      :title="`移动「${pendingMoveFile?.fileName ?? ''}」到…`"
      @confirm="onMoveFileConfirm"
      @cancel="cancelMoveFile"
    />

    <!-- 移动文件夹到目标位置 -->
    <FolderPickerDialog
      :visible="folderMovePickerVisible"
      :folders="store.dirs"
      :root-name="vaultName"
      :exclude-dir-rel="pendingMoveFolderRel || null"
      :max-target-level="3"
      :title="`移动「${store.dirs.find(d => d.relPath === pendingMoveFolderRel)?.name ?? '文件夹'}」到…`"
      @confirm="onMoveFolderConfirm"
      @cancel="cancelMoveFolder"
    />

    <!-- 批量移动文件 -->
    <FolderPickerDialog
      :visible="batchMovePickerVisible"
      :folders="store.dirs"
      :root-name="vaultName"
      :title="`移动 ${selectedIds.size} 篇到…`"
      @confirm="onBatchMoveConfirm"
      @cancel="cancelBatchMove"
    />

    <!-- 删除文件夹确认 -->
    <ConfirmDialog
      :visible="deleteFolderVisible"
      :title="`删除文件夹「${pendingDeleteFolder?.name ?? ''}」？`"
      message="将同时删除磁盘上的文件夹及其所有内容，该操作不可撤销。"
      confirm-text="删除"
      :danger="true"
      @confirm="onConfirmDeleteFolder"
      @cancel="deleteFolderVisible = false; pendingDeleteFolder = null"
    />

    <!-- 批量删除确认 -->
    <ConfirmDialog
      :visible="batchDeleteVisible"
      :title="`删除选中的 ${selectedIds.size} 篇文档？`"
      message="将同时删除磁盘上的文件，该操作不可撤销。"
      confirm-text="删除"
      :danger="true"
      @confirm="onConfirmBatchDelete"
      @cancel="batchDeleteVisible = false"
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
  position: relative;
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
.topbar-traffic-pad {
  width: 0;
  flex-shrink: 0;
  height: 100%;
  transition: width 0.12s cubic-bezier(0.4, 0, 0.2, 1);
}
/* macOS 需要为红黄绿按钮预留空间 */
.platform-mac .topbar-traffic-pad {
  width: 60px;
}
.platform-mac.maximized .topbar-traffic-pad {
  width: 0;
}
.platform-mac.maximized .topbar-left .icon-btn {
  margin-left: -1px;
}
.topbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
  /* 避免被 Windows 窗口控制按钮遮挡 */
  padding-right: var(--tauri-frame-controls-width, 0px);
}

/* Windows 窗口控制按钮容器 */
[data-tauri-frame-tb] {
  position: absolute;
  top: 0;
  right: 0;
  height: 44px;
}

/* ---------- 按钮 ---------- */
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
.btn-danger {
  color: #fff;
  background: var(--danger);
  border-color: var(--danger);
}
.btn-danger:hover {
  background: #b04a44;
  border-color: #b04a44;
}
.btn-danger:disabled {
  opacity: 0.45;
  cursor: not-allowed;
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

.sidebar {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  contain: layout style;
  transition: width 0.2s cubic-bezier(0.4, 0, 0.2, 1), border-color 0.2s ease;
}
.sidebar.resizing {
  transition: none;
  will-change: width;
}
.sidebar.collapsed {
  width: 0;
  border-right-color: transparent;
}
.sidebar-inner {
  width: 100%;
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-handle {
  width: 6px;
  flex-shrink: 0;
  cursor: col-resize;
  background: transparent;
  transition: background 0.15s ease;
}
.sidebar-handle:hover,
.sidebar-handle.active {
  background: var(--border);
}

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
  padding: 14px 14px 10px;
}
.sidebar-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-dim);
}
.sidebar-head-right {
  display: flex;
  align-items: center;
  gap: 2px;
}
.sidebar-head-right > .icon-btn:last-child {
  margin-left: 2px;
}
.sidebar-count {
  font-weight: 400;
  color: var(--text-faint);
  margin-left: 2px;
}
.sidebar-body {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px 8px;
}

.select-actionbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 12px;
  background: var(--bg-sidebar);
  border-top: 1px solid var(--border);
  flex-shrink: 0;
}
.select-actionbar-left {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.select-count {
  font-size: 12px;
  color: var(--text-dim);
  white-space: nowrap;
}
.select-link-btn {
  padding: 0;
  border: none;
  background: transparent;
  color: var(--accent-blue);
  font-size: 12px;
  cursor: pointer;
  white-space: nowrap;
}
.select-link-btn:hover {
  text-decoration: underline;
}
.select-actionbar-right {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.select-action-btn {
  padding: 4px 10px;
  font-size: 12px;
}

.status {
  padding: 20px 8px;
  color: var(--text-faint);
  font-size: 13px;
}
.status.error {
  color: var(--danger);
}

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

.app.presenting .topbar {
  display: none;
}

.toast {
  position: fixed;
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
  z-index: 1100;
  pointer-events: none;
  white-space: nowrap;
  max-width: 420px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.toast.error {
  color: var(--danger);
}

.preview-iframe {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: none;
  transition: box-shadow 0.2s ease;
}
.preview-iframe.editing {
  box-shadow: inset 0 0 0 2px var(--accent-blue);
}

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

.overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 15, 15, 0.25);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.dialog {
  background: var(--bg);
  border-radius: 8px;
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
  padding: 20px 24px;
  width: 360px;
  max-width: 90%;
}
.dialog-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text);
  margin-bottom: 14px;
}
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}
.new-folder-input {
  width: 100%;
  box-sizing: border-box;
  padding: 6px 10px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--bg);
  font-size: 13px;
  font-family: inherit;
  color: var(--text);
  outline: none;
  transition: border-color 0.1s;
}
.new-folder-input:focus {
  border-color: var(--accent-blue);
}

.btn-cancel {
  background: var(--bg-hover);
  color: var(--text);
}
.btn-cancel:hover {
  background: var(--bg-active);
}
.btn-confirm {
  background: var(--accent-blue);
  color: #fff;
}
.btn-confirm:hover {
  background: var(--accent-blue-hover);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.18s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.pop-enter-active {
  transition: transform 0.18s ease, opacity 0.18s ease;
}
.pop-enter-from {
  transform: scale(0.95);
  opacity: 0;
}
</style>
