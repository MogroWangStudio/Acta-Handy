<script setup lang="ts">
import { computed } from "vue";
import type { IconName } from "../types/icons";

// Single-color line icons drawn in Acta's style (1.7 stroke, round caps).
const props = defineProps<{ name: IconName; size?: number }>();

const PATHS: Record<IconName, string> = {
  note: '<rect x="4.5" y="3.5" width="15" height="17" rx="2.5"/><path d="M8.5 8.5h7M8.5 12h7M8.5 15.5h4"/>',
  todo: '<rect x="3.5" y="3.5" width="17" height="17" rx="4.5"/><path d="M8.5 12.2l2.6 2.6 5-5.4"/>',
  database: '<ellipse cx="12" cy="5.5" rx="7.5" ry="2.8"/><path d="M4.5 5.5v13c0 1.55 3.36 2.8 7.5 2.8s7.5-1.25 7.5-2.8v-13"/><path d="M4.5 12c0 1.55 3.36 2.8 7.5 2.8s7.5-1.25 7.5-2.8"/>',
  sliders: '<path d="M5 4.5v6m0 3.5v5.5M12 4.5v3m0 3.5v8.5M19 4.5v9.5m0 3.5v2"/><circle cx="5" cy="12.2" r="1.9"/><circle cx="12" cy="9.2" r="1.9"/><circle cx="19" cy="15.7" r="1.9"/>',
  hud: '<rect x="3.5" y="4.5" width="17" height="15" rx="3"/><rect x="11.5" y="11" width="6.5" height="5.5" rx="1.5"/>',
  sun: '<circle cx="12" cy="12" r="4.2"/><path d="M12 3v2m0 14v2M3 12h2m14 0h2M5.4 5.4l1.4 1.4m10.4 10.4l1.4 1.4M18.6 5.4l-1.4 1.4M6.8 17.2l-1.4 1.4"/>',
  moon: '<path d="M20 13.5A8.5 8.5 0 1 1 10.5 4a6.8 6.8 0 0 0 9.5 9.5Z"/>',
  globe: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.4 2.3 3.6 5.2 3.6 8.5s-1.2 6.2-3.6 8.5c-2.4-2.3-3.6-5.2-3.6-8.5s1.2-6.2 3.6-8.5Z"/>',
  info: '<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.2"/><circle cx="12" cy="8" r="0.4" fill="currentColor"/>',
  refresh: '<path d="M19.5 12a7.5 7.5 0 1 1-2.2-5.3"/><path d="M19.7 3.8v3.4h-3.4"/>',
  folder: '<path d="M3.5 7.5c0-1.4 1.1-2.5 2.5-2.5h3l2 2.5h6c1.4 0 2.5 1.1 2.5 2.5v8c0 1.4-1.1 2.5-2.5 2.5H6c-1.4 0-2.5-1.1-2.5-2.5v-10.5Z"/>',
  calendar: '<rect x="4" y="5.5" width="16" height="15" rx="2.5"/><path d="M4 10h16M8.5 3.5v3.5m7-3.5v3.5"/>',
  alert: '<path d="M12 4.2 21 19.5H3L12 4.2Z"/><path d="M12 10v4.2"/><circle cx="12" cy="17" r="0.4" fill="currentColor"/>',
  check: '<path d="M5 12.5l4.5 4.5L19 7.5"/>',
  palette: '<path d="M12 3.5a8.5 8.5 0 1 0 0 17c1.3 0 2-.8 2-1.8 0-1.3-1-1.7-1-2.9 0-1 .8-1.8 2-1.8h1.8c2.1 0 3.7-1.6 3.7-3.7C20.5 6.7 16.7 3.5 12 3.5Z"/><circle cx="8" cy="10" r="0.5" fill="currentColor"/><circle cx="12" cy="7.5" r="0.5" fill="currentColor"/><circle cx="16" cy="10" r="0.5" fill="currentColor"/>',
};

const svg = computed(() => PATHS[props.name]);
</script>

<template>
  <svg
    :width="size ?? 16"
    :height="size ?? 16"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    v-html="svg"
  />
</template>
