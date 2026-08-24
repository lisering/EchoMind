//! 嵌入模型对比评估引擎（REQ-VEC-018）。
//!
//! 在 REQ-RAG-048 检索质量评估框架基础上，实现多嵌入模型对比评估：
//! 选择 2-3 个嵌入模型 → 逐模型嵌入全部 chunks → 逐 query 检索 → 计算指标 → 对比。
//!
//! # 架构设计
//!
//! ## 核心函数 `run_embed_comparison`
//!
//! 接收 `Storage` + `Embedder` 工厂闭包 + `RagEvalDataset` + 模型名称列表。
//! 工厂闭包按模型名称创建 `Embedder` 实例（生产环境用 `LocalEmbedder`，测试用 mock）。
//!
//! ## 评估流程
//!
//! 1. 对每个模型，调用工厂闭包创建 embedder
//! 2. 读取知识库中全部 chunks
//! 3. 用当前模型嵌入全部 chunks（写入临时内存向量存储）
//! 4. 对数据集中每个 query，用当前模型嵌入 → 检索 top-k → 计算指标
//! 5. 聚合 Hit Rate / MRR / NDCG 平均值
//! 6. 返回 `Vec<EmbedComparisonResult>` 供前端展示
//!
//! ## 进度回调
//!
//! `ProgressFn` 回调在每个模型开始/完成时推送进度事件。
//!
//! # 零新增依赖
//!
//! 复用 `RagEvaluator` + `RagEvalDataset` + `Embedder` trait。

use std::sync::Arc;

use anyhow::{Context, Result};
use echomind_models::{
    EmbedComparisonResult, EmbedMetricScores, RagEvalDataset, RagEvalDatasetSample, RetrievalResult,
};

use crate::Embedder;
use crate::rag_eval::{hit_rate, mrr, ndcg};

/// 进度回调函数类型（REQ-VEC-018 AC-5）。
///
/// 在每个模型开始/完成嵌入+检索时调用，推送进度事件。
/// 使用 `Arc<dyn Fn>` 以便跨线程传递。
pub type ProgressFn = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

/// 评估进度事件（REQ-VEC-018 AC-5）。
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// 开始评估某个模型
    ModelStarted {
        /// 模型名称
        model_name: String,
        /// 模型序号（0-based）
        index: usize,
        /// 总模型数
        total: usize,
    },
    /// 某个模型嵌入 chunks 完成
    EmbeddingDone {
        /// 模型名称
        model_name: String,
        /// 嵌入的 chunk 数量
        chunk_count: usize,
    },
    /// 某个模型评估完成
    ModelCompleted {
        /// 模型名称
        model_name: String,
        /// 评估结果
        result: EmbedComparisonResult,
    },
    /// 全部评估完成
    AllCompleted {
        /// 全部模型结果
        results: Vec<EmbedComparisonResult>,
    },
}

/// 内存临时向量存储（用于嵌入对比评估）。
///
/// 不写入 SQLite，仅在内存中存储 chunk_id → 向量映射，
/// 支持 `vector_search` 检索操作。
struct TempVectorStore {
    /// (chunk_id, vector) 对列表
    vectors: Vec<(String, Vec<f32>)>,
}

impl TempVectorStore {
    fn new() -> Self {
        Self {
            vectors: Vec::new(),
        }
    }

    /// 添加向量。
    fn add(&mut self, chunk_id: String, vector: Vec<f32>) {
        self.vectors.push((chunk_id, vector));
    }

    /// 向量检索：返回 top-k 最相似的 chunk_id。
    ///
    /// 使用余弦相似度全量扫描（评估场景向量数少，无需 HNSW）。
    fn search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)> {
        if self.vectors.is_empty() || query.is_empty() || top_k == 0 {
            return Vec::new();
        }

        let mut scored: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(id, vec)| {
                let score = cosine_similarity(query, vec);
                (id.clone(), score)
            })
            .collect();

        // 降序排序，取 top-k
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }
}

/// 余弦相似度计算（复用 rag_eval.rs 中的实现逻辑）。
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

/// 运行嵌入模型对比评估（REQ-VEC-018 核心函数）。
///
/// 对每个模型执行：创建 embedder → 嵌入全部 chunks → 嵌入 query → 检索 → 计算指标。
///
/// # 参数
/// - `chunks`：参与评估的 (chunk_id, chunk_text) 列表（从知识库读取）
/// - `dataset`：评估数据集（包含 query + relevant_chunk_ids）
/// - `model_names`：参与对比的模型名称列表
/// - `embedder_factory`：按模型名称创建 Embedder 的工厂闭包
/// - `top_k`：每个查询检索的 top-k 结果数
/// - `progress`：进度回调（`None` 表示不需要进度推送）
///
/// # 返回
/// `Vec<EmbedComparisonResult>`，每个模型一个结果
///
/// # 错误
/// - 模型创建失败（如 ONNX 加载失败）
/// - 嵌入推理失败
pub async fn run_embed_comparison<E, F>(
    chunks: &[(String, String)],
    dataset: &RagEvalDataset,
    model_names: &[String],
    embedder_factory: F,
    top_k: usize,
    progress: Option<ProgressFn>,
) -> Result<Vec<EmbedComparisonResult>>
where
    E: Embedder,
    F: Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<E>> + Send>>,
{
    if model_names.is_empty() {
        return Ok(Vec::new());
    }

    if dataset.is_empty() {
        return Ok(Vec::new());
    }

    let total = model_names.len();
    let mut results = Vec::with_capacity(total);

    for (index, model_name) in model_names.iter().enumerate() {
        // 推送开始进度
        if let Some(ref p) = progress {
            p(ProgressEvent::ModelStarted {
                model_name: model_name.clone(),
                index,
                total,
            });
        }

        // 创建 embedder
        let embedder = embedder_factory(model_name)
            .await
            .with_context(|| format!("创建嵌入模型 '{model_name}' 失败"))?;

        // 嵌入全部 chunks
        let chunk_texts: Vec<String> = chunks.iter().map(|(_, text)| text.clone()).collect();
        let chunk_embeddings = embedder
            .embed_batch(&chunk_texts)
            .await
            .with_context(|| format!("嵌入 chunks 失败（模型: {model_name}）"))?;

        // 从嵌入结果推断向量维度
        let dim = chunk_embeddings.first().map(|v| v.len()).unwrap_or(0);

        // 构建临时向量存储
        let mut store = TempVectorStore::new();
        for ((chunk_id, _), embedding) in chunks.iter().zip(chunk_embeddings.iter()) {
            store.add(chunk_id.clone(), embedding.clone());
        }

        if let Some(ref p) = progress {
            p(ProgressEvent::EmbeddingDone {
                model_name: model_name.clone(),
                chunk_count: store.vectors.len(),
            });
        }

        // 逐 query 评估
        let mut per_sample_metrics: Vec<Vec<echomind_models::RagEvalMetric>> =
            Vec::with_capacity(dataset.len());

        for sample in &dataset.samples {
            let metrics = evaluate_single_query(&embedder, &store, sample, top_k).await?;
            per_sample_metrics.push(metrics);
        }

        // 聚合指标
        let avg_metrics = EmbedMetricScores::from_metrics_avg(&per_sample_metrics);

        let result = EmbedComparisonResult {
            model_name: model_name.clone(),
            dim,
            metrics: avg_metrics,
            sample_count: dataset.len(),
        };

        if let Some(ref p) = progress {
            p(ProgressEvent::ModelCompleted {
                model_name: model_name.clone(),
                result: result.clone(),
            });
        }

        results.push(result);
    }

    if let Some(ref p) = progress {
        p(ProgressEvent::AllCompleted {
            results: results.clone(),
        });
    }

    Ok(results)
}

/// 对单个查询执行检索评估。
///
/// 嵌入 query → 临时存储检索 top-k → 与 relevant_chunk_ids 比对 → 计算指标。
async fn evaluate_single_query<E: Embedder>(
    embedder: &E,
    store: &TempVectorStore,
    sample: &RagEvalDatasetSample,
    top_k: usize,
) -> Result<Vec<echomind_models::RagEvalMetric>> {
    use echomind_models::{RagEvalMetric, RagMetricType};

    if top_k == 0 || sample.relevant_chunk_ids.is_empty() {
        return Ok(vec![
            RagEvalMetric::new(RagMetricType::HitRate, 0.0),
            RagEvalMetric::new(RagMetricType::MRR, 0.0),
            RagEvalMetric::new(RagMetricType::NDCG, 0.0),
        ]);
    }

    // 嵌入 query
    let query_embedding = embedder
        .embed(&sample.query)
        .await
        .context("嵌入 query 失败")?;

    // 检索 top-k
    let search_results = store.search(&query_embedding, top_k);

    // 如果无检索结果
    if search_results.is_empty() {
        return Ok(vec![
            RagEvalMetric::new(RagMetricType::HitRate, 0.0),
            RagEvalMetric::new(RagMetricType::MRR, 0.0),
            RagEvalMetric::new(RagMetricType::NDCG, 0.0),
        ]);
    }

    // 构建 relevance 布尔数组
    let relevance: Vec<bool> = search_results
        .iter()
        .map(|(chunk_id, _)| sample.relevant_chunk_ids.contains(chunk_id))
        .collect();

    // 构建 relevance_scores（相关=1.0, 不相关=0.0）
    let relevance_scores: Vec<f32> = relevance
        .iter()
        .map(|&r| if r { 1.0 } else { 0.0 })
        .collect();

    // 计算三个纯 Rust 检索指标
    let hit = hit_rate(&relevance);
    let mrr_score = mrr(&relevance);
    let ndcg_score = ndcg(&relevance_scores);

    Ok(vec![
        RagEvalMetric::new(RagMetricType::HitRate, hit),
        RagEvalMetric::new(RagMetricType::MRR, mrr_score),
        RagEvalMetric::new(RagMetricType::NDCG, ndcg_score),
    ])
}

/// 将检索结果转换为 chunk_id 列表（辅助函数）。
#[allow(dead_code)]
pub fn extract_chunk_ids(results: &[RetrievalResult]) -> Vec<String> {
    results.iter().map(|r| r.chunk.id.clone()).collect()
}
