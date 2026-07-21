<script setup lang="ts">
import type { Document } from "../types/document";
import { formatDate, formatSize } from "../utils/format";
import { Square, CheckSquare } from "@lucide/vue";

const props = defineProps<{
  doc: Document
  active: boolean
  /** 树内缩进（像素），用于在文件夹内对齐层级。默认 0（根目录）。 */
  indent?: number
  /** 是否处于多选模式 */
  selectMode?: boolean
  /** 当前是否被选中（多选模式下） */
  selected?: boolean
}>();

const emit = defineEmits<{
  contextmenu: [doc: Document, event: MouseEvent]
  /** 拖拽开始：携带文档 id */
  dragstart: [docId: string]
  /** 多选模式下点击：切换选中态 */
  toggle: [docId: string]
}>();

function onContextMenu(e: MouseEvent) {
  emit("contextmenu", props.doc, e);
}

function onDragstart(e: DragEvent) {
  // 多选模式下禁用拖拽（避免和勾选冲突）
  if (props.selectMode) {
    e.preventDefault();
    return;
  }
  // 通过 dataTransfer 携带文档 id，供文件夹 drop 时读取
  e.dataTransfer?.setData("text/doc-id", props.doc.id);
  e.dataTransfer!.effectAllowed = "move";
  emit("dragstart", props.doc.id);
}

function onClick() {
  // 多选模式下点击切换选中，不预览
  if (props.selectMode) {
    emit("toggle", props.doc.id);
  }
}
</script>

<template>
  <div
    class="list-item"
    :class="{ active: !selectMode && active, selected: selectMode && selected }"
    :style="indent ? { paddingLeft: 8 + indent * 12 + 'px' } : undefined"
    :draggable="!selectMode"
    @click="onClick"
    @contextmenu="onContextMenu"
    @dragstart="onDragstart"
  >
    <!-- 多选模式：左侧勾选框 -->
    <component
      v-if="selectMode"
      :is="selected ? CheckSquare : Square"
      :size="14"
      :stroke-width="1.5"
      class="check-icon"
      :class="{ checked: selected }"
    />
    <div class="list-item-main">
      <!-- 名称（默认只显示这一行，保持简洁） -->
      <div class="title">{{ doc.fileName }}</div>
      <!-- 悬停时滑出的元信息条：大小 · 导入时间 -->
      <div class="meta">{{ formatSize(doc.fileSize) }}<span class="meta-sep">·</span>{{ formatDate(doc.importedAt) }}</div>
    </div>
  </div>
</template>

<style scoped>
.list-item {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 5px 8px;          /* 收紧：原 6px → 5px，因默认只一行 */
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.1s;
}
.list-item:hover {
  background: var(--bg-hover);
}
/* 普通预览选中态 */
.list-item.active {
  background: var(--bg-active);
}
.list-item.active .title {
  color: var(--text);
  font-weight: 600;
}
/* 多选选中态：同样用 --bg-active 半透明叠层 */
.list-item.selected {
  background: var(--bg-active);
}
.list-item.selected .title {
  color: var(--text);
}

/* 多选勾选框 */
.check-icon {
  flex-shrink: 0;
  margin-top: 2px;
  color: var(--text-faint);
}
.check-icon.checked {
  color: var(--accent-blue);
}

.list-item-main {
  flex: 1;
  min-width: 0;
}

.title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-dim);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 元信息条：默认隐藏，悬停时滑出（不占据空间，避免推动下方项） */
.meta {
  max-height: 0;
  opacity: 0;
  overflow: hidden;
  font-size: 12px;
  color: var(--text-faint);
  white-space: nowrap;
  transition: max-height 0.15s ease, opacity 0.15s ease;
}
.list-item:hover .meta {
  max-height: 18px;
  opacity: 1;
}
.meta-sep {
  margin: 0 5px;
  opacity: 0.6;
}
</style>
