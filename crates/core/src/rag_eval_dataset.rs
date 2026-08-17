//! RAG 评估数据集与端到端检索质量评估（REQ-RAG-048）。
//!
//! 在 REQ-RAG-045 RAGAS 指标框架基础上，构建标准评估数据集，
//! 实现端到端检索质量评估管线：
//! 导入语料 → 嵌入 → 检索 → 评估指标计算 → 聚合报告。
//!
//! # 架构设计
//!
//! ## 评估数据集
//!
//! - `RagEvalDataset`：包含多个 `RagEvalDatasetSample`，JSON 格式
//! - 每个 sample 定义 query + ground_truth + relevant_chunk_ids
//! - `create_sample_dataset()` 提供内置中英文混合数据集
//!
//! ## 端到端检索评估
//!
//! - `run_retrieval_eval()` 接收 Storage + Dataset + query_embedding
//! - 对每个 sample 执行 `vector_search`，收集检索结果
//! - 计算纯 Rust 检索指标（HitRate / MRR / NDCG）
//! - 返回 `RagEvalReport` 聚合报告
//!
//! # 零 LLM 依赖
//!
//! 纯 Rust 检索指标不需要 LLM 调用，可在 CI 中无 API Key 运行。

use anyhow::Result;
use echomind_models::{
    RagEvalDataset, RagEvalDatasetSample, RagEvalMetric, RagEvalReport, RagMetricType,
    RetrievalResult,
};

use crate::Storage;
use crate::rag_eval::{hit_rate, mrr, ndcg};

/// 从 JSON 字符串加载评估数据集。
///
/// # 参数
/// - `json`：JSON 格式的数据集字符串
///
/// # 返回
/// - `Ok(dataset)`：反序列化成功
/// - `Err(...)`：JSON 解析失败
///
/// # 示例
/// ```ignore
/// let json = r#"{"name":"test","samples":[{"query":"q","ground_truth":"a","relevant_chunk_ids":["c1"]}]}"#;
/// let dataset = load_eval_dataset(json)?;
/// ```
pub fn load_eval_dataset(json: &str) -> Result<RagEvalDataset> {
    Ok(serde_json::from_str(json)?)
}

/// 对数据集执行端到端检索质量评估。
///
/// 对数据集中每个 query 使用预计算的查询嵌入执行 `vector_search`，
/// 收集检索结果，计算纯 Rust 检索指标（HitRate / MRR / NDCG），
/// 返回 `RagEvalReport` 聚合报告。
///
/// # 参数
/// - `storage`：实现 `Storage` trait 的存储适配器
/// - `dataset`：评估数据集
/// - `query_embedding`：查询的嵌入向量（所有 sample 共用同一个测试嵌入）
/// - `top_k`：每个查询检索的 top-k 结果数
///
/// # 返回
/// - `Ok(report)`：评估完成，返回聚合报告
/// - `Err(...)`：存储层错误
///
/// # 注意
/// - 空知识库（0 文档）时返回全 0 分报告，不 panic
/// - `top_k=0` 时每个 sample 的指标列表为空
pub async fn run_retrieval_eval(
    storage: &impl Storage,
    dataset: &RagEvalDataset,
    query_embedding: &[f32],
    top_k: usize,
) -> Result<RagEvalReport> {
    if dataset.is_empty() {
        return Ok(RagEvalReport::empty());
    }

    let mut per_sample_metrics: Vec<Vec<RagEvalMetric>> = Vec::with_capacity(dataset.len());

    for sample in &dataset.samples {
        let metrics = run_single_eval(storage, sample, query_embedding, top_k).await?;
        per_sample_metrics.push(metrics);
    }

    Ok(RagEvalReport::from_samples(per_sample_metrics))
}

/// 对单个评估样本执行检索评估。
///
/// 执行 `vector_search` 获取 top-k 检索结果，
/// 将结果与 `relevant_chunk_ids` 比对计算指标。
async fn run_single_eval(
    storage: &impl Storage,
    sample: &RagEvalDatasetSample,
    query_embedding: &[f32],
    top_k: usize,
) -> Result<Vec<RagEvalMetric>> {
    if top_k == 0 {
        return Ok(vec![]);
    }

    // 执行向量检索
    let results = storage.vector_search(query_embedding, top_k).await?;

    // 如果无检索结果或无相关 chunk IDs，返回全 0 指标
    if results.is_empty() || sample.relevant_chunk_ids.is_empty() {
        return Ok(vec![
            RagEvalMetric::new(RagMetricType::HitRate, 0.0),
            RagEvalMetric::new(RagMetricType::MRR, 0.0),
            RagEvalMetric::new(RagMetricType::NDCG, 0.0),
        ]);
    }

    // 构建 relevance 布尔数组（每个检索结果是否在 relevant_chunk_ids 中）
    let relevance: Vec<bool> = results
        .iter()
        .map(|r| sample.relevant_chunk_ids.contains(&r.chunk.id))
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

/// 创建内置样例评估数据集（中英文混合）。
///
/// 包含 6 个查询样本，覆盖不同主题：
/// - RAG 检索增强生成
/// - 向量嵌入
/// - SQLite 存储引擎
/// - 隐私安全
/// - Hexagonal architecture
/// - ONNX model
///
/// # 返回
/// `RagEvalDataset`，包含 6 个 sample
pub fn create_sample_dataset() -> RagEvalDataset {
    let samples = vec![
        RagEvalDatasetSample::new(
            "什么是 RAG 检索增强生成".to_string(),
            "RAG (Retrieval-Augmented Generation) 是一种结合检索和生成的技术，\
             系统从知识库中检索相关文档，然后使用检索到的上下文生成答案。"
                .to_string(),
        )
        .with_doc_ids(vec!["doc-rag-intro".to_string()])
        .with_chunk_ids(vec!["chunk-rag-1".to_string(), "chunk-rag-2".to_string()]),
        RagEvalDatasetSample::new(
            "向量嵌入模型有哪些".to_string(),
            "EchoMind 使用 fastembed ONNX 嵌入模型，默认 bge-small-en-v1.5（384 维），\
             可选 bge-m3（1024 维多语言）。"
                .to_string(),
        )
        .with_doc_ids(vec!["doc-embedding".to_string()])
        .with_chunk_ids(vec!["chunk-emb-1".to_string()]),
        RagEvalDatasetSample::new(
            "How does SQLite WAL mode work".to_string(),
            "SQLite WAL (Write-Ahead Logging) mode enables concurrent readers and a single writer. \
             Changes are written to a WAL file first, then checkpointed to the main database."
                .to_string(),
        )
        .with_doc_ids(vec!["doc-sqlite".to_string()])
        .with_chunk_ids(vec!["chunk-sqlite-1".to_string(), "chunk-sqlite-2".to_string()]),
        RagEvalDatasetSample::new(
            "隐私安全 PII 检测脱敏".to_string(),
            "EchoMind 检测 8 类 PII（邮箱/电话/身份证/银行卡/IP/SSN/护照/国际电话），\
             使用正则表达式扫描并替换为 [REDACTED-TYPE] 标记。"
                .to_string(),
        )
        .with_doc_ids(vec!["doc-privacy".to_string()])
        .with_chunk_ids(vec!["chunk-pii-1".to_string()]),
        RagEvalDatasetSample::new(
            "What is hexagonal architecture".to_string(),
            "Hexagonal architecture (ports and adapters) separates business logic from \
             infrastructure. Core defines ports (traits), adapters implement them."
                .to_string(),
        )
        .with_doc_ids(vec!["doc-arch".to_string()])
        .with_chunk_ids(vec!["chunk-arch-1".to_string(), "chunk-arch-2".to_string()]),
        RagEvalDatasetSample::new(
            "ONNX 模型本地推理".to_string(),
            "EchoMind 使用 fastembed 库加载 ONNX Runtime 进行本地嵌入推理，\
             避免向远程 API 发送数据，保护隐私。"
                .to_string(),
        )
        .with_doc_ids(vec!["doc-onnx".to_string()])
        .with_chunk_ids(vec!["chunk-onnx-1".to_string()]),
    ];

    RagEvalDataset::new("echomind-default-eval-dataset".to_string(), samples).with_description(
        "EchoMind 内置 RAG 评估数据集（6 个中英文混合查询样本），\
             用于端到端检索质量评估（REQ-RAG-048）"
            .to_string(),
    )
}

// ============================================================
// 辅助函数
// ============================================================

/// 将检索结果转换为 chunk_id 列表。
#[allow(dead_code)]
fn extract_chunk_ids(results: &[RetrievalResult]) -> Vec<String> {
    results.iter().map(|r| r.chunk.id.clone()).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// 单元测试：load_eval_dataset 正常解析。
    #[test]
    fn test_load_eval_dataset_valid() {
        let json = r#"{"name":"test","samples":[{"query":"q","ground_truth":"a","relevant_chunk_ids":["c1"]}]}"#;
        let dataset = load_eval_dataset(json).unwrap();
        assert_eq!(dataset.name, "test");
        assert_eq!(dataset.samples.len(), 1);
    }

    /// 单元测试：load_eval_dataset 无效 JSON。
    #[test]
    fn test_load_eval_dataset_invalid() {
        let result = load_eval_dataset("invalid");
        assert!(result.is_err());
    }

    /// 单元测试：create_sample_dataset 包含 ≥ 5 个 sample。
    #[test]
    fn test_create_sample_dataset() {
        let dataset = create_sample_dataset();
        assert!(dataset.len() >= 5);
    }
}
