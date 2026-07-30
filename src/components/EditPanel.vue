<script setup lang="ts">
/**
 * 编辑属性面板（EditPanel）。
 *
 * 借鉴 Keynote「格式」侧边栏的三分类设计，在编辑模式下始终显示于右侧：
 * - 文本：字体、字号、加粗/倾斜/下划线/删除线、文字颜色
 * - 样式：背景颜色、对齐方式
 * - 排列：宽高、复制/删除/重置元素
 *
 * 设计规范 A/B/C 层：所有样式严格遵循 `0 项目文档/01 设计规范.md`。
 */
import { computed, watch, ref, onMounted, onUnmounted } from "vue";
import {
  Copy,
  Trash2,
  RotateCcw,
  ChevronUp,
  ChevronDown,
  Bold,
  Italic,
  Underline,
  Strikethrough,
  Check,
} from "@lucide/vue";

// ─── 字体列表 ───

const FONT_FAMILIES = [
  { value: "", label: "系统默认" },
  { value: "PingFang SC, sans-serif", label: "苹方" },
  { value: "Microsoft YaHei, sans-serif", label: "微软雅黑" },
  { value: '"Noto Sans SC", sans-serif', label: "思源黑体" },
  { value: "SimSun, serif", label: "宋体" },
  { value: "SimHei, sans-serif", label: "黑体" },
  { value: "KaiTi, serif", label: "楷体" },
  { value: "Inter, sans-serif", label: "Inter" },
  { value: "Georgia, serif", label: "Georgia" },
  { value: "Arial, sans-serif", label: "Arial" },
  { value: "Helvetica, sans-serif", label: "Helvetica" },
  { value: "Menlo, Monaco, monospace", label: "等宽" },
];

// ─── 颜色预设 ───

const TEXT_COLORS = [
  { value: "#212121", label: "黑色" },
  { value: "#757575", label: "灰色" },
  { value: "#E03440", label: "红色" },
  { value: "#FF7A45", label: "橙色" },
  { value: "#FFC400", label: "黄色" },
  { value: "#00B42A", label: "绿色" },
  { value: "#006AFA", label: "蓝色" },
  { value: "#722ED1", label: "紫色" },
];

const BG_COLORS = [
  { value: "transparent", label: "无填充" },
  { value: "#F2F2F2", label: "浅灰" },
  { value: "#FFECE8", label: "浅红" },
  { value: "#FFF3E8", label: "浅橙" },
  { value: "#FFF7E8", label: "浅黄" },
  { value: "#E8FFEA", label: "浅绿" },
  { value: "#E8F3FF", label: "浅蓝" },
  { value: "#F5E8FF", label: "浅紫" },
  { value: "#E5E5E5", label: "中灰" },
  { value: "#BFBFBF", label: "深灰" },
  { value: "#FFCCC7", label: "红" },
  { value: "#FFD591", label: "橙" },
  { value: "#FFFB8F", label: "黄" },
  { value: "#B7EB8F", label: "绿" },
  { value: "#ADC6FF", label: "蓝" },
  { value: "#D3ADF7", label: "紫" },
];

// ─── Props / Emits ───

const props = defineProps<{
  selected: { tag: string; kind: string } | null;
  computed: Record<string, string>;
  isTextEditing: boolean;
}>();

const emit = defineEmits<{
  "update-style": [prop: string, value: string];
  delete: [];
  duplicate: [];
  reset: [];
  format: [command: string, value?: string];
  "bump-font-size": [dir: number];
}>();

// ─── 本地输入值 ───

const fontInput = ref("");
const widthInput = ref("");
const heightInput = ref("");
const fontFamilyValue = ref("");
const fontDropdownOpen = ref(false);
const fontDropdownRef = ref<HTMLElement | null>(null);

watch(
  () => props.selected,
  () => {
    fontInput.value = stripUnit(props.computed.fontSize);
    widthInput.value = stripUnit(props.computed.width);
    heightInput.value = stripUnit(props.computed.height);
    fontFamilyValue.value = props.computed.fontFamily
      ? props.computed.fontFamily.split(",")[0].replace(/"/g, "").trim()
      : "";
  },
  { immediate: true },
);

function stripUnit(v: string | undefined): string {
  if (!v) return "";
  const m = String(v).match(/^-?\d+(\.\d+)?/);
  return m ? m[0] : "";
}

function commitFontFamily(v: string) {
  fontFamilyValue.value = v;
  fontDropdownOpen.value = false;
  if (props.isTextEditing) {
    emit("format", "fontName", v);
  } else {
    emit("update-style", "font-family", v || "");
  }
}

function toggleFontDropdown() {
  fontDropdownOpen.value = !fontDropdownOpen.value;
}

function onFontDropdownMousedown(e: MouseEvent) {
  // 阻止下拉面板内部 mousedown 触发外部 blur/click-outside
  e.preventDefault();
}

// ─── 点击外部关闭下拉 ───

function onDocumentClick(e: MouseEvent) {
  if (!fontDropdownOpen.value) return;
  const target = e.target as HTMLElement;
  if (fontDropdownRef.value && !fontDropdownRef.value.contains(target)) {
    fontDropdownOpen.value = false;
  }
}

onMounted(() => {
  document.addEventListener("click", onDocumentClick, true);
});

onUnmounted(() => {
  document.removeEventListener("click", onDocumentClick, true);
});

function commitFont() {
  const v = fontInput.value.trim();
  if (v) emit("update-style", "font-size", `${v}px`);
}

function commitWidth() {
  const v = widthInput.value.trim();
  if (v) emit("update-style", "width", `${v}px`);
}
function commitHeight() {
  const v = heightInput.value.trim();
  if (v) emit("update-style", "height", `${v}px`);
}


// ─── 颜色转换 ───

function toHex(v: string): string {
  if (!v) return "#212121";
  const s = v.trim();
  if (/^#[0-9a-fA-F]{6}$/.test(s)) return s;
  if (/^#[0-9a-fA-F]{3}$/.test(s))
    return "#" + s.slice(1).split("").map((c) => c + c).join("");
  const m = s.match(/rgba?\(([^)]+)\)/i);
  if (m) {
    const parts = m[1].split(",").map((p) => parseFloat(p.trim()));
    const [r, g, b, a] = parts;
    if (a === 0) return "#ffffff";
    const hex = (n: number) =>
      Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, "0");
    return `#${hex(r)}${hex(g)}${hex(b)}`;
  }
  return "#212121";
}

const colorValue = computed(() => toHex(props.computed.color || ""));
const bgValue = computed(() => toHex(props.computed.backgroundColor || ""));
const alignValue = computed(() => props.computed.textAlign || "left");

// ─── 样式修改 ───

function setAlign(v: string) {
  emit("update-style", "text-align", v);
}
function setColor(e: Event) {
  emit("update-style", "color", (e.target as HTMLInputElement).value);
}
function setBg(e: Event) {
  emit("update-style", "background-color", (e.target as HTMLInputElement).value);
}
function setTextColor(v: string) {
  if (props.isTextEditing) {
    emit("format", "foreColor", v);
  } else {
    emit("update-style", "color", v);
  }
}
function setBgColor(v: string) {
  if (props.isTextEditing) {
    emit("format", "hiliteColor", v);
  } else {
    emit("update-style", "background-color", v);
  }
}

// ─── 文字格式快捷键 ───

function doBold() {
  emit("format", "bold");
}
function doItalic() {
  emit("format", "italic");
}
function doUnderline() {
  emit("format", "underline");
}
function doStrike() {
  emit("format", "strikeThrough");
}
function bumpUp() {
  emit("bump-font-size", 1);
}
function bumpDown() {
  emit("bump-font-size", -1);
}
</script>

<template>
  <transition name="slide">
    <aside v-if="selected" class="edit-panel">
      <!-- 面板头 -->
      <div class="panel-head">
        <span class="panel-head-title">属性</span>
        <span class="panel-head-tag">{{ selected.tag }}</span>
      </div>

      <div class="panel-body">
        <!-- ========== 文本 ========== -->
        <div class="panel-section">
          <div class="panel-section-title">文本</div>

          <!-- 字体选择 -->
          <div class="panel-row">
            <span class="panel-label">字体</span>
            <div class="panel-dropdown" ref="fontDropdownRef">
              <button
                class="panel-dropdown-trigger"
                @click="toggleFontDropdown"
              >
                <span class="panel-dropdown-label">
                  {{ FONT_FAMILIES.find(f => f.value === fontFamilyValue)?.label || '系统默认' }}
                </span>
                <ChevronDown :size="12" :stroke-width="1.8" class="panel-dropdown-chevron" />
              </button>
              <transition name="drop">
                <div v-if="fontDropdownOpen" class="panel-dropdown-menu" @mousedown="onFontDropdownMousedown">
                  <button
                    v-for="f in FONT_FAMILIES"
                    :key="f.value"
                    class="panel-dropdown-item"
                    :class="{ active: f.value === fontFamilyValue }"
                    @click="commitFontFamily(f.value)"
                  >
                    <span class="panel-dropdown-item-label">{{ f.label }}</span>
                    <Check v-if="f.value === fontFamilyValue" :size="13" :stroke-width="2" />
                  </button>
                </div>
              </transition>
            </div>
          </div>

          <!-- 字号 -->
          <div class="panel-row">
            <span class="panel-label">字号</span>
            <input
              class="panel-input"
              type="text"
              v-model="fontInput"
              placeholder="16"
              @change="commitFont"
            />
            <span class="panel-unit">px</span>
            <button
              class="panel-fmt-btn panel-fmt-btn-sm"
              title="增大字号"
              :disabled="!isTextEditing"
              @click="bumpUp"
            >
              <ChevronUp :size="12" :stroke-width="2" />
            </button>
            <button
              class="panel-fmt-btn panel-fmt-btn-sm"
              title="减小字号"
              :disabled="!isTextEditing"
              @click="bumpDown"
            >
              <ChevronDown :size="12" :stroke-width="2" />
            </button>
          </div>

          <!-- 文字格式 B/I/U/S -->
          <div class="panel-row">
            <span class="panel-label"></span>
            <div class="panel-fmt-group">
              <button
                class="panel-fmt-btn"
                title="加粗"
                :disabled="!isTextEditing"
                @click="doBold"
              >
                <Bold :size="14" :stroke-width="2" />
              </button>
              <button
                class="panel-fmt-btn"
                title="倾斜"
                :disabled="!isTextEditing"
                @click="doItalic"
              >
                <Italic :size="14" :stroke-width="2" />
              </button>
              <button
                class="panel-fmt-btn"
                title="下划线"
                :disabled="!isTextEditing"
                @click="doUnderline"
              >
                <Underline :size="14" :stroke-width="2" />
              </button>
              <button
                class="panel-fmt-btn"
                title="删除线"
                :disabled="!isTextEditing"
                @click="doStrike"
              >
                <Strikethrough :size="14" :stroke-width="2" />
              </button>
            </div>
          </div>

          <!-- 文字颜色 -->
          <div class="panel-row panel-row-wrap">
            <span class="panel-label">颜色</span>
            <div class="panel-color-grid">
              <button
                v-for="c in TEXT_COLORS"
                :key="c.value"
                class="panel-color-swatch panel-text-swatch"
                :style="{ color: c.value }"
                :title="c.label"
                @click="setTextColor(c.value)"
              >A</button>
              <input
                class="panel-color-input"
                type="color"
                :value="colorValue"
                @input="setColor"
                title="自定义颜色"
              />
            </div>
          </div>
        </div>

        <!-- ========== 样式 ========== -->
        <div class="panel-section">
          <div class="panel-section-title">样式</div>

          <!-- 背景颜色 -->
          <div class="panel-row panel-row-wrap">
            <span class="panel-label">背景</span>
            <div class="panel-color-grid">
              <button
                v-for="c in BG_COLORS"
                :key="c.value"
                class="panel-color-swatch panel-bg-swatch"
                :class="{ 'is-none': c.value === 'transparent' }"
                :style="{ background: c.value === 'transparent' ? undefined : c.value }"
                :title="c.label"
                @click="setBgColor(c.value)"
              >
                <svg
                  v-if="c.value === 'transparent'"
                  width="12"
                  height="12"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                ><line x1="5" y1="5" x2="19" y2="19" /><line x1="19" y1="5" x2="5" y2="19" /></svg>
                <span v-else></span>
              </button>
              <input
                class="panel-color-input"
                type="color"
                :value="bgValue"
                @input="setBg"
                title="自定义背景色"
              />
            </div>
          </div>

          <!-- 对齐 -->
          <div class="panel-row">
            <span class="panel-label">对齐</span>
            <div class="panel-seg">
              <button
                class="panel-seg-btn"
                :class="{ active: alignValue === 'left' }"
                @click="setAlign('left')"
              >左</button>
              <button
                class="panel-seg-btn"
                :class="{ active: alignValue === 'center' }"
                @click="setAlign('center')"
              >中</button>
              <button
                class="panel-seg-btn"
                :class="{ active: alignValue === 'right' }"
                @click="setAlign('right')"
              >右</button>
            </div>
          </div>
        </div>

        <!-- ========== 排列 ========== -->
        <div class="panel-section">
          <div class="panel-section-title">排列</div>

          <!-- 宽度 -->
          <div class="panel-row">
            <span class="panel-label">宽度</span>
            <input
              class="panel-input"
              type="text"
              v-model="widthInput"
              placeholder="auto"
              @change="commitWidth"
            />
            <span class="panel-unit">px</span>
          </div>

          <!-- 高度 -->
          <div class="panel-row">
            <span class="panel-label">高度</span>
            <input
              class="panel-input"
              type="text"
              v-model="heightInput"
              placeholder="auto"
              @change="commitHeight"
            />
            <span class="panel-unit">px</span>
          </div>

          <!-- 操作 -->
          <div class="panel-section panel-section-actions">
            <button class="panel-action" @click="emit('duplicate')">
              <Copy :size="14" :stroke-width="1.5" />
              <span>复制元素</span>
            </button>
            <button class="panel-action panel-action-danger" @click="emit('delete')">
              <Trash2 :size="14" :stroke-width="1.5" />
              <span>删除元素</span>
            </button>
            <button class="panel-action" @click="emit('reset')">
              <RotateCcw :size="14" :stroke-width="1.5" />
              <span>重置样式</span>
            </button>
          </div>
        </div>
      </div>

      <!-- 空选中时的文字编辑提示 -->
      <div v-if="!isTextEditing" class="panel-hint">
        双击文字元素开始编辑
      </div>
    </aside>
  </transition>
</template>

<style scoped>
/* ========== 面板容器：复刻 .toc-panel ========== */
.edit-panel {
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
  z-index: 10;
}

/* ========== 面板头：复刻 .toc-head ========== */
.panel-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 14px;
  height: 44px;
  border-bottom: 1px solid var(--border);
  flex-shrink: 0;
}
.panel-head-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.panel-head-tag {
  font-size: 11px;
  color: var(--text-faint);
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}

/* ========== 底部提示 ========== */
.panel-hint {
  padding: 8px 12px;
  font-size: 11px;
  color: var(--text-faint);
  border-top: 1px solid var(--border);
  flex-shrink: 0;
  text-align: center;
}

/* ========== 滚动体 ========== */
.panel-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px 4px;
}

/* ========== 分组 ========== */
.panel-section {
  padding: 2px 8px 10px;
}
.panel-section + .panel-section {
  border-top: 1px solid var(--border);
  padding-top: 10px;
}
.panel-section:first-child {
  border-top: none;
}
.panel-section-actions {
  padding-top: 6px;
}
.panel-section-title {
  font-size: 12px;
  font-weight: 600;
  color: var(--text);
  padding: 0 0 8px;
  user-select: none;
}

/* ========== 属性行 ========== */
.panel-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  min-height: 28px;
}
.panel-row-wrap {
  flex-wrap: wrap;
}
.panel-label {
  flex-shrink: 0;
  width: 36px;
  font-size: 12px;
  color: var(--text-dim);
}
.panel-input {
  flex: 1;
  min-width: 0;
  padding: 3px 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg);
  color: var(--text);
  font-size: 12px;
  font-family: inherit;
  outline: none;
  transition: border-color 0.1s, background 0.1s;
}
.panel-input:focus {
  border-color: var(--accent-blue);
  background: var(--select-bg);
}
.panel-input:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.panel-unit {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--text-faint);
  width: 16px;
  text-align: center;
}

/* 下拉字体选择（自定义，Notion 风格） */
.panel-dropdown {
  position: relative;
  flex: 1;
  min-width: 0;
}
.panel-dropdown-trigger {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 3px 6px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg);
  color: var(--text);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: border-color 0.1s, background 0.1s;
}
.panel-dropdown-trigger:hover {
  background: var(--bg-hover);
}
.panel-dropdown-label {
  flex: 1;
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.panel-dropdown-chevron {
  flex-shrink: 0;
  color: var(--text-faint);
  margin-left: 4px;
}
.panel-dropdown-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  z-index: 50;
  min-width: 100%;
  max-height: 240px;
  overflow-y: auto;
  padding: 4px;
  background: var(--bg);
  border-radius: 8px;
  box-shadow: rgba(15, 15, 15, 0.05) 0px 0px 0px 1px,
    rgba(15, 15, 15, 0.1) 0px 3px 6px, rgba(15, 15, 15, 0.2) 0px 9px 24px;
}
.panel-dropdown-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 5px 8px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  font-size: 12px;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.1s;
}
.panel-dropdown-item:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.panel-dropdown-item.active {
  color: var(--text);
  font-weight: 500;
}
.panel-dropdown-item-label {
  flex: 1;
}

/* 文字格式按钮组 */
.panel-fmt-group {
  display: flex;
  gap: 2px;
}
.panel-fmt-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 26px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  transition: background 0.1s, color 0.1s;
}
.panel-fmt-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.panel-fmt-btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.panel-fmt-btn:disabled:hover {
  background: transparent;
  color: var(--text-dim);
}
.panel-fmt-btn-sm {
  width: 22px;
  height: 22px;
}

/* 颜色色板 */
.panel-color-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 2px;
  align-items: center;
}
.panel-color-swatch {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: none;
  border-radius: 3px;
  background: transparent;
  cursor: pointer;
  transition: transform 0.1s, box-shadow 0.1s;
}
.panel-color-swatch:hover {
  transform: scale(1.15);
  box-shadow: 0 0 0 1px var(--border-strong);
}
.panel-color-swatch:disabled {
  opacity: 0.35;
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}
.panel-text-swatch {
  font-size: 12px;
  font-weight: 700;
}
.panel-bg-swatch {
  border: 1px solid var(--border);
  font-size: 0;
  color: var(--text-faint);
}
.panel-bg-swatch.is-none {
  background: var(--bg);
}
.panel-color-input {
  width: 22px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 3px;
  background: var(--bg);
  cursor: pointer;
}
.panel-color-input:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.panel-color-input::-webkit-color-swatch-wrapper {
  padding: 2px;
}
.panel-color-input::-webkit-color-swatch {
  border: none;
  border-radius: 2px;
}

/* 分段控件（对齐） */
.panel-seg {
  display: flex;
  gap: 2px;
}
.panel-seg-btn {
  padding: 2px 8px;
  border: 1px solid var(--border);
  border-radius: 4px;
  background: var(--bg);
  color: var(--text-dim);
  font-size: 12px;
  cursor: pointer;
  transition: background 0.1s, color 0.1s;
}
.panel-seg-btn:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.panel-seg-btn.active {
  background: var(--bg-active);
  color: var(--text);
  font-weight: 500;
}

/* ========== 操作项 ========== */
.panel-action {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 6px 8px;
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
.panel-action:hover {
  background: var(--bg-hover);
  color: var(--text);
}
.panel-action-danger:hover {
  color: var(--danger);
  background: var(--bg-hover);
}

/* ========== 动画 ========== */
.slide-enter-active,
.slide-leave-active {
  transition: transform 0.25s ease, opacity 0.25s ease;
}
.slide-enter-from,
.slide-leave-to {
  transform: translateX(100%);
  opacity: 0;
}

/* 下拉菜单动画 */
.drop-enter-active {
  transition: opacity 0.15s ease, transform 0.15s ease;
}
.drop-leave-active {
  transition: opacity 0.1s ease, transform 0.1s ease;
}
.drop-enter-from {
  opacity: 0;
  transform: translateY(-4px);
}
.drop-leave-to {
  opacity: 0;
  transform: translateY(-2px);
}
</style>
