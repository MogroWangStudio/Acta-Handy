<script setup lang="ts">
// 悬浮窗：三种形态——
//   bar   自由浮动信息条（原有形态）
//   pill  吸附屏幕边缘的小药丸，鼠标移过后展开面板
//   panel 快速编辑面板（待办勾选 + 笔记编辑，改动自动保存）
// 另有隐匿模式：超过设定延迟没有交互就淡出，后端监控光标、靠近时唤醒。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { initStore, store } from "../lib/store";
import { bucketTodos } from "../lib/view";
import { dueLabel, isTodoDone } from "../lib/format";
import { t } from "../lib/i18n";
import { onHudWake, setHudCursorWatch, setHudMode } from "../lib/api";
import HudPanel from "../components/HudPanel.vue";

onMounted(async () => {
  await initStore();
  shape.value = store.settings.hud.snapToEdge ? "pill" : "bar";
  void onHudWake(wake);
  armStealth();
});

const cfg = computed(() => store.settings.hud);
const shape = ref<"bar" | "pill" | "panel">("bar");
const edge = ref<"left" | "right">("right");
const hidden = ref(false);
const slowFade = ref(false);
const pointerInside = ref(false);

watch(
  () => cfg.value.snapToEdge,
  (snap) => {
    if (shape.value !== "panel") shape.value = snap ? "pill" : "bar";
    armStealth();
  },
);
watch(() => cfg.value.stealth, () => armStealth());

// --- hover 展开 / 收起 ------------------------------------------------------

let expandTimer: ReturnType<typeof setTimeout> | null = null;
let collapseTimer: ReturnType<typeof setTimeout> | null = null;

function editingInput(): boolean {
  const el = document.activeElement;
  return Boolean(el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA"));
}

function onEnter(): void {
  pointerInside.value = true;
  wake();
  if (shape.value !== "pill") return;
  if (collapseTimer) clearTimeout(collapseTimer);
  expandTimer = setTimeout(() => void expand(), 120);
}

function onLeave(): void {
  pointerInside.value = false;
  if (expandTimer) clearTimeout(expandTimer);
  if (shape.value !== "panel") {
    armStealth();
    return;
  }
  scheduleCollapse();
}

/** 输入框失焦时若指针早已离开，把面板收回去。 */
function onFocusOut(): void {
  if (shape.value !== "panel") return;
  setTimeout(() => {
    if (!pointerInside.value && !editingInput()) scheduleCollapse();
  }, 60);
}

function scheduleCollapse(): void {
  if (collapseTimer) clearTimeout(collapseTimer);
  collapseTimer = setTimeout(() => {
    if (!pointerInside.value && !editingInput()) void collapse();
  }, 200);
}

async function expand(): Promise<void> {
  if (shape.value === "panel") return;
  const e = await setHudMode("panel");
  if (e) edge.value = e;
  shape.value = "panel";
  armStealth();
}

async function collapse(): Promise<void> {
  if (shape.value !== "panel") return;
  await setHudMode("pill");
  shape.value = "pill";
  armStealth();
}

// --- 隐匿模式 ----------------------------------------------------------------

let stealthTimer: ReturnType<typeof setTimeout> | null = null;
let watchTimer: ReturnType<typeof setTimeout> | null = null;

function armStealth(): void {
  if (stealthTimer) clearTimeout(stealthTimer);
  if (watchTimer) clearTimeout(watchTimer);
  if (hidden.value) wake();
  if (!cfg.value.stealth || pointerInside.value || editingInput() || shape.value === "panel") return;
  stealthTimer = setTimeout(() => void fadeOut(), cfg.value.stealthDelaySecs * 1000);
}

async function fadeOut(): Promise<void> {
  slowFade.value = true;
  hidden.value = true;
  // 等淡出动画结束后再把窗口设为穿透，让桌面继续可点。
  watchTimer = setTimeout(() => {
    if (hidden.value) void setHudCursorWatch(true);
  }, 1000);
}

function wake(): void {
  if (!hidden.value) return;
  void setHudCursorWatch(false);
  slowFade.value = false;
  hidden.value = false;
  armStealth();
}

onBeforeUnmount(() => {
  void setHudCursorWatch(false);
  if (stealthTimer) clearTimeout(stealthTimer);
  if (watchTimer) clearTimeout(watchTimer);
  if (expandTimer) clearTimeout(expandTimer);
  if (collapseTimer) clearTimeout(collapseTimer);
});

// --- 展示数据 -----------------------------------------------------------------

const buckets = computed(() => bucketTodos(store.data?.todos ?? []));
const total = computed(() => buckets.value.totalToday);
const pct = computed(() => (total.value ? buckets.value.doneToday / total.value : 0));

const RADIUS = 13;
const CIRCUM = 2 * Math.PI * RADIUS;

const next = computed(() => buckets.value.current[0] ?? null);
const nextTitle = computed(() => {
  if (next.value) return next.value.title || t("untitledTodo");
  return total.value > 0 ? t("hudAllDone") : t("hudNothing");
});
const nextDue = computed(() => {
  if (!next.value) return "";
  const label = dueLabel(next.value.startAt, next.value.dueAt);
  return next.value && !isTodoDone(next.value) && label ? `${t("hudNext")} · ${label}` : t("hudNext");
});
const overdue = computed(() => {
  if (!next.value) return false;
  const iso = next.value.dueAt || next.value.startAt;
  return Boolean(iso) && new Date(iso).getTime() < Date.now();
});
</script>

<template>
  <div
    class="hud-root"
    :class="[`edge-${edge}`]"
    @pointerenter="onEnter"
    @pointerleave="onLeave"
    @pointerdown.capture="armStealth"
    @focusin="armStealth"
    @focusout="onFocusOut"
  >
    <!-- 吸附药丸 -->
    <div v-if="shape === 'pill'" class="hud-card pill" :style="{ opacity: hidden ? 0 : cfg.opacity }" :class="{ slow: slowFade }" data-tauri-drag-region>
      <svg class="hud-ring" viewBox="0 0 34 34" aria-hidden="true">
        <circle class="ring-track" cx="17" cy="17" :r="RADIUS" />
        <circle class="ring-fill" cx="17" cy="17" :r="RADIUS" :stroke-dasharray="`${pct * CIRCUM} ${CIRCUM}`" />
        <text x="17" y="20.5" text-anchor="middle">{{ t("doneCount")(buckets.doneToday, total) }}</text>
      </svg>
    </div>

    <!-- 自由信息条 -->
    <div v-else-if="shape === 'bar'" class="hud-card bar" :style="{ opacity: hidden ? 0 : cfg.opacity }" :class="{ slow: slowFade }">
      <div class="hud-bar-inner" data-tauri-drag-region>
        <svg class="hud-ring" viewBox="0 0 34 34" aria-hidden="true">
          <circle class="ring-track" cx="17" cy="17" :r="RADIUS" />
          <circle class="ring-fill" cx="17" cy="17" :r="RADIUS" :stroke-dasharray="`${pct * CIRCUM} ${CIRCUM}`" />
          <text x="17" y="20.5" text-anchor="middle">{{ t("doneCount")(buckets.doneToday, total) }}</text>
        </svg>
        <div class="hud-copy" data-tauri-drag-region>
          <p class="hud-title" :class="{ done: !next && total > 0, dim: total === 0 }">{{ nextTitle }}</p>
          <p v-if="next" class="hud-sub" :class="{ overdue }">{{ nextDue }}</p>
        </div>
      </div>
    </div>

    <!-- 快速编辑面板 -->
    <div v-else class="hud-card panel-card" :style="{ opacity: cfg.opacity }">
      <HudPanel class="panel-slide" />
    </div>
  </div>
</template>

<style scoped>
.hud-root {
  position: fixed;
  inset: 0;
  display: flex;
}

.hud-card {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  border: 1px solid var(--line);
  background: var(--paper);
  transition: opacity .3s ease, transform .1s ease-out;
}
.hud-card.slow { transition: opacity .95s ease; }
.hud-card:active { transform: scale(.985); transition-duration: .06s; }

/* 药丸：贴边小胶囊，整块可拖动 */
.hud-card.pill {
  border-radius: 24px;
  justify-content: center;
  cursor: default;
}

/* 信息条 */
.hud-card.bar { border-radius: 16px; }
.hud-bar-inner {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 15px;
}

.hud-ring { width: 34px; height: 34px; flex: 0 0 auto; }
.ring-track, .ring-fill {
  fill: none;
  stroke-width: 3.4;
  stroke-linecap: round;
  transform: rotate(-90deg);
  transform-origin: 50% 50%;
}
.ring-track { stroke: color-mix(in srgb, var(--muted) 22%, transparent); }
.ring-fill { stroke: var(--sage); transition: stroke-dasharray .7s var(--ease-out); }
.hud-ring text { fill: var(--ink); font-size: 8px; font-weight: 700; letter-spacing: .02em; }

.hud-copy { min-width: 0; }
.hud-title {
  margin: 0;
  color: var(--ink);
  font-size: 12px;
  font-weight: 650;
  line-height: 1.3;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.hud-title.done { color: var(--sage); }
.hud-title.dim { color: var(--muted); font-weight: 600; }
.hud-sub {
  margin: 2px 0 0;
  color: var(--faint);
  font-size: 9px;
  letter-spacing: .04em;
  text-transform: uppercase;
  font-weight: 700;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.hud-sub.overdue { color: var(--priority-high-ink); }

/* 面板 */
.hud-card.panel-card {
  align-items: stretch;
  border-radius: 16px;
  overflow: hidden;
}
.panel-slide {
  flex: 1;
  min-width: 0;
  animation: panelIn .26s var(--ease-out) both;
}
.hud-root.edge-right .panel-slide { transform-origin: right center; }
.hud-root.edge-left .panel-slide { transform-origin: left center; }

@keyframes panelIn {
  from { opacity: 0; transform: translateX(var(--panel-from, 14px)); }
  to { opacity: 1; transform: translateX(0); }
}
.hud-root.edge-left { --panel-from: -14px; }
.hud-root.edge-right { --panel-from: 14px; }

@media (prefers-reduced-motion: reduce) {
  .panel-slide { animation: none; }
}
</style>
