<div align="center">
  <img src="docs/acta-handy-text-logo.svg" alt="Acta Handy 行记·便易" width="420" />
  <p><strong>把 Acta 的待办与笔记带到桌面上：小组件、悬浮窗，一眼即达。</strong></p>
</div>

---

**Acta Handy 行记·便易** 是 Acta · 行记 的伴生小工具：它读取 Acta 同步出的**数据文件夹**，把今日待办、最近笔记以桌面小组件和悬浮窗的形式常驻桌面。勾选待办、编辑笔记会以 Acta 的格式**自动写回**数据文件夹，回到 Acta 随时继续。

- **待办小组件** — 今日待办、逾期提醒、子任务进度；点击整行展开子任务列表，勾选主待办或任意子待办，立即写回数据文件夹
- **笔记小组件** — 最近笔记按更新时间排列，点开即可新建、编辑标题与 Markdown 正文，停笔自动保存
- **悬浮窗** — 常驻的今日进度环；可吸附屏幕边缘变成小药丸，鼠标移过弹出快速编辑面板；支持隐匿模式（闲置淡出、靠近淡入）
- **独立设置窗口** — 数据源、每个窗口的位置/不透明度/置顶、悬浮窗吸附与隐匿，一一可调
- **浅色 / 深色主题**，跟随系统或手动指定，界面语言支持简体中文与 English

## 数据来源

Acta Handy 读取 Acta「设置 → 数据」中同步出的 **Acta 数据文件夹**（含 `acta-manifest.json`、`classifications.json`、`notes/`、`todos/` 的 V3 格式，兼容 V2 单文件笔记）。

写入与 Acta 自身保存完全同构：条目文件保持 `{format, version, item}` 封套、`updatedAt` 使用 ISO 8601 UTC、manifest 最后重写（`syncedAt` 与对应条目的 `updatedAt`），因此 Acta 重新打开或同步时可以无缝接续。勾选待办遵循 Acta 的完成规则——勾选主待办会同步所有子任务，子任务全部完成后待办自动视为完成。编辑器输入停顿约一秒后自动保存，无需手动操作。

小组件以自动刷新（默认 30 秒，可调）跟随 Acta 的最近同步；正在输入时刷新会自动让路，不会打断编辑。

## 使用

1. 在 Acta 中把行记数据同步到任意本地文件夹；
2. 打开 Acta Handy，在「数据源」中选择该文件夹；
3. 在「待办小组件 / 笔记小组件 / 悬浮窗」中启用你需要的窗口，拖动即可摆放位置，位置与大小会自动记住；
4. 悬浮窗开启「吸附屏幕边缘」后变成贴边药丸，鼠标移过即可勾选待办、编辑笔记；「隐匿模式」让它在闲置后悄悄淡出，鼠标靠近再淡入。

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

技术栈：Vue 3 + TypeScript + Vite + Tauri 2（Rust 端负责数据读写、窗口管理与托盘）。

## 构建

GitHub Actions 会在推送 `v*` 标签时自动构建：

- **Windows x64 便携版**（单个 exe，免安装）
- **macOS Apple Silicon**（arm64 dmg）

## 许可

[MIT](LICENSE) © MogroWang Studio。Acta Handy 是 Acta 的独立伴生应用，界面与数据格式与 Acta 保持一致。
