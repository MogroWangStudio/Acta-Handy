//! Acta Handy — desktop widgets and a floating HUD for Acta's 行记 data.

mod acta;
mod history;
mod settings;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow, WindowEvent};

use settings::HandySettings;

const TRAY_ID: &str = "acta-handy-tray";
/// 几何需要持久化的窗口（设置窗口关闭只是隐藏，位置与大小同样记忆）。
const GEOMETRY_LABELS: [&str; 4] = ["main", "todo-widget", "notes-widget", "hud"];
/// 仅 Windows 的图标修复使用；非 Windows 编译时视为保留。
#[allow(dead_code)]
const ALL_WINDOWS: [&str; 4] = ["main", "todo-widget", "notes-widget", "hud"];

/// HUD 窗口（Handy 本体）的形态。`Free` 是站在桌面上；`Peek` 是贴边探头
/// （身体一部分探出屏幕被裁掉）。快速编辑面板与右键菜单在独立的 hud-panel
/// 窗口展开，不属于这里的形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudMode {
    Free,
    Peek,
}

impl HudMode {
    fn to_id(self) -> u32 {
        match self {
            HudMode::Free => 0,
            HudMode::Peek => 1,
        }
    }

    fn from_id(id: u32) -> Self {
        match id {
            1 => HudMode::Peek,
            _ => HudMode::Free,
        }
    }
}

// 当前 HUD 形态与面板停靠信息：位置映射（面板开合时 Handy 原地不动）和
// 几何持久化都要读它们。
static HUD_MODE: AtomicU32 = AtomicU32::new(0);
/// 面板开合与右键菜单都在独立的 hud-panel 窗口里进行：Handy 窗口只在
/// 自由 / 探头两种形态间切换，开合面板时纹丝不动。
static PANEL_OPEN: AtomicBool = AtomicBool::new(false);
/// 面板窗口当前内容：0 = 快速编辑面板，1 = 右键菜单。
static PANEL_MENU: AtomicBool = AtomicBool::new(false);
/// 贴边过渡动画播放中：期间设置应用与几何持久化全部让路。
static HUD_ANIMATING: AtomicBool = AtomicBool::new(false);
/// 滑块拖动中的实时缩放（×1000；0 表示无实时值，读设置文件）。
pub(crate) static HUD_LIVE_SCALE: AtomicU32 = AtomicU32::new(0);
/// 最近一次生效的缩放（×1000）：滑块连续拖动时用来反推旧窗口几何。
pub(crate) static HUD_LAST_SCALE: AtomicU32 = AtomicU32::new(0);

/// While true, a watcher thread polls the cursor and wakes the HUD when the
/// pointer comes near (stealth mode keeps the window invisible otherwise).
static HUD_WATCH: AtomicBool = AtomicBool::new(false);
static HUD_WATCH_RUNNING: AtomicBool = AtomicBool::new(false);

fn hud_mode() -> HudMode {
    HudMode::from_id(HUD_MODE.load(Ordering::SeqCst))
}

fn hud_scale(app: &AppHandle) -> f64 {
    let live = HUD_LIVE_SCALE.load(Ordering::SeqCst);
    if live > 0 {
        return live as f64 / 1000.0;
    }
    settings::load_from_disk(app).hud.scale
}

/// Handy 窗口（Handy 本体）各形态的逻辑尺寸。面板 / 菜单在独立的 hud-panel
/// 窗口，不再占 Handy 的框架。
fn hud_sizes(mode: HudMode, s: f64) -> (f64, f64) {
    match mode {
        HudMode::Free => (92.0 * s, 136.0 * s),
        HudMode::Peek => (50.0 * s, 146.0 * s),
    }
}

/// hud-panel 窗口（快速编辑面板 / 右键菜单共用）的逻辑尺寸：内容卡片四周
/// 留 8px 呼吸边（阴影与圆角），卡片 CSS inset: 8px。
fn panel_sizes(menu: bool) -> (f64, f64) {
    if menu {
        (240.0, 200.0)
    } else {
        (318.0, 470.0)
    }
}

fn side_str(handy_left: bool) -> &'static str {
    if handy_left {
        "left"
    } else {
        "right"
    }
}

// --- 原子窗口框架 -----------------------------------------------------------
//
// 面板开合要同时改窗口的位置与尺寸。分开调用 set_size + set_position 时窗
// 口框架分两步变化，Handy 会在中间帧错位（表现为弹出面板时闪烁）；这里用
// 平台原生调用一次完成框架变化：Windows 的 SetWindowPos、macOS 的 NSWindow
// setFrame:（AppKit 坐标 y 自屏幕底部向上，与 Tauri 的顶部原点相反，用「与
// 当前逻辑位置的差值」换算，免掉显式的跨屏坐标换算）。

#[cfg(windows)]
fn set_window_frame(
    window: &WebviewWindow,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
) {
    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(
            hwnd: isize,
            after: isize,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = SetWindowPos(
                hwnd.0 as isize,
                0,
                (x * scale).round() as i32,
                (y * scale).round() as i32,
                (width * scale).round() as i32,
                (height * scale).round() as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod mac_frame {
    use std::ffi::{c_char, c_void};

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct NSPoint {
        pub x: f64,
        pub y: f64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct NSSize {
        pub width: f64,
        pub height: f64,
    }
    /// AppKit 的窗口框架：原点在左下、y 向上（点 = 逻辑像素）。
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct NSRect {
        pub origin: NSPoint,
        pub size: NSSize,
    }

    extern "C" {
        fn sel_registerName(name: *const c_char) -> *const c_void;
        #[link_name = "objc_msgSend"]
        fn msg_send_frame(receiver: *mut c_void, sel: *const c_void) -> NSRect;
        // objc_msgSend 本就按调用方签名声明（ObjC 惯例），两处签名不同是预期。
        #[allow(clashing_extern_declarations)]
        #[link_name = "objc_msgSend"]
        fn msg_send_set_frame(receiver: *mut c_void, sel: *const c_void, frame: NSRect, display: bool);
    }

    pub fn current(ns_window: *mut c_void) -> NSRect {
        unsafe {
            let sel = sel_registerName(b"frame\0".as_ptr() as *const c_char);
            msg_send_frame(ns_window, sel)
        }
    }

    pub fn set(ns_window: *mut c_void, frame: NSRect) {
        unsafe {
            let sel = sel_registerName(b"setFrame:display:\0".as_ptr() as *const c_char);
            msg_send_set_frame(ns_window, sel, frame, true);
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn set_window_frame(
    window: &WebviewWindow,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
) {
    let Ok(pos) = window.outer_position() else { return };
    let cur_x = pos.x as f64 / scale;
    let cur_y = pos.y as f64 / scale;
    let Ok(ns_window) = window.ns_window() else { return };
    if ns_window.is_null() {
        return;
    }
    let cur = mac_frame::current(ns_window);
    let frame = mac_frame::NSRect {
        origin: mac_frame::NSPoint {
            x: cur.origin.x + (x - cur_x),
            y: cur.origin.y - (y - cur_y),
        },
        size: mac_frame::NSSize { width, height },
    };
    mac_frame::set(ns_window, frame);
}

/// 其他 macOS 架构（未随发布构建）：退回两步调用，位置仍正确，仅可能闪烁。
#[cfg(all(target_os = "macos", not(target_arch = "aarch64")))]
fn set_window_frame(
    window: &WebviewWindow,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    _scale: f64,
) {
    let _ = window.set_size(LogicalSize::new(width, height));
    let _ = window.set_position(LogicalPosition::new(x, y));
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HudPlacement {
    /// Handy 站在窗口的哪一侧（也是贴边方向）。
    side: &'static str,
    /// 面板形态下 Handy 脚底离窗口底部的逻辑像素数。
    lift: f64,
}

/// 把 monitor 尺寸换算成逻辑坐标矩形。
fn monitor_rect(m: &tauri::Monitor) -> (f64, f64, f64, f64) {
    let scale = m.scale_factor();
    (
        m.position().x as f64 / scale,
        m.position().y as f64 / scale,
        m.size().width as f64 / scale,
        m.size().height as f64 / scale,
    )
}

#[tauri::command]
fn read_acta_data(folder: String) -> Result<acta::ActaData, String> {
    if folder.trim().is_empty() {
        return Err("尚未选择 Acta 数据文件夹".to_string());
    }
    acta::read_data_folder(std::path::Path::new(&folder))
}

/// Acta 客户端运行时可能正持有数据文件句柄（Windows 上表现为共享冲突），
/// 写入按退避节奏短暂重试，等它放手后再落盘；仍失败才把原因交还前端。
fn with_write_retry<T>(op: impl Fn() -> Result<T, String>) -> Result<T, String> {
    let mut last = String::new();
    for attempt in 0..4 {
        match op() {
            Ok(value) => return Ok(value),
            Err(e) => {
                last = e;
                if attempt < 3 {
                    std::thread::sleep(Duration::from_millis(120 << attempt));
                }
            }
        }
    }
    Err(format!(
        "{last}\n文件可能正被 Acta 占用（Acta 正在同步或写入），请稍后重试。"
    ))
}

#[tauri::command]
fn write_todo_check(
    app: AppHandle,
    folder: String,
    patch: acta::TodoCheckPatch,
) -> Result<acta::ActaTodo, String> {
    if folder.trim().is_empty() {
        return Err("尚未选择 Acta 数据文件夹".to_string());
    }
    let (todo, before) =
        with_write_retry(|| acta::write_todo_check(std::path::Path::new(&folder), &patch))?;
    // 修改历史：记下写入前的完整待办，供「数据源」页回溯恢复。
    history::record(&app, &folder, "todo-check", &patch.todo_id, &todo.title, Some(todo.completed), Some(before));
    let _ = app.emit("acta-data-changed", ());
    Ok(todo)
}

#[tauri::command]
fn write_note(
    app: AppHandle,
    folder: String,
    patch: acta::NotePatch,
) -> Result<acta::ActaNote, String> {
    if folder.trim().is_empty() {
        return Err("尚未选择 Acta 数据文件夹".to_string());
    }
    let (note, before) =
        with_write_retry(|| acta::write_note(std::path::Path::new(&folder), &patch))?;
    // 新建没有「之前」可记；编辑记下写入前的标题、正文。
    let kind = if before.is_some() { "note-edit" } else { "note-create" };
    history::record(&app, &folder, kind, &note.id, &note.title, None, before);
    let _ = app.emit("acta-data-changed", ());
    Ok(note)
}

#[tauri::command]
fn read_history(app: AppHandle, folder: String) -> Vec<history::HistoryEntry> {
    history::load(&app, &folder)
}

/// 把一条历史恢复回写入前的模样；恢复本身也是一次写入，同样留档。
#[tauri::command]
fn restore_history(app: AppHandle, folder: String, entry_id: String) -> Result<(), String> {
    if folder.trim().is_empty() {
        return Err("尚未选择 Acta 数据文件夹".to_string());
    }
    let entry = history::find(&app, &entry_id).ok_or("未找到这条历史记录")?;
    if entry.folder != folder {
        return Err("这条历史来自另一个数据文件夹".to_string());
    }
    let Some(before) = entry.before else {
        return Err("新建的条目没有更早的状态可以恢复".to_string());
    };
    let kind = match entry.kind.as_str() {
        "todo-check" | "todo-restore" => "todos",
        "note-edit" | "note-create" | "note-restore" => "notes",
        other => return Err(format!("未知的历史类型：{other}")),
    };
    let previous = with_write_retry(|| {
        acta::restore_item(std::path::Path::new(&folder), kind, &entry.item_id, &before)
    })?;
    // 恢复后的完成状态 = 快照里的 completed（todo）；恢复动作自身记一条。
    let restored_completed = before.item.get("completed").and_then(serde_json::Value::as_bool);
    let restore_kind = if kind == "todos" { "todo-restore" } else { "note-restore" };
    let title = jstr_title(&before.item);
    history::record(&app, &folder, restore_kind, &entry.item_id, &title, restored_completed, previous);
    let _ = app.emit("acta-data-changed", ());
    Ok(())
}

/// 快照 item 的标题（todo 的 `title` / note 的 `title`）。
fn jstr_title(item: &serde_json::Value) -> String {
    item.get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[tauri::command]
fn refresh_data(app: AppHandle) {
    let _ = app.emit("acta-data-changed", ());
}

#[tauri::command]
fn show_window(app: AppHandle, label: String) {
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// hud-panel 窗口的落位：以 Handy 窗口当前位姿为锚——卡片贴着 Handy 图形
/// 朝桌面内侧展开（留 8px 呼吸缝），窗口底对齐脚底附近，越界时收进屏幕。
/// Handy 窗口本身永远不动。返回 (x, y, dock_left)；dock_left 为 Handy 是否
/// 贴在屏幕 / 卡片的左侧。
fn panel_layout(app: &AppHandle, menu: bool) -> Option<(f64, f64, bool)> {
    let window = app.get_webview_window("hud")?;
    let m = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let s = hud_scale(app);
    let scale = m.scale_factor();
    let (mx, my, mw, mh) = monitor_rect(&m);
    let pos = window.outer_position().ok()?;
    let (hx, hy) = (pos.x as f64 / scale, pos.y as f64 / scale);
    let peek = hud_mode() == HudMode::Peek;
    let (pw, ph) = hud_sizes(HudMode::Peek, s);
    let (fw, fh) = hud_sizes(HudMode::Free, s);
    // 停靠边 = Handy 在屏幕的左半还是右半，面板朝另一侧展开。
    let dock_left = if peek {
        hx + pw / 2.0 < mx + mw / 2.0
    } else {
        hx + fw / 2.0 < mx + mw / 2.0
    };
    // Handy 图形在窗口内的左右缘：free 站位 14s–78s；探头贴左缘时 -18s–50s、
    // 贴右缘时 4s–72s（身体探出窗口、被屏幕边缘裁掉的那侧）。
    let (graphic_l, graphic_r) = if peek {
        if dock_left {
            (-18.0 * s, 50.0 * s)
        } else {
            (4.0 * s, 72.0 * s)
        }
    } else {
        (14.0 * s, 78.0 * s)
    };
    let (w, h) = panel_sizes(menu);
    let gap = 8.0;
    let mut x = if dock_left {
        hx + graphic_r + gap
    } else {
        hx + graphic_l - gap - w
    };
    // 脚底：探头在窗口底上方 8s，自由站在窗口底上方 5s。卡片窗口底落在
    // 脚底下 8px（CSS inset 的呼吸边），视觉上卡片与脚底平齐。
    let feet_y = if peek { hy + ph - 8.0 * s } else { hy + fh - 5.0 * s };
    let mut y = feet_y + 8.0 - h;
    x = x.clamp(mx + 4.0, (mx + mw - w - 4.0).max(mx + 4.0));
    y = y.clamp(my + 12.0, (my + mh - h - 12.0).max(my + 12.0));
    Some((x, y, dock_left))
}

/// 显示 hud-panel 窗口（快速编辑面板或右键菜单）。`focus`：菜单要抢焦点
/// （点外失焦收起），面板绝不抢焦点（不打断正在打字的用户）。
fn show_panel(app: &AppHandle, menu: bool, focus: bool) {
    let Some(window) = app.get_webview_window("hud-panel") else {
        return;
    };
    let Some((x, y, dock_left)) = panel_layout(app, menu) else {
        return;
    };
    PANEL_MENU.store(menu, Ordering::SeqCst);
    PANEL_OPEN.store(true, Ordering::SeqCst);
    let side = side_str(dock_left);
    let _ = app.emit(
        "hud-panel",
        serde_json::json!({ "shown": true, "kind": if menu { "menu" } else { "panel" }, "side": side }),
    );
    let m = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let scale = m.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (w, h) = panel_sizes(menu);
    set_window_frame(&window, x, y, w, h, scale);
    let _ = window.show();
    if focus {
        let _ = window.set_focus();
    }
}

fn hide_panel(app: &AppHandle) {
    PANEL_OPEN.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("hud-panel") {
        let _ = window.hide();
    }
    // 关闭同样要广播：Handy 窗口的「面板开着」状态全靠这个事件归位，
    // 漏了它面板就成了「一次性」——收起后再也唤不出来。
    let kind = if PANEL_MENU.load(Ordering::SeqCst) { "menu" } else { "panel" };
    let _ = app.emit("hud-panel", serde_json::json!({ "shown": false, "kind": kind, "side": "right" }));
}

/// 面板开着时的重锚：Handy 位姿变了（缩放 / 吸附切换），面板窗口按新位姿
/// 重新定位。内容与尺寸不变，只动位置。
fn reposition_panel(app: &AppHandle) {
    if !PANEL_OPEN.load(Ordering::SeqCst) {
        return;
    }
    let Some(window) = app.get_webview_window("hud-panel") else {
        return;
    };
    let Some((x, y, _dock_left)) = panel_layout(app, PANEL_MENU.load(Ordering::SeqCst)) else {
        return;
    };
    let m = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let scale = m.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (w, h) = panel_sizes(PANEL_MENU.load(Ordering::SeqCst));
    set_window_frame(&window, x, y, w, h, scale);
}

/// 把 HUD 窗口调整为指定形态并返回停靠信息。`restore` 表示使用设置里的
/// 记忆位置（启动 / 设置变更）；否则保持实时位置做形态切换 / 面板开合。
fn apply_hud(app: &AppHandle, mode: HudMode, restore: bool) -> Option<HudPlacement> {
    let window = app.get_webview_window("hud")?;
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let s = hud_scale(app);
    let (width, height) = hud_sizes(mode, s);
    let scale = monitor
        .as_ref()
        .map(|m| m.scale_factor())
        .or_else(|| window.scale_factor().ok())
        .unwrap_or(1.0);
    let (cur_x, cur_y) = match window.outer_position().ok() {
        Some(p) => (Some(p.x as f64 / scale), Some(p.y as f64 / scale)),
        None => (None, None),
    };

    let Some(m) = monitor else {
        let _ = window.set_size(LogicalSize::new(width, height));
        HUD_MODE.store(mode.to_id(), Ordering::SeqCst);
        return None;
    };
    let (mx, my, mw, mh) = monitor_rect(&m);
    let default = default_position(app, width, 0.0);

    let (x, y, placement) = match mode {
        HudMode::Free => {
            let (x, y) = if restore {
                let hud = settings::load_from_disk(app).hud;
                match (hud.x, hud.y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                }
            } else {
                match (cur_x, cur_y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                }
            };
            (x, y, None)
        }
        HudMode::Peek => {
            let hud = settings::load_from_disk(app).hud;
            let base_x = if restore { hud.x.or(cur_x) } else { cur_x };
            let dock_left = match base_x {
                Some(a) => a + width / 2.0 < mx + mw / 2.0,
                None => true,
            };
            let x = if dock_left { mx } else { mx + mw - width };
            let top = if restore { hud.y.or(cur_y) } else { cur_y }.unwrap_or(my + 20.0);
            let y = top.clamp(my + 12.0, (my + mh - height - 12.0).max(my + 12.0));
            (x, y, Some(HudPlacement { side: side_str(dock_left), lift: 8.0 * s }))
        }
    };

    set_window_frame(&window, x, y, width, height, scale);
    let _ = window.show();
    HUD_MODE.store(mode.to_id(), Ordering::SeqCst);
    placement
}

#[tauri::command]
fn set_hud_mode(app: AppHandle, mode: String) -> Option<HudPlacement> {
    if HUD_ANIMATING.load(Ordering::SeqCst) {
        return None;
    }
    let mode = if mode == "peek" { HudMode::Peek } else { HudMode::Free };
    apply_hud(&app, mode, false)
}

/// 展开快速编辑面板（Handy 旁边的独立窗口）：Handy 窗口纹丝不动，面板
/// 窗口以 Handy 位姿为锚定位显示。动画 / 重复调用时忽略。
#[tauri::command]
fn set_hud_panel(app: AppHandle, shown: bool) {
    if !shown {
        hide_panel(&app);
        return;
    }
    if HUD_ANIMATING.load(Ordering::SeqCst) {
        return;
    }
    if PANEL_OPEN.load(Ordering::SeqCst) {
        // 状态漂移防御：后端记着「开着」但窗口实际看不见（收起命令丢失、
        // 或异常路径漏了归位），就当没开过重新展开——面板永不因此打不开。
        let visible = app
            .get_webview_window("hud-panel")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);
        if visible {
            return;
        }
    }
    show_panel(&app, false, false);
}

/// 滑块拖动中的实时缩放：窗口尺寸与站位立即跟随，落盘在 commit 时一次完成。
#[tauri::command]
fn set_hud_scale(app: AppHandle, scale: f64) -> Option<HudPlacement> {
    let new_raw = (scale.clamp(0.2, 1.5) * 1000.0).round() as u32;
    let old_raw = HUD_LIVE_SCALE.load(Ordering::SeqCst);
    HUD_LIVE_SCALE.store(new_raw, Ordering::SeqCst);
    // 实时广播给 HUD 窗口：窗口框架重排了，CSS 的 Handy 也要跟上同一档
    // 缩放，否则拖动滑块时看到的是「窗口变大、小人没变」的错位预览。
    if old_raw != new_raw {
        let _ = app.emit("hud-scale", new_raw as f64 / 1000.0);
    }
    if old_raw == new_raw || HUD_ANIMATING.load(Ordering::SeqCst) {
        return None;
    }
    let old_s = if old_raw == 0 {
        settings::load_from_disk(&app).hud.scale
    } else {
        old_raw as f64 / 1000.0
    };
    let window = app.get_webview_window("hud")?;
    let mode = hud_mode();
    // 自由 / 探头形态：以脚底为锚缩放，Handy 站在原地长大或缩小。
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())?;
    let scale_factor = monitor.scale_factor();
    let (mx, my, mw, mh) = monitor_rect(&monitor);
    let pos = window.outer_position().ok()?;
    let (px, py) = (pos.x as f64 / scale_factor, pos.y as f64 / scale_factor);
    let new_s = new_raw as f64 / 1000.0;
    let (x, y) = match mode {
        HudMode::Peek => {
            let (pw, ph) = hud_sizes(HudMode::Peek, old_s);
            let (nw, nh) = hud_sizes(HudMode::Peek, new_s);
            let dock_left = px + pw / 2.0 < mx + mw / 2.0;
            let feet_y = py + ph - 8.0 * old_s;
            let y = (feet_y - (nh - 8.0 * new_s)).clamp(my + 12.0, (my + mh - nh - 12.0).max(my + 12.0));
            let x = if dock_left { mx } else { mx + mw - nw };
            (x, y)
        }
        _ => {
            let (pw, ph) = hud_sizes(HudMode::Free, old_s);
            let (nw, nh) = hud_sizes(HudMode::Free, new_s);
            let feet_x = px + pw / 2.0;
            let feet_y = py + ph - 5.0 * old_s;
            let x = feet_x - nw / 2.0;
            let y = feet_y - (nh - 5.0 * new_s);
            (x, y)
        }
    };
    let (w, h) = hud_sizes(mode, new_s);
    set_window_frame(&window, x, y, w, h, scale_factor);
    // 菜单开着时（滑块就在菜单里）：菜单窗口跟着 Handy 的新位姿重锚。
    if PANEL_OPEN.load(Ordering::SeqCst) {
        reposition_panel(&app);
    }
    None
}

/// 滑块松手：把实时缩落盘并广播（清掉实时值，之后以设置文件为准）。
#[tauri::command]
fn commit_hud_scale(app: AppHandle) {
    let raw = HUD_LIVE_SCALE.swap(0, Ordering::SeqCst);
    if raw == 0 {
        return;
    }
    let mut s = settings::load_from_disk(&app);
    s.hud.scale = raw as f64 / 1000.0;
    HUD_LAST_SCALE.store(raw, Ordering::SeqCst);
    let _ = settings::save_to_disk(&app, &s);
    let _ = app.emit("hud-scale", raw as f64 / 1000.0);
    let _ = app.emit("settings-changed", &s);
}

/// 右键菜单里的「关闭 Handy」；重新开启后窗口以吸附 / 自由形态回来。
#[tauri::command]
fn set_hud_enabled(app: AppHandle, enabled: bool) {
    let mut s = settings::load_from_disk(&app);
    s.hud.enabled = enabled;
    let _ = settings::save_to_disk(&app, &s);
    apply_windows(&app, &s); // 先应用（动画 start 先发），再广播，时序同 save_settings。
    let _ = app.emit("settings-changed", &s);
}

// --- 贴边过渡动画 -----------------------------------------------------------
//
// 开启吸附时：Handy 保持站姿、朝目标边探身，先缓入缓出地走到屏幕边缘（身
// 体完整可见），再加速钻出屏幕，最后以探头位姿贴边出现；关闭时反向——以
// 站姿出现在边缘，滑回记忆位置。播放期间 HUD_ANIMATING 为真，期间落下的设
// 置变更在动画结束时统一生效。

fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn ease_in_cubic(t: f64) -> f64 {
    t * t * t
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn animate_window_to(
    window: &WebviewWindow,
    from: (f64, f64),
    to: (f64, f64),
    ms: u64,
    ease: fn(f64) -> f64,
) {
    let dur = Duration::from_millis(ms);
    let t0 = Instant::now();
    loop {
        let t = (t0.elapsed().as_secs_f64() / dur.as_secs_f64()).min(1.0);
        let e = ease(t);
        let _ = window.set_position(LogicalPosition::new(
            from.0 + (to.0 - from.0) * e,
            from.1 + (to.1 - from.1) * e,
        ));
        if t >= 1.0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// 滑向屏幕边缘的两段式：前段缓入缓出走到贴边，后段加速钻出——身体被屏幕
/// 边缘裁切的时间被压到最短，观感是「钻进边缘」而不是「被切掉」。
fn animate_window_depart(
    window: &WebviewWindow,
    from: (f64, f64),
    edge: (f64, f64),
    off: (f64, f64),
    ms: u64,
) {
    let split = 0.62;
    let dur = Duration::from_millis(ms);
    let t0 = Instant::now();
    loop {
        let t = (t0.elapsed().as_secs_f64() / dur.as_secs_f64()).min(1.0);
        let ((tx, ty), e) = if t < split {
            (edge, ease_in_out_cubic(t / split))
        } else {
            (off, ease_in_cubic((t - split) / (1.0 - split)))
        };
        let _ = window.set_position(LogicalPosition::new(
            from.0 + (tx - from.0) * e,
            from.1 + (ty - from.1) * e,
        ));
        if t >= 1.0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn spawn_snap_animation(app: &AppHandle, target: HudMode) {
    if HUD_ANIMATING.swap(true, Ordering::SeqCst) {
        return;
    }
    let to_peek = target == HudMode::Peek;
    let bail = |app: &AppHandle| {
        HUD_ANIMATING.store(false, Ordering::SeqCst);
        let _ = app.emit("hud-anim", serde_json::json!({ "phase": "end" }));
    };
    // 几何与方向在这里算好，start 也同步发出：它必须赶在「设置变更」广播
    // 之前到达前端（本函数可能从 save_settings 路径进入），animating 先置
    // 位，前端才不会抢先切换形态、让 Handy 在旧窗口框架里错位闪现。
    let Some(window) = app.get_webview_window("hud") else {
        bail(app);
        return;
    };
    let s = hud_scale(app);
    let Some(m) = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten())
    else {
        bail(app);
        apply_windows(app, &settings::load_from_disk(app));
        return;
    };
    let scale = m.scale_factor();
    let (mx, my, mw, mh) = monitor_rect(&m);
    let (cur_x, cur_y) = match window.outer_position().ok() {
        Some(p) => (p.x as f64 / scale, p.y as f64 / scale),
        None => (mx, my),
    };
    let app = app.clone();

    if to_peek {
        let (w, _h) = hud_sizes(HudMode::Free, s);
        let dock_left = cur_x + w / 2.0 < mx + mw / 2.0;
        let side = side_str(dock_left);
        // start 即带方向：前端让 Handy 提前朝目标边探身，动画有身体语言。
        let _ = app.emit(
            "hud-anim",
            serde_json::json!({ "phase": "start", "to": "peek", "side": side }),
        );
        let (pw, ph) = hud_sizes(HudMode::Peek, s);
        let dock_x = if dock_left { mx } else { mx + mw - pw };
        let dock_y = cur_y.clamp(my + 12.0, (my + mh - ph - 12.0).max(my + 12.0));
        let edge_x = if dock_left { mx } else { mx + mw - w };
        let off_x = if dock_left { mx - w } else { mx + mw };
        std::thread::spawn(move || {
            animate_window_depart(&window, (cur_x, cur_y), (edge_x, cur_y), (off_x, cur_y), 720);
            let _ = app.emit(
                "hud-anim",
                serde_json::json!({ "phase": "reveal", "to": "peek", "side": side }),
            );
            set_window_frame(&window, dock_x, dock_y, pw, ph, scale);
            HUD_MODE.store(HudMode::Peek.to_id(), Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(480));
            std::thread::sleep(Duration::from_millis(140));
            HUD_ANIMATING.store(false, Ordering::SeqCst);
            let _ = app.emit("hud-anim", serde_json::json!({ "phase": "end" }));
            // 先把动画落点记进设置，再让动画期间的设置变更统一生效，避免回跳。
            persist_geometry(&app, "hud");
            apply_windows(&app, &settings::load_from_disk(&app));
        });
    } else {
        let (fw, fh) = hud_sizes(HudMode::Free, s);
        let dock_left = cur_x + 25.0 * s < mx + mw / 2.0;
        let side = side_str(dock_left);
        let _ = app.emit(
            "hud-anim",
            serde_json::json!({ "phase": "start", "to": "free", "side": side }),
        );
        let app2 = app.clone();
        let window2 = window.clone();
        std::thread::spawn(move || {
            let _ = app2.emit(
                "hud-anim",
                serde_json::json!({ "phase": "reveal", "to": "free", "side": side }),
            );
            set_window_frame(&window2, cur_x, cur_y, fw, fh, scale);
            HUD_MODE.store(HudMode::Free.to_id(), Ordering::SeqCst);
            let hud = settings::load_from_disk(&app2).hud;
            let target = match (hud.x, hud.y) {
                (Some(x), Some(y)) => (x, y),
                _ => {
                    let x = if dock_left { mx + 20.0 } else { mx + mw - fw - 20.0 };
                    let y = cur_y.clamp(my + 20.0, (my + mh - fh - 20.0).max(my + 20.0));
                    (x, y)
                }
            };
            animate_window_to(&window2, (cur_x, cur_y), target, 640, ease_out_cubic);
            std::thread::sleep(Duration::from_millis(140));
            HUD_ANIMATING.store(false, Ordering::SeqCst);
            let _ = app2.emit("hud-anim", serde_json::json!({ "phase": "end" }));
            persist_geometry(&app2, "hud");
            apply_windows(&app2, &settings::load_from_disk(&app2));
        });
    }
}

// --- Handy 右键菜单（应用内自绘，替代原生菜单） ------------------------------

#[tauri::command]
fn popup_hud_menu(app: AppHandle) {
    if HUD_ANIMATING.load(Ordering::SeqCst) {
        return;
    }
    if PANEL_OPEN.load(Ordering::SeqCst) {
        return;
    }
    // 菜单抢焦点：点外部失焦即收起（panel 窗口监听 blur），Esc 也能生效。
    show_panel(&app, true, true);
}

// --- eye tracking ------------------------------------------------------------
//
// 眼睛跟随光标：一个轻量线程按 32ms 轮询光标相对 Handy 的方向，只有方向
// 变化超过量化步长才广播，静止时不产生事件。

static HUD_EYE_WATCH: AtomicBool = AtomicBool::new(false);
static HUD_EYE_RUNNING: AtomicBool = AtomicBool::new(false);

#[tauri::command]
fn set_hud_eye_watch(app: AppHandle, watch: bool) {
    HUD_EYE_WATCH.store(watch, Ordering::SeqCst);
    if !watch || HUD_EYE_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut last: Option<(i32, i32)> = None;
        while HUD_EYE_WATCH.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(32));
            let Some(window) = app.get_webview_window("hud") else {
                break;
            };
            if !window.is_visible().unwrap_or(false) {
                break;
            }
            let (Ok(cursor), Ok(pos), Ok(size)) = (
                app.cursor_position(),
                window.outer_position(),
                window.outer_size(),
            ) else {
                continue;
            };
            let scale = window.scale_factor().unwrap_or(1.0);
            let center_x = pos.x as f64 + size.width as f64 / 2.0;
            let center_y = pos.y as f64 + size.height as f64 / 2.0;
            let dx = (cursor.x as f64 - center_x) / scale;
            let dy = (cursor.y as f64 - center_y) / scale;
            let nx = (dx / 420.0).clamp(-1.0, 1.0);
            let ny = (dy / 420.0).clamp(-1.0, 1.0);
            // 量化到 1/24 档：光标微微抖动不广播，视线保持安静。
            let key = ((nx * 24.0).round() as i32, (ny * 24.0).round() as i32);
            if last != Some(key) {
                last = Some(key);
                let _ = app.emit("hud-gaze", serde_json::json!({ "nx": nx, "ny": ny }));
            }
        }
        HUD_EYE_RUNNING.store(false, Ordering::SeqCst);
    });
}

/// Enable (or disable) the near-cursor watch. While active, a thread polls the
/// cursor and emits `hud-wake` once it comes within `pad` logical px of the
/// window — stealth mode uses a small pad to revive the hidden HUD, and the
/// peeking Handy uses a large one to jump out as the pointer approaches.
static HUD_WATCH_PAD: AtomicU32 = AtomicU32::new(26);

#[tauri::command]
fn set_hud_cursor_watch(app: AppHandle, watch: bool, pad: Option<f64>) {
    HUD_WATCH_PAD.store(pad.unwrap_or(26.0).round().clamp(8.0, 200.0) as u32, Ordering::SeqCst);
    HUD_WATCH.store(watch, Ordering::SeqCst);
    if !watch || HUD_WATCH_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        // 只在「由远及近」的上升沿发射一次事件：面板收起后光标若仍停在
        // 附近，不会立刻再次触发造成弹跳，需要先离开再靠近。启动时视为
        // 「已在附近」，光标原地不动就不会触发。
        let mut was_near = true;
        while HUD_WATCH.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(160));
            let Some(window) = app.get_webview_window("hud") else {
                break;
            };
            if !window.is_visible().unwrap_or(false) {
                break;
            }
            let (Ok(cursor), Ok(pos), Ok(size)) = (
                app.cursor_position(),
                window.outer_position(),
                window.outer_size(),
            ) else {
                continue;
            };
            let scale = window.scale_factor().unwrap_or(1.0);
            let pad = HUD_WATCH_PAD.load(Ordering::SeqCst) as f64 * scale;
            let near = (cursor.x as f64) >= pos.x as f64 - pad
                && (cursor.x as f64) <= pos.x as f64 + size.width as f64 + pad
                && (cursor.y as f64) >= pos.y as f64 - pad
                && (cursor.y as f64) <= pos.y as f64 + size.height as f64 + pad;
            if near && !was_near {
                let _ = app.emit("hud-wake", ());
            }
            was_near = near;
        }
        HUD_WATCH_RUNNING.store(false, Ordering::SeqCst);
    });
}

/// Make sure a restored position actually lands on a connected monitor;
/// otherwise fall back to the top-right default.
fn position_on_screen(
    app: &AppHandle,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<LogicalPosition<f64>> {
    let monitors = app.available_monitors().ok()?;
    if monitors.is_empty() {
        return None;
    }
    let visible = monitors.iter().any(|m| {
        let (mx, my, mw, mh) = monitor_rect(m);
        x + width > mx + 40.0 && x < mx + mw - 40.0 && y + height > my + 40.0 && y < my + mh - 40.0
    });
    if visible {
        None
    } else {
        Some(default_position(app, width, 0.0))
    }
}

fn default_position(app: &AppHandle, width: f64, offset: f64) -> LogicalPosition<f64> {
    if let Some(m) = app.primary_monitor().ok().flatten() {
        let (mx, my, mw, _mh) = monitor_rect(&m);
        let right = mx + mw - width - 20.0;
        let top = my + 20.0 + offset;
        return LogicalPosition::new(right.max(20.0), top);
    }
    LogicalPosition::new(60.0, 60.0)
}

// --- widget edge snapping ----------------------------------------------------

const SNAP_THRESHOLD: f64 = 48.0;
const SNAP_MARGIN: f64 = 8.0;
/// 探头形态松手时离两条竖边都超过这个距离，视为把 Handy 拖离边缘（解除吸附）。
const PEEK_RELEASE_PX: f64 = 120.0;

/// 磁吸：离屏幕边缘足够近时贴合到固定边距处（左右与上下各自判定）。
fn snap_to_edges(snap: bool, x: f64, y: f64, w: f64, h: f64, m: &tauri::Monitor) -> (f64, f64) {
    if !snap {
        return (x, y);
    }
    let (mx, my, mw, mh) = monitor_rect(m);
    let mut sx = x;
    if (x - (mx + SNAP_MARGIN)).abs() <= SNAP_THRESHOLD {
        sx = mx + SNAP_MARGIN;
    } else if ((mx + mw - SNAP_MARGIN) - (x + w)).abs() <= SNAP_THRESHOLD {
        sx = mx + mw - w - SNAP_MARGIN;
    }
    let mut sy = y;
    if (y - (my + SNAP_MARGIN)).abs() <= SNAP_THRESHOLD {
        sy = my + SNAP_MARGIN;
    } else if ((my + mh - SNAP_MARGIN) - (y + h)).abs() <= SNAP_THRESHOLD {
        sy = my + mh - h - SNAP_MARGIN;
    }
    (sx, sy)
}

fn widget_snap(app: &AppHandle, label: &str) -> bool {
    let s = settings::load_from_disk(app);
    match label {
        "todo-widget" => s.todo_widget.snap_to_edge,
        "notes-widget" => s.notes_widget.snap_to_edge,
        _ => false,
    }
}

/// 另一个小组件的当前逻辑矩形（可见才有意义）——互相吸附的目标。
fn other_widget_rect(app: &AppHandle, label: &str) -> Option<(f64, f64, f64, f64)> {
    let other = if label == "todo-widget" {
        "notes-widget"
    } else if label == "notes-widget" {
        "todo-widget"
    } else {
        return None;
    };
    let window = app.get_webview_window(other)?;
    if !window.is_visible().ok()? {
        return None;
    }
    let sc = window.scale_factor().ok()?;
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    Some((pos.x as f64 / sc, pos.y as f64 / sc, size.width as f64 / sc, size.height as f64 / sc))
}

/// 互相吸附：拖到另一个小组件身边（48px 内）时贴合到它的边（留 8px 缝隙）。
/// 只吸靠近的那条轴、另一轴保持拖动落点，不会把窗口瞬移到别处；
/// 水平 / 垂直命中都可能时取更近的一个。返回吸附后的位置，未命中 None。
fn snap_to_widget(x: f64, y: f64, w: f64, h: f64, other: (f64, f64, f64, f64)) -> Option<(f64, f64)> {
    let (ox, oy, ow, oh) = other;
    let v_overlap = y < oy + oh + SNAP_THRESHOLD && y + h > oy - SNAP_THRESHOLD;
    let h_overlap = x < ox + ow + SNAP_THRESHOLD && x + w > ox - SNAP_THRESHOLD;
    let mut best: Option<(f64, f64, f64)> = None; // (距离, x, y)
    let mut consider = |ax: f64, ay: f64| {
        let d = (ax - x).hypot(ay - y);
        if best.map_or(true, |(bd, _, _)| d < bd) {
            best = Some((d, ax, ay));
        }
    };
    if v_overlap {
        let rx = ox + ow + SNAP_MARGIN;
        if (rx - x).abs() <= SNAP_THRESHOLD {
            consider(rx, y);
        }
        let lx = ox - SNAP_MARGIN - w;
        if (lx - x).abs() <= SNAP_THRESHOLD {
            consider(lx, y);
        }
    }
    if h_overlap {
        let by = oy + oh + SNAP_MARGIN;
        if (by - y).abs() <= SNAP_THRESHOLD {
            consider(x, by);
        }
        let ty = oy - SNAP_MARGIN - h;
        if (ty - y).abs() <= SNAP_THRESHOLD {
            consider(x, ty);
        }
    }
    best.map(|(_, ax, ay)| (ax, ay))
}

fn apply_widget(
    app: &AppHandle,
    label: &str,
    enabled: bool,
    x: Option<f64>,
    y: Option<f64>,
    width: f64,
    height: f64,
    on_top: bool,
) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    if !enabled {
        let _ = window.hide();
        return;
    }
    let (width, height) = (width.max(200.0), height.max(100.0));
    let _ = window.set_size(LogicalSize::new(width, height));
    let snap = widget_snap(app, label);
    let pos = match (x, y) {
        (Some(x), Some(y)) => {
            let p = position_on_screen(app, x, y, width, height)
                .unwrap_or_else(|| LogicalPosition::new(x, y));
            let m = window
                .current_monitor()
                .ok()
                .flatten()
                .or_else(|| app.primary_monitor().ok().flatten());
            match m {
                Some(m) => {
                    let (sx, sy) = snap_to_edges(snap, p.x, p.y, width, height, &m);
                    LogicalPosition::new(sx, sy)
                }
                None => p,
            }
        }
        _ => {
            let offset = match label {
                "notes-widget" => height + 12.0,
                _ => 0.0,
            };
            default_position(app, width, offset)
        }
    };
    let _ = window.set_position(pos);
    let _ = window.set_always_on_top(on_top);
    let _ = window.show();
}

/// 设置窗口自身：恢复记忆的位置与大小（首次运行为默认 940×640）。
fn apply_main_window(app: &AppHandle, s: &HandySettings) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // 最大化 / 全屏时不打扰当前窗口状态：此时 set_size 会把窗口拽出最大化。
    if window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
        return;
    }
    // 窗口正开着就不再摆布几何：几何只在启动时恢复一次，之后随用户的
    // 拖动与缩放走。保存任何设置都重放一遍尺寸，窗口会被来回改动。
    if window.is_visible().unwrap_or(false) {
        return;
    }
    let width = s.window.width.max(860.0);
    let height = s.window.height.max(560.0);
    let _ = window.set_size(LogicalSize::new(width, height));
    if let (Some(x), Some(y)) = (s.window.x, s.window.y) {
        let pos = position_on_screen(app, x, y, width, height)
            .unwrap_or_else(|| LogicalPosition::new(x, y));
        let _ = window.set_position(pos);
    }
}

pub fn apply_windows(app: &AppHandle, s: &HandySettings) {
    apply_main_window(app, s);
    apply_widget(
        app,
        "todo-widget",
        s.todo_widget.enabled,
        s.todo_widget.x,
        s.todo_widget.y,
        s.todo_widget.width,
        s.todo_widget.height,
        s.todo_widget.always_on_top,
    );
    apply_widget(
        app,
        "notes-widget",
        s.notes_widget.enabled,
        s.notes_widget.x,
        s.notes_widget.y,
        s.notes_widget.width,
        s.notes_widget.height,
        s.notes_widget.always_on_top,
    );
    if !s.hud.enabled {
        HUD_WATCH.store(false, Ordering::SeqCst);
        hide_panel(app);
        // 记下目标形态，重新启用时从这里继续，不触发贴边动画。
        HUD_MODE.store(if s.hud.snap_to_edge { 1 } else { 0 }, Ordering::SeqCst);
        if let Some(window) = app.get_webview_window("hud") {
            let _ = window.hide();
        }
        return;
    }
    let mode = if s.hud.snap_to_edge {
        HudMode::Peek
    } else {
        HudMode::Free
    };
    if let Some(window) = app.get_webview_window("hud") {
        let _ = window.set_always_on_top(s.hud.always_on_top);
        if HUD_ANIMATING.load(Ordering::SeqCst) {
            return; // 贴边动画结束时统一重放当前设置
        }
        // 面板开着时改吸附设置：先收起面板再切换 Handy 形态，避免面板
        // 悬在一个突然换姿势的锚点上。
        if PANEL_OPEN.load(Ordering::SeqCst) && hud_mode() != mode {
            hide_panel(app);
        }
        if window.is_visible().unwrap_or(false) && hud_mode() != mode {
            spawn_snap_animation(app, mode);
            if PANEL_OPEN.load(Ordering::SeqCst) {
                reposition_panel(app);
            }
            return;
        }
    }
    apply_hud(app, mode, true);
    if PANEL_OPEN.load(Ordering::SeqCst) {
        reposition_panel(app);
    }
}

// --- geometry persistence ---------------------------------------------------

static PENDING_SAVES: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
const GEOMETRY_DEBOUNCE_MS: u128 = 900;

fn schedule_geometry_save(_app: &AppHandle, label: &str) {
    if !GEOMETRY_LABELS.contains(&label) {
        return;
    }
    // 只记录最近一次移动的时间：持续拖动会不断刷新计时，常驻调度线程要等
    // 停歇 900ms 才落盘。拖动中途落盘会与系统拖动抢窗口（探头形态表现为
    // 被强行拉回边缘），也会把中间位置写成记忆。
    PENDING_SAVES
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(label.to_string(), Instant::now());
}

fn start_geometry_saver(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(120));
        let due: Vec<String> = {
            let pending = PENDING_SAVES.lock().unwrap();
            match pending.as_ref() {
                Some(map) => map
                    .iter()
                    .filter(|(_, at)| at.elapsed().as_millis() >= GEOMETRY_DEBOUNCE_MS)
                    .map(|(label, _)| label.clone())
                    .collect(),
                None => Vec::new(),
            }
        };
        if due.is_empty() {
            continue;
        }
        {
            let mut pending = PENDING_SAVES.lock().unwrap();
            if let Some(map) = pending.as_mut() {
                for label in &due {
                    map.remove(label);
                }
            }
        }
        for label in due {
            persist_geometry(&app, &label);
        }
    });
}

fn persist_geometry(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    // 记内容尺寸（inner）：set_size 恢复的正是内容尺寸。macOS 上设置窗口
    // 有系统标题栏，外部尺寸比内容高出一个标题栏，存外部尺寸再放回内容，
    // 窗口每往返一次就长高一截。
    let (Ok(scale), Ok(pos), Ok(size)) =
        (window.scale_factor(), window.outer_position(), window.inner_size())
    else {
        return;
    };
    let mut s = settings::load_from_disk(app);
    let x = Some(pos.x as f64 / scale);
    let y = Some(pos.y as f64 / scale);
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;
    match label {
        "main" => {
            // 最大化 / 全屏时的窗口尺寸不代表用户偏好的还原尺寸：不落盘。
            // 否则一次最大化就会把记忆尺寸放大到满屏，之后每次应用设置都会
            // 把窗口撑到那么大。
            if window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
                return;
            }
            s.window.x = x;
            s.window.y = y;
            s.window.width = width;
            s.window.height = height;
        }
        "todo-widget" | "notes-widget" => {
            // 吸附开启：拖动停歇后先试着贴到另一个小组件身边，贴不上屏幕
            // 边缘再兜底，最后记忆贴合后的位置。
            let snap = widget_snap(app, label);
            let (mut px, mut py) = (x.unwrap_or(0.0), y.unwrap_or(0.0));
            if snap {
                let mut target = other_widget_rect(app, label).and_then(|other| {
                    snap_to_widget(px, py, width, height, other)
                });
                if target.is_none() {
                    if let Some(m) = window.current_monitor().ok().flatten() {
                        let (sx, sy) = snap_to_edges(true, px, py, width, height, &m);
                        if (sx - px).abs() > 0.5 || (sy - py).abs() > 0.5 {
                            target = Some((sx, sy));
                        }
                    }
                }
                if let Some((tx, ty)) = target {
                    // 互吸点可能被推到屏幕外：收回屏内再落位。
                    let (tx, ty) = match window.current_monitor().ok().flatten() {
                        Some(m) => {
                            let (mx, my, mw, mh) = monitor_rect(&m);
                            (
                                tx.clamp(mx, (mx + mw - width).max(mx)),
                                ty.clamp(my, (my + mh - height).max(my)),
                            )
                        }
                        None => (tx, ty),
                    };
                    let _ = window.set_position(LogicalPosition::new(tx, ty));
                    if let (Ok(p), Ok(sc)) = (window.outer_position(), window.scale_factor()) {
                        px = p.x as f64 / sc;
                        py = p.y as f64 / sc;
                    }
                }
            }
            let widget = if label == "todo-widget" {
                &mut s.todo_widget
            } else {
                &mut s.notes_widget
            };
            widget.x = Some(px);
            widget.y = Some(py);
            widget.width = width;
            widget.height = height;
        }
        "hud" => {
            if HUD_ANIMATING.load(Ordering::SeqCst) {
                return;
            }
            match hud_mode() {
                HudMode::Peek => {
                    // 探头形态松手后重新贴边：记住贴边位置，不记落点。
                    if let (Ok(scale), Ok(monitor)) = (window.scale_factor(), window.current_monitor())
                    {
                        if let Some(m) = monitor {
                            let (mx, _my, mw, _mh) = monitor_rect(&m);
                            let px = pos.x as f64 / scale;
                            let py = pos.y as f64 / scale;
                            let away =
                                px - mx > PEEK_RELEASE_PX && (mx + mw) - (px + width) > PEEK_RELEASE_PX;
                            if away {
                                // 拖离边缘：用户想把 Handy 拖出来站。解除吸附，
                                // 以自由形态站到落点——落盘后走吸附过渡动画，
                                // 把他从边缘自然带到落点。
                                s.hud.snap_to_edge = false;
                                s.hud.x = Some(px);
                                s.hud.y = Some(py);
                                let _ = settings::save_to_disk(app, &s);
                                apply_windows(app, &s); // 先应用（动画 start 先发），再广播。
                                let _ = app.emit("settings-changed", &s);
                                return;
                            }
                            let dock_left = px + width / 2.0 < mx + mw / 2.0;
                            let x = if dock_left { mx } else { mx + mw - width };
                            let _ = window.set_position(LogicalPosition::new(x, py));
                            if let Ok(pos) = window.outer_position() {
                                s.hud.x = Some(pos.x as f64 / scale);
                                s.hud.y = Some(pos.y as f64 / scale);
                            }
                        }
                    }
                }
                HudMode::Free => {
                    s.hud.x = x;
                    s.hud.y = y;
                }
            }
        }
        _ => return,
    }
    let _ = settings::save_to_disk(app, &s);
    // 几何是后端独自持有的真相：广播最新设置，前端快照不再过期，
    // 保存其它设置时也就不会把窗口尺寸带回旧值。
    let _ = app.emit("settings-changed", &s);
}

// --- data-change watcher ---------------------------------------------------

fn start_watch_thread(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_sig: Option<(i64, u32)> = None;
        let mut first_pass = true;
        loop {
            let interval = settings::load_from_disk(&app)
                .refresh_interval_secs
                .clamp(5, 3600) as u64;
            if !first_pass {
                std::thread::sleep(Duration::from_secs(interval));
            }
            let folder = settings::load_from_disk(&app).data_folder;
            let sig = if folder.is_empty() {
                None
            } else {
                acta::folder_signature(std::path::Path::new(&folder))
            };
            if !first_pass && sig != last_sig {
                let _ = app.emit("acta-data-changed", ());
            }
            last_sig = sig;
            first_pass = false;
        }
    });
}

// --- tray ------------------------------------------------------------------

fn tray_labels(zh: bool) -> (&'static str, &'static str, &'static str, &'static str, &'static str) {
    if zh {
        (
            "打开 Acta Handy 设置",
            "待办小组件",
            "笔记小组件",
            "Handy",
            "退出 Acta Handy",
        )
    } else {
        (
            "Open Acta Handy Settings",
            "Todo Widget",
            "Notes Widget",
            "Handy",
            "Quit Acta Handy",
        )
    }
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let s = settings::load_from_disk(app);
    let (open_txt, todo_txt, notes_txt, hud_txt, quit_txt) = tray_labels(s.is_chinese());
    let open = MenuItem::with_id(app, "open-settings", open_txt, true, None::<&str>)?;
    let todo = CheckMenuItem::with_id(
        app,
        "toggle-todo-widget",
        todo_txt,
        true,
        s.todo_widget.enabled,
        None::<&str>,
    )?;
    let notes = CheckMenuItem::with_id(
        app,
        "toggle-notes-widget",
        notes_txt,
        true,
        s.notes_widget.enabled,
        None::<&str>,
    )?;
    let hud = CheckMenuItem::with_id(
        app,
        "toggle-hud",
        hud_txt,
        true,
        s.hud.enabled,
        None::<&str>,
    )?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", quit_txt, true, None::<&str>)?;
    Menu::with_items(app, &[&open, &sep1, &todo, &notes, &hud, &sep2, &quit])
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Acta Handy")
        .on_menu_event(|app, event| handle_tray_event(app, event.id().as_ref()));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn handle_tray_event(app: &AppHandle, id: &str) {
    match id {
        "open-settings" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "quit" => app.exit(0),
        "toggle-todo-widget" | "toggle-notes-widget" | "toggle-hud" => {
            let mut s = settings::load_from_disk(app);
            match id {
                "toggle-todo-widget" => s.todo_widget.enabled = !s.todo_widget.enabled,
                "toggle-notes-widget" => s.notes_widget.enabled = !s.notes_widget.enabled,
                _ => s.hud.enabled = !s.hud.enabled,
            }
            let _ = settings::save_to_disk(app, &s);
            apply_windows(app, &s); // 先应用（动画 start 先发），再广播，时序同 save_settings。
            let _ = app.emit("settings-changed", &s);
            refresh_tray_menu(app);
        }
        _ => {}
    }
}

fn refresh_tray_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_tray_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

// --- app run ---------------------------------------------------------------

/// Tauri 在 Windows 上把 ico 的第一帧（本项目为 16×16）内嵌为默认窗口图标，
/// 且只设置小图标；任务栏取不到大图标时回退到它并放大显示，ico 内置多少
/// 尺寸都无济于事。启动时为每个窗口显式设置 256px 图标，让系统按需高质量
/// 缩小，任务栏不再发虚。
#[cfg(windows)]
fn fix_windows_window_icons(app: &AppHandle) {
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/128x128@2x.png"));
    if let Ok(icon) = icon {
        for label in ALL_WINDOWS {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.set_icon(icon.clone());
            }
        }
    }
}

/// Windows 便携化：WebView2 的用户数据（缓存、GPU 与站点数据）默认落系统盘
/// AppData。我们在窗口创建前指到 exe 同目录的 webview-data，整份软件拷走即
/// 全部数据随行——设置与修改历史本就在 exe 目录，这里补上最后一块。WebView2
/// 拿到空数据目录时会尊重该环境变量（官方 loader 机制）；exe 所在目录不可写
/// 时不动它，退回系统默认，宁可回落也不让 webview 建不出来。
#[cfg(windows)]
fn keep_webview_data_portable() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let Some(dir) = exe_dir else { return };
    let udf = dir.join("webview-data");
    if std::fs::create_dir_all(&udf).is_ok() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &udf);
    }
}

pub fn run() {
    #[cfg(windows)]
    keep_webview_data_portable();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二次启动：把已有实例的设置窗口带到前台。
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_acta_data,
            write_todo_check,
            write_note,
            read_history,
            restore_history,
            refresh_data,
            show_window,
            quit_app,
            set_hud_mode,
            set_hud_panel,
            set_hud_scale,
            commit_hud_scale,
            set_hud_enabled,
            set_hud_eye_watch,
            set_hud_cursor_watch,
            popup_hud_menu,
            settings::load_settings,
            settings::save_settings
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            #[cfg(windows)]
            fix_windows_window_icons(&handle);
            let s = settings::load_from_disk(&handle);
            apply_windows(&handle, &s);
            setup_tray(&handle)?;
            start_watch_thread(handle.clone());
            start_geometry_saver(handle.clone());

            // The settings window reveals itself once the UI is ready; if the
            // renderer never gets there, show it anyway.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(4));
                if let Some(window) = handle.get_webview_window("main") {
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                schedule_geometry_save(window.app_handle(), window.label());
            }
            WindowEvent::CloseRequested { api, .. } => {
                // Closing a window hides it; the tray's quit item exits for real.
                api.prevent_close();
                let _ = window.hide();
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building Acta Handy")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                if let Some(window) = _app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
