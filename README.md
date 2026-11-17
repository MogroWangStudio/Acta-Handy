<div align="center">
  <img src="docs/acta-handy-text-logo.svg" alt="Acta Handy 行记·便易" width="420" />
  <p><strong>把 Acta 的待办与笔记带到桌面上：小组件、悬浮窗，一眼即达。</strong></p>
</div>

---

**Acta Handy 行记·便易** 是 Acta · 行记 的伴生小工具：它与 Acta 共用同步出的**数据文件夹**，把今日待办、最近笔记以桌面小组件和悬浮窗的形式常驻桌面。勾选待办、编辑笔记会以 Acta 的格式**自动写回**数据文件夹，回到 Acta 随时继续；每一次写回都留有**修改历史**，可随时恢复到改动前。

- **待办小组件** — 今日待办、逾期提醒、子任务进度；点击整行展开子任务列表，勾选主待办或任意子待办，立即写回数据文件夹；已完成的待办自动沉底并默认折叠（可展开回看）；支持按 Acta 分类（文件夹）筛选
- **笔记小组件** — 最近笔记按更新时间排列，点开即可新建、编辑标题与 Markdown 正文，停笔自动保存；同样支持按分类筛选
- **修改历史可回溯** — 每次写回都有留档（改了什么、何时改的），在设置窗口的「数据源」页一键恢复到改动前的模样
- **Handy** — 桌面吉祥物「汉迪」：会眨眼、会呼吸，眼睛还会跟着你的鼠标看；身体颜色可从应用色板里挑选（默认跟随主题）；悬停时身体轻轻染上主题色。左键点击就地弹出快速编辑面板（Handy 原地不动，面板从它身后弹出、避开屏幕边缘），按住即可拖动换位置（摇晃有自然的起摆与站稳节奏），右键弹出应用内菜单、无极调整大小（20%–150%）或直接关闭——关闭即彻底隐藏，重新开启便以原形态回到原位。开启「吸附屏幕边缘」后，他会探身跑向屏幕边缘、钻出后再探头趴在边上，光标一靠近就弹出快速编辑面板（Handy 保持探头姿势、原地不动）；按住可拖动换边或调整高度，把他在屏幕中间松手则自动解除吸附、以自由形态站到落点；支持隐匿模式（闲置淡出、靠近唤醒，且淡出后不挡桌面点击）
- **小组件吸附屏幕边缘** — 待办与笔记小组件拖到屏幕边缘附近松手时自动贴合对齐，拖动中贴合一侧亮起光效提示；两个小组件之间也能互相吸附、拼在一起
- **独立设置窗口** — 数据源、每个小组件的不透明度/置顶/吸附、Handy 大小/吸附/隐匿，一一可调；设置窗口自身的位置与大小也会被记住，切换各项设置不会改变它的尺寸
- **应用内更新** — 设置 → 关于即可检查更新：稳定通道与测试通道二选一，GitHub Raw 与 GitHub-proxy 两个下载源可一键测速比对；确认后下载，完成即读秒重启安装，读秒期间随时可以取消
- **单实例运行** — 同时只能开启一个 Acta Handy，重复启动会把已有实例的设置窗口带到前台
- **与 Acta 相同的十二套主题** — 鼠尾草、纸色护眼、黑白浅色 / 黑白深色、蓝黄、森林晨雾、海盐晚霞、糖果气泡、MWS 浅色 / MWS 深色、深夜霓虹、极光夜色，色板与 Acta 的主题选择一一对应，Handy 与 Acta 桌面同处一色；可跟随系统（浅鼠尾草、深黑白）或手动指定，深色系主题自动适配全部界面。界面字体（跟随系统 / 衬线 / 楷体 / 圆体）与字号大小可调；界面语言支持简体中文与 English
- **Windows 便携版数据随 exe 走** — 设置与窗口位置保存在便携版 exe 同目录的 `handy-settings.json`，修改历史与 WebView2 数据（缓存等）同样收在 exe 目录内，全程不写 %APPDATA%；拷走 exe 所在文件夹即可带走全部数据（旧版本存在 AppData 的设置会在首次启动时自动迁移）

## 数据来源

Acta Handy 读取 Acta「设置 → 数据」中同步出的 **Acta 数据文件夹**（含 `acta-manifest.json`、`classifications.json`、`notes/`、`todos/` 的 V3 格式，兼容 V2 单文件笔记）。

写入与 Acta 自身保存完全同构：条目文件保持 `{format, version, item}` 封套、`updatedAt` 使用 ISO 8601 UTC、manifest 最后重写（`syncedAt` 与对应条目的 `updatedAt`），因此 Acta 重新打开或同步时可以无缝接续。勾选待办遵循 Acta 的完成规则——勾选主待办会同步所有子任务，子任务全部完成后待办自动视为完成。编辑器输入停顿约一秒后自动保存，无需手动操作。

写入前的条目状态会快照到修改历史（存放在应用自己的数据目录——Windows 便携版与设置同目录、随 exe 走——不进 Acta 数据文件夹）。在设置窗口的「数据源」页可以查看每次修改并一键恢复；恢复本身也是一次写入，同样留档。

小组件以自动刷新（默认 30 秒，可调）跟随 Acta 的最近同步；正在输入时刷新会自动让路，不会打断编辑。

## 使用

1. 在 Acta 中把行记数据同步到任意本地文件夹；
2. 打开 Acta Handy，在「数据源」中选择该文件夹；
3. 在「待办小组件 / 笔记小组件 / Handy」中启用你需要的窗口，拖动即可摆放位置，位置与大小会自动记住；开启「吸附屏幕边缘」后，拖到边缘附近松手会自动贴合；
4. 左键点击 Handy 就地弹出快速编辑面板，在面板里即可勾选待办、编辑笔记；开启「吸附屏幕边缘」后，他会探头扒在屏幕边缘，光标靠近时跳出来；「隐匿模式」让他在闲置后悄悄淡出，光标靠近再回来；右键 Handy 可随时无极调整大小或关闭。

菜单栏（macOS，Handy 剪影的模板图标，随菜单栏深浅自动染成白色或黑色；Windows 为系统托盘，彩色图标）可以随时打开设置、开关各个小组件，或退出应用。

## 更新

应用内自带更新，入口在 设置 → 关于 →「检查更新」。

- **更新通道**：稳定通道（Release）只收 `x.y.z`；测试通道（Pre-Release）连 `x.y.z-betax` 一起收。
- **下载源**：GitHub Raw 为原生直链，GitHub-proxy 经 gh-proxy 加速。「测速」拿最近一次发布里当前平台的安装包做样本，两条链路各读 1 MB 计时，标出更快的一条。
- **安装**：确认下载后，下载完成即开始读秒，归零自动重启安装；读秒期间可以「取消自动安装」，停在就绪态再决定何时重启。换包由应用自己的第二个进程（`--handy-apply-update`）在主窗口退出后完成——Windows 替换 exe 本身（旧的改名为 `*.handy-old`，下次启动清理），macOS 替换 `.app` 并自动清除隔离标记。

每个版本的改动都记在 [CHANGELOG.md](CHANGELOG.md)。

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

版本号只写在 `src-tauri/tauri.conf.json`（改版本时 `package.json` 与 `src-tauri/Cargo.toml` 跟着同步，这属于发版操作的一部分）。推送到 `main` 后由 `.github/workflows/release.yml` 按版本号发版：

| 版本号 | 发布 |
| --- | --- |
| `1.1.0`（不带后缀） | 正式 Release，标记 latest |
| `1.1.0-beta1`（带后缀） | Pre-Release |

流水线分三步：读版本、定 tag 与渠道 → 三平台并行构建 → 汇总产物、从 CHANGELOG 摘出该版本段落发版。该版本的 Release 已存在时整条流水线跳过，日常提交不会重复构建；Actions 页面手动触发视为强制重发。

产物固定为三个平台的压缩包，名字 `ActaHandy-<版本>-<平台>-<架构>.zip`，包内只放单个安装文件：

| 平台 | 压缩包 | 包内 |
| --- | --- | --- |
| Windows x64 | `ActaHandy-<版本>-windows-x64.zip` | `ActaHandy.exe`（便携版单文件） |
| macOS Apple Silicon | `ActaHandy-<版本>-macos-arm64.zip` | `ActaHandy-<版本>-macos-arm64.dmg` |
| macOS Intel | `ActaHandy-<版本>-macos-x64.zip` | `ActaHandy-<版本>-macos-x64.dmg` |

macOS 的 dmg 里是 `.app`、Applications 软链，「修复启动损坏.command」解锁脚本与「损坏修复说明.txt」；这套命名同时是应用内更新识别平台包的依据（见 `src-tauri/src/updater.rs` 的 `asset_suffix`）。

`.github/workflows/build.yml` 只做按需的构建验证：手动触发，产出同样的三份压缩包挂在 Actions 的 Artifacts 上，不发 Release、不打 tag。

本地单独打某一份产物：

```bash
npm run windows:portable                      # Windows 便携版 exe
npm run macos:app                             # macOS arm64 的 .app
bash scripts/dmg/package-dmg.sh 1.1.0 arm64   # 连解锁脚本一起打成 dmg
```

## 许可

[MIT](LICENSE) © MogroWang Studio。Acta Handy 是 Acta 的独立伴生应用，界面与数据格式与 Acta 保持一致。
