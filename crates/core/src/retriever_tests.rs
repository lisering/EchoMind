#![allow(clippy::unwrap_used)]
//! QA 红灯测试：TC-RAG-001 相关度阈值过滤（REQ-RAG-003-AC-1）。

use anyhow::Result;
use echomind_models::{Chunk, DocStatus, Document, RetrievalResult};

use crate::retriever::{DEFAULT_SCORE_THRESHOLD, VectorRetriever};
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
struct MockStorage {
    hits: Vec<RetrievalResult>,
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

    async fn cleanup_zombies(&self) -> Result<usize> {
        Ok(0)
    }

    async fn set_setting(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }

    async fn get_setting(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

/// TC-RAG-001：score < 0.5 的检索结果必须被过滤丢弃（REQ-RAG-003-AC-1）。
#[tokio::test]
async fn tc_rag_001_low_score_results_filtered() {
    let storage = MockStorage {
        hits: vec![
            MockStorage::hit(0.92),
            MockStorage::hit(0.41),
            MockStorage::hit(0.49),
        ],
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
    };
    let retriever = VectorRetriever::new(MockEmbedder, storage);

    let hits = retriever.retrieve("任意问题", 5).await.unwrap();

    assert!(hits.is_empty(), "全部低分时必须返回空结果");
}
