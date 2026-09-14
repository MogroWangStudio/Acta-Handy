<script setup lang="ts">
import type { Priority } from "../types/acta";
import { isChinese } from "../lib/i18n";

defineProps<{ priority: Priority }>();
const LABELS: Record<Priority, [string, string]> = {
  high: ["高", "High"],
  medium: ["中", "Med"],
  low: ["低", "Low"],
};
</script>

<template>
  <span class="priority-pill" :class="priority">{{ isChinese() ? LABELS[priority][0] : LABELS[priority][1] }}</span>
</template>
