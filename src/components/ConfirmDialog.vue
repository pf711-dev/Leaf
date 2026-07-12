<script setup lang="ts">
defineProps<{
  visible: boolean
  title: string
  message: string
  confirmText?: string
  cancelText?: string
  danger?: boolean
}>()

const emit = defineEmits<{
  confirm: []
  cancel: []
}>()
</script>

<template>
  <transition name="fade">
    <div v-if="visible" class="overlay" @click.self="emit('cancel')">
      <transition name="pop" appear>
        <div v-if="visible" class="dialog">
          <div class="dialog-title">{{ title }}</div>
          <div class="dialog-message">{{ message }}</div>
          <div class="dialog-actions">
            <button class="btn btn-cancel" @click="emit('cancel')">
              {{ cancelText || '取消' }}
            </button>
            <button
              class="btn"
              :class="danger ? 'btn-danger-solid' : 'btn-confirm'"
              @click="emit('confirm')"
            >
              {{ confirmText || '确定' }}
            </button>
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
  margin-bottom: 8px;
}

.dialog-message {
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.5;
  margin-bottom: 20px;
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

.btn-danger-solid {
  background: var(--danger);
  color: #fff;
}
.btn-danger-solid:hover {
  background: #b04a44;
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
