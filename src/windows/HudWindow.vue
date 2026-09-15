<script setup lang="ts">
// Handy 悬浮窗：三种形态——
//   free  自由站在桌面上：悬停轻轻提亮；左键点击就地弹出快速编辑面板
//         （Handy 原地不动）；按住拖动改变位置；右键弹出大小 / 关闭菜单
//   peek  吸附屏幕边缘：探头趴在边缘，光标靠近就跳出完整身体并展开面板
//   panel 快速编辑面板（待办勾选 + 笔记编辑，改动自动保存）
// 另有隐匿模式：超过设定延迟没有交互就淡出，后端监控光标、靠近时唤醒。
// 贴边开合由后端驱动 hud-anim 动画事件，动画期间不响应任何形态操作。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initStore, store } from "../lib/store";
import { onHudAnim, onHudWake, popupHudMenu, setHudCursorWatch, setHudMode } from "../lib/api";
import HudPanel from "../components/HudPanel.vue";
import HandyChar from "../components/HandyChar.vue";

const win = getCurrentWindow();

onMounted(async () => {
  await initStore();
  shape.value = store.settings.hud.snapToEdge ? "peek" : "free";
  // 让后端按当前形态重排一次，顺便拿到贴边方向。
  const placement = await setHudMode(shape.value);
  if (placement) {
    edge.value = placement.side;
    lift.value = placement.lift;
  }
  void onHudWake(onCursorNear);
  void onHudAnim(onAnim);
  armStealth();
  void watchWindowMoves();
});

const cfg = computed(() => store.settings.hud);
const s = computed(() => cfg.value.scale);
const shape = ref<"free" | "peek" | "panel">("free");
const edge = ref<"left" | "right">("right");
const lift = ref(0);
const hidden = ref(false);
const slowFade = ref(false);
const pointerInside = ref(false);
const shaking = ref(false);
const hovering = ref(false);
const animating = ref(false);

const charW = computed(() => Math.round((shape.value === "peek" ? 68 : 64) * s.value));
const panelLift = computed(() => (lift.value > 0 ? lift.value : 5 * s.value));

/** 悬停提亮只在自由形态生效；探头形态用「靠近展开」代替。 */
const stageOpacity = computed(() => {
  if (hidden.value) return 0;
  const base = cfg.value.opacity;
  if (shape.value === "free" && hovering.value) return Math.min(1, base + 0.14);
  return base;
});

watch(
  () => cfg.value.snapToEdge,
  (snap) => {
    if (animating.value) return; // 动画事件负责切换形态
    if (shape.value === "panel") return; // 面板由后端按新吸附设置重排
    shape.value = snap ? "peek" : "free";
    armStealth();
  },
);
watch(
  () => cfg.value.enabled,
  (enabled) => {
    // 右键菜单关闭再开启时，窗口以自由 / 探头形态出现。
    if (enabled && shape.value === "panel") {
      shape.value = cfg.value.snapToEdge ? "peek" : "free";
    }
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

// --- 点击 / 拖动 / 右键 -------------------------------------------------------
// 左键按下后位移超过阈值交给系统拖动（原地拖动，1:1 跟手）；未超过视为
// 点击：free 弹出面板、peek 直接展开。右键交给后端弹原生菜单。

const PRESS_DRAG_PX = 8;
const pressArmed = ref(false);
let pressStart = { x: 0, y: 0 };

function onBodyPointerDown(e: PointerEvent): void {
  if (animating.value || hidden.value || e.button !== 0) return;
  pressArmed.value = true;
  pressStart = { x: e.clientX, y: e.clientY };
  try {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  } catch {
    // 指针已失效时无需捕获。
  }
}

function onBodyPointerMove(e: PointerEvent): void {
  if (!pressArmed.value) return;
  const dx = e.clientX - pressStart.x;
  const dy = e.clientY - pressStart.y;
  if (dx * dx + dy * dy < PRESS_DRAG_PX * PRESS_DRAG_PX) return;
  pressArmed.value = false;
  hovering.value = false;
  void win.startDragging().catch(() => undefined);
}

function onBodyPointerUp(e: PointerEvent): void {
  if (!pressArmed.value || e.button !== 0) return;
  pressArmed.value = false;
  if (shape.value === "free") void openPanel();
  else if (shape.value === "peek") void expand();
}

function onBodyContextMenu(e: MouseEvent): void {
  e.preventDefault();
  if (animating.value || hidden.value || shape.value === "panel") return;
  void popupHudMenu();
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

async function openPanel(): Promise<void> {
  if (shape.value !== "free" || animating.value) return;
  // 先切布局再等后端展开窗口：两种布局里 Handy 在窗口内的位置完全一致
  // （14s 内缩、5s 落地），窗口扩开时 Handy 纹丝不动；卡片以透明起点弹出，
  // 展开方向在等后端返回的间隙内即已定准。
  shape.value = "panel";
  try {
    const placement = await setHudMode("panel");
    if (placement) {
      edge.value = placement.side;
      lift.value = placement.lift;
    }
  } catch {
    shape.value = "free";
  }
  armStealth();
}

async function expand(): Promise<void> {
  if (shape.value !== "peek" || animating.value) return;
  const placement = await setHudMode("panel");
  if (placement) {
    edge.value = placement.side;
    lift.value = placement.lift;
  }
  shape.value = "panel";
  armStealth();
}

async function collapse(): Promise<void> {
  if (shape.value !== "panel") return;
  const target = cfg.value.snapToEdge ? "peek" : "free";
  await setHudMode(target);
  shape.value = target;
  armStealth();
}

// --- 贴边过渡动画 ------------------------------------------------------------

function onAnim(payload: { phase: "start" | "reveal" | "end"; to?: "peek" | "free" }): void {
  if (payload.phase === "start") {
    animating.value = true;
    hovering.value = false;
  } else if (payload.phase === "reveal" && payload.to) {
    shape.value = payload.to;
  } else if (payload.phase === "end") {
    animating.value = false;
    shape.value = cfg.value.snapToEdge ? "peek" : "free";
    armStealth();
  }
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
  if (animating.value) return;
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
    :style="{ '--s': s, '--lift': panelLift }"
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
      :style="{ opacity: stageOpacity }"
    >
      <div class="handy-pos">
        <div class="handy-tilt">
          <div
            class="handy-lean"
            :class="{ 'handy-shake': shaking }"
            @pointerenter="hovering = true"
            @pointerleave="hovering = false"
            @pointerdown="onBodyPointerDown"
            @pointermove="onBodyPointerMove"
            @pointerup="onBodyPointerUp"
            @pointercancel="pressArmed = false"
            @contextmenu="onBodyContextMenu"
          >
            <HandyChar class="handy-char" :width="charW" />
          </div>
        </div>
      </div>
    </div>

    <!-- 快速编辑面板：Handy 站在屏幕边缘 / 原位一侧，面板从它身旁展开 -->
    <div v-else class="panel-root">
      <div class="panel-card" :style="{ opacity: cfg.opacity }">
        <HudPanel class="panel-slide" />
      </div>
      <div class="panel-handy">
        <div class="handy-hop" :class="{ 'handy-shake': shaking }">
          <HandyChar :width="Math.round(64 * s)" />
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
  /* --s：Handy 缩放档位；--lift：面板形态下脚底离窗口底部的距离。 */
}

/* --- Handy 本体 ------------------------------------------------------------- */

.handy-stage {
  position: absolute;
  inset: 0;
  overflow: hidden; /* 探头形态靠它把探出屏幕外的身体裁掉 */
  transition: opacity .16s ease-out;
}
.handy-stage.slow { transition: opacity .95s ease; }

.handy-pos { position: absolute; bottom: calc(5px * var(--s)); left: calc(50% - 32px * var(--s)); }
.hud-root.peek .handy-pos { bottom: calc(8px * var(--s)); }
.hud-root.peek.edge-right .handy-pos { left: auto; right: calc(-18px * var(--s)); }
.hud-root.peek.edge-left .handy-pos { left: calc(-18px * var(--s)); }

/* 探头时朝桌面一侧探身：以抓边的脚底为轴。 */
.handy-tilt { transform-origin: 50% 100%; }
.hud-root.peek.edge-right .handy-tilt { transform: rotate(-9deg); transform-origin: 100% 100%; }
.hud-root.peek.edge-left .handy-tilt { transform: rotate(9deg); transform-origin: 0 100%; }

/* 拖动中：以脚底为轴左右摇晃。选择器与 peek-pop 同优先级、放在其后，
   保证探头形态下摇晃仍能覆盖登场动画。 */
.handy-lean { transform-origin: 50% 100%; cursor: default; }
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
}

.panel-card {
  position: absolute;
  top: 8px;
  bottom: 8px;
  display: flex;
  border: 1px solid var(--line);
  border-radius: 16px;
  background: var(--paper);
  overflow: hidden;
}
/* Handy 站在屏幕边缘（或原位）一侧，卡片让出整个站位。 */
.hud-root.edge-right .panel-card {
  left: 8px;
  right: calc(82px * var(--s));
  animation: panel-pop-right .36s var(--ease-spring) both;
}
.hud-root.edge-left .panel-card {
  right: 8px;
  left: calc(82px * var(--s));
  animation: panel-pop-left .36s var(--ease-spring) both;
}
@keyframes panel-pop-right {
  from { opacity: 0; transform: translateX(34px); }
  to { opacity: 1; transform: translateX(0); }
}
@keyframes panel-pop-left {
  from { opacity: 0; transform: translateX(-34px); }
  to { opacity: 1; transform: translateX(0); }
}
.panel-slide { flex: 1; min-width: 0; }

/* 面板里的 Handy 与自由形态完全同偏移（14s 内缩、5s 落地），开合不挪位；
   屏幕上缘截断面板时由后端抬高 --lift 兜底。 */
.panel-handy {
  position: absolute;
  bottom: calc(var(--lift) * 1px);
  line-height: 0;
}
.hud-root.edge-right .panel-handy { right: calc(14px * var(--s)); }
.hud-root.edge-left .panel-handy { left: calc(14px * var(--s)); }
.panel-handy .handy-hop { animation: handy-hop .5s var(--ease-out) .08s both; }
@keyframes handy-hop {
  0% { transform: translateY(0); }
  38% { transform: translateY(-9px) rotate(-3deg); }
  72% { transform: translateY(0); }
  86% { transform: translateY(-3px); }
  100% { transform: translateY(0); }
}
.hud-root.edge-left .panel-handy .handy-hop { animation-name: handy-hop-left; }
@keyframes handy-hop-left {
  0% { transform: translateY(0); }
  38% { transform: translateY(-9px) rotate(3deg); }
  72% { transform: translateY(0); }
  86% { transform: translateY(-3px); }
  100% { transform: translateY(0); }
}

/* 摇晃同样适用于面板形态（拖标题栏时 Handy 跟着晃）。 */
.panel-handy .handy-hop.handy-shake { animation: handy-shake .24s ease-in-out infinite; }

@media (prefers-reduced-motion: reduce) {
  .handy-lean, .panel-handy .handy-hop, .panel-card { animation: none; }
}
</style>
