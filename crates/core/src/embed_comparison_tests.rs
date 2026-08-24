//! 嵌入模型对比评估 TDD 测试（REQ-VEC-018）。
//!
//! TC-VEC-EVAL-001~005 覆盖：
//! - 基本对比流程（2 模型）
//! - 内置示例数据集
//! - Hit Rate / MRR / NDCG 指标计算
//! - 进度回调
//! - 空数据集/空模型列表边界场景

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use echomind_models::{
    EmbedComparisonRequest, EmbedMetricScores, RagEvalDataset, RagEvalDatasetSample,
};

use crate::Embedder;
use crate::embed_comparison::{ProgressEvent, ProgressFn, run_embed_comparison};
use std::sync::Arc;

/// Mock Embedder：返回确定性向量（每个词映射为固定维度）。
///
/// 不同 "模型" 通过不同的 seed 产生略微不同的向量，
/// 模拟不同嵌入模型的检索质量差异。
struct MockEmbedder {
    /// 向量维度
    dim: usize,
    /// 种子（模拟不同模型的不同嵌入质量）
    seed: f32,
}

impl MockEmbedder {
    fn new(dim: usize, seed: f32) -> Self {
        Self { dim, seed }
    }

    /// 将文本转换为确定性向量。
    ///
    /// 策略：对每个字符的 ASCII 值取模 dim 作为索引，
    /// 在对应位置累加 1.0 * seed。最后归一化。
    fn text_to_vector(&self, text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dim];
        for ch in text.chars() {
            let idx = (ch as usize) % self.dim;
            vec[idx] += 1.0 * self.seed;
        }
        // 归一化
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }
        vec
    }
}

impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(self.text_to_vector(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| self.text_to_vector(t)).collect())
    }
}

/// TC-VEC-EVAL-001：基本对比流程（2 个模型）。
///
/// 验证 `run_embed_comparison` 对 2 个模型返回 2 个结果，
/// 每个结果包含模型名称、维度和三指标。
#[tokio::test]
async fn tc_vec_eval_001_basic_comparison() {
    let chunks = vec![
        ("chunk-1".to_string(), "RAG 检索增强生成技术".to_string()),
        (
            "chunk-2".to_string(),
            "向量嵌入模型 fastembed ONNX".to_string(),
        ),
        ("chunk-3".to_string(), "SQLite WAL mode storage".to_string()),
    ];

    let dataset = RagEvalDataset::new(
        "test-dataset".to_string(),
        vec![
            RagEvalDatasetSample::new("什么是 RAG".to_string(), "RAG 是检索增强生成".to_string())
                .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );

    let model_names = vec!["model-a".to_string(), "model-b".to_string()];
    let top_k = 3;

    let results = run_embed_comparison(
        &chunks,
        &dataset,
        &model_names,
        |name| {
            let dim = if name == "model-a" { 384 } else { 512 };
            let seed = if name == "model-a" { 1.0 } else { 1.1 };
            Box::pin(async move { Ok(MockEmbedder::new(dim, seed)) })
        },
        top_k,
        None,
    )
    .await
    .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].model_name, "model-a");
    assert_eq!(results[0].dim, 384);
    assert_eq!(results[0].sample_count, 1);
    assert_eq!(results[1].model_name, "model-b");
    assert_eq!(results[1].dim, 512);

    // 指标在 [0, 1] 范围内
    for r in &results {
        assert!((0.0..=1.0).contains(&r.metrics.hit_rate));
        assert!((0.0..=1.0).contains(&r.metrics.mrr));
        assert!((0.0..=1.0).contains(&r.metrics.ndcg));
    }
}

/// TC-VEC-EVAL-002：使用内置示例数据集。
///
/// 验证 `create_sample_dataset()` 返回的数据集可用于对比评估。
#[tokio::test]
async fn tc_vec_eval_002_sample_dataset() {
    let dataset = crate::rag_eval_dataset::create_sample_dataset();
    assert!(dataset.len() >= 5);

    let chunks = vec![
        ("chunk-rag-1".to_string(), "RAG 检索增强生成".to_string()),
        (
            "chunk-rag-2".to_string(),
            "RAG combines retrieval and generation".to_string(),
        ),
        (
            "chunk-emb-1".to_string(),
            "fastembed ONNX embedding model".to_string(),
        ),
        ("chunk-sqlite-1".to_string(), "SQLite WAL mode".to_string()),
        (
            "chunk-sqlite-2".to_string(),
            "Write-Ahead Logging".to_string(),
        ),
        (
            "chunk-pii-1".to_string(),
            "PII detection privacy".to_string(),
        ),
        (
            "chunk-arch-1".to_string(),
            "hexagonal architecture ports".to_string(),
        ),
        ("chunk-arch-2".to_string(), "adapters pattern".to_string()),
        (
            "chunk-onnx-1".to_string(),
            "ONNX model local inference".to_string(),
        ),
    ];

    let model_names = vec!["model-a".to_string()];
    let results = run_embed_comparison(
        &chunks,
        &dataset,
        &model_names,
        |_| Box::pin(async move { Ok(MockEmbedder::new(384, 1.0)) }),
        5,
        None,
    )
    .await
    .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].sample_count, dataset.len());
}

/// TC-VEC-EVAL-003：Hit Rate / MRR / NDCG 指标计算正确性。
///
/// 当 query 与某个 chunk 完全相同时，该 chunk 应排在第一位，
/// Hit Rate = 1.0, MRR = 1.0, NDCG = 1.0。
#[tokio::test]
async fn tc_vec_eval_003_metric_correctness() {
    let chunks = vec![
        ("chunk-1".to_string(), "machine learning".to_string()),
        (
            "chunk-2".to_string(),
            "deep learning neural network".to_string(),
        ),
        ("chunk-3".to_string(), "database SQL query".to_string()),
    ];

    let dataset = RagEvalDataset::new(
        "test".to_string(),
        vec![
            RagEvalDatasetSample::new(
                "machine learning".to_string(), // query 与 chunk-1 完全相同
                "ML is a subset of AI".to_string(),
            )
            .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );

    let results = run_embed_comparison(
        &chunks,
        &dataset,
        &["model-x".to_string()],
        |_| Box::pin(async move { Ok(MockEmbedder::new(256, 1.0)) }),
        3,
        None,
    )
    .await
    .unwrap();

    // query "machine learning" 与 chunk-1 "machine learning" 完全相同
    // 理想情况下 chunk-1 排第一，Hit Rate=1.0, MRR=1.0
    let r = &results[0];
    assert_eq!(r.model_name, "model-x");
    // 由于 MockEmbedder 的确定性映射，query 与 chunk-1 的向量完全一致
    // cosine similarity = 1.0，chunk-1 必然排第一
    assert!(
        r.metrics.hit_rate >= 0.99,
        "Hit Rate should be ~1.0, got {}",
        r.metrics.hit_rate
    );
    assert!(
        r.metrics.mrr >= 0.99,
        "MRR should be ~1.0, got {}",
        r.metrics.mrr
    );
}

/// TC-VEC-EVAL-004：进度回调被正确调用。
///
/// 验证 ProgressEvent 在评估过程中被推送：
/// - ModelStarted（每个模型开始时）
/// - EmbeddingDone（嵌入完成后）
/// - ModelCompleted（每个模型完成时）
/// - AllCompleted（全部完成时）
#[tokio::test]
async fn tc_vec_eval_004_progress_callback() {
    use std::sync::Mutex;

    let chunks = vec![
        ("chunk-1".to_string(), "hello world".to_string()),
        ("chunk-2".to_string(), "foo bar baz".to_string()),
    ];

    let dataset = RagEvalDataset::new(
        "test".to_string(),
        vec![
            RagEvalDatasetSample::new("hello".to_string(), "world greeting".to_string())
                .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );

    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let progress: ProgressFn = Arc::new(move |evt: ProgressEvent| {
        let label = match &evt {
            ProgressEvent::ModelStarted { model_name, .. } => {
                format!("started:{model_name}")
            }
            ProgressEvent::EmbeddingDone { model_name, .. } => {
                format!("embedded:{model_name}")
            }
            ProgressEvent::ModelCompleted { model_name, .. } => {
                format!("completed:{model_name}")
            }
            ProgressEvent::AllCompleted { .. } => "all_completed".to_string(),
        };
        events_clone.lock().unwrap().push(label);
    });

    let model_names = vec!["m1".to_string(), "m2".to_string()];
    let _results = run_embed_comparison(
        &chunks,
        &dataset,
        &model_names,
        |_| Box::pin(async move { Ok(MockEmbedder::new(128, 1.0)) }),
        2,
        Some(progress),
    )
    .await
    .unwrap();

    let recorded = events.lock().unwrap();
    // 2 models × 3 events each + 1 AllCompleted = 7
    assert!(
        recorded.len() >= 7,
        "Expected at least 7 progress events, got {}: {:?}",
        recorded.len(),
        recorded
    );
    assert!(recorded.iter().any(|e| e == "started:m1"));
    assert!(recorded.iter().any(|e| e == "started:m2"));
    assert!(recorded.iter().any(|e| e == "completed:m1"));
    assert!(recorded.iter().any(|e| e == "completed:m2"));
    assert!(recorded.iter().any(|e| e == "all_completed"));
}

/// TC-VEC-EVAL-005：空数据集和空模型列表边界场景。
///
/// - 空模型列表 → 返回空结果
/// - 空数据集 → 返回空结果
/// - top_k=0 → 每个指标为 0.0
#[tokio::test]
async fn tc_vec_eval_005_edge_cases() {
    let chunks = vec![("chunk-1".to_string(), "test".to_string())];
    let empty_dataset = RagEvalDataset::new("empty".to_string(), vec![]);

    // 空模型列表
    let results = run_embed_comparison(
        &chunks,
        &empty_dataset,
        &[],
        |_| Box::pin(async move { Ok(MockEmbedder::new(64, 1.0)) }),
        5,
        None,
    )
    .await
    .unwrap();
    assert!(
        results.is_empty(),
        "Empty model list should return empty results"
    );

    // 空数据集
    let results = run_embed_comparison(
        &chunks,
        &empty_dataset,
        &["m1".to_string()],
        |_| Box::pin(async move { Ok(MockEmbedder::new(64, 1.0)) }),
        5,
        None,
    )
    .await
    .unwrap();
    assert!(
        results.is_empty(),
        "Empty dataset should return empty results"
    );

    // top_k=0
    let dataset = RagEvalDataset::new(
        "test".to_string(),
        vec![
            RagEvalDatasetSample::new("q".to_string(), "a".to_string())
                .with_chunk_ids(vec!["chunk-1".to_string()]),
        ],
    );
    let results = run_embed_comparison(
        &chunks,
        &dataset,
        &["m1".to_string()],
        |_| Box::pin(async move { Ok(MockEmbedder::new(64, 1.0)) }),
        0,
        None,
    )
    .await
    .unwrap();
    assert_eq!(results.len(), 1);
    // top_k=0 时指标应为 0
    assert_eq!(results[0].metrics.hit_rate, 0.0);
    assert_eq!(results[0].metrics.mrr, 0.0);
    assert_eq!(results[0].metrics.ndcg, 0.0);
}

/// TC-VEC-EVAL-006：EmbedMetricScores::from_metrics_avg 聚合正确性。
#[test]
fn tc_vec_eval_006_metrics_aggregation() {
    use echomind_models::{RagEvalMetric, RagMetricType};

    let metrics = vec![
        vec![
            RagEvalMetric::new(RagMetricType::HitRate, 1.0),
            RagEvalMetric::new(RagMetricType::MRR, 0.5),
            RagEvalMetric::new(RagMetricType::NDCG, 0.8),
        ],
        vec![
            RagEvalMetric::new(RagMetricType::HitRate, 0.0),
            RagEvalMetric::new(RagMetricType::MRR, 0.0),
            RagEvalMetric::new(RagMetricType::NDCG, 0.6),
        ],
    ];

    let scores = EmbedMetricScores::from_metrics_avg(&metrics);
    // avg hit_rate = (1.0 + 0.0) / 2 = 0.5
    assert!((scores.hit_rate - 0.5).abs() < 0.01);
    // avg mrr = (0.5 + 0.0) / 2 = 0.25
    assert!((scores.mrr - 0.25).abs() < 0.01);
    // avg ndcg = (0.8 + 0.6) / 2 = 0.7
    assert!((scores.ndcg - 0.7).abs() < 0.01);
}

/// TC-VEC-EVAL-007：EmbedComparisonRequest 序列化/反序列化。
#[test]
fn tc_vec_eval_007_request_serde() {
    let req = EmbedComparisonRequest {
        model_names: vec!["model-a".to_string(), "model-b".to_string()],
        top_k: 10,
        dataset_json: Some(r#"{"name":"test","samples":[]}"#.to_string()),
    };
    let json = serde_json::to_string(&req).unwrap();
    let decoded: EmbedComparisonRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.model_names, req.model_names);
    assert_eq!(decoded.top_k, req.top_k);
    assert_eq!(decoded.dataset_json, req.dataset_json);
}
