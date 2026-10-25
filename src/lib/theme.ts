import type { HandySettings, ThemeChoice } from "../types/settings";

/** 与 Acta 相同的主题清单（sage 为 Acta 的 CSS 兜底色板，即 Handy 0.x 的浅色）。 */
export const THEMES: readonly ThemeChoice[] = [
  "sage",
  "eye-yellow",
  "mono-light",
  "mono-dark",
  "blue-yellow",
  "forest-mist",
  "sunset-coral",
  "candy-pop",
  "mws-light",
  "mws-dark",
  "neon-ocean",
  "aurora-night",
];

const DARK_THEMES = new Set<string>(["mono-dark", "mws-dark", "neon-ocean", "aurora-night"]);

/** 跟随系统时的落点：浅色用默认色板，深色用 Acta 的黑白深色（与 0.x 一致）。 */
const AUTO_LIGHT = "sage";
const AUTO_DARK = "mono-dark";

const QUERY = "(prefers-color-scheme: dark)";
let media: MediaQueryList | null = null;
let choice: ThemeChoice = "auto";

/** 0.x 的 light / dark 在前端同样迁移（后端 sanitized 已迁移，mock 预览走这里）。 */
function normalize(next: string): string {
  if (next === "light") return AUTO_LIGHT;
  if (next === "dark") return AUTO_DARK;
  return next;
}

function apply(): void {
  const theme =
    choice === "auto"
      ? media?.matches
        ? AUTO_DARK
        : AUTO_LIGHT
      : normalize(choice);
  document.documentElement.dataset.handyTheme = theme;
  document.documentElement.dataset.handyScheme = DARK_THEMES.has(theme) ? "dark" : "light";
}

export function applyTheme(next: ThemeChoice): void {
  choice = THEMES.includes(next) || next === "auto" ? next : "auto";
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
