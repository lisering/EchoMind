#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! TDD 测试：TC-DREAM-001~010 后台空闲整理引擎。
//!
//! 测试策略：
//! - Mock Storage：预置文档 + chunk 数据
//! - Mock Embedder：返回可控向量（高相似/低相似）
//! - Mock LLM：返回预置 JSON（矛盾/一致判定）
//! - 取消信号：Arc<AtomicBool>

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, DocStatus, Document, RetrievalResult};
use futures::stream::BoxStream;

use crate::auto_dream::{DreamCancelFlag, DreamEngine, DreamResult, DreamSeverity, SuggestionType};
use crate::{Embedder, LLMProvider, Storage, idempotency::IdempotencyStore};

// ================== Mock 实现 ==================

/// 预置文档 + chunk 的 Mock Storage。
#[derive(Clone)]
struct MockStorage {
    documents: Vec<Document>,
    chunks: HashMap<String, Vec<Chunk>>,
}

use std::collections::HashMap;

impl MockStorage {
    fn new() -> Self {
        Self {
            documents: vec![],
            chunks: HashMap::new(),
        }
    }

    fn with_doc(mut self, doc: Document, chunks: Vec<Chunk>) -> Self {
        let doc_id = doc.id.clone();
        self.chunks.insert(doc_id, chunks);
        self.documents.push(doc);
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
    async fn find_document_by_hash(&self, hash: &str) -> Result<Option<Document>> {
        Ok(self.documents.iter().find(|d| d.file_hash == hash).cloned())
    }
    async fn count_documents(&self) -> Result<usize> {
        Ok(self.documents.len())
    }
    async fn count_chunks(&self) -> Result<usize> {
        Ok(self.chunks.values().map(|v| v.len()).sum())
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
    async fn list_chunks(&self, doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self.chunks.get(doc_id).cloned().unwrap_or_default())
    }
    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// 可控向量 Mock Embedder。
///
/// 根据 `embed()` 的输入文本返回预设向量。
/// 同主题文本返回相似向量，异主题返回正交向量。
struct MockEmbedder {
    /// 调用计数
    call_count: Arc<AtomicUsize>,
}

impl MockEmbedder {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// 生成与文本内容关联的确定性向量。
    /// 相同文本 → 相同向量；相同字符 → 相同维度激活 → 高余弦相似度。
    /// 不同字符集合 → 不同维度激活 → 低余弦相似度（近正交）。
    fn text_to_vector(text: &str) -> Vec<f32> {
        // 基于字符哈希的词袋模型：每个字符激活一个维度
        let mut vec = vec![0.0f32; 384];
        for c in text.chars() {
            let idx = (c as usize) % 384;
            vec[idx] += 1.0;
        }
        // 归一化
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut vec {
                *x /= norm;
            }
        }
        vec
    }
}

impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(Self::text_to_vector(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(texts.iter().map(|t| Self::text_to_vector(t)).collect())
    }
}

/// 固定响应 Mock LLM。
struct MockLlm {
    /// 固定返回的文本
    response: String,
}

impl MockLlm {
    fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
        }
    }
}

impl LLMProvider for MockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        let response = self.response.clone();
        Ok(Box::pin(futures::stream::once(async move { Ok(response) })))
    }
}

/// 返回矛盾判定 JSON 的 Mock LLM。
fn contradiction_llm() -> MockLlm {
    MockLlm::new(
        r#"{"is_contradiction": true, "topic": "温度参数", "explanation": "文档A称温度为25°C，文档B称温度为20°C，存在矛盾。", "severity": "high"}"#,
    )
}

/// 返回无矛盾判定 JSON 的 Mock LLM。
fn no_contradiction_llm() -> MockLlm {
    MockLlm::new(
        r#"{"is_contradiction": false, "topic": "实验方法", "explanation": "两段文本描述不同实验，无直接矛盾。", "severity": "low"}"#,
    )
}

// ================== 辅助构造函数 ==================

fn make_doc(name: &str, hash: &str, chunks: &[&str]) -> (Document, Vec<Chunk>) {
    let doc = Document {
        id: format!("doc-{name}"),
        file_path: name.to_string(),
        file_hash: hash.to_string(),
        status: DocStatus::Indexed,
        created_at: 1000,
        original_path: None,
        domain: None,
        summary: None,
        tags: Vec::new(),
    };
    let chunk_list: Vec<Chunk> = chunks
        .iter()
        .enumerate()
        .map(|(i, text)| Chunk {
            id: format!("chunk-{name}-{i}"),
            doc_id: doc.id.clone(),
            content: text.to_string(),
            token_count: 50,
            sequence: i,
        })
        .collect();
    (doc, chunk_list)
}

fn no_cancel() -> DreamCancelFlag {
    Arc::new(AtomicBool::new(false))
}

// ================== 测试用例 ==================

/// TC-DREAM-001：空知识库返回空结果。
#[tokio::test]
async fn tc_dream_001_empty_kb_returns_empty() {
    let storage = MockStorage::new();
    let embedder = MockEmbedder::new();
    let llm = no_contradiction_llm();
    let engine = DreamEngine::new(embedder, storage, llm, IdempotencyStore::new());

    let result = engine.dream(no_cancel()).await.unwrap();

    assert_eq!(result.total_documents, 0);
    assert_eq!(result.total_suggestions, 0);
    assert!(result.suggestions.is_empty());
}

/// TC-DREAM-002：精确重复文档检测（相同 hash）。
#[tokio::test]
async fn tc_dream_002_exact_duplicate_detection() {
    let (doc_a, chunks_a) = make_doc("a.md", "same-hash", &["内容 A"]);
    let (doc_b, chunks_b) = make_doc("b.md", "same-hash", &["内容 B"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        no_contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let dups: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::DuplicateDocuments)
        .collect();
    assert!(
        dups.iter().any(|s| s.similarity == Some(1.0)),
        "应检测到精确重复（similarity=1.0）"
    );
}

/// TC-DREAM-003：近似重复文档检测（高相似度嵌入）。
#[tokio::test]
async fn tc_dream_003_near_duplicate_detection() {
    // 两个文档 hash 不同但内容高度相似（相同文本前缀）
    let (doc_a, chunks_a) = make_doc("a.md", "hash-a", &["相同的实验内容描述"]);
    let (doc_b, chunks_b) = make_doc("b.md", "hash-b", &["相同的实验内容描述"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        no_contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let dups: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| {
            s.suggestion_type == SuggestionType::DuplicateDocuments && s.similarity.is_some()
        })
        .collect();
    assert!(!dups.is_empty(), "应检测到近似重复文档");
    // 精确重复不会出现（hash 不同），只有近似重复
    assert!(
        !dups.iter().any(|s| s.similarity == Some(1.0)),
        "hash 不同时不应有精确重复"
    );
}

/// TC-DREAM-004：不同内容文档不应被标记为重复。
#[tokio::test]
async fn tc_dream_004_distinct_docs_not_flagged() {
    let (doc_a, chunks_a) = make_doc("a.md", "hash-a", &["苹果香蕉橙子葡萄西瓜"]);
    let (doc_b, chunks_b) = make_doc("b.md", "hash-b", &["电脑手机平板键盘鼠标"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        no_contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let dups: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::DuplicateDocuments)
        .collect();
    assert!(dups.is_empty(), "内容完全不同的文档不应被标记为重复");
}

/// TC-DREAM-005：跨文档矛盾检测（LLM 判定矛盾）。
#[tokio::test]
async fn tc_dream_005_cross_doc_contradiction_detected() {
    // 使用相同的文本使嵌入高相似度（通过预筛阈值）
    let (doc_a, chunks_a) = make_doc("a.md", "hash-a", &["温度参数实验结果"]);
    let (doc_b, chunks_b) = make_doc("b.md", "hash-b", &["温度参数实验结果"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let contras: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::Contradiction)
        .collect();
    assert!(!contras.is_empty(), "应检测到跨文档矛盾");
    assert_eq!(
        contras[0].severity,
        DreamSeverity::High,
        "LLM 返回 high 严重等级"
    );
    assert!(contras[0].title.contains("温度参数"), "矛盾标题应包含主题");
}

/// TC-DREAM-006：无矛盾时不应产生矛盾建议。
#[tokio::test]
async fn tc_dream_006_no_contradiction_when_consistent() {
    let (doc_a, chunks_a) = make_doc("a.md", "hash-a", &["温度参数实验结果"]);
    let (doc_b, chunks_b) = make_doc("b.md", "hash-b", &["温度参数实验结果"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        no_contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let contras: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::Contradiction)
        .collect();
    assert!(contras.is_empty(), "LLM 判定无矛盾时不应产生矛盾建议");
}

/// TC-DREAM-007：未分类文档生成整理建议。
#[tokio::test]
async fn tc_dream_007_unclassified_docs_suggestion() {
    let (doc_a, chunks_a) = make_doc("a.md", "hash-a", &["内容 A"]);
    let (doc_b, chunks_b) = make_doc("b.md", "hash-b", &["内容 B"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        no_contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let orgs: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::Organization)
        .collect();
    assert!(
        orgs.iter().any(|s| s.title.contains("尚未分类")),
        "应生成未分类文档建议"
    );
}

/// TC-DREAM-008：取消信号中断分析，返回已完成的部分结果。
#[tokio::test]
async fn tc_dream_008_cancellation_returns_partial() {
    let (doc_a, chunks_a) = make_doc("a.md", "hash-a", &["内容 A"]);
    let (doc_b, chunks_b) = make_doc("b.md", "hash-b", &["内容 B"]);
    let storage = MockStorage::new()
        .with_doc(doc_a, chunks_a)
        .with_doc(doc_b, chunks_b);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        no_contradiction_llm(),
        IdempotencyStore::new(),
    );

    let cancel = Arc::new(AtomicBool::new(true)); // 立即取消
    let result = engine.dream(cancel).await.unwrap();

    // 取消后不应执行任何分析
    assert_eq!(result.total_suggestions, 0, "立即取消应返回 0 条建议");
    // 但文档数仍正确
    assert_eq!(result.total_documents, 2);
}

/// TC-DREAM-009：DreamResult 序列化/反序列化往返一致。
#[tokio::test]
async fn tc_dream_009_result_serde_roundtrip() {
    let result = DreamResult {
        suggestions: vec![crate::auto_dream::DreamSuggestion {
            suggestion_id: "test-1".to_string(),
            suggestion_type: SuggestionType::DuplicateDocuments,
            title: "测试标题".to_string(),
            description: "测试描述".to_string(),
            doc_ids: vec!["doc-a".to_string(), "doc-b".to_string()],
            doc_names: vec!["a.md".to_string(), "b.md".to_string()],
            severity: DreamSeverity::Medium,
            similarity: Some(0.95),
        }],
        total_documents: 2,
        total_suggestions: 1,
        elapsed_ms: 500,
    };

    let json = serde_json::to_string(&result).unwrap();
    let back: DreamResult = serde_json::from_str(&json).unwrap();

    assert_eq!(back.total_documents, 2);
    assert_eq!(back.total_suggestions, 1);
    assert_eq!(back.suggestions.len(), 1);
    assert_eq!(
        back.suggestions[0].suggestion_type,
        SuggestionType::DuplicateDocuments
    );
    assert_eq!(back.suggestions[0].similarity, Some(0.95));
}

/// TC-DREAM-010：单个文档不产生重复或矛盾建议。
#[tokio::test]
async fn tc_dream_010_single_doc_no_dup_or_contra() {
    let (doc, chunks) = make_doc("only.md", "unique-hash", &["唯一文档内容"]);
    let storage = MockStorage::new().with_doc(doc, chunks);
    let engine = DreamEngine::new(
        MockEmbedder::new(),
        storage,
        contradiction_llm(),
        IdempotencyStore::new(),
    );

    let result = engine.dream(no_cancel()).await.unwrap();

    let dups: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::DuplicateDocuments)
        .collect();
    let contras: Vec<_> = result
        .suggestions
        .iter()
        .filter(|s| s.suggestion_type == SuggestionType::Contradiction)
        .collect();
    assert!(dups.is_empty(), "单个文档不应有重复建议");
    assert!(contras.is_empty(), "单个文档不应有跨文档矛盾");
}
