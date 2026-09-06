//! 增量同步领域数据契约。
//!
//! 对应 SRS：REQ-SYNC-001/002。

use serde::{Deserialize, Serialize};

/// 增量同步结果（REQ-SYNC-002）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

/// 监听文件夹信息（REQ-SYNC-001）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchedFolderInfo {
    pub path: String,
    pub name: String,
    pub sync_status: String,
    pub last_synced_at: Option<i64>,
}

/// 同步进度事件负载（Tauri Event `sync_progress`，REQ-SYNC-002 AC-7）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgressPayload {
    pub phase: String,
    pub folder_path: String,
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub message: String,
}
