<script setup lang="ts">
// Handy 小人悬浮窗：三种形态——
//   free  自由站在桌面上的 Handy（眨眼呼吸，拖动时会摇晃）
//   peek  吸附屏幕边缘：扒着边缘探头，光标靠近就跳出完整身体并展开面板
//   panel 快速编辑面板（待办勾选 + 笔记编辑，改动自动保存）
// 另有隐匿模式：超过设定延迟没有交互就淡出，后端监控光标、靠近时唤醒。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initStore, store } from "../lib/store";
import { onHudWake, setHudCursorWatch, setHudMode } from "../lib/api";
import HudPanel from "../components/HudPanel.vue";
import HandyChar from "../components/HandyChar.vue";

const win = getCurrentWindow();

onMounted(async () => {
  await initStore();
  shape.value = store.settings.hud.snapToEdge ? "peek" : "free";
  // 让后端按当前形态重排一次，顺便拿到贴边方向。
  const side = await setHudMode(shape.value);
  if (side) edge.value = side;
  void onHudWake(onCursorNear);
  armStealth();
  void watchWindowMoves();
});

const cfg = computed(() => store.settings.hud);
const shape = ref<"free" | "peek" | "panel">("free");
const edge = ref<"left" | "right">("right");
const hidden = ref(false);
const slowFade = ref(false);
const pointerInside = ref(false);
const shaking = ref(false);

watch(
  () => cfg.value.snapToEdge,
  (snap) => {
    if (shape.value !== "panel") shape.value = snap ? "peek" : "free";
    armStealth();
  },
);
watch(() => cfg.value.stealth, () => armStealth());

// --- 拖动摇晃 ----------------------------------------------------------------
// 原生窗口拖动不经过网页指针事件，靠窗口 Moved 事件驱动：移动中摇晃，
// 事件停歇约 150ms 后站稳。

let moveTimer: ReturnType<typeof setTimeout> | null = null;

async function watchWindowMoves(): Promise<void> {
  try {
    await getCurrentWindow().onMoved(() => {
      shaking.value = true;
      if (moveTimer) clearTimeout(moveTimer);
      moveTimer = setTimeout(() => {
        shaking.value = false;
      }, 150);
    });
  } catch {
    // 浏览器 mock 预览没有窗口事件。
  }
}

// --- hover / 靠近 展开 -------------------------------------------------------

let expandTimer: ReturnType<typeof setTimeout> | null = null;
let collapseTimer: ReturnType<typeof setTimeout> | null = null;

function editingInput(): boolean {
  const el = document.activeElement;
  return Boolean(el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA"));
}

function onEnter(): void {
  pointerInside.value = true;
  wake();
  if (shape.value !== "peek") return;
  if (collapseTimer) clearTimeout(collapseTimer);
  expandTimer = setTimeout(() => void expand(), 90);
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
  if (shape.value !== "peek") return;
  const side = await setHudMode("panel");
  if (side) edge.value = side;
  shape.value = "panel";
  armStealth();
}

async function collapse(): Promise<void> {
  if (shape.value !== "panel") return;
  await setHudMode("peek");
  shape.value = "peek";
  armStealth();
}

// --- 隐匿模式 ----------------------------------------------------------------

let stealthTimer: ReturnType<typeof setTimeout> | null = null;
let watchTimer: ReturnType<typeof setTimeout> | null = null;

/** 光标监控：隐匿淡出后用小感应半径唤醒；探头形态常驻大感应半径，
    光标一靠近 Handy 就自己跳出来。 */
function syncCursorWatch(): void {
  if (hidden.value) void setHudCursorWatch(true, 26);
  else if (shape.value === "peek") void setHudCursorWatch(true, 64);
  else void setHudCursorWatch(false);
}

function armStealth(): void {
  if (stealthTimer) clearTimeout(stealthTimer);
  syncCursorWatch();
  if (hidden.value) return;
  if (!cfg.value.stealth || pointerInside.value || editingInput() || shape.value === "panel") return;
  stealthTimer = setTimeout(() => void fadeOut(), cfg.value.stealthDelaySecs * 1000);
}

async function fadeOut(): Promise<void> {
  slowFade.value = true;
  hidden.value = true;
  // 等淡出动画结束后让窗口穿透鼠标（桌面恢复可点），光标监控负责唤醒。
  watchTimer = setTimeout(() => {
    if (hidden.value) {
      void win.setIgnoreCursorEvents(true).catch(() => undefined);
      syncCursorWatch();
    }
  }, 1000);
}

function wake(): void {
  if (!hidden.value) return;
  void win.setIgnoreCursorEvents(false).catch(() => undefined);
  slowFade.value = false;
  hidden.value = false;
  armStealth();
}

/** 光标监控事件：隐匿中则唤醒；探头形态则跳出完整身体并展开面板。 */
function onCursorNear(): void {
  if (hidden.value) {
    wake();
    return;
  }
  if (shape.value === "peek") void expand();
}

onBeforeUnmount(() => {
  void win.setIgnoreCursorEvents(false).catch(() => undefined);
  void setHudCursorWatch(false);
  if (stealthTimer) clearTimeout(stealthTimer);
  if (watchTimer) clearTimeout(watchTimer);
  if (expandTimer) clearTimeout(expandTimer);
  if (collapseTimer) clearTimeout(collapseTimer);
  if (moveTimer) clearTimeout(moveTimer);
});
</script>

<template>
  <div
    class="hud-root"
    :class="[shape, `edge-${edge}`]"
    @pointerenter="onEnter"
    @pointerleave="onLeave"
    @pointerdown.capture="armStealth"
    @focusin="armStealth"
    @focusout="onFocusOut"
  >
    <!-- 自由站立 / 贴边探头：Handy 本体 -->
    <div
      v-if="shape !== 'panel'"
      class="handy-stage"
      :class="{ slow: slowFade }"
      :style="{ opacity: hidden ? 0 : cfg.opacity }"
      data-tauri-drag-region
    >
      <div class="handy-pos">
        <div class="handy-tilt">
          <div class="handy-lean" :class="{ 'handy-shake': shaking }">
            <HandyChar class="handy-char" :width="shape === 'peek' ? 68 : 64" />
          </div>
        </div>
      </div>
    </div>

    <!-- 快速编辑面板：Handy 站在屏幕边缘一侧，把面板拽出来 -->
    <div v-else class="panel-root">
      <div class="panel-card" :style="{ opacity: cfg.opacity }">
        <HudPanel class="panel-slide" />
      </div>
      <div class="handy-column" :class="{ 'handy-shake': shaking }" data-tauri-drag-region>
        <div class="handy-hop">
          <HandyChar :width="56" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hud-root {
  position: fixed;
  inset: 0;
  color: var(--ink);
}

/* --- Handy 本体 ------------------------------------------------------------- */

.handy-stage {
  position: absolute;
  inset: 0;
  overflow: hidden; /* 探头形态靠它把探出屏幕外的身体裁掉 */
  transition: opacity .3s ease;
}
.handy-stage.slow { transition: opacity .95s ease; }

.handy-pos { position: absolute; bottom: 5px; left: calc(50% - 32px); }
.hud-root.peek .handy-pos { bottom: 8px; width: 68px; }
.hud-root.peek.edge-right .handy-pos { left: auto; right: -18px; }
.hud-root.peek.edge-left .handy-pos { left: -18px; }

/* 探头时朝桌面一侧探身：以抓边的脚底为轴。 */
.handy-tilt { transform-origin: 50% 100%; }
.hud-root.peek.edge-right .handy-tilt { transform: rotate(-9deg); transform-origin: 100% 100%; }
.hud-root.peek.edge-left .handy-tilt { transform: rotate(9deg); transform-origin: 0 100%; }

/* 拖动中：以脚底为轴左右摇晃。选择器与 peek-pop 同优先级、放在其后，
   保证探头形态下摇晃仍能覆盖登场动画。 */
.handy-lean { transform-origin: 50% 100%; }
.hud-root .handy-lean.handy-shake { animation: handy-shake .24s ease-in-out infinite; }
@keyframes handy-shake {
  0%, 100% { transform: rotate(-4.5deg); }
  50% { transform: rotate(4.5deg); }
}

/* 收回探头形态时小弹跳登场。 */
.hud-root.peek .handy-lean { animation: peek-pop .32s var(--ease-out) both; transform-origin: 50% 100%; }
@keyframes peek-pop {
  from { opacity: 0; transform: scale(.86); }
  to { opacity: 1; transform: scale(1); }
}

/* --- 面板 -------------------------------------------------------------------- */

.panel-root {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: stretch;
  gap: 4px;
  padding: 8px 0;
}
.hud-root.edge-left .panel-root { flex-direction: row-reverse; }

.panel-card {
  flex: 1;
  min-width: 0;
  display: flex;
  border: 1px solid var(--line);
  border-radius: 16px;
  background: var(--paper);
  overflow: hidden;
}
.hud-root.edge-right .panel-card { margin-left: 8px; animation: panel-pop-right .36s var(--ease-spring) both; }
.hud-root.edge-left .panel-card { margin-right: 8px; animation: panel-pop-left .36s var(--ease-spring) both; }
@keyframes panel-pop-right {
  from { opacity: 0; transform: translateX(34px); }
  to { opacity: 1; transform: translateX(0); }
}
@keyframes panel-pop-left {
  from { opacity: 0; transform: translateX(-34px); }
  to { opacity: 1; transform: translateX(0); }
}
.panel-slide { flex: 1; min-width: 0; }

/* Handy 站在面板靠屏幕边的一侧，像把面板从边缘拽出来。 */
.handy-column {
  flex: 0 0 62px;
  display: flex;
  align-items: flex-end;
  justify-content: center;
}
.handy-hop { animation: handy-hop .5s var(--ease-out) .08s both; }
@keyframes handy-hop {
  0% { transform: translateY(0); }
  38% { transform: translateY(-9px) rotate(-3deg); }
  72% { transform: translateY(0); }
  86% { transform: translateY(-3px); }
  100% { transform: translateY(0); }
}
.hud-root.edge-left .handy-hop { animation-name: handy-hop-left; }
@keyframes handy-hop-left {
  0% { transform: translateY(0); }
  38% { transform: translateY(-9px) rotate(3deg); }
  72% { transform: translateY(0); }
  86% { transform: translateY(-3px); }
  100% { transform: translateY(0); }
}

/* 摇晃同样适用于面板形态（拖标题栏时 Handy 跟着晃）。 */
.handy-column.handy-shake .handy-hop { animation: handy-shake .24s ease-in-out infinite; }

@media (prefers-reduced-motion: reduce) {
  .handy-lean, .handy-hop, .panel-card { animation: none; }
}
</style>
