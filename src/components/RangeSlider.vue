<script setup lang="ts">
// 自绘滑块：细轨 + 填充段 + 圆形拇指，全部取主题令牌，替代各平台样式
// 不一的原生 accent-color 方案。填充段由 --pct 驱动，拖动 1:1 跟手；
// 按下即时放大拇指并泛起主题色光环（反馈在按下的一瞬，不在松手后）。
import { computed } from "vue";

const props = withDefaults(
  defineProps<{
    modelValue: number;
    min: number;
    max: number;
    step?: number;
  }>(),
  { step: 1 },
);
const emit = defineEmits<{ (e: "update:modelValue", value: number): void }>();

const pct = computed(() => {
  const span = props.max - props.min;
  if (span <= 0) return 0;
  return Math.min(100, Math.max(0, ((props.modelValue - props.min) / span) * 100));
});

function onInput(event: Event): void {
  emit("update:modelValue", Number((event.target as HTMLInputElement).value));
}
</script>

<template>
  <input
    class="range-slider"
    type="range"
    :min="min"
    :max="max"
    :step="step"
    :value="modelValue"
    :style="{ '--pct': `${pct}%` }"
    @input="onInput"
  />
</template>

<style scoped>
.range-slider {
  --track: 4px;
  --thumb: 14px;
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 22px; /* 命中区高于可视轨道，细轨也不难点中 */
  margin: 0;
  background: transparent;
  cursor: pointer;
}
.range-slider:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--sage) 35%, transparent);
  outline-offset: 2px;
  border-radius: 999px;
}

.range-slider::-webkit-slider-runnable-track {
  height: var(--track);
  border-radius: 999px;
  /* 填充段到当前值为止：与拇指边缘相接，一眼读出取值。 */
  background: linear-gradient(
    to right,
    var(--sage) 0 var(--pct),
    color-mix(in srgb, var(--muted) 26%, transparent) var(--pct) 100%
  );
}

.range-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: var(--thumb);
  height: var(--thumb);
  margin-top: calc((var(--track) - var(--thumb)) / 2);
  border-radius: 50%;
  background: var(--white);
  border: 0.5px solid color-mix(in srgb, var(--ink) 14%, transparent);
  box-shadow: 0 1px 2px rgba(24, 28, 23, .26);
  transition: transform .16s var(--ease-out), box-shadow .16s ease;
}
.range-slider:hover::-webkit-slider-thumb { transform: scale(1.12); }
.range-slider:active::-webkit-slider-thumb {
  transform: scale(1.24);
  box-shadow:
    0 1px 2px rgba(24, 28, 23, .26),
    0 0 0 6px color-mix(in srgb, var(--sage) 18%, transparent);
}

@media (prefers-reduced-motion: reduce) {
  .range-slider::-webkit-slider-thumb { transition: none; }
}
</style>
