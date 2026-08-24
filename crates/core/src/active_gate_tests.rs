#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! 检索质量门控主动干预 TDD 测试（REQ-PERF-014）。
//!
//! 测试策略：先写红灯测试（期望函数存在且行为正确），再实现。
//! 10 个测试覆盖：高质量不重试 / 低质量重试 / 空结果重试 / 增强配置 /
//! 最大重试次数 / serde roundtrip / 默认配置 / 阈值边界。

use crate::quality_gate::*;
use echomind_models::{Chunk, RetrievalResult};

/// 辅助函数：创建测试用 RetrievalResult
fn make_result(score: f32, doc_name: &str) -> RetrievalResult {
    RetrievalResult {
        chunk: Chunk::new("doc1".into(), "content".into(), 10, 0),
        score,
        doc_name: doc_name.into(),
    }
}

/// 辅助函数：创建高质量检索结果
fn high_quality_results() -> Vec<RetrievalResult> {
    vec![
        make_result(0.85, "doc_a.md"),
        make_result(0.80, "doc_b.md"),
        make_result(0.75, "doc_c.md"),
        make_result(0.70, "doc_d.md"),
    ]
}

/// 辅助函数：创建低质量检索结果
fn low_quality_results() -> Vec<RetrievalResult> {
    vec![
        make_result(0.15, "doc_a.md"),
        make_result(0.12, "doc_a.md"),
        make_result(0.10, "doc_a.md"),
    ]
}

// ============================================================
// TC-GATE-ACTIVE-001：高质量检索结果 → 不触发重试
// 高质量结果通过门控 → RetryDecision.should_retry = false
// ============================================================
#[test]
fn tc_gate_active_001_high_quality_no_retry() {
    let results = high_quality_results();
    let config = GateConfig::default();
    let score = evaluate(&results, &config);
    assert!(score.passed, "高质量结果应通过门控");

    let active_config = ActiveGateConfig::default();
    let decision = should_retry(&score, &active_config);
    assert!(
        !decision.should_retry,
        "高质量结果不应触发重试，weighted={:.3}",
        score.weighted
    );
}

// ============================================================
// TC-GATE-ACTIVE-002：低质量检索结果 → 触发重试
// 低质量结果未通过门控 → RetryDecision.should_retry = true
// ============================================================
#[test]
fn tc_gate_active_002_low_quality_triggers_retry() {
    let results = low_quality_results();
    let config = GateConfig::default();
    let score = evaluate(&results, &config);
    assert!(!score.passed, "低质量结果不应通过门控");

    let active_config = ActiveGateConfig::default();
    let decision = should_retry(&score, &active_config);
    assert!(
        decision.should_retry,
        "低质量结果应触发重试，weighted={:.3}",
        score.weighted
    );
    assert!(decision.reason.contains("质量"), "原因应包含质量信息");
}

// ============================================================
// TC-GATE-ACTIVE-003：空结果 → 触发重试
// ============================================================
#[test]
fn tc_gate_active_003_empty_results_triggers_retry() {
    let results: Vec<RetrievalResult> = vec![];
    let config = GateConfig::default();
    let score = evaluate(&results, &config);
    assert!(!score.passed);

    let active_config = ActiveGateConfig::default();
    let decision = should_retry(&score, &active_config);
    assert!(decision.should_retry, "空结果应触发重试");
}

// ============================================================
// TC-GATE-ACTIVE-004：单条低分结果 → 触发重试
// ============================================================
#[test]
fn tc_gate_active_004_single_low_score_triggers_retry() {
    let results = vec![make_result(0.10, "doc_a.md")];
    let config = GateConfig::default();
    let score = evaluate(&results, &config);

    let active_config = ActiveGateConfig::default();
    let decision = should_retry(&score, &active_config);
    assert!(decision.should_retry, "单条低分应触发重试");
}

// ============================================================
// TC-GATE-ACTIVE-005：所有分数低于阈值 → 触发重试
// ============================================================
#[test]
fn tc_gate_active_005_all_scores_below_threshold() {
    let results = vec![
        make_result(0.20, "doc_a.md"),
        make_result(0.18, "doc_b.md"),
        make_result(0.15, "doc_c.md"),
    ];
    let config = GateConfig::default();
    let score = evaluate(&results, &config);

    let active_config = ActiveGateConfig::default();
    let decision = should_retry(&score, &active_config);
    assert!(decision.should_retry, "全低分应触发重试");
}

// ============================================================
// TC-GATE-ACTIVE-006：增强配置扩大 top_k
// 默认 multiplier = 3 → top_k 5 → enhanced_top_k = 15
// ============================================================
#[test]
fn tc_gate_active_006_enhanced_config_multiplies_top_k() {
    let config = ActiveGateConfig::default();
    let enhanced = build_enhanced_config(&config, 5);
    assert_eq!(
        enhanced.enhanced_top_k, 15,
        "top_k 5 × 3 = 15，实际: {}",
        enhanced.enhanced_top_k
    );
}

// ============================================================
// TC-GATE-ACTIVE-007：增强配置启用 HyDE
// ============================================================
#[test]
fn tc_gate_active_007_enhanced_config_enables_hyde() {
    let config = ActiveGateConfig::default();
    let enhanced = build_enhanced_config(&config, 5);
    assert!(enhanced.enable_hyde, "增强配置应启用 HyDE 查询改写");
}

// ============================================================
// TC-GATE-ACTIVE-008：增强配置启用 Rerank
// ============================================================
#[test]
fn tc_gate_active_008_enhanced_config_enables_rerank() {
    let config = ActiveGateConfig::default();
    let enhanced = build_enhanced_config(&config, 5);
    assert!(
        enhanced.enable_rerank,
        "增强配置应启用 Cross-Encoder 重排序"
    );
}

// ============================================================
// TC-GATE-ACTIVE-009：RetryOutcome serde roundtrip
// ============================================================
#[test]
fn tc_gate_active_009_retry_outcome_serde() {
    let outcome = RetryOutcome {
        original_score: 0.45,
        retry_score: 0.72,
        improvement: 0.27,
        retried: true,
    };
    let json = serde_json::to_string(&outcome).unwrap();
    let decoded: RetryOutcome = serde_json::from_str(&json).unwrap();
    assert!((decoded.original_score - 0.45).abs() < 0.01);
    assert!((decoded.retry_score - 0.72).abs() < 0.01);
    assert!((decoded.improvement - 0.27).abs() < 0.01);
    assert!(decoded.retried);
}

// ============================================================
// TC-GATE-ACTIVE-010：最大重试次数 = 1（防无限重试）
// 第二次 should_retry 应返回 false（已超过 max_retries）
// ============================================================
#[test]
fn tc_gate_active_010_max_retries_prevents_infinite() {
    let results = low_quality_results();
    let config = GateConfig::default();
    let score = evaluate(&results, &config);

    let active_config = ActiveGateConfig::default();
    // 第一次重试：应触发
    let decision1 = should_retry(&score, &active_config);
    assert!(decision1.should_retry, "第一次应触发重试");

    // 模拟已重试 1 次（current_retry = 1）
    let decision2 = should_retry_with_count(&score, &active_config, 1);
    assert!(
        !decision2.should_retry,
        "第二次不应触发重试（已达 max_retries=1）"
    );
}
