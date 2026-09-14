import type { ActaFolder, ActaTodo } from "../types/acta";
import { isChinese } from "./i18n";

const FOLDER_NAME_KEYS: Record<string, [string, string]> = {
  inboxFolder: ["灵感收集", "Idea inbox"],
  workFolder: ["工作计划", "Work plans"],
  lifeFolder: ["生活清单", "Life lists"],
  readingFolder: ["阅读摘记", "Reading notes"],
};

export function folderName(folder: ActaFolder | undefined | null): string {
  if (!folder) return isChinese() ? "未归类" : "Unfiled";
  if (folder.nameKey && FOLDER_NAME_KEYS[folder.nameKey]) {
    const [zhName, enName] = FOLDER_NAME_KEYS[folder.nameKey];
    return isChinese() ? zhName : enName;
  }
  return folder.name || (isChinese() ? "未归类" : "Unfiled");
}

/** Acta colors folders with #rgb/#rrggbb/#rrggbbaa; anything else falls back to #999 (like Acta). */
export function folderColor(folder: ActaFolder | undefined | null): string {
  const raw = folder?.color;
  if (raw && /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(raw)) {
    return raw.length === 5
      ? `#${raw[1]}${raw[1]}${raw[2]}${raw[2]}${raw[3]}${raw[3]}`
      : `#${raw.slice(1, 7)}`;
  }
  return "#999999";
}

export function isTodoDone(todo: ActaTodo): boolean {
  if (todo.completed) return true;
  return todo.tasks.length > 0 && todo.tasks.every((task) => task.done);
}

export function taskProgress(todo: ActaTodo): { done: number; total: number } {
  return {
    done: todo.tasks.filter((task) => task.done).length,
    total: todo.tasks.length,
  };
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

export function dayDiff(iso: string): number {
  if (!iso) return Number.NaN;
  const target = startOfDay(new Date(iso));
  if (Number.isNaN(target)) return Number.NaN;
  return Math.round((target - startOfDay(new Date())) / 86400000);
}

/** "今天 14:00" style labels; overdue items get their own treatment in CSS. */
export function dueLabel(startAt: string, dueAt: string): string {
  const ref = dueAt || startAt;
  if (!ref) return "";
  const date = new Date(ref);
  if (Number.isNaN(date.getTime())) return "";
  const diff = dayDiff(ref);
  const time = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
  const cn = isChinese();
  let day: string;
  if (diff === 0) day = cn ? "今天" : "Today";
  else if (diff === 1) day = cn ? "明天" : "Tomorrow";
  else if (diff === -1) day = cn ? "昨天" : "Yesterday";
  else day = `${date.getMonth() + 1}${cn ? "月" : "/"}${date.getDate()}${cn ? "日" : ""}`;
  const showTime = date.getHours() !== 0 || date.getMinutes() !== 0;
  return showTime ? `${day} ${time}` : day;
}

export function shortDate(iso: string): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const cn = isChinese();
  return cn
    ? `${date.getMonth() + 1}月${date.getDate()}日`
    : `${date.toLocaleString("en", { month: "short" })} ${date.getDate()}`;
}

export function longDate(date: Date): string {
  const cn = isChinese();
  const week = t_week(date, cn);
  return cn
    ? `${date.getFullYear()}年${date.getMonth() + 1}月${date.getDate()}日 ${week}`
    : `${date.toLocaleString("en", { weekday: "long", month: "long", day: "numeric" })}`;
}

function t_week(date: Date, cn: boolean): string {
  const names = cn
    ? ["周日", "周一", "周二", "周三", "周四", "周五", "周六"]
    : ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
  return names[date.getDay()];
}

/** Plain-text snippet for a note body written in Markdown. */
export function markdownToPlain(md: string, maxLength = 140): string {
  const text = md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\s*([-*+]|\d+\.)\s+/gm, "")
    .replace(/^\s*>\s?/gm, "")
    .replace(/(\*\*|__|\*|_|~~)/g, "")
    .replace(/^\s*([-*_]\s*){3,}\s*$/gm, " ")
    .replace(/\r/g, "")
    .replace(/\n+/g, " ")
    .replace(/\s{2,}/g, " ")
    .trim();
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}
