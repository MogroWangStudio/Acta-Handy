import { reactive } from "vue";
import type { ActaData } from "../types/acta";
import type { HandySettings } from "../types/settings";
import {
  loadSettings,
  onSettingsChanged,
  readActaData,
  saveSettings as saveSettingsApi,
} from "./api";
import { applyTheme } from "./theme";

interface StoreState {
  ready: boolean;
  settings: HandySettings;
  data: ActaData | null;
  dataError: string;
  loadingData: boolean;
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
    hud: { enabled: false, x: null, y: null, opacity: 1, alwaysOnTop: true },
  },
  data: null,
  dataError: "",
  loadingData: false,
});

let initPromise: Promise<void> | null = null;

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
  void onSettingsChanged((settings) => {
    const folderChanged = settings.dataFolder !== store.settings.dataFolder;
    store.settings = settings;
    applyTheme(settings.theme);
    if (folderChanged) void refreshActaData();
  });
  store.ready = true;
}

export async function refreshActaData(): Promise<void> {
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
