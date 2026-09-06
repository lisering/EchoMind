//! 对话/会话领域数据契约。
//!
//! 对应 SRS：REQ-RAG-001/004（ChatMessage）、REQ-RAG-006（Conversation）。

use serde::{Deserialize, Serialize};

use crate::document::RetrievalResult;

/// 对话消息（对应 REQ-RAG-001/004）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatMessage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub role: String,
    pub content: String,
    pub sources: Option<Vec<RetrievalResult>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i32>,
}

/// 会话（REQ-RAG-006）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub created_at: i64,
    pub sort_order: i64,
}

impl Conversation {
    pub fn new(workspace_id: String, title: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            sort_order: 0,
            workspace_id,
            title,
            created_at: chrono::Utc::now().timestamp(),
        }
    }

    pub fn with_id(id: String, workspace_id: String, title: String) -> Self {
        Self {
            id,
            workspace_id,
            title,
            created_at: chrono::Utc::now().timestamp(),
            sort_order: 0,
        }
    }
}

/// 持久化提示接纳（B05 Durable Prompt Admission）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingInput {
    pub id: String,
    pub conversation_id: String,
    pub content: String,
    pub delivery: String,
    pub created_at: i64,
    pub promoted_seq: Option<i64>,
}

impl PendingInput {
    pub fn new(conversation_id: String, content: String, delivery: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id,
            content,
            delivery,
            created_at: chrono::Utc::now().timestamp(),
            promoted_seq: None,
        }
    }
}

/// Todo 状态枚举（REQ-RAG-044 Session Todo 持久化）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

impl TodoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

/// Todo 优先级枚举（REQ-RAG-044 Session Todo 持久化）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoPriority {
    Low,
    Medium,
    High,
}

impl TodoPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

/// 会话待办项（REQ-RAG-044 Session Todo 持久化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTodo {
    pub id: String,
    pub conversation_id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
    pub position: i64,
    pub created_at: i64,
}

impl SessionTodo {
    pub fn new(conversation_id: String, content: String, position: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            conversation_id,
            content,
            status: TodoStatus::Pending,
            priority: TodoPriority::Medium,
            position,
            created_at: chrono::Utc::now().timestamp(),
        }
    }
}

/// 对话全文搜索结果（REQ-RAG-040）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchResult {
    pub message_id: String,
    pub conversation_id: String,
    pub conversation_title: String,
    pub role: String,
    pub content: String,
    pub score: f64,
    pub created_at: i64,
}

/// 全局搜索结果组（REQ-IX-008）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSearchResults {
    pub messages: Vec<MessageSearchResult>,
    pub documents: Vec<DocumentSearchResult>,
    pub entities: Vec<EntitySearchResult>,
}

/// 文档搜索结果（REQ-IX-008）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSearchResult {
    pub doc_id: String,
    pub file_path: String,
    pub summary: Option<String>,
    pub match_type: String,
}

/// 实体搜索结果（REQ-IX-008）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySearchResult {
    pub entity_text: String,
    pub entity_type: String,
    pub chunk_id: String,
    pub doc_id: String,
}

/// 轮次活跃版本记录（分支切换状态持久化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnActiveVersion {
    pub turn_group: String,
    pub active_version: i32,
}

/// 对话分支树节点（REQ-RAG-039 对话分支/版本树）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTreeNode {
    pub node_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
    #[serde(default)]
    pub child_message_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_child: Option<String>,
    pub created_at: i64,
    pub turn_group: String,
    pub version: i32,
    #[serde(default)]
    pub preview: String,
}

impl ConversationTreeNode {
    pub fn new(
        conversation_id: &str,
        turn_group: &str,
        version: i32,
        created_at: i64,
        preview: &str,
    ) -> Self {
        let node_id = format!("{turn_group}#{version}");
        let preview: String = if preview.chars().count() > 100 {
            let truncated: String = preview.chars().take(100).collect();
            format!("{truncated}…")
        } else {
            preview.to_string()
        };
        Self {
            node_id,
            conversation_id: conversation_id.to_string(),
            parent_message_id: None,
            child_message_ids: Vec::new(),
            active_child: None,
            created_at,
            turn_group: turn_group.to_string(),
            version,
            preview,
        }
    }

    pub fn is_root(&self) -> bool {
        self.parent_message_id.is_none()
    }

    pub fn is_leaf(&self) -> bool {
        self.child_message_ids.is_empty()
    }
}

/// 对话分支树（REQ-RAG-039）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTree {
    pub conversation_id: String,
    pub nodes: Vec<ConversationTreeNode>,
    pub root_ids: Vec<String>,
    pub active_path: Vec<String>,
}

impl ConversationTree {
    pub fn empty(conversation_id: &str) -> Self {
        Self {
            conversation_id: conversation_id.to_string(),
            nodes: Vec::new(),
            root_ids: Vec::new(),
            active_path: Vec::new(),
        }
    }

    pub fn find(&self, node_id: &str) -> Option<&ConversationTreeNode> {
        self.nodes.iter().find(|n| n.node_id == node_id)
    }

    pub fn children_of(&self, node_id: &str) -> Vec<&ConversationTreeNode> {
        self.nodes
            .iter()
            .filter(|n| n.parent_message_id.as_deref() == Some(node_id))
            .collect()
    }
}

/// 对话书签（REQ-RAG-047）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationBookmark {
    pub conversation_id: String,
    pub note: Option<String>,
    pub created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

impl ConversationBookmark {
    pub fn new(conversation_id: String) -> Self {
        Self {
            conversation_id,
            note: None,
            created_at: chrono::Utc::now().timestamp(),
            message_id: None,
            summary: None,
        }
    }

    pub fn new_message(conversation_id: String, message_id: String, content: &str) -> Self {
        let summary: String = content.chars().take(50).collect();
        Self {
            conversation_id,
            note: None,
            created_at: chrono::Utc::now().timestamp(),
            message_id: Some(message_id),
            summary: Some(summary),
        }
    }

    pub fn with_note(mut self, note: String) -> Self {
        self.note = Some(note);
        self
    }

    pub fn is_message_bookmark(&self) -> bool {
        self.message_id.is_some()
    }
}

/// 对话历史压缩结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    pub history: Vec<ChatMessage>,
    pub info: Option<CompactionInfo>,
}

/// 对话历史压缩信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionInfo {
    pub compacted_count: usize,
    pub total_tokens: usize,
    pub compacted_tokens: usize,
    pub token_limit: usize,
    #[serde(default)]
    pub compaction_kind: Option<CompactionKind>,
}

/// 压缩触发类型（Q03：双阈值压缩）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionKind {
    Background,
    Synchronous,
}

impl CompactionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Synchronous => "synchronous",
        }
    }
}

/// 对话上下文截断信息（REQ-RAG-017 AC-2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryTruncationPayload {
    pub truncated_count: usize,
    pub total_tokens: usize,
    pub retained_tokens: usize,
    pub token_limit: usize,
}

/// Token 预算配置（S69：Cherry Studio 借鉴）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetConfig {
    #[serde(default = "crate::default_context_token_limit")]
    pub max_tokens: usize,
    #[serde(default = "crate::default_compaction_threshold")]
    pub compaction_threshold: f32,
    #[serde(default = "crate::default_recent_keep_ratio")]
    pub recent_keep_ratio: f32,
    #[serde(default = "crate::default_min_messages_to_compact")]
    pub min_messages_to_compact: usize,
}

impl Default for TokenBudgetConfig {
    fn default() -> Self {
        Self {
            max_tokens: crate::default_context_token_limit(),
            compaction_threshold: crate::default_compaction_threshold(),
            recent_keep_ratio: crate::default_recent_keep_ratio(),
            min_messages_to_compact: crate::default_min_messages_to_compact(),
        }
    }
}

impl TokenBudgetConfig {
    pub fn compaction_trigger(&self) -> usize {
        (self.max_tokens as f32 * self.compaction_threshold) as usize
    }

    pub fn recent_budget(&self) -> usize {
        (self.max_tokens as f32 * self.recent_keep_ratio) as usize
    }

    pub fn should_compact(&self, total_tokens: usize, message_count: usize) -> bool {
        message_count >= self.min_messages_to_compact && total_tokens >= self.compaction_trigger()
    }
}

/// Strip 操作配置（REQ-RAG-046，DS4 DS-03 借鉴）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripConfig {
    pub from_index: usize,
    pub to_index: usize,
    pub replace_with_summary: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub summary_text: Option<String>,
}

impl StripConfig {
    pub fn new(from_index: usize, to_index: usize) -> Self {
        Self {
            from_index,
            to_index,
            replace_with_summary: false,
            summary_text: None,
        }
    }

    pub fn with_summary(mut self, summary: String) -> Self {
        self.replace_with_summary = true;
        self.summary_text = Some(summary);
        self
    }
}

/// Strip 操作结果（REQ-RAG-046 + DS-03 Session Strip）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StripResult {
    pub stripped_count: usize,
    pub summary_inserted: bool,
    pub stripped_message_ids: Vec<String>,
    pub estimated_tokens_saved: usize,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub kept_count: usize,
}

impl StripResult {
    pub fn empty() -> Self {
        Self {
            stripped_count: 0,
            summary_inserted: false,
            stripped_message_ids: Vec::new(),
            estimated_tokens_saved: 0,
            summary: String::new(),
            kept_count: 0,
        }
    }
}

/// Strip 预览（REQ-RAG-046）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StripPreview {
    pub messages: Vec<ChatMessage>,
    pub total_messages: usize,
    pub estimated_tokens_saved: usize,
}

impl StripPreview {
    pub fn empty(total_messages: usize) -> Self {
        Self {
            messages: Vec::new(),
            total_messages,
            estimated_tokens_saved: 0,
        }
    }
}
