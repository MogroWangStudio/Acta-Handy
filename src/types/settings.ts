/** Handy's own settings, mirrored from src-tauri/src/settings.rs. */

export type ThemeChoice = "auto" | "light" | "dark";
export type LanguageChoice = "zh" | "en";

export interface WidgetConfig {
  enabled: boolean;
  x: number | null;
  y: number | null;
  width: number;
  height: number;
  opacity: number;
  alwaysOnTop: boolean;
  showCompleted: boolean;
}

export interface HudConfig {
  enabled: boolean;
  x: number | null;
  y: number | null;
  opacity: number;
  alwaysOnTop: boolean;
  /** 药丸模式：吸附到最近的屏幕边缘，鼠标移过时展开快速编辑面板。 */
  snapToEdge: boolean;
  /** 隐匿模式：超过设定延迟后淡出，鼠标靠近时再淡入。 */
  stealth: boolean;
  stealthDelaySecs: number;
}

export interface HandySettings {
  version: number;
  dataFolder: string;
  theme: ThemeChoice;
  language: LanguageChoice;
  refreshIntervalSecs: number;
  todoWidget: WidgetConfig;
  notesWidget: WidgetConfig;
  hud: HudConfig;
}

export const DEFAULT_SETTINGS: HandySettings = {
  version: 1,
  dataFolder: "",
  theme: "auto",
  language: "zh",
  refreshIntervalSecs: 30,
  todoWidget: {
    enabled: true,
    x: null,
    y: null,
    width: 300,
    height: 360,
    opacity: 1,
    alwaysOnTop: false,
    showCompleted: false,
  },
  notesWidget: {
    enabled: false,
    x: null,
    y: null,
    width: 300,
    height: 380,
    opacity: 1,
    alwaysOnTop: false,
    showCompleted: false,
  },
  hud: {
    enabled: false,
    x: null,
    y: null,
    opacity: 1,
    alwaysOnTop: true,
    snapToEdge: false,
    stealth: false,
    stealthDelaySecs: 15,
  },
};
