<script setup lang="ts">
import { computed, onMounted } from "vue";
import { checkTask, checkTodo, initStore, refreshActaData, store } from "../lib/store";
import { bucketTodos, folderMap } from "../lib/view";
import { longDate } from "../lib/format";
import { t } from "../lib/i18n";
import { refreshData, showWindow } from "../lib/api";
import AppIcon from "../components/AppIcon.vue";
import LogoMark from "../components/LogoMark.vue";
import TodoRow from "../components/TodoRow.vue";

onMounted(initStore);

const cfg = computed(() => store.settings.todoWidget);
const buckets = computed(() => bucketTodos(store.data?.todos ?? []));
const folders = computed(() => folderMap(store.data));
const today = computed(() => longDate(new Date()));

const completedToday = computed(() =>
  buckets.value.completed.filter((todo) => {
    const iso = todo.dueAt || todo.startAt;
    if (!iso) return true;
    return new Date(iso).getTime() <= endOfToday();
  }),
);

function endOfToday(): number {
  const d = new Date();
  d.setHours(23, 59, 59, 999);
  return d.getTime();
}

const shown = computed(() =>
  cfg.value.showCompleted
    ? [...buckets.value.current, ...completedToday.value]
    : buckets.value.current,
);
type WidgetState = "no-folder" | "error" | "clear" | "upcoming" | "list";
const state = computed<WidgetState>(() => {
  if (!store.settings.dataFolder) return "no-folder";
  if (store.dataError) return "error";
  if (buckets.value.totalToday === 0) {
    return buckets.value.upcoming.length > 0 ? "upcoming" : "clear";
  }
  return "list";
});

async function openSettings(): Promise<void> {
  await showWindow("main");
}
async function reload(): Promise<void> {
  await refreshData();
  await refreshActaData();
}
</script>

<template>
  <div class="widget-root">
    <div class="widget-card" :style="{ opacity: cfg.opacity }">
      <header class="widget-head" data-tauri-drag-region>
        <div>
          <h1 class="widget-title" data-tauri-drag-region>{{ t("todayTodos") }}</h1>
          <p class="widget-subline" data-tauri-drag-region>{{ today }}</p>
        </div>
        <span class="widget-count">{{ t("doneCount")(buckets.doneToday, buckets.totalToday) }}</span>
      </header>
      <div class="widget-progress progress-track" aria-hidden="true">
        <i :style="{ width: buckets.totalToday ? `${(buckets.doneToday / buckets.totalToday) * 100}%` : '0%' }" />
      </div>

      <template v-if="state === 'list'">
        <ul class="widget-list">
          <TodoRow
            v-for="todo in shown"
            :key="todo.id"
            :todo="todo"
            :folder="folders.get(todo.folderId)"
            @check="(completed) => checkTodo(todo.id, completed)"
            @check-task="(taskId) => checkTask(todo.id, taskId)"
          />
          <template v-if="buckets.upcoming.length > 0">
            <li class="widget-section-label section-label">{{ t("upcoming") }}</li>
            <TodoRow
              v-for="todo in buckets.upcoming.slice(0, 3)"
              :key="todo.id"
              :todo="todo"
              :folder="folders.get(todo.folderId)"
              @check="(completed) => checkTodo(todo.id, completed)"
              @check-task="(taskId) => checkTask(todo.id, taskId)"
            />
          </template>
        </ul>
      </template>

      <div v-else-if="state === 'no-folder'" class="widget-empty">
        <div>
          <span><LogoMark :size="20" /></span>
          <h3>{{ t("appName") }}</h3>
          <p>{{ t("pickFolderFirst") }}</p>
          <button class="settings-button" @click="openSettings">{{ t("openSettings") }}</button>
        </div>
      </div>

      <div v-else-if="state === 'error'" class="widget-empty">
        <div>
          <span><AppIcon name="alert" :size="21" /></span>
          <h3>{{ t("dataErrorTitle") }}</h3>
          <button class="settings-button secondary" @click="reload">
            <AppIcon name="refresh" :size="13" />{{ t("rescan") }}
          </button>
        </div>
      </div>

      <div v-else class="widget-empty">
        <div>
          <span><AppIcon name="todo" :size="21" /></span>
          <h3>{{ state === "clear" ? t("allCaughtUp") : t("nothingToday") }}</h3>
          <p v-if="state === 'upcoming'">{{ t("upcoming") }} · {{ buckets.upcoming.length }}</p>
        </div>
      </div>

      <footer class="widget-foot">
        <AppIcon name="check" :size="11" />
        <span data-tauri-drag-region>{{ t("autosaveHint") }}</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.widget-section-label {
  height: 30px;
  margin-top: 4px;
  border-bottom: 1px solid rgba(47, 52, 45, .07);
}
html[data-handy-theme="dark"] .widget-section-label { border-bottom-color: rgba(255, 255, 255, .07); }
</style>
