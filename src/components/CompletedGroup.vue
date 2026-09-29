<script setup lang="ts">
// 已完成待办折叠区：列表底部的一条「已完成 N」，默认收起，点开可查看。
// toggleable 模式（Handy 面板）右端带开关，就地切换「显示已完成」——
// 开关关闭时折叠条退化为入口（只显示计数与开关），打开展开后可查看。
import { computed, ref } from "vue";
import type { ActaTodo } from "../types/acta";
import { checkTask, checkTodo, persistSettings, store } from "../lib/store";
import { folderMap } from "../lib/view";
import { t } from "../lib/i18n";
import TodoRow from "./TodoRow.vue";

const props = defineProps<{ todos: ActaTodo[]; toggleable?: boolean }>();

const expanded = ref(false);
const show = computed(() => store.settings.todoWidget.showCompleted);
const folders = computed(() => folderMap(store.data));

function onBarClick(): void {
  if (show.value) expanded.value = !expanded.value;
}

async function setShow(value: boolean): Promise<void> {
  store.settings.todoWidget.showCompleted = value;
  await persistSettings(store.settings);
}
</script>

<template>
  <li class="completed-group">
    <div
      class="completed-bar"
      :class="{ open: show && expanded, clickable: show }"
      role="button"
      :aria-expanded="show ? expanded : undefined"
      @click="onBarClick"
    >
      <svg class="chev" viewBox="0 0 12 12" aria-hidden="true"><path d="M3.5 4.5l2.5 2.5 2.5-2.5" /></svg>
      <span class="bar-copy">{{ t("completedCount")(todos.length) }}</span>
      <button
        v-if="toggleable"
        class="switch"
        role="switch"
        :aria-checked="show"
        :title="t('showCompleted')"
        @click.stop="setShow(!show)"
      />
    </div>
    <template v-if="show && expanded">
      <div class="completed-items">
        <TodoRow
          v-for="todo in props.todos"
          :key="todo.id"
          :todo="todo"
          :folder="folders.get(todo.folderId)"
          @check="(completed) => checkTodo(todo.id, completed)"
          @check-task="(taskId) => checkTask(todo.id, taskId)"
        />
      </div>
    </template>
  </li>
</template>

<style scoped>
.completed-group { list-style: none; }

.completed-bar {
  height: 28px;
  padding: 0 2px;
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--faint);
  font-size: 10.5px;
  font-weight: 650;
  user-select: none;
}
.completed-bar.clickable { cursor: pointer; }
.completed-bar.clickable:hover { color: var(--muted); }

.chev {
  width: 10px;
  height: 10px;
  flex: 0 0 auto;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.4;
  stroke-linecap: round;
  stroke-linejoin: round;
  transition: transform .18s var(--ease-out);
}
.completed-bar.open .chev { transform: rotate(180deg); }

.bar-copy { letter-spacing: .02em; }
.completed-bar .switch { margin-left: auto; }

.completed-items { animation: completed-in .22s var(--ease-out) both; }
@keyframes completed-in {
  from { opacity: 0; transform: translateY(-5px); }
  to { opacity: 1; transform: none; }
}
@media (prefers-reduced-motion: reduce) {
  .completed-items { animation: none; }
  .chev { transition: none; }
}
</style>
