//! RAG 评估数据集与端到端检索质量评估 TDD 测试（REQ-RAG-048）。
//!
//! 测试覆盖：
//! - RagEvalDataset / RagEvalDatasetSample 数据模型
//! - load_eval_dataset / create_sample_dataset 函数
//! - run_retrieval_eval 端到端检索评估（使用 MockStorage）

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use anyhow::Result;
use echomind_models::{
    ChatMessage, Chunk, Conversation, DocStatus, Document, EntityRelation, RagEvalDataset,
    RagEvalDatasetSample, RagEvalReport, RetrievalResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::Storage;
use crate::rag_eval_dataset::{create_sample_dataset, load_eval_dataset, run_retrieval_eval};

// ============================================================
// Mock Storage（用于端到端检索评估测试）
// ============================================================

/// 存储的向量条目：(chunk_id, doc_id, content, embedding)
type VectorEntry = (String, String, String, Vec<f32>);

/// 测试用 MockStorage，在内存中存储向量，支持 vector_search。
struct MockStorage {
    /// 存储的向量列表
    vectors: Arc<Mutex<Vec<VectorEntry>>>,
    /// 文档列表
    docs: Arc<Mutex<HashMap<String, Document>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            vectors: Arc::new(Mutex::new(Vec::new())),
            docs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 添加一个 chunk 和其嵌入向量。
    async fn add_vector(&self, chunk_id: &str, doc_id: &str, content: &str, embedding: Vec<f32>) {
        self.vectors.lock().await.push((
            chunk_id.to_string(),
            doc_id.to_string(),
            content.to_string(),
            embedding,
        ));
    }
}

// 实现 Storage trait（只实现评估需要的部分，其余默认空操作）
impl Storage for MockStorage {
    async fn add_document(&self, doc: &Document) -> Result<()> {
        self.docs.lock().await.insert(doc.id.clone(), doc.clone());
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
        query_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }

        let vectors = self.vectors.lock().await;
        let mut results: Vec<(f32, RetrievalResult)> = vectors
            .iter()
            .map(|(chunk_id, doc_id, content, emb)| {
                let score = cosine_similarity(query_embedding, emb);
                let chunk = Chunk {
                    id: chunk_id.clone(),
                    doc_id: doc_id.clone(),
                    content: content.clone(),
                    token_count: 0,
                    sequence: 0,
                };
                (
                    score,
                    RetrievalResult {
                        chunk,
                        score,
                        doc_name: format!("doc-{doc_id}"),
                    },
                )
            })
            .collect();

        // 按分数降序排列
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        Ok(results.into_iter().map(|(_, r)| r).collect())
    }

    async fn find_document_by_hash(&self, _hash: &str) -> Result<Option<Document>> {
        Ok(None)
    }

    async fn count_documents(&self) -> Result<usize> {
        Ok(self.docs.lock().await.len())
    }

    async fn count_chunks(&self) -> Result<usize> {
        Ok(self.vectors.lock().await.len())
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

    async fn create_conversation(&self, _conv: &Conversation) -> Result<()> {
        Ok(())
    }

    async fn list_conversations(&self, _workspace_id: &str) -> Result<Vec<Conversation>> {
        Ok(vec![])
    }

    async fn delete_conversation(&self, _id: &str) -> Result<()> {
        Ok(())
    }

    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }

    async fn add_message(&self, _conv_id: &str, _msg: &ChatMessage) -> Result<()> {
        Ok(())
    }

    async fn list_messages(&self, _conv_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }

    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(self.docs.lock().await.values().cloned().collect())
    }

    async fn delete_document(&self, doc_id: &str) -> Result<()> {
        self.docs.lock().await.remove(doc_id);
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

    async fn get_relations_for_entity(&self, _entity_text: &str) -> Result<Vec<EntityRelation>> {
        Ok(vec![])
    }
}

/// 计算余弦相似度（测试辅助函数）。
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

// ============================================================
// 数据模型测试
// ============================================================

/// TC-EVAL-DATASET-001：RagEvalDataset 数据模型基本字段验证。
#[test]
fn tc_eval_dataset_001_model_fields() {
    let sample =
        RagEvalDatasetSample::new("什么是 RAG？".to_string(), "RAG 是检索增强生成".to_string())
            .with_doc_ids(vec!["doc-1".to_string()])
            .with_chunk_ids(vec!["chunk-1".to_string()]);

    assert_eq!(sample.query, "什么是 RAG？");
    assert_eq!(sample.ground_truth, "RAG 是检索增强生成");
    assert_eq!(sample.relevant_doc_ids, vec!["doc-1"]);
    assert_eq!(sample.relevant_chunk_ids, vec!["chunk-1"]);

    let dataset = RagEvalDataset::new("test-dataset".to_string(), vec![sample])
        .with_description("测试数据集".to_string());

    assert_eq!(dataset.name, "test-dataset");
    assert_eq!(dataset.description, "测试数据集");
    assert_eq!(dataset.len(), 1);
    assert!(!dataset.is_empty());
}

/// TC-EVAL-DATASET-002：load_eval_dataset 从 JSON 反序列化，无效 JSON 返回 Err。
#[test]
fn tc_eval_dataset_002_load_from_json() {
    let json = r#"{
        "name": "test",
        "description": "测试",
        "samples": [
            {
                "query": "什么是 RAG",
                "ground_truth": "检索增强生成",
                "relevant_doc_ids": ["doc-1"],
                "relevant_chunk_ids": ["chunk-1", "chunk-2"]
            }
        ]
    }"#;

    let dataset = load_eval_dataset(json).unwrap();
    assert_eq!(dataset.name, "test");
    assert_eq!(dataset.samples.len(), 1);
    assert_eq!(dataset.samples[0].query, "什么是 RAG");
    assert_eq!(
        dataset.samples[0].relevant_chunk_ids,
        vec!["chunk-1", "chunk-2"]
    );

    // 无效 JSON 返回 Err
    let result = load_eval_dataset("invalid json");
    assert!(result.is_err());

    // 空 JSON 返回 Err
    let result = load_eval_dataset("");
    assert!(result.is_err());
}

/// TC-EVAL-DATASET-003：run_retrieval_eval 端到端检索评估，返回 RagEvalReport。
#[tokio::test]
async fn tc_eval_dataset_003_run_retrieval_eval() {
    let storage = MockStorage::new();

    // 添加文档
    let mut doc = Document::new("test.md".to_string(), "hash123".to_string());
    doc.id = "doc-1".to_string();
    doc.status = DocStatus::Indexed;
    storage.add_document(&doc).await.unwrap();

    // 添加向量（3 个 chunk，不同内容）
    storage
        .add_vector("chunk-1", "doc-1", "RAG 检索增强生成", vec![1.0, 0.0, 0.0])
        .await;
    storage
        .add_vector(
            "chunk-2",
            "doc-1",
            "向量搜索余弦相似度",
            vec![0.0, 1.0, 0.0],
        )
        .await;
    storage
        .add_vector(
            "chunk-3",
            "doc-1",
            "SQLite WAL 模式存储",
            vec![0.0, 0.0, 1.0],
        )
        .await;

    // 构建评估数据集（query 嵌入与 chunk-1 相似）
    let dataset = RagEvalDataset::new(
        "test".to_string(),
        vec![
            RagEvalDatasetSample::new("什么是 RAG".to_string(), "RAG 是检索增强生成".to_string())
                .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );

    // 查询嵌入与 chunk-1 一致
    let query_embedding = vec![1.0, 0.0, 0.0];
    let report = run_retrieval_eval(&storage, &dataset, &query_embedding, 3)
        .await
        .unwrap();

    assert_eq!(report.sample_count, 1);
    assert!(!report.aggregate_metrics.is_empty());
    assert!(!report.per_sample_metrics.is_empty());
}

/// TC-EVAL-DATASET-004：HitRate 在检索到相关 chunk 时 = 1.0，无相关时 = 0.0。
#[tokio::test]
async fn tc_eval_dataset_004_hit_rate() {
    let storage = MockStorage::new();

    // 添加 2 个 chunk
    storage
        .add_vector("chunk-a", "doc-1", "内容 A", vec![1.0, 0.0])
        .await;
    storage
        .add_vector("chunk-b", "doc-1", "内容 B", vec![0.0, 1.0])
        .await;

    // 场景 1：检索到相关 chunk → HitRate = 1.0
    let dataset_hit = RagEvalDataset::new(
        "test-hit".to_string(),
        vec![
            RagEvalDatasetSample::new("查询".to_string(), "答案".to_string())
                .with_chunk_ids(vec!["chunk-a".to_string()]),
        ],
    );
    let report = run_retrieval_eval(&storage, &dataset_hit, &[1.0, 0.0], 2)
        .await
        .unwrap();
    let hit_rate_score = report
        .aggregate_metrics
        .iter()
        .find(|m| m.metric_type == echomind_models::RagMetricType::HitRate)
        .map(|m| m.score);
    assert_eq!(hit_rate_score, Some(1.0));

    // 场景 2：没有相关 chunk → HitRate = 0.0
    let dataset_miss = RagEvalDataset::new(
        "test-miss".to_string(),
        vec![
            RagEvalDatasetSample::new("查询".to_string(), "答案".to_string())
                .with_chunk_ids(vec!["nonexistent".to_string()]),
        ],
    );
    let report = run_retrieval_eval(&storage, &dataset_miss, &[1.0, 0.0], 2)
        .await
        .unwrap();
    let hit_rate_score = report
        .aggregate_metrics
        .iter()
        .find(|m| m.metric_type == echomind_models::RagMetricType::HitRate)
        .map(|m| m.score);
    assert_eq!(hit_rate_score, Some(0.0));
}

/// TC-EVAL-DATASET-005：MRR 第一个结果相关时 = 1.0，第 n 个 = 1/n。
#[tokio::test]
async fn tc_eval_dataset_005_mrr() {
    let storage = MockStorage::new();

    // 添加 3 个 chunk，分数排序：chunk-1 > chunk-2 > chunk-3
    storage
        .add_vector("chunk-1", "doc-1", "内容 1", vec![1.0, 0.0, 0.0])
        .await;
    storage
        .add_vector("chunk-2", "doc-1", "内容 2", vec![0.9, 0.1, 0.0])
        .await;
    storage
        .add_vector("chunk-3", "doc-1", "内容 3", vec![0.0, 0.0, 1.0])
        .await;

    // 场景 1：chunk-1 排名第一 → MRR = 1.0
    let dataset = RagEvalDataset::new(
        "test-mrr-1".to_string(),
        vec![
            RagEvalDatasetSample::new("查询".to_string(), "答案".to_string())
                .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );
    let report = run_retrieval_eval(&storage, &dataset, &[1.0, 0.0, 0.0], 3)
        .await
        .unwrap();
    let mrr_score = report
        .aggregate_metrics
        .iter()
        .find(|m| m.metric_type == echomind_models::RagMetricType::MRR)
        .map(|m| m.score);
    assert_eq!(mrr_score, Some(1.0));

    // 场景 2：chunk-3 排名第三 → MRR = 1/3
    let dataset2 = RagEvalDataset::new(
        "test-mrr-3".to_string(),
        vec![
            RagEvalDatasetSample::new("查询".to_string(), "答案".to_string())
                .with_chunk_ids(vec!["chunk-3".to_string()]),
        ],
    );
    let report = run_retrieval_eval(&storage, &dataset2, &[1.0, 0.0, 0.0], 3)
        .await
        .unwrap();
    let mrr_score = report
        .aggregate_metrics
        .iter()
        .find(|m| m.metric_type == echomind_models::RagMetricType::MRR)
        .map(|m| m.score);
    assert!(
        mrr_score.is_some_and(|s| (s - 1.0 / 3.0).abs() < 0.01),
        "MRR 应约为 1/3 = 0.333, 实际 {mrr_score:?}"
    );
}

/// TC-EVAL-DATASET-006：NDCG 完美排序 = 1.0，逆序 < 1.0。
#[tokio::test]
async fn tc_eval_dataset_006_ndcg() {
    let storage = MockStorage::new();

    // 添加 2 个 chunk
    storage
        .add_vector("chunk-rel", "doc-1", "相关内容", vec![1.0, 0.0])
        .await;
    storage
        .add_vector("chunk-irrel", "doc-1", "无关内容", vec![0.0, 1.0])
        .await;

    // 场景 1：相关 chunk 排名第一 → NDCG = 1.0（完美排序）
    let dataset_perfect = RagEvalDataset::new(
        "test-ndcg-perfect".to_string(),
        vec![
            RagEvalDatasetSample::new("查询".to_string(), "答案".to_string())
                .with_chunk_ids(vec!["chunk-rel".to_string()]),
        ],
    );
    let report = run_retrieval_eval(&storage, &dataset_perfect, &[1.0, 0.0], 2)
        .await
        .unwrap();
    let ndcg_score = report
        .aggregate_metrics
        .iter()
        .find(|m| m.metric_type == echomind_models::RagMetricType::NDCG)
        .map(|m| m.score);
    // 完美排序 NDCG = 1.0（[1.0, 0.0] 是理想排序本身）
    assert!(
        ndcg_score.is_some_and(|s| s > 0.9),
        "完美排序 NDCG 应 > 0.9, 实际 {ndcg_score:?}"
    );

    // 场景 2：相关 chunk 排名最后 → NDCG < 完美排序
    // 逆序 [0.0, 1.0]：DCG = 0/log2(2) + 1/log2(3) = 0.631, IDCG = 1.0
    // NDCG = 0.631 < 1.0
    let dataset_reverse = RagEvalDataset::new(
        "test-ndcg-reverse".to_string(),
        vec![
            RagEvalDatasetSample::new("查询".to_string(), "答案".to_string())
                .with_chunk_ids(vec!["chunk-irrel".to_string()]),
        ],
    );
    let report = run_retrieval_eval(&storage, &dataset_reverse, &[1.0, 0.0], 2)
        .await
        .unwrap();
    let ndcg_reverse = report
        .aggregate_metrics
        .iter()
        .find(|m| m.metric_type == echomind_models::RagMetricType::NDCG)
        .map(|m| m.score);
    assert!(
        ndcg_reverse.is_some_and(|s| s < ndcg_score.unwrap_or(1.0)),
        "逆序 NDCG ({ndcg_reverse:?}) 应 < 完美排序 NDCG ({ndcg_score:?})"
    );
}

/// TC-EVAL-DATASET-007：create_sample_dataset 返回 ≥ 5 个 query 的中英文混合数据集。
#[test]
fn tc_eval_dataset_007_create_sample_dataset() {
    let dataset = create_sample_dataset();
    assert!(!dataset.is_empty());
    assert!(dataset.len() >= 5, "应包含至少 5 个 query");
    assert!(!dataset.name.is_empty());
    // 确保中英文混合
    let has_chinese = dataset.samples.iter().any(|s| {
        s.query
            .chars()
            .any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
    });
    let has_english = dataset
        .samples
        .iter()
        .any(|s| s.query.chars().any(|c| c.is_ascii_alphabetic()));
    assert!(has_chinese, "应包含中文 query");
    assert!(has_english, "应包含英文 query");
}

/// TC-EVAL-DATASET-008：run_retrieval_eval 在空知识库时不 panic，返回全 0 分报告。
#[tokio::test]
async fn tc_eval_dataset_008_empty_kb() {
    let storage = MockStorage::new(); // 空库
    let dataset = RagEvalDataset::new(
        "empty".to_string(),
        vec![RagEvalDatasetSample::new(
            "查询".to_string(),
            "答案".to_string(),
        )],
    );

    let report = run_retrieval_eval(&storage, &dataset, &[1.0, 0.0], 5)
        .await
        .unwrap();
    assert_eq!(report.sample_count, 1);
    // 所有指标应为 0.0
    for metric in &report.aggregate_metrics {
        assert_eq!(
            metric.score, 0.0,
            "空库所有指标应为 0.0, {:?} = {}",
            metric.metric_type, metric.score
        );
    }
}

/// TC-EVAL-DATASET-009：run_retrieval_eval 在 top_k=0 时返回空指标列表。
#[tokio::test]
async fn tc_eval_dataset_009_top_k_zero() {
    let storage = MockStorage::new();
    storage
        .add_vector("chunk-1", "doc-1", "内容", vec![1.0])
        .await;

    let dataset = RagEvalDataset::new(
        "test".to_string(),
        vec![RagEvalDatasetSample::new(
            "查询".to_string(),
            "答案".to_string(),
        )],
    );

    let report = run_retrieval_eval(&storage, &dataset, &[1.0], 0)
        .await
        .unwrap();
    // top_k=0 → 每个样本的指标列表为空
    for sample_metrics in &report.per_sample_metrics {
        assert!(sample_metrics.is_empty(), "top_k=0 时每样本指标应为空");
    }
}

/// TC-EVAL-DATASET-010：RagEvalReport 序列化往返一致。
#[tokio::test]
async fn tc_eval_dataset_010_report_serde_roundtrip() {
    let storage = MockStorage::new();
    storage
        .add_vector("chunk-1", "doc-1", "RAG 检索增强生成", vec![1.0, 0.0])
        .await;

    let dataset = RagEvalDataset::new(
        "serde-test".to_string(),
        vec![
            RagEvalDatasetSample::new("什么是 RAG".to_string(), "检索增强生成".to_string())
                .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );

    let report = run_retrieval_eval(&storage, &dataset, &[1.0, 0.0], 1)
        .await
        .unwrap();

    // 序列化 → 反序列化 → 比较
    let json = serde_json::to_string(&report).unwrap();
    let deserialized: RagEvalReport = serde_json::from_str(&json).unwrap();
    assert_eq!(report.sample_count, deserialized.sample_count);
    assert_eq!(
        report.aggregate_metrics.len(),
        deserialized.aggregate_metrics.len()
    );
    assert_eq!(
        report.per_sample_metrics.len(),
        deserialized.per_sample_metrics.len()
    );
}
