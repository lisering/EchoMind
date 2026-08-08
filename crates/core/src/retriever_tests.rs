#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-RAG-001 相关度阈值过滤（REQ-RAG-003-AC-1）。

use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, DocStatus, Document, RetrievalResult};

use crate::retriever::{DEFAULT_SCORE_THRESHOLD, VectorRetriever, build_contextual_text};
use crate::{Embedder, Retriever, Storage};

/// 固定向量 Embedder（检索内容不影响本测试断言）。
struct MockEmbedder;

impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(vec![1.0, 0.0, 0.0])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
    }
}

/// 返回预置检索结果的 Storage（含低分污染项）。
/// `all_chunks` 用于 Chunk Expansion 测试（TC-RAG-009）：list_chunks 返回该字段。
#[derive(Default)]
struct MockStorage {
    hits: Vec<RetrievalResult>,
    /// 文档的全部 chunk（按 sequence 正序），供 Chunk Expansion 扩展相邻 chunk 用。
    all_chunks: Vec<Chunk>,
}

impl MockStorage {
    fn hit(score: f32) -> RetrievalResult {
        RetrievalResult {
            chunk: Chunk::new("doc-1".to_string(), format!("片段 {score}"), 5, 0),
            score,
            doc_name: "note.md".to_string(),
        }
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
        Ok(self.hits.clone())
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

    async fn create_conversation(
        &self,
        _conversation: &echomind_models::Conversation,
    ) -> Result<()> {
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
        Ok(vec![])
    }

    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }

    async fn list_chunks(&self, _doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self.all_chunks.clone())
    }

    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }

    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// TC-RAG-001：score < 阈值的检索结果必须被过滤丢弃（REQ-RAG-003-AC-1）。
#[tokio::test]
async fn tc_rag_001_low_score_results_filtered() {
    let storage = MockStorage {
        hits: vec![
            MockStorage::hit(0.92),
            MockStorage::hit(0.31),
            MockStorage::hit(0.34),
        ],
        ..Default::default()
    };
    let retriever = VectorRetriever::new(MockEmbedder, storage);

    let hits = retriever.retrieve("任意问题", 5).await.unwrap();

    assert_eq!(hits.len(), 1, "低于阈值的结果必须全部被丢弃");
    assert!(hits[0].score >= DEFAULT_SCORE_THRESHOLD);
}

/// TC-RAG-001 补充：全部低分时返回空 Vec（交由上层拒答，REQ-RAG-003-AC-2）。
#[tokio::test]
async fn tc_rag_001b_all_low_scores_yield_empty() {
    let storage = MockStorage {
        hits: vec![MockStorage::hit(0.1), MockStorage::hit(0.2)],
        ..Default::default()
    };
    let retriever = VectorRetriever::new(MockEmbedder, storage);

    let hits = retriever.retrieve("任意问题", 5).await.unwrap();

    assert!(hits.is_empty(), "全部低分时必须返回空结果");
}

/// TC-RAG-009：Chunk Expansion — 检索命中后扩展相邻 chunk，补全跨 chunk 上下文（Phase 4）。
///
/// 验证 `VectorRetriever::retrieve()` 在命中后自动扩展前后各 1 个相邻 chunk：
/// - 命中 chunk 在中间（sequence=1），扩展后应包含 sequence=0 和 sequence=2
/// - 扩展的 chunk 使用命中 chunk 的 score 作为近似
/// - 结果按 sequence 排序（前→命中→后）
#[tokio::test]
async fn tc_rag_009_chunk_expansion_includes_neighbors() {
    let doc_id = "expansion-doc".to_string();
    let chunk0 = Chunk::new(doc_id.clone(), "前一段落内容".to_string(), 10, 0);
    let chunk1 = Chunk::new(doc_id.clone(), "命中的片段内容".to_string(), 10, 1);
    let chunk2 = Chunk::new(doc_id.clone(), "后一段落内容".to_string(), 10, 2);

    let storage = MockStorage {
        hits: vec![RetrievalResult {
            chunk: chunk1.clone(),
            score: 0.9,
            doc_name: "expansion.md".to_string(),
        }],
        all_chunks: vec![chunk0, chunk1, chunk2],
    };
    let retriever = VectorRetriever::new(MockEmbedder, storage);

    let results = retriever.retrieve("任意问题", 5).await.unwrap();

    // 断言：扩展后应包含 3 个 chunk（命中 + 前后各 1 个）
    assert_eq!(
        results.len(),
        3,
        "Chunk Expansion 应扩展前后相邻 chunk，实际 {} 个",
        results.len()
    );

    // 断言：包含前一段落
    assert!(
        results.iter().any(|r| r.chunk.content.contains("前一段落")),
        "应包含前一个相邻 chunk"
    );
    // 断言：包含后一段落
    assert!(
        results.iter().any(|r| r.chunk.content.contains("后一段落")),
        "应包含后一个相邻 chunk"
    );
    // 断言：包含命中本身
    assert!(
        results
            .iter()
            .any(|r| r.chunk.content.contains("命中的片段")),
        "应包含命中 chunk 本身"
    );
}

/// TC-RAG-010：ContextualEmbedding 文档名上下文前缀拼接（Phase 3 低成本方案）。
///
/// 验证 `build_contextual_text` 返回的文本包含文档名 + 原始 chunk 内容，
/// 使嵌入向量包含文档上下文信息（Anthropic Contextual Retrieval 最佳实践）。
/// EchoMind 采用零 LLM 成本规则方案：仅拼接文档名前缀。
#[test]
fn tc_rag_010_contextual_text_includes_doc_name() {
    let text = build_contextual_text("lisp-intro.md", "Lisp 是第二古老的编程语言");
    assert!(text.contains("lisp-intro.md"), "应包含文档名作为上下文前缀");
    assert!(
        text.contains("Lisp 是第二古老的编程语言"),
        "应包含原始 chunk 内容"
    );
}
