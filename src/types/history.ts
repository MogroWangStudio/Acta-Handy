/** 修改历史：一次写回 Acta 数据文件夹的记录，含写入前快照（可恢复）。 */

import type { ActaTask } from "./acta";

export type HistoryKind = "todo-check" | "note-edit" | "note-create" | "todo-restore" | "note-restore";

export interface HistorySnapshot {
  item: Record<string, unknown> & {
    title?: string;
    completed?: boolean;
    tasks?: ActaTask[];
  };
  body?: string | null;
}

export interface HistoryEntry {
  id: string;
  time: string;
  kind: HistoryKind;
  /** 这条写入指向的数据文件夹；恢复时按它校验，避免跨数据源错写。 */
  folder: string;
  itemId: string;
  title: string;
  completed?: boolean | null;
  before?: HistorySnapshot | null;
}
