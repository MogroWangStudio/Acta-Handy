<script setup lang="ts">
// Handy 悬浮窗：四种形态——
//   free  自由站在桌面上：悬停时身体轻轻变色、眼睛看向光标；左键点击就地
//         弹出快速编辑面板（Handy 原地不动）；按住拖动改变位置（摇晃以
//         非线性包络起摆、站稳）
//   peek  吸附屏幕边缘：探头趴在边缘，光标靠近就跳出完整身体并展开面板
//   panel 快速编辑面板（待办勾选 + 笔记编辑，改动自动保存）
//   menu  右键自绘菜单（大小滑块 + 关闭），替代原生菜单
// 四种形态共用同一个 Handy 实例：形态切换只改布局，眨眼与呼吸从不停顿。
// 另有隐匿模式：超过设定延迟没有交互就淡出，后端监控光标、靠近时唤醒。
// 贴边开合由后端驱动 hud-anim 动画事件，动画期间不响应任何形态操作。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initStore, store } from "../lib/store";
import {
  commitHudScale,
  onHudAnim,
  onHudGaze,
  onHudMenu,
  onHudScale,
  onHudWake,
  popupHudMenu,
  setHudCursorWatch,
  setHudEnabled,
  setHudEyeWatch,
  setHudMode,
  setHudScale,
} from "../lib/api";
import type { HudPlacement } from "../lib/api";
import { t } from "../lib/i18n";
import HudPanel from "../components/HudPanel.vue";
import HandyChar from "../components/HandyChar.vue";

const win = getCurrentWindow();
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

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
  void onHudGaze(onGaze);
  void onHudMenu(onMenu);
  void onHudScale((scale) => {
    liveScalePreview.value = scale;
  });
  void setHudEyeWatch(true);
  unlistenFocus = await win.onFocusChanged((focused) => {
    // 失焦即收起菜单，与系统菜单的点外关闭一致。
    if (!focused && shape.value === "menu") void closeMenu();
  });
  window.addEventListener("keydown", onKeydown);
  armStealth();
  void watchWindowMoves();
});

const cfg = computed(() => store.settings.hud);
// 缩放滑块拖动中的实时值优先于设置文件：窗口框架由后端实时重排，CSS 的
// 小人要跟同一档缩放，预览才跟手；松手落盘后以 settings-changed 为准。
const liveScalePreview = ref<number | null>(null);
const s = computed(() => liveScalePreview.value ?? cfg.value.scale);
const shape = ref<"free" | "peek" | "panel" | "menu">("free");
const prevShape = ref<"free" | "peek">("free");
const edge = ref<"left" | "right">("right");
const lift = ref(0);
const hidden = ref(false);
const slowFade = ref(false);
const pointerInside = ref(false);
const departing = ref(false);
const closing = ref(false);
const animating = ref(false);
const panelFromPeek = ref(false);
/** 收起面板 / 菜单回到探头的那次切换置真：Handy 一直站在原地，不该重播
    登场动画（peek-pop 从透明弹现，常驻时重播就是闪一下）；离开探头即复位。 */
const noPeekPop = ref(false);
watch(shape, (s) => {
  if (s !== "peek") noPeekPop.value = false;
});

const charW = computed(() => Math.round((shape.value === "peek" || fromPeek.value ? 68 : 64) * s.value));
const panelLift = computed(() => (lift.value > 0 ? lift.value : 5 * s.value));

/** Handy 身体颜色：应用色板预设映射到主题 token（深浅主题自动适配），
    auto 跟随主题墨色。SVG 以 currentColor 填充，改 color 即全身生效。 */
const HANDY_COLORS: Record<string, string> = {
  auto: "var(--ink)",
  sage: "var(--sage)",
  amber: "var(--amber)",
  violet: "var(--violet)",
  danger: "var(--danger)",
};
const handyColor = computed(() => HANDY_COLORS[cfg.value.color] ?? HANDY_COLORS.auto!);

/** 面板 / 菜单是否从探头形态展开：Handy 保持探头位姿原地不动，身体仍被
    屏幕边缘裁掉，卡片朝桌面内侧展开（后端 peek_docked_layout 配合）。 */
const fromPeek = computed(() =>
  shape.value === "panel" ? panelFromPeek.value : shape.value === "menu" ? prevShape.value === "peek" : false,
);

// --- 眼睛跟随 ----------------------------------------------------------------
// 后端以 32ms 轮询光标方向广播 hud-gaze；这里换算成 viewBox 单位的偏移，
// 交给 CSS 过渡插值，视线平滑地追上光标。

const eye = ref({ x: 0, y: 0 });

function onGaze(gaze: { nx: number; ny: number }): void {
  if (reducedMotion.matches) return;
  eye.value = { x: +(gaze.nx * 2).toFixed(2), y: +(gaze.ny * 1.1).toFixed(2) };
}

// --- 拖动摇晃 ----------------------------------------------------------------
// 原生窗口拖动不经过网页指针事件，靠窗口 Moved 事件驱动。摆角 = 包络 ×
// 正弦：包络对「拖动中 / 已松手」做指数趋近，起摆与站稳都是非线性渐变，
// 松手后小人在半摆中自然站稳，绝不生硬归零。

const shakeAngle = ref(0);
const SHAKE_DEG = 4.2;
const SHAKE_PERIOD = 0.46;
let shakeRaf = 0;
let shakeLast = 0;
let shakeEnv = 0;
let shakePhase = 0;
let shakeActive = false;
let shakeIdleTimer: ReturnType<typeof setTimeout> | null = null;

function shakeLoop(now: number): void {
  const dt = Math.min(0.05, (now - shakeLast) / 1000);
  shakeLast = now;
  const tau = shakeActive ? 0.085 : 0.14;
  shakeEnv += ((shakeActive ? 1 : 0) - shakeEnv) * (1 - Math.exp(-dt / tau));
  shakePhase += dt;
  const gain = shakeEnv * shakeEnv; // 平方增益：起步更缓
  shakeAngle.value = +(gain * SHAKE_DEG * Math.sin((shakePhase * 2 * Math.PI) / SHAKE_PERIOD)).toFixed(3);
  if (!shakeActive && shakeEnv < 0.003) {
    shakeEnv = 0;
    shakeAngle.value = 0;
    shakeRaf = 0;
    return;
  }
  shakeRaf = requestAnimationFrame(shakeLoop);
}

function pokeShake(): void {
  if (reducedMotion.matches) return;
  shakeActive = true;
  if (shakeIdleTimer) clearTimeout(shakeIdleTimer);
  shakeIdleTimer = setTimeout(() => {
    shakeActive = false;
  }, 150);
  if (!shakeRaf) {
    shakeLast = performance.now();
    shakeRaf = requestAnimationFrame(shakeLoop);
  }
}

let moveTimer: ReturnType<typeof setTimeout> | null = null;

async function watchWindowMoves(): Promise<void> {
  try {
    await getCurrentWindow().onMoved(() => {
      pokeShake();
      if (moveTimer) clearTimeout(moveTimer);
      moveTimer = setTimeout(() => {
        shakeActive = false;
      }, 150);
    });
  } catch {
    // 浏览器 mock 预览没有窗口事件。
  }
}

// --- 点击 / 拖动 / 右键 -------------------------------------------------------
// 左键按下后位移超过阈值交给系统拖动（原地拖动，1:1 跟手）；未超过视为
// 点击：free 弹出面板、peek 直接展开。右键交给后端切到自绘菜单形态。

const PRESS_DRAG_PX = 8;
const pressArmed = ref(false);
let pressStart = { x: 0, y: 0 };

function onBodyPointerDown(e: PointerEvent): void {
  if (animating.value || hidden.value || e.button !== 0) return;
  // 按住 Handy 即取消悬停展开计时：吸附探头形态下光标一靠近（90ms）就会
  // 弹出面板，会把随后的系统拖动打断（表现为「吸边后无法拖动」）。
  if (expandTimer) {
    clearTimeout(expandTimer);
    expandTimer = null;
  }
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
  void win.startDragging().catch(() => undefined);
  // 系统拖动接管后指针事件不会再到达：兜底清理按压状态，否则悬停展开
  // 会被残留的按压状态挡住，探头形态点开面板从此失灵。
  setTimeout(() => {
    pressArmed.value = false;
  }, 400);
}

function onBodyPointerUp(e: PointerEvent): void {
  if (!pressArmed.value || e.button !== 0) return;
  pressArmed.value = false;
  if (shape.value === "free") void openPanel();
  else if (shape.value === "peek") void expand();
}

function onBodyContextMenu(e: MouseEvent): void {
  e.preventDefault();
  if (animating.value || hidden.value || closing.value) return;
  if (shape.value !== "free" && shape.value !== "peek") return;
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
  expandTimer = setTimeout(autoExpand, 90);
}

function onLeave(): void {
  pointerInside.value = false;
  if (expandTimer) clearTimeout(expandTimer);
  if (shape.value !== "panel" && shape.value !== "menu") {
    armStealth();
    return;
  }
  if (shape.value === "panel") scheduleCollapse();
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

/** 自动展开（悬停 / 光标靠近）的统一入口：按住 Handy 时让路给拖动。 */
function autoExpand(): void {
  if (pressArmed.value) return;
  if (shape.value === "peek") void expand();
}

async function openPanel(): Promise<void> {
  if (shape.value !== "free" || animating.value || closing.value) return;
  // 先切布局再等后端展开窗口：两种布局里 Handy 在窗口内的位置完全一致
  // （14s 内缩、5s 落地），窗口扩开时 Handy 纹丝不动；卡片以透明起点从
  // Handy 一侧弹出，同一实例继续眨眼呼吸，没有任何重挂载的闪动。
  panelFromPeek.value = false;
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
  if (shape.value !== "peek" || animating.value || closing.value) return;
  // Handy 保持探头位姿原地不动，窗口朝桌面内侧扩开（后端配合），只展开面板。
  panelFromPeek.value = true;
  const placement = await setHudMode("panel");
  if (placement) {
    edge.value = placement.side;
    lift.value = placement.lift;
  }
  shape.value = "panel";
  armStealth();
}

async function collapse(): Promise<void> {
  if (shape.value !== "panel" || closing.value) return;
  // 先播 180ms 退出动画（原路滑回 Handy 身后），再切形态，进出同一条路径。
  // closing 要保持到形态切换之后才清：提前清掉会把 card-in 进场动画（fill
  // both）重新触发，刚收起的卡片又弹出来闪一下，等 IPC 返回才被卸载。
  closing.value = true;
  await sleep(190);
  const target = cfg.value.snapToEdge ? "peek" : "free";
  noPeekPop.value = target === "peek";
  const placement = await setHudMode(target);
  if (placement) {
    edge.value = placement.side;
    lift.value = placement.lift;
  }
  shape.value = target;
  panelFromPeek.value = false;
  closing.value = false;
  armStealth();
}

// --- 右键菜单（自绘） ---------------------------------------------------------

const liveScale = ref(1);

function onMenu(placement: HudPlacement): void {
  prevShape.value = shape.value === "peek" ? "peek" : "free";
  edge.value = placement.side;
  lift.value = placement.lift;
  liveScale.value = store.settings.hud.scale;
  shape.value = "menu";
  armStealth();
}

function onScaleInput(): void {
  void setHudScale(liveScale.value);
}

function onScaleCommit(): void {
  void commitHudScale();
}

async function closeMenu(): Promise<void> {
  if (shape.value !== "menu" || closing.value) return;
  // 时序同 collapse：closing 保持到形态切换之后，卡片不会重播进场动画。
  closing.value = true;
  await sleep(180);
  noPeekPop.value = prevShape.value === "peek";
  const placement = await setHudMode(prevShape.value);
  if (placement) {
    edge.value = placement.side;
    lift.value = placement.lift;
  }
  shape.value = prevShape.value;
  closing.value = false;
  armStealth();
}

async function closeHandy(): Promise<void> {
  await setHudEnabled(false);
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === "Escape" && shape.value === "menu") void closeMenu();
}

// --- 形态与设置联动 -----------------------------------------------------------

watch(
  () => cfg.value.snapToEdge,
  (snap) => {
    if (animating.value) return; // 动画事件负责切换形态
    if (shape.value === "panel" || shape.value === "menu") return; // 关闭时按新设置重排
    shape.value = snap ? "peek" : "free";
    armStealth();
  },
);
watch(
  () => cfg.value.enabled,
  (enabled) => {
    // 右键菜单关闭再开启时，窗口以自由 / 探头形态出现；眼动监控线程随
    // 隐藏退出，重新可见后要再武装。
    if (enabled) {
      void setHudEyeWatch(true);
      if (shape.value === "panel" || shape.value === "menu") {
        shape.value = cfg.value.snapToEdge ? "peek" : "free";
      }
    }
  },
);
watch(() => cfg.value.stealth, () => armStealth());

// --- 贴边过渡动画 ------------------------------------------------------------

function onAnim(payload: {
  phase: "start" | "reveal" | "end";
  to?: "peek" | "free";
  side?: "left" | "right";
}): void {
  if (payload.phase === "start") {
    animating.value = true;
    if (payload.to === "peek") {
      // start 即带方向：Handy 提前朝目标边探身，滑行有了身体语言。
      departing.value = true;
      if (payload.side) edge.value = payload.side;
    }
  } else if (payload.phase === "reveal" && payload.to) {
    departing.value = false;
    if (payload.side) edge.value = payload.side;
    // 动画中的形态切换：Handy 一直可见（此刻刚从屏幕外探进来 / 滑回落点），
    // 不重播 peek-pop 登场动画（透明起点弹现 = 闪一下）；登场只留给真正的出场。
    if (payload.to === "peek") noPeekPop.value = true;
    shape.value = payload.to;
  } else if (payload.phase === "end") {
    animating.value = false;
    departing.value = false;
    panelFromPeek.value = false;
    const next = cfg.value.snapToEdge ? "peek" : "free";
    noPeekPop.value = next === "peek";
    shape.value = next;
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
  if (shape.value !== "free" && shape.value !== "peek") return;
  if (!cfg.value.stealth || pointerInside.value || editingInput()) return;
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
  if (shape.value === "peek") autoExpand();
}

let unlistenFocus: (() => void) | null = null;

onBeforeUnmount(() => {
  void win.setIgnoreCursorEvents(false).catch(() => undefined);
  void setHudCursorWatch(false);
  void setHudEyeWatch(false);
  unlistenFocus?.();
  window.removeEventListener("keydown", onKeydown);
  if (stealthTimer) clearTimeout(stealthTimer);
  if (watchTimer) clearTimeout(watchTimer);
  if (expandTimer) clearTimeout(expandTimer);
  if (collapseTimer) clearTimeout(collapseTimer);
  if (moveTimer) clearTimeout(moveTimer);
  if (shakeIdleTimer) clearTimeout(shakeIdleTimer);
  if (shakeRaf) cancelAnimationFrame(shakeRaf);
});
</script>

<template>
  <div
    class="hud-root"
    :class="[shape, `edge-${edge}`, { departing, hidden, 'from-peek': fromPeek, 'no-pop': noPeekPop }]"
    :style="{ '--s': s, '--lift': panelLift, '--eye-x': eye.x, '--eye-y': eye.y, '--shake': `${shakeAngle}deg`, '--handy-color': handyColor }"
    @pointerenter="onEnter"
    @pointerleave="onLeave"
    @pointerdown.capture="armStealth"
    @focusin="armStealth"
    @focusout="onFocusOut"
  >
    <!-- Handy 本体：四种形态共用同一实例，切换形态只改布局，眨眼与呼吸不停顿 -->
    <div class="handy-stage" :class="{ slow: slowFade }">
      <div class="handy-pos">
        <div class="handy-tilt">
          <div
            class="handy-lean"
            @pointerdown="onBodyPointerDown"
            @pointermove="onBodyPointerMove"
            @pointerup="onBodyPointerUp"
            @pointercancel="pressArmed = false"
            @contextmenu="onBodyContextMenu"
          >
            <HandyChar :width="charW" />
          </div>
        </div>
      </div>
    </div>

    <!-- 快速编辑面板：Handy 站在屏幕边缘 / 原位一侧，面板从它身后展开 -->
    <div v-if="shape === 'panel'" class="overlay" @pointerdown.self="void collapse()">
      <div class="panel-card" :class="{ closing }">
        <HudPanel class="panel-slide" />
      </div>
    </div>

    <!-- 右键菜单：与面板同材质的应用内卡片，大小无极滑块 + 关闭 -->
    <div v-else-if="shape === 'menu'" class="overlay" @pointerdown.self="closeMenu">
      <div class="menu-card" :class="{ closing }" @pointerdown.stop>
        <p class="menu-title">{{ t("hudSize") }}</p>
        <div class="menu-row">
          <input
            v-model.number="liveScale"
            class="menu-slider"
            type="range"
            min="0.2"
            max="1.5"
            step="0.05"
            @input="onScaleInput"
            @change="onScaleCommit"
            @pointerup="onScaleCommit"
          />
          <span class="menu-pct">{{ Math.round(liveScale * 100) }}%</span>
        </div>
        <div class="menu-sep" />
        <button class="menu-item" @click="closeHandy">
          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3.2 3.2l5.6 5.6M8.8 3.2l-5.6 5.6" /></svg>
          {{ t("hudClose") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hud-root {
  position: fixed;
  inset: 0;
  color: var(--ink);
  /* --s：Handy 缩放（0.2–1.5）；--lift：面板/菜单形态下脚底离窗口底部的距离。 */
}

/* --- Handy 本体 ------------------------------------------------------------- */

.handy-stage {
  position: absolute;
  inset: 0;
  overflow: hidden; /* 探头形态靠它把探出屏幕外的身体裁掉 */
  opacity: 1;
  transition: opacity .16s ease-out;
}
.handy-stage.slow { transition: opacity .95s ease; }
.hud-root.hidden .handy-stage { opacity: 0; }

/* 站位统一用 left + bottom 表达；形态切换时窗口瞬移、坐标同步瞬切，
   两相抵消让 Handy 的屏幕位置连续（free 与 panel/menu 的内偏移完全一致：
   14s 内缩、5s 落地，开合纹丝不动），绝不出现「窗口先跳、元素再滑」。 */
.handy-pos {
  position: absolute;
  bottom: calc(5px * var(--s));
  left: calc(14px * var(--s));
}
.hud-root.peek .handy-pos { bottom: calc(8px * var(--s)); }
.hud-root.peek.edge-right .handy-pos { left: calc(4px * var(--s)); } /* 右缘探出窗口外 */
.hud-root.peek.edge-left .handy-pos { left: calc(-18px * var(--s)); }
.hud-root.panel .handy-pos, .hud-root.menu .handy-pos { bottom: calc(var(--lift) * 1px); }
.hud-root.panel.edge-right .handy-pos, .hud-root.menu.edge-right .handy-pos { left: calc(100% - 78px * var(--s)); }
.hud-root.panel.edge-left .handy-pos, .hud-root.menu.edge-left .handy-pos { left: calc(14px * var(--s)); }
/* 从探头形态展开的面板 / 菜单：Handy 仍以探头位姿扒在屏幕边缘，身体探出
   窗口（被裁掉），与 peek 的内偏移一致——开合只动窗口，不动 Handy。 */
.hud-root.panel.from-peek.edge-left .handy-pos, .hud-root.menu.from-peek.edge-left .handy-pos { left: calc(-18px * var(--s)); }
.hud-root.panel.from-peek.edge-right .handy-pos, .hud-root.menu.from-peek.edge-right .handy-pos { left: calc(100% - 46px * var(--s)); }
.hud-root.panel.from-peek.edge-left .handy-tilt, .hud-root.menu.from-peek.edge-left .handy-tilt { transform: rotate(9deg); transform-origin: 0 100%; }
.hud-root.panel.from-peek.edge-right .handy-tilt, .hud-root.menu.from-peek.edge-right .handy-tilt { transform: rotate(-9deg); transform-origin: 100% 100%; }

/* 探头时朝桌面一侧探身：以抓边的脚底为轴；开合时倾角平滑过渡，
   滑向边缘的途中先探一半（departing），像跑向边缘扒住。 */
.handy-tilt {
  transform-origin: 50% 100%;
  transition: transform .36s var(--ease-out);
}
.hud-root.peek.edge-right .handy-tilt { transform: rotate(-9deg); transform-origin: 100% 100%; }
.hud-root.peek.edge-left .handy-tilt { transform: rotate(9deg); transform-origin: 0 100%; }
.hud-root.departing.edge-right .handy-tilt { transform: rotate(-6deg); transform-origin: 100% 100%; }
.hud-root.departing.edge-left .handy-tilt { transform: rotate(6deg); transform-origin: 0 100%; }

/* 拖动摇晃：角度由 rAF 包络注入 --shake（挂在 HandyChar 内层），
   这里只保留登场小弹跳。 */
.handy-lean {
  transform-origin: 50% 100%;
  cursor: default;
  /* 身体颜色来自设置的色板预设（--handy-color，默认墨色）；悬停仍染主题
     强调色，离开缓缓褪回——设置改色时也走这段过渡，渐变换色。 */
  color: var(--handy-color, var(--ink));
  transition: color .35s var(--ease-out);
}
.handy-lean:hover { color: var(--sage); }

/* 收回探头形态时小弹跳登场；展开面板 / 菜单时原地蹦一下把卡片「拽」
   出来——落点不变，只是身体语言。 */
.hud-root.peek .handy-lean { animation: peek-pop .32s var(--ease-out) both; }
/* 收起面板 / 菜单回到探头：Handy 常驻原地，不重播登场动画（peek-pop 以透明
   起点 弹现，常驻元素重播就是闪一下）；真正的出场（启用、贴边动画）不受影响。 */
.hud-root.peek.no-pop .handy-lean { animation: none; }
@keyframes peek-pop {
  from { opacity: 0; transform: scale(.86); }
  to { opacity: 1; transform: scale(1); }
}
.hud-root.panel .handy-lean, .hud-root.menu .handy-lean { animation: handy-hop .5s var(--ease-out) both; }
.hud-root.panel.edge-left .handy-lean, .hud-root.menu.edge-left .handy-lean { animation-name: handy-hop-left; }
@keyframes handy-hop {
  0% { transform: translateY(0); }
  38% { transform: translateY(-9px) rotate(-3deg); }
  72% { transform: translateY(0); }
  86% { transform: translateY(-3px); }
  100% { transform: translateY(0); }
}
@keyframes handy-hop-left {
  0% { transform: translateY(0); }
  38% { transform: translateY(-9px) rotate(3deg); }
  72% { transform: translateY(0); }
  86% { transform: translateY(-3px); }
  100% { transform: translateY(0); }
}

/* --- 面板 / 菜单 -------------------------------------------------------------- */

.overlay {
  position: absolute;
  inset: 0;
}

.panel-card, .menu-card {
  position: absolute;
  top: 8px;
  bottom: 8px;
  display: flex;
  border: 1px solid var(--line);
  border-radius: 16px;
  background: var(--paper);
  box-shadow: var(--shadow-pop);
  overflow: hidden;
}
/* Handy 站在屏幕边缘（或原位）一侧，卡片让出整个站位；transform-origin
   锚定 Handy 一侧，展开时从它身后生长出来，收起沿原路退回。 */
.hud-root.edge-right .panel-card, .hud-root.edge-right .menu-card {
  left: 8px;
  right: calc(82px * var(--s));
  transform-origin: 100% 50%;
  animation: card-in-right .34s var(--ease-spring) both;
}
.hud-root.edge-left .panel-card, .hud-root.edge-left .menu-card {
  right: 8px;
  left: calc(82px * var(--s));
  transform-origin: 0 50%;
  animation: card-in-left .34s var(--ease-spring) both;
}
@keyframes card-in-right {
  from { opacity: 0; transform: translateX(26px) scale(.95); }
  to { opacity: 1; transform: none; }
}
@keyframes card-in-left {
  from { opacity: 0; transform: translateX(-26px) scale(.95); }
  to { opacity: 1; transform: none; }
}
.panel-card.closing { animation: card-out-right .18s ease-in both; }
.hud-root.edge-left .panel-card.closing, .hud-root.edge-left .menu-card.closing { animation-name: card-out-left; }
.menu-card.closing { animation: card-out-right .18s ease-in both; }
@keyframes card-out-right {
  from { opacity: 1; transform: none; }
  to { opacity: 0; transform: translateX(26px) scale(.95); }
}
@keyframes card-out-left {
  from { opacity: 1; transform: none; }
  to { opacity: 0; transform: translateX(-26px) scale(.95); }
}
.panel-slide { flex: 1; min-width: 0; }

/* --- 自绘右键菜单 ------------------------------------------------------------- */

.menu-card {
  display: flex;
  flex-direction: column;
  padding: 12px 14px 8px;
}
.menu-title {
  margin: 0 0 8px;
  color: var(--muted);
  font-size: 9px;
  font-weight: 700;
  letter-spacing: .1em;
  text-transform: uppercase;
}
.menu-row { display: flex; align-items: center; gap: 9px; }
.menu-slider { flex: 1; min-width: 0; accent-color: var(--sage); }
.menu-pct {
  flex: 0 0 auto;
  min-width: 38px;
  text-align: right;
  color: var(--ink);
  font-size: 11px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.menu-sep { height: 1px; margin: 10px -14px 4px; background: var(--line); }
.menu-item {
  height: 30px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--danger);
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 0 7px;
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: background .15s ease;
}
.menu-item:hover { background: var(--danger-soft); }
.menu-item svg { width: 11px; height: 11px; fill: none; stroke: currentColor; stroke-width: 1.4; stroke-linecap: round; }

@media (prefers-reduced-motion: reduce) {
  .handy-tilt, .handy-pos, .handy-lean { transition: none; }
  .handy-lean, .panel-card, .menu-card { animation: none; }
}
</style>
