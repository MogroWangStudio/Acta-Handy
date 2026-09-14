<div align="center">
  <img src="docs/acta-handy-text-logo.svg" alt="Acta Handy 行记·便易" width="420" />
  <p><strong>把 Acta 的待办与笔记带到桌面上：小组件、悬浮窗，一眼即达。</strong></p>
</div>

---

**Acta Handy 行记·便易** 是 Acta · 行记 的伴生小工具：它读取 Acta 同步出的**数据文件夹**，把今日待办、最近笔记以桌面小组件和悬浮窗的形式常驻桌面。数据始终保存在你自己的文件夹里，Acta Handy **只读取、不写入**。

- 🧩 **待办小组件** — 今日待办、逾期提醒、子任务进度，鼠尾草绿的进度条与 Acta 一脉相承
- 🗒️ **笔记小组件** — 最近笔记按更新时间排列，标题与摘要一览
- 🫧 **悬浮窗** — 一条常驻的今日进度环，随时瞄一眼
- 🎛️ **独立设置窗口** — 数据源、每个小组件的位置/不透明度/置顶，一一可调
- 🌓 **浅色 / 深色主题**，跟随系统或手动指定，界面语言支持简体中文与 English

## 数据来源

Acta Handy 读取 Acta「设置 → 数据」中同步出的 **Acta 数据文件夹**（含 `acta-manifest.json`、`classifications.json`、`notes/`、`todos/` 的 V3 格式，兼容 V2 单文件笔记）。它只做只读访问，不会修改任何数据；所有编辑请回到 Acta 完成，改动会自动同步到这里。

小组件以自动刷新（默认 30 秒，可调）跟随 Acta 的最近同步。

## 使用

1. 在 Acta 中把行记数据同步到任意本地文件夹；
2. 打开 Acta Handy，在「数据源」中选择该文件夹；
3. 在「待办小组件 / 笔记小组件 / 悬浮窗」中启用你需要的窗口，拖动即可摆放位置，位置与大小会自动记住。

菜单栏（Windows 为系统托盘）图标可以随时打开设置、开关各个小组件，或退出应用。

## 开发

```bash
npm install
npm run desktop:dev        # Tauri 开发模式
npm run typecheck          # vue-tsc 类型检查
npm run desktop:build      # 构建当前平台安装包
```

在纯浏览器中预览界面（使用内置演示数据，不影响生产构建）：

```bash
VITE_MOCK_TAURI=1 npx vite --port 5201
# 主窗口 http://localhost:5201/?label=main
# 待办小组件 http://localhost:5201/?label=todo-widget …
```

技术栈：Vue 3 + TypeScript + Vite + Tauri 2（Rust 端负责数据读取、窗口管理与托盘）。

## 构建

GitHub Actions 会在推送 `v*` 标签时自动构建：

- **Windows x64 便携版**（单个 exe，免安装）
- **macOS Apple Silicon**（arm64 dmg）

## 许可

[MIT](LICENSE) © MogroWang Studio。Acta Handy 是 Acta 的独立伴生应用，界面与数据格式与 Acta 保持一致。
