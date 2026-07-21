<script setup lang="ts">
import { ref, watch } from "vue";
import type { Document } from "../types/document";
import { Square, CheckSquare } from "@lucide/vue";

/** 自定义指令：元素插入时自动聚焦并全选文本（用于重命名输入框） */
const vFocus = {
  mounted(el: HTMLInputElement) {
    el.focus();
    el.select();
  },
};

const props = defineProps<{
  doc: Document
  active: boolean
  /** 树内缩进（像素），用于在文件夹内对齐层级。默认 0（根目录）。 */
  indent?: number
  /** 是否处于多选模式 */
  selectMode?: boolean
  /** 当前是否被选中（多选模式下） */
  selected?: boolean
  /** 当前是否处于行内重命名态 */
  renaming?: boolean
}>();

const emit = defineEmits<{
  contextmenu: [doc: Document, event: MouseEvent]
  /** 拖拽开始：携带文档 id */
  dragstart: [docId: string]
  /** 多选模式下点击：切换选中态 */
  toggle: [docId: string]
  /** 提交重命名（Enter / blur）。newName 为不含后缀的主名 */
  commitRename: [docId: string, newName: string]
  /** 取消重命名（Esc） */
  cancelRename: []
}>();

// 重命名输入框的当前值（主名，不含后缀）
const renameInput = ref("");

/**
 * 进入重命名态时预填当前文件名的主名（去掉 .html/.htm 等后缀）。
 * 否则会残留上一次输入，或显示完整文件名不便编辑。
 */
watch(
  () => props.renaming,
  (r) => {
    if (!r) return;
    const name = props.doc.fileName;
    const dot = name.lastIndexOf(".");
    renameInput.value = dot > 0 ? name.slice(0, dot) : name;
  },
);

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

/** 计算当前文件名的主名（不含后缀） */
function currentStem(): string {
  const name = props.doc.fileName;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** 重命名输入框：Enter 提交 / Esc 取消 */
function onRenameKeydown(e: KeyboardEvent) {
  if (e.key === "Enter") {
    e.preventDefault();
    const name = renameInput.value.trim();
    if (name && name !== currentStem()) {
      emit("commitRename", props.doc.id, name);
    } else {
      emit("cancelRename");
    }
  } else if (e.key === "Escape") {
    e.preventDefault();
    emit("cancelRename");
  }
}

/** 重命名输入框失焦：提交 */
function onRenameBlur() {
  const name = renameInput.value.trim();
  if (name && name !== currentStem()) {
    emit("commitRename", props.doc.id, name);
  } else {
    emit("cancelRename");
  }
}
</script>

<template>
  <div
    class="list-item"
    :class="{ active: !selectMode && active, selected: selectMode && selected, 'is-nested': indent && indent > 0 }"
    :style="indent ? { paddingLeft: 8 + indent * 8 + 'px' } : undefined"
    :draggable="!selectMode && !renaming"
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
      <!-- 重命名态：行内输入框；否则显示文件名 -->
      <input
        v-if="renaming"
        v-focus
        v-model="renameInput"
        class="rename-input"
        type="text"
        @click.stop
        @dblclick.stop
        @contextmenu.stop
        @keydown="onRenameKeydown"
        @blur="onRenameBlur"
      />
      <div v-else class="title">{{ doc.fileName }}</div>
    </div>
  </div>
</template>

<style scoped>
.list-item {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  padding: 5px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.1s;
  position: relative;
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

.list-item .title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-dim);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 文件夹内的文档：文字更淡，与根级文档形成层级区分 */
.list-item.is-nested .title {
  color: var(--text-faint);
}

/* 行内重命名输入框（与 FolderTree 风格保持一致） */
.rename-input {
  width: 100%;
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
</style>
