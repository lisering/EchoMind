#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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

// ============================================================
// TC-GATE-001：高分检索结果通过门控
// results = [0.85, 0.80, 0.75]（同文档）→ coverage 高 → passed = true
// ============================================================
#[test]
fn tc_gate_001_high_score_passes() {
    let results = vec![
        make_result(0.85, "doc_a.md"),
        make_result(0.80, "doc_a.md"),
        make_result(0.75, "doc_a.md"),
    ];
    let config = GateConfig::default();
    let score = evaluate(&results, &config);
    assert!(score.passed, "高分结果应通过门控");
    // coverage = 0.85/0.3 ≈ 2.83, clamp 到 2.0
    assert!(
        (score.coverage - 2.0).abs() < 0.01,
        "coverage 应为 2.0（clamp 后），实际: {}",
        score.coverage
    );
}

// ============================================================
// TC-GATE-002：低分检索结果未通过门控
// results = [0.15, 0.12, 0.10] → coverage 低 → passed = false
// ============================================================
#[test]
fn tc_gate_002_low_score_fails() {
    let results = vec![
        make_result(0.15, "doc_a.md"),
        make_result(0.12, "doc_a.md"),
        make_result(0.10, "doc_a.md"),
    ];
    let config = GateConfig::default();
    let score = evaluate(&results, &config);
    assert!(!score.passed, "低分结果不应通过门控");
    // coverage = 0.15/0.3 = 0.5
    assert!(
        (score.coverage - 0.5).abs() < 0.01,
        "coverage 应为 0.5，实际: {}",
        score.coverage
    );
}

// ============================================================
// TC-GATE-003：多样性计算正确
// 3 个结果来自 3 个不同文档 → diversity = 1.0
// 3 个结果来自 1 个文档 → diversity = 1/3 ≈ 0.33
// ============================================================
#[test]
fn tc_gate_003_diversity_calculation() {
    // 3 个结果来自 3 个不同文档 → diversity = 1.0
    let results_diff = vec![
        make_result(0.8, "doc_a.md"),
        make_result(0.7, "doc_b.md"),
        make_result(0.6, "doc_c.md"),
    ];
    let config = GateConfig::default();
    let score = evaluate(&results_diff, &config);
    assert!(
        (score.diversity - 1.0).abs() < 0.01,
        "3 个不同文档的 diversity 应为 1.0，实际: {}",
        score.diversity
    );

    // 3 个结果来自 1 个文档 → diversity = 1/3 ≈ 0.333
    let results_same = vec![
        make_result(0.8, "doc_a.md"),
        make_result(0.7, "doc_a.md"),
        make_result(0.6, "doc_a.md"),
    ];
    let score = evaluate(&results_same, &config);
    assert!(
        (score.diversity - (1.0 / 3.0)).abs() < 0.01,
        "1 个文档的 diversity 应为 ≈ 0.333，实际: {}",
        score.diversity
    );
}

// ============================================================
// TC-GATE-004：分数方差计算正确
// [0.9, 0.1, 0.1] → 高方差（有明显优劣）
// [0.5, 0.5, 0.5] → 零方差（无明显区分）
// ============================================================
#[test]
fn tc_gate_004_score_variance_calculation() {
    // [0.9, 0.1, 0.1] → 高方差（有明显优劣）
    let results_high_var = vec![
        make_result(0.9, "doc_a.md"),
        make_result(0.1, "doc_b.md"),
        make_result(0.1, "doc_c.md"),
    ];
    let config = GateConfig::default();
    let score = evaluate(&results_high_var, &config);
    // 手动计算: mean=0.3667, variance = (0.5333^2 + 0.2667^2 + 0.2667^2)/3 ≈ 0.1322, stddev ≈ 0.3636
    assert!(
        score.score_variance > 0.3,
        "高方差场景的 score_variance 应 > 0.3，实际: {}",
        score.score_variance
    );

    // [0.5, 0.5, 0.5] → 零方差（无明显区分）
    let results_zero_var = vec![
        make_result(0.5, "doc_a.md"),
        make_result(0.5, "doc_b.md"),
        make_result(0.5, "doc_c.md"),
    ];
    let score = evaluate(&results_zero_var, &config);
    assert!(
        score.score_variance < 0.001,
        "零方差场景的 score_variance 应 ≈ 0.0，实际: {}",
        score.score_variance
    );
}

// ============================================================
// TC-GATE-005：加权总分公式验证
// 手动计算 coverage/diversity/variance → 验证 weighted 公式
// ============================================================
#[test]
fn tc_gate_005_weighted_formula() {
    let results = vec![make_result(0.6, "doc_a.md"), make_result(0.4, "doc_b.md")];
    let config = GateConfig {
        threshold: 0.6,
        weight_coverage: 0.4,
        weight_diversity: 0.3,
        weight_score: 0.3,
        degradation: DegradationStrategy::PassThrough,
    };
    let score = evaluate(&results, &config);

    // 手动计算
    let expected_coverage = (0.6_f32 / 0.3).clamp(0.0, 2.0); // 2.0
    let expected_diversity = 2.0_f32 / 2.0; // 1.0
    let scores = [0.6_f32, 0.4];
    let mean: f32 = 0.5;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / 2.0;
    let expected_stddev = variance.sqrt();
    let expected_weighted =
        expected_coverage * 0.4 + expected_diversity * 0.3 + expected_stddev * 0.3;

    assert!(
        (score.coverage - expected_coverage).abs() < 0.01,
        "coverage 不匹配: {} vs {}",
        score.coverage,
        expected_coverage
    );
    assert!(
        (score.diversity - expected_diversity).abs() < 0.01,
        "diversity 不匹配: {} vs {}",
        score.diversity,
        expected_diversity
    );
    assert!(
        (score.score_variance - expected_stddev).abs() < 0.01,
        "score_variance 不匹配: {} vs {}",
        score.score_variance,
        expected_stddev
    );
    assert!(
        (score.weighted - expected_weighted).abs() < 0.01,
        "weighted 不匹配: {} vs {}",
        score.weighted,
        expected_weighted
    );
}

// ============================================================
// TC-GATE-006：空结果返回全零 + passed = false
// ============================================================
#[test]
fn tc_gate_006_empty_results() {
    let results: Vec<RetrievalResult> = vec![];
    let config = GateConfig::default();
    let score = evaluate(&results, &config);
    assert!((score.coverage).abs() < 0.01, "空结果 coverage 应为 0.0");
    assert!((score.diversity).abs() < 0.01, "空结果 diversity 应为 0.0");
    assert!(
        (score.score_variance).abs() < 0.01,
        "空结果 score_variance 应为 0.0"
    );
    assert!((score.weighted).abs() < 0.01, "空结果 weighted 应为 0.0");
    assert!(!score.passed, "空结果 passed 应为 false");
}

// ============================================================
// TC-GATE-007：自定义阈值生效
// threshold = 0.9 → 原本通过的 0.7 分变为不通过
// ============================================================
#[test]
fn tc_gate_007_custom_threshold() {
    // results: [0.6, 0.5, 0.4]（同文档）
    // coverage = 0.6/0.3 = 2.0
    // diversity = 1/3 ≈ 0.333
    // scores = [0.6, 0.5, 0.4], mean=0.5, variance=(0.01+0+0.01)/3≈0.00667, stddev≈0.0816
    // weighted = 2.0*0.4 + 0.333*0.3 + 0.0816*0.3 = 0.8 + 0.1 + 0.0245 ≈ 0.924
    let results = vec![
        make_result(0.6, "doc_a.md"),
        make_result(0.5, "doc_a.md"),
        make_result(0.4, "doc_a.md"),
    ];

    // 默认阈值 0.6 → 应通过
    let config_default = GateConfig::default();
    let score_default = evaluate(&results, &config_default);
    assert!(
        score_default.passed,
        "默认阈值 0.6 下应通过，weighted={:.3}",
        score_default.weighted
    );

    // 高阈值 0.95 → 不应通过
    let config_high = GateConfig {
        threshold: 0.95,
        ..GateConfig::default()
    };
    let score_high = evaluate(&results, &config_high);
    assert!(
        !score_high.passed,
        "阈值 0.95 下不应通过，weighted={:.3}",
        score_high.weighted
    );
}
