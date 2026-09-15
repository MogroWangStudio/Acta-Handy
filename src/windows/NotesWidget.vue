<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { createNote, initStore, store } from "../lib/store";
import { folderMap, recentNotes } from "../lib/view";
import { folderColor, folderName, markdownToPlain, shortDate } from "../lib/format";
import { t } from "../lib/i18n";
import { showWindow } from "../lib/api";
import AppIcon from "../components/AppIcon.vue";
import FilterChips from "../components/FilterChips.vue";
import LogoMark from "../components/LogoMark.vue";
import NoteEditor from "../components/NoteEditor.vue";

onMounted(initStore);

const cfg = computed(() => store.settings.notesWidget);
const notes = computed(() => recentNotes(store.data?.notes ?? []));
const snippets = computed(
  () => new Map(notes.value.map((note) => [note.id, markdownToPlain(note.bodyMarkdown)])),
);
const folders = computed(() => folderMap(store.data));

// 归类筛选：null = 全部；"" = 未归类；否则为文件夹 id。
const filter = ref<string | null>(null);
const folderList = computed(() => store.data?.folders ?? []);
const unfiledCount = computed(
  () => notes.value.filter((note) => !folders.value.has(note.folderId)).length,
);
const hasChips = computed(() => folderList.value.length > 0 || unfiledCount.value > 0);
const shownNotes = computed(() =>
  filter.value === null
    ? notes.value
    : notes.value.filter(
        (note) => (folders.value.has(note.folderId) ? note.folderId : "") === filter.value,
      ),
);

/** 正在编辑的笔记；null 时显示列表。用 id + 数据里的对象，刷新后仍指向新对象。 */
const editingId = ref<string | null>(null);
const editingNote = computed(() => notes.value.find((n) => n.id === editingId.value) ?? null);

const state = computed<"no-folder" | "error" | "empty" | "list" | "editor">(() => {
  if (!store.settings.dataFolder) return "no-folder";
  if (store.dataError) return "error";
  if (editingId.value) return "editor";
  return notes.value.length > 0 ? "list" : "empty";
});

function openNote(id: string): void {
  editingId.value = id;
}
function backToList(): void {
  editingId.value = null;
}
async function newNote(): Promise<void> {
  const saved = await createNote("", "");
  if (saved) editingId.value = saved.id;
}

async function openSettings(): Promise<void> {
  await showWindow("main");
}
</script>

<template>
  <div class="widget-root">
    <div class="widget-card" :style="{ opacity: cfg.opacity }">
      <header v-if="state !== 'editor'" class="widget-head" data-tauri-drag-region>
        <div>
          <h1 class="widget-title" data-tauri-drag-region>{{ t("recentNotes") }}</h1>
        </div>
        <button class="new-note" :disabled="!store.settings.dataFolder || Boolean(store.dataError)" @click="newNote">
          <AppIcon name="note" :size="12" />{{ t("newNote") }}
        </button>
      </header>

      <FilterChips
        v-if="state === 'list' && hasChips"
        v-model="filter"
        :folders="folderList"
        :unfiled="unfiledCount"
      />

      <template v-if="state === 'list'">
        <ul v-if="shownNotes.length > 0" class="widget-list notes-list">
          <li
            v-for="note in shownNotes"
            :key="note.id"
            class="note-row"
            role="button"
            tabindex="0"
            @click="openNote(note.id)"
            @keydown.enter="openNote(note.id)"
          >
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
        <div v-else class="widget-empty">
          <div>
            <span><AppIcon name="note" :size="21" /></span>
            <h3>{{ t("nothingInFolder") }}</h3>
          </div>
        </div>
      </template>

      <template v-else-if="state === 'editor'">
        <header class="widget-head editor-bar">
          <button class="back-button" @click="backToList">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M14.5 6l-6 6 6 6" />
            </svg>
            {{ t("backToNotes") }}
          </button>
        </header>
        <NoteEditor v-if="editingNote" :key="editingNote.id" :note="editingNote" />
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
          <button class="settings-button" :disabled="!store.settings.dataFolder" @click="newNote">
            <AppIcon name="note" :size="12" />{{ t("newNote") }}
          </button>
        </div>
      </div>

      <footer v-if="state !== 'editor'" class="widget-foot">
        <AppIcon name="check" :size="11" />
        <span data-tauri-drag-region>{{ t("autosaveHint") }}</span>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.new-note {
  margin-left: auto;
  height: 26px;
  padding: 0 9px;
  border: 0;
  border-radius: 9px;
  background: var(--sage-2);
  color: var(--sage);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10px;
  font-weight: 700;
  cursor: pointer;
  transition: background .15s ease, transform .1s ease-out;
}
.new-note:hover { background: color-mix(in srgb, var(--sage-2) 70%, var(--sage) 12%); }
.new-note:active { transform: scale(.96); }
.new-note:disabled { opacity: .4; pointer-events: none; }
html[data-handy-theme="dark"] .new-note { color: var(--ink); }

.editor-bar { padding-bottom: 0; }
.back-button {
  height: 26px;
  padding: 0 8px 0 4px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  display: inline-flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: background .15s ease, color .15s ease;
}
.back-button:hover { background: var(--panel); color: var(--ink); }
.back-button svg { width: 13px; height: 13px; }

.notes-list { display: flex; flex-direction: column; }

.note-row {
  padding: 11px 1px 12px;
  border-bottom: 1px solid rgba(47, 52, 45, .07);
  cursor: pointer;
  border-radius: 8px;
  transition: background .15s ease;
  animation: noteIn .4s var(--ease-out) both;
}
.note-row:hover { background: color-mix(in srgb, var(--sage-2) 45%, transparent); }
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
