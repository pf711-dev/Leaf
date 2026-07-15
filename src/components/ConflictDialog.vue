<script setup lang="ts">
import type { ConflictInfo } from "../types/document";

defineProps<{
  visible: boolean
  conflicts: ConflictInfo[]
}>()

const emit = defineEmits<{
  skip: []
  overwrite: []
  cancel: []
}>()
</script>

<template>
  <transition name="fade">
    <div v-if="visible" class="overlay" @click.self="emit('cancel')">
      <transition name="pop" appear>
        <div v-if="visible" class="dialog">
          <div class="dialog-title">文档已存在</div>
          <div class="dialog-message">
            以下 {{ conflicts.length }} 篇文档已存在于库中，你可以选择跳过它们，或覆盖原有文档：
          </div>
          <ul class="conflict-list">
            <li v-for="(c, i) in conflicts" :key="i" class="conflict-item">
              <span class="conflict-name" :title="c.fileName">{{ c.fileName }}</span>
              <span class="conflict-existing" :title="c.existingTitle">
                库中：{{ c.existingTitle || '（无标题）' }}
              </span>
            </li>
          </ul>
          <div class="dialog-actions">
            <button class="btn btn-cancel" @click="emit('cancel')">取消</button>
            <button class="btn btn-cancel" @click="emit('skip')">全部跳过</button>
            <button class="btn btn-confirm" @click="emit('overwrite')">全部覆盖</button>
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
  width: 420px;
  max-width: 90%;
}

.dialog-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text);
  margin-bottom: 8px;
}

.dialog-message {
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.5;
  margin-bottom: 14px;
}

.conflict-list {
  list-style: none;
  margin: 0 0 18px;
  padding: 0;
  max-height: 200px;
  overflow-y: auto;
  border-top: 1px solid var(--border);
}

.conflict-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 4px;
  border-bottom: 1px solid var(--border);
}
.conflict-item:last-child {
  border-bottom: none;
}

.conflict-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.conflict-existing {
  font-size: 12px;
  color: var(--text-faint);
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
  background: #095d82;
}

/* 过渡动画（与 ConfirmDialog 一致） */
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
