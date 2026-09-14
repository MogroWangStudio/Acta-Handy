//! Acta Handy — desktop widgets and a floating HUD for Acta's 行记 data.

mod acta;
mod settings;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WindowEvent};

use settings::HandySettings;

const TRAY_ID: &str = "acta-handy-tray";
const WIDGET_LABELS: [&str; 3] = ["todo-widget", "notes-widget", "hud"];

/// HUD shapes. `Bar` is the classic floating strip; `Pill` is the docked
/// capsule; `Panel` is the expanded quick-edit panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudMode {
    Bar,
    Pill,
    Panel,
}

const HUD_BAR: (f64, f64) = (292.0, 66.0);
const HUD_PILL: (f64, f64) = (56.0, 68.0);
const HUD_PANEL: (f64, f64) = (324.0, 460.0);

/// While true, a watcher thread polls the cursor and wakes the HUD when the
/// pointer comes near (stealth mode keeps the window invisible otherwise).
static HUD_WATCH: AtomicBool = AtomicBool::new(false);
static HUD_WATCH_RUNNING: AtomicBool = AtomicBool::new(false);

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

/// Resize/position the HUD for a shape. Returns which screen edge the pill is
/// docked to ("left"/"right"), or None for the free-floating bar.
/// With `restore`, the saved settings position wins (app startup / settings
/// change); otherwise the window keeps its live position (pill ↔ panel).
fn apply_hud(app: &AppHandle, mode: HudMode, restore: bool) -> Option<&'static str> {
    let window = app.get_webview_window("hud")?;
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let scale = monitor.as_ref().map(|m| m.scale_factor());
    let cur = window.outer_position().ok();
    let (cur_x, cur_y) = match cur {
        Some(p) => {
            let s = scale.unwrap_or(1.0);
            (Some(p.x as f64 / s), Some(p.y as f64 / s))
        }
        None => (None, None),
    };

    let (width, height) = match mode {
        HudMode::Bar => HUD_BAR,
        HudMode::Pill => HUD_PILL,
        HudMode::Panel => HUD_PANEL,
    };
    let _ = window.set_size(LogicalSize::new(width, height));

    let Some(m) = monitor else {
        return None;
    };
    let s = scale.unwrap_or(1.0);
    let mx = m.position().x as f64 / s;
    let my = m.position().y as f64 / s;
    let mw = m.size().width as f64 / s;
    let mh = m.size().height as f64 / s;

    let saved_x = if restore { None } else { cur_x };
    let saved_y = if restore { None } else { cur_y };
    let (x, y) = match mode {
        HudMode::Bar => {
            let settings = settings::load_from_disk(app);
            let base = match (settings.hud.x, settings.hud.y) {
                (Some(x), Some(y)) => (x, y),
                _ => {
                    let d = default_position(app, width, 0.0);
                    (d.x, d.y)
                }
            };
            (saved_x.unwrap_or(base.0), saved_y.unwrap_or(base.1))
        }
        HudMode::Pill | HudMode::Panel => {
            let base_x = saved_x.or(cur_x);
            let dock_left = match base_x {
                Some(x) => x + width / 2.0 < mx + mw / 2.0,
                None => true,
            };
            let x = if dock_left { mx } else { mx + mw - width };
            let top = saved_y.or(cur_y).unwrap_or(my + 20.0);
            let y = top.clamp(my + 12.0, (my + mh - height - 12.0).max(my + 12.0));
            (x, y)
        }
    };
    let _ = window.set_position(LogicalPosition::new(x, y));
    let _ = window.show();
    if x + width / 2.0 < mx + mw / 2.0 {
        Some("left")
    } else {
        Some("right")
    }
}

#[tauri::command]
fn set_hud_mode(app: AppHandle, mode: String) -> Option<&'static str> {
    let mode = match mode.as_str() {
        "pill" => HudMode::Pill,
        "panel" => HudMode::Panel,
        _ => HudMode::Bar,
    };
    apply_hud(&app, mode, false)
}

/// Enable (or disable) the near-cursor wake watch for the hidden HUD.
#[tauri::command]
fn set_hud_cursor_watch(app: AppHandle, watch: bool) {
    HUD_WATCH.store(watch, Ordering::SeqCst);
    if !watch || HUD_WATCH_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
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
            let pad = 26.0;
            let near = (cursor.x as f64) >= pos.x as f64 - pad
                && (cursor.x as f64) <= pos.x as f64 + size.width as f64 + pad
                && (cursor.y as f64) >= pos.y as f64 - pad
                && (cursor.y as f64) <= pos.y as f64 + size.height as f64 + pad;
            if near {
                let _ = app.emit("hud-wake", ());
                break;
            }
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
        if let Some(window) = app.get_webview_window("hud") {
            let _ = window.hide();
        }
        return;
    }
    let mode = if s.hud.snap_to_edge {
        HudMode::Pill
    } else {
        HudMode::Bar
    };
    if let Some(window) = app.get_webview_window("hud") {
        let _ = window.set_always_on_top(s.hud.always_on_top);
    }
    apply_hud(app, mode, true);
}

// --- geometry persistence -------------------------------------------------

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
            // In pill mode the HUD snaps back to the screen edge once the
            // drag settles; remember the docked position, not the drop spot.
            let dock = settings::load_from_disk(app).hud.snap_to_edge;
            if dock {
                if let (Ok(scale), Ok(monitor)) = (window.scale_factor(), window.current_monitor())
                {
                    if let Some(m) = monitor {
                        let mx = m.position().x as f64 / scale;
                        let mw = m.size().width as f64 / scale;
                        let width = size.width as f64 / scale;
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
            } else {
                s.hud.x = x;
                s.hud.y = y;
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
            "悬浮窗",
            "退出 Acta Handy",
        )
    } else {
        (
            "Open Acta Handy Settings",
            "Todo Widget",
            "Notes Widget",
            "Floating HUD",
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

pub fn run() {
    tauri::Builder::default()
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
            settings::load_settings,
            settings::save_settings
        ])
        .setup(|app| {
            let handle = app.handle().clone();
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
