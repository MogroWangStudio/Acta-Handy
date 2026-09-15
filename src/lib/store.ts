import { reactive } from "vue";
import type { ActaData, ActaNote, ActaTodo } from "../types/acta";
import type { HandySettings } from "../types/settings";
import {
  loadSettings,
  onDataChanged,
  onSettingsChanged,
  readActaData,
  saveSettings as saveSettingsApi,
  writeNote,
  writeTodoCheck,
} from "./api";
import { applyTheme } from "./theme";

interface StoreState {
  ready: boolean;
  settings: HandySettings;
  data: ActaData | null;
  dataError: string;
  loadingData: boolean;
  /** An editor holds unsaved text; auto-refresh waits until it's saved. */
  editing: number;
  pendingRefresh: boolean;
}

export const store = reactive<StoreState>({
  ready: false,
  settings: {
    version: 1,
    dataFolder: "",
    theme: "auto",
    language: "zh",
    refreshIntervalSecs: 30,
    todoWidget: { enabled: false, x: null, y: null, width: 300, height: 360, opacity: 1, alwaysOnTop: false, showCompleted: false },
    notesWidget: { enabled: false, x: null, y: null, width: 300, height: 380, opacity: 1, alwaysOnTop: false, showCompleted: false },
    hud: { enabled: false, x: null, y: null, opacity: 1, alwaysOnTop: true, scale: 1, snapToEdge: false, stealth: false, stealthDelaySecs: 15 },
  },
  data: null,
  dataError: "",
  loadingData: false,
  editing: 0,
  pendingRefresh: false,
});

let initPromise: Promise<void> | null = null;
/** Writes touch the files the watcher watches; swallow the echo it triggers. */
let lastWriteAt = 0;

function markWrote(): void {
  lastWriteAt = Date.now();
}

function wroteRecently(): boolean {
  return Date.now() - lastWriteAt < 2500;
}

/** Enter/leave edit sessions; auto-refresh is deferred while editing. */
export function beginEdit(): void {
  store.editing += 1;
}

export function endEdit(): void {
  store.editing = Math.max(0, store.editing - 1);
  if (store.editing === 0 && store.pendingRefresh) {
    store.pendingRefresh = false;
    void refreshActaData();
  }
}

export function initStore(): Promise<void> {
  initPromise ??= doInit();
  return initPromise;
}

async function doInit(): Promise<void> {
  try {
    store.settings = await loadSettings();
  } catch {
    // Keep placeholder defaults; the settings window still renders.
  }
  applyTheme(store.settings.theme);
  await refreshActaData();
  void onDataChanged(() => {
    if (wroteRecently()) return; // echo of our own write
    void refreshActaData();
  });
  void onSettingsChanged((settings) => {
    const folderChanged = settings.dataFolder !== store.settings.dataFolder;
    store.settings = settings;
    applyTheme(settings.theme);
    if (folderChanged) void refreshActaData();
  });
  store.ready = true;
}

export async function refreshActaData(force = false): Promise<void> {
  if (!force && store.editing > 0) {
    store.pendingRefresh = true;
    return;
  }
  const folder = store.settings.dataFolder;
  if (!folder) {
    store.data = null;
    store.dataError = "";
    return;
  }
  store.loadingData = true;
  try {
    store.data = await readActaData(folder);
    store.dataError = "";
  } catch (error) {
    store.data = null;
    store.dataError = String(error);
  } finally {
    store.loadingData = false;
  }
}

/** Push settings to the backend; every window (incl. this one) reloads via the event. */
export async function persistSettings(settings: HandySettings): Promise<void> {
  store.settings = await saveSettingsApi(settings);
  applyTheme(store.settings.theme);
}

// --- writes -----------------------------------------------------------------
//
// All writes follow Acta's completion rules so the folder stays exactly like
// Acta itself would have saved it, and the local store updates eagerly (the
// disk write happens behind it; failures roll back with a full re-read).

function localTodo(todoId: string): ActaTodo | undefined {
  return store.data?.todos.find((t) => t.id === todoId);
}

function localNote(noteId: string): ActaNote | undefined {
  return store.data?.notes.find((n) => n.id === noteId);
}

/** Acta's setTodoCompletion: flipping a todo flips every subtask with it. */
export async function checkTodo(todoId: string, completed: boolean): Promise<void> {
  const todo = localTodo(todoId);
  const folder = store.settings.dataFolder;
  if (!todo || !folder) return;
  const tasks = todo.tasks.map((t) => ({ id: t.id, done: completed }));
  todo.completed = completed;
  todo.tasks.forEach((t, i) => {
    t.done = tasks[i].done;
  });
  todo.updatedAt = new Date().toISOString();
  markWrote();
  try {
    const saved = await writeTodoCheck(folder, { todoId, completed, tasks });
    const local = localTodo(todoId);
    if (local) Object.assign(local, saved);
  } catch {
    await refreshActaData(true);
  }
}

/** Acta's subtask toggle: recompute `completed` as "all subtasks done". */
export async function checkTask(todoId: string, taskId: string): Promise<void> {
  const todo = localTodo(todoId);
  const folder = store.settings.dataFolder;
  if (!todo || !folder) return;
  const tasks = todo.tasks.map((t) =>
    t.id === taskId ? { id: t.id, done: !t.done } : { id: t.id, done: t.done },
  );
  const completed = tasks.length > 0 && tasks.every((t) => t.done);
  const task = todo.tasks.find((t) => t.id === taskId);
  if (task) task.done = !task.done;
  todo.completed = completed;
  todo.updatedAt = new Date().toISOString();
  markWrote();
  try {
    const saved = await writeTodoCheck(folder, { todoId, completed, tasks });
    const local = localTodo(todoId);
    if (local) Object.assign(local, saved);
  } catch {
    await refreshActaData(true);
  }
}

/** Debounced-save target for note editors; returns the saved note. */
export async function saveNote(
  noteId: string,
  title: string,
  bodyMarkdown: string,
): Promise<ActaNote | null> {
  const folder = store.settings.dataFolder;
  if (!folder) return null;
  markWrote();
  const saved = await writeNote(folder, { noteId, title, bodyMarkdown });
  const local = localNote(noteId);
  if (local) {
    local.title = saved.title;
    local.bodyMarkdown = saved.bodyMarkdown;
    local.updatedAt = saved.updatedAt;
  }
  return saved;
}

export async function createNote(title: string, bodyMarkdown: string): Promise<ActaNote | null> {
  const folder = store.settings.dataFolder;
  if (!folder) return null;
  markWrote();
  const saved = await writeNote(folder, { noteId: null, title, bodyMarkdown, folderId: "" });
  store.data?.notes.unshift(saved);
  return saved;
}
