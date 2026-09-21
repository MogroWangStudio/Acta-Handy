//! 修改历史：每次写回 Acta 数据文件夹前，把写入前的条目状态快照下来。
//!
//! 历史保存在应用自己的设置目录（Windows 便携版与设置同目录、随 exe 走），
//! 绝不进 Acta 数据文件夹——那里的多余文件会被改动监控误判为 Acta 同步。
//! 每条记录保存「写入前的完整 item」（笔记附带正文），因此可以从任意一条
//! 历史把条目恢复回当时的模样；恢复本身也是一次写入，同样会留下记录。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::acta::{self, HistorySnapshot};

const HISTORY_FILE: &str = "handy-history.json";
/// 只回溯最近的历史：条目快照含笔记全文，不设上限会让历史文件无限膨胀。
const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    /// 写入时刻（ISO 8601 UTC，与 Acta 条目的时间格式一致）。
    pub time: String,
    /// todo-check | note-edit | note-create | todo-restore | note-restore
    pub kind: String,
    /// 这条写入指向的数据文件夹；恢复时校验，避免跨数据源错写。
    pub folder: String,
    pub item_id: String,
    /// 改动后条目的标题（摘要展示用）。
    pub title: String,
    /// todo 类记录改动后的完成状态。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed: Option<bool>,
    /// 写入前快照；新建笔记没有「之前」，为 None。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<HistorySnapshot>,
}

fn history_path(app: &AppHandle) -> Option<PathBuf> {
    crate::settings::data_dir(app).map(|dir| dir.join(HISTORY_FILE))
}

fn load_all(app: &AppHandle) -> Vec<HistoryEntry> {
    let Some(path) = history_path(app) else {
        return Vec::new();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<HistoryEntry>>(&raw).ok())
        .unwrap_or_default()
}

/// 某个数据文件夹的最近修改历史（新→旧）。文件缺失或损坏视为没有历史。
pub fn load(app: &AppHandle, folder: &str) -> Vec<HistoryEntry> {
    load_all(app)
        .into_iter()
        .filter(|e| e.folder == folder)
        .collect()
}

fn save(app: &AppHandle, entries: &[HistoryEntry]) {
    let Some(path) = history_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = fs::write(&path, json);
    }
}

/// 记录一次写入。历史落盘失败不影响数据写入本身，静默即可。
pub fn record(
    app: &AppHandle,
    folder: &str,
    kind: &str,
    item_id: &str,
    title: &str,
    completed: Option<bool>,
    before: Option<HistorySnapshot>,
) {
    let mut entries = load_all(app);
    entries.insert(
        0,
        HistoryEntry {
            id: format!("h-{}", acta::gen_item_id()),
            time: acta::now_iso(),
            kind: kind.to_string(),
            folder: folder.to_string(),
            item_id: item_id.to_string(),
            title: title.to_string(),
            completed,
            before,
        },
    );
    entries.truncate(HISTORY_LIMIT);
    save(app, &entries);
}

pub fn find(app: &AppHandle, entry_id: &str) -> Option<HistoryEntry> {
    load_all(app).into_iter().find(|e| e.id == entry_id)
}
