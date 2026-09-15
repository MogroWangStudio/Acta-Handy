//! Acta Handy — desktop widgets and a floating HUD for Acta's 行记 data.

mod acta;
mod settings;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow, WindowEvent};

use settings::HandySettings;

const TRAY_ID: &str = "acta-handy-tray";
const WIDGET_LABELS: [&str; 3] = ["todo-widget", "notes-widget", "hud"];
const ALL_WINDOWS: [&str; 4] = ["main", "todo-widget", "notes-widget", "hud"];

/// HUD shapes. `Free` is Handy standing on the desktop; `Peek` is the
/// edge-clinging pose (part of the body clipped beyond the screen edge);
/// `Panel` is the expanded quick-edit panel with Handy beside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudMode {
    Free,
    Peek,
    Panel,
}

impl HudMode {
    fn to_id(self) -> u32 {
        match self {
            HudMode::Free => 0,
            HudMode::Peek => 1,
            HudMode::Panel => 2,
        }
    }

    fn from_id(id: u32) -> Self {
        match id {
            1 => HudMode::Peek,
            2 => HudMode::Panel,
            _ => HudMode::Free,
        }
    }
}

// 当前 HUD 形态与面板停靠信息：位置映射（面板开合时 Handy 原地不动）和
// 几何持久化都要读它们。
static HUD_MODE: AtomicU32 = AtomicU32::new(0);
/// 面板里 Handy 是否站在窗口右侧（也是贴边停靠时的屏幕右缘一侧）。
static HUD_PANEL_SIDE_RIGHT: AtomicBool = AtomicBool::new(true);
/// 面板形态下 Handy 脚底离窗口底部的逻辑距离：面板向上展开被屏幕上缘
/// 截断时变大，Handy 的屏幕位置因此保持不动。
static HUD_PANEL_LIFT: Mutex<f64> = Mutex::new(5.0);
/// 自由形态打开面板时的 Handy 窗口位置：面板被拖动或重排时以它为锚。
static HUD_ANCHOR: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// 贴边过渡动画播放中：期间设置应用与几何持久化全部让路。
static HUD_ANIMATING: AtomicBool = AtomicBool::new(false);

/// While true, a watcher thread polls the cursor and wakes the HUD when the
/// pointer comes near (stealth mode keeps the window invisible otherwise).
static HUD_WATCH: AtomicBool = AtomicBool::new(false);
static HUD_WATCH_RUNNING: AtomicBool = AtomicBool::new(false);

fn hud_mode() -> HudMode {
    HudMode::from_id(HUD_MODE.load(Ordering::SeqCst))
}

fn hud_scale(app: &AppHandle) -> f64 {
    settings::load_from_disk(app).hud.scale
}

/// 各形态的窗口逻辑尺寸。面板宽度 = 固定卡片宽 + Handy 站位（随缩放加宽），
/// 高度固定；Handy 在面板里的站位与自由形态完全同偏移，开合时不挪位。
fn hud_sizes(mode: HudMode, s: f64) -> (f64, f64) {
    match mode {
        HudMode::Free => (92.0 * s, 136.0 * s),
        HudMode::Peek => (50.0 * s, 146.0 * s),
        HudMode::Panel => (310.0 + 82.0 * s, 470.0),
    }
}

fn side_str(handy_left: bool) -> &'static str {
    if handy_left {
        "left"
    } else {
        "right"
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HudPlacement {
    /// Handy 站在窗口的哪一侧（也是贴边方向）。
    side: &'static str,
    /// 面板形态下 Handy 脚底离窗口底部的逻辑像素数。
    lift: f64,
}

#[tauri::command]
fn read_acta_data(folder: String) -> Result<acta::ActaData, String> {
    if folder.trim().is_empty() {
        return Err("尚未选择 Acta 数据文件夹".to_string());
    }
    acta::read_data_folder(std::path::Path::new(&folder))
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
    let todo = acta::write_todo_check(std::path::Path::new(&folder), &patch)?;
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
    let note = acta::write_note(std::path::Path::new(&folder), &patch)?;
    let _ = app.emit("acta-data-changed", ());
    Ok(note)
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
    let _ = window.set_size(LogicalSize::new(width, height));

    let Some(m) = monitor else {
        HUD_MODE.store(mode.to_id(), Ordering::SeqCst);
        return None;
    };
    let mx = m.position().x as f64 / scale;
    let my = m.position().y as f64 / scale;
    let mw = m.size().width as f64 / scale;
    let mh = m.size().height as f64 / scale;
    let prev = hud_mode();
    let default = default_position(app, width, 0.0);

    let (x, y, placement) = match mode {
        HudMode::Free => {
            let (x, y) = if restore {
                let hud = settings::load_from_disk(app).hud;
                match (hud.x, hud.y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                }
            } else if prev == HudMode::Panel {
                // 面板收起：按面板停靠信息反推自由位姿，Handy 原地站好。
                let (pw, ph) = hud_sizes(HudMode::Panel, s);
                let (fw, fh) = hud_sizes(HudMode::Free, s);
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
            } else {
                match (cur_x, cur_y) {
                    (Some(a), Some(b)) => (a, b),
                    _ => (default.x, default.y),
                }
            };
            HUD_ANCHOR.lock().unwrap().take();
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
            HUD_ANCHOR.lock().unwrap().take();
            HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
            (x, y, Some(HudPlacement { side: side_str(dock_left), lift: 8.0 * s }))
        }
        HudMode::Panel => {
            let snap = settings::load_from_disk(app).hud.snap_to_edge;
            if snap {
                // 贴边面板：停靠在探头一侧的屏幕边缘，Handy 站回窗口底部。
                let dock_left = match cur_x {
                    Some(a) => a + width / 2.0 < mx + mw / 2.0,
                    None => true,
                };
                let x = if dock_left { mx } else { mx + mw - width };
                let top = cur_y.unwrap_or(my + 20.0);
                let y = top.clamp(my + 12.0, (my + mh - height - 12.0).max(my + 12.0));
                HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
                *HUD_PANEL_LIFT.lock().unwrap() = 5.0 * s;
                (x, y, Some(HudPlacement { side: side_str(dock_left), lift: 5.0 * s }))
            } else {
                // 自由面板：Handy 原地站好，面板朝桌面内侧展开。
                let (fx, fy) = match HUD_ANCHOR.lock().unwrap().as_ref() {
                    Some((a, b)) => (*a, *b),
                    None => match (cur_x, cur_y) {
                        (Some(a), Some(b)) => (a, b),
                        _ => (default.x, default.y),
                    },
                };
                let (fw, fh) = hud_sizes(HudMode::Free, s);
                let right = fx + fw / 2.0 >= mx + mw / 2.0;
                let feet_y = fy + fh - 5.0 * s;
                let py = (fy + fh - height).clamp(my + 8.0, (my + mh - height - 8.0).max(my + 8.0));
                let base_x = if right { fx + fw - width } else { fx };
                let px = base_x.clamp(mx + 4.0, (mx + mw - width - 4.0).max(mx + 4.0));
                let lift = (feet_y - py).max(5.0 * s);
                HUD_PANEL_SIDE_RIGHT.store(right, Ordering::SeqCst);
                *HUD_PANEL_LIFT.lock().unwrap() = lift;
                let _ = HUD_ANCHOR.lock().unwrap().replace((fx, fy));
                (px, py, Some(HudPlacement { side: side_str(!right), lift }))
            }
        }
    };

    let _ = window.set_position(LogicalPosition::new(x, y));
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

// --- 贴边过渡动画 -----------------------------------------------------------
//
// 开启吸附时：Handy 保持站姿缓入缓出地滑向屏幕边缘、整个滑出消失，再以
// 探头位姿贴边出现；关闭时反向——以站姿出现在边缘，滑回记忆位置。播放
// 期间 HUD_ANIMATING 为真，期间落下的设置变更在动画结束时统一生效。

fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
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

fn spawn_snap_animation(app: &AppHandle, target: HudMode) {
    if HUD_ANIMATING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let to_peek = target == HudMode::Peek;
    let _ = app.emit(
        "hud-anim",
        serde_json::json!({ "phase": "start", "to": if to_peek { "peek" } else { "free" } }),
    );
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
        let mx = m.position().x as f64 / scale;
        let my = m.position().y as f64 / scale;
        let mw = m.size().width as f64 / scale;
        let mh = m.size().height as f64 / scale;
        let (cur_x, cur_y) = match window.outer_position().ok() {
            Some(p) => (p.x as f64 / scale, p.y as f64 / scale),
            None => (mx, my),
        };

        if to_peek {
            let (w, _h) = hud_sizes(HudMode::Free, s);
            let dock_left = cur_x + w / 2.0 < mx + mw / 2.0;
            let (pw, ph) = hud_sizes(HudMode::Peek, s);
            let dock_x = if dock_left { mx } else { mx + mw - pw };
            let dock_y = cur_y.clamp(my + 12.0, (my + mh - ph - 12.0).max(my + 12.0));
            let off_x = if dock_left { mx - w } else { mx + mw };
            animate_window_to(&window, (cur_x, cur_y), (off_x, cur_y), 640, ease_in_out_cubic);
            let _ = app.emit("hud-anim", serde_json::json!({ "phase": "reveal", "to": "peek" }));
            let _ = window.set_size(LogicalSize::new(pw, ph));
            let _ = window.set_position(LogicalPosition::new(dock_x, dock_y));
            HUD_PANEL_SIDE_RIGHT.store(!dock_left, Ordering::SeqCst);
            HUD_MODE.store(HudMode::Peek.to_id(), Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(480));
        } else {
            let (fw, fh) = hud_sizes(HudMode::Free, s);
            let dock_left = cur_x + 25.0 * s < mx + mw / 2.0;
            let _ = app.emit("hud-anim", serde_json::json!({ "phase": "reveal", "to": "free" }));
            let _ = window.set_size(LogicalSize::new(fw, fh));
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

// --- Handy 右键菜单 ---------------------------------------------------------

#[tauri::command]
fn popup_hud_menu(app: AppHandle) {
    if HUD_ANIMATING.load(Ordering::SeqCst) {
        return;
    }
    let Some(window) = app.get_webview_window("hud") else {
        return;
    };
    let s = settings::load_from_disk(&app);
    let (small, medium, large, close) = if s.is_chinese() {
        ("小", "中", "大", "关闭 Handy")
    } else {
        ("Small", "Medium", "Large", "Close Handy")
    };
    let scale = s.hud.scale;
    let checked = |v: f64| (scale - v).abs() < 0.01;
    let build = || -> tauri::Result<Menu<tauri::Wry>> {
        let small_item =
            CheckMenuItem::with_id(&app, "hud-scale-small", small, true, checked(0.8), None::<&str>)?;
        let medium_item =
            CheckMenuItem::with_id(&app, "hud-scale-medium", medium, true, checked(1.0), None::<&str>)?;
        let large_item =
            CheckMenuItem::with_id(&app, "hud-scale-large", large, true, checked(1.25), None::<&str>)?;
        let sep = PredefinedMenuItem::separator(&app)?;
        let close_item = MenuItem::with_id(&app, "hud-close", close, true, None::<&str>)?;
        Menu::with_items(&app, &[&small_item, &medium_item, &large_item, &sep, &close_item])
    };
    if let Ok(menu) = build() {
        let _ = window.popup_menu(&menu);
    }
}

fn handle_hud_menu_event(app: &AppHandle, id: &str) {
    let mut s = settings::load_from_disk(app);
    match id {
        "hud-scale-small" => s.hud.scale = 0.8,
        "hud-scale-medium" => s.hud.scale = 1.0,
        "hud-scale-large" => s.hud.scale = 1.25,
        "hud-close" => s.hud.enabled = false,
        _ => return,
    }
    let _ = settings::save_to_disk(app, &s);
    let _ = app.emit("settings-changed", &s);
    apply_windows(app, &s);
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
        let scale = m.scale_factor();
        let mx = m.position().x as f64 / scale;
        let my = m.position().y as f64 / scale;
        let mw = m.size().width as f64 / scale;
        let mh = m.size().height as f64 / scale;
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
        let scale = m.scale_factor();
        let right = (m.position().x as f64 + m.size().width as f64) / scale - width - 20.0;
        let top = m.position().y as f64 / scale + 20.0 + offset;
        return LogicalPosition::new(right.max(20.0), top);
    }
    LogicalPosition::new(60.0, 60.0)
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
    let pos = match (x, y) {
        (Some(x), Some(y)) => {
            position_on_screen(app, x, y, width, height)
                .unwrap_or_else(|| LogicalPosition::new(x, y))
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

pub fn apply_windows(app: &AppHandle, s: &HandySettings) {
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
        if hud_mode() == HudMode::Panel {
            // 面板开着：按新设置（缩放 / 吸附）原样重排面板。
            apply_hud(app, HudMode::Panel, false);
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

static PENDING_SAVES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn schedule_geometry_save(app: &AppHandle, label: &str) {
    if !WIDGET_LABELS.contains(&label) {
        return;
    }
    {
        let mut pending = PENDING_SAVES.lock().unwrap();
        let set = pending.get_or_insert_with(HashSet::new);
        if !set.insert(label.to_string()) {
            return; // a save is already scheduled; it reads the latest state
        }
    }
    let app = app.clone();
    let label = label.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        persist_geometry(&app, &label);
        if let Some(set) = PENDING_SAVES.lock().unwrap().as_mut() {
            set.remove(&label);
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
        "todo-widget" => {
            s.todo_widget.x = x;
            s.todo_widget.y = y;
            s.todo_widget.width = width;
            s.todo_widget.height = height;
        }
        "notes-widget" => {
            s.notes_widget.x = x;
            s.notes_widget.y = y;
            s.notes_widget.width = width;
            s.notes_widget.height = height;
        }
        "hud" => {
            if HUD_ANIMATING.load(Ordering::SeqCst) {
                return;
            }
            match hud_mode() {
                HudMode::Panel => {
                    // 面板可拖动：只把当前位姿反推为自由锚点，不落盘。
                    if let (Some(px), Some(py)) = (x, y) {
                        let sc = s.hud.scale;
                        let (pw, ph) = hud_sizes(HudMode::Panel, sc);
                        let (fw, fh) = hud_sizes(HudMode::Free, sc);
                        let lift = *HUD_PANEL_LIFT.lock().unwrap();
                        let feet_y = py + ph - lift;
                        let fx = if HUD_PANEL_SIDE_RIGHT.load(Ordering::SeqCst) {
                            px + pw - fw
                        } else {
                            px
                        };
                        *HUD_ANCHOR.lock().unwrap() = Some((fx, feet_y - fh + 5.0 * sc));
                    }
                    return;
                }
                HudMode::Peek => {
                    // 探头形态松手后重新贴边：记住贴边位置，不记落点。
                    if let (Ok(scale), Ok(monitor)) = (window.scale_factor(), window.current_monitor())
                    {
                        if let Some(m) = monitor {
                            let mx = m.position().x as f64 / scale;
                            let mw = m.size().width as f64 / scale;
                            let dock_left = pos.x as f64 / scale + width / 2.0 < mx + mw / 2.0;
                            let x = if dock_left { mx } else { mx + mw - width };
                            let _ = window.set_position(LogicalPosition::new(
                                x,
                                pos.y as f64 / scale,
                            ));
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
            refresh_data,
            show_window,
            quit_app,
            set_hud_mode,
            set_hud_cursor_watch,
            popup_hud_menu,
            settings::load_settings,
            settings::save_settings
        ])
        .on_menu_event(|app, event| handle_hud_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            let handle = app.handle().clone();
            #[cfg(windows)]
            fix_windows_window_icons(&handle);
            let s = settings::load_from_disk(&handle);
            apply_windows(&handle, &s);
            setup_tray(&handle)?;
            start_watch_thread(handle.clone());

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
