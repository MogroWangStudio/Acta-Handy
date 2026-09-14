<script setup lang="ts">
import { computed } from "vue";
import type { ActaFolder, ActaTodo } from "../types/acta";
import { dueLabel, isTodoDone, taskProgress } from "../lib/format";
import { t } from "../lib/i18n";
import PriorityPill from "./PriorityPill.vue";

const props = defineProps<{ todo: ActaTodo; folder?: ActaFolder | null }>();

const done = computed(() => isTodoDone(props.todo));
const due = computed(() => {
  const label = dueLabel(props.todo.startAt, props.todo.dueAt);
  if (!label) return "";
  const diff = new Date(props.todo.dueAt || props.todo.startAt).getTime() - Date.now();
  const cls = diff < 0 ? "overdue" : diff < 86400000 ? "soon" : "";
  return cls ? `${label}|${cls}` : label;
});
const dueText = computed(() => due.value.split("|")[0]);
const dueClass = computed(() => due.value.split("|")[1] ?? "");
const subtasks = computed(() => {
  const p = taskProgress(props.todo);
  return p.total > 0 ? t("subtaskOf")(p.done, p.total) : "";
});
</script>

<template>
  <li class="todo-row" :class="{ done }">
    <span class="todo-check" :class="{ on: done }" aria-hidden="true">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 12.5l4.5 4.5L19 7.5" />
      </svg>
    </span>
    <span class="todo-main">
      <span class="todo-title">{{ todo.title || t("untitledTodo") }}</span>
      <span v-if="subtasks && !done" class="todo-sub">{{ subtasks }}</span>
    </span>
    <span class="todo-side">
      <PriorityPill v-if="todo.priority === 'high'" :priority="todo.priority" />
      <span v-if="dueText" class="todo-due" :class="dueClass">{{ dueText }}</span>
    </span>
  </li>
</template>

<style scoped>
.todo-row {
  min-height: 40px;
  display: grid;
  grid-template-columns: 19px minmax(0, 1fr) auto;
  gap: 9px;
  align-items: center;
  padding: 5px 0;
  border-bottom: 1px solid rgba(47, 52, 45, .07);
  animation: rowIn .4s var(--ease-out) both;
}
.todo-row:last-child { border-bottom: 0; }
html[data-handy-theme="dark"] .todo-row { border-bottom-color: rgba(255, 255, 255, .07); }

.todo-check {
  width: 18px;
  height: 18px;
  border: 1.5px solid color-mix(in srgb, var(--muted) 55%, transparent);
  border-radius: 6px;
  display: grid;
  place-items: center;
  color: #fff;
  transition: background .2s ease, border-color .2s ease, transform .18s var(--ease-out);
}
.todo-check svg { width: 11px; height: 11px; opacity: 0; transform: scale(.4) rotate(-18deg); transition: opacity .15s, transform .34s var(--ease-out); }
.todo-check.on { background: var(--sage); border-color: var(--sage); }
.todo-check.on svg { opacity: 1; transform: scale(1) rotate(0); }
html[data-handy-theme="dark"] .todo-check.on { color: var(--sidebar); }

.todo-main { min-width: 0; display: flex; align-items: baseline; gap: 8px; }
.todo-title {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.4;
  transition: color .2s, opacity .2s;
}
.todo-sub { flex: 0 0 auto; color: var(--faint); font-size: 9px; letter-spacing: .02em; }
.todo-row.done .todo-title { color: var(--faint); text-decoration: line-through; }

.todo-side { display: flex; align-items: center; gap: 7px; min-width: 0; }
.todo-due { white-space: nowrap; color: var(--faint); font-size: 9.5px; letter-spacing: .02em; }
.todo-due.soon { color: var(--amber); }
.todo-due.overdue { color: var(--priority-high-ink); font-weight: 700; }

@keyframes rowIn {
  from { opacity: 0; transform: translateY(5px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
