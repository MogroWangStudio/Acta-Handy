<script setup lang="ts">
import { computed, onMounted } from "vue";
import { initStore, store } from "../lib/store";
import { bucketTodos } from "../lib/view";
import { dueLabel, isTodoDone } from "../lib/format";
import { t } from "../lib/i18n";

onMounted(initStore);

const cfg = computed(() => store.settings.hud);
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
  <div class="hud-root">
    <div class="hud-card" :style="{ opacity: cfg.opacity }" data-tauri-drag-region>
      <svg class="hud-ring" viewBox="0 0 34 34" aria-hidden="true">
        <circle class="ring-track" cx="17" cy="17" :r="RADIUS" />
        <circle
          class="ring-fill"
          cx="17"
          cy="17"
          :r="RADIUS"
          :stroke-dasharray="`${pct * CIRCUM} ${CIRCUM}`"
        />
        <text x="17" y="20.5" text-anchor="middle">{{ t("doneCount")(buckets.doneToday, total) }}</text>
      </svg>
      <div class="hud-copy" data-tauri-drag-region>
        <p class="hud-title" :class="{ done: !next && total > 0, dim: total === 0 }">{{ nextTitle }}</p>
        <p v-if="next" class="hud-sub" :class="{ overdue }">{{ nextDue }}</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hud-root {
  position: fixed;
  inset: 0;
  padding: 10px;
  display: flex;
}

.hud-card {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 11px;
  padding: 0 15px;
  border: 1px solid var(--line);
  border-radius: 17px;
  background: color-mix(in srgb, var(--paper) 88%, transparent);
  backdrop-filter: blur(22px) saturate(1.4);
  box-shadow: 0 2px 9px rgba(40, 42, 38, .1), 0 14px 34px -10px rgba(40, 42, 38, .24);
  transition: opacity .3s ease, transform .18s var(--ease-out);
}
.hud-card:active { transform: scale(.985); transition-duration: .08s; }

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

@media (prefers-reduced-transparency: reduce) {
  .hud-card { background: var(--paper); backdrop-filter: none; }
}
</style>
