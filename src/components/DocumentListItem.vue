<script setup lang="ts">
import type { Document } from "../types/document";
import { formatDate, formatSize } from "../utils/format";

const props = defineProps<{ doc: Document; active: boolean }>();

const emit = defineEmits<{
  contextmenu: [doc: Document, event: MouseEvent]
}>();

function onContextMenu(e: MouseEvent) {
  emit("contextmenu", props.doc, e);
}
</script>

<template>
  <div class="list-item" :class="{ active }" @contextmenu="onContextMenu">
    <div class="title">{{ doc.fileName }}</div>
    <div class="meta">
      <span>{{ formatSize(doc.fileSize) }}</span>
      <span class="meta-sep">·</span>
      <span>{{ formatDate(doc.importedAt) }}</span>
    </div>
  </div>
</template>

<style scoped>
.list-item {
  padding: 6px 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.1s;
}
.list-item:hover {
  background: var(--bg-hover);
}
.list-item.active {
  background: var(--bg-active);
}
.list-item.active .title {
  color: var(--text);
  font-weight: 600;
}

.title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-dim);
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  margin-top: 2px;
  font-size: 12px;
  color: var(--text-faint);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.meta-name {
  color: var(--text-dim);
}
.meta-sep {
  margin: 0 5px;
  opacity: 0.6;
}
</style>
