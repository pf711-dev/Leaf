<script setup lang="ts">
import { ref, watch } from "vue";
import type { TocItem } from "../utils/html";

const props = defineProps<{ items: TocItem[]; activeId: string }>();
const emit = defineEmits<{ navigate: [id: string] }>();

/** 面板展开/收起状态 */
const expanded = ref(false);

// 切换文档时默认收起
watch(
  () => props.items,
  () => {
    expanded.value = false;
  },
);
</script>

<template>
  <div v-if="items.length" class="toc-layer">
    <!-- ============ 展开态：右侧目录面板 ============ -->
    <transition name="slide">
      <div v-if="expanded" class="toc-panel">
        <div class="toc-head">
          <span class="toc-head-title">目录</span>
          <button class="toc-close" title="收起目录" @click="expanded = false">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M9 2L4 7l5 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </button>
        </div>
        <div class="toc-list">
          <a
            v-for="(item, i) in items"
            :key="item.id"
            :class="{ active: item.id === activeId }"
            :title="item.text"
            @click="emit('navigate', item.id)"
          >
            <span class="toc-idx">{{ i + 1 }}</span>
            <span class="toc-text">{{ item.text }}</span>
          </a>
        </div>
      </div>
    </transition>

    <!-- ============ 收起态：独立浮动按钮 ============ -->
    <transition name="fab">
      <button
        v-if="!expanded"
        class="toc-fab"
        title="展开目录"
        @click="expanded = true"
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
          <path d="M2 3.5h12M2 8h12M2 12.5h8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        <span class="fab-label">目录</span>
      </button>
    </transition>
  </div>
</template>

<style scoped>
.toc-layer {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 10;
  pointer-events: none;
}
.toc-panel,
.toc-fab {
  pointer-events: auto;
}

/* ========== 展开态面板 ========== */
.toc-panel {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 224px;
  background: var(--bg);
  border-left: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
}

.toc-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 6px 0 14px;
  height: 44px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.toc-head-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.toc-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: var(--text-faint);
  cursor: pointer;
  transition: background 0.1s;
}
.toc-close:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.toc-list {
  flex: 1;
  overflow-y: auto;
  padding: 6px;
}
.toc-list a {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 5px 8px;
  font-size: 13px;
  line-height: 1.35;
  color: var(--text-dim);
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.1s;
}
.toc-list a:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.toc-list a.active {
  color: var(--text);
  background: var(--bg-active);
  font-weight: 500;
}
.toc-idx {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-faint);
  min-width: 16px;
  text-align: right;
}
.toc-list a.active .toc-idx {
  color: var(--text-dim);
}
.toc-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ========== 收起态浮动按钮（Notion 阴影风） ========== */
.toc-fab {
  position: absolute;
  right: 16px;
  top: 16px;
  display: flex;
  align-items: center;
  gap: 5px;
  height: 32px;
  padding: 0 14px 0 10px;
  border-radius: 16px;
  border: 1px solid var(--border);
  background: var(--bg);
  color: var(--text-dim);
  font-size: 13px;
  white-space: nowrap;
  cursor: pointer;
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
  transition: background 0.12s;
}
.toc-fab:hover {
  background: var(--bg-hover);
}
.fab-label {
  letter-spacing: 0.02em;
}

/* ========== 过渡动画 ========== */
.slide-enter-active,
.slide-leave-active {
  transition: transform 0.25s ease, opacity 0.25s ease;
}
.slide-enter-from,
.slide-leave-to {
  transform: translateX(100%);
  opacity: 0;
}

.fab-enter-active,
.fab-leave-active {
  transition: transform 0.2s ease, opacity 0.2s ease;
}
.fab-enter-from,
.fab-leave-to {
  transform: scale(0.85);
  opacity: 0;
}
</style>
