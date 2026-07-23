<script setup lang="ts">
/**
 * 文件夹选择对话框：树形单选，含「根目录」选项。
 * 用于「移动到…」场景。
 *
 * 视觉遵循设计规范：Notion 三层阴影浮层、0.18s fade/pop 过渡、Lucide 图标 14/1.5、
 * 暖灰文本变量、半透明 hover 叠层。
 *
 * 渲染策略：把树拍平成「带 depth + 是否可见」的一维数组，
 * 某节点可见当且仅当其所有祖先都处于展开态。
 */
import { computed, ref, watch } from "vue";
import {
  Folder as FolderIcon,
  FolderOpen as FolderOpenIcon,
  Home,
  ChevronRight,
  ChevronDown,
} from "@lucide/vue";
import type { VaultDir } from "../types/document";

const props = defineProps<{
  visible: boolean
  folders: VaultDir[]
  /** 对话框标题 */
  title?: string
  /** 仓库根目录的自定义名称（默认"根目录"） */
  rootName?: string
  /** 默认选中的目录 relPath（"" = 根目录） */
  defaultSelected?: string
  /** 排除某个目录 relPath 及其子目录（防止把文档移到当前目录下） */
  excludeDirRel?: string | null
  /** 排除层级 >= 此值的目录（用于文件夹移动时的层级限制），默认不限制 */
  maxTargetLevel?: number
}>();

const emit = defineEmits<{
  confirm: [dirRel: string]
  cancel: []
}>();

/** 当前选中的目录 relPath（"" = 根目录） */
const selected = ref("");

/** 对话框打开时初始化选中态，默认展开所有文件夹 */
watch(
  () => props.visible,
  (v) => {
    if (v) {
      selected.value = props.defaultSelected ?? "";
      expandedIds.value = new Set(availableFolders.value.map((f) => f.relPath));
    }
  },
);

/** 展开的文件夹 relPath 集合 */
const expandedIds = ref<Set<string>>(new Set());

/** 可用文件夹（排除 excludeDirRel 及其子孙 + maxTargetLevel 限制） */
const availableFolders = computed<VaultDir[]>(() => {
  let list = props.folders;

  // 排除 excludeDirRel 及其子孙
  if (props.excludeDirRel) {
    const excluded = new Set<string>([props.excludeDirRel]);
    const collectChildren = (parentRel: string) => {
      for (const f of props.folders) {
        if (f.parentPath === parentRel) {
          excluded.add(f.relPath);
          collectChildren(f.relPath);
        }
      }
    };
    collectChildren(props.excludeDirRel);
    list = list.filter((f) => !excluded.has(f.relPath));
  }

  // 排除层级 >= maxTargetLevel 的目录
  if (props.maxTargetLevel !== undefined) {
    list = list.filter((f) => f.level < props.maxTargetLevel!);
  }

  return list;
});

interface FlatRow {
  folder: VaultDir
  depth: number
}

/** 拍平后的行：folder + depth + 仅展开态可见 */
const flatRows = computed<FlatRow[]>(() => {
  const rows: FlatRow[] = [];
  const walk = (parentPath: string | null, depth: number) => {
    availableFolders.value
      .filter((f) => f.parentPath === parentPath)
      .forEach((folder) => {
        rows.push({ folder, depth });
        if (expandedIds.value.has(folder.relPath)) {
          walk(folder.relPath, depth + 1);
        }
      });
  };
  walk(null, 0);
  return rows;
});

function isExpanded(relPath: string): boolean {
  return expandedIds.value.has(relPath);
}

function toggle(relPath: string) {
  const next = new Set(expandedIds.value);
  if (next.has(relPath)) next.delete(relPath);
  else next.add(relPath);
  expandedIds.value = next;
}

function onSelect(dirRel: string) {
  selected.value = dirRel;
}

function onConfirm() {
  emit("confirm", selected.value);
}

function onCancel() {
  emit("cancel");
}
</script>

<template>
  <transition name="fade">
    <div v-if="visible" class="overlay" @click.self="onCancel">
      <transition name="pop" appear>
        <div v-if="visible" class="dialog">
          <div class="dialog-title">{{ title || '选择文件夹' }}</div>
          <div class="tree-body">
            <!-- 根目录选项 -->
            <div
              class="tree-row root-row"
              :class="{ selected: selected === '' }"
              @click="onSelect('')"
            >
              <Home :size="14" :stroke-width="1.5" class="row-icon root-icon" />
              <span class="row-label">{{ rootName || '根目录' }}</span>
            </div>
            <!-- 拍平后的文件夹行 -->
            <div
              v-for="row in flatRows"
              :key="row.folder.relPath"
              class="tree-row"
              :class="{ selected: selected === row.folder.relPath }"
              :style="{ paddingLeft: 12 + (row.depth + 1) * 12 + 'px' }"
              @click="onSelect(row.folder.relPath)"
            >
              <component
                :is="isExpanded(row.folder.relPath) ? ChevronDown : ChevronRight"
                :size="14"
                :stroke-width="1.5"
                class="row-chevron"
                @click.stop="toggle(row.folder.relPath)"
              />
              <component
                :is="isExpanded(row.folder.relPath) ? FolderOpenIcon : FolderIcon"
                :size="14"
                :stroke-width="1.5"
                class="row-icon"
              />
              <span class="row-label">{{ row.folder.name }}</span>
            </div>
          </div>
          <div class="dialog-actions">
            <button class="btn btn-cancel" @click="onCancel">取消</button>
            <button class="btn btn-confirm" @click="onConfirm">确定</button>
          </div>
        </div>
      </transition>
    </div>
  </transition>
</template>

<style scoped>
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
  width: 380px;
  max-width: 90%;
}

.dialog-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text);
  margin-bottom: 14px;
}

.tree-body {
  max-height: 320px;
  overflow-y: auto;
  margin-bottom: 18px;
}

.tree-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 5px 8px;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.1s;
  user-select: none;
}
.tree-row:hover {
  background: var(--bg-hover);
}
.tree-row.selected {
  background: var(--bg-active);
}
.tree-row.selected .row-label {
  color: var(--text);
  font-weight: 600;
}

.row-chevron {
  color: var(--text-faint);
  flex-shrink: 0;
}
.row-icon {
  color: var(--text-dim);
  flex-shrink: 0;
}
.root-icon {
  color: var(--text-dim);
}
.row-label {
  font-size: 13px;
  color: var(--text-dim);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.btn {
  padding: 5px 16px;
  border-radius: 5px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  border: none;
  transition: background 0.1s;
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

/* 过渡动画 */
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
