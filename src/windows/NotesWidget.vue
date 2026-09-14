<script setup lang="ts">
import { computed, onMounted } from "vue";
import { initStore, store } from "../lib/store";
import { folderMap, recentNotes } from "../lib/view";
import { folderColor, folderName, markdownToPlain, shortDate } from "../lib/format";
import { t } from "../lib/i18n";
import { showWindow } from "../lib/api";
import AppIcon from "../components/AppIcon.vue";
import LogoMark from "../components/LogoMark.vue";

onMounted(initStore);

const cfg = computed(() => store.settings.notesWidget);
const notes = computed(() => recentNotes(store.data?.notes ?? []));
const snippets = computed(
  () => new Map(notes.value.map((note) => [note.id, markdownToPlain(note.bodyMarkdown)])),
);
const folders = computed(() => folderMap(store.data));
const state = computed<"no-folder" | "error" | "empty" | "list">(() => {
  if (!store.settings.dataFolder) return "no-folder";
  if (store.dataError) return "error";
  return notes.value.length > 0 ? "list" : "empty";
});

async function openSettings(): Promise<void> {
  await showWindow("main");
}
</script>

<template>
  <div class="widget-root">
    <div class="widget-card" :style="{ opacity: cfg.opacity }">
      <header class="widget-head" data-tauri-drag-region>
        <div>
          <h1 class="widget-title" data-tauri-drag-region>{{ t("recentNotes") }}</h1>
        </div>
        <span class="widget-count">{{ notes.length }}</span>
      </header>

      <template v-if="state === 'list'">
        <ul class="widget-list notes-list">
          <li v-for="note in notes" :key="note.id" class="note-row">
            <h3 class="note-title">{{ note.title || t("untitledNote") }}</h3>
            <p v-if="snippets.get(note.id)" class="note-snippet">
              {{ snippets.get(note.id) }}
            </p>
            <div class="note-meta">
              <i class="folder-dot" :style="{ color: folderColor(folders.get(note.folderId)), background: folderColor(folders.get(note.folderId)) }" />
              <span>{{ folderName(folders.get(note.folderId)) }}</span>
              <time>{{ shortDate(note.updatedAt) }}</time>
            </div>
          </li>
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
        </div>
      </div>

      <div v-else class="widget-empty">
        <div>
          <span><AppIcon name="note" :size="21" /></span>
          <h3>{{ t("noNotes") }}</h3>
          <p>{{ t("noNotesHint") }}</p>
        </div>
      </div>

      <footer class="widget-foot">
        <AppIcon name="info" :size="11" />
        <span data-tauri-drag-region>{{ t("readonlyHint") }}</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.notes-list { display: flex; flex-direction: column; }

.note-row {
  padding: 11px 1px 12px;
  border-bottom: 1px solid rgba(47, 52, 45, .07);
  animation: noteIn .4s var(--ease-out) both;
}
.note-row:last-child { border-bottom: 0; }
html[data-handy-theme="dark"] .note-row { border-bottom-color: rgba(255, 255, 255, .07); }

.note-title {
  margin: 0 0 4px;
  font: 600 13px/1.35 var(--font-display);
  letter-spacing: -.01em;
  color: var(--ink);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.note-snippet {
  margin: 0;
  color: var(--muted);
  font-size: 10.5px;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.note-meta {
  margin-top: 7px;
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--faint);
  font-size: 8.5px;
  letter-spacing: .04em;
}
.note-meta time { margin-left: auto; }

@keyframes noteIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}
</style>
