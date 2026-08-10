//! RAG 评估指标系统 TDD 测试（REQ-RAG-045）。
//!
//! 测试覆盖：
//! - 纯 Rust 检索指标（Hit Rate / MRR / NDCG / Keyword Overlap / Context Similarity）
//! - LLM-as-Judge 生成指标（Faithfulness / Answer Relevance / Context Precision / Context Recall）
//! - 主评估器（RagEvaluator）集成测试
//! - 数据模型序列化

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use futures::StreamExt;
use futures::stream::BoxStream;

use echomind_models::{
    RagEvalMetric, RagEvalReport, RagEvalSample, RagEvalSettings, RagMetricType,
};

use crate::LLMProvider;
use crate::rag_eval::{
    self, RagEvaluator, context_embedding_similarity, hit_rate, keyword_overlap, mrr, ndcg,
};
use echomind_models::ChatMessage;

// ============================================================
// Mock LLM Provider
// ============================================================

/// 可配置 Mock LLM，模拟 `one_shot` 返回预设文本。
struct MockLlm {
    response: String,
    one_shot_called: AtomicBool,
}

impl MockLlm {
    fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
            one_shot_called: AtomicBool::new(false),
        }
    }

    fn was_called(&self) -> bool {
        self.one_shot_called.load(Ordering::SeqCst)
    }
}

impl LLMProvider for MockLlm {
    async fn chat_stream(
        &self,
        _system: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::empty().boxed())
    }

    async fn one_shot(&self, _system: &str, _prompt: &str) -> Result<Option<String>> {
        self.one_shot_called.store(true, Ordering::SeqCst);
        Ok(Some(self.response.clone()))
    }
}

/// 不支持 one_shot 的 Mock（返回 Ok(None)）。
struct NoOneShotMock;

impl LLMProvider for NoOneShotMock {
    async fn chat_stream(
        &self,
        _system: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::empty().boxed())
    }
}

/// one_shot 返回 Err 的 Mock。
struct FailingLlm;

impl LLMProvider for FailingLlm {
    async fn chat_stream(
        &self,
        _system: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::empty().boxed())
    }

    async fn one_shot(&self, _system: &str, _prompt: &str) -> Result<Option<String>> {
        Err(anyhow::anyhow!("LLM API 不可用"))
    }
}

// ============================================================
// 纯 Rust 检索指标测试
// ============================================================

/// TC-EVAL-001：hit_rate — 有相关文档时返回 1.0。
#[test]
fn tc_eval_001_hit_rate_with_relevant() {
    let relevance = vec![false, true, false, true];
    assert!((hit_rate(&relevance) - 1.0).abs() < 1e-6);
}

/// TC-EVAL-002：hit_rate — 无相关文档时返回 0.0。
#[test]
fn tc_eval_002_hit_rate_no_relevant() {
    let relevance = vec![false, false, false];
    assert!((hit_rate(&relevance) - 0.0).abs() < 1e-6);
}

/// TC-EVAL-003：hit_rate — 空数组返回 0.0。
#[test]
fn tc_eval_003_hit_rate_empty() {
    let relevance: Vec<bool> = vec![];
    assert!((hit_rate(&relevance) - 0.0).abs() < 1e-6);
}

/// TC-EVAL-004：mrr — 第一个结果相关时返回 1.0。
#[test]
fn tc_eval_004_mrr_first_relevant() {
    let relevance = vec![true, false, false];
    assert!((mrr(&relevance) - 1.0).abs() < 1e-6);
}

/// TC-EVAL-005：mrr — 第三个结果相关时返回 1/3。
#[test]
fn tc_eval_005_mrr_third_relevant() {
    let relevance = vec![false, false, true];
    assert!((mrr(&relevance) - 1.0 / 3.0).abs() < 1e-6);
}

/// TC-EVAL-006：mrr — 无相关文档时返回 0.0。
#[test]
fn tc_eval_006_mrr_no_relevant() {
    let relevance = vec![false, false, false];
    assert!((mrr(&relevance) - 0.0).abs() < 1e-6);
}

/// TC-EVAL-007：ndcg — 完美排序（降序）返回 1.0。
#[test]
fn tc_eval_007_ndcg_perfect_ordering() {
    let scores = vec![1.0, 0.8, 0.5, 0.2];
    let result = ndcg(&scores);
    assert!(
        (result - 1.0).abs() < 1e-6,
        "完美排序应返回 1.0，实际: {result}"
    );
}

/// TC-EVAL-008：ndcg — 空数组返回 0.0。
#[test]
fn tc_eval_008_ndcg_empty() {
    let scores: Vec<f32> = vec![];
    assert!((ndcg(&scores) - 0.0).abs() < 1e-6);
}

/// TC-EVAL-009：ndcg — 全零分数返回 0.0。
#[test]
fn tc_eval_009_ndcg_all_zero() {
    let scores = vec![0.0, 0.0, 0.0];
    assert!((ndcg(&scores) - 0.0).abs() < 1e-6);
}

/// TC-EVAL-010：ndcg — 逆序排序得分低于正序。
#[test]
fn tc_eval_010_ndcg_worse_ordering_lower() {
    let good_order = vec![1.0, 0.5, 0.0];
    let bad_order = vec![0.0, 0.5, 1.0];
    assert!(ndcg(&good_order) > ndcg(&bad_order), "正序 NDCG 应高于逆序");
}

/// TC-EVAL-011：keyword_overlap — 查询 token 全部在上下文中出现时返回 1.0。
#[test]
fn tc_eval_011_keyword_overlap_full_match() {
    let query = "Rust programming language";
    let contexts = vec![
        "Rust is a systems programming language focused on safety".to_string(),
        "The Rust programming language was created by Mozilla".to_string(),
    ];
    let result = keyword_overlap(query, &contexts);
    assert!(result > 0.99, "全部匹配应接近 1.0，实际: {result}");
}

/// TC-EVAL-012：keyword_overlap — 无匹配时返回 0.0。
#[test]
fn tc_eval_012_keyword_overlap_no_match() {
    let query = "Python JavaScript";
    let contexts = vec!["Rust is a systems language".to_string()];
    let result = keyword_overlap(query, &contexts);
    assert!(
        (result - 0.0).abs() < 1e-6,
        "无匹配应返回 0.0，实际: {result}"
    );
}

/// TC-EVAL-013：keyword_overlap — 空查询返回 0.0。
#[test]
fn tc_eval_013_keyword_overlap_empty_query() {
    let result = keyword_overlap("", &["context".to_string()]);
    assert!((result - 0.0).abs() < 1e-6);
}

/// TC-EVAL-014：keyword_overlap — 中文分词正常工作。
#[test]
fn tc_eval_014_keyword_overlap_chinese() {
    let query = "检索增强生成";
    let contexts = vec!["检索增强生成是RAG的核心技术".to_string()];
    let result = keyword_overlap(query, &contexts);
    assert!(result > 0.99, "中文全匹配应接近 1.0，实际: {result}");
}

/// TC-EVAL-015：context_embedding_similarity — 完全相同的向量返回 1.0。
#[test]
fn tc_eval_015_embedding_similarity_identical() {
    let query = vec![1.0, 0.0, 0.0];
    let contexts = vec![vec![1.0, 0.0, 0.0]];
    let result = context_embedding_similarity(&query, &contexts);
    assert!(
        (result - 1.0).abs() < 1e-5,
        "相同向量应返回 1.0，实际: {result}"
    );
}

/// TC-EVAL-016：context_embedding_similarity — 正交向量返回 0.5。
#[test]
fn tc_eval_016_embedding_similarity_orthogonal() {
    let query = vec![1.0, 0.0];
    let contexts = vec![vec![0.0, 1.0]];
    let result = context_embedding_similarity(&query, &contexts);
    // 余弦相似度 = 0，归一化 (0 + 1) / 2 = 0.5
    assert!(
        (result - 0.5).abs() < 1e-5,
        "正交向量应返回 0.5，实际: {result}"
    );
}

/// TC-EVAL-017：context_embedding_similarity — 空输入返回 0.0。
#[test]
fn tc_eval_017_embedding_similarity_empty() {
    let query: Vec<f32> = vec![];
    let contexts: Vec<Vec<f32>> = vec![];
    assert!((context_embedding_similarity(&query, &contexts) - 0.0).abs() < 1e-6);
}

// ============================================================
// LLM-as-Judge 生成指标测试
// ============================================================

/// TC-EVAL-018：faithfulness — LLM 返回 2/3 声明可推导 → 分数 2/3。
#[tokio::test]
async fn tc_eval_018_faithfulness_partial() {
    let llm = MockLlm::new("CLAIMS: 3\nSUPPORTED: 2\nREASON: Two claims supported.");
    let answer =
        "Rust is a systems language. It was created by Google. It has zero-cost abstractions.";
    let contexts =
        vec!["Rust is a systems programming language with zero-cost abstractions.".to_string()];

    let score = rag_eval::faithfulness(&llm, answer, &contexts)
        .await
        .unwrap();
    assert!(
        (score - 2.0 / 3.0).abs() < 1e-6,
        "2/3 声明可推导应返回 2/3，实际: {score}"
    );
    assert!(llm.was_called(), "one_shot 应被调用");
}

/// TC-EVAL-019：faithfulness — 全部声明可推导 → 1.0。
#[tokio::test]
async fn tc_eval_019_faithfulness_all_supported() {
    let llm = MockLlm::new("CLAIMS: 3\nSUPPORTED: 3\nREASON: All claims supported.");
    let answer = "Rust is safe and fast.";
    let contexts = vec!["Rust is a safe and fast programming language.".to_string()];

    let score = rag_eval::faithfulness(&llm, answer, &contexts)
        .await
        .unwrap();
    assert!((score - 1.0).abs() < 1e-6);
}

/// TC-EVAL-020：faithfulness — 空答案或空上下文 → 0.0。
#[tokio::test]
async fn tc_eval_020_faithfulness_empty_input() {
    let llm = MockLlm::new("CLAIMS: 0\nSUPPORTED: 0\nREASON: Empty.");

    let score = rag_eval::faithfulness(&llm, "", &["context".to_string()])
        .await
        .unwrap();
    assert!((score - 0.0).abs() < 1e-6);

    let score = rag_eval::faithfulness(&llm, "answer", &[]).await.unwrap();
    assert!((score - 0.0).abs() < 1e-6);
}

/// TC-EVAL-021：faithfulness — Provider 不支持 one_shot（返回 None）→ 0.0。
#[tokio::test]
async fn tc_eval_021_faithfulness_provider_no_one_shot() {
    let llm = NoOneShotMock;
    let score = rag_eval::faithfulness(&llm, "answer", &["context".to_string()])
        .await
        .unwrap();
    assert!((score - 0.0).abs() < 1e-6);
}

/// TC-EVAL-022：faithfulness — LLM 返回 Err → 向上传播错误。
#[tokio::test]
async fn tc_eval_022_faithfulness_llm_error() {
    let llm = FailingLlm;
    let result = rag_eval::faithfulness(&llm, "answer", &["context".to_string()]).await;
    assert!(result.is_err());
}

/// TC-EVAL-023：answer_relevance — LLM 评分 8/10 → 0.8。
#[tokio::test]
async fn tc_eval_023_answer_relevance_high_score() {
    let llm = MockLlm::new("SCORE: 8\nREASON: Mostly relevant.");
    let score = rag_eval::answer_relevance(
        &llm,
        "What is RAG?",
        "RAG is retrieval-augmented generation.",
    )
    .await
    .unwrap();
    assert!((score - 0.8).abs() < 1e-6);
}

/// TC-EVAL-024：answer_relevance — LLM 评分 0/10 → 0.0。
#[tokio::test]
async fn tc_eval_024_answer_relevance_zero_score() {
    let llm = MockLlm::new("SCORE: 0\nREASON: Completely irrelevant.");
    let score = rag_eval::answer_relevance(&llm, "What is Python?", "Rust is a systems language.")
        .await
        .unwrap();
    assert!((score - 0.0).abs() < 1e-6);
}

/// TC-EVAL-025：answer_relevance — 空输入 → 0.0。
#[tokio::test]
async fn tc_eval_025_answer_relevance_empty() {
    let llm = MockLlm::new("SCORE: 5\nREASON: N/A");
    let score = rag_eval::answer_relevance(&llm, "", "answer")
        .await
        .unwrap();
    assert!((score - 0.0).abs() < 1e-6);
}

/// TC-EVAL-026：context_precision — LLM 返回 [1, 0, 1] → 加权精度 > 0。
#[tokio::test]
async fn tc_eval_026_context_precision_partial() {
    let llm = MockLlm::new("[1, 0, 1]");
    let contexts = vec![
        "Rust is a systems language".to_string(),
        "Python is a scripting language".to_string(),
        "Rust has zero-cost abstractions".to_string(),
    ];
    let score = rag_eval::context_precision(&llm, "What is Rust?", &contexts)
        .await
        .unwrap();
    assert!(score > 0.0, "部分相关应返回 > 0，实际: {score}");
    assert!(score <= 1.0, "分数不应超过 1.0");
}

/// TC-EVAL-027：context_precision — 全部相关 → 1.0。
#[tokio::test]
async fn tc_eval_027_context_precision_all_relevant() {
    let llm = MockLlm::new("[1, 1, 1]");
    let contexts = vec![
        "Rust is fast".to_string(),
        "Rust is safe".to_string(),
        "Rust is concurrent".to_string(),
    ];
    let score = rag_eval::context_precision(&llm, "Tell me about Rust", &contexts)
        .await
        .unwrap();
    assert!(
        (score - 1.0).abs() < 1e-6,
        "全相关应返回 1.0，实际: {score}"
    );
}

/// TC-EVAL-028：context_precision — 空上下文 → 0.0。
#[tokio::test]
async fn tc_eval_028_context_precision_empty() {
    let llm = MockLlm::new("[]");
    let score = rag_eval::context_precision(&llm, "query", &[])
        .await
        .unwrap();
    assert!((score - 0.0).abs() < 1e-6);
}

/// TC-EVAL-029：context_recall — LLM 返回 3/4 声明可推导 → 0.75。
#[tokio::test]
async fn tc_eval_029_context_recall_partial() {
    let llm = MockLlm::new("STATEMENTS: 4\nCOVERED: 3\nREASON: 3 of 4 covered.");
    let gt = "Rust is safe. Rust is fast. Rust is concurrent. Rust was created by Apple.";
    let contexts = vec!["Rust is a safe, fast, concurrent programming language.".to_string()];
    let score = rag_eval::context_recall(&llm, gt, &contexts).await.unwrap();
    assert!(
        (score - 0.75).abs() < 1e-6,
        "3/4 声明可推导应返回 0.75，实际: {score}"
    );
}

/// TC-EVAL-030：context_recall — 无 ground truth → 0.0。
#[tokio::test]
async fn tc_eval_030_context_recall_empty_gt() {
    let llm = MockLlm::new("STATEMENTS: 0\nCOVERED: 0\nREASON: Empty GT.");
    let score = rag_eval::context_recall(&llm, "", &["context".to_string()])
        .await
        .unwrap();
    assert!((score - 0.0).abs() < 1e-6);
}

// ============================================================
// RagEvaluator 集成测试
// ============================================================

/// TC-EVAL-031：RagEvaluator 默认设置 — 纯 Rust 指标正常计算。
#[tokio::test]
async fn tc_eval_031_evaluator_rust_metrics() {
    let llm =
        MockLlm::new("SCORE: 8\nREASON: Good.\nCLAIMS: 2\nSUPPORTED: 2\nREASON: All.\n[1, 1]");
    let sample = RagEvalSample::new(
        "What is Rust?".to_string(),
        "Rust is a systems programming language.".to_string(),
        vec![
            "Rust is a systems programming language focused on safety.".to_string(),
            "Rust was created by Mozilla.".to_string(),
        ],
    )
    .with_relevant_indices(vec![0, 1])
    .with_relevance_scores(vec![0.9, 0.7]);

    let evaluator = RagEvaluator::new();
    let metrics = evaluator.evaluate(&llm, &sample).await.unwrap();

    let metric_names: Vec<&str> = metrics.iter().map(|m| m.metric_type.as_str()).collect();
    assert!(metric_names.contains(&"keyword_overlap"));
    assert!(metric_names.contains(&"hit_rate"));
    assert!(metric_names.contains(&"mrr"));
    assert!(metric_names.contains(&"ndcg"));
    assert!(metric_names.contains(&"faithfulness"));
    assert!(metric_names.contains(&"answer_relevance"));
    assert!(metric_names.contains(&"context_precision"));
}

/// TC-EVAL-032：RagEvaluator — LLM 失败时跳过 LLM 指标，保留 Rust 指标。
#[tokio::test]
async fn tc_eval_032_evaluator_llm_failure_graceful() {
    let llm = FailingLlm;
    let sample = RagEvalSample::new(
        "What is Rust?".to_string(),
        "Rust is a systems language.".to_string(),
        vec!["Rust is a systems programming language.".to_string()],
    )
    .with_relevant_indices(vec![0]);

    let evaluator = RagEvaluator::new();
    let metrics = evaluator.evaluate(&llm, &sample).await.unwrap();

    let metric_names: Vec<&str> = metrics.iter().map(|m| m.metric_type.as_str()).collect();
    assert!(metric_names.contains(&"hit_rate"), "HitRate 应保留");
    assert!(
        metric_names.contains(&"keyword_overlap"),
        "KeywordOverlap 应保留"
    );
    assert!(
        !metric_names.contains(&"faithfulness"),
        "Faithfulness 应被跳过"
    );
}

/// TC-EVAL-033：RagEvaluator — 禁用 LLM 指标后只计算 Rust 指标。
#[tokio::test]
async fn tc_eval_033_evaluator_rust_only() {
    let llm = MockLlm::new("SCORE: 5\nREASON: N/A");
    let sample = RagEvalSample::new(
        "Rust language".to_string(),
        "Rust is safe.".to_string(),
        vec!["Rust is a safe language.".to_string()],
    )
    .with_relevant_indices(vec![0])
    .with_relevance_scores(vec![0.9]);

    let settings = RagEvalSettings {
        enable_faithfulness: false,
        enable_answer_relevance: false,
        enable_context_precision: false,
        enable_context_recall: false,
        enable_retrieval_metrics: true,
        enable_embedding_metrics: false,
        enable_keyword_overlap: true,
    };
    let evaluator = RagEvaluator::with_settings(settings);
    let metrics = evaluator.evaluate(&llm, &sample).await.unwrap();

    for m in &metrics {
        assert!(
            !m.metric_type.needs_llm(),
            "不应包含 LLM 指标: {:?}",
            m.metric_type
        );
    }
    assert!(!llm.was_called(), "one_shot 不应被调用");
}

/// TC-EVAL-034：RagEvaluator — 批量评估返回正确报告。
#[tokio::test]
async fn tc_eval_034_evaluator_batch() {
    let llm =
        MockLlm::new("SCORE: 7\nREASON: Good.\nCLAIMS: 2\nSUPPORTED: 2\nREASON: All.\n[1, 1]");
    let samples = vec![
        RagEvalSample::new(
            "What is Rust?".to_string(),
            "Rust is a systems language.".to_string(),
            vec!["Rust is a systems programming language.".to_string()],
        ),
        RagEvalSample::new(
            "What is Python?".to_string(),
            "Python is a scripting language.".to_string(),
            vec!["Python is a high-level scripting language.".to_string()],
        ),
    ];

    let evaluator = RagEvaluator::new();
    let report = evaluator.evaluate_batch(&llm, &samples).await.unwrap();

    assert_eq!(report.sample_count, 2);
    assert_eq!(report.per_sample_metrics.len(), 2);
    assert!(!report.aggregate_metrics.is_empty(), "聚合指标不应为空");

    for m in &report.aggregate_metrics {
        assert!(
            m.score >= 0.0 && m.score <= 1.0,
            "聚合分数应在 [0,1]，实际: {}",
            m.score
        );
    }
}

/// TC-EVAL-035：RagEvaluator — 空样本列表返回空报告。
#[tokio::test]
async fn tc_eval_035_evaluator_empty_batch() {
    let llm = MockLlm::new("SCORE: 5\nREASON: N/A");
    let evaluator = RagEvaluator::new();
    let report = evaluator.evaluate_batch(&llm, &[]).await.unwrap();
    assert_eq!(report.sample_count, 0);
    assert!(report.aggregate_metrics.is_empty());
}

/// TC-EVAL-036：RagEvaluator — ContextRecall 需要 ground truth。
#[tokio::test]
async fn tc_eval_036_evaluator_context_recall_needs_gt() {
    let llm = MockLlm::new(
        "STATEMENTS: 2\nCOVERED: 2\nREASON: All covered.\nSCORE: 8\nREASON: Good.\nCLAIMS: 2\nSUPPORTED: 2\nREASON: All.\n[1]",
    );

    let sample_with_gt = RagEvalSample::new(
        "What is Rust?".to_string(),
        "Rust is a systems language.".to_string(),
        vec!["Rust is a systems programming language.".to_string()],
    )
    .with_ground_truth("Rust is a systems programming language.".to_string());

    let settings = RagEvalSettings {
        enable_faithfulness: false,
        enable_answer_relevance: false,
        enable_context_precision: false,
        enable_context_recall: true,
        enable_retrieval_metrics: false,
        enable_embedding_metrics: false,
        enable_keyword_overlap: false,
    };
    let evaluator = RagEvaluator::with_settings(settings);
    let metrics = evaluator.evaluate(&llm, &sample_with_gt).await.unwrap();

    let has_recall = metrics
        .iter()
        .any(|m| m.metric_type == RagMetricType::ContextRecall);
    assert!(has_recall, "有 GT 时应计算 ContextRecall");
}

// ============================================================
// 数据模型测试
// ============================================================

/// TC-EVAL-037：RagMetricType::as_str / parse_str 往返一致。
#[test]
fn tc_eval_037_metric_type_roundtrip() {
    let all_types = [
        RagMetricType::Faithfulness,
        RagMetricType::AnswerRelevance,
        RagMetricType::ContextPrecision,
        RagMetricType::ContextRecall,
        RagMetricType::HitRate,
        RagMetricType::MRR,
        RagMetricType::NDCG,
        RagMetricType::ContextSimilarity,
        RagMetricType::KeywordOverlap,
    ];
    for mt in &all_types {
        let s = mt.as_str();
        let back = RagMetricType::parse_str(s);
        assert_eq!(back.as_ref(), Some(mt), "往返不一致: {s}");
    }
    assert!(RagMetricType::parse_str("unknown").is_none());
}

/// TC-EVAL-038：RagMetricType::needs_llm / needs_ground_truth / needs_embedding。
#[test]
fn tc_eval_038_metric_type_flags() {
    assert!(RagMetricType::Faithfulness.needs_llm());
    assert!(RagMetricType::AnswerRelevance.needs_llm());
    assert!(RagMetricType::ContextPrecision.needs_llm());
    assert!(RagMetricType::ContextRecall.needs_llm());
    assert!(!RagMetricType::HitRate.needs_llm());
    assert!(!RagMetricType::NDCG.needs_llm());

    assert!(RagMetricType::ContextRecall.needs_ground_truth());
    assert!(!RagMetricType::Faithfulness.needs_ground_truth());

    assert!(RagMetricType::ContextSimilarity.needs_embedding());
    assert!(!RagMetricType::HitRate.needs_embedding());
}

/// TC-EVAL-039：RagEvalSample builder 链式调用。
#[test]
fn tc_eval_039_sample_builder() {
    let sample = RagEvalSample::new(
        "query".to_string(),
        "answer".to_string(),
        vec!["context".to_string()],
    )
    .with_ground_truth("truth".to_string())
    .with_relevance_scores(vec![0.9])
    .with_relevant_indices(vec![0]);

    assert_eq!(sample.query, "query");
    assert_eq!(sample.answer, "answer");
    assert_eq!(sample.contexts.len(), 1);
    assert_eq!(sample.ground_truth.as_deref(), Some("truth"));
    assert_eq!(sample.relevance_scores.as_deref(), Some(&[0.9][..]));
    assert_eq!(sample.relevant_indices.as_deref(), Some(&[0][..]));
}

/// TC-EVAL-040：RagEvalMetric 分数裁剪到 [0, 1]。
#[test]
fn tc_eval_040_metric_score_clamped() {
    let m1 = RagEvalMetric::new(RagMetricType::HitRate, 1.5);
    assert!((m1.score - 1.0).abs() < 1e-6);

    let m2 = RagEvalMetric::new(RagMetricType::HitRate, -0.5);
    assert!((m2.score - 0.0).abs() < 1e-6);

    let m3 = RagEvalMetric::new(RagMetricType::HitRate, 0.7);
    assert!((m3.score - 0.7).abs() < 1e-6);
}

/// TC-EVAL-041：RagEvalReport::from_samples 聚合平均值正确。
#[test]
fn tc_eval_041_report_aggregate() {
    let per_sample = vec![
        vec![
            RagEvalMetric::new(RagMetricType::Faithfulness, 0.8),
            RagEvalMetric::new(RagMetricType::HitRate, 1.0),
        ],
        vec![
            RagEvalMetric::new(RagMetricType::Faithfulness, 0.6),
            RagEvalMetric::new(RagMetricType::HitRate, 0.0),
        ],
    ];

    let report = RagEvalReport::from_samples(per_sample);
    assert_eq!(report.sample_count, 2);

    let faithfulness = report.get_metric(&RagMetricType::Faithfulness);
    assert!(faithfulness.is_some());
    assert!(
        (faithfulness.unwrap() - 0.7).abs() < 1e-6,
        "平均 Faithfulness 应为 0.7"
    );

    let hit_rate = report.get_metric(&RagMetricType::HitRate);
    assert!(hit_rate.is_some());
    assert!(
        (hit_rate.unwrap() - 0.5).abs() < 1e-6,
        "平均 HitRate 应为 0.5"
    );
}

/// TC-EVAL-042：RagEvalReport::empty 空报告。
#[test]
fn tc_eval_042_report_empty() {
    let report = RagEvalReport::empty();
    assert_eq!(report.sample_count, 0);
    assert!(report.aggregate_metrics.is_empty());
    assert!(report.per_sample_metrics.is_empty());
}

/// TC-EVAL-043：RagEvalSettings::default 默认值合理。
#[test]
fn tc_eval_043_settings_default() {
    let s = RagEvalSettings::default();
    assert!(s.enable_faithfulness);
    assert!(s.enable_answer_relevance);
    assert!(s.enable_context_precision);
    assert!(
        !s.enable_context_recall,
        "ContextRecall 默认关闭（需要 GT）"
    );
    assert!(s.enable_retrieval_metrics);
    assert!(
        !s.enable_embedding_metrics,
        "Embedding 默认关闭（需要向量）"
    );
    assert!(s.enable_keyword_overlap);
}

/// TC-EVAL-044：RagEvalMetric serde 序列化往返一致。
#[test]
fn tc_eval_044_metric_serde_roundtrip() {
    let metric = RagEvalMetric::with_details(
        RagMetricType::Faithfulness,
        0.85,
        "2/3 claims supported".to_string(),
    );
    let json = serde_json::to_string(&metric).unwrap();
    let back: RagEvalMetric = serde_json::from_str(&json).unwrap();
    assert_eq!(metric.metric_type, back.metric_type);
    assert!((metric.score - back.score).abs() < 1e-6);
    assert_eq!(metric.details, back.details);
}

/// TC-EVAL-045：RagEvalSample serde 序列化往返一致（含可选字段）。
#[test]
fn tc_eval_045_sample_serde_roundtrip() {
    let sample = RagEvalSample::new(
        "What is Rust?".to_string(),
        "Rust is a systems language.".to_string(),
        vec!["Rust is safe.".to_string(), "Rust is fast.".to_string()],
    )
    .with_ground_truth("Rust is a safe and fast systems language.".to_string())
    .with_relevance_scores(vec![0.9, 0.7])
    .with_relevant_indices(vec![0]);

    let json = serde_json::to_string(&sample).unwrap();
    let back: RagEvalSample = serde_json::from_str(&json).unwrap();
    assert_eq!(sample.query, back.query);
    assert_eq!(sample.answer, back.answer);
    assert_eq!(sample.contexts, back.contexts);
    assert_eq!(sample.ground_truth, back.ground_truth);
    assert_eq!(sample.relevance_scores, back.relevance_scores);
    assert_eq!(sample.relevant_indices, back.relevant_indices);
}
