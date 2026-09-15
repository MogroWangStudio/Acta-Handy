import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { ActaData, ActaNote, ActaTodo } from "../types/acta";
import type { HandySettings } from "../types/settings";

export function readActaData(folder: string): Promise<ActaData> {
  return invoke<ActaData>("read_acta_data", { folder });
}

export interface TodoCheckPatch {
  todoId: string;
  completed: boolean;
  tasks: Array<{ id: string; done: boolean }>;
}

/** 勾选待办 / 子待办，写回 Acta 数据文件夹并返回落盘后的待办。 */
export function writeTodoCheck(folder: string, patch: TodoCheckPatch): Promise<ActaTodo> {
  return invoke<ActaTodo>("write_todo_check", { folder, patch });
}

export interface NotePatch {
  noteId?: string | null;
  title?: string | null;
  bodyMarkdown?: string | null;
  folderId?: string | null;
}

/** 新建 / 编辑笔记，写回 Acta 数据文件夹并返回落盘后的笔记。 */
export function writeNote(folder: string, patch: NotePatch): Promise<ActaNote> {
  return invoke<ActaNote>("write_note", { folder, patch });
}

export function loadSettings(): Promise<HandySettings> {
  return invoke<HandySettings>("load_settings");
}

export function saveSettings(settings: HandySettings): Promise<HandySettings> {
  return invoke<HandySettings>("save_settings", { settings });
}

export function refreshData(): Promise<void> {
  return invoke("refresh_data");
}

export function showWindow(label: string): Promise<void> {
  return invoke("show_window", { label });
}

export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** 切换悬浮窗形态；返回贴在哪一侧（"left" / "right"），自由形态为 null。 */
export function setHudMode(mode: "free" | "peek" | "panel"): Promise<"left" | "right" | null> {
  return invoke<"left" | "right" | null>("set_hud_mode", { mode });
}

/** 隐匿淡出后开启光标监控；鼠标靠近窗口 pad 像素内时后端会广播 hud-wake。
    探头形态用更大的 pad，让 Handy 在光标靠近时就主动跳出。 */
export function setHudCursorWatch(watch: boolean, pad = 26): Promise<void> {
  return invoke("set_hud_cursor_watch", { watch, pad });
}

export function pickDataFolder(): Promise<string | null> {
  return open({
    directory: true,
    multiple: false,
    title: "选择 Acta 数据文件夹",
  }) as Promise<string | null>;
}

export function onDataChanged(cb: () => void): Promise<void> {
  return listen("acta-data-changed", () => cb()).then(() => undefined);
}

export function onSettingsChanged(cb: (settings: HandySettings) => void): Promise<void> {
  return listen<HandySettings>("settings-changed", (e) => cb(e.payload)).then(() => undefined);
}

export function onHudWake(cb: () => void): Promise<void> {
  return listen("hud-wake", () => cb()).then(() => undefined);
}
