import type { HandySettings, ThemeChoice } from "../types/settings";

const QUERY = "(prefers-color-scheme: dark)";
let media: MediaQueryList | null = null;
let choice: ThemeChoice = "auto";

function apply(): void {
  const dark = choice === "dark" || (choice === "auto" && media?.matches);
  document.documentElement.dataset.handyTheme = dark ? "dark" : "light";
}

export function applyTheme(next: ThemeChoice): void {
  choice = next;
  if (!media) {
    media = window.matchMedia(QUERY);
    media.addEventListener("change", apply);
  }
  apply();
}

type FontChoice = "system" | "serif" | "kai" | "rounded";

/** 字体预设 → CSS 字体栈：跟随系统保持令牌原值，其余按平台列出首选与回退。 */
const FONT_STACKS: Record<FontChoice, string | null> = {
  system: null,
  serif: 'Georgia, "Times New Roman", "Songti SC", "SimSun", serif',
  kai: '"Kaiti SC", "STKaiti", "KaiTi", serif',
  rounded: '"Yuanti SC", "YouYuan", "PingFang SC", sans-serif',
};

function applyTypography(font: string, scale: number): void {
  const root = document.documentElement;
  const stack = FONT_STACKS[(font as FontChoice) in FONT_STACKS ? (font as FontChoice) : "system"];
  if (stack) root.style.setProperty("--font-body", stack);
  else root.style.removeProperty("--font-body");
  const zoom = Math.min(1.3, Math.max(0.9, scale || 1));
  root.style.zoom = String(zoom);
}

/** 主题与排版一起应用：设置加载与每次变更都从这里走，全部窗口保持一致。 */
export function applyAppearance(settings: HandySettings): void {
  applyTheme(settings.theme);
  applyTypography(settings.font, settings.fontScale);
}
