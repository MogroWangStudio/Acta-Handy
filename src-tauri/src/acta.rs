//! Reading Acta's data folder (行记数据文件夹).
//!
//! Layout (V3): `acta-manifest.json` + `classifications.json` + `notes/`
//! (per-note `.json` config + `.md` body) + `todos/` (per-todo `.json`).
//! V2 manifests keep single `.json` files with an HTML body inside.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use serde_json::Value;

const MANIFEST_FILE: &str = "acta-manifest.json";
const CLASSIFICATIONS_FILE: &str = "classifications.json";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActaFolder {
    pub id: String,
    pub name_key: Option<String>,
    pub name: Option<String>,
    pub color: Option<String>,
    pub short_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActaNote {
    pub id: String,
    pub title: String,
    pub folder_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub body_markdown: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActaTask {
    pub id: String,
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActaTodo {
    pub id: String,
    pub title: String,
    pub folder_id: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub start_at: String,
    pub due_at: String,
    pub priority: String,
    pub tasks: Vec<ActaTask>,
    pub completed: bool,
    pub notes: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActaData {
    pub path: String,
    pub synced_at: Option<String>,
    pub folders: Vec<ActaFolder>,
    pub notes: Vec<ActaNote>,
    pub todos: Vec<ActaTodo>,
    pub warnings: Vec<String>,
}

fn jstr(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn jopt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn jbool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// File names come from the manifest; refuse anything that could escape the
/// data folder before joining.
fn safe_file_name(name: &str) -> Option<String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_folder(v: &Value) -> ActaFolder {
    ActaFolder {
        id: jstr(v, "id"),
        name_key: jopt_str(v, "nameKey"),
        name: jopt_str(v, "name"),
        color: jopt_str(v, "color"),
        short_name: jopt_str(v, "shortName"),
    }
}

fn parse_note(id: &str, item: &Value, body_markdown: String) -> ActaNote {
    ActaNote {
        id: if item.get("id").is_some() {
            jstr(item, "id")
        } else {
            id.to_string()
        },
        title: jstr(item, "title"),
        folder_id: jstr(item, "folderId"),
        created_at: jstr(item, "createdAt"),
        updated_at: jstr(item, "updatedAt"),
        deleted_at: jopt_str(item, "deletedAt"),
        body_markdown,
    }
}

fn parse_todo(id: &str, item: &Value) -> ActaTodo {
    let tasks = item
        .get("tasks")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|t| ActaTask {
                    id: jstr(t, "id"),
                    text: jstr(t, "text"),
                    done: jbool(t, "done"),
                })
                .collect()
        })
        .unwrap_or_default();
    let priority = match jstr(item, "priority").as_str() {
        "high" => "high",
        "low" => "low",
        _ => "medium",
    };
    ActaTodo {
        id: if item.get("id").is_some() {
            jstr(item, "id")
        } else {
            id.to_string()
        },
        title: jstr(item, "title"),
        folder_id: jstr(item, "folderId"),
        created_at: jstr(item, "createdAt"),
        updated_at: jstr(item, "updatedAt"),
        deleted_at: jopt_str(item, "deletedAt"),
        start_at: jstr(item, "startAt"),
        due_at: jstr(item, "dueAt"),
        priority: priority.to_string(),
        tasks,
        completed: jbool(item, "completed"),
        notes: jstr(item, "notes"),
    }
}

fn read_note(folder: &Path, entry: &Value, id: &str) -> Result<Option<ActaNote>, String> {
    if let Some(config) = jopt_str(entry, "config") {
        let Some(config) = safe_file_name(&config) else {
            return Ok(None);
        };
        let md_file = jopt_str(entry, "markdown").and_then(|s| safe_file_name(&s));
        let raw = fs::read_to_string(folder.join("notes").join(&config))
            .map_err(|_| format!("笔记配置读取失败：{config}"))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|e| format!("笔记配置解析失败：{config}（{e}）"))?;
        let item = value.get("item").cloned().unwrap_or(Value::Null);
        let body = md_file
            .and_then(|name| fs::read_to_string(folder.join("notes").join(name)).ok())
            .unwrap_or_default();
        Ok(Some(parse_note(id, &item, body)))
    } else if let Some(file) = jopt_str(entry, "file") {
        // V2: one JSON per note, body stored as HTML.
        let Some(file) = safe_file_name(&file) else {
            return Ok(None);
        };
        let raw = fs::read_to_string(folder.join("notes").join(&file))
            .map_err(|_| format!("笔记文件读取失败：{file}"))?;
        let value: Value =
            serde_json::from_str(&raw).map_err(|e| format!("笔记文件解析失败：{file}（{e}）"))?;
        let item = value.get("item").cloned().unwrap_or(Value::Null);
        let body = html_to_plain(&jstr(&item, "body"));
        Ok(Some(parse_note(id, &item, body)))
    } else {
        Ok(None)
    }
}

fn read_todo(folder: &Path, entry: &Value, id: &str) -> Result<Option<ActaTodo>, String> {
    let Some(file) = jopt_str(entry, "file").and_then(|s| safe_file_name(&s)) else {
        return Ok(None);
    };
    let raw = fs::read_to_string(folder.join("todos").join(&file))
        .map_err(|_| format!("待办文件读取失败：{file}"))?;
    let value: Value =
        serde_json::from_str(&raw).map_err(|e| format!("待办文件解析失败：{file}（{e}）"))?;
    let item = value.get("item").cloned().unwrap_or(Value::Null);
    Ok(Some(parse_todo(id, &item)))
}

pub fn read_data_folder(folder: &Path) -> Result<ActaData, String> {
    let manifest_path = folder.join(MANIFEST_FILE);
    let raw = fs::read_to_string(&manifest_path).map_err(|_| {
        format!(
            "未找到 Acta 数据清单（{}）\n请在 Acta 的「设置 → 数据」中同步到数据文件夹后，再选择该文件夹。",
            manifest_path.display()
        )
    })?;
    let manifest: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("acta-manifest.json 解析失败：{e}"))?;
    let format = jstr(&manifest, "format");
    if !format.is_empty() && format != "acta-data-folder" {
        return Err("所选文件夹不是 Acta 数据文件夹".to_string());
    }

    let mut warnings: Vec<String> = Vec::new();

    let folders = fs::read_to_string(folder.join(CLASSIFICATIONS_FILE))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|v| v.get("folders").and_then(Value::as_array).cloned())
        .map(|arr| arr.iter().map(parse_folder).collect())
        .unwrap_or_default();

    let mut notes = Vec::new();
    if let Some(entries) = manifest.get("notes").and_then(Value::as_array) {
        for entry in entries {
            let id = jstr(entry, "id");
            if id.is_empty() {
                continue;
            }
            match read_note(folder, entry, &id) {
                Ok(Some(note)) => notes.push(note),
                Ok(None) => {}
                Err(w) => warnings.push(w),
            }
        }
    }

    let mut todos = Vec::new();
    if let Some(entries) = manifest.get("todos").and_then(Value::as_array) {
        for entry in entries {
            let id = jstr(entry, "id");
            if id.is_empty() {
                continue;
            }
            match read_todo(folder, entry, &id) {
                Ok(Some(todo)) => todos.push(todo),
                Ok(None) => {}
                Err(w) => warnings.push(w),
            }
        }
    }

    Ok(ActaData {
        path: folder.display().to_string(),
        synced_at: jopt_str(&manifest, "syncedAt"),
        folders,
        notes,
        todos,
        warnings,
    })
}

/// Highest mtime across manifest, classifications and every item file, used
/// by the change watcher. Returns None when the folder has no manifest yet.
pub fn folder_signature(folder: &Path) -> Option<(i64, u32)> {
    let mut latest: Option<(i64, u32)> = None;
    let mut consider = |path: &PathBuf| {
        if let Ok(meta) = fs::metadata(path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(dur) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                    let sig = (dur.as_secs() as i64, dur.subsec_nanos());
                    if latest.map_or(true, |cur| sig > cur) {
                        latest = Some(sig);
                    }
                }
            }
        }
    };
    consider(&folder.join(MANIFEST_FILE));
    consider(&folder.join(CLASSIFICATIONS_FILE));
    for dir in ["notes", "todos"] {
        if let Ok(entries) = fs::read_dir(folder.join(dir)) {
            for entry in entries.flatten() {
                consider(&entry.path());
            }
        }
    }
    latest
}

/// Best-effort HTML → plain text for legacy (V2) note bodies only.
fn html_to_plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    let mut in_tag = false;
    while let Some(c) = chars.next() {
        if in_tag {
            if c == '>' {
                in_tag = false;
            }
            continue;
        }
        if c == '<' {
            let rest: String = chars.clone().take(8).collect();
            let lower = rest.to_lowercase();
            if lower.starts_with("br") || lower.starts_with("/p>") || lower.starts_with("/div>")
                || lower.starts_with("/h1") || lower.starts_with("/h2") || lower.starts_with("/h3")
                || lower.starts_with("/li>")
            {
                out.push('\n');
            }
            in_tag = true;
            continue;
        }
        out.push(c);
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}
