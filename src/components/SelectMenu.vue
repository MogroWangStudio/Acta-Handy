<script setup lang="ts">
// MWS ComboBox：自绘下拉，设置窗口统一使用。
// 视觉与交互严格对齐 MWS 控件样式：面板从选中项的位置生长展开（frame 收缩
// 成按钮、plate 内选中行对齐），展开后行按与锚点的距离级联淡入；改选其他项
// 时整列滑动到新选中项居中，再收进按钮——收起点永远对准当前选中行。
// 几何动画全部从当前值继续（settle / snapOpen），动画途中可以随时改主意。
// 颜色不写死：MWS 变量在根上映射到 Handy 主题令牌，十二套主题自动适配。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";

export interface SelectOption {
  value: string | number;
  label: string;
}

const props = defineProps<{ modelValue: string | number; options: SelectOption[] }>();
const emit = defineEmits<{ (e: "update:modelValue", value: string | number): void }>();

// --- 时序参数（与 MWS ComboBox 一致） ----------------------------------------
const DUR = 420;                       // 展开：带轻微回弹的 ease-out
const CLOSE_DUR = Math.round(DUR * 0.6); // 收起更快，收进当前选中行
const MOVE_DUR = 320;                  // 改选后整列滑动
const EASE_OPEN = "cubic-bezier(0.34, 1.56, 0.64, 1)";
const EASE_CLOSE = "cubic-bezier(0.55, 0, 1, 0.45)";
const SHADOW = "0 8px 24px rgba(0,0,0,.14)";
// 系统要求减少动效：时长归零直接切换形态，状态反馈保留。
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const motionMs = (base: number): number => (reducedMotion.matches ? 0 : base);

const ROW = 38;                        // 每行高度，按钮同高
const rows = computed(() => props.options.length);
const popH = computed(() => rows.value * ROW);

const idx = computed(() => {
  const i = props.options.findIndex((o) => o.value === props.modelValue);
  return i < 0 ? 0 : i;
});
const current = computed(() => props.options[idx.value]);

const open = ref(false);
const hoverI = ref(-1);
const anchor = ref(idx.value);         // 展开时对齐按钮的那一行

const root = ref<HTMLElement | null>(null);
const btn = ref<HTMLButtonElement | null>(null);
const pop = ref<HTMLElement | null>(null);
const frame = ref<HTMLElement | null>(null);
const plate = ref<HTMLElement | null>(null);
const chev = ref<SVGElement | null>(null);
const rowEls: HTMLElement[] = [];
function setRow(el: unknown, i: number): void {
  if (el) rowEls[i] = el as HTMLElement;
}

// --- 动画状态机 --------------------------------------------------------------
type Phase = "closed" | "opening" | "open" | "closing";
let phase: Phase = "closed";
let busy = false;                      // recenter 滑动进行中
let lastDir: "center" | "up" | "down" = "center";
let timer = 0;
let moveTimer = 0;
let lastEl: HTMLElement | null = null; // recenter 时被滑动的元素（pop）

function setGeom(top: number, height: number, plateTop: number): void {
  if (!frame.value || !plate.value) return;
  frame.value.style.top = `${top}px`;
  frame.value.style.height = `${height}px`;
  plate.value.style.top = `${plateTop}px`;
}

/** 展开时 pop 相对根的偏移：优先以第 to 行为中心；上下放不下时整体向下/向上贴边。 */
function openOffset(to: number): number {
  const M = 12;
  const vh = window.innerHeight || document.documentElement.clientHeight;
  const rect = root.value?.getBoundingClientRect();
  const top = rect?.top ?? 0;
  const total = popH.value;
  const up = Math.max(0, top - M);
  const down = Math.max(0, vh - (top + ROW) - M);
  const needUp = to * ROW;
  const needDown = (rows.value - to) * ROW;
  const lo = M - top;
  const hi = vh - M - total - top;
  if (up >= needUp && down >= needDown) {
    lastDir = "center";
    return -to * ROW;
  }
  if (down >= total) {
    lastDir = "down";
    return ROW;
  }
  if (up >= total) {
    lastDir = "up";
    return -total;
  }
  lastDir = up >= down ? "down" : "up";
  return lastDir === "down" ? lo : hi;
}

function popTop(): number {
  const v = parseFloat(pop.value?.style.top ?? "");
  return Number.isNaN(v) ? 0 : v;
}

/** 元素当前top：动画途中读的是实时呈现值（从这里继续动画，不跳变）。 */
function curTop(el: HTMLElement): number {
  const v = parseFloat(getComputedStyle(el).top);
  if (!Number.isNaN(v)) return v;
  const s = parseFloat(el.style.top);
  return Number.isNaN(s) ? 0 : s;
}

function contentTop(): number {
  return curTop(pop.value!) + curTop(frame.value!) + curTop(plate.value!);
}

/** 收起几何：frame 缩回按钮位置，plate 内选中行对齐按钮。 */
function closedGeom(): [number, number] {
  const b = popTop();
  return [-b, -idx.value * ROW];
}

/** 行的入场位移：与锚点同侧归零，异侧向锚点方向偏 8px。 */
function shift(i: number): number {
  return i === anchor.value ? 0 : i > anchor.value ? 8 : -8;
}

function layout(): void {
  if (!pop.value) return;
  pop.value.style.transition = "none";
  pop.value.style.top = `${openOffset(anchor.value)}px`;
}

/** 复位到关闭态几何：面板隐形、行全部归零（无过渡），下次展开从这里起步。 */
function closedFrame(): void {
  if (!frame.value || !plate.value) return;
  frame.value.style.transition = "none";
  plate.value.style.transition = "none";
  frame.value.style.boxShadow = "none";
  layout();
  const g = closedGeom();
  setGeom(g[0], ROW, g[1]);
  rowEls.forEach((el, i) => {
    el.style.transition = "none";
    el.style.opacity = "0";
    el.style.transform = `translateY(${shift(i)}px)`;
  });
  void frame.value.offsetWidth;
}

function show(): void {
  if (!pop.value || !frame.value || !plate.value || !chev.value) return;
  clearTimeout(timer);
  clearTimeout(moveTimer);
  busy = false;
  anchor.value = idx.value;
  hoverI.value = -1;
  open.value = true;
  phase = "opening";
  const d = motionMs(DUR);
  pop.value.style.visibility = "visible";
  pop.value.style.pointerEvents = "auto";
  closedFrame();
  frame.value.style.boxShadow = SHADOW;
  frame.value.style.transition = `top ${d}ms ${EASE_OPEN}, height ${d}ms ${EASE_OPEN}`;
  plate.value.style.transition = `top ${d}ms ${EASE_OPEN}`;
  setGeom(0, popH.value, 0);
  chev.value.style.transition = `transform ${d}ms ${EASE_OPEN}`;
  chev.value.style.transform = "rotate(180deg)";
  rowEls.forEach((el, i) => {
    const delay = Math.round(Math.abs(i - anchor.value) * 24);
    el.style.transition = d === 0
      ? "none"
      : `opacity ${Math.round(d * 0.4)}ms ${EASE_OPEN} ${delay}ms, transform ${Math.round(d * 0.55)}ms ${EASE_OPEN} ${delay}ms, background 120ms ease`;
    el.style.opacity = "1";
    el.style.transform = "translateY(0)";
  });
  timer = window.setTimeout(() => {
    if (open.value) phase = "open";
  }, d + 160);
}

function hide(): void {
  if (!open.value || !frame.value || !plate.value || !chev.value) return;
  open.value = false;
  phase = "closing";
  clearTimeout(timer);
  const cd = motionMs(CLOSE_DUR);
  const g = closedGeom();
  frame.value.style.transition = `top ${cd}ms ${EASE_CLOSE}, height ${cd}ms ${EASE_CLOSE}`;
  plate.value.style.transition = `top ${cd}ms ${EASE_CLOSE}`;
  setGeom(g[0], ROW, g[1]);
  chev.value.style.transition = `transform ${cd}ms ${EASE_CLOSE}`;
  chev.value.style.transform = "rotate(0deg)";
  rowEls.forEach((el, i) => {
    if (i === anchor.value) {
      el.style.transition = "none";
      el.style.opacity = "1";
    } else {
      el.style.transition = `opacity ${Math.round(cd * 0.5)}ms ${EASE_CLOSE}`;
      el.style.opacity = "0";
    }
  });
  timer = window.setTimeout(() => {
    if (open.value) return;
    if (pop.value) {
      pop.value.style.visibility = "hidden";
      pop.value.style.pointerEvents = "none";
    }
    phase = "closed";
    anchor.value = idx.value;
    hoverI.value = -1;
    closedFrame();
  }, cd + 30);
}

/** 展开动画未完就改选：先冻结到展开终态，再滑动。 */
function snapOpen(): void {
  if (!frame.value || !plate.value || !chev.value) return;
  clearTimeout(timer);
  frame.value.style.transition = "none";
  plate.value.style.transition = "none";
  chev.value.style.transition = "none";
  setGeom(0, popH.value, 0);
  chev.value.style.transform = "rotate(180deg)";
  rowEls.forEach((el) => {
    el.style.transition = "none";
    el.style.opacity = "1";
    el.style.transform = "translateY(0)";
  });
  void frame.value.offsetWidth;
  phase = "open";
}

/** 改选其他项：整列滑到新选中行居中，滑完收进按钮。 */
function recenter(to: number): void {
  if (!pop.value || !plate.value) return;
  const md = motionMs(MOVE_DUR);
  const p = curTop(pop.value);
  const plateTop = curTop(plate.value);
  const delta = openOffset(to) - contentTop();
  clearTimeout(timer);
  clearTimeout(moveTimer);
  busy = true;
  anchor.value = to;
  // slide 模式：挪 pop（frame/plate 相对它原地不动）。
  plate.value.style.transition = "none";
  plate.value.style.top = `${plateTop}px`;
  pop.value.style.transition = "none";
  pop.value.style.top = `${p}px`;
  void pop.value.offsetWidth;
  pop.value.style.transition = `top ${md}ms ${EASE_OPEN}`;
  pop.value.style.top = `${p + delta}px`;
  lastEl = pop.value;
  moveTimer = window.setTimeout(() => {
    if (!busy) return;
    busy = false;
    if (lastEl) lastEl.style.transition = "none";
    if (!open.value) return;
    if (lastEl) lastEl.style.top = `${p + delta}px`;
    hide();
  }, md + 30);
}

/** 外部事件打断滑动：把 pop 冻结在当前呈现值，再按常规收起。 */
function settle(): void {
  if (!busy) return;
  clearTimeout(moveTimer);
  busy = false;
  if (!lastEl) return;
  const v = curTop(lastEl);
  lastEl.style.transition = "none";
  lastEl.style.top = `${v}px`;
}

// --- 交互 --------------------------------------------------------------------

function commit(k: number): void {
  if (!open.value) return;
  const target = props.options[k];
  if (!target) return;
  if (k === idx.value) {
    anchor.value = k;
    hide();
    return;
  }
  emit("update:modelValue", target.value);
  // 以选中项为中心展开时改选：整列滑到新项居中再收起；贴边模式直接收进新项。
  if (open.value && lastDir === "center") {
    if (phase === "opening") snapOpen();
    recenter(k);
  } else {
    anchor.value = k;
    hide();
  }
}

function toggle(): void {
  settle();
  if (open.value) hide();
  else show();
}

function rowEnter(i: number): void {
  if (!open.value || busy) return;
  hoverI.value = i;
}
function rowLeave(i: number): void {
  if (hoverI.value === i) hoverI.value = -1;
}

function onPointerDown(event: PointerEvent): void {
  if (!open.value || !root.value) return;
  if (!root.value.contains(event.target as Node)) {
    settle();
    hide();
  }
}

function onKeydown(event: KeyboardEvent): void {
  const key = event.key;
  if (!open.value) {
    if (document.activeElement === btn.value && (key === "ArrowDown" || key === "ArrowUp" || key === "Enter" || key === " ")) {
      event.preventDefault();
      show();
    }
    return;
  }
  // 滑动途中只认 Esc（先冻结再收起），其余键不打断动画。
  if (busy && key !== "Escape") return;
  if (key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    settle();
    hide();
  } else if (key === "Tab") {
    hide();
  } else if (key === "ArrowDown" || key === "ArrowUp") {
    event.preventDefault();
    const n = rows.value;
    if (!n) return;
    const from = hoverI.value < 0 ? idx.value : hoverI.value;
    hoverI.value = (from + (key === "ArrowDown" ? 1 : -1) + n) % n;
  } else if (key === "Enter" || key === " ") {
    event.preventDefault();
    if (hoverI.value >= 0 && hoverI.value !== idx.value) commit(hoverI.value);
    else hide();
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("keydown", onKeydown, true);
  reducedMotion.addEventListener("change", onReducedChange);
  nextTick(closedFrame);
});

function onReducedChange(): void {
  // 动效偏好切换时重摆关闭态，避免残留过渡中间值。
  if (!open.value) closedFrame();
}

onBeforeUnmount(() => {
  clearTimeout(timer);
  clearTimeout(moveTimer);
  document.removeEventListener("pointerdown", onPointerDown, true);
  document.removeEventListener("keydown", onKeydown, true);
  reducedMotion.removeEventListener("change", onReducedChange);
});

// 选项列表变化（切换语言后文案更新等）只在关闭态重摆几何。开着时绝不能动：
// 保存设置会让 options 的 computed 重新求值（数组引用变了），若在此收起，
// 改选时的「滑动到新选中项」动画会被半途打断。
watch(() => props.options, () => {
  rowEls.length = rows.value;
  nextTick(() => {
    if (!open.value) closedFrame();
  });
});
</script>

<template>
  <div ref="root" class="mws-cbx">
    <button
      ref="btn"
      type="button"
      class="mws-btn"
      :aria-expanded="open"
      aria-haspopup="listbox"
      @click="toggle"
    >
      <span>{{ current?.label ?? "" }}</span>
      <svg ref="chev" width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 4.5L6 8L9.5 4.5" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" /></svg>
    </button>
    <div ref="pop" class="mws-pop" role="listbox">
      <div ref="frame" class="mws-frame">
        <div ref="plate" class="mws-plate">
          <div
            v-for="(option, i) in options"
            :key="option.value"
            :ref="(el) => setRow(el, i)"
            class="mws-row"
            role="option"
            :aria-selected="i === idx"
            :class="{ 'is-sel': i === idx, 'is-hover': hoverI === i }"
            @pointerenter="rowEnter(i)"
            @pointerleave="rowLeave(i)"
            @click.stop="commit(i)"
          >
            <span>{{ option.label }}</span>
            <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6.2L4.7 9L10 3.2" fill="none" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mws-cbx {
  position: relative;
  flex: 0 0 auto;
  min-width: 132px;
  font-size: 13px;
  /* MWS 变量 → Handy 主题令牌：强调色与选中底色走主题色，面板、文字、
     悬停底色走中性令牌，描边由墨色派生——十二套主题（含深色）自动适配。 */
  --mws-accent: var(--sage);
  --mws-sel: var(--sage-2);
  --mws-text: var(--ink);
  --mws-panel: var(--white);
  --mws-border: color-mix(in srgb, var(--ink) 26%, transparent);
  --mws-hover: var(--panel);
  --mws-font: var(--font-body);
  --mws-size: 13px;
  --mws-weight: 400;
  --mws-spacing: 0px;
  font-family: var(--mws-font);
  letter-spacing: var(--mws-spacing);
}

.mws-btn {
  width: 100%;
  height: 38px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  background: var(--mws-panel);
  color: var(--mws-text);
  border: 0.5px solid var(--mws-border);
  border-radius: 10px;
  font: inherit;
  font-weight: var(--mws-weight);
  letter-spacing: inherit;
  cursor: pointer;
  text-align: left;
}
.mws-btn:focus-visible {
  outline: 2px solid var(--mws-accent);
  outline-offset: 2px;
}
.mws-btn svg { flex: none; }

.mws-pop {
  position: absolute;
  left: 0;
  top: 0;
  width: 100%;
  z-index: 60;
  visibility: hidden;
  pointer-events: none;
}
.mws-frame {
  position: absolute;
  left: 0;
  width: 100%;
  overflow: hidden;
  background: var(--mws-panel);
  border: 0.5px solid var(--mws-border);
  border-radius: 12px;
  corner-shape: round;
}
.mws-plate { position: absolute; left: 0; width: 100%; }

.mws-row {
  height: 38px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  cursor: pointer;
  color: var(--mws-text);
  border-left: 2px solid transparent;
  opacity: 0;
  font-weight: var(--mws-weight);
}
.mws-row svg { flex: none; opacity: 0; }
.mws-row svg path { stroke: var(--mws-accent); }
.mws-row.is-sel { background: var(--mws-sel); font-weight: 500; border-left-color: var(--mws-accent); }
.mws-row.is-sel svg { opacity: 1; }
.mws-row.is-hover { background: var(--mws-hover); }
</style>
