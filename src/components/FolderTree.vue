<script setup lang="ts">
/**
 * 文件夹目录树（递归组件）。
 *
 * 渲染规则：每个树节点 = 一个文件夹行 + 其下文档 + 递归的子文件夹。
 * 支持展开收起、右键菜单、拖拽文档进入。
 *
 * 视觉严格遵循 01 设计规范：暖灰半透明叠层、Lucide 图标 14/1.5、
 * 列表项圆角 6px、悬停 0.1s、展开过渡 0.25s ease、缩进每级 12px。
 */
import { ref, watch } from "vue";
import { ChevronRight, ChevronDown, Folder, FolderOpen } from "@lucide/vue";
import type { Document, FolderTreeNode } from "../types/document";
import DocumentListItem from "./DocumentListItem.vue";

/** 自定义指令：元素插入时自动聚焦并全选文本（用于重命名输入框） */
const vFocus = {
  mounted(el: HTMLInputElement) {
    el.focus();
    el.select();
  },
};

const props = defineProps<{
  /** 当前层级的树节点数组 */
  nodes: FolderTreeNode[]
  /** 层级深度（根 = 0，用于缩进计算） */
  level: number
  /** 当前选中的文档 id（高亮） */
  activeDocId?: string
  /** 展开的文件夹 id 集合（由顶层统一管理状态） */
  expandedIds: Set<string>
  /** 正在内联重命名的文件夹 id */
  renamingId?: string | null
  /** 正在内联重命名的文档 id */
  renamingDocId?: string | null
  /** 是否处于多选模式 */
  selectMode?: boolean
  /** 多选模式下选中的文档 id 集合 */
  selectedIds?: Set<string>
}>();

const emit = defineEmits<{
  /** 点击文档 */
  selectDoc: [doc: Document]
  /** 文档右键 */
  docContextmenu: [doc: Document, event: MouseEvent]
  /** 文件夹右键 */
  folderContextmenu: [folderId: string, event: MouseEvent]
  /** 切换文件夹展开/收起 */
  toggle: [folderId: string]
  /** 拖拽文档到文件夹释放 */
  moveDoc: [docId: string, folderId: string]
  /** 提交文件夹重命名（Enter / blur） */
  commitRename: [folderId: string, newName: string]
  /** 取消文件夹重命名（Esc） */
  cancelRename: []
  /** 提交文档重命名（Enter / blur）。newName 为不含后缀的主名 */
  commitDocRename: [docId: string, newName: string]
  /** 取消文档重命名（Esc） */
  cancelDocRename: []
  /** 多选模式下切换文档选中态 */
  toggleSelect: [docId: string]
}>();

// 拖拽高亮的文件夹 id
const dragOverId = ref<string | null>(null);
// 重命名输入框的当前值
const renameInput = ref("");

/**
 * 进入重命名态时，预填当前文件夹的名字（并依赖 v-focus 指令自动全选）。
 * 否则 renameInput 会残留上一次输入的值，导致"带入上一个文件夹名"。
 */
watch(
  () => props.renamingId,
  (id) => {
    if (!id) return;
    const node = findNodeById(props.nodes, id);
    renameInput.value = node?.folder.name ?? "";
  },
);

/** 在当前层级（含递归子层级）中按 id 查找节点 */
function findNodeById(nodes: FolderTreeNode[], id: string): FolderTreeNode | undefined {
  for (const n of nodes) {
    if (n.folder.id === id) return n;
    const hit = findNodeById(n.children, id);
    if (hit) return hit;
  }
  return undefined;
}

function isExpanded(folderId: string): boolean {
  return props.expandedIds.has(folderId);
}

/** 点击文件夹行：切换展开 + 不阻止文档选中（文件夹本身不"选中"文档） */
function onFolderClick(node: FolderTreeNode) {
  emit("toggle", node.folder.id);
}

/** 文件夹右键 */
function onFolderContextmenu(node: FolderTreeNode, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  emit("folderContextmenu", node.folder.id, e);
}

/** 文档拖到文件夹上 */
function onDragover(node: FolderTreeNode, e: DragEvent) {
  // 仅当拖的是文档（带 text/doc-id）时才高亮
  if (e.dataTransfer?.types.includes("text/doc-id")) {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    dragOverId.value = node.folder.id;
  }
}

function onDragleave(node: FolderTreeNode) {
  if (dragOverId.value === node.folder.id) {
    dragOverId.value = null;
  }
}

function onDrop(node: FolderTreeNode, e: DragEvent) {
  e.preventDefault();
  e.stopPropagation();
  dragOverId.value = null;
  const docId = e.dataTransfer?.getData("text/doc-id");
  if (docId) {
    emit("moveDoc", docId, node.folder.id);
  }
}

/** 重命名输入框：Enter 提交 / Esc 取消 */
function onRenameKeydown(node: FolderTreeNode, e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    const name = renameInput.value.trim();
    if (name && name !== node.folder.name) {
      emit("commitRename", node.folder.id, name);
    } else {
      emit("cancelRename");
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("cancelRename");
  }
}

/** 重命名输入框失焦：提交 */
function onRenameBlur(node: FolderTreeNode) {
  const name = renameInput.value.trim();
  if (name && name !== node.folder.name) {
    emit("commitRename", node.folder.id, name);
  } else {
    emit("cancelRename");
  }
}
</script>

<template>
  <div class="folder-tree">
    <!-- 遍历当前层级的每个节点 -->
    <div v-for="node in nodes" :key="node.folder.id" class="tree-node">
      <!-- 文件夹行 -->
      <div
        class="folder-row"
        :class="{ 'drag-over': dragOverId === node.folder.id }"
        :style="{ paddingLeft: 8 + level * 8 + 'px' }"
        @click="onFolderClick(node)"
        @contextmenu="onFolderContextmenu(node, $event)"
        @dragover="onDragover(node, $event)"
        @dragleave="onDragleave(node)"
        @drop="onDrop(node, $event)"
      >
        <!-- 展开/收起箭头：图标显示「点击后的动作」（规范 4.2） -->
        <component
          :is="isExpanded(node.folder.id) ? ChevronDown : ChevronRight"
          :size="14"
          :stroke-width="1.5"
          class="chevron"
        />
        <!-- 文件夹图标：收起用 Folder，展开用 FolderOpen -->
        <component
          :is="isExpanded(node.folder.id) ? FolderOpen : Folder"
          :size="14"
          :stroke-width="1.5"
          class="folder-icon"
        />
        <!-- 名称或重命名输入框 -->
        <input
          v-if="renamingId === node.folder.id"
          v-focus
          v-model="renameInput"
          class="rename-input"
          type="text"
          @click.stop
          @dblclick.stop
          @contextmenu.stop
          @keydown="onRenameKeydown(node, $event)"
          @blur="onRenameBlur(node)"
        />
        <span v-else class="folder-name">{{ node.folder.name }}</span>
      </div>

      <!-- 展开内容：子文档 + 递归子文件夹 -->
      <transition name="expand">
        <div v-show="isExpanded(node.folder.id)" class="folder-content">
          <!-- 文档 -->
          <DocumentListItem
            v-for="doc in node.docs"
            :key="doc.id"
            :doc="doc"
            :active="activeDocId === doc.id"
            :indent="level + 1"
            :select-mode="selectMode"
            :selected="selectMode ? selectedIds?.has(doc.id) : false"
            :renaming="renamingDocId === doc.id"
            @click="emit('selectDoc', doc)"
            @contextmenu="(d, ev) => emit('docContextmenu', d, ev)"
            @toggle="(did: string) => emit('toggleSelect', did)"
            @commit-rename="(did: string, n: string) => emit('commitDocRename', did, n)"
            @cancel-rename="emit('cancelDocRename')"
          />
          <!-- 递归子文件夹 -->
          <FolderTree
            :nodes="node.children"
            :level="level + 1"
            :active-doc-id="activeDocId"
            :expanded-ids="expandedIds"
            :renaming-id="renamingId"
            :renaming-doc-id="renamingDocId"
            :select-mode="selectMode"
            :selected-ids="selectedIds"
            @select-doc="(d: Document) => emit('selectDoc', d)"
            @doc-contextmenu="(d: Document, ev: MouseEvent) => emit('docContextmenu', d, ev)"
            @folder-contextmenu="(fid: string, ev: MouseEvent) => emit('folderContextmenu', fid, ev)"
            @toggle="(fid: string) => emit('toggle', fid)"
            @move-doc="(did: string, fid: string) => emit('moveDoc', did, fid)"
            @commit-rename="(fid: string, n: string) => emit('commitRename', fid, n)"
            @cancel-rename="emit('cancelRename')"
            @commit-doc-rename="(did: string, n: string) => emit('commitDocRename', did, n)"
            @cancel-doc-rename="emit('cancelDocRename')"
            @toggle-select="(did: string) => emit('toggleSelect', did)"
          />
        </div>
      </transition>
    </div>
  </div>
</template>

<style scoped>
.folder-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 8px;
  padding-right: 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.1s;
  user-select: none;
}
.folder-row:hover {
  background: var(--bg-hover);
}
.folder-row.drag-over {
  background: var(--bg-active);
}

.chevron {
  color: var(--text-faint);
  flex-shrink: 0;
}
.folder-icon {
  color: var(--text-dim);
  flex-shrink: 0;
}
.folder-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rename-input {
  flex: 1;
  min-width: 0;
  padding: 1px 4px;
  margin: 0;
  border: 1px solid var(--accent-blue);
  border-radius: 4px;
  background: var(--bg);
  font-size: 14px;
  font-weight: 500;
  color: var(--text);
  font-family: inherit;
  outline: none;
}

/* 展开内容：左边线视觉包裹 */
.folder-content {
  border-left: 1px solid var(--border);
  margin-left: 14px;
  padding-left: 6px;
}

/* 展开过渡（规范：0.25s ease） */
.expand-enter-active,
.expand-leave-active {
  transition: opacity 0.25s ease;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  opacity: 0;
}
</style>
