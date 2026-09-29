<script setup lang="ts">
// Handy 面板窗口：快速编辑面板与右键菜单的独立宿主。Handy 本体在 hud
// 窗口里原地不动，本窗口以后端 panel_layout 按他的位姿定位（贴着图形、
// 底对齐脚底），开合动画只发生在本窗口——两个窗口互不牵动，不再有
// 「整个窗口变大小」的中间帧。
// 收起规则：面板光标离开片刻后收起（光标进入 Handy 会广播 hud-panel-keep
// 取消计时，跨窗口移动不误收）；菜单抢焦点、点外失焦即收，Esc 也能收。
import { onBeforeUnmount, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { initStore, store } from "../lib/store";
import { commitHudScale, onHudPanel, setHudEnabled, setHudPanel, setHudScale } from "../lib/api";
import { t } from "../lib/i18n";
import HudPanel from "../components/HudPanel.vue";

const win = getCurrentWindow();
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

const kind = ref<null | "panel" | "menu">(null);
const side = ref<"left" | "right">("right");
const closing = ref(false);
const liveScale = ref(1);

onMounted(async () => {
  await initStore();
  liveScale.value = store.settings.hud.scale;
  void onHudPanel((p) => {
    if (!p.shown) return;
    closing.value = false;
    kind.value = p.kind;
    side.value = p.side;
    liveScale.value = store.settings.hud.scale;
  });
  void listen("hud-panel-keep", cancelCollapse);
  await win.onFocusChanged(({ payload: focused }) => {
    // 菜单开着时失焦 = 点了外面，与系统菜单的点外关闭一致。面板不抢焦点，
    // 失焦无意义，只按光标离开收起。
    if (!focused && kind.value === "menu") void close();
  });
  window.addEventListener("keydown", onKeydown);
});

let collapseTimer: ReturnType<typeof setTimeout> | null = null;

function onLeave(): void {
  if (kind.value !== "panel" || closing.value) return;
  if (collapseTimer) clearTimeout(collapseTimer);
  collapseTimer = setTimeout(() => {
    if (kind.value === "panel" && !closing.value) void close();
  }, 360);
}

function cancelCollapse(): void {
  if (collapseTimer) clearTimeout(collapseTimer);
  collapseTimer = null;
}

async function close(): Promise<void> {
  if (!kind.value || closing.value) return;
  cancelCollapse();
  // 先播 200ms 退出动画（原路滑回 Handy 身后），再让后端隐藏窗口；
  // 收起全程 Handy 纹丝不动。
  closing.value = true;
  await sleep(200);
  await setHudPanel(false);
  kind.value = null;
  closing.value = false;
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === "Escape") void close();
}

function onScaleInput(): void {
  void setHudScale(liveScale.value);
}

function onScaleCommit(): void {
  void commitHudScale();
  liveScale.value = store.settings.hud.scale;
}

async function closeHandy(): Promise<void> {
  await setHudEnabled(false);
}

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  if (collapseTimer) clearTimeout(collapseTimer);
});
</script>

<template>
  <div class="panel-root" :class="[`edge-${side}`, { closing }]">
    <!-- 快速编辑面板：Handy 站在屏幕边缘 / 原位一侧，面板从它身后展开 -->
    <div v-if="kind === 'panel'" class="panel-card" @pointerleave="onLeave">
      <HudPanel class="panel-slide" />
    </div>

    <!-- 右键菜单：与面板同材质的应用内卡片，大小无极滑块 + 关闭 -->
    <div v-else-if="kind === 'menu'" class="menu-card">
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
</template>

<style scoped>
.panel-root {
  position: fixed;
  inset: 0;
  color: var(--ink);
}

/* --- 卡片 ------------------------------------------------------------------ */

.panel-card, .menu-card {
  position: absolute;
  inset: 8px;
  display: flex;
  border: 1px solid var(--line);
  border-radius: 16px;
  background: var(--paper);
  box-shadow: var(--shadow-pop);
  overflow: hidden;
}
/* Handy 站在卡片的一侧，transform-origin 锚定那一侧：展开时从他身后
   生长出来，收起沿原路退回。 */
.panel-root.edge-right .panel-card, .panel-root.edge-right .menu-card {
  transform-origin: 100% 50%;
  animation: card-in-right .34s var(--ease-spring) both;
}
.panel-root.edge-left .panel-card, .panel-root.edge-left .menu-card {
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
/* 收起：closing 挂在根上，卡片沿原路退出，动画播完由后端隐藏窗口。 */
.panel-root.closing .panel-card, .panel-root.closing .menu-card { animation: card-out-right .18s ease-in both; }
.panel-root.edge-left.closing .panel-card, .panel-root.edge-left.closing .menu-card { animation-name: card-out-left; }
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
  .panel-card, .menu-card { animation: none; }
}
</style>
