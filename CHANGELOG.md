# Changelog

本项目的全部重要变更都记录在此文件中。
格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.1.0] - 2026-09-14

首个公开版本。

### 新增

- 读取 Acta（行记）的数据文件夹（`acta-manifest.json` + `classifications.json` + `notes/` + `todos/`，V3 格式，兼容 V2 单文件笔记），全程只读、不写入。
- 待办小组件：今日待办与逾期提醒、优先级胶囊、子任务进度、今日完成进度条，可选显示已完成事项。
- 笔记小组件：最近笔记按更新时间排列，展示标题、摘要与归类。
- 悬浮窗：常驻桌面的今日进度环，一眼看到下一个待办。
- 独立设置窗口：数据源选择与状态、自动刷新间隔、每个窗口的不透明度与置顶、浅色/深色/跟随系统主题、界面语言（简体中文 / English）。
- 小组件与悬浮窗支持拖动摆放、拖边调整大小，位置与尺寸自动记忆。
- 菜单栏 / 系统托盘：快速打开设置、开关各窗口、退出应用。
- GitHub Actions 自动构建：Windows x64 便携版（免安装 exe）与 macOS Apple Silicon（arm64 dmg）。

[0.1.0]: #
