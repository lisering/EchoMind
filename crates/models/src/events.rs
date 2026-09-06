//! Tauri 事件载荷数据契约。

use serde::{Deserialize, Serialize};

/// 文档索引状态事件负载（Tauri Event `doc-status-changed`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocStatusPayload {
    pub status: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_phase: Option<String>,
}

/// 对话阶段事件负载（Tauri Event `chat_phase`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPhasePayload {
    pub phase: String,
    pub message: String,
}

/// 导入进度事件负载（Tauri Event `import-progress`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgressPayload {
    pub completed: usize,
    pub total: usize,
    pub current_file: String,
    pub cancelled: bool,
}

/// 向量化进度事件负载（Tauri Event `embedding_progress`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingProgressPayload {
    pub doc_id: String,
    pub doc_name: String,
    pub embedded: usize,
    pub total: usize,
}

/// Agentic RAG 步骤事件负载（Tauri Event `agent_step`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStepPayload {
    pub step_type: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    pub iteration: usize,
}
