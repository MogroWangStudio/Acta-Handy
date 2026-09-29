import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { ActaData, ActaNote, ActaTodo } from "../types/acta";
import type { HandySettings } from "../types/settings";
import type { HistoryEntry } from "../types/history";

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

/** 某个数据文件夹最近的修改历史（新→旧），在设置窗口的「数据源」页回溯。 */
export function readHistory(folder: string): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>("read_history", { folder });
}

/** 把一条历史恢复回写入前的模样；恢复本身也会记入历史。 */
export function restoreHistory(folder: string, entryId: string): Promise<void> {
  return invoke("restore_history", { folder, entryId });
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

/** Handy 面板停靠信息：side 是 Handy 站在窗口的哪一侧（也是贴边方向），
    lift 是面板形态下 Handy 脚底离窗口底部的距离（面板向上展开被屏幕
    上缘截断时变大，Handy 的屏幕位置因此保持不动）。 */
export interface HudPlacement {
  side: "left" | "right";
  lift: number;
}

/** 切换 Handy 窗口形态（自由站立 / 贴边探头）；返回停靠信息，自由形态为 null。 */
export function setHudMode(mode: "free" | "peek"): Promise<HudPlacement | null> {
  return invoke<HudPlacement | null>("set_hud_mode", { mode });
}

/** 展开快速编辑面板 / 收起面板与菜单（独立的 hud-panel 窗口，Handy 原地不动）。 */
export function setHudPanel(shown: boolean): Promise<void> {
  return invoke("set_hud_panel", { shown });
}

/** 弹出 Handy 的右键菜单（应用内自绘：大小滑块 + 关闭）。 */
export function popupHudMenu(): Promise<void> {
  return invoke("popup_hud_menu");
}

/** 滑块拖动中的实时缩放：窗口尺寸立即跟随，落盘在 commitHudScale 时完成。 */
export function setHudScale(scale: number): Promise<HudPlacement | null> {
  return invoke<HudPlacement | null>("set_hud_scale", { scale });
}

/** 滑块松手：把实时缩放落盘并广播。 */
export function commitHudScale(): Promise<void> {
  return invoke("commit_hud_scale");
}

/** 缩放滑块拖动中的实时值（0.2–1.5）：HUD 窗口用它同步预览大小。 */
export function onHudScale(cb: (scale: number) => void): Promise<void> {
  return listen<number>("hud-scale", (e) => cb(e.payload)).then(() => undefined);
}

/** 右键菜单里的「关闭 Handy」；重新开启后以吸附 / 自由形态回来。 */
export function setHudEnabled(enabled: boolean): Promise<void> {
  return invoke("set_hud_enabled", { enabled });
}

/** 开启眼睛跟随：后端线程轮询光标相对 Handy 的方向并广播 hud-gaze。 */
export function setHudEyeWatch(watch: boolean): Promise<void> {
  return invoke("set_hud_eye_watch", { watch });
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

/** 贴边过渡动画进度：start 开始滑行（side 为目标贴边方向）、reveal 切换
    探头/站姿、end 结束。动画期间后端会挡下设置应用，前端也应暂停形态交互。 */
export function onHudAnim(
  cb: (payload: { phase: "start" | "reveal" | "end"; to?: "peek" | "free"; side?: "left" | "right" }) => void,
): Promise<void> {
  return listen<{
    phase: "start" | "reveal" | "end";
    to?: "peek" | "free";
    side?: "left" | "right";
  }>("hud-anim", (e) => cb(e.payload)).then(() => undefined);
}

/** 光标相对 Handy 的方向（-1..1 归一化），驱动眼睛微微跟随。 */
export function onHudGaze(cb: (gaze: { nx: number; ny: number }) => void): Promise<void> {
  return listen<{ nx: number; ny: number }>("hud-gaze", (e) => cb(e.payload)).then(() => undefined);
}

/** 面板 / 菜单窗口的开合广播：kind 为 panel（快速编辑）或 menu（右键菜单），
    side 为 Handy 站在卡片的哪一侧（卡片朝另一侧展开）。 */
export function onHudPanel(
  cb: (payload: { shown: boolean; kind: "panel" | "menu"; side: "left" | "right" }) => void,
): Promise<void> {
  return listen<{ shown: boolean; kind: "panel" | "menu"; side: "left" | "right" }>("hud-panel", (e) => cb(e.payload)).then(() => undefined);
}

/** 光标进入 Handy 窗口的通知：面板用它取消「光标离开就收起」的计时，
    让光标在两个窗口之间移动时不收起面板。 */
export function emitHudPanelKeep(): void {
  void emit("hud-panel-keep");
}

/** 光标离开 Handy 窗口的通知：面板据此启动收起计时——光标若正移向面板，
    面板的 pointerenter 会立刻取消计时，跨过窗口缝隙不误收。 */
export function emitHudPanelAway(): void {
  void emit("hud-panel-away");
}
