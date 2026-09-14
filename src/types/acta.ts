/** Types mirroring Acta's 行记 data folder (V3) and what our Rust reader returns. */

export type Priority = "high" | "medium" | "low";

export interface ActaFolder {
  id: string;
  nameKey?: string | null;
  name?: string | null;
  color?: string | null;
  shortName?: string | null;
}

export interface ActaNote {
  id: string;
  title: string;
  folderId: string;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
  bodyMarkdown: string;
}

export interface ActaTask {
  id: string;
  text: string;
  done: boolean;
}

export interface ActaTodo {
  id: string;
  title: string;
  folderId: string;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
  startAt: string;
  dueAt: string;
  priority: Priority;
  tasks: ActaTask[];
  completed: boolean;
  notes: string;
}

export interface ActaData {
  path: string;
  syncedAt?: string | null;
  folders: ActaFolder[];
  notes: ActaNote[];
  todos: ActaTodo[];
  warnings: string[];
}
