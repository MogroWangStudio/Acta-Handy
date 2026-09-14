<script setup lang="ts">
// 悬浮窗快速编辑面板：今日待办（可勾选、子任务可展开）+ 最近笔记（可新建、编辑）。
import { computed, ref } from "vue";
import { checkTask, checkTodo, createNote, store } from "../lib/store";
import { bucketTodos, folderMap, recentNotes } from "../lib/view";
import { folderColor, folderName, longDate, markdownToPlain } from "../lib/format";
import { t } from "../lib/i18n";
import TodoRow from "./TodoRow.vue";
import NoteEditor from "./NoteEditor.vue";

const tab = ref<"todos" | "notes">("todos");

const buckets = computed(() => bucketTodos(store.data?.todos ?? []));
const folders = computed(() => folderMap(store.data));
const today = computed(() => longDate(new Date()));
const pct = computed(() => (buckets.value.totalToday ? buckets.value.doneToday / buckets.value.totalToday : 0));

const shown = computed(() =>
  store.settings.todoWidget.showCompleted
    ? [...buckets.value.current, ...buckets.value.completed]
    : buckets.value.current,
);

const notes = computed(() => recentNotes(store.data?.notes ?? [], 30));
const snippets = computed(
  () => new Map(notes.value.map((note) => [note.id, markdownToPlain(note.bodyMarkdown)])),
);
const editingId = ref<string | null>(null);
const editingNote = computed(() => notes.value.find((n) => n.id === editingId.value) ?? null);

const RADIUS = 13;
const CIRCUM = 2 * Math.PI * RADIUS;

async function newNote(): Promise<void> {
  const saved = await createNote("", "");
  if (saved) {
    editingId.value = saved.id;
    tab.value = "notes";
  }
}
</script>

<template>
  <div class="panel-root">
    <header class="panel-head" data-tauri-drag-region>
      <svg class="panel-ring" viewBox="0 0 34 34" aria-hidden="true">
        <circle class="ring-track" cx="17" cy="17" :r="RADIUS" />
        <circle class="ring-fill" cx="17" cy="17" :r="RADIUS" :stroke-dasharray="`${pct * CIRCUM} ${CIRCUM}`" />
        <text x="17" y="20.5" text-anchor="middle">{{ t("doneCount")(buckets.doneToday, buckets.totalToday) }}</text>
      </svg>
      <div class="panel-head-copy" data-tauri-drag-region>
        <p class="panel-title" data-tauri-drag-region>{{ t("panelQuickEdit") }}</p>
        <p class="panel-date" data-tauri-drag-region>{{ today }}</p>
      </div>
    </header>

    <nav class="panel-tabs" role="tablist">
      <button role="tab" :aria-selected="tab === 'todos'" :class="{ active: tab === 'todos' }" @click="tab = 'todos'">
        {{ t("panelTodos") }}<i>{{ shown.length }}</i>
      </button>
      <button role="tab" :aria-selected="tab === 'notes'" :class="{ active: tab === 'notes' }" @click="tab = 'notes'">
        {{ t("panelNotes") }}<i>{{ notes.length }}</i>
      </button>
    </nav>

    <div v-if="tab === 'todos'" class="panel-body">
      <p v-if="!store.settings.dataFolder" class="panel-hint">{{ t("panelNoFolder") }}</p>
      <ul v-else-if="shown.length > 0" class="panel-list">
        <TodoRow
          v-for="todo in shown"
          :key="todo.id"
          :todo="todo"
          :folder="folders.get(todo.folderId)"
          @check="(completed) => checkTodo(todo.id, completed)"
          @check-task="(taskId) => checkTask(todo.id, taskId)"
        />
      </ul>
      <p v-else class="panel-hint">{{ buckets.totalToday > 0 ? t("hudAllDone") : t("hudNothing") }}</p>
    </div>

    <div v-else class="panel-body">
      <template v-if="editingId && editingNote">
        <button class="panel-back" @click="editingId = null">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M14.5 6l-6 6 6 6" />
          </svg>
          {{ t("backToNotes") }}
        </button>
        <NoteEditor :key="editingNote.id" :note="editingNote" class="panel-editor" />
      </template>
      <template v-else>
        <button class="panel-new" :disabled="!store.settings.dataFolder" @click="newNote">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
            <path d="M12 5.5v13M5.5 12h13" />
          </svg>
          {{ t("newNote") }}
        </button>
        <p v-if="!store.settings.dataFolder" class="panel-hint">{{ t("panelNoFolder") }}</p>
        <ul v-else-if="notes.length > 0" class="panel-list notes">
          <li
            v-for="note in notes"
            :key="note.id"
            class="panel-note"
            role="button"
            tabindex="0"
            @click="editingId = note.id"
            @keydown.enter="editingId = note.id"
          >
            <h4>{{ note.title || t("untitledNote") }}</h4>
            <p v-if="snippets.get(note.id)">{{ snippets.get(note.id) }}</p>
            <div class="panel-note-meta">
              <i :style="{ color: folderColor(folders.get(note.folderId)), background: folderColor(folders.get(note.folderId)) }" />
              <span>{{ folderName(folders.get(note.folderId)) }}</span>
              <time>{{ (note.updatedAt || "").slice(5, 10) }}</time>
            </div>
          </li>
        </ul>
        <p v-else class="panel-hint">{{ t("noNotes") }}</p>
      </template>
    </div>
  </div>
</template>

<style scoped>
.panel-root {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel-head {
  flex: 0 0 auto;
  padding: 12px 16px 8px;
  display: flex;
  align-items: center;
  gap: 11px;
}
.panel-ring { width: 34px; height: 34px; flex: 0 0 auto; }
.ring-track, .ring-fill {
  fill: none;
  stroke-width: 3.4;
  stroke-linecap: round;
  transform: rotate(-90deg);
  transform-origin: 50% 50%;
}
.ring-track { stroke: color-mix(in srgb, var(--muted) 22%, transparent); }
.ring-fill { stroke: var(--sage); transition: stroke-dasharray .6s var(--ease-out); }
.panel-ring text { fill: var(--ink); font-size: 8px; font-weight: 700; letter-spacing: .02em; }
.panel-head-copy { min-width: 0; }
.panel-title { margin: 0; color: var(--ink); font-size: 12.5px; font-weight: 650; line-height: 1.3; }
.panel-date { margin: 1px 0 0; color: var(--faint); font-size: 8.5px; letter-spacing: .05em; text-transform: uppercase; font-weight: 700; }

.panel-tabs {
  flex: 0 0 auto;
  margin: 2px 16px 6px;
  padding: 3px;
  border-radius: 10px;
  background: var(--panel);
  display: flex;
  gap: 3px;
}
.panel-tabs button {
  flex: 1;
  height: 26px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 650;
  cursor: pointer;
  transition: background .18s ease, color .18s ease;
}
.panel-tabs button.active { background: var(--white); color: var(--ink); box-shadow: 0 1px 3px rgba(47, 50, 43, .08); }
.panel-tabs i {
  min-width: 18px;
  height: 15px;
  padding: 0 4px;
  border-radius: 6px;
  background: var(--sage-2);
  color: var(--sage);
  display: inline-grid;
  place-items: center;
  font-style: normal;
  font-size: 8.5px;
  font-weight: 700;
}
html[data-handy-theme="dark"] .panel-tabs button.active { background: var(--sage-2); }

.panel-body {
  flex: 1;
  min-height: 0;
  overflow: hidden auto;
  padding: 2px 16px 12px;
  display: flex;
  flex-direction: column;
}
.panel-body::-webkit-scrollbar { width: 9px; }

.panel-hint {
  margin: 18px 4px;
  color: var(--faint);
  font-size: 10.5px;
  line-height: 1.6;
  text-align: center;
}

.panel-list { margin: 0; padding: 0; list-style: none; }
.panel-list.notes { display: flex; flex-direction: column; }

.panel-back {
  align-self: flex-start;
  margin: 2px 0 2px -4px;
  height: 24px;
  padding: 0 8px 0 3px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  display: inline-flex;
  align-items: center;
  gap: 2px;
  font-size: 10.5px;
  font-weight: 600;
  cursor: pointer;
  transition: background .15s ease, color .15s ease;
}
.panel-back:hover { background: var(--panel); color: var(--ink); }
.panel-back svg { width: 12px; height: 12px; }

.panel-editor { flex: 1; min-height: 0; }

.panel-new {
  align-self: flex-end;
  margin: 4px 0 6px;
  height: 26px;
  padding: 0 10px;
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
.panel-new:hover { background: color-mix(in srgb, var(--sage-2) 70%, var(--sage) 12%); }
.panel-new:active { transform: scale(.96); }
.panel-new:disabled { opacity: .4; pointer-events: none; }
.panel-new svg { width: 11px; height: 11px; }
html[data-handy-theme="dark"] .panel-new { color: var(--ink); }

.panel-note {
  padding: 9px 2px;
  border-bottom: 1px solid rgba(47, 52, 45, .07);
  cursor: pointer;
  border-radius: 8px;
  transition: background .15s ease;
}
.panel-note:hover { background: color-mix(in srgb, var(--sage-2) 45%, transparent); }
.panel-note:last-child { border-bottom: 0; }
html[data-handy-theme="dark"] .panel-note { border-bottom-color: rgba(255, 255, 255, .07); }
.panel-note h4 {
  margin: 0 0 3px;
  font: 600 12px/1.35 var(--font-display);
  color: var(--ink);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.panel-note p {
  margin: 0;
  color: var(--muted);
  font-size: 10px;
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.panel-note-meta {
  margin-top: 5px;
  display: flex;
  align-items: center;
  gap: 5px;
  color: var(--faint);
  font-size: 8.5px;
}
.panel-note-meta i { width: 6px; height: 6px; border-radius: 50%; }
.panel-note-meta time { margin-left: auto; }
</style>
