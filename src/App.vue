<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { t } from "./lib/i18n";
import { store } from "./lib/store";
import SettingsWindow from "./windows/SettingsWindow.vue";
import TodoWidget from "./windows/TodoWidget.vue";
import NotesWidget from "./windows/NotesWidget.vue";
import HudWindow from "./windows/HudWindow.vue";

const label = getCurrentWindow().label;
const ready = ref(false);

// 浏览器调试各窗口时靠 document.title 区分；Tauri 桌面端窗口标题仍由
// tauri.conf.json 提供，不受它影响。
const titleKey =
  label === "main" ? "titleMain"
  : label === "todo-widget" ? "titleTodoWidget"
  : label === "notes-widget" ? "titleNotesWidget"
  : label === "hud" ? "titleHud"
  : null;

function applyTitle(): void {
  if (titleKey) document.title = t(titleKey);
}
watch(() => store.settings?.language, applyTitle);
applyTitle();

onMounted(() => {
  // Widgets may reveal before the store finishes loading; a short hold keeps
  // them from flashing an empty card.
  setTimeout(() => {
    ready.value = true;
  }, 120);
});
</script>

<template>
  <SettingsWindow v-if="label === 'main'" />
  <template v-else-if="ready && label === 'todo-widget'">
    <TodoWidget />
  </template>
  <template v-else-if="ready && label === 'notes-widget'">
    <NotesWidget />
  </template>
  <template v-else-if="ready && label === 'hud'">
    <HudWindow />
  </template>
</template>
