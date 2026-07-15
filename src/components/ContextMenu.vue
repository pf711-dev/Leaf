<script setup lang="ts">
defineProps<{
  visible: boolean
  x: number
  y: number
}>()

const emit = defineEmits<{
  copyPath: []
  delete: []
  close: []
}>()
</script>

<template>
  <transition name="fade">
    <div v-if="visible" class="context-overlay" @click="emit('close')" @contextmenu.prevent="emit('close')">
      <transition name="pop" appear>
        <div
          v-if="visible"
          class="context-menu"
          :style="{ left: x + 'px', top: y + 'px' }"
          @click.stop
        >
          <button class="menu-item" @click="emit('copyPath')">复制路径</button>
          <button class="menu-item menu-item-danger" @click="emit('delete')">删除</button>
        </div>
      </transition>
    </div>
  </transition>
</template>

<style scoped>
/* 透明遮罩：捕获菜单外的点击以关闭菜单 */
.context-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
}

.context-menu {
  position: fixed;
  min-width: 140px;
  padding: 4px;
  background: var(--bg);
  border-radius: 8px;
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
}

.menu-item {
  display: block;
  width: 100%;
  padding: 6px 10px;
  border: none;
  background: transparent;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 400;
  color: var(--text-dim);
  text-align: left;
  cursor: pointer;
  transition: background 0.1s;
}
.menu-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.menu-item-danger:hover {
  color: var(--danger);
  background: var(--bg-hover);
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
