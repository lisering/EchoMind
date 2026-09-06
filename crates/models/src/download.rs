//! 健壮下载系统数据模型。
//!
//! 对应 SRS：REQ-LLM-004 v2（RobustDownloader）。

use serde::{Deserialize, Serialize};

/// 下载源定义（多源容错）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSourceDef {
    pub name: String,
    pub base_url: String,
    pub branch: String,
}

/// 下载分块信息（并发分块下载用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadPart {
    pub index: usize,
    pub offset: u64,
    pub size: u64,
    pub completed: u64,
    pub retries: u32,
}

/// 下载状态机。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    #[default]
    Queued,
    Downloading {
        completed: u64,
        total: u64,
        speed: u64,
    },
    Paused {
        completed: u64,
        total: u64,
    },
    Verifying {
        completed: u64,
        total: u64,
    },
    Completed,
    Failed {
        error: String,
        completed: u64,
        total: u64,
    },
}

/// 下载清单 — 持久化到 `.meta.json`，崩溃后可恢复。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadManifest {
    pub filename: String,
    pub url: String,
    pub total_size: u64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sha256: Option<String>,
    pub sources: Vec<DownloadSourceDef>,
    pub parts: Vec<DownloadPart>,
    #[serde(default)]
    pub status: DownloadStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prefer_source: Option<usize>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 下载进度事件载荷（增强版，REQ-LLM-004 v2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressPayload {
    pub filename: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
    #[serde(default = "default_status_str")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

/// 下载状态摘要（用于 `list_pending_downloads` IPC 命令）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadStatusSummary {
    pub filename: String,
    pub status: DownloadStatus,
    pub total_size: u64,
    pub completed: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

fn default_status_str() -> String {
    "downloading".to_string()
}
