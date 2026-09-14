<script setup lang="ts">
// 自绘下拉：设置窗口与悬浮窗统一使用，替代各平台样式不一的原生 select。
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

export interface SelectOption {
  value: string | number;
  label: string;
}

const props = defineProps<{ modelValue: string | number; options: SelectOption[] }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string | number): void }>();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
const dropUp = ref(false);

const current = computed(() => props.options.find((o) => o.value === props.modelValue));

function choose(option: SelectOption): void {
  emit("update:modelValue", option.value);
  open.value = false;
}

function toggle(): void {
  if (open.value) {
    open.value = false;
    return;
  }
  // 靠近视口底部时向上展开，面板始终朝内容方向打开。
  const rect = root.value?.getBoundingClientRect();
  dropUp.value = rect ? rect.bottom + Math.min(props.options.length * 34 + 12, 238) > window.innerHeight - 12 : false;
  open.value = true;
}

function onPointerDown(event: PointerEvent): void {
  if (open.value && root.value && !root.value.contains(event.target as Node)) {
    open.value = false;
  }
}
function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape" && open.value) {
    open.value = false;
    event.stopPropagation();
  }
}
onMounted(() => {
  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("keydown", onKeydown, true);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onPointerDown, true);
  document.removeEventListener("keydown", onKeydown, true);
});
</script>

<template>
  <div ref="root" class="select-menu">
    <button
      type="button"
      class="select-trigger"
      :aria-expanded="open"
      :aria-haspopup="'listbox'"
      @click="toggle"
    >
      <span class="select-label">{{ current?.label ?? "" }}</span>
      <svg class="select-chevron" :class="{ open }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M6 9.5l6 6 6-6" />
      </svg>
    </button>
    <Transition name="select">
      <ul v-if="open" class="select-list" :class="{ up: dropUp }" role="listbox">
        <li v-for="option in options" :key="option.value">
          <button
            type="button"
            role="option"
            :aria-selected="option.value === modelValue"
            :class="{ active: option.value === modelValue }"
            @click="choose(option)"
          >
            <span>{{ option.label }}</span>
            <svg v-if="option.value === modelValue" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M5 12.5l4.5 4.5L19 7.5" />
            </svg>
          </button>
        </li>
      </ul>
    </Transition>
  </div>
</template>

<style scoped>
.select-menu { position: relative; flex: 0 0 auto; }

.select-trigger {
  min-width: 132px;
  height: 33px;
  padding: 0 10px;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--white);
  color: var(--ink);
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  cursor: pointer;
  transition: border-color .15s ease, background .15s ease;
}
.select-trigger:hover { background: var(--panel); }
.select-trigger[aria-expanded="true"] { border-color: color-mix(in srgb, var(--sage) 45%, var(--line)); }
.select-label { flex: 1; text-align: left; white-space: nowrap; }
.select-chevron { width: 13px; height: 13px; color: var(--faint); flex: 0 0 auto; transition: transform .18s var(--ease-out); }
.select-chevron.open { transform: rotate(180deg); }

.select-list {
  position: absolute;
  top: calc(100% + 5px);
  left: 0;
  z-index: 60;
  min-width: 100%;
  margin: 0;
  padding: 4px;
  list-style: none;
  border: 1px solid var(--line);
  border-radius: 11px;
  background: var(--white);
  box-shadow: var(--shadow-pop);
  transform-origin: top;
}
.select-list.up { top: auto; bottom: calc(100% + 5px); transform-origin: bottom; }

.select-list button {
  width: 100%;
  height: 30px;
  padding: 0 9px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--ink);
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  transition: background .12s ease;
}
.select-list button:hover { background: var(--sage-2); }
.select-list button.active { color: var(--sage); font-weight: 650; }
.select-list button.active:hover { color: var(--sage); }
.select-list button svg { width: 12px; height: 12px; margin-left: auto; flex: 0 0 auto; }

.select-enter-active { transition: opacity .16s ease-out, transform .16s var(--ease-out); }
.select-leave-active { transition: opacity .1s ease-in, transform .1s ease-in; }
.select-enter-from, .select-leave-to { opacity: 0; transform: scaleY(.92); }
</style>
