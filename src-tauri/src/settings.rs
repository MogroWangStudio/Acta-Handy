//! Handy's own settings, stored as JSON in the OS app-config directory.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

const SETTINGS_FILE: &str = "handy-settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HandySettings {
    pub version: u32,
    pub data_folder: String,
    pub theme: String,
    pub language: String,
    pub refresh_interval_secs: u32,
    pub todo_widget: WidgetConfig,
    pub notes_widget: WidgetConfig,
    pub hud: HudConfig,
}

impl Default for HandySettings {
    fn default() -> Self {
        Self {
            version: 1,
            data_folder: String::new(),
            theme: "auto".to_string(),
            language: "zh".to_string(),
            refresh_interval_secs: 30,
            todo_widget: WidgetConfig {
                enabled: true,
                ..WidgetConfig::default()
            },
            notes_widget: WidgetConfig::default(),
            hud: HudConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WidgetConfig {
    pub enabled: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: f64,
    pub height: f64,
    pub opacity: f64,
    pub always_on_top: bool,
    pub show_completed: bool,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            x: None,
            y: None,
            width: 300.0,
            height: 360.0,
            opacity: 1.0,
            always_on_top: false,
            show_completed: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HudConfig {
    pub enabled: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub opacity: f64,
    pub always_on_top: bool,
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            x: None,
            y: None,
            opacity: 1.0,
            always_on_top: true,
        }
    }
}

impl HandySettings {
    pub fn is_chinese(&self) -> bool {
        self.language != "en"
    }

    /// Clamp user-editable numbers into safe ranges so a bad value can never
    /// push a window off-screen or busy-loop the watcher.
    pub fn sanitized(mut self) -> Self {
        self.refresh_interval_secs = self.refresh_interval_secs.clamp(5, 3600);
        self.theme = match self.theme.as_str() {
            "light" | "dark" => self.theme,
            _ => "auto".to_string(),
        };
        self.language = match self.language.as_str() {
            "en" => self.language,
            _ => "zh".to_string(),
        };
        for cfg in [&mut self.todo_widget, &mut self.notes_widget] {
            cfg.width = cfg.width.clamp(240.0, 720.0);
            cfg.height = cfg.height.clamp(200.0, 1200.0);
            cfg.opacity = cfg.opacity.clamp(0.3, 1.0);
        }
        self.hud.opacity = self.hud.opacity.clamp(0.3, 1.0);
        self
    }
}

fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join(SETTINGS_FILE))
}

pub fn load_from_disk(app: &AppHandle) -> HandySettings {
    let Some(path) = settings_path(app) else {
        return HandySettings::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<HandySettings>(&raw).ok())
        .map(|s| s.sanitized())
        .unwrap_or_default()
}

pub fn save_to_disk(app: &AppHandle, settings: &HandySettings) -> Result<(), String> {
    let Some(path) = settings_path(app) else {
        return Err("无法确定设置文件位置".to_string());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("无法创建设置目录：{e}"))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("无法写入设置：{e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("无法保存设置：{e}"))?;
    Ok(())
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> HandySettings {
    load_from_disk(&app)
}

/// Save, broadcast to every window, then apply window visibility/geometry.
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: HandySettings) -> Result<HandySettings, String> {
    let clean = settings.sanitized();
    save_to_disk(&app, &clean)?;
    let _ = app.emit("settings-changed", &clean);
    crate::apply_windows(&app, &clean);
    Ok(clean)
}
