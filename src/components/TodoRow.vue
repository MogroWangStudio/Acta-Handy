<script setup lang="ts">
import { computed, ref } from "vue";
import type { ActaFolder, ActaTodo } from "../types/acta";
import { dueLabel, isTodoDone, taskProgress } from "../lib/format";
import { t } from "../lib/i18n";
import PriorityPill from "./PriorityPill.vue";

const props = defineProps<{ todo: ActaTodo; folder?: ActaFolder | null }>();
const emit = defineEmits<{
  (e: "check", completed: boolean): void;
  (e: "checkTask", taskId: string): void;
}>();

/** 有子任务的行可以展开；默认收起，点行展开完整子任务列表。 */
const expanded = ref(false);
const expandable = computed(() => props.todo.tasks.length > 0);

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
const subtasks = computed(() => taskProgress(props.todo));
</script>

<template>
  <!-- 整行都是展开的点击目标；勾选框与子任务列表自行拦截点击。 -->
  <li class="todo-row" :class="{ done, expandable }" @click="expandable && (expanded = !expanded)">
    <button
      type="button"
      class="todo-check"
      :class="{ on: done }"
      :aria-label="done ? t('markUndone') : t('markDone')"
      @click.stop="emit('check', !done)"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M5 12.5l4.5 4.5L19 7.5" />
      </svg>
    </button>
    <span class="todo-main">
      <span class="todo-title">{{ todo.title || t("untitledTodo") }}</span>
      <span v-if="expandable && !expanded" class="todo-sub">{{ t("subtaskOf")(subtasks.done, subtasks.total) }}</span>
    </span>
    <span class="todo-side">
      <PriorityPill v-if="todo.priority === 'high'" :priority="todo.priority" />
      <span v-if="dueText" class="todo-due" :class="dueClass">{{ dueText }}</span>
      <svg v-if="expandable" class="todo-chevron" :class="{ open: expanded }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M6 9.5l6 6 6-6" />
      </svg>
    </span>

    <ul v-if="expanded" class="task-list" @click.stop>
      <li v-for="task in todo.tasks" :key="task.id" class="task-row" :class="{ done: task.done }">
        <button
          type="button"
          class="task-check"
          :class="{ on: task.done }"
          :aria-label="t('toggleSubtask')"
          @click.stop="emit('checkTask', task.id)"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M5 12.5l4.5 4.5L19 7.5" />
          </svg>
        </button>
        <span class="task-text">{{ task.text }}</span>
      </li>
    </ul>
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
  border-radius: 8px;
  animation: rowIn .4s var(--ease-out) both;
}
.todo-row.expandable { cursor: pointer; transition: background .15s ease; }
.todo-row.expandable:hover { background: color-mix(in srgb, var(--sage-2) 45%, transparent); }
.todo-row:last-child { border-bottom: 0; }
html[data-handy-theme="dark"] .todo-row { border-bottom-color: rgba(255, 255, 255, .07); }

.todo-check {
  width: 18px;
  height: 18px;
  padding: 0;
  border: 1.5px solid color-mix(in srgb, var(--muted) 55%, transparent);
  border-radius: 6px;
  background: transparent;
  display: grid;
  place-items: center;
  color: #fff;
  cursor: pointer;
  transition: background .2s ease, border-color .2s ease, transform .1s ease-out;
}
.todo-check:active { transform: scale(.88); transition-duration: .05s; }
.todo-check svg { width: 11px; height: 11px; opacity: 0; transform: scale(.4) rotate(-18deg); transition: opacity .15s, transform .3s var(--ease-out); }
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
.todo-chevron { width: 12px; height: 12px; color: var(--faint); flex: 0 0 auto; transition: transform .22s var(--ease-out); }
.todo-chevron.open { transform: rotate(180deg); }

.task-list {
  grid-column: 1 / -1;
  margin: 2px 0 4px;
  padding: 0 0 2px 28px;
  list-style: none;
  display: flex;
  flex-direction: column;
  animation: taskListIn .22s var(--ease-out) both;
}
.task-row {
  min-height: 27px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.task-check {
  width: 14px;
  height: 14px;
  flex: 0 0 auto;
  padding: 0;
  border: 1.5px solid color-mix(in srgb, var(--muted) 45%, transparent);
  border-radius: 5px;
  background: transparent;
  color: #fff;
  display: grid;
  place-items: center;
  cursor: pointer;
  transition: background .2s ease, border-color .2s ease, transform .1s ease-out;
}
.task-check:active { transform: scale(.85); transition-duration: .05s; }
.task-check svg { width: 9px; height: 9px; opacity: 0; transform: scale(.4); transition: opacity .12s, transform .24s var(--ease-out); }
.task-check.on { background: var(--sage); border-color: var(--sage); }
.task-check.on svg { opacity: 1; transform: scale(1); }
html[data-handy-theme="dark"] .task-check.on { color: var(--sidebar); }
.task-text {
  font-size: 11px;
  line-height: 1.45;
  color: var(--muted);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  transition: color .2s;
}
.task-row.done .task-text { color: var(--faint); text-decoration: line-through; }

@keyframes rowIn {
  from { opacity: 0; transform: translateY(5px); }
  to { opacity: 1; transform: translateY(0); }
}

/* 展开方向与 chevron 一致：内容自上而下浮现。 */
@keyframes taskListIn {
  from { opacity: 0; transform: translateY(-3px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
