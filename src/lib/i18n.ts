import { store } from "./store";

const zh = {
  appName: "Acta Handy",
  appNameCn: "行记·便易",
  tagline: "把 Acta 的待办与笔记带到桌面上",

  // Nav
  navData: "数据源",
  navTodoWidget: "待办小组件",
  navNotesWidget: "笔记小组件",
  navHud: "悬浮窗",
  navGeneral: "通用",
  navAbout: "关于",

  // Data source
  dataFolder: "Acta 数据文件夹",
  dataFolderDesc: "在 Acta 的「设置 → 数据」中同步出的数据文件夹，Acta Handy 只读取，不写入。",
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

  // Widgets
  enableWidget: "启用小组件",
  enableWidgetDesc: "在桌面上显示这个小组件。",
  opacity: "不透明度",
  opacityDesc: "数值越低，小组件与桌面融合得越好。",
  alwaysOnTop: "保持在最前",
  alwaysOnTopDesc: "让小组件一直浮在其他窗口上方。",
  showCompleted: "显示已完成",
  showCompletedDesc: "已勾选完成的待办仍保留在列表里。",
  widgetHint: "小组件可以直接拖动摆放位置，拖动边缘调整大小，位置会自动记住。",
  hudEnableDesc: "在桌面上显示一条常驻的今日进度。",
  hudAlwaysOnTopDesc: "悬浮窗默认保持在最前，方便随时瞄一眼。",

  // General
  appearance: "外观",
  theme: "主题",
  themeAuto: "跟随系统",
  themeLight: "浅色",
  themeDark: "深色",
  language: "语言",

  // About
  aboutTitle: "Acta Handy 行记·便易",
  aboutDesc: "Acta Handy 读取 Acta（行记）的数据文件夹，把今日待办、最近笔记以小组件和悬浮窗的形式常驻桌面。数据始终保存在你自己的文件夹里，Acta Handy 不会写入或修改。",
  aboutReadonly: "只读视图",
  aboutReadonlyDesc: "所有编辑请回到 Acta 完成，改动会自动同步到这里。",
  version: "版本",
  author: "作者",
  license: "开源协议",
  basedOn: "来自",

  // Todo widget
  todayTodos: "今日待办",
  upcoming: "即将到来",
  allCaughtUp: "今日事项已完成",
  nothingToday: "今天没有安排待办",
  pickFolderFirst: "选择 Acta 数据文件夹后，待办会出现在这里",
  openSettings: "打开设置",
  readonlyHint: "只读视图 · 在 Acta 中编辑",
  today: "今天",
  tomorrow: "明天",
  overdue: "已逾期",
  yesterday: "昨天",
  doneCount: (done: number, total: number) => `${done}/${total}`,
  subtaskOf: (done: number, total: number) => `子任务 ${done}/${total}`,
  untitledTodo: "未命名待办",
  weekDays: ["周日", "周一", "周二", "周三", "周四", "周五", "周六"],

  // Notes widget
  recentNotes: "最近笔记",
  noNotes: "还没有笔记",
  noNotesHint: "在 Acta 中写下的笔记会按更新时间出现在这里",
  untitledNote: "无标题笔记",

  // HUD
  hudNext: "接下来",
  hudNothing: "今日无待办",
  hudAllDone: "今日已完成",
};

type Dict = typeof zh;

const en: Dict = {
  appName: "Acta Handy",
  appNameCn: "Acta Handy",
  tagline: "Acta's todos and notes, right on your desktop",

  navData: "Data Source",
  navTodoWidget: "Todo Widget",
  navNotesWidget: "Notes Widget",
  navHud: "Floating HUD",
  navGeneral: "General",
  navAbout: "About",

  dataFolder: "Acta Data Folder",
  dataFolderDesc:
    "The data folder Acta syncs to (Settings → Data). Acta Handy only reads it, never writes.",
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

  enableWidget: "Enable widget",
  enableWidgetDesc: "Show this widget on your desktop.",
  opacity: "Opacity",
  opacityDesc: "Lower values blend the widget into your desktop.",
  alwaysOnTop: "Stay on top",
  alwaysOnTopDesc: "Keep the widget above other windows.",
  showCompleted: "Show completed",
  showCompletedDesc: "Keep checked-off todos in the list.",
  widgetHint: "Drag a widget to move it, drag its edges to resize — positions are remembered.",
  hudEnableDesc: "Show a slim strip with today's progress.",
  hudAlwaysOnTopDesc: "The HUD stays on top by default for quick glances.",

  appearance: "Appearance",
  theme: "Theme",
  themeAuto: "Match system",
  themeLight: "Light",
  themeDark: "Dark",
  language: "Language",

  aboutTitle: "Acta Handy",
  aboutDesc:
    "Acta Handy reads Acta's data folder and keeps your todos and notes on the desktop as widgets and a floating HUD. Your data stays in your own folder — Acta Handy never writes to it.",
  aboutReadonly: "Read-only view",
  aboutReadonlyDesc: "Edit in Acta; changes sync here automatically.",
  version: "Version",
  author: "Author",
  license: "License",
  basedOn: "From",

  todayTodos: "Today's Todos",
  upcoming: "Upcoming",
  allCaughtUp: "All caught up",
  nothingToday: "Nothing scheduled today",
  pickFolderFirst: "Pick an Acta data folder and your todos will show up here",
  openSettings: "Open Settings",
  readonlyHint: "Read-only · edit in Acta",
  today: "Today",
  tomorrow: "Tomorrow",
  overdue: "Overdue",
  yesterday: "Yesterday",
  doneCount: (done: number, total: number) => `${done}/${total}`,
  subtaskOf: (done: number, total: number) => `${done}/${total} subtasks`,
  untitledTodo: "Untitled todo",
  weekDays: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],

  recentNotes: "Recent Notes",
  noNotes: "No notes yet",
  noNotesHint: "Notes written in Acta appear here by update time",
  untitledNote: "Untitled note",

  hudNext: "Next",
  hudNothing: "No todos today",
  hudAllDone: "All done today",
};

const dicts: Record<"zh" | "en", Dict> = { zh, en };

export function t<K extends keyof Dict>(key: K): Dict[K] {
  const lang = store.settings?.language === "en" ? "en" : "zh";
  return dicts[lang][key];
}

export function isChinese(): boolean {
  return store.settings?.language !== "en";
}
