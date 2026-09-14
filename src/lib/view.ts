import type { ActaData, ActaFolder, ActaNote, ActaTodo } from "../types/acta";
import { isTodoDone } from "./format";

export interface TodoBuckets {
  /** Scheduled today or overdue, still open. */
  current: ActaTodo[];
  /** Scheduled later, open. */
  upcoming: ActaTodo[];
  /** Completed items relevant to today (done today-ish or due today). */
  completed: ActaTodo[];
  doneToday: number;
  totalToday: number;
}

function refTime(todo: ActaTodo): number {
  const iso = todo.dueAt || todo.startAt;
  const time = iso ? new Date(iso).getTime() : Number.NaN;
  return Number.isNaN(time) ? Number.NaN : time;
}

export function bucketTodos(todos: ActaTodo[]): TodoBuckets {
  const endOfDay = new Date();
  endOfDay.setHours(23, 59, 59, 999);
  const buckets: TodoBuckets = {
    current: [],
    upcoming: [],
    completed: [],
    doneToday: 0,
    totalToday: 0,
  };
  for (const todo of todos) {
    if (todo.deletedAt) continue;
    const time = refTime(todo);
    const scheduledTodayOrEarlier =
      !Number.isNaN(time) && time <= endOfDay.getTime();
    const upcoming = !Number.isNaN(time) && time > endOfDay.getTime();
    if (isTodoDone(todo)) {
      buckets.completed.push(todo);
      if (scheduledTodayOrEarlier || Number.isNaN(time)) buckets.doneToday += 1;
      continue;
    }
    if (scheduledTodayOrEarlier) {
      buckets.current.push(todo);
      buckets.totalToday += 1;
    } else if (upcoming) {
      buckets.upcoming.push(todo);
    } else {
      // Undated todos belong to today's working list too.
      buckets.current.push(todo);
      buckets.totalToday += 1;
    }
  }
  const byTime = (a: ActaTodo, b: ActaTodo) => {
    const ta = refTime(a);
    const tb = refTime(b);
    if (Number.isNaN(ta) && Number.isNaN(tb)) return 0;
    if (Number.isNaN(ta)) return 1;
    if (Number.isNaN(tb)) return -1;
    return ta - tb;
  };
  buckets.current.sort(byTime);
  buckets.upcoming.sort(byTime);
  const byPriority = { high: 0, medium: 1, low: 2 };
  buckets.current.sort(
    (a, b) => byPriority[a.priority] - byPriority[b.priority] || byTime(a, b),
  );
  buckets.totalToday += buckets.doneToday;
  return buckets;
}

export function recentNotes(notes: ActaNote[], limit = 20): ActaNote[] {
  return notes
    .filter((note) => !note.deletedAt)
    .sort((a, b) => (b.updatedAt || "").localeCompare(a.updatedAt || ""))
    .slice(0, limit);
}

export function folderMap(data: ActaData | null): Map<string, ActaFolder> {
  const map = new Map<string, ActaFolder>();
  for (const folder of data?.folders ?? []) map.set(folder.id, folder);
  return map;
}

export function countsFor(data: ActaData | null): { notes: number; todos: number } {
  return {
    notes: (data?.notes ?? []).filter((n) => !n.deletedAt).length,
    todos: (data?.todos ?? []).filter((n) => !n.deletedAt).length,
  };
}
