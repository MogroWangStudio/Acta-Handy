<script setup lang="ts">
import { onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import SettingsWindow from "./windows/SettingsWindow.vue";
import TodoWidget from "./windows/TodoWidget.vue";
import NotesWidget from "./windows/NotesWidget.vue";
import HudWindow from "./windows/HudWindow.vue";

const label = getCurrentWindow().label;
const ready = ref(false);

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
