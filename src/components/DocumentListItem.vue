<script setup lang="ts">
import type { Document } from "../types/document";
import { formatDate, formatSize } from "../utils/format";

defineProps<{ doc: Document; active: boolean }>();
</script>

<template>
  <div class="list-item" :class="{ active }">
    <div class="title">{{ doc.title }}</div>
    <div class="meta">
      <span class="meta-name">{{ doc.fileName }}</span>
      <span class="meta-sep">·</span>
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
