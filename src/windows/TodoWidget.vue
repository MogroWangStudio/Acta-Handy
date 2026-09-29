<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import type { ActaTodo } from "../types/acta";
import { checkTask, checkTodo, initStore, refreshActaData, store } from "../lib/store";
import { bucketTodos, folderMap } from "../lib/view";
import { longDate } from "../lib/format";
import { t } from "../lib/i18n";
import { refreshData, showWindow } from "../lib/api";
import { useSnapHint } from "../lib/snapHint";
import AppIcon from "../components/AppIcon.vue";
import FilterChips from "../components/FilterChips.vue";
import CompletedGroup from "../components/CompletedGroup.vue";
import LogoMark from "../components/LogoMark.vue";
import TodoRow from "../components/TodoRow.vue";

onMounted(initStore);

// 拖动中的吸附提示光效：靠近屏幕边缘或笔记小组件时亮起（松手即按此贴合）。
const snapSide = useSnapHint(
  () => store.settings.todoWidget,
  () => (store.settings.notesWidget.enabled ? store.settings.notesWidget : null),
);

const cfg = computed(() => store.settings.todoWidget);
const buckets = computed(() => bucketTodos(store.data?.todos ?? []));
const folders = computed(() => folderMap(store.data));
const today = computed(() => longDate(new Date()));

// 归类筛选：null = 全部；"" = 未归类；否则为文件夹 id。
const filter = ref<string | null>(null);
const folderList = computed(() => store.data?.folders ?? []);
const unfiledCount = computed(
  () => (store.data?.todos ?? []).filter((todo) => !todo.deletedAt && !folders.value.has(todo.folderId)).length,
);
const hasChips = computed(() => folderList.value.length > 0 || unfiledCount.value > 0);

function inFilter(todo: ActaTodo): boolean {
  if (filter.value === null) return true;
  const id = folders.value.has(todo.folderId) ? todo.folderId : "";
  return id === filter.value;
}

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

const shown = computed(() => buckets.value.current.filter(inFilter));
const shownCompleted = computed(() =>
  cfg.value.showCompleted ? completedToday.value.filter(inFilter) : [],
);
const shownUpcoming = computed(() => buckets.value.upcoming.filter(inFilter));
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
    <div class="widget-card" :data-snap="snapSide" :style="{ opacity: cfg.opacity }">
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

      <FilterChips
        v-if="state === 'list' && hasChips"
        v-model="filter"
        :folders="folderList"
        :unfiled="unfiledCount"
      />

      <template v-if="state === 'list'">
        <ul class="widget-list">
          <template v-if="shown.length > 0">
            <TodoRow
              v-for="todo in shown"
              :key="todo.id"
              :todo="todo"
              :folder="folders.get(todo.folderId)"
              @check="(completed) => checkTodo(todo.id, completed)"
              @check-task="(taskId) => checkTask(todo.id, taskId)"
            />
          </template>
          <li v-else class="widget-nofilter">{{ t("nothingInFolder") }}</li>
          <template v-if="shownUpcoming.length > 0">
            <li class="widget-section-label section-label">{{ t("upcoming") }}</li>
            <TodoRow
              v-for="todo in shownUpcoming.slice(0, 3)"
              :key="todo.id"
              :todo="todo"
              :folder="folders.get(todo.folderId)"
              @check="(completed) => checkTodo(todo.id, completed)"
              @check-task="(taskId) => checkTask(todo.id, taskId)"
            />
          </template>
          <!-- 已完成沉底：默认折叠的一条「已完成 N」，点开查看 -->
          <CompletedGroup v-if="shownCompleted.length > 0" :todos="shownCompleted" />
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

.widget-nofilter {
  padding: 14px 2px;
  color: var(--faint);
  font-size: 10.5px;
}
</style>
