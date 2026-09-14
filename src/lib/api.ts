import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { ActaData } from "../types/acta";
import type { HandySettings } from "../types/settings";

export function readActaData(folder: string): Promise<ActaData> {
  return invoke<ActaData>("read_acta_data", { folder });
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
