import { store } from "./store";

const zh = {
  appName: "Acta Handy",
  appNameCn: "行记·便易",
  tagline: "把 Acta 的待办与笔记带到桌面上",

  // Window titles（浏览器里调试各窗口时靠 document.title 区分）
  titleMain: "Acta Handy · 设置",
  titleTodoWidget: "Acta Handy · 待办小组件",
  titleNotesWidget: "Acta Handy · 笔记小组件",
  titleHud: "Acta Handy · Handy",
  titleHudPanel: "Acta Handy · Handy 面板",

  // Nav
  navData: "数据源",
  navTodoWidget: "待办小组件",
  navNotesWidget: "笔记小组件",
  navHud: "Handy",
  navGeneral: "通用",
  navAbout: "关于",

  // Data source
  dataFolder: "Acta 数据文件夹",
  dataFolderDesc:
    "在 Acta 的「设置 → 数据」中同步出的数据文件夹。Acta Handy 与它共用数据：勾选待办、编辑笔记都会像 Acta 一样直接写回，每次修改都留有历史，可随时恢复。",
  pickFolder: "选择文件夹",
  rescan: "重新读取",
  noFolder: "尚未选择数据文件夹",
  lastSync: "Acta 最近同步",
  itemCounts: (notes: number, todos: number) => `${notes} 则笔记 · ${todos} 条待办`,
  refreshInterval: "自动刷新间隔",
  refreshIntervalDesc: "Acta 在其他设备同步后，小组件跟随更新的最长时间。",
  dataErrorTitle: "读取数据文件夹时出现问题",
  dataWarnings: (n: number) => `${n} 个条目文件读取失败，已跳过`,
  minutes: "分钟",
  seconds: "秒",

  // Edit history
  historyTitle: "修改历史",
  historyDesc: "每一次写回都有留档，必要时可恢复到改动前的模样。",
  historyEmpty: "还没有修改记录",
  historySummary: (kind: string, title: string, completed: boolean | null | undefined) => {
    const name = title || "未命名";
    switch (kind) {
      case "todo-check":
        return completed === true ? `完成待办「${name}」` : completed === false ? `重开待办「${name}」` : `调整子任务「${name}」`;
      case "note-edit":
        return `编辑笔记「${name}」`;
      case "note-create":
        return `新建笔记「${name}」`;
      case "todo-restore":
        return `恢复待办「${name}」到更早状态`;
      case "note-restore":
        return `恢复笔记「${name}」到更早状态`;
      default:
        return name;
    }
  },
  historyRestore: "恢复",
  historyRestoreFail: "恢复失败",
  historyNoBefore: "新建的条目没有更早的状态",

  // Widgets
  enableWidget: "启用小组件",
  enableWidgetDesc: "在桌面上显示这个小组件。",
  opacity: "不透明度",
  opacityDesc: "数值越低，小组件与桌面融合得越好。",
  alwaysOnTop: "保持在最前",
  alwaysOnTopDesc: "让小组件一直浮在其他窗口上方。",
  showCompleted: "显示已完成",
  showCompletedDesc: "已勾选完成的待办仍保留在列表里。",
  widgetHint: "小组件可以直接拖动摆放位置，勾选待办、编辑笔记都会自动保存。",
  widgetSnapToEdge: "吸附屏幕边缘",
  widgetSnapToEdgeDesc: "拖到屏幕边缘附近松手时，自动贴合对齐。",
  hudEnableDesc: "在桌面上显示 Handy（汉迪）：会眨眼、会呼吸，眼睛还会跟着你的鼠标看。",
  hudHint: "悬停时 Handy 会看向你、身体轻轻变色；左键点击弹出快速编辑面板，按住即可拖动换位置，右键打开大小菜单。",
  hudSize: "Handy 大小",
  hudSizeDesc: "拖动调整 Handy 在桌面上的个头（20%–150%），右键 Handy 也能随时调整。",
  hudColor: "Handy 颜色",
  hudColorDesc: "从应用色板里挑一个他喜欢的身体颜色，默认跟随主题。",
  colorAuto: "跟随主题",
  colorSage: "鼠尾草绿",
  colorAmber: "琥珀",
  colorViolet: "紫罗兰",
  colorClay: "陶红",
  hudAlwaysOnTopDesc: "Handy 默认保持在最前，方便随时找到他。",
  hudSnapToEdge: "吸附屏幕边缘",
  hudSnapToEdgeDesc: "开启后 Handy 会自然跑到屏幕边缘、探头趴在边上；光标靠近时跳出完整身体并弹出快速编辑面板。",
  hudStealth: "隐匿模式",
  hudStealthDesc: "超过设定时间没有操作时自动淡出，光标靠近时 Handy 再悄悄回来。",
  hudStealthDelay: "淡出延迟",
  hudStealthDelayDesc: "最后一次交互之后，等待多久开始淡出。",

  // General
  appearance: "外观",
  theme: "主题",
  themeAuto: "跟随系统",
  themeLight: "浅色",
  themeDark: "深色",
  language: "语言",

  // About
  aboutTitle: "Acta Handy 行记·便易",
  aboutDesc:
    "Acta Handy 与 Acta（行记）共用数据文件夹，把今日待办、最近笔记以小组件和悬浮窗的形式常驻桌面；修改像 Acta 一样直接写回，并留有可回溯的历史。",
  version: "版本",
  author: "作者",
  license: "开源协议",
  basedOn: "来自",

  // Todo widget
  todayTodos: "今日待办",
  upcoming: "即将到来",
  allCaughtUp: "今日事项已完成",
  nothingToday: "今天没有安排待办",
  filterAll: "全部",
  unfiled: "未归类",
  nothingInFolder: "该分类下暂时没有内容",
  pickFolderFirst: "选择 Acta 数据文件夹后，待办会出现在这里",
  openSettings: "打开设置",
  autosaveHint: "勾选与编辑会自动保存",
  today: "今天",
  tomorrow: "明天",
  overdue: "已逾期",
  yesterday: "昨天",
  doneCount: (done: number, total: number) => `${done}/${total}`,
  subtaskOf: (done: number, total: number) => `子任务 ${done}/${total}`,
  untitledTodo: "未命名待办",
  markDone: "标记完成",
  markUndone: "标记未完成",
  toggleSubtask: "勾选子任务",
  weekDays: ["周日", "周一", "周二", "周三", "周四", "周五", "周六"],

  // Notes widget
  recentNotes: "最近笔记",
  noNotes: "还没有笔记",
  noNotesHint: "点击「新建笔记」写一条，或在 Acta 中写下后自动出现在这里",
  untitledNote: "无标题笔记",
  newNote: "新建笔记",
  noteTitlePlaceholder: "笔记标题",
  noteBodyPlaceholder: "用 Markdown 书写…",
  backToNotes: "返回列表",
  saving: "保存中…",
  saved: "已保存",
  saveFailed: "保存失败",

  // HUD
  hudNext: "接下来",
  hudNothing: "今日无待办",
  hudAllDone: "今日已完成",
  hudClose: "关闭 Handy",
  panelTodos: "待办",
  panelNotes: "笔记",
  panelNoFolder: "先在设置中选择数据文件夹",
  panelQuickEdit: "快速编辑",
};

type Dict = typeof zh;

const en: Dict = {
  appName: "Acta Handy",
  appNameCn: "Acta Handy",
  tagline: "Acta's todos and notes, right on your desktop",

  titleMain: "Acta Handy · Settings",
  titleTodoWidget: "Acta Handy · Todo Widget",
  titleNotesWidget: "Acta Handy · Notes Widget",
  titleHud: "Acta Handy · Handy",
  titleHudPanel: "Acta Handy · Handy Panel",

  navData: "Data Source",
  navTodoWidget: "Todo Widget",
  navNotesWidget: "Notes Widget",
  navHud: "Handy",
  navGeneral: "General",
  navAbout: "About",

  dataFolder: "Acta Data Folder",
  dataFolderDesc:
    "The data folder Acta syncs to (Settings → Data). Acta Handy shares it with Acta: checks and edits are written back in Acta's own format, and every change is kept in a history you can restore.",
  pickFolder: "Choose Folder",
  rescan: "Reload",
  noFolder: "No data folder selected",
  lastSync: "Acta last synced",
  itemCounts: (notes: number, todos: number) => `${notes} notes · ${todos} todos`,
  refreshInterval: "Auto-refresh interval",
  refreshIntervalDesc: "How long widgets may lag behind Acta's latest sync.",
  dataErrorTitle: "Couldn't read the data folder",
  dataWarnings: (n: number) => `${n} item files failed to load and were skipped`,
  minutes: "min",
  seconds: "sec",

  // Edit history
  historyTitle: "Edit history",
  historyDesc: "Every write-back is recorded — restore an earlier state whenever you need to.",
  historyEmpty: "No changes yet",
  historySummary: (kind: string, title: string, completed: boolean | null | undefined) => {
    const name = title || "Untitled";
    switch (kind) {
      case "todo-check":
        return completed === true ? `Completed “${name}”` : completed === false ? `Reopened “${name}”` : `Updated subtasks of “${name}”`;
      case "note-edit":
        return `Edited note “${name}”`;
      case "note-create":
        return `Created note “${name}”`;
      case "todo-restore":
        return `Restored “${name}” to an earlier state`;
      case "note-restore":
        return `Restored “${name}” to an earlier state`;
      default:
        return name;
    }
  },
  historyRestore: "Restore",
  historyRestoreFail: "Restore failed",
  historyNoBefore: "Nothing earlier to restore for a created item",

  enableWidget: "Enable widget",
  enableWidgetDesc: "Show this widget on your desktop.",
  opacity: "Opacity",
  opacityDesc: "Lower values blend the widget into your desktop.",
  alwaysOnTop: "Stay on top",
  alwaysOnTopDesc: "Keep the widget above other windows.",
  showCompleted: "Show completed",
  showCompletedDesc: "Keep checked-off todos in the list.",
  widgetHint: "Drag a widget to place it — checks and edits save themselves.",
  widgetSnapToEdge: "Snap to screen edge",
  widgetSnapToEdgeDesc: "Release near a screen edge and the widget aligns itself to it.",
  hudEnableDesc: "Show Handy on your desktop — it blinks, breathes, and its eyes follow your cursor.",
  hudHint: "Hover and Handy looks at you, its body tinting softly; left-click opens the quick-edit panel, hold to drag it around, right-click for the size menu.",
  hudSize: "Handy size",
  hudSizeDesc: "Drag to set how big Handy stands on your desktop (20%–150%) — also in Handy's right-click menu.",
  hudColor: "Handy color",
  hudColorDesc: "Pick a body color for Handy from the app palette — the default follows the theme.",
  colorAuto: "Follow theme",
  colorSage: "Sage green",
  colorAmber: "Amber",
  colorViolet: "Violet",
  colorClay: "Clay red",
  hudAlwaysOnTopDesc: "Handy stays on top by default so you can always find it.",
  hudSnapToEdge: "Dock to screen edge",
  hudSnapToEdgeDesc: "Handy runs to the screen edge and peeks out; move the cursor close and it jumps down with the quick-edit panel.",
  hudStealth: "Stealth mode",
  hudStealthDesc: "Fade out after a period of inactivity; Handy sneaks back when the cursor comes near.",
  hudStealthDelay: "Fade-out delay",
  hudStealthDelayDesc: "How long to wait after the last interaction before fading out.",

  appearance: "Appearance",
  theme: "Theme",
  themeAuto: "Match system",
  themeLight: "Light",
  themeDark: "Dark",
  language: "Language",

  aboutTitle: "Acta Handy",
  aboutDesc:
    "Acta Handy shares Acta's data folder and keeps your todos and notes on the desktop as widgets and a floating HUD — edits are written back in Acta's format, with a restorable history.",
  version: "Version",
  author: "Author",
  license: "License",
  basedOn: "From",

  todayTodos: "Today's Todos",
  upcoming: "Upcoming",
  allCaughtUp: "All caught up",
  nothingToday: "Nothing scheduled today",
  filterAll: "All",
  unfiled: "Unfiled",
  nothingInFolder: "Nothing in this folder yet",
  pickFolderFirst: "Pick an Acta data folder and your todos will show up here",
  openSettings: "Open Settings",
  autosaveHint: "Checks & edits save automatically",
  today: "Today",
  tomorrow: "Tomorrow",
  overdue: "Overdue",
  yesterday: "Yesterday",
  doneCount: (done: number, total: number) => `${done}/${total}`,
  subtaskOf: (done: number, total: number) => `${done}/${total} subtasks`,
  untitledTodo: "Untitled todo",
  markDone: "Mark done",
  markUndone: "Mark undone",
  toggleSubtask: "Toggle subtask",
  weekDays: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],

  recentNotes: "Recent Notes",
  noNotes: "No notes yet",
  noNotesHint: "Write one with “New note”, or add it in Acta and it shows up here",
  untitledNote: "Untitled note",
  newNote: "New Note",
  noteTitlePlaceholder: "Note title",
  noteBodyPlaceholder: "Write in Markdown…",
  backToNotes: "Back to list",
  saving: "Saving…",
  saved: "Saved",
  saveFailed: "Save failed",

  hudNext: "Next",
  hudNothing: "No todos today",
  hudAllDone: "All done today",
  hudClose: "Close Handy",
  panelTodos: "Todos",
  panelNotes: "Notes",
  panelNoFolder: "Pick a data folder in Settings first",
  panelQuickEdit: "Quick edit",
};

const dicts: Record<"zh" | "en", Dict> = { zh, en };

export function t<K extends keyof Dict>(key: K): Dict[K] {
  const lang = store.settings?.language === "en" ? "en" : "zh";
  return dicts[lang][key];
}

export function isChinese(): boolean {
  return store.settings?.language !== "en";
}
