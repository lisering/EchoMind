//! 对话记忆系统数据契约。
//!
//! 对应 SRS：REQ-RAG-032 持久化记忆系统增强。

use serde::{Deserialize, Serialize};

/// 记忆层级（借鉴 IfAI Wing/Hall/Room 空间隐喻）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    Wing,
    Hall,
    Room,
}

impl MemoryTier {
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "wing" => Some(Self::Wing),
            "hall" => Some(Self::Hall),
            "room" => Some(Self::Room),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Wing => "wing",
            Self::Hall => "hall",
            Self::Room => "room",
        }
    }
}

/// 记忆来源类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    UserStatement,
    AssistantAnswer,
    AutoExtracted,
    UserPinned,
}

impl MemorySource {
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "user_statement" => Some(Self::UserStatement),
            "assistant_answer" => Some(Self::AssistantAnswer),
            "auto_extracted" => Some(Self::AutoExtracted),
            "user_pinned" => Some(Self::UserPinned),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserStatement => "user_statement",
            Self::AssistantAnswer => "assistant_answer",
            Self::AutoExtracted => "auto_extracted",
            Self::UserPinned => "user_pinned",
        }
    }
}

/// 对话记忆条目（跨会话知识沉淀，REQ-RAG-032）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub tier: MemoryTier,
    pub content: String,
    pub source: MemorySource,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub conversation_id: Option<String>,
    pub created_at: i64,
    pub last_accessed: i64,
    pub access_count: u32,
    pub importance: f32,
}

impl MemoryEntry {
    pub fn new(content: String, source: MemorySource, tier: MemoryTier) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tier,
            content,
            source,
            conversation_id: None,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            importance: 0.5,
        }
    }

    pub fn new_with_conversation(
        content: String,
        source: MemorySource,
        tier: MemoryTier,
        conversation_id: String,
    ) -> Self {
        let mut entry = Self::new(content, source, tier);
        entry.conversation_id = Some(conversation_id);
        entry
    }
}

/// 记忆整合结果（由 AutoDream 后台调用 consolidate() 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    pub promoted: usize,
    pub forgotten: usize,
    pub remaining_wing: usize,
    pub remaining_hall: usize,
    pub remaining_room: usize,
}

/// 记忆整合动作（借鉴 QM consolidation.ts 的 UPDATE/DELETE/ADD 模式）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryConsolidationAction {
    Update { id: String, content: String },
    Delete { id: String },
    Add { content: String, tier: MemoryTier },
}

/// Scratch 层日志条目（借鉴 QM scratch-promote 的 daily log）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchLogEntry {
    pub id: String,
    pub date: String,
    pub content: String,
    pub created_at: i64,
}

impl ScratchLogEntry {
    pub fn new(content: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            date: now.format("%Y-%m-%d").to_string(),
            content,
            created_at: now.timestamp(),
        }
    }

    pub fn with_date(content: String, date: String, created_at: i64) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            date,
            content,
            created_at,
        }
    }
}

/// Scratch 层整合结果（由 `consolidate_scratch()` 返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScratchConsolidationResult {
    pub actions: Vec<MemoryConsolidationAction>,
    pub expired_cleaned: usize,
    pub remaining_scratch: usize,
}

/// 记忆来源标记（借鉴 QM provenance tracking）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceTag {
    pub conversation_id: String,
    pub message_seq: usize,
    pub captured_at: i64,
    pub source_label: String,
}

impl ProvenanceTag {
    pub fn new(conversation_id: String, message_seq: usize, source_label: String) -> Self {
        Self {
            conversation_id,
            message_seq,
            captured_at: chrono::Utc::now().timestamp(),
            source_label,
        }
    }

    pub fn with_timestamp(
        conversation_id: String,
        message_seq: usize,
        captured_at: i64,
        source_label: String,
    ) -> Self {
        Self {
            conversation_id,
            message_seq,
            captured_at,
            source_label,
        }
    }

    pub fn said_in_suffix(&self) -> String {
        format!(" (said in {})", self.source_label)
    }
}
