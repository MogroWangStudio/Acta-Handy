/**
 * Dev-only Tauri API mock so the UI can be previewed in a plain browser
 * (`VITE_MOCK_TAURI=1 vite`). Inert in production builds; window label comes
 * from `?label=<todo-widget|notes-widget|hud|main>`.
 */

import type { ActaData } from "../types/acta";
import type { HandySettings } from "../types/settings";

const HOUR = 3600_000;
const now = Date.now();
const iso = (offsetMs: number) => new Date(now + offsetMs).toISOString();

function mockSettings(): HandySettings {
  // ?snap：预览贴边探头形态（?label=hud&snap）。
  const snap = new URLSearchParams(location.search).has("snap");
  return {
    version: 1,
    dataFolder: "/Users/demo/Acta 数据文件夹",
    theme: "auto",
    language: "zh",
    refreshIntervalSecs: 30,
    todoWidget: { enabled: true, x: null, y: null, width: 300, height: 360, opacity: 1, alwaysOnTop: false, showCompleted: true },
    notesWidget: { enabled: false, x: null, y: null, width: 300, height: 380, opacity: 1, alwaysOnTop: false, showCompleted: false },
    hud: { enabled: true, x: null, y: null, opacity: 1, alwaysOnTop: true, snapToEdge: snap, stealth: false, stealthDelaySecs: 15 },
  };
}

const data: ActaData = {
  path: "/Users/demo/Acta 数据文件夹",
  syncedAt: iso(-5 * 60_000),
  folders: [
    { id: "ideas", nameKey: "inboxFolder", color: "#b68b54" },
    { id: "work", nameKey: "workFolder", color: "#6f8a72" },
    { id: "life", nameKey: "lifeFolder", color: "#a87876" },
    { id: "reading", nameKey: "readingFolder", color: "#7a7799" },
  ],
  notes: [
    {
      id: "note-1",
      title: "同步机制备忘",
      folderId: "work",
      createdAt: iso(-72 * HOUR),
      updatedAt: iso(-3 * HOUR),
      bodyMarkdown:
        "数据文件夹以 manifest 为准，最后写入保证原子性。\n\n- notes/ 每则笔记两个文件\n- todos/ 每条待办一个文件\n- 冲突时以整库回写解决",
    },
    {
      id: "note-2",
      title: "桌面小组件灵感",
      folderId: "ideas",
      createdAt: iso(-30 * 24 * HOUR),
      updatedAt: iso(-26 * HOUR),
      bodyMarkdown:
        "小组件应该像纸片一样贴在桌面上：安静的底色、衬线标题、一枚鼠尾草绿的进度条。\n\n悬浮窗则要更轻，一眼扫过就好。",
    },
    {
      id: "note-3",
      title: "《卡片笔记写作法》摘录",
      folderId: "reading",
      createdAt: iso(-9 * 24 * HOUR),
      updatedAt: iso(-3 * 24 * HOUR),
      bodyMarkdown: "「笔记的价值不在于收集，而在于建立联系。」\n\n—— 每张卡片都应该指向另一张卡片。",
    },
  ],
  todos: [
    {
      id: "todo-1",
      title: "整理 Q4 路线图草稿",
      folderId: "work",
      createdAt: iso(-30 * HOUR),
      updatedAt: iso(-2 * HOUR),
      startAt: iso(-3 * HOUR),
      dueAt: iso(3 * HOUR),
      priority: "high",
      tasks: [
        { id: "t1", text: "列出三个主线目标", done: true },
        { id: "t2", text: "补充时间点", done: true },
        { id: "t3", text: "发给团队评审", done: false },
      ],
      completed: false,
      notes: "",
    },
    {
      id: "todo-2",
      title: "回复设计评审的邮件",
      folderId: "work",
      createdAt: iso(-28 * HOUR),
      updatedAt: iso(-27 * HOUR),
      startAt: iso(-26 * HOUR),
      dueAt: iso(-22 * HOUR),
      priority: "medium",
      tasks: [],
      completed: false,
      notes: "",
    },
    {
      id: "todo-3",
      title: "买咖啡豆和挂耳滤纸",
      folderId: "life",
      createdAt: iso(-4 * HOUR),
      updatedAt: iso(-4 * HOUR),
      startAt: "",
      dueAt: "",
      priority: "low",
      tasks: [],
      completed: false,
      notes: "",
    },
    {
      id: "todo-4",
      title: "读完《卡片笔记写作法》第四章",
      folderId: "reading",
      createdAt: iso(-2 * 24 * HOUR),
      updatedAt: iso(-20 * HOUR),
      startAt: iso(20 * HOUR),
      dueAt: iso(26 * HOUR),
      priority: "low",
      tasks: [],
      completed: false,
      notes: "",
    },
    {
      id: "todo-5",
      title: "晨间拉伸十五分钟",
      folderId: "life",
      createdAt: iso(-6 * HOUR),
      updatedAt: iso(-1 * HOUR),
      startAt: "",
      dueAt: "",
      priority: "low",
      tasks: [],
      completed: true,
      notes: "",
    },
  ],
  warnings: [],
};

type EventCallback = (evt: { event: string; id: number; payload: unknown }) => void;
const callbacks = new Map<number, EventCallback>();
let nextCallbackId = 1;

const settings = mockSettings();

function invoke(cmd: string, args: Record<string, unknown>): unknown {
  switch (cmd) {
    case "load_settings":
      // 真实 Tauri 每次返回新 JSON 对象；mock 保持一致，避免 Vue 响应式引用相等而不触发。
      return JSON.parse(JSON.stringify(settings));
    case "save_settings":
      Object.assign(settings, args.settings);
      fire("settings-changed", JSON.parse(JSON.stringify(settings)));
      return JSON.parse(JSON.stringify(settings));
    case "read_acta_data":
      return Promise.resolve(data);
    case "write_todo_check": {
      const patch = args.patch as { todoId: string; completed: boolean; tasks: Array<{ id: string; done: boolean }> };
      const todo = data.todos.find((t) => t.id === patch.todoId);
      if (!todo) return Promise.reject("未找到待办");
      todo.completed = patch.completed;
      for (const task of todo.tasks) {
        const check = patch.tasks.find((c) => c.id === task.id);
        if (check) task.done = check.done;
      }
      todo.updatedAt = new Date().toISOString();
      return Promise.resolve(JSON.parse(JSON.stringify(todo)));
    }
    case "write_note": {
      const patch = args.patch as { noteId?: string | null; title?: string | null; bodyMarkdown?: string | null };
      if (!patch.noteId) {
        const note = {
          id: `mock-${Date.now().toString(36)}`,
          title: patch.title ?? "",
          folderId: "",
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          bodyMarkdown: patch.bodyMarkdown ?? "",
        };
        data.notes.unshift(note);
        return Promise.resolve(note);
      }
      const note = data.notes.find((n) => n.id === patch.noteId);
      if (!note) return Promise.reject("未找到笔记");
      if (patch.title !== null && patch.title !== undefined) note.title = patch.title;
      if (patch.bodyMarkdown !== null && patch.bodyMarkdown !== undefined) note.bodyMarkdown = patch.bodyMarkdown;
      note.updatedAt = new Date().toISOString();
      return Promise.resolve(JSON.parse(JSON.stringify(note)));
    }
    case "set_hud_mode":
      return Promise.resolve(args.mode === "free" ? null : "right");
    case "set_hud_cursor_watch":
    case "refresh_data":
      if (cmd === "refresh_data") fire("acta-data-changed", null);
      return null;
    case "show_window":
    case "quit_app":
      return null;
    case "plugin:event|listen":
      return nextCallbackId++;
    case "plugin:event|unlisten":
      return null;
    case "plugin:window|is_maximized":
      return false;
    case "plugin:dialog|open":
      return "/Users/demo/Acta 数据文件夹";
    default:
      console.warn("[mock] unhandled invoke:", cmd);
      return null;
  }
}

function fire(event: string, payload: unknown): void {
  window.dispatchEvent(new CustomEvent("mock-event", { detail: { event, payload } }));
}

export function installMock(): void {
  const label = new URLSearchParams(location.search).get("label") ?? "main";
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label }, currentWebview: { label } },
    transformCallback: (cb: (evt: { event: string; id: number; payload: unknown }) => void) => {
      const id = nextCallbackId++;
      callbacks.set(id, cb);
      window.addEventListener("mock-event", ((e: CustomEvent) => {
        if (e.detail.event === "settings-changed" || e.detail.event === "acta-data-changed") {
          cb({ event: e.detail.event, id, payload: e.detail.payload });
        }
      }) as EventListener);
      return id;
    },
    invoke,
  };
}
