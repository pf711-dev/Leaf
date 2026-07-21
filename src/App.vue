<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useDocumentsStore } from "./stores/documents";
import { getDocumentPath, readDocumentContent, writeDocumentContent } from "./api/client";
import { extractToc, preparePreviewHtml, type TocItem } from "./utils/html";
import DocumentListItem from "./components/DocumentListItem.vue";
import FolderTree from "./components/FolderTree.vue";
import TocPanel from "./components/TocPanel.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ConflictDialog from "./components/ConflictDialog.vue";
import ContextMenu, { type MenuItem } from "./components/ContextMenu.vue";
import FolderPickerDialog from "./components/FolderPickerDialog.vue";
import { enableModernWindowStyle } from "@cloudworxx/tauri-plugin-mac-rounded-corners";
import { PanelLeftOpen, PanelLeftClose, Bold, Italic, Underline, Strikethrough, AlignLeft, AlignCenter, AlignRight, Undo2, Redo2, RotateCcw, ChevronDown, Baseline, AArrowUp, AArrowDown, Plus, FileUp, FolderUp, FolderOpen, ListTodo, X } from "@lucide/vue";
import type { ConflictInfo, Document } from "./types/document";
import { formatDate, formatSize } from "./utils/format";

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

// 展开的文件夹 id 集合（不持久化，重启恢复默认展开一级文件夹）
const expandedFolderIds = ref<Set<string>>(new Set());
// 正在重命名的文件夹 id（null = 无）
const renamingFolderId = ref<string | null>(null);

// 多选模式：批量移动/删除文档。与编辑/演示模式互斥。
const selectMode = ref(false);
// 选中的文档 id 集合（跨文件夹）。响应式：每次变更需重新赋值 Set。
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

/** 切换某文档的选中态（多选模式下点击文档项调用） */
function toggleSelect(docId: string) {
  const next = new Set(selectedIds.value);
  if (next.has(docId)) next.delete(docId);
  else next.add(docId);
  selectedIds.value = next;
}

/** 全选所有文档 */
function selectAll() {
  selectedIds.value = new Set(store.documents.map((d) => d.id));
}

/** 取消全选 */
function deselectAll() {
  selectedIds.value = new Set();
}

// -------------------- 批量操作 --------------------

/** 批量移动：弹 FolderPickerDialog 选目标 */
function onBatchMove() {
  if (selectedIds.value.size === 0) return;
  pickerContext = "batchMove";
  folderPickerTitle.value = `移动 ${selectedIds.value.size} 篇到…`;
  pickerExcludeDocFolderId.value = null;
  folderPickerVisible.value = true;
}

/** 批量删除：弹 ConfirmDialog 二次确认 */
const batchDeleteVisible = ref(false);
function onBatchDelete() {
  if (selectedIds.value.size === 0) return;
  batchDeleteVisible.value = true;
}

/** 确认批量删除 */
async function onConfirmBatchDelete() {
  batchDeleteVisible.value = false;
  const ids = Array.from(selectedIds.value);
  const docs = store.documents.filter((d) => ids.includes(d.id));
  let ok = 0;
  let fail = 0;
  for (const doc of docs) {
    try {
      await store.remove(doc.id, doc.libraryPath);
      ok += 1;
    } catch {
      fail += 1;
    }
  }
  // 如果删除的是当前预览的文档，清空预览
  if (currentDoc.value && ids.includes(currentDoc.value.id)) {
    currentDoc.value = null;
    currentHtml.value = "";
    tocItems.value = [];
  }
  if (fail > 0) {
    showInfoToast(`已删除 ${ok} 篇，${fail} 篇失败`);
  } else {
    showInfoToast(`已删除 ${ok} 篇文档`);
  }
  exitSelectMode();
}

// 侧边栏宽度（0 = 收起，260 默认，500 最大）
const SIDEBAR_DEFAULT = 260;
const SIDEBAR_MAX = 500;
const SIDEBAR_SNAP = 80; // 低于此自动收起
const sidebarWidth = ref(SIDEBAR_DEFAULT);
const sidebarCollapsed = computed(() => sidebarWidth.value <= 0);

// 侧边栏收起时自动退出多选模式
watch(sidebarCollapsed, (collapsed) => {
  if (collapsed && selectMode.value) exitSelectMode();
});

// 拖拽调整侧边栏宽度
const isResizing = ref(false);
let resizeStartX = 0;
let resizeStartW = 0;

function onResizeStart(e: MouseEvent) {
  e.preventDefault();
  isResizing.value = true;
  resizeStartX = e.clientX;
  resizeStartW = sidebarWidth.value;
  document.addEventListener("mousemove", onResizeMove);
  document.addEventListener("mouseup", onResizeEnd);
}

function onResizeMove(e: MouseEvent) {
  if (!isResizing.value) return;
  const delta = e.clientX - resizeStartX;
  if (delta <= 0) return; // 只允许向右拖
  let w = resizeStartW + delta;
  w = Math.min(SIDEBAR_MAX, w);
  sidebarWidth.value = w;
}

function onResizeEnd() {
  isResizing.value = false;
  // 微调：离默认值很近时吸附回去
  if (Math.abs(sidebarWidth.value - SIDEBAR_DEFAULT) < SIDEBAR_SNAP && sidebarWidth.value > 0) {
    sidebarWidth.value = SIDEBAR_DEFAULT;
  }
  document.removeEventListener("mousemove", onResizeMove);
  document.removeEventListener("mouseup", onResizeEnd);
}

function toggleSidebar() {
  sidebarWidth.value = sidebarCollapsed.value ? SIDEBAR_DEFAULT : 0;
}

// 窗口最大化/全屏状态
const windowMaximized = ref(false);
const appWindow = getCurrentWindow();

// 演示（沉浸式）模式
const presenting = ref(false);
const sidebarWasWidth = ref(SIDEBAR_DEFAULT);

// 统一 toast：顶部居中浮层，替换所有右下角 / 分散位置的提示
// type: 'info'（默认蓝灰）| 'error'（暖红）
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

// 文件夹操作错误：通过统一 toast 显示
watch(
  () => store.folderError,
  (msg) => {
    if (!msg) return;
    showToast(msg, "error", 3000);
    // 延迟清空 store 中的错误，防止组件反复触发
    setTimeout(() => { store.folderError = ""; }, 3100);
  },
);

function showInfoToast(msg: string) {
  showToast(msg, "info", 3500);
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
  // 关闭编辑模式颜色下拉
  if (openDropdown.value) {
    const target = e.target as HTMLElement;
    if (!target.closest(".fmt-dropdown")) {
      openDropdown.value = null;
    }
  }
  // 关闭导入下拉
  if (importDropdownOpen.value) {
    const target = e.target as HTMLElement;
    if (!target.closest(".import-dropdown")) {
      importDropdownOpen.value = false;
    }
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

// 导入下拉菜单状态
const importDropdownOpen = ref(false);

/** 切换导入下拉 */
function toggleImportDropdown() {
  if (store.importing > 0) return;
  importDropdownOpen.value = !importDropdownOpen.value;
}

/** 下拉项：导入 HTML 文件 */
function onPickImportFiles() {
  importDropdownOpen.value = false;
  pickAndImport();
}

/** 下拉项：导入文件夹 */
async function onPickImportFolder() {
  importDropdownOpen.value = false;
  
  // 先选系统文件夹
  const dir = await open({ directory: true, recursive: true });
  if (!dir || typeof dir !== "string") return;
  
  // 再弹文件夹选择器选目标位置
  pickerContext = "importFolder";
  folderPickerTitle.value = "导入到…";
  pickerExcludeDocFolderId.value = null;
  pendingImportDir.value = dir;
  folderPickerVisible.value = true;
}

/** 上次选中的导入目录（文件夹选择器确认后使用） */
const pendingImportDir = ref<string>("");

/** 根据导入结果摘要，组装 toast 文案（成功走 info toast，异常走 error toast） */
function showImportResultToast(r: { importedCount: number; skippedCount: number; flattenedCount: number; folderCount: number; firstError: string; failedCount: number }) {
  // 全部失败：用 error toast 显示首个失败原因，便于诊断
  if (r.importedCount === 0 && r.failedCount > 0) {
    store.folderError = `导入失败（${r.failedCount} 个文件）：${r.firstError}`;
    return;
  }
  if (r.importedCount === 0 && r.skippedCount === 0) {
    store.folderError = "没有导入任何文档";
    return;
  }
  const parts: string[] = [`已导入 ${r.importedCount} 篇文档`];
  if (r.folderCount > 0) parts.push(`新建 ${r.folderCount} 个文件夹`);
  if (r.flattenedCount > 0) {
    parts.push(`${r.flattenedCount} 篇因超过最大层级（3 级）已合并到上级`);
  }
  if (r.skippedCount > 0) parts.push(`${r.skippedCount} 篇同名已跳过`);
  if (r.failedCount > 0) parts.push(`${r.failedCount} 篇导入失败`);
  showInfoToast(parts.join("，"));
}

async function pickAndImport() {
  const selected = await open({
    multiple: true,
    filters: [{ name: "HTML", extensions: ["html", "htm"] }],
  });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  await tryImport(paths);
}

/** 预检冲突 → 弹文件夹选择器选目标 → 有冲突再弹冲突弹窗 → 导入 */
async function tryImport(paths: string[]) {
  // 先弹文件夹选择器选目标（默认根目录）
  pickerContext = "import";
  folderPickerTitle.value = "导入到…";
  pickerExcludeDocFolderId.value = null;
  pendingPaths.value = paths;
  folderPickerVisible.value = true;
}

/** 选好目标后：预检冲突 → 无冲突直接导入，有冲突弹窗等用户选择 */
async function continueImportAfterPicker() {
  const folderId = pendingImportFolderId.value;
  pendingImportFolderId.value = null;
  if (pendingPaths.value.length === 0) return;
  const conflicts = await store.checkConflicts(pendingPaths.value);
  if (conflicts.length > 0) {
    // 记住目标文件夹，等冲突弹窗回调时用
    pendingImportFolderId.value = folderId;
    conflictList.value = conflicts;
    conflictVisible.value = true;
  } else {
    await store.importWithResolution(pendingPaths.value, undefined, folderId);
    pendingPaths.value = [];
  }
}

/** 用户在冲突弹窗选「全部跳过」 */
async function onConflictSkip() {
  conflictVisible.value = false;
  const folderId = pendingImportFolderId.value;
  pendingImportFolderId.value = null;
  // 跳过所有撞名文件，只导入非冲突项
  await store.importWithResolution(pendingPaths.value, "skip", folderId);
  pendingPaths.value = [];
}

/** 用户在冲突弹窗选「全部覆盖」 */
async function onConflictOverwrite() {
  conflictVisible.value = false;
  const folderId = pendingImportFolderId.value;
  pendingImportFolderId.value = null;
  await store.importWithResolution(pendingPaths.value, "overwrite", folderId);
  pendingPaths.value = [];
}

/** 用户在冲突弹窗选「取消」 */
function onConflictCancel() {
  conflictVisible.value = false;
  pendingPaths.value = [];
  pendingImportFolderId.value = null;
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

// ==================== 文件夹操作 ====================

/** 新建文件夹对话框可见性 + 目标父文件夹（null = 根目录） */
const newFolderVisible = ref(false);
const newFolderParentId = ref<string | null>(null);
const newFolderName = ref("");
const newFolderInputEl = ref<HTMLInputElement | null>(null);

/** 对话框打开时自动聚焦输入框 */
watch(newFolderVisible, (v) => {
  if (v) {
    nextTick(() => {
      newFolderInputEl.value?.focus();
    });
  }
});

/** 文件夹选择器（移动文档 / 导入目标）可见性 + 上下文 */
const folderPickerVisible = ref(false);
const folderPickerTitle = ref("选择文件夹");
/** 'move'（移动文档）| 'import'（导入目标） */
/** 'move'（移动单个文档）| 'batchMove'（批量移动）| 'import'（导入目标） */
let pickerContext: "move" | "batchMove" | "import" | "importFolder" | "moveFolder" = "move";
/** 移动文档模式下：待移动的文档 id（用于 excludeId 防止移到原文件夹） */
const pickerExcludeDocFolderId = ref<string | null>(null);

/** 删除文件夹确认 */
const deleteFolderVisible = ref(false);
const pendingDeleteFolder = ref<{ id: string; name: string } | null>(null);

/** 右键菜单：区分文档 / 文件夹两种目标 */
const contextMenuVisible = ref(false);
const contextMenuX = ref(0);
const contextMenuY = ref(0);
const contextMenuItems = ref<MenuItem[]>([]);
/** 右键菜单底部信息（文档元信息，Notion 风格） */
const contextMenuFooter = ref("");
let contextMenuTarget: { type: "doc"; doc: Document } | { type: "folder"; folderId: string } | null = null;

/** 在根目录新建文件夹（侧边栏头部「+」按钮） */
function onNewFolderAtRoot() {
  newFolderParentId.value = null;
  newFolderName.value = "";
  newFolderVisible.value = true;
}

/** 在指定父文件夹下新建子文件夹（右键菜单「新建子文件夹」） */
function onNewSubfolder(parentId: string) {
  newFolderParentId.value = parentId;
  newFolderName.value = "";
  newFolderVisible.value = true;
}

/** 确认新建文件夹 */
async function confirmNewFolder() {
  const name = newFolderName.value.trim();
  if (!name) {
    newFolderVisible.value = false;
    return;
  }
  newFolderVisible.value = false;
  const folder = await store.createFolder(name, newFolderParentId.value);
  if (folder) {
    // 新建的文件夹自动展开其父（如果有的话）+ 自身默认收起
    if (folder.parentId) {
      const next = new Set(expandedFolderIds.value);
      next.add(folder.parentId);
      expandedFolderIds.value = next;
    }
  }
}

/** 切换文件夹展开/收起 */
function onToggleFolder(folderId: string) {
  const next = new Set(expandedFolderIds.value);
  if (next.has(folderId)) next.delete(folderId);
  else next.add(folderId);
  expandedFolderIds.value = next;
}

/** 提交重命名 */
async function onCommitRename(folderId: string, newName: string) {
  // 同级重名校验：同一 parentId 下不允许出现两个同名文件夹。
  const folder = store.folders.find((f) => f.id === folderId);
  const parentId = folder?.parentId ?? null;
  const dup = store.folders.some(
    (f) => f.id !== folderId && f.parentId === parentId && f.name === newName,
  );
  if (dup) {
    // 不退出重命名态，提示用户改名后重试
    showToast(`已存在同名文件夹「${newName}」，请使用其他名称`, "error", 3000);
    return;
  }
  renamingFolderId.value = null;
  await store.renameFolder(folderId, newName);
}

/** 取消重命名 */
function onCancelRename() {
  renamingFolderId.value = null;
}

/** 拖拽文档到文件夹 */
async function onMoveDoc(docId: string, folderId: string) {
  await store.moveDocument(docId, folderId);
}

/** 列表右键：根据目标类型组装菜单 */
function onItemContextMenu(doc: Document, e: MouseEvent) {
  e.preventDefault();
  contextMenuTarget = { type: "doc", doc };
  contextMenuItems.value = [
    { key: "move", label: "移动到…" },
    { key: "copyPath", label: "复制文件路径" },
    { key: "delete", label: "删除", danger: true },
  ];
  contextMenuFooter.value = `${formatSize(doc.fileSize)} · ${formatDate(doc.importedAt)}`;
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuVisible.value = true;
}

/** 文件夹右键。第 3 级（最深层）不显示「新建子文件夹」，避免无意义地走到报错。 */
function onFolderContextMenu(folderId: string, e: MouseEvent) {
  e.preventDefault();
  contextMenuTarget = { type: "folder", folderId };
  contextMenuFooter.value = "";
  const folder = store.folders.find((f) => f.id === folderId);
  const isMaxLevel = (folder?.level ?? 0) >= 3;
  const items: MenuItem[] = [
    { key: "rename", label: "重命名" },
    { key: "moveFolder", label: "移动到…" },
  ];
  if (!isMaxLevel) {
    items.push({ key: "newSub", label: "新建子文件夹" });
  }
  items.push({ key: "delete", label: "删除", danger: true });
  contextMenuItems.value = items;
  contextMenuX.value = e.clientX;
  contextMenuY.value = e.clientY;
  contextMenuVisible.value = true;
}

/** 右键菜单选中某项 */
function onContextSelect(key: string) {
  contextMenuVisible.value = false;
  const target = contextMenuTarget;
  contextMenuTarget = null;
  if (!target) return;

  if (target.type === "doc") {
    const doc = target.doc;
    // 多选模式下：右键的文档若在选中集合里，则整批操作；否则只操作右键的这一篇。
    const batchEligible = selectMode.value && selectedIds.value.has(doc.id);
    if (key === "move") {
      if (batchEligible && selectedIds.value.size > 1) {
        // 批量移动选中项
        pickerContext = "batchMove";
        folderPickerTitle.value = `移动 ${selectedIds.value.size} 篇到…`;
        pickerExcludeDocFolderId.value = null;
        folderPickerVisible.value = true;
      } else {
        // 单篇移动
        pickerContext = "move";
        folderPickerTitle.value = "移动到…";
        pickerExcludeDocFolderId.value = doc.folderId ?? null;
        pendingMoveDocId.value = doc.id;
        folderPickerVisible.value = true;
      }
    } else if (key === "copyPath") {
      copyPath(doc);
    } else if (key === "delete") {
      if (batchEligible && selectedIds.value.size > 1) {
        // 批量删除选中项
        batchDeleteVisible.value = true;
      } else {
        removeDoc(doc);
      }
    }
  } else if (target.type === "folder") {
    const folderId = target.folderId;
    if (key === "rename") {
      renamingFolderId.value = folderId;
    } else if (key === "moveFolder") {
      pickerContext = "moveFolder";
      folderPickerTitle.value = "移动到…";
      pickerExcludeDocFolderId.value = folderId;
      pendingMoveFolderId.value = folderId;
      folderPickerVisible.value = true;
    } else if (key === "newSub") {
      onNewSubfolder(folderId);
    } else if (key === "delete") {
      const folder = store.folders.find((f) => f.id === folderId);
      pendingDeleteFolder.value = { id: folderId, name: folder?.name ?? "" };
      deleteFolderVisible.value = true;
    }
  }
}

/** 移动文档模式下：待移动的文档 id */
const pendingMoveDocId = ref<string>("");

/** 移动文件夹模式下：待移动的文件夹 id */
const pendingMoveFolderId = ref<string>("");

/** 文件夹选择器确认 */
async function onFolderPickerConfirm(folderId: string | null) {
  folderPickerVisible.value = false;
  if (pickerContext === "move") {
    const docId = pendingMoveDocId.value;
    pendingMoveDocId.value = "";
    if (docId) {
      await store.moveDocument(docId, folderId);
    }
  } else if (pickerContext === "batchMove") {
    // 批量移动：循环 moveDocument，统计成功/失败
    const ids = Array.from(selectedIds.value);
    let ok = 0;
    let fail = 0;
    for (const id of ids) {
      const success = await store.moveDocument(id, folderId);
      if (success) ok += 1;
      else fail += 1;
    }
    if (fail > 0) {
      showInfoToast(`已移动 ${ok} 篇，${fail} 篇失败`);
    } else {
      showInfoToast(`已移动 ${ok} 篇文档`);
    }
    exitSelectMode();
  } else if (pickerContext === "import") {
    // 导入目标选定后：预检冲突 → 导入
    pendingImportFolderId.value = folderId;
    await continueImportAfterPicker();
  } else if (pickerContext === "importFolder") {
    // 导入文件夹：选定目标后直接导入（不预检冲突）
    const dir = pendingImportDir.value;
    pendingImportDir.value = "";
    if (!dir) return;
    const result = await store.importDirectory(dir, folderId);
    if (!result) return;
    showImportResultToast(result);
  } else if (pickerContext === "moveFolder") {
    // 移动文件夹到目标位置
    const fid = pendingMoveFolderId.value;
    pendingMoveFolderId.value = "";
    if (!fid) return;
    await store.moveFolder(fid, folderId);
  }
}

function onFolderPickerCancel() {
  folderPickerVisible.value = false;
  pendingMoveDocId.value = "";
  if (pickerContext === "import") {
    // 取消导入目标选择 = 放弃整个导入
    pendingPaths.value = [];
  }
  if (pickerContext === "importFolder") {
    pendingImportDir.value = "";
  }
  if (pickerContext === "moveFolder") {
    pendingMoveFolderId.value = "";
  }
}

/** 删除文件夹确认 */
async function onConfirmDeleteFolder() {
  const target = pendingDeleteFolder.value;
  if (!target) return;
  deleteFolderVisible.value = false;
  await store.deleteFolder(target.id);
  pendingDeleteFolder.value = null;
}

/** 导入流程：选完文件后弹出文件夹选择器选目标 */
const pendingImportFolderId = ref<string | null>(null);

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
          <!-- 导入下拉：导入文件 / 导入文件夹 -->
          <div class="import-dropdown">
            <button
              class="btn btn-primary import-trigger"
              :disabled="store.importing > 0"
              @click="toggleImportDropdown"
            >
              {{ store.importing > 0 ? "导入中…" : "导入" }}
              <ChevronDown v-if="!store.importing" :size="12" :stroke-width="1.8" />
            </button>
            <div v-if="importDropdownOpen" class="import-menu">
              <button class="import-menu-item" @click="onPickImportFiles">
                <FileUp :size="14" :stroke-width="1.5" />
                <span>导入 HTML 文件</span>
              </button>
              <button class="import-menu-item" @click="onPickImportFolder">
                <FolderUp :size="14" :stroke-width="1.5" />
                <span>导入文件夹</span>
              </button>
            </div>
          </div>
        </template>
      </div>
    </header>

    <!-- 主内容：左右分栏 -->
    <main class="content">
      <!-- 左侧：文档列表 -->
      <aside id="sidebar" class="sidebar" :class="{ collapsed: sidebarCollapsed, resizing: isResizing }" :style="{ width: sidebarWidth + 'px' }">
        <div class="sidebar-inner">
          <div class="sidebar-head">
            <span class="sidebar-title">文档 <span v-if="hasDocuments" class="sidebar-count">{{ store.documents.length }}</span></span>
            <div class="sidebar-head-right">
              <button
                v-if="hasDocuments"
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

            <!-- 空库时侧边栏留空：引导由右侧预览区欢迎卡片承担 -->

            <!-- 目录树 + 根目录文档 -->
            <template v-else>
              <!-- 文件夹树（一级及嵌套） -->
              <FolderTree
                :nodes="store.tree"
                :level="0"
                :active-doc-id="currentDoc?.id"
                :expanded-ids="expandedFolderIds"
                :renaming-id="renamingFolderId"
                :select-mode="selectMode"
                :selected-ids="selectedIds"
                @select-doc="selectDoc"
                @doc-contextmenu="onItemContextMenu"
                @folder-contextmenu="onFolderContextMenu"
                @toggle="onToggleFolder"
                @move-doc="onMoveDoc"
                @commit-rename="onCommitRename"
                @cancel-rename="onCancelRename"
                @toggle-select="toggleSelect"
              />
              <!-- 根目录下散落的文档（folderId 为空） -->
              <DocumentListItem
                v-for="doc in store.rootDocuments"
                :key="doc.id"
                :doc="doc"
                :active="currentDoc?.id === doc.id"
                :select-mode="selectMode"
                :selected="selectMode ? selectedIds.has(doc.id) : false"
                @click="selectDoc(doc)"
                @contextmenu="(d, ev) => onItemContextMenu(d, ev)"
                @toggle="toggleSelect"
              />
            </template>
          </div>

          <!-- 多选模式：底部操作栏 -->
          <div v-if="selectMode" class="select-actionbar">
            <div class="select-actionbar-left">
              <span class="select-count">已选 {{ selectedIds.size }} 篇</span>
              <button
                class="select-link-btn"
                @click="selectedIds.size === store.documents.length ? deselectAll() : selectAll()"
              >
                {{ selectedIds.size === store.documents.length ? '取消全选' : '全选' }}
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

      <!-- 侧边栏拖拽手柄 -->
      <div
        class="sidebar-handle"
        :class="{ active: isResizing }"
        @mousedown="onResizeStart"
      />

      <!-- 右侧：预览 -->
      <section class="preview">
        <!-- 有文档选中 -->
        <template v-if="currentDoc">
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
          <!-- 空库：引导卡片（两个主 CTA 按钮） -->
          <div v-if="!store.hasItems && !store.loading" class="welcome-card">
            <div class="welcome-icon">
              <FolderOpen :size="40" :stroke-width="1.3" />
            </div>
            <h2 class="welcome-title">欢迎使用 Leaf</h2>
            <p class="welcome-subtitle">把你的 HTML 文档导入到这里，开始阅读与管理</p>
            <div class="welcome-actions">
              <button class="btn btn-primary welcome-btn" @click="onPickImportFiles">
                <FileUp :size="14" :stroke-width="1.5" />
                <span>导入 HTML 文件</span>
              </button>
              <button class="btn welcome-btn" @click="onPickImportFolder">
                <FolderUp :size="14" :stroke-width="1.5" />
                <span>导入文件夹</span>
              </button>
            </div>
          </div>

          <!-- 有内容但未选中：轻量占位 -->
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

    <!-- 列表项 / 文件夹右键菜单 -->
    <ContextMenu
      :visible="contextMenuVisible"
      :x="contextMenuX"
      :y="contextMenuY"
      :items="contextMenuItems"
      :footer="contextMenuFooter"
      @select="onContextSelect"
      @close="contextMenuVisible = false"
    />

    <!-- 统一 toast：顶部居中浮层（替换所有右下角 / 分散位置的提示） -->
    <transition name="fade">
      <div v-if="toastVisible" class="toast" :class="toastType">
        {{ toastMessage }}
      </div>
    </transition>

    <!-- 新建文件夹对话框 -->
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

    <!-- 文件夹选择器（移动文档 / 导入目标） -->
    <FolderPickerDialog
      :visible="folderPickerVisible"
      :folders="store.folders"
      :title="folderPickerTitle"
      :exclude-id="pickerExcludeDocFolderId"
      @confirm="onFolderPickerConfirm"
      @cancel="onFolderPickerCancel"
    />

    <!-- 删除文件夹确认 -->
    <ConfirmDialog
      :visible="deleteFolderVisible"
      :title="`删除文件夹「${pendingDeleteFolder?.name ?? ''}」？`"
      message="将同时删除其下所有子文件夹与文档，该操作不可撤销。"
      confirm-text="删除"
      :danger="true"
      @confirm="onConfirmDeleteFolder"
      @cancel="deleteFolderVisible = false; pendingDeleteFolder = null"
    />

    <!-- 批量删除确认 -->
    <ConfirmDialog
      :visible="batchDeleteVisible"
      :title="`删除选中的 ${selectedIds.size} 篇文档？`"
      message="该操作不可撤销，确认删除这些文档吗？"
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
/* 实心危险按钮（批量删除等），复用 --danger 变量 */
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
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border);
  transition: width 0.25s ease, border-color 0.25s ease;
}
/* 拖拽时禁用过渡，跟手丝滑 */
.sidebar.resizing {
  transition: none;
}
.sidebar.collapsed {
  width: 0;
  border-right-color: transparent;
}
.sidebar-inner {
  min-width: 260px;
  flex: 1;
  display: flex;
  flex-direction: column;
}

/* 侧边栏拖拽手柄 */
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
/* 新建文件夹按钮微调：向右偏移 2px 让图标组与右侧边缘更协调 */
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

/* 多选模式：底部固定操作栏 */
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
/* 统一 toast：顶部居中浮层，基于 present-hint 设计 */
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
/* error 类型：暖红色文本 */
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

/* 空库引导卡片（位于右侧预览区，宽区，按钮横排） */
.welcome-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  max-width: 360px;
  text-align: center;
}
.welcome-icon {
  color: var(--text-faint);
  opacity: 0.7;
  margin-bottom: 4px;
}
.welcome-title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--text);
}
.welcome-subtitle {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.5;
}
.welcome-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  justify-content: center;
}
.welcome-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
}

/* ---------- 对话框（新建文件夹） ---------- */
/* 遮罩 + 三层阴影 + 圆角 + 过渡，复用规范公式 */
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

/* 对话框按钮：与 ConfirmDialog 一致 */
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

/* 对话框过渡（规范：0.18s ease） */
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

/* 导入下拉 */
.import-dropdown {
  position: relative;
}
.import-trigger {
  display: flex;
  align-items: center;
  gap: 4px;
}
.import-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: 50;
  min-width: 180px;
  padding: 4px;
  background: var(--bg);
  border-radius: 8px;
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
}
.import-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 10px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
  transition: background 0.1s;
}
.import-menu-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}
</style>
