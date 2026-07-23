<script setup lang="ts">
import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen } from "@lucide/vue";

const emit = defineEmits<{
  (e: "selected", rootPath: string): void;
}>();

async function pickFolder() {
  const dir = await open({ directory: true, recursive: false });
  if (dir && typeof dir === "string") {
    emit("selected", dir);
  }
}

/** 拖拽文件夹到窗口 */
function onDrop(e: DragEvent) {
  e.preventDefault();
  // 浏览器拖拽在 Tauri 中无法获取绝对路径，引导用户使用按钮
}
function onDragOver(e: DragEvent) {
  e.preventDefault();
}
</script>

<template>
  <div
    class="vault-setup"
    @drop="onDrop"
    @dragover="onDragOver"
  >
    <div class="vault-setup-card">
      <div class="vault-setup-icon">
        <FolderOpen :size="48" :stroke-width="1.3" />
      </div>
      <h1 class="vault-setup-title">欢迎使用 Leaf</h1>
      <p class="vault-setup-desc">
        选择一个本地文件夹作为仓库，<br />
        开始阅读、演示和管理 HTML 文档。
      </p>
      <button class="vault-setup-btn" @click="pickFolder">
        选择本地文件夹
      </button>
    </div>
  </div>
</template>

<style scoped>
.vault-setup {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  background: var(--bg-sidebar);
  -webkit-app-region: drag;
}

.vault-setup-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  max-width: 380px;
  text-align: center;
  padding: 40px 32px;
  background: var(--bg);
  border-radius: 8px;
  box-shadow:
    rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px,
    rgba(15, 15, 15, 0.2) 0px 9px 24px;
  -webkit-app-region: no-drag;
}

.vault-setup-icon {
  color: var(--text-faint);
  opacity: 0.7;
  margin-bottom: 4px;
}

.vault-setup-title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--text);
}

.vault-setup-desc {
  margin: 0 0 8px;
  font-size: 13px;
  color: var(--text-dim);
  line-height: 1.6;
}

.vault-setup-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 18px;
  border-radius: 4px;
  font-size: 14px;
  font-weight: 500;
  font-family: inherit;
  color: #fff;
  background: var(--accent-blue);
  border: 1px solid var(--accent-blue);
  cursor: pointer;
  transition: background 0.1s;
}
.vault-setup-btn:hover {
  background: var(--accent-blue-hover);
  border-color: var(--accent-blue-hover);
}
</style>
