<script setup lang="ts">
/** 通用右键菜单：items 驱动，可同时服务文档、文件夹等不同场景。
 *  调用方传入 items 配置，通过 select(key) 事件获知用户点击了哪一项。 */
export interface MenuItem {
  /** 事件 key，回传给调用方 */
  key: string
  /** 显示文案 */
  label: string
  /** 危险项（删除等）：hover 时变红 */
  danger?: boolean
}

const props = defineProps<{
  visible: boolean
  x: number
  y: number
  items: MenuItem[]
  /** 菜单底部非点击信息（如文件大小、导入时间，Notion 风格） */
  footer?: string
}>();

const emit = defineEmits<{
  select: [key: string]
  close: []
}>();

function onClick(key: string) {
  emit("select", key);
  emit("close");
}
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
          <button
            v-for="item in props.items"
            :key="item.key"
            class="menu-item"
            :class="{ 'menu-item-danger': item.danger }"
            @click="onClick(item.key)"
          >
            {{ item.label }}
          </button>
          <div v-if="footer" class="menu-footer">{{ footer }}</div>
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

/* 菜单底部信息区（Notion 风格：分隔线 + 非点击信息） */
.menu-footer {
  margin-top: 4px;
  padding: 6px 10px 4px;
  border-top: 1px solid var(--border);
  font-size: 11px;
  color: var(--text-faint);
  user-select: none;
  cursor: default;
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
