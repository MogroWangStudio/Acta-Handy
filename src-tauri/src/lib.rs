//! Acta Handy — desktop widgets and a floating HUD for Acta's 行记 data.

mod acta;
mod settings;

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WindowEvent};

use settings::HandySettings;

const TRAY_ID: &str = "acta-handy-tray";
const WIDGET_LABELS: [&str; 3] = ["todo-widget", "notes-widget", "hud"];

#[tauri::command]
fn read_acta_data(folder: String) -> Result<acta::ActaData, String> {
    if folder.trim().is_empty() {
        return Err("尚未选择 Acta 数据文件夹".to_string());
    }
    acta::read_data_folder(std::path::Path::new(&folder))
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
    apply_widget(
        app,
        "hud",
        s.hud.enabled,
        s.hud.x,
        s.hud.y,
        292.0,
        66.0,
        s.hud.always_on_top,
    );
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
            s.hud.x = x;
            s.hud.y = y;
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
            refresh_data,
            show_window,
            quit_app,
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
