import type { ThemeChoice } from "../types/settings";

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
