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
  /** 吸附屏幕边缘：拖到边缘附近松手后自动贴合对齐。 */
  snapToEdge: boolean;
}

export interface HudConfig {
  enabled: boolean;
  x: number | null;
  y: number | null;
  alwaysOnTop: boolean;
  /** Handy 缩放：0.2–1.5 无极调整（右键菜单滑块与设置窗口共用）。 */
  scale: number;
  /** 探头模式：吸附到最近的屏幕边缘，光标靠近时展开快速编辑面板。 */
  snapToEdge: boolean;
  /** 隐匿模式：超过设定延迟后淡出，光标靠近时再唤醒。 */
  stealth: boolean;
  stealthDelaySecs: number;
  /** Handy 身体颜色：应用色板预设名，auto = 跟随主题墨色。 */
  color: "auto" | "sage" | "amber" | "violet" | "danger";
}

export interface MainWindowConfig {
  x: number | null;
  y: number | null;
  width: number;
  height: number;
}

export type FontChoice = "system" | "serif" | "kai" | "rounded";

export interface HandySettings {
  version: number;
  dataFolder: string;
  theme: ThemeChoice;
  language: LanguageChoice;
  /** 界面字体预设：跟随系统 / 衬线 / 楷体 / 圆体。 */
  font: FontChoice;
  /** 字体大小（整体缩放）：0.9–1.25。 */
  fontScale: number;
  refreshIntervalSecs: number;
  window: MainWindowConfig;
  todoWidget: WidgetConfig;
  notesWidget: WidgetConfig;
  hud: HudConfig;
}

export const DEFAULT_SETTINGS: HandySettings = {
  version: 1,
  dataFolder: "",
  theme: "auto",
  language: "zh",
  font: "system",
  fontScale: 1,
  refreshIntervalSecs: 30,
  window: { x: null, y: null, width: 940, height: 640 },
  todoWidget: {
    enabled: true,
    x: null,
    y: null,
    width: 300,
    height: 360,
    opacity: 1,
    alwaysOnTop: false,
    showCompleted: false,
    snapToEdge: false,
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
    snapToEdge: false,
  },
  hud: {
    enabled: false,
    x: null,
    y: null,
    alwaysOnTop: true,
    scale: 1,
    snapToEdge: false,
    stealth: false,
    stealthDelaySecs: 15,
    color: "auto",
  },
};
