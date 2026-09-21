//! Reading Acta's data folder (行记数据文件夹).
//!
//! Layout (V3): `acta-manifest.json` + `classifications.json` + `notes/`
//! (per-note `.json` config + `.md` body) + `todos/` (per-todo `.json`).
//! V2 manifests keep single `.json` files with an HTML body inside.
//!
//! Writes are minimal deltas that keep the folder byte-compatible with what
//! Acta itself produces: item files keep their `{format, version, item}`
//! envelope, `updatedAt` uses ISO-8601 UTC, and the manifest (entry
//! `updatedAt` + `syncedAt`) is rewritten *last*, exactly like Acta does.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const MANIFEST_FILE: &str = "acta-manifest.json";
const CLASSIFICATIONS_FILE: &str = "classifications.json";

/// Serialize every write to the data folder; manifest rewrites must not race.
static WRITE_LOCK: Mutex<()> = Mutex::new(());

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

// --- writes ---------------------------------------------------------------
//
// Acta Handy edits items in place (a checked checkbox, a saved note) instead
// of rewriting the whole folder. That is exactly what Acta's reader accepts:
// item files keep their envelope, and the manifest — the only file readers
// treat as authoritative — is rewritten last.

/// Current time in Acta's format: ISO 8601 UTC with milliseconds (`…T…Z`).
pub fn now_iso() -> String {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let millis = d.subsec_millis();
    let (year, month, day) = civil_from_days(secs.div_euclid(86400));
    let rem = secs.rem_euclid(86400);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 → (year, month, day), Howard Hinnant's civil algorithm.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}

fn base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
    }
    out.iter().rev().map(|&b| b as char).collect()
}

/// New item ids, same shape as Acta's `uid()`: base36 timestamp + base36 salt.
pub fn gen_item_id() -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SALT: AtomicU32 = AtomicU32::new(0);
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let salt = SALT.fetch_add(1, Ordering::Relaxed) as u64
        ^ ((d.subsec_nanos() as u64) << 20)
        ^ ((std::process::id() as u64) << 8);
    format!("{}-{}", base36(d.as_millis() as u64), base36(salt % 2_176_782_336))
}

/// `item-<id as utf8 hex>` — Acta's file naming rule; its reader rejects anything else.
fn item_file_base_name(id: &str) -> String {
    let encoded: String = id
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("item-{encoded}")
}

fn write_file_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    fs::write(&tmp, bytes).map_err(|e| format!("无法写入 {}：{e}", path.display()))?;
    // Windows 的 rename 拒绝覆盖已存在的文件，编辑任何已有条目都会失败；
    // 那种情况下退化为 copy + 清理临时文件，保证两个平台都能落盘。
    if fs::rename(&tmp, path).is_err() {
        fs::copy(&tmp, path).map_err(|e| format!("无法保存 {}：{e}", path.display()))?;
        let _ = fs::remove_file(&tmp);
    }
    Ok(())
}

fn write_json_atomic(path: &Path, value: &Value) -> Result<(), String> {
    let mut json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    json.push('\n');
    write_file_atomic(path, json.as_bytes())
}

/// Refresh one entry's `updatedAt` and the manifest's `syncedAt`; the manifest
/// goes to disk last so readers never see a half-updated bundle.
fn touch_manifest(manifest: &mut Value, entry_id: &str, kind: &str, updated_at: &str) -> Result<(), String> {
    manifest["syncedAt"] = Value::String(updated_at.to_string());
    let entries = manifest
        .get_mut(kind)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("manifest 缺少 {kind} 数组"))?;
    let entry = entries
        .iter_mut()
        .find(|e| e.get("id").and_then(Value::as_str) == Some(entry_id))
        .ok_or_else(|| format!("manifest 中未找到条目：{entry_id}"))?;
    entry["updatedAt"] = Value::String(updated_at.to_string());
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCheck {
    pub id: String,
    pub done: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoCheckPatch {
    pub todo_id: String,
    pub completed: bool,
    /// Full subtask list with new done flags (the frontend applies Acta's
    /// completion rules before calling; we write exactly what it decided).
    pub tasks: Vec<TaskCheck>,
}

/// 写入前的条目快照：修改历史用它实现「恢复到当时」。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySnapshot {
    pub item: Value,
    /// V3 笔记的 `.md` 正文（item 里不含正文）；todo 与 V2 笔记为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// Check off a todo (or its subtasks). Mirrors Acta's `setTodoCompletion`:
/// `completed` and every task's `done` travel together, `updatedAt` refreshes.
/// Returns the todo as written plus the pre-write snapshot for the history.
pub fn write_todo_check(folder: &Path, patch: &TodoCheckPatch) -> Result<(ActaTodo, HistorySnapshot), String> {
    let _guard = WRITE_LOCK.lock().unwrap();
    let manifest_path = folder.join(MANIFEST_FILE);
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path)
            .map_err(|e| format!("读取 manifest 失败：{e}"))?,
    )
    .map_err(|e| format!("manifest 解析失败：{e}"))?;

    let file = {
        let entries = manifest
            .get("todos")
            .and_then(Value::as_array)
            .ok_or("manifest 缺少 todos 数组")?;
        let entry = entries
            .iter()
            .find(|e| e.get("id").and_then(Value::as_str) == Some(patch.todo_id.as_str()))
            .ok_or_else(|| format!("未找到待办：{}", patch.todo_id))?;
        jopt_str(entry, "file").ok_or_else(|| format!("待办 {} 缺少文件名", patch.todo_id))?
    };

    let todo_path = folder.join("todos").join(&file);
    let mut doc: Value = serde_json::from_str(
        &fs::read_to_string(&todo_path)
            .map_err(|e| format!("读取待办失败：{file}（{e}）"))?,
    )
    .map_err(|e| format!("待办解析失败：{file}（{e}）"))?;
    let item = doc
        .get_mut("item")
        .ok_or_else(|| format!("待办文件缺少 item：{file}"))?;

    let updated_at = now_iso();
    let snapshot = HistorySnapshot { item: item.clone(), body: None };
    item["completed"] = Value::Bool(patch.completed);
    if let Some(tasks) = item.get_mut("tasks").and_then(Value::as_array_mut) {
        for task in tasks {
            let Some(id) = task.get("id").and_then(Value::as_str) else {
                continue;
            };
            if let Some(check) = patch.tasks.iter().find(|t| t.id == id) {
                task["done"] = Value::Bool(check.done);
            }
        }
    }
    item["updatedAt"] = Value::String(updated_at.clone());

    let item_snapshot = item.clone();
    write_json_atomic(&todo_path, &doc)?;
    touch_manifest(&mut manifest, &patch.todo_id, "todos", &updated_at)?;
    write_json_atomic(&manifest_path, &manifest)?;

    Ok((parse_todo(&patch.todo_id, &item_snapshot), snapshot))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePatch {
    /// None → create a new note.
    pub note_id: Option<String>,
    pub title: Option<String>,
    pub body_markdown: Option<String>,
    /// Only used when creating; editing keeps the existing folder.
    pub folder_id: Option<String>,
}

/// Create or edit a note. V3 notes update the config JSON + `.md` body;
/// legacy V2 notes (single JSON with an HTML body) get their body written
/// back as escaped paragraphs — Acta upgrades those folders to V3 on next load.
/// Returns the note as written plus the pre-write snapshot (None = created).
pub fn write_note(folder: &Path, patch: &NotePatch) -> Result<(ActaNote, Option<HistorySnapshot>), String> {
    let _guard = WRITE_LOCK.lock().unwrap();
    let manifest_path = folder.join(MANIFEST_FILE);
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path)
            .map_err(|e| format!("读取 manifest 失败：{e}"))?,
    )
    .map_err(|e| format!("manifest 解析失败：{e}"))?;

    match patch.note_id.as_deref() {
        Some(note_id) if !note_id.is_empty() => {
            let (config_file, markdown_file, legacy_file) = {
                let entries = manifest
                    .get("notes")
                    .and_then(Value::as_array)
                    .ok_or("manifest 缺少 notes 数组")?;
                let entry = entries
                    .iter()
                    .find(|e| e.get("id").and_then(Value::as_str) == Some(note_id))
                    .ok_or_else(|| format!("未找到笔记：{note_id}"))?;
                (
                    jopt_str(entry, "config"),
                    jopt_str(entry, "markdown"),
                    jopt_str(entry, "file"),
                )
            };
            let updated_at = now_iso();
            if let Some(config) = config_file {
                // V3: config JSON + markdown sidecar.
                let config_path = folder.join("notes").join(&config);
                let mut doc: Value = serde_json::from_str(
                    &fs::read_to_string(&config_path)
                        .map_err(|e| format!("读取笔记失败：{config}（{e}）"))?,
                )
                .map_err(|e| format!("笔记解析失败：{config}（{e}）"))?;
                let snapshot = HistorySnapshot {
                    item: doc["item"].clone(),
                    body: markdown_file
                        .as_ref()
                        .and_then(|md| fs::read_to_string(folder.join("notes").join(md)).ok()),
                };
                if let Some(title) = &patch.title {
                    doc["item"]["title"] = Value::String(title.clone());
                }
                doc["item"]["updatedAt"] = Value::String(updated_at.clone());
                write_json_atomic(&config_path, &doc)?;
                if let Some(body) = &patch.body_markdown {
                    if let Some(md) = &markdown_file {
                        write_file_atomic(&folder.join("notes").join(md), body.as_bytes())?;
                    }
                }
                Ok((
                    parse_note(
                        note_id,
                        &doc["item"],
                        patch.body_markdown.clone().unwrap_or_default(),
                    ),
                    Some(snapshot),
                ))
            } else {
                // V2: single JSON with an HTML body.
                let file = legacy_file.ok_or_else(|| format!("笔记 {note_id} 缺少文件名"))?;
                let note_path = folder.join("notes").join(&file);
                let mut doc: Value = serde_json::from_str(
                    &fs::read_to_string(&note_path)
                        .map_err(|e| format!("读取笔记失败：{file}（{e}）"))?,
                )
                .map_err(|e| format!("笔记解析失败：{file}（{e}）"))?;
                let snapshot = HistorySnapshot { item: doc["item"].clone(), body: None };
                if let Some(title) = &patch.title {
                    doc["item"]["title"] = Value::String(title.clone());
                }
                if let Some(body) = &patch.body_markdown {
                    doc["item"]["body"] = Value::String(markdown_to_basic_html(body));
                }
                doc["item"]["updatedAt"] = Value::String(updated_at.clone());
                write_json_atomic(&note_path, &doc)?;
                let plain = patch
                    .body_markdown
                    .clone()
                    .unwrap_or_else(|| html_to_plain(&jstr(&doc["item"], "body")));
                Ok((parse_note(note_id, &doc["item"], plain), Some(snapshot)))
            }
        }
        _ => create_note(folder, &mut manifest, patch).map(|note| (note, None)),
    }
}

/// Restore an item to a pre-write snapshot: the snapshot's item is written
/// back whole (for V3 notes the markdown body too), `updatedAt` refreshes and
/// the manifest is rewritten last — exactly like any other write. Returns the
/// snapshot of what the item looked like *before* the restore, so the restore
/// itself becomes a step in the history.
pub fn restore_item(
    folder: &Path,
    kind: &str,
    item_id: &str,
    snap: &HistorySnapshot,
) -> Result<Option<HistorySnapshot>, String> {
    let _guard = WRITE_LOCK.lock().unwrap();
    let manifest_path = folder.join(MANIFEST_FILE);
    let mut manifest: Value = serde_json::from_str(
        &fs::read_to_string(&manifest_path).map_err(|e| format!("读取 manifest 失败：{e}"))?,
    )
    .map_err(|e| format!("manifest 解析失败：{e}"))?;

    let entries = manifest
        .get_mut(kind)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("manifest 缺少 {kind} 数组"))?;
    let entry = entries
        .iter()
        .find(|e| e.get("id").and_then(Value::as_str) == Some(item_id))
        .ok_or_else(|| format!("manifest 中未找到条目：{item_id}"))?
        .clone();
    let updated_at = now_iso();

    match kind {
        "todos" => {
            let file = jopt_str(&entry, "file").ok_or_else(|| format!("待办 {item_id} 缺少文件名"))?;
            let todo_path = folder.join("todos").join(&file);
            let mut doc: Value = serde_json::from_str(
                &fs::read_to_string(&todo_path).map_err(|e| format!("读取待办失败：{file}（{e}）"))?,
            )
            .map_err(|e| format!("待办解析失败：{file}（{e}）"))?;
            let previous = HistorySnapshot { item: doc["item"].clone(), body: None };
            doc["item"] = snap.item.clone();
            doc["item"]["updatedAt"] = Value::String(updated_at.clone());
            write_json_atomic(&todo_path, &doc)?;
            touch_manifest(&mut manifest, item_id, "todos", &updated_at)?;
            write_json_atomic(&manifest_path, &manifest)?;
            Ok(Some(previous))
        }
        "notes" => {
            let config_file = jopt_str(&entry, "config");
            let markdown_file = jopt_str(&entry, "markdown");
            let legacy_file = jopt_str(&entry, "file");
            if let Some(config) = config_file {
                // V3: config JSON + markdown sidecar.
                let config_path = folder.join("notes").join(&config);
                let mut doc: Value = serde_json::from_str(
                    &fs::read_to_string(&config_path)
                        .map_err(|e| format!("读取笔记失败：{config}（{e}）"))?,
                )
                .map_err(|e| format!("笔记解析失败：{config}（{e}）"))?;
                let previous = HistorySnapshot {
                    item: doc["item"].clone(),
                    body: markdown_file
                        .as_ref()
                        .and_then(|md| fs::read_to_string(folder.join("notes").join(md)).ok()),
                };
                doc["item"] = snap.item.clone();
                doc["item"]["updatedAt"] = Value::String(updated_at.clone());
                write_json_atomic(&config_path, &doc)?;
                if let Some(body) = &snap.body {
                    if let Some(md) = &markdown_file {
                        write_file_atomic(&folder.join("notes").join(md), body.as_bytes())?;
                    }
                }
                touch_manifest(&mut manifest, item_id, "notes", &updated_at)?;
                write_json_atomic(&manifest_path, &manifest)?;
                Ok(Some(previous))
            } else {
                // V2: single JSON with an HTML body (the body lives in the item).
                let file = legacy_file.ok_or_else(|| format!("笔记 {item_id} 缺少文件名"))?;
                let note_path = folder.join("notes").join(&file);
                let mut doc: Value = serde_json::from_str(
                    &fs::read_to_string(&note_path)
                        .map_err(|e| format!("读取笔记失败：{file}（{e}）"))?,
                )
                .map_err(|e| format!("笔记解析失败：{file}（{e}）"))?;
                let previous = HistorySnapshot { item: doc["item"].clone(), body: None };
                doc["item"] = snap.item.clone();
                doc["item"]["updatedAt"] = Value::String(updated_at.clone());
                write_json_atomic(&note_path, &doc)?;
                touch_manifest(&mut manifest, item_id, "notes", &updated_at)?;
                write_json_atomic(&manifest_path, &manifest)?;
                Ok(Some(previous))
            }
        }
        _ => Err(format!("未知的条目类型：{kind}")),
    }
}

fn create_note(folder: &Path, manifest: &mut Value, patch: &NotePatch) -> Result<ActaNote, String> {
    // 全新同步的数据文件夹可能还没有 notes/ 目录。
    fs::create_dir_all(folder.join("notes")).map_err(|e| format!("无法创建 notes 目录：{e}"))?;
    let id = gen_item_id();
    let base_name = item_file_base_name(&id);
    let config_file = format!("{base_name}.json");
    let markdown_file = format!("{base_name}.md");
    let updated_at = now_iso();

    let item = serde_json::json!({
        "id": id,
        "type": "note",
        "folderId": patch.folder_id.clone().unwrap_or_default(),
        "title": patch.title.clone().unwrap_or_default(),
        "linkedIds": [],
        "createdAt": updated_at,
        "updatedAt": updated_at,
    });
    let doc = serde_json::json!({
        "format": "acta-note-config",
        "version": 1,
        "contentFile": markdown_file,
        "item": item,
    });
    write_json_atomic(&folder.join("notes").join(&config_file), &doc)?;
    let body = patch.body_markdown.clone().unwrap_or_default();
    write_file_atomic(&folder.join("notes").join(&markdown_file), body.as_bytes())?;

    // Manifest last: register the new note and keep itemOrder authoritative.
    manifest["syncedAt"] = Value::String(updated_at.clone());
    let entry = serde_json::json!({
        "id": id,
        "markdown": markdown_file,
        "config": config_file,
        "updatedAt": updated_at,
    });
    manifest
        .get_mut("notes")
        .and_then(Value::as_array_mut)
        .ok_or("manifest 缺少 notes 数组")?
        .push(entry);
    if let Some(order) = manifest.get_mut("itemOrder").and_then(Value::as_array_mut) {
        order.push(Value::String(id.clone()));
    }
    write_json_atomic(&folder.join(MANIFEST_FILE), manifest)?;

    Ok(parse_note(&id, &doc["item"], body))
}

/// Minimal markdown → escaped HTML paragraphs for legacy V2 note bodies.
fn markdown_to_basic_html(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 32);
    for block in md.replace("\r\n", "\n").split("\n\n") {
        if block.trim().is_empty() {
            continue;
        }
        out.push_str("<p>");
        let mut first_line = true;
        for line in block.split('\n') {
            if !first_line {
                out.push_str("<br>");
            }
            first_line = false;
            for c in line.chars() {
                match c {
                    '&' => out.push_str("&amp;"),
                    '<' => out.push_str("&lt;"),
                    '>' => out.push_str("&gt;"),
                    _ => out.push(c),
                }
            }
        }
        out.push_str("</p>");
    }
    out
}
