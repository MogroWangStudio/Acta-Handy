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

/// HUD shapes. `Free` is Handy standing on the desktop; `Peek` is the
/// edge-clinging pose (part of the body clipped beyond the screen edge);
/// `Panel` is the expanded quick-edit panel with Handy beside it; `Menu` is
/// the in-app context menu (a size slider + close, drawn by the webview).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudMode {
    Free,
    Peek,
    Panel,
    Menu,
}

impl HudMode {
    fn to_id(self) -> u32 {
        match self {
            HudMode::Free => 0,
            HudMode::Peek => 1,
            HudMode::Panel => 2,
            HudMode::Menu => 3,
        }
    }

    fn from_id(id: u32) -> Self {
        match id {
            1 => HudMode::Peek,
            2 => HudMode::Panel,
            3 => HudMode::Menu,
            _ => HudMode::Free,
        }
    }
}

// 当前 HUD 形态与面板停靠信息：位置映射（面板开合时 Handy 原地不动）和
// 几何持久化都要读它们。
static HUD_MODE: AtomicU32 = AtomicU32::new(0);
/// 面板 / 菜单形态里 Handy 是否站在窗口右侧（也是贴边停靠时的屏幕右缘一侧）。
static HUD_PANEL_SIDE_RIGHT: AtomicBool = AtomicBool::new(true);
/// 面板 / 菜单形态下 Handy 脚底离窗口底部的逻辑距离：面板向上展开被屏幕
/// 上缘截断时变大，Handy 的屏幕位置因此保持不动。
static HUD_PANEL_LIFT: Mutex<f64> = Mutex::new(5.0);
/// 自由形态打开面板 / 菜单时的 Handy 窗口位置：收起时站回这里。存的是打开
/// 前的精确逻辑坐标——收起若从「读回窗口坐标（物理整数）→ 反推 → 再写入」
/// 走，每一轮开合都会因取整往返净移约 1 物理像素并累积；用锚点恢复则恰好
/// 还原为开合前的同一物理像素，零位移。面板被拖动或重排时由几何持久化更新。
static HUD_ANCHOR: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// 贴边面板 / 菜单对应的探头锚点：收起时 Handy 原位钻回边缘的窗口位姿，
/// 语义同上（精确、不取整往返）。
static HUD_PEEK_ANCHOR: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// 贴边过渡动画播放中：期间设置应用与几何持久化全部让路。
static HUD_ANIMATING: AtomicBool = AtomicBool::new(false);
/// 面板 / 菜单是从探头形态打开的：布局与收起落点都按探头一侧处理（重排时
/// 保持不变）。
static HUD_PANEL_FROM_PEEK: AtomicBool = AtomicBool::new(false);
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

/// 各形态的窗口逻辑尺寸。面板 / 菜单宽度 = 固定卡片宽 + Handy 站位（随缩放
/// 加宽）；Handy 在面板 / 菜单里的站位与自由形态完全同偏移，开合时不挪位。
fn hud_sizes(mode: HudMode, s: f64) -> (f64, f64) {
    match mode {
        HudMode::Free => (92.0 * s, 136.0 * s),
        HudMode::Peek => (50.0 * s, 146.0 * s),
        HudMode::Panel => (310.0 + 82.0 * s, 470.0),
        // 菜单卡片不随缩放，窗口高度取「卡片高度」与「Handy 站高」的较大者。
        HudMode::Menu => (232.0 + 82.0 * s, (136.0 * s + 16.0).max(184.0)),
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

/// 自由面板布局：Handy 原地站好（fx, fy 为自由形态窗口位置），卡片朝桌面
/// 内侧展开、被屏幕边缘截断时抬高兜底。返回窗口位置、Handy 是否在右侧、lift。
#[allow(clippy::too_many_arguments)]
fn free_floating_layout(
    m: &tauri::Monitor,
    s: f64,
    width: f64,
    height: f64,
    fx: f64,
    fy: f64,
) -> (f64, f64, bool, f64) {
    let (mx, my, mw, mh) = monitor_rect(m);
    let (fw, fh) = hud_sizes(HudMode::Free, s);
    let right = fx + fw / 2.0 >= mx + mw / 2.0;
    let feet_y = fy + fh - 5.0 * s;
    let py = (fy + fh - height).clamp(my + 8.0, (my + mh - height - 8.0).max(my + 8.0));
    let base_x = if right { fx + fw - width } else { fx };
    let px = base_x.clamp(mx + 4.0, (mx + mw - width - 4.0).max(mx + 4.0));
    // lift 是 Handy 脚底离窗口底部的距离：脚底位置固定，窗口被屏幕截断时
    // 窗口底随之上移，lift 随之变大，Handy 在窗口内的站位补齐差值。
    let lift = ((py + height) - feet_y).max(5.0 * s);
    (px, py, right, lift)
}

/// 贴边探头打开面板 / 菜单：Handy 保持探头位姿原地不动，窗口朝桌面内侧
/// 扩开、底部对齐探头窗口底部（脚底屏幕位置不变），被屏幕上缘截断时以
/// lift 抬高窗口兜底。返回窗口位置、停靠边（Handy 在左）、lift 与脚底 y。
#[allow(clippy::too_many_arguments)]
fn peek_docked_layout(
    m: &tauri::Monitor,
    s: f64,
    width: f64,
    height: f64,
    prev: HudMode,
    cur_x: Option<f64>,
    cur_y: Option<f64>,
) -> (f64, f64, bool, f64, f64) {
    let (mx, my, mw, mh) = monitor_rect(m);
    let (pw, ph) = hud_sizes(HudMode::Peek, s);
    // 探头与面板 / 菜单都 dock 在屏幕边缘（x 只取屏幕左缘或右缘 - 窗口宽），
    // 以探头窗口的宽度判定停靠边，重排（已在面板 / 菜单形态）时同样成立。
    let dock_left = match cur_x {
        Some(a) => a + pw / 2.0 < mx + mw / 2.0,
        None => true,
    };
    // 脚底 = 当前窗口底 - lift（面板 / 菜单形态重排）；探头形态脚底固定
    // 在窗口底上方 8s。
    let feet_y = match cur_y {
        Some(y) => {
            if prev == HudMode::Panel || prev == HudMode::Menu {
                y + hud_sizes(prev, s).1 - *HUD_PANEL_LIFT.lock().unwrap()
            } else {
                y + ph - 8.0 * s
            }
        }
        None => my + 20.0 + ph - 8.0 * s,
    };
    let x = if dock_left { mx } else { mx + mw - width };
    // 窗口底对齐探头窗口底（feet_y + 8s），被上缘截断时窗口下压、lift 变大。
    let y = (feet_y + 8.0 * s - height).clamp(my + 12.0, (my + mh - height - 12.0).max(my + 12.0));
    let lift = ((y + height) - feet_y).max(5.0 * s);
    (x, y, dock_left, lift, feet_y)
}

/// 从探头形态打开面板 / 菜单时，记下收起后 Handy 应钻回的探头窗口位姿
/// （由脚底反推的精确逻辑坐标，见 HUD_PEEK_ANCHOR）。
fn store_peek_anchor(m: &tauri::Monitor, s: f64, dock_left: bool, feet_y: f64) {
    let (mx, _my, mw, _mh) = monitor_rect(m);
    let (pw, ph) = hud_sizes(HudMode::Peek, s);
    let peek_x = if dock_left { mx } else { mx + mw - pw };
    *HUD_PEEK_ANCHOR.lock().unwrap() = Some((peek_x, feet_y + 8.0 * s - ph));
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
    let prev = hud_mode();
    let default = default_position(app, width, 0.0);
    // 记录面板 / 菜单的来源形态（从面板重排面板时保持原值）。
    if prev == HudMode::Free || prev == HudMode::Peek {
        HUD_PANEL_FROM_PEEK.store(prev == HudMode::Peek, Ordering::SeqCst);
    }

    let (x, y, placement) = match mode {
        HudMode::Free => {
            let (x, y) = if restore {
                let hud = settings::load_from_disk(app).hud;
                match (hud.x, hud.y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                }
            } else if prev == HudMode::Panel || prev == HudMode::Menu {
                // 面板 / 菜单收起：优先站回开面板时记下的精确自由位姿
                // （避免「读回 → 反推 → 再写入」的取整往返漂移）；锚点缺失
                // 时按停靠信息反推兜底。
                let (fw, fh) = hud_sizes(HudMode::Free, s);
                match HUD_ANCHOR.lock().unwrap().take() {
                    Some((fx, fy)) => (fx, fy),
                    None => {
                        let (pw, ph) = hud_sizes(prev, s);
                        let lift = *HUD_PANEL_LIFT.lock().unwrap();
                        match (cur_x, cur_y) {
                            (Some(px), Some(py)) => {
                                let feet_y = py + ph - lift;
                                let fx = if HUD_PANEL_SIDE_RIGHT.load(Ordering::SeqCst) {
                                    px + pw - fw
                                } else {
                                    px
                                };
                                (fx, feet_y - fh + 5.0 * s)
                            }
                            _ => (default.x, default.y),
                        }
                    }
                }
            } else {
                match (cur_x, cur_y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                }
            };
            HUD_ANCHOR.lock().unwrap().take();
            HUD_PEEK_ANCHOR.lock().unwrap().take();
            (x, y, None)
        }
        HudMode::Peek => {
            let hud = settings::load_from_disk(app).hud;
            let base_x = if restore { hud.x.or(cur_x) } else { cur_x };
            let dock_left = match base_x {
                Some(a) => a + width / 2.0 < mx + mw / 2.0,
                None => true,
            };
            let mut x = if dock_left { mx } else { mx + mw - width };
            let mut top = if restore { hud.y.or(cur_y) } else { cur_y }.unwrap_or(my + 20.0);
            if !restore && (prev == HudMode::Panel || prev == HudMode::Menu) {
                // 从面板 / 菜单收回探头：优先用记下的精确探头位姿，Handy
                // 原位钻回边缘；锚点缺失时按 lift 反推兜底。
                match HUD_PEEK_ANCHOR.lock().unwrap().take() {
                    Some((ax, ay)) => {
                        x = ax;
                        top = ay;
                    }
                    None => {
                        let feet_y = top + hud_sizes(prev, s).1 - *HUD_PANEL_LIFT.lock().unwrap();
                        top = feet_y + 8.0 * s - height;
                    }
                }
            }
            let y = top.clamp(my + 12.0, (my + mh - height - 12.0).max(my + 12.0));
            HUD_ANCHOR.lock().unwrap().take();
            HUD_PEEK_ANCHOR.lock().unwrap().take();
            HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
            (x, y, Some(HudPlacement { side: side_str(dock_left), lift: 8.0 * s }))
        }
        HudMode::Panel => {
            let snap = settings::load_from_disk(app).hud.snap_to_edge;
            if snap {
                // 贴边面板：Handy 保持探头位姿原地不动，窗口朝桌面内侧扩开。
                let (x, y, dock_left, lift, feet_y) =
                    peek_docked_layout(&m, s, width, height, prev, cur_x, cur_y);
                HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
                *HUD_PANEL_LIFT.lock().unwrap() = lift;
                if prev == HudMode::Peek {
                    store_peek_anchor(&m, s, dock_left, feet_y);
                }
                (x, y, Some(HudPlacement { side: side_str(dock_left), lift }))
            } else {
                // 自由面板：Handy 原地站好，面板朝桌面内侧展开。
                let (fx, fy) = match HUD_ANCHOR.lock().unwrap().as_ref() {
                    Some((a, b)) => (*a, *b),
                    None => match (cur_x, cur_y) {
                        (Some(a), Some(b)) => (a, b),
                        _ => (default.x, default.y),
                    },
                };
                let (px, py, right, lift) = free_floating_layout(&m, s, width, height, fx, fy);
                HUD_PANEL_SIDE_RIGHT.store(right, Ordering::SeqCst);
                *HUD_PANEL_LIFT.lock().unwrap() = lift;
                let _ = HUD_ANCHOR.lock().unwrap().replace((fx, fy));
                (px, py, Some(HudPlacement { side: side_str(!right), lift }))
            }
        }
        HudMode::Menu => {
            if HUD_PANEL_FROM_PEEK.load(Ordering::SeqCst) {
                // 从探头打开：Handy 保持探头位姿原地不动，菜单卡片朝桌面内侧展开。
                let (x, y, dock_left, lift, feet_y) =
                    peek_docked_layout(&m, s, width, height, prev, cur_x, cur_y);
                HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
                *HUD_PANEL_LIFT.lock().unwrap() = lift;
                if prev == HudMode::Peek {
                    store_peek_anchor(&m, s, dock_left, feet_y);
                }
                (x, y, Some(HudPlacement { side: side_str(dock_left), lift }))
            } else {
                // 从自由形态打开：Handy 原地不动，菜单卡片朝桌面内侧展开。
                let (fx, fy) = match (cur_x, cur_y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                };
                let (px, py, right, lift) = free_floating_layout(&m, s, width, height, fx, fy);
                HUD_PANEL_SIDE_RIGHT.store(right, Ordering::SeqCst);
                *HUD_PANEL_LIFT.lock().unwrap() = lift;
                let _ = HUD_ANCHOR.lock().unwrap().replace((fx, fy));
                (px, py, Some(HudPlacement { side: side_str(!right), lift }))
            }
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
    let mode = match mode.as_str() {
        "peek" => HudMode::Peek,
        "panel" => HudMode::Panel,
        _ => HudMode::Free,
    };
    apply_hud(&app, mode, false)
}

/// 滑块拖动中的实时缩放：窗口尺寸与站位立即跟随，落盘在 commit 时一次完成。
#[tauri::command]
fn set_hud_scale(app: AppHandle, scale: f64) -> Option<HudPlacement> {
    let new_raw = (scale.clamp(0.2, 1.5) * 1000.0).round() as u32;
    let old_raw = HUD_LIVE_SCALE.load(Ordering::SeqCst);
    HUD_LIVE_SCALE.store(new_raw, Ordering::SeqCst);
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
    if mode == HudMode::Panel || mode == HudMode::Menu {
        return apply_hud(&app, mode, false);
    }
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
    let (x, y, side) = match mode {
        HudMode::Peek => {
            let (pw, ph) = hud_sizes(HudMode::Peek, old_s);
            let (nw, nh) = hud_sizes(HudMode::Peek, new_s);
            let dock_left = px + pw / 2.0 < mx + mw / 2.0;
            let feet_y = py + ph - 8.0 * old_s;
            let y = (feet_y - (nh - 8.0 * new_s)).clamp(my + 12.0, (my + mh - nh - 12.0).max(my + 12.0));
            let x = if dock_left { mx } else { mx + mw - nw };
            HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
            (x, y, Some(HudPlacement { side: side_str(dock_left), lift: 8.0 * new_s }))
        }
        _ => {
            let (pw, ph) = hud_sizes(HudMode::Free, old_s);
            let (nw, nh) = hud_sizes(HudMode::Free, new_s);
            let feet_x = px + pw / 2.0;
            let feet_y = py + ph - 5.0 * old_s;
            let x = feet_x - nw / 2.0;
            let y = feet_y - (nh - 5.0 * new_s);
            (x, y, None)
        }
    };
    let (w, h) = hud_sizes(mode, new_s);
    set_window_frame(&window, x, y, w, h, scale_factor);
    side
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
    let _ = app.emit("settings-changed", &s);
}

/// 右键菜单里的「关闭 Handy」；重新开启后窗口以吸附 / 自由形态回来。
#[tauri::command]
fn set_hud_enabled(app: AppHandle, enabled: bool) {
    let mut s = settings::load_from_disk(&app);
    s.hud.enabled = enabled;
    let _ = settings::save_to_disk(&app, &s);
    let _ = app.emit("settings-changed", &s);
    apply_windows(&app, &s);
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
    let app = app.clone();
    let to_peek = target == HudMode::Peek;
    std::thread::spawn(move || {
        let bail = |app: &AppHandle| {
            HUD_ANIMATING.store(false, Ordering::SeqCst);
            let _ = app.emit("hud-anim", serde_json::json!({ "phase": "end" }));
        };
        let Some(window) = app.get_webview_window("hud") else {
            bail(&app);
            return;
        };
        let s = hud_scale(&app);
        let Some(m) = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| app.primary_monitor().ok().flatten())
        else {
            bail(&app);
            apply_windows(&app, &settings::load_from_disk(&app));
            return;
        };
        let scale = m.scale_factor();
        let (mx, my, mw, mh) = monitor_rect(&m);
        let (cur_x, cur_y) = match window.outer_position().ok() {
            Some(p) => (p.x as f64 / scale, p.y as f64 / scale),
            None => (mx, my),
        };

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
            animate_window_depart(&window, (cur_x, cur_y), (edge_x, cur_y), (off_x, cur_y), 720);
            let _ = app.emit(
                "hud-anim",
                serde_json::json!({ "phase": "reveal", "to": "peek", "side": side }),
            );
            set_window_frame(&window, dock_x, dock_y, pw, ph, scale);
            HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
            HUD_MODE.store(HudMode::Peek.to_id(), Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(480));
        } else {
            let (fw, fh) = hud_sizes(HudMode::Free, s);
            let dock_left = cur_x + 25.0 * s < mx + mw / 2.0;
            let side = side_str(dock_left);
            let _ = app.emit(
                "hud-anim",
                serde_json::json!({ "phase": "start", "to": "free", "side": side }),
            );
            let _ = app.emit(
                "hud-anim",
                serde_json::json!({ "phase": "reveal", "to": "free", "side": side }),
            );
            set_window_frame(&window, cur_x, cur_y, fw, fh, scale);
            HUD_MODE.store(HudMode::Free.to_id(), Ordering::SeqCst);
            let hud = settings::load_from_disk(&app).hud;
            let target = match (hud.x, hud.y) {
                (Some(x), Some(y)) => (x, y),
                _ => {
                    let x = if dock_left { mx + 20.0 } else { mx + mw - fw - 20.0 };
                    let y = cur_y.clamp(my + 20.0, (my + mh - fh - 20.0).max(my + 20.0));
                    (x, y)
                }
            };
            animate_window_to(&window, (cur_x, cur_y), target, 640, ease_out_cubic);
        }

        std::thread::sleep(Duration::from_millis(140));
        HUD_ANIMATING.store(false, Ordering::SeqCst);
        let _ = app.emit("hud-anim", serde_json::json!({ "phase": "end" }));
        // 先把动画落点记进设置，再让动画期间的设置变更统一生效，避免回跳。
        persist_geometry(&app, "hud");
        apply_windows(&app, &settings::load_from_disk(&app));
    });
}

// --- Handy 右键菜单（应用内自绘，替代原生菜单） ------------------------------

#[tauri::command]
fn popup_hud_menu(app: AppHandle) {
    if HUD_ANIMATING.load(Ordering::SeqCst) {
        return;
    }
    if hud_mode() == HudMode::Menu {
        return;
    }
    let Some(placement) = apply_hud(&app, HudMode::Menu, false) else {
        return;
    };
    // 聚焦窗口：失焦收起菜单（等价系统菜单的点外关闭），Escape 也能生效。
    if let Some(window) = app.get_webview_window("hud") {
        let _ = window.set_focus();
    }
    let _ = app.emit("hud-menu", &placement);
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
        HUD_ANCHOR.lock().unwrap().take();
        HUD_PEEK_ANCHOR.lock().unwrap().take();
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
        if hud_mode() == HudMode::Panel || hud_mode() == HudMode::Menu {
            // 面板 / 菜单开着：按新设置（缩放 / 吸附）原样重排。
            apply_hud(app, hud_mode(), false);
            return;
        }
        if window.is_visible().unwrap_or(false) && hud_mode() != mode {
            spawn_snap_animation(app, mode);
            return;
        }
    }
    apply_hud(app, mode, true);
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
    let (Ok(scale), Ok(pos), Ok(size)) =
        (window.scale_factor(), window.outer_position(), window.outer_size())
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
            s.window.x = x;
            s.window.y = y;
            s.window.width = width;
            s.window.height = height;
        }
        "todo-widget" | "notes-widget" => {
            // 吸附开启：拖动停歇后先贴合到边缘，再记忆贴合后的位置。
            let snap = widget_snap(app, label);
            let (mut px, mut py) = (x.unwrap_or(0.0), y.unwrap_or(0.0));
            if snap {
                if let Some(m) = window.current_monitor().ok().flatten() {
                    let (sx, sy) = snap_to_edges(true, px, py, width, height, &m);
                    if (sx - px).abs() > 0.5 || (sy - py).abs() > 0.5 {
                        let _ = window.set_position(LogicalPosition::new(sx, sy));
                        if let (Ok(p), Ok(sc)) = (window.outer_position(), window.scale_factor()) {
                            px = p.x as f64 / sc;
                            py = p.y as f64 / sc;
                        }
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
                HudMode::Panel | HudMode::Menu => {
                    // 面板 / 菜单可拖动：把当前位姿反推为收起锚点（自由面板
                    // 回自由位姿、贴边面板钻回探头边缘），不落盘。
                    if let (Some(px), Some(py)) = (x, y) {
                        let sc = s.hud.scale;
                        let (pw, ph) = hud_sizes(hud_mode(), sc);
                        let lift = *HUD_PANEL_LIFT.lock().unwrap();
                        let feet_y = py + ph - lift;
                        if HUD_PANEL_FROM_PEEK.load(Ordering::SeqCst) {
                            if let Ok(Some(m)) = window.current_monitor() {
                                let dock_left = !HUD_PANEL_SIDE_RIGHT.load(Ordering::SeqCst);
                                store_peek_anchor(&m, sc, dock_left, feet_y);
                            }
                        } else {
                            let (fw, fh) = hud_sizes(HudMode::Free, sc);
                            let fx = if HUD_PANEL_SIDE_RIGHT.load(Ordering::SeqCst) {
                                px + pw - fw
                            } else {
                                px
                            };
                            *HUD_ANCHOR.lock().unwrap() = Some((fx, feet_y - fh + 5.0 * sc));
                        }
                    }
                    return;
                }
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
                                let _ = app.emit("settings-changed", &s);
                                apply_windows(app, &s);
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
            let _ = app.emit("settings-changed", &s);
            apply_windows(app, &s);
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

pub fn run() {
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
