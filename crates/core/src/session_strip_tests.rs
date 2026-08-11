#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Session Strip TDD 测试（TC-STRIP-001~013，REQ-RAG-046）。
//!
//! 测试覆盖：
//! - TC-STRIP-001: strip_range 基本删除
//! - TC-STRIP-002: strip_range 带摘要替代
//! - TC-STRIP-003: strip_keeping_recent 保留最后 N 条
//! - TC-STRIP-004: strip_keeping_recent keep_last_n=0 删除全部
//! - TC-STRIP-005: preview 不执行实际删除
//! - TC-STRIP-006: 索引越界返回 Err
//! - TC-STRIP-007: 空对话 strip 返回空结果
//! - TC-STRIP-008: estimate_tokens 粗略估算
//! - TC-STRIP-009: StripConfig serde 往返一致
//! - TC-STRIP-010: StripResult/StripPreview serde 往返一致
//! - TC-STRIP-011: delete_messages_by_ids Storage trait 默认空操作
//! - TC-STRIP-012: strip 后 list_messages 不包含已删除消息
//! - TC-STRIP-013: 摘要 system 消息 role 和 content 验证

use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use echomind_models::{
    ChatMessage, Chunk, Conversation, DocStatus, Document, RetrievalResult, StripConfig,
    StripPreview, StripResult,
};

use crate::Storage;
use crate::session_strip::SessionStripper;

// ============================================================
// Mock Storage（支持消息删除）
// ============================================================

/// 内存 Mock Storage，支持消息 CRUD + 按ID批量删除。
#[derive(Clone, Default)]
struct MockStorage {
    conversations: Arc<Mutex<Vec<Conversation>>>,
    messages: Arc<Mutex<Vec<(String, ChatMessage)>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self::default()
    }

    /// 添加测试消息。
    fn add_test_message(&self, conversation_id: &str, role: &str, content: &str, id: &str) {
        let msg = ChatMessage {
            id: Some(id.to_string()),
            role: role.to_string(),
            content: content.to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: None,
        };
        self.messages
            .lock()
            .unwrap()
            .push((conversation_id.to_string(), msg));
    }

    /// 创建包含 N 条消息的测试会话。
    fn with_n_messages(conversation_id: &str, n: usize) -> Self {
        let storage = Self::new();
        for i in 0..n {
            let id = format!("msg-{i}");
            let role = if i % 2 == 0 { "user" } else { "assistant" };
            let content = format!("消息 {i}：这是一段测试内容，长度约二十个字符。");
            storage.add_test_message(conversation_id, role, &content, &id);
        }
        storage
    }

    /// 获取指定会话的消息数量。
    fn message_count(&self, conversation_id: &str) -> usize {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter(|(cid, _)| cid == conversation_id)
            .count()
    }
}

impl Storage for MockStorage {
    async fn add_document(&self, _doc: &Document) -> Result<()> {
        Ok(())
    }
    async fn update_doc_status(&self, _doc_id: &str, _status: DocStatus) -> Result<()> {
        Ok(())
    }
    async fn add_chunk(&self, _chunk: &Chunk) -> Result<()> {
        Ok(())
    }
    async fn add_embedding(&self, _chunk_id: &str, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
    async fn find_document_by_hash(&self, _hash: &str) -> Result<Option<Document>> {
        Ok(None)
    }
    async fn count_documents(&self) -> Result<usize> {
        Ok(0)
    }
    async fn count_chunks(&self) -> Result<usize> {
        Ok(0)
    }
    async fn cleanup_zombies(&self) -> Result<usize> {
        Ok(0)
    }
    async fn set_setting(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
    async fn get_setting(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    async fn create_conversation(&self, conv: &Conversation) -> Result<()> {
        self.conversations.lock().unwrap().push(conv.clone());
        Ok(())
    }
    async fn list_conversations(&self, _workspace_id: &str) -> Result<Vec<Conversation>> {
        Ok(self.conversations.lock().unwrap().clone())
    }
    async fn delete_conversation(&self, id: &str) -> Result<()> {
        self.conversations.lock().unwrap().retain(|c| c.id != id);
        self.messages.lock().unwrap().retain(|(cid, _)| cid != id);
        Ok(())
    }
    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }
    async fn add_message(&self, conversation_id: &str, message: &ChatMessage) -> Result<()> {
        let id = message
            .id
            .clone()
            .unwrap_or_else(|| format!("msg-{}", uuid::Uuid::new_v4()));
        self.messages.lock().unwrap().push((
            conversation_id.to_string(),
            ChatMessage {
                id: Some(id),
                ..message.clone()
            },
        ));
        Ok(())
    }
    async fn list_messages(&self, conversation_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(self
            .messages
            .lock()
            .unwrap()
            .iter()
            .filter(|(cid, _)| cid == conversation_id)
            .map(|(_, msg)| msg.clone())
            .collect())
    }
    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(vec![])
    }
    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn list_chunks(&self, _doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(vec![])
    }
    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
    async fn delete_messages_by_ids(
        &self,
        conversation_id: &str,
        message_ids: &[String],
    ) -> Result<usize> {
        let mut msgs = self.messages.lock().unwrap();
        let before = msgs.len();
        msgs.retain(|(cid, msg)| {
            cid != conversation_id
                || !message_ids
                    .iter()
                    .any(|id| msg.id.as_deref() == Some(id.as_str()))
        });
        Ok(before - msgs.len())
    }
}

// ============================================================
// TDD 测试
// ============================================================

/// TC-STRIP-001：strip_range 按闭区间删除指定消息（AC-1）。
#[tokio::test]
async fn tc_strip_001_basic_range_strip() {
    let storage = MockStorage::with_n_messages("conv-1", 6);
    let config = StripConfig::new(1, 3);

    let result = SessionStripper::strip_range(&storage, "conv-1", &config)
        .await
        .unwrap();

    assert_eq!(result.stripped_count, 3, "应删除 3 条消息（索引 1-3）");
    assert!(!result.summary_inserted, "未请求摘要，不应插入");
    assert_eq!(result.stripped_message_ids.len(), 3, "应返回 3 个消息 ID");
    assert!(result.estimated_tokens_saved > 0, "应有 token 节省估算");
    assert_eq!(storage.message_count("conv-1"), 3, "strip 后应剩 3 条消息");
}

/// TC-STRIP-002：strip_range 带摘要替代（AC-2）。
#[tokio::test]
async fn tc_strip_002_strip_with_summary() {
    let storage = MockStorage::with_n_messages("conv-1", 6);
    let config = StripConfig::new(0, 2).with_summary("前 3 条消息的摘要".to_string());

    let result = SessionStripper::strip_range(&storage, "conv-1", &config)
        .await
        .unwrap();

    assert_eq!(result.stripped_count, 3, "应删除 3 条消息");
    assert!(result.summary_inserted, "应插入摘要");
    assert_eq!(
        storage.message_count("conv-1"),
        4,
        "strip 3 + 插入 1 摘要 = 6 - 3 + 1 = 4 条消息"
    );
}

/// TC-STRIP-003：strip_keeping_recent 保留最后 N 条（AC-3）。
#[tokio::test]
async fn tc_strip_003_keeping_recent() {
    let storage = MockStorage::with_n_messages("conv-1", 10);

    let result = SessionStripper::strip_keeping_recent(&storage, "conv-1", 3, false, None)
        .await
        .unwrap();

    assert_eq!(result.stripped_count, 7, "应删除 7 条消息（保留最后 3 条）");
    assert_eq!(storage.message_count("conv-1"), 3, "应剩 3 条消息");
}

/// TC-STRIP-004：strip_keeping_recent keep_last_n=0 删除全部（AC-4）。
#[tokio::test]
async fn tc_strip_004_keeping_recent_zero() {
    let storage = MockStorage::with_n_messages("conv-1", 5);

    let result = SessionStripper::strip_keeping_recent(&storage, "conv-1", 0, false, None)
        .await
        .unwrap();

    assert_eq!(result.stripped_count, 5, "应删除全部 5 条消息");
    assert_eq!(storage.message_count("conv-1"), 0, "应剩 0 条消息");
}

/// TC-STRIP-005：preview 不执行实际删除（AC-5）。
#[tokio::test]
async fn tc_strip_005_preview_no_delete() {
    let storage = MockStorage::with_n_messages("conv-1", 6);

    let preview = SessionStripper::preview(&storage, "conv-1", 1, 3)
        .await
        .unwrap();

    assert_eq!(preview.messages.len(), 3, "预览应包含 3 条消息");
    assert_eq!(preview.total_messages, 6, "总消息数应为 6");
    assert!(preview.estimated_tokens_saved > 0, "应有 token 估算");
    // 关键：预览不执行删除
    assert_eq!(storage.message_count("conv-1"), 6, "预览不应删除任何消息");
}

/// TC-STRIP-006：索引越界返回 Err（AC-6）。
#[tokio::test]
async fn tc_strip_006_index_out_of_bounds() {
    let storage = MockStorage::with_n_messages("conv-1", 5);

    // from_index > to_index
    let config = StripConfig::new(3, 1);
    let result = SessionStripper::strip_range(&storage, "conv-1", &config).await;
    assert!(result.is_err(), "from_index > to_index 应返回 Err");

    // to_index >= total
    let config = StripConfig::new(0, 5);
    let result = SessionStripper::strip_range(&storage, "conv-1", &config).await;
    assert!(result.is_err(), "to_index >= total 应返回 Err");
}

/// TC-STRIP-007：空对话 strip 返回空结果（AC-7）。
#[tokio::test]
async fn tc_strip_007_empty_conversation() {
    let storage = MockStorage::new();
    let config = StripConfig::new(0, 3);

    let result = SessionStripper::strip_range(&storage, "conv-1", &config)
        .await
        .unwrap();

    assert_eq!(result.stripped_count, 0, "空对话 stripped_count 应为 0");
    assert!(!result.summary_inserted);
    assert!(result.stripped_message_ids.is_empty());
    assert_eq!(result.estimated_tokens_saved, 0);
}

/// TC-STRIP-008：estimate_tokens 使用 4 字符 ≈ 1 token（AC-8）。
#[test]
fn tc_strip_008_estimate_tokens() {
    // 8 字符 → 2 token
    assert_eq!(SessionStripper::estimate_tokens("12345678"), 2);

    // 7 字符 → 1 token（整数除法）
    assert_eq!(SessionStripper::estimate_tokens("1234567"), 1);

    // 0 字符 → 0 token
    assert_eq!(SessionStripper::estimate_tokens(""), 0);

    // 100 字符 → 25 token
    let long_text = "a".repeat(100);
    assert_eq!(SessionStripper::estimate_tokens(&long_text), 25);
}

/// TC-STRIP-009：StripConfig serde 序列化往返一致（AC-9）。
#[test]
fn tc_strip_009_strip_config_serde_roundtrip() {
    let config = StripConfig::new(1, 5).with_summary("测试摘要".to_string());
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: StripConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.from_index, 1);
    assert_eq!(deserialized.to_index, 5);
    assert!(deserialized.replace_with_summary);
    assert_eq!(deserialized.summary_text.as_deref(), Some("测试摘要"));

    // 不带摘要的配置
    let config2 = StripConfig::new(0, 2);
    let json2 = serde_json::to_string(&config2).unwrap();
    let deserialized2: StripConfig = serde_json::from_str(&json2).unwrap();
    assert!(!deserialized2.replace_with_summary);
    assert!(deserialized2.summary_text.is_none());
}

/// TC-STRIP-010：StripResult/StripPreview serde 往返一致（AC-9）。
#[test]
fn tc_strip_010_strip_result_preview_serde_roundtrip() {
    let result = StripResult {
        stripped_count: 3,
        summary_inserted: true,
        stripped_message_ids: vec!["msg-1".to_string(), "msg-2".to_string()],
        estimated_tokens_saved: 50,
        summary: String::new(),
        kept_count: 0,
    };
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: StripResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.stripped_count, 3);
    assert!(deserialized.summary_inserted);
    assert_eq!(deserialized.stripped_message_ids.len(), 2);
    assert_eq!(deserialized.estimated_tokens_saved, 50);

    let preview = StripPreview {
        messages: vec![],
        total_messages: 10,
        estimated_tokens_saved: 100,
    };
    let json = serde_json::to_string(&preview).unwrap();
    let deserialized: StripPreview = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.total_messages, 10);
    assert_eq!(deserialized.estimated_tokens_saved, 100);
}

/// TC-STRIP-011：delete_messages_by_ids Storage trait 默认空操作（AC-10）。
#[tokio::test]
async fn tc_strip_011_delete_messages_default_noop() {
    // 使用 MockStorage，验证 delete_messages_by_ids 返回删除数量
    let storage = MockStorage::with_n_messages("conv-1", 3);
    let ids = vec!["msg-0".to_string(), "msg-1".to_string()];

    let deleted = storage
        .delete_messages_by_ids("conv-1", &ids)
        .await
        .unwrap();

    assert_eq!(deleted, 2, "应删除 2 条消息");
    assert_eq!(storage.message_count("conv-1"), 1, "应剩 1 条消息");
}

/// TC-STRIP-012：strip 后 list_messages 不包含已删除消息（AC-11）。
#[tokio::test]
async fn tc_strip_012_list_after_strip() {
    let storage = MockStorage::with_n_messages("conv-1", 6);
    let config = StripConfig::new(2, 4);

    SessionStripper::strip_range(&storage, "conv-1", &config)
        .await
        .unwrap();

    let remaining = storage.list_messages("conv-1").await.unwrap();
    assert_eq!(remaining.len(), 3, "应剩 3 条消息");

    // 验证剩余消息不包含已删除的 msg-2, msg-3, msg-4
    let remaining_ids: Vec<&str> = remaining.iter().filter_map(|m| m.id.as_deref()).collect();
    assert!(!remaining_ids.contains(&"msg-2"));
    assert!(!remaining_ids.contains(&"msg-3"));
    assert!(!remaining_ids.contains(&"msg-4"));
    assert!(remaining_ids.contains(&"msg-0"));
    assert!(remaining_ids.contains(&"msg-1"));
    assert!(remaining_ids.contains(&"msg-5"));
}

/// TC-STRIP-013：摘要 system 消息 role 和 content 验证（AC-12）。
#[tokio::test]
async fn tc_strip_013_summary_message_role_content() {
    let storage = MockStorage::with_n_messages("conv-1", 4);
    let summary = "这是被 strip 消息的摘要内容";
    let config = StripConfig::new(0, 1).with_summary(summary.to_string());

    SessionStripper::strip_range(&storage, "conv-1", &config)
        .await
        .unwrap();

    let messages = storage.list_messages("conv-1").await.unwrap();

    // 查找 system 消息
    let system_msg = messages.iter().find(|m| m.role == "system");
    assert!(system_msg.is_some(), "应存在一条 system 消息");

    let system_msg = system_msg.unwrap();
    assert!(
        system_msg.content.contains(summary),
        "system 消息内容应包含摘要文本"
    );
    assert!(
        system_msg.content.contains("[摘要]"),
        "system 消息内容应包含 [摘要] 前缀"
    );
}
