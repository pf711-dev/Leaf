<script setup lang="ts">
/**
 * 文件夹目录树（递归组件）。
 *
 * 渲染规则：每个树节点 = 一个文件夹行 + 其下文件 + 递归的子文件夹。
 * 支持展开收起、右键菜单。
 *
 * 视觉严格遵循 01 设计规范：暖灰半透明叠层、Lucide 图标 14/1.5、
 * 列表项圆角 6px、悬停 0.1s、展开过渡 0.25s ease、缩进每级 8px。
 */
import { ref, watch } from "vue";
import { ChevronRight, ChevronDown, Folder, FolderOpen } from "@lucide/vue";
import type { DirTreeNode, VaultFile } from "../types/document";
import DocumentListItem from "./DocumentListItem.vue";

const vFocus = {
  mounted(el: HTMLInputElement) {
    el.focus();
    el.select();
  },
};

const props = defineProps<{
  nodes: DirTreeNode[]
  level: number
  activeFileId?: string
  expandedIds: Set<string>
  renamingId?: string | null
  renamingFileId?: string | null
  selectMode?: boolean
  selectedIds?: Set<string>
}>();

const emit = defineEmits<{
  selectFile: [file: VaultFile]
  fileContextmenu: [file: VaultFile, event: MouseEvent]
  folderContextmenu: [dirRel: string, event: MouseEvent]
  toggle: [dirRel: string]
  commitRename: [dirRel: string, newName: string]
  cancelRename: []
  commitFileRename: [fileId: string, newName: string]
  cancelFileRename: []
  toggleSelect: [fileId: string]
}>();

const renameInput = ref("");

watch(
  () => props.renamingId,
  (id) => {
    if (!id) return;
    const node = findNodeByDirRel(props.nodes, id);
    renameInput.value = node?.dir.name ?? "";
  },
);

function findNodeByDirRel(nodes: DirTreeNode[], dirRel: string): DirTreeNode | undefined {
  for (const n of nodes) {
    if (n.dir.relPath === dirRel) return n;
    const hit = findNodeByDirRel(n.children, dirRel);
    if (hit) return hit;
  }
  return undefined;
}

function isExpanded(dirRel: string): boolean {
  return props.expandedIds.has(dirRel);
}

function onFolderClick(node: DirTreeNode) {
  emit("toggle", node.dir.relPath);
}

function onFolderContextmenu(node: DirTreeNode, e: MouseEvent) {
  e.preventDefault();
  e.stopPropagation();
  emit("folderContextmenu", node.dir.relPath, e);
}

function onRenameKeydown(node: DirTreeNode, e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    const name = renameInput.value.trim();
    if (name && name !== node.dir.name) {
      emit("commitRename", node.dir.relPath, name);
    } else {
      emit("cancelRename");
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("cancelRename");
  }
}

function onRenameBlur(node: DirTreeNode) {
  const name = renameInput.value.trim();
  if (name && name !== node.dir.name) {
    emit("commitRename", node.dir.relPath, name);
  } else {
    emit("cancelRename");
  }
}

/** 将 VaultFile 转为 DocumentListItem 兼容格式 */
function toDocItem(f: VaultFile) {
  return {
    id: f.id,
    title: f.title,
    fileName: f.fileName,
    libraryPath: f.relPath,
    fileSize: f.fileSize,
    summary: f.summary,
    importedAt: f.lastModified,
    sourceCreatedAt: f.lastModified,
  };
}
</script>

<template>
  <div class="folder-tree">
    <div v-for="node in nodes" :key="node.dir.relPath" class="tree-node">
      <!-- 文件夹行 -->
      <div
        class="folder-row"
        :style="{ paddingLeft: 8 + level * 8 + 'px' }"
        @click="onFolderClick(node)"
        @contextmenu="onFolderContextmenu(node, $event)"
      >
        <component
          :is="isExpanded(node.dir.relPath) ? ChevronDown : ChevronRight"
          :size="14"
          :stroke-width="1.5"
          class="chevron"
        />
        <component
          :is="isExpanded(node.dir.relPath) ? FolderOpen : Folder"
          :size="14"
          :stroke-width="1.5"
          class="folder-icon"
        />
        <input
          v-if="renamingId === node.dir.relPath"
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
        <span v-else class="folder-name">{{ node.dir.name }}</span>
      </div>

      <!-- 展开内容 -->
      <transition name="expand">
        <div v-show="isExpanded(node.dir.relPath)" class="folder-content">
          <DocumentListItem
            v-for="f in node.files"
            :key="f.id"
            :doc="toDocItem(f)"
            :active="activeFileId === f.id"
            :indent="level + 1"
            :select-mode="selectMode"
            :selected="selectMode ? selectedIds?.has(f.id) : false"
            :renaming="renamingFileId === f.id"
            @click="emit('selectFile', f)"
            @contextmenu="(_d, ev) => emit('fileContextmenu', f, ev)"
            @toggle="(_did: string) => emit('toggleSelect', f.id)"
            @commit-rename="(_did: string, n: string) => emit('commitFileRename', f.id, n)"
            @cancel-rename="emit('cancelFileRename')"
          />
          <FolderTree
            :nodes="node.children"
            :level="level + 1"
            :active-file-id="activeFileId"
            :expanded-ids="expandedIds"
            :renaming-id="renamingId"
            :renaming-file-id="renamingFileId"
            :select-mode="selectMode"
            :selected-ids="selectedIds"
            @select-file="(f: VaultFile) => emit('selectFile', f)"
            @file-contextmenu="(f: VaultFile, ev: MouseEvent) => emit('fileContextmenu', f, ev)"
            @folder-contextmenu="(dirRel: string, ev: MouseEvent) => emit('folderContextmenu', dirRel, ev)"
            @toggle="(dirRel: string) => emit('toggle', dirRel)"
            @commit-rename="(dirRel: string, n: string) => emit('commitRename', dirRel, n)"
            @cancel-rename="emit('cancelRename')"
            @commit-file-rename="(fid: string, n: string) => emit('commitFileRename', fid, n)"
            @cancel-file-rename="emit('cancelFileRename')"
            @toggle-select="(fid: string) => emit('toggleSelect', fid)"
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

.folder-content {
  border-left: 1px solid var(--border);
  margin-left: 14px;
  padding-left: 6px;
}

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
