//! 知识库/工作空间领域数据契约。
//!
//! 对应 SRS：REQ-WS-001 多知识库创建与切换。

use serde::{Deserialize, Serialize};

/// 知识库/工作空间（REQ-WS-001 多知识库创建与切换）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

impl Workspace {
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn with_id(id: String, name: String) -> Self {
        Self {
            id,
            name,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 工作空间数据量预览（REQ-WS-003 删除前确认对话框）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStats {
    pub document_count: usize,
    pub conversation_count: usize,
}
