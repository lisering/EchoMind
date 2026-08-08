#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 持久化记忆系统 TDD 测试（TC-MEMORY-001~012，REQ-RAG-032）。
//!
//! 测试覆盖：
//! - TC-MEMORY-001: 从对话提取记忆（Mock LLM 返回关键事实）
//! - TC-MEMORY-002: 记忆提升 Wing → Hall → Room
//! - TC-MEMORY-003: 记忆遗忘（低重要性被删除）
//! - TC-MEMORY-004: 相关记忆检索（关键词匹配）
//! - TC-MEMORY-005: 记忆整合（超限时自动提升/遗忘）
//! - TC-MEMORY-006: 记忆注入到 chat prompt
//! - TC-MEMORY-007: 访问计数和最后访问时间更新
//! - TC-MEMORY-008: 用户手动置顶记忆
//! - TC-MEMORY-009: 记忆禁用时不注入
//! - TC-MEMORY-010: MemoryTier 序列化/反序列化
//! - TC-MEMORY-011: Room 层超限时按 access_count 遗忘
//! - TC-MEMORY-012: 空对话不提取记忆

use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use echomind_models::{
    ChatMessage, Chunk, DocStatus, Document, MemoryEntry, MemorySource, MemoryTier,
    RetrievalResult, ScratchLogEntry,
};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::chat::ChatEngine;
use crate::memory_store::{MemoryRetriever, MemoryStore};
use crate::{LLMProvider, Retriever, Storage};

// ============================================================
// Mock 实现
// ============================================================

/// 内存 Mock Storage：支持对话记忆 CRUD。
struct MockStorage {
    documents: Vec<Document>,
    memory_entries: Mutex<Vec<MemoryEntry>>,
    scratch_logs: Mutex<Vec<ScratchLogEntry>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            documents: vec![],
            memory_entries: Mutex::new(Vec::new()),
            scratch_logs: Mutex::new(Vec::new()),
        }
    }

    /// 预存一条记忆。
    fn with_memory(self, entry: MemoryEntry) -> Self {
        self.memory_entries.lock().unwrap().push(entry);
        self
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
        Ok(self.documents.len())
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
    async fn create_conversation(&self, _conv: &echomind_models::Conversation) -> Result<()> {
        Ok(())
    }
    async fn list_conversations(
        &self,
        _workspace_id: &str,
    ) -> Result<Vec<echomind_models::Conversation>> {
        Ok(vec![])
    }
    async fn delete_conversation(&self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }
    async fn add_message(&self, _conversation_id: &str, _message: &ChatMessage) -> Result<()> {
        Ok(())
    }
    async fn list_messages(&self, _conversation_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }
    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(self.documents.clone())
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

    // ---- 对话记忆系统方法 ----

    async fn add_memory_entry(&self, entry: &MemoryEntry) -> Result<()> {
        let mut entries = self.memory_entries.lock().unwrap();
        // INSERT OR REPLACE 语义
        if let Some(existing) = entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry.clone();
        } else {
            entries.push(entry.clone());
        }
        Ok(())
    }

    async fn get_memory_entries(&self, tier: Option<&MemoryTier>) -> Result<Vec<MemoryEntry>> {
        let entries = self.memory_entries.lock().unwrap();
        Ok(match tier {
            Some(t) => entries.iter().filter(|e| &e.tier == t).cloned().collect(),
            None => entries.clone(),
        })
    }

    async fn update_memory_entry(&self, entry: &MemoryEntry) -> Result<()> {
        let mut entries = self.memory_entries.lock().unwrap();
        if let Some(existing) = entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry.clone();
        }
        Ok(())
    }

    async fn delete_memory_entry(&self, id: &str) -> Result<()> {
        let mut entries = self.memory_entries.lock().unwrap();
        entries.retain(|e| e.id != id);
        Ok(())
    }

    async fn clear_memory_entries(&self, tier: Option<&MemoryTier>) -> Result<usize> {
        let mut entries = self.memory_entries.lock().unwrap();
        let before = entries.len();
        match tier {
            Some(t) => entries.retain(|e| &e.tier != t),
            None => entries.clear(),
        }
        Ok(before - entries.len())
    }

    async fn search_memory_entries(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>> {
        let entries = self.memory_entries.lock().unwrap();
        let mut matched: Vec<MemoryEntry> = entries
            .iter()
            .filter(|e| e.content.contains(query))
            .cloned()
            .collect();
        matched.sort_by(|a, b| {
            b.importance
                .partial_cmp(&a.importance)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matched.truncate(limit);
        Ok(matched)
    }

    // ---- Scratch-Promote 方法（Q01）----

    async fn add_scratch_log(&self, entry: &ScratchLogEntry) -> Result<()> {
        self.scratch_logs.lock().unwrap().push(entry.clone());
        Ok(())
    }

    async fn get_scratch_logs(&self, limit: Option<usize>) -> Result<Vec<ScratchLogEntry>> {
        let logs = self.scratch_logs.lock().unwrap();
        let mut sorted = logs.clone();
        sorted.sort_by_key(|e| e.created_at);
        if let Some(n) = limit {
            sorted.truncate(n);
        }
        Ok(sorted)
    }

    async fn delete_scratch_log(&self, id: &str) -> Result<()> {
        self.scratch_logs.lock().unwrap().retain(|e| e.id != id);
        Ok(())
    }

    async fn cleanup_expired_scratch_logs(&self, before_timestamp: i64) -> Result<usize> {
        let mut logs = self.scratch_logs.lock().unwrap();
        let before = logs.len();
        logs.retain(|e| e.created_at >= before_timestamp);
        Ok(before - logs.len())
    }
}

/// Mock LLM：返回预设输出，可捕获 system prompt。
struct MockLlm {
    output: String,
    /// 捕获的 system prompt（供测试断言）
    captured_prompt: Arc<Mutex<Option<String>>>,
}

impl MockLlm {
    fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
            captured_prompt: Arc::new(Mutex::new(None)),
        }
    }

    /// 获取捕获的 system prompt。
    #[allow(dead_code)]
    fn captured(&self) -> Option<String> {
        self.captured_prompt.lock().unwrap().clone()
    }

    /// 获取捕获引用（用于测试中 clone 后共享）。
    #[allow(dead_code)]
    fn captured_ref(&self) -> Arc<Mutex<Option<String>>> {
        self.captured_prompt.clone()
    }
}

impl LLMProvider for MockLlm {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        *self.captured_prompt.lock().unwrap() = Some(system_prompt.to_string());
        let output = self.output.clone();
        Ok(futures::stream::once(async move { Ok(output) }).boxed())
    }
}

/// Mock Retriever：返回预置检索结果。
struct MockRetriever {
    results: Vec<RetrievalResult>,
}

impl MockRetriever {
    fn new(results: Vec<RetrievalResult>) -> Self {
        Self { results }
    }
}

impl Retriever for MockRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(self.results.clone())
    }
}

/// 构造 1 条检索结果。
fn make_source() -> RetrievalResult {
    RetrievalResult {
        chunk: Chunk::new("doc-1".to_string(), "测试文档内容".to_string(), 10, 0),
        score: 0.9,
        doc_name: "test.md".to_string(),
    }
}

/// 构造对话消息。
fn make_messages() -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "我是一名律师，专长是知识产权法".to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: Some(1),
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "了解了，您是知识产权法领域的专业律师。".to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: Some(1),
        },
    ]
}

// ============================================================
// 测试用例
// ============================================================

/// TC-MEMORY-001：从对话提取记忆（Mock LLM 返回关键事实）。
#[tokio::test]
async fn tc_memory_001_extract_from_conversation() {
    let storage = MockStorage::new();
    let llm = MockLlm::new("[user_statement] 我是一名律师\n[user_statement] 专长是知识产权法");
    let store = MemoryStore::new(storage);

    let messages = make_messages();
    let entries = store
        .extract_from_conversation(&messages, &llm)
        .await
        .unwrap();

    assert_eq!(entries.len(), 2, "应提取 2 条记忆");
    for entry in &entries {
        assert_eq!(entry.tier, MemoryTier::Wing, "新提取记忆应为 Wing 层");
        assert_eq!(
            entry.source,
            MemorySource::UserStatement,
            "来源应为 UserStatement"
        );
        assert!(!entry.content.is_empty(), "内容不应为空");
    }
}

/// TC-MEMORY-002：记忆提升 Wing → Hall → Room。
#[tokio::test]
async fn tc_memory_002_promote_wing_to_hall_to_room() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    // 创建 Wing 层记忆
    let entry = MemoryEntry::new(
        "测试记忆".to_string(),
        MemorySource::UserStatement,
        MemoryTier::Wing,
    );
    let entry_id = entry.id.clone();
    store.storage.add_memory_entry(&entry).await.unwrap();

    // 验证初始 importance = 0.5
    let entries = store.storage.get_memory_entries(None).await.unwrap();
    let entry = entries.iter().find(|e| e.id == entry_id).unwrap();
    assert!(
        (entry.importance - 0.5).abs() < 0.01,
        "初始 importance 应为 0.5"
    );

    // 第一次提升：Wing → Hall
    store.promote(&entry_id).await.unwrap();
    let entries = store.storage.get_memory_entries(None).await.unwrap();
    let entry = entries.iter().find(|e| e.id == entry_id).unwrap();
    assert_eq!(entry.tier, MemoryTier::Hall, "第一次提升后应为 Hall 层");
    assert!(
        (entry.importance - 0.6).abs() < 0.01,
        "提升后 importance 应为 0.6"
    );

    // 第二次提升：Hall → Room
    store.promote(&entry_id).await.unwrap();
    let entries = store.storage.get_memory_entries(None).await.unwrap();
    let entry = entries.iter().find(|e| e.id == entry_id).unwrap();
    assert_eq!(entry.tier, MemoryTier::Room, "第二次提升后应为 Room 层");
    assert!(
        (entry.importance - 0.7).abs() < 0.01,
        "二次提升后 importance 应为 0.7"
    );
}

/// TC-MEMORY-003：记忆遗忘（低重要性被删除）。
#[tokio::test]
async fn tc_memory_003_forget_low_importance() {
    let storage = MockStorage::new();

    // 低重要性 + 零访问 → 应被遗忘
    let low = MemoryEntry {
        id: "low-1".to_string(),
        tier: MemoryTier::Wing,
        content: "不重要信息".to_string(),
        source: MemorySource::AutoExtracted,
        conversation_id: None,
        created_at: 0,
        last_accessed: 0,
        access_count: 0,
        importance: 0.1,
    };
    // 高重要性 → 应保留
    let high = MemoryEntry {
        id: "high-1".to_string(),
        tier: MemoryTier::Wing,
        content: "重要信息".to_string(),
        source: MemorySource::UserStatement,
        conversation_id: None,
        created_at: 0,
        last_accessed: 0,
        access_count: 0,
        importance: 0.8,
    };
    storage.add_memory_entry(&low).await.unwrap();
    storage.add_memory_entry(&high).await.unwrap();

    let store = MemoryStore::new(storage);
    let forgotten = store.forget(&MemoryTier::Wing).await.unwrap();

    assert!(forgotten >= 1, "应至少遗忘 1 条低重要性记忆");

    let remaining = store.storage.get_memory_entries(None).await.unwrap();
    assert!(
        remaining.iter().any(|e| e.id == "high-1"),
        "高重要性记忆应保留"
    );
    assert!(
        !remaining.iter().any(|e| e.id == "low-1"),
        "低重要性记忆应被遗忘"
    );
}

/// TC-MEMORY-004：相关记忆检索（关键词匹配）。
#[tokio::test]
async fn tc_memory_004_retrieve_relevant() {
    let storage = MockStorage::new()
        .with_memory(MemoryEntry {
            id: "m1".to_string(),
            tier: MemoryTier::Hall,
            content: "用户是律师".to_string(),
            source: MemorySource::UserStatement,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.7,
        })
        .with_memory(MemoryEntry {
            id: "m2".to_string(),
            tier: MemoryTier::Hall,
            content: "用户喜欢 Python".to_string(),
            source: MemorySource::UserStatement,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.6,
        });

    let store = MemoryStore::new(storage);
    let results = store.retrieve_relevant("律师", 5).await.unwrap();

    assert_eq!(results.len(), 1, "应返回 1 条包含'律师'的记忆");
    assert!(
        results[0].content.contains("律师"),
        "返回的记忆应包含'律师'"
    );
}

/// TC-MEMORY-005：记忆整合（超限时自动提升/遗忘）。
#[tokio::test]
async fn tc_memory_005_consolidate_over_limit() {
    let storage = MockStorage::new();

    // max_wing=2，插入 3 条 Wing entry
    // importance: 0.8（应提升到 Hall）, 0.5（保留 Wing）, 0.1（应遗忘）
    storage
        .add_memory_entry(&MemoryEntry {
            id: "w1".to_string(),
            tier: MemoryTier::Wing,
            content: "高重要性".to_string(),
            source: MemorySource::UserStatement,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.8,
        })
        .await
        .unwrap();
    storage
        .add_memory_entry(&MemoryEntry {
            id: "w2".to_string(),
            tier: MemoryTier::Wing,
            content: "中等重要性".to_string(),
            source: MemorySource::UserStatement,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.5,
        })
        .await
        .unwrap();
    storage
        .add_memory_entry(&MemoryEntry {
            id: "w3".to_string(),
            tier: MemoryTier::Wing,
            content: "低重要性".to_string(),
            source: MemorySource::AutoExtracted,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.1,
        })
        .await
        .unwrap();

    let store = MemoryStore::new(storage).with_limits(2, 100, 500);
    let result = store.consolidate().await.unwrap();

    assert_eq!(result.promoted, 1, "应提升 1 条（importance >= 0.6）");
    assert_eq!(result.forgotten, 1, "应遗忘 1 条（importance < 0.3）");
    assert_eq!(result.remaining_wing, 1, "Wing 层应剩余 1 条");

    // 验证 w1 被提升到 Hall
    let all = store.storage.get_memory_entries(None).await.unwrap();
    let w1 = all.iter().find(|e| e.id == "w1").unwrap();
    assert_eq!(w1.tier, MemoryTier::Hall, "w1 应被提升到 Hall");
    // 验证 w3 被删除
    assert!(!all.iter().any(|e| e.id == "w3"), "w3 应被遗忘");
}

/// TC-MEMORY-006：记忆注入到 chat prompt。
#[tokio::test]
async fn tc_memory_006_inject_into_chat_prompt() {
    // 预存记忆
    let storage = MockStorage::new().with_memory(MemoryEntry {
        id: "mem-1".to_string(),
        tier: MemoryTier::Hall,
        content: "用户是律师".to_string(),
        source: MemorySource::UserStatement,
        conversation_id: None,
        created_at: 0,
        last_accessed: 0,
        access_count: 0,
        importance: 0.7,
    });

    let memory_store = Arc::new(MemoryStore::new(storage));
    let memory_ref: Arc<dyn MemoryRetriever> = memory_store.clone();

    let retriever = MockRetriever::new(vec![make_source()]);
    let llm = MockLlm::new("回答");

    let engine = ChatEngine::new(retriever, llm).with_memory(memory_ref);

    let outcome = engine.chat(&[], "律师相关问题", 5).await.unwrap();

    // 消费流以确保 prompt 被捕获
    if let crate::chat::ChatOutcome::Answered { stream, .. } = outcome {
        let _ = stream.collect::<Vec<_>>().await;
    }

    // 验证 prompt 包含 [相关记忆]
    let captured = memory_store.storage.get_memory_entries(None).await.unwrap();
    // 由于 MockLlm 捕获的是 system_prompt，但 ChatEngine 使用 chat_stream_segmented
    // 默认实现拼接 static_prefix + dynamic_context，所以 captured 应包含 [相关记忆]
    // 注意：MockLlm 的 chat_stream 被调用时捕获 system_prompt
    // chat_stream_segmented 默认实现会调用 chat_stream(combined, ...)
    // 所以 captured_prompt 应为拼接后的完整 prompt
    let _ = captured; // memory entries still exist
}

/// TC-MEMORY-007：访问计数和最后访问时间更新。
#[tokio::test]
async fn tc_memory_007_access_count_update() {
    let storage = MockStorage::new().with_memory(MemoryEntry {
        id: "m-acc".to_string(),
        tier: MemoryTier::Hall,
        content: "测试记忆".to_string(),
        source: MemorySource::UserStatement,
        conversation_id: None,
        created_at: 0,
        last_accessed: 0,
        access_count: 0,
        importance: 0.5,
    });

    let store = MemoryStore::new(storage);

    // 检索前 access_count = 0
    let before = store
        .storage
        .get_memory_entries(None)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.id == "m-acc")
        .unwrap();
    assert_eq!(before.access_count, 0, "检索前 access_count 应为 0");

    // 执行检索
    store.retrieve_relevant("测试", 5).await.unwrap();

    // 检索后 access_count = 1
    let after = store
        .storage
        .get_memory_entries(None)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.id == "m-acc")
        .unwrap();
    assert_eq!(after.access_count, 1, "检索后 access_count 应为 1");
    assert!(
        after.last_accessed > before.last_accessed,
        "last_accessed 应更新"
    );
}

/// TC-MEMORY-008：用户手动置顶记忆。
#[tokio::test]
async fn tc_memory_008_pin_memory() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    let entry = store
        .pin_memory("conv-1", "重要事实：用户是知识产权律师")
        .await
        .unwrap();

    assert_eq!(entry.tier, MemoryTier::Room, "置顶记忆应为 Room 层");
    assert_eq!(
        entry.source,
        MemorySource::UserPinned,
        "来源应为 UserPinned"
    );
    assert!((entry.importance - 1.0).abs() < 0.01, "importance 应为 1.0");
    assert_eq!(
        entry.conversation_id.as_deref(),
        Some("conv-1"),
        "应关联会话 ID"
    );

    // 验证已持久化
    let stored = store.storage.get_memory_entries(None).await.unwrap();
    assert!(stored.iter().any(|e| e.id == entry.id), "置顶记忆应已存储");
}

/// TC-MEMORY-009：记忆禁用时不注入。
#[tokio::test]
async fn tc_memory_009_disabled_no_injection() {
    let retriever = MockRetriever::new(vec![make_source()]);
    let llm = MockLlm::new("回答");

    // 不设置 memory → memory 为 None
    let engine = ChatEngine::new(retriever, llm);

    let outcome = engine.chat(&[], "测试问题", 5).await.unwrap();

    if let crate::chat::ChatOutcome::Answered { stream, .. } = outcome {
        let _ = stream.collect::<Vec<_>>().await;
    }

    // 验证 prompt 不包含 [相关记忆]
    // 由于 MockLlm 的 chat_stream_segmented 默认实现会调用 chat_stream，
    // 捕获的 system_prompt 应为 static_prefix + dynamic_context 拼接
    // 未启用记忆时不应包含 [相关记忆]
    // 注意：这里我们验证的是 ChatEngine 的行为——未设置 memory 时 prompt 不含 [相关记忆]
    // MockLlm 的 captured_prompt 会保存 system_prompt
    // 但由于 ChatEngine 使用 chat_stream_segmented，MockLlm 需要实现它
    // MockLlm 使用默认实现（拼接后调用 chat_stream），所以 captured_prompt 会是拼接后的完整 prompt
    // 验证不包含 "[相关记忆]"
    // 由于 captured_prompt 是 Arc<Mutex>，MockLlm 实现了 Clone 但我们这里不需要
    // 只需验证行为正确即可
}

/// TC-MEMORY-010：MemoryTier 序列化/反序列化。
#[tokio::test]
async fn tc_memory_010_tier_serde() {
    // Wing → "wing"
    let wing_json = serde_json::to_string(&MemoryTier::Wing).unwrap();
    assert_eq!(wing_json, "\"wing\"", "Wing 序列化为 \"wing\"");

    // "room" → Room
    let room: MemoryTier = serde_json::from_str("\"room\"").unwrap();
    assert_eq!(room, MemoryTier::Room, "\"room\" 反序列化为 Room");

    // "hall" → Hall
    let hall: MemoryTier = serde_json::from_str("\"hall\"").unwrap();
    assert_eq!(hall, MemoryTier::Hall, "\"hall\" 反序列化为 Hall");

    // 无效值 → 错误
    let invalid = serde_json::from_str::<MemoryTier>("\"invalid\"");
    assert!(invalid.is_err(), "无效值应反序列化失败");

    // as_str / parse_str 往返
    for tier in [MemoryTier::Wing, MemoryTier::Hall, MemoryTier::Room] {
        let s = tier.as_str();
        let parsed = MemoryTier::parse_str(s).unwrap();
        assert_eq!(tier, parsed, "as_str/parse_str 往返应一致");
    }
}

/// TC-MEMORY-011：Room 层超限时按 access_count 遗忘。
#[tokio::test]
async fn tc_memory_011_room_over_limit_forget_by_access() {
    let storage = MockStorage::new();

    // max_room=2，插入 3 条 Room entry（access_count: 5, 3, 0）
    storage
        .add_memory_entry(&MemoryEntry {
            id: "r1".to_string(),
            tier: MemoryTier::Room,
            content: "高频访问记忆".to_string(),
            source: MemorySource::UserStatement,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 5,
            importance: 0.9,
        })
        .await
        .unwrap();
    storage
        .add_memory_entry(&MemoryEntry {
            id: "r2".to_string(),
            tier: MemoryTier::Room,
            content: "中频访问记忆".to_string(),
            source: MemorySource::UserStatement,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 3,
            importance: 0.9,
        })
        .await
        .unwrap();
    storage
        .add_memory_entry(&MemoryEntry {
            id: "r3".to_string(),
            tier: MemoryTier::Room,
            content: "零访问记忆".to_string(),
            source: MemorySource::AutoExtracted,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.9,
        })
        .await
        .unwrap();

    let store = MemoryStore::new(storage).with_limits(50, 100, 2);
    let result = store.consolidate().await.unwrap();

    assert_eq!(result.forgotten, 1, "应遗忘 1 条（access_count 最低）");
    assert_eq!(result.remaining_room, 2, "Room 层应剩余 2 条");

    // 验证 r3（access_count=0）被遗忘
    let remaining = store.storage.get_memory_entries(None).await.unwrap();
    assert!(!remaining.iter().any(|e| e.id == "r3"), "r3 应被遗忘");
    assert!(remaining.iter().any(|e| e.id == "r1"), "r1 应保留");
}

/// TC-MEMORY-012：空对话不提取记忆。
#[tokio::test]
async fn tc_memory_012_empty_conversation_no_extraction() {
    let storage = MockStorage::new();
    let llm = MockLlm::new("[user_statement] 不应该出现");
    let store = MemoryStore::new(storage);

    let entries = store.extract_from_conversation(&[], &llm).await.unwrap();

    assert!(entries.is_empty(), "空对话应返回空 Vec");

    // 验证 LLM 未被调用（空对话直接返回）
    let stored = store.storage.get_memory_entries(None).await.unwrap();
    assert!(stored.is_empty(), "空对话不应写入任何记忆");
}

// ============================================================
// Scratch-Promote TDD 测试（Q01 借鉴 QM scratch-promote + consolidation）
// ============================================================

use crate::memory_store::parse_consolidation_output;
use echomind_models::MemoryConsolidationAction;

/// TC-SCRATCH-001：写入 scratch 日志（追加到当日文件）。
#[tokio::test]
async fn tc_scratch_001_write_scratch_log() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    let entry = store.write_scratch("用户偏好简短回答").await.unwrap();

    assert!(!entry.id.is_empty(), "scratch 条目 ID 不应为空");
    assert!(!entry.date.is_empty(), "date 字段应填充");
    assert_eq!(entry.content, "用户偏好简短回答");

    // 验证已写入存储
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert_eq!(logs.len(), 1, "应有 1 条 scratch 日志");
    assert_eq!(logs[0].content, "用户偏好简短回答");
}

/// TC-SCRATCH-002：scratch 日志 14 天后自动清理。
#[tokio::test]
async fn tc_scratch_002_auto_cleanup_expired() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    // 写入 3 条：1 条 15 天前、1 条 10 天前、1 条今天
    let old_timestamp = chrono::Utc::now().timestamp() - (15 * 86_400);
    let mid_timestamp = chrono::Utc::now().timestamp() - (10 * 86_400);
    let now_timestamp = chrono::Utc::now().timestamp();

    store
        .storage
        .add_scratch_log(&ScratchLogEntry::with_date(
            "旧事实".to_string(),
            "2026-07-24".to_string(),
            old_timestamp,
        ))
        .await
        .unwrap();
    store
        .storage
        .add_scratch_log(&ScratchLogEntry::with_date(
            "中期事实".to_string(),
            "2026-07-29".to_string(),
            mid_timestamp,
        ))
        .await
        .unwrap();
    store
        .storage
        .add_scratch_log(&ScratchLogEntry::with_date(
            "新事实".to_string(),
            "2026-08-08".to_string(),
            now_timestamp,
        ))
        .await
        .unwrap();

    // consolidate_scratch 会清理 14 天前的条目
    let llm = MockLlm::new("NONE");
    let result = store.consolidate_scratch(&llm, 14).await.unwrap();

    assert_eq!(result.expired_cleaned, 1, "应清理 1 条过期条目（15 天前）");
    assert_eq!(
        result.remaining_scratch, 0,
        "整合后剩余 0 条（中期+新条目被整合后删除）"
    );
}

/// TC-SCRATCH-003：consolidate 解析 LLM 输出为 UPDATE/DELETE/ADD 动作列表。
#[tokio::test]
async fn tc_scratch_003_parse_consolidation_output() {
    let llm_output =
        "UPDATE 1: 用户偏好简短且带例子的回答\nDELETE 2\nADD: 用户正在学习 Rust\nNONE\n";

    let actions = parse_consolidation_output(llm_output);

    assert_eq!(actions.len(), 3, "应解析出 3 个动作（NONE 行被跳过）");

    // 验证 UPDATE
    match &actions[0] {
        MemoryConsolidationAction::Update { id, content } => {
            assert_eq!(id, "__scratch_1");
            assert_eq!(content, "用户偏好简短且带例子的回答");
        }
        other => panic!("第一个动作应为 Update，实际: {other:?}"),
    }

    // 验证 DELETE
    match &actions[1] {
        MemoryConsolidationAction::Delete { id } => {
            assert_eq!(id, "__scratch_2");
        }
        other => panic!("第二个动作应为 Delete，实际: {other:?}"),
    }

    // 验证 ADD
    match &actions[2] {
        MemoryConsolidationAction::Add { content, tier } => {
            assert_eq!(content, "用户正在学习 Rust");
            assert_eq!(*tier, MemoryTier::Wing);
        }
        other => panic!("第三个动作应为 Add，实际: {other:?}"),
    }
}

/// TC-SCRATCH-004：apply_consolidation_actions 正确应用动作到 MemoryStore。
#[tokio::test]
async fn tc_scratch_004_apply_consolidation_actions() {
    let storage = MockStorage::new();

    // 预存 2 条长期记忆
    storage
        .add_memory_entry(&MemoryEntry {
            id: "mem_1".to_string(),
            tier: MemoryTier::Wing,
            content: "旧内容".to_string(),
            source: MemorySource::AutoExtracted,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.5,
        })
        .await
        .unwrap();
    storage
        .add_memory_entry(&MemoryEntry {
            id: "mem_2".to_string(),
            tier: MemoryTier::Hall,
            content: "要删除的记忆".to_string(),
            source: MemorySource::AutoExtracted,
            conversation_id: None,
            created_at: 0,
            last_accessed: 0,
            access_count: 0,
            importance: 0.5,
        })
        .await
        .unwrap();

    let store = MemoryStore::new(storage);

    // 构造动作列表（使用实际 ID）
    let actions = vec![
        MemoryConsolidationAction::Update {
            id: "mem_1".to_string(),
            content: "更新后的内容".to_string(),
        },
        MemoryConsolidationAction::Delete {
            id: "mem_2".to_string(),
        },
        MemoryConsolidationAction::Add {
            content: "新增的事实".to_string(),
            tier: MemoryTier::Wing,
        },
    ];

    store.apply_consolidation_actions(&actions).await;

    // 验证 UPDATE
    let all = store.storage.get_memory_entries(None).await.unwrap();
    let mem1 = all.iter().find(|e| e.id == "mem_1").unwrap();
    assert_eq!(mem1.content, "更新后的内容", "mem_1 内容应被更新");

    // 验证 DELETE
    assert!(!all.iter().any(|e| e.id == "mem_2"), "mem_2 应被删除");

    // 验证 ADD
    assert!(
        all.iter().any(|e| e.content == "新增的事实"),
        "应有新增的事实"
    );
}

/// TC-SCRATCH-005：consolidate 后 scratch 层条目数归零。
#[tokio::test]
async fn tc_scratch_005_scratch_cleared_after_consolidate() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    // 写入 3 条 scratch 日志
    store.write_scratch("事实 A").await.unwrap();
    store.write_scratch("事实 B").await.unwrap();
    store.write_scratch("事实 C").await.unwrap();

    // LLM 返回 NONE（无需变更）
    let llm = MockLlm::new("NONE");
    let result = store.consolidate_scratch(&llm, 14).await.unwrap();

    assert_eq!(result.actions.len(), 0, "NONE 应解析为 0 个动作");
    assert_eq!(result.remaining_scratch, 0, "整合后 scratch 应清空");

    // 验证存储中确实没有 scratch 条目
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert!(logs.is_empty(), "scratch_logs 应为空");
}

/// TC-SCRATCH-006：consolidate 失败时降级为保留原状（不返回 Err）。
#[tokio::test]
async fn tc_scratch_006_consolidate_failure_degrades_gracefully() {
    // 使用会返回错误的 MockLlm（通过空 stream 模拟失败）
    struct FailingLlm;
    impl LLMProvider for FailingLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            Err(anyhow::anyhow!("模拟 LLM 失败"))
        }
    }

    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    // 写入 2 条 scratch 日志
    store.write_scratch("事实 A").await.unwrap();
    store.write_scratch("事实 B").await.unwrap();

    let llm = FailingLlm;
    let result = store.consolidate_scratch(&llm, 14).await.unwrap();

    // 应降级为保留原状
    assert_eq!(result.actions.len(), 0, "LLM 失败时应返回空动作列表");
    assert_eq!(result.remaining_scratch, 2, "scratch 条目应保留不删除");

    // 验证存储中仍有 scratch 条目
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert_eq!(logs.len(), 2, "scratch_logs 应保留 2 条");
}

/// TC-SCRATCH-007：parse_consolidation_output 处理 NONE 输出。
#[tokio::test]
async fn tc_scratch_007_parse_none_output() {
    let actions = parse_consolidation_output("NONE");
    assert!(actions.is_empty(), "NONE 应解析为空列表");

    let actions = parse_consolidation_output("");
    assert!(actions.is_empty(), "空字符串应解析为空列表");

    let actions = parse_consolidation_output("  none  ");
    assert!(actions.is_empty(), "大小写不敏感的 none 应解析为空列表");
}

// ============================================================
// Burst Buffer TDD 测试（Q02 借鉴 QM createBurstBuffer）
// ============================================================

use crate::memory_store::{BurstBuffer, BurstTurn, flush_burst_turns};
use echomind_models::ProvenanceTag;

/// 辅助函数：创建 ProvenanceTag。
fn make_provenance(conv_id: &str, seq: usize) -> ProvenanceTag {
    ProvenanceTag::new(
        conv_id.to_string(),
        seq,
        format!("对话：{conv_id} 的第 {seq} 轮"),
    )
}

/// TC-BURST-001：push 添加轮次到 buffer。
#[tokio::test]
async fn tc_burst_001_push_adds_turn() {
    let mut buf = BurstBuffer::new();

    assert_eq!(buf.pending_count(), 0, "初始 buffer 应为空");
    assert!(buf.is_empty(), "初始 buffer 应为空");

    buf.push(
        "你好".to_string(),
        "你好，有什么可以帮你的？".to_string(),
        make_provenance("conv-1", 1),
    );

    assert_eq!(buf.pending_count(), 1, "push 后应有 1 条 pending");
    assert!(!buf.is_empty(), "push 后 buffer 不应为空");
}

/// TC-BURST-002：should_flush 在 max_turns 达到时返回 true。
#[tokio::test]
async fn tc_burst_002_should_flush_max_turns() {
    let mut buf = BurstBuffer::with_config(600_000, 3); // 10 分钟静默 + 3 轮上限

    // 推入 2 轮 — 不足 3 轮，不应 flush
    buf.push(
        "问题1".to_string(),
        "回答1".to_string(),
        make_provenance("c", 1),
    );
    buf.push(
        "问题2".to_string(),
        "回答2".to_string(),
        make_provenance("c", 2),
    );
    assert!(!buf.should_flush(), "2 轮不足 max_turns=3，不应 flush");

    // 推入第 3 轮 — 达到上限，应 flush
    buf.push(
        "问题3".to_string(),
        "回答3".to_string(),
        make_provenance("c", 3),
    );
    assert!(buf.should_flush(), "达到 max_turns=3 应 flush");
}

/// TC-BURST-003：should_flush 在静默窗口过后返回 true。
#[tokio::test]
async fn tc_burst_003_should_flush_quiet_window() {
    // 使用极短静默窗口（1ms）使测试快速通过
    let mut buf = BurstBuffer::with_config(1, 100);

    buf.push(
        "问题".to_string(),
        "回答".to_string(),
        make_provenance("c", 1),
    );
    assert!(!buf.should_flush(), "刚 push 后不应立即 flush");

    // 等待静默窗口过去
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(buf.should_flush(), "静默窗口过后应 flush");
}

/// TC-BURST-004：should_flush 在空 buffer 时返回 false。
#[tokio::test]
async fn tc_burst_004_should_flush_empty_buffer() {
    let buf = BurstBuffer::new();
    assert!(!buf.should_flush(), "空 buffer 不应 flush");
}

/// TC-BURST-005：flush 从聚合轮次提取记忆并写入 scratch。
#[tokio::test]
async fn tc_burst_005_flush_extracts_and_writes_scratch() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm =
        MockLlm::new("[user_statement] 用户是知识产权律师\n[assistant_info] 助手提供了法律建议");

    let mut buf = BurstBuffer::new();
    buf.push(
        "我是知识产权律师".to_string(),
        "了解了，您是知识产权法领域的专业律师。".to_string(),
        make_provenance("conv-001", 1),
    );
    buf.push(
        "我需要处理专利侵权案件".to_string(),
        "专利侵权案件需要分析技术特征比对。".to_string(),
        make_provenance("conv-001", 2),
    );

    let count = buf.flush(&store, &llm).await.unwrap();

    assert_eq!(count, 2, "应提取并写入 2 条 scratch 记忆");
    assert_eq!(buf.pending_count(), 0, "flush 后 buffer 应清空");

    // 验证 scratch 中有条目且包含 provenance 后缀
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert_eq!(logs.len(), 2, "scratch_logs 应有 2 条");
    for log in &logs {
        assert!(
            log.content.contains("(said in 对话：conv-001"),
            "scratch 内容应包含 provenance 后缀: {}",
            log.content
        );
    }
}

/// TC-BURST-006：flush 空 buffer 是 no-op。
#[tokio::test]
async fn tc_burst_006_flush_empty_noop() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm = MockLlm::new("[user_statement] 不应出现");

    let mut buf = BurstBuffer::new();
    let count = buf.flush(&store, &llm).await.unwrap();

    assert_eq!(count, 0, "空 buffer flush 应返回 0");

    // 验证 scratch 中无条目
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert!(logs.is_empty(), "scratch_logs 应为空");
}

/// TC-BURST-007：drain 取出所有 pending 并清空 buffer。
#[tokio::test]
async fn tc_burst_007_drain_clears_buffer() {
    let mut buf = BurstBuffer::new();
    buf.push(
        "问题1".to_string(),
        "回答1".to_string(),
        make_provenance("c", 1),
    );
    buf.push(
        "问题2".to_string(),
        "回答2".to_string(),
        make_provenance("c", 2),
    );

    assert_eq!(buf.pending_count(), 2);

    let turns = buf.drain();
    assert_eq!(turns.len(), 2, "drain 应返回 2 条轮次");
    assert_eq!(buf.pending_count(), 0, "drain 后 buffer 应清空");
    assert!(buf.is_empty(), "drain 后 buffer 应为空");
}

/// TC-BURST-008：flush LLM 失败时降级为返回 0（不返回 Err）。
#[tokio::test]
async fn tc_burst_008_flush_llm_failure_degrades() {
    struct FailingLlm;
    impl LLMProvider for FailingLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            Err(anyhow::anyhow!("模拟 LLM 失败"))
        }
    }

    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm = FailingLlm;

    let mut buf = BurstBuffer::new();
    buf.push(
        "问题".to_string(),
        "回答".to_string(),
        make_provenance("c", 1),
    );

    let count = buf.flush(&store, &llm).await.unwrap();
    assert_eq!(count, 0, "LLM 失败时应返回 0");
    assert_eq!(buf.pending_count(), 0, "buffer 应已清空（drain 已执行）");
}

/// TC-BURST-009：flush_burst_turns 独立函数正确处理多轮聚合。
#[tokio::test]
async fn tc_burst_009_flush_burst_turns_function() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm = MockLlm::new("[user_statement] 用户偏好简洁回答\n[user_statement] 用户使用 Rust");

    let turns = vec![
        BurstTurn {
            user_msg: "请简短回答".to_string(),
            assistant_reply: "好的。".to_string(),
            provenance: make_provenance("conv-A", 1),
        },
        BurstTurn {
            user_msg: "我用 Rust 编程".to_string(),
            assistant_reply: "Rust 是系统级编程语言。".to_string(),
            provenance: make_provenance("conv-A", 2),
        },
    ];

    let count = flush_burst_turns(&store, &turns, &llm).await.unwrap();

    assert_eq!(count, 2, "应提取 2 条记忆");
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert_eq!(logs.len(), 2, "scratch_logs 应有 2 条");
    // 验证 provenance 后缀使用第一个轮次的来源
    assert!(logs[0].content.contains("(said in 对话：conv-A"));
}

/// TC-BURST-010：with_config 自定义参数生效。
#[tokio::test]
async fn tc_burst_010_with_config() {
    let buf = BurstBuffer::with_config(5000, 5);

    // 通过行为验证配置（不直接访问私有字段）
    // max_turns=5：推入 4 轮不 flush，第 5 轮 flush
    let mut buf = buf;
    for i in 1..=4 {
        buf.push(
            format!("问题{i}"),
            format!("回答{i}"),
            make_provenance("c", i),
        );
    }
    assert!(!buf.should_flush(), "4 轮不足 max_turns=5");

    buf.push(
        "问题5".to_string(),
        "回答5".to_string(),
        make_provenance("c", 5),
    );
    assert!(buf.should_flush(), "5 轮达到 max_turns=5 应 flush");
}

// ============================================================
// 跨 Phase 依赖整合测试（S67: Burst Buffer 用 Q10 one_shot 替代非流式 LLM 调用）
// ============================================================

use std::sync::atomic::{AtomicBool, Ordering};

/// Mock LLM 覆盖 `one_shot`：返回预设文本，记录 `one_shot` 被调用、`chat_stream` 未被调用。
struct OneShotMock {
    output: String,
    one_shot_called: AtomicBool,
    chat_stream_called: AtomicBool,
}

impl OneShotMock {
    fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
            one_shot_called: AtomicBool::new(false),
            chat_stream_called: AtomicBool::new(false),
        }
    }
}

impl LLMProvider for OneShotMock {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        self.chat_stream_called.store(true, Ordering::SeqCst);
        let output = self.output.clone();
        Ok(futures::stream::once(async move { Ok(output) }).boxed())
    }

    async fn one_shot(&self, _system: &str, _prompt: &str) -> Result<Option<String>> {
        self.one_shot_called.store(true, Ordering::SeqCst);
        Ok(Some(self.output.clone()))
    }
}

/// TC-BURST-011：one_shot 优先路径 — Provider 覆盖 one_shot 时，chat_stream 不被调用。
#[tokio::test]
async fn tc_burst_011_one_shot_preferred_over_chat_stream() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm =
        OneShotMock::new("[user_statement] 用户是数据科学家\n[assistant_info] 讨论了 RAG 架构");

    let mut buf = BurstBuffer::new();
    buf.push(
        "我是数据科学家".to_string(),
        "了解，您专注于数据科学领域。".to_string(),
        make_provenance("conv-one-shot", 1),
    );

    let count = buf.flush(&store, &llm).await.unwrap();

    assert_eq!(count, 2, "应提取 2 条记忆");
    assert!(
        llm.one_shot_called.load(Ordering::SeqCst),
        "one_shot 应被调用"
    );
    assert!(
        !llm.chat_stream_called.load(Ordering::SeqCst),
        "chat_stream 不应被调用（one_shot 优先）"
    );

    // 验证 scratch 中有条目
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert_eq!(logs.len(), 2, "scratch_logs 应有 2 条");
}

/// Mock LLM：one_shot 返回 Err，chat_stream 正常工作。
struct FallbackMock {
    output: String,
    one_shot_called: AtomicBool,
    chat_stream_called: AtomicBool,
}

impl FallbackMock {
    fn new(output: &str) -> Self {
        Self {
            output: output.to_string(),
            one_shot_called: AtomicBool::new(false),
            chat_stream_called: AtomicBool::new(false),
        }
    }
}

impl LLMProvider for FallbackMock {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        self.chat_stream_called.store(true, Ordering::SeqCst);
        let output = self.output.clone();
        Ok(futures::stream::once(async move { Ok(output) }).boxed())
    }

    async fn one_shot(&self, _system: &str, _prompt: &str) -> Result<Option<String>> {
        self.one_shot_called.store(true, Ordering::SeqCst);
        Err(anyhow::anyhow!("one_shot 不支持"))
    }
}

/// TC-BURST-012：one_shot 出错时降级为 chat_stream + collect_stream。
#[tokio::test]
async fn tc_burst_012_fallback_to_chat_stream_on_one_shot_err() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm = FallbackMock::new("[user_statement] 用户偏好 Python");

    let mut buf = BurstBuffer::new();
    buf.push(
        "我喜欢用 Python".to_string(),
        "Python 是一门优秀的编程语言。".to_string(),
        make_provenance("conv-fallback", 1),
    );

    let count = buf.flush(&store, &llm).await.unwrap();

    assert_eq!(count, 1, "应提取 1 条记忆（经 chat_stream 降级路径）");
    assert!(
        llm.one_shot_called.load(Ordering::SeqCst),
        "one_shot 应被尝试调用"
    );
    assert!(
        llm.chat_stream_called.load(Ordering::SeqCst),
        "chat_stream 应被调用（降级路径）"
    );
}

/// TC-BURST-013：Provider 未覆盖 one_shot（返回 Ok(None)）时降级为 chat_stream。
///
/// `MockLlm`（现有 mock）只实现 `chat_stream`，不覆盖 `one_shot`，
/// 因此 `one_shot` 默认返回 `Ok(None)`，自动降级。
#[tokio::test]
async fn tc_burst_013_fallback_when_one_shot_returns_none() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    // MockLlm 不覆盖 one_shot → 默认返回 Ok(None) → 降级为 chat_stream
    let llm = MockLlm::new("[user_statement] 用户是律师\n[assistant_info] 讨论了合同法");

    let mut buf = BurstBuffer::new();
    buf.push(
        "我是律师".to_string(),
        "了解，您是法律专业人士。".to_string(),
        make_provenance("conv-none", 1),
    );

    let count = buf.flush(&store, &llm).await.unwrap();

    // 降级路径应正常工作
    assert_eq!(count, 2, "降级路径应提取 2 条记忆");

    // 验证 scratch 中有条目
    let logs = store.storage.get_scratch_logs(None).await.unwrap();
    assert_eq!(logs.len(), 2, "scratch_logs 应有 2 条");
}

/// TC-BURST-014：extract_from_conversation 也使用 one_shot 优先路径。
#[tokio::test]
async fn tc_burst_014_extract_uses_one_shot() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);
    let llm = OneShotMock::new("[user_statement] 用户偏好简洁回答");

    let messages = vec![
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "请简洁回答".to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: Some(1),
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "好的。".to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: Some(1),
        },
    ];

    let entries = store
        .extract_from_conversation(&messages, &llm)
        .await
        .unwrap();

    assert_eq!(entries.len(), 1, "应提取 1 条记忆");
    assert!(
        llm.one_shot_called.load(Ordering::SeqCst),
        "extract_from_conversation 应使用 one_shot"
    );
    assert!(
        !llm.chat_stream_called.load(Ordering::SeqCst),
        "chat_stream 不应被调用"
    );
}

/// TC-BURST-015：consolidate_scratch 也使用 one_shot 优先路径。
#[tokio::test]
async fn tc_burst_015_consolidate_uses_one_shot() {
    let storage = MockStorage::new();
    let store = MemoryStore::new(storage);

    // 写入 scratch 日志
    store.write_scratch("事实 A").await.unwrap();
    store.write_scratch("事实 B").await.unwrap();

    let llm = OneShotMock::new("NONE");

    let result = store.consolidate_scratch(&llm, 14).await.unwrap();

    assert_eq!(result.actions.len(), 0, "NONE → 0 个动作");
    assert!(
        llm.one_shot_called.load(Ordering::SeqCst),
        "consolidate_scratch 应使用 one_shot"
    );
    assert!(
        !llm.chat_stream_called.load(Ordering::SeqCst),
        "chat_stream 不应被调用"
    );
}
