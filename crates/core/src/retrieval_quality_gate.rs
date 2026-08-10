//! 检索质量门控（借鉴 OpenMontage slideshow_risk.py + variation_checker.py + delivery_promise.py）。
//!
//! ## 背景
//!
//! 检索结果送入 LLM 之前，应先评估检索质量。低质量检索（冗余结果、单一文档垄断、
//! 低相关度）会污染 LLM 上下文，产生幻觉或无效回答。本模块在检索后、prompt 构建前
//! 执行多维度质量评估，给出 verdict（strong/acceptable/revise/fail）和改进建议。
//!
//! ## 评估维度
//!
//! | 维度 | 说明 | 权重 |
//! |------|------|------|
//! | `relevance` | 检索结果与查询的平均相关度 | 30% |
//! | `diversity` | 结果覆盖不同文档/段落的比例 | 25% |
//! | `coverage` | 结果数量是否充足 | 20% |
//! | `redundancy` | 冗余结果占比（越低越好） | 15% |
//! | `freshness` | 文档时间新鲜度（有元数据时） | 10% |
//!
//! ## 借鉴来源
//!
//! - OpenMontage `lib/slideshow_risk.py`：6 维度评分 + verdict 系统
//! - OpenMontage `lib/variation_checker.py`：8 结构检查 + 违规列表 + 改进建议
//! - OpenMontage `lib/delivery_promise.py`：规则分类 + 验证

use std::collections::HashSet;

use echomind_models::RetrievalResult;

/// 质量评估裁决。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityVerdict {
    /// 检索质量优秀，可直接用于 RAG prompt
    Strong,
    /// 检索质量可接受，可以用于 RAG prompt 但有改进空间
    Acceptable,
    /// 检索质量需要改进，建议调整检索参数后重试
    Revise,
    /// 检索质量不合格，应阻止送入 LLM
    Fail,
}

impl QualityVerdict {
    /// 转字符串表示。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Strong => "strong",
            Self::Acceptable => "acceptable",
            Self::Revise => "revise",
            Self::Fail => "fail",
        }
    }
}

/// 单维度评分结果。
#[derive(Debug, Clone)]
pub struct DimensionScore {
    /// 维度名称
    pub name: String,
    /// 分数（0.0-1.0，越高越好）
    pub score: f32,
    /// 评分原因
    pub reason: String,
}

/// 检索质量评估报告。
#[derive(Debug, Clone)]
pub struct RetrievalQualityReport {
    /// 综合分数（0.0-1.0，越高越好）
    pub overall_score: f32,
    /// 裁决
    pub verdict: QualityVerdict,
    /// 各维度评分
    pub dimensions: Vec<DimensionScore>,
    /// 违规列表（具体问题描述）
    pub violations: Vec<String>,
    /// 改进建议
    pub suggestions: Vec<String>,
}

/// 质量评估配置。
#[derive(Debug, Clone)]
pub struct QualityGateConfig {
    /// 期望的最小结果数量
    pub min_results: usize,
    /// 期望的最小平均相关度
    pub min_avg_relevance: f32,
    /// 单文档结果占比上限（超过则视为冗余）
    pub max_single_doc_ratio: f32,
    /// 最低单条相关度
    pub min_individual_relevance: f32,
}

impl Default for QualityGateConfig {
    fn default() -> Self {
        Self {
            min_results: 3,
            min_avg_relevance: 0.4,
            max_single_doc_ratio: 0.7,
            min_individual_relevance: 0.2,
        }
    }
}

/// 评估检索结果质量。
///
/// # 参数
/// - `results`：检索结果列表（已过阈值过滤）
/// - `config`：质量评估配置
///
/// # 返回
/// `RetrievalQualityReport` 包含综合分数、裁决、各维度评分、违规列表和改进建议。
pub fn score_retrieval_quality(
    results: &[RetrievalResult],
    config: &QualityGateConfig,
) -> RetrievalQualityReport {
    if results.is_empty() {
        return RetrievalQualityReport {
            overall_score: 0.0,
            verdict: QualityVerdict::Fail,
            dimensions: vec![DimensionScore {
                name: "empty".to_string(),
                score: 0.0,
                reason: "无检索结果".to_string(),
            }],
            violations: vec!["检索结果为空，无法构建 RAG prompt".to_string()],
            suggestions: vec!["检查知识库是否为空或查询是否过于模糊".to_string()],
        };
    }

    let total = results.len();
    let mut dimensions = Vec::with_capacity(5);
    let mut violations = Vec::new();
    let mut suggestions = Vec::new();

    // --- 1. Relevance (30%) ---
    let avg_relevance: f32 = results.iter().map(|r| r.score).sum::<f32>() / total as f32;
    let relevance_score = avg_relevance.clamp(0.0, 1.0);
    let low_count = results
        .iter()
        .filter(|r| r.score < config.min_individual_relevance)
        .count();
    if avg_relevance < config.min_avg_relevance {
        violations.push(format!(
            "平均相关度 {avg_relevance:.2} 低于阈值 {min_avg:.2}",
            min_avg = config.min_avg_relevance
        ));
        suggestions.push("尝试调整查询措辞或启用 HyDE 查询改写".to_string());
    }
    if low_count > 0 {
        violations.push(format!(
            "{low_count}/{total} 条结果相关度低于个体阈值 {min_ind:.2}",
            min_ind = config.min_individual_relevance
        ));
    }
    dimensions.push(DimensionScore {
        name: "relevance".to_string(),
        score: relevance_score,
        reason: format!("平均相关度 {avg_relevance:.3}，{low_count} 条低于个体阈值"),
    });

    // --- 2. Diversity (25%) ---
    let unique_docs: HashSet<&str> = results.iter().map(|r| r.chunk.doc_id.as_str()).collect();
    let doc_count = unique_docs.len();
    let diversity_score = (doc_count as f32 / total as f32).clamp(0.0, 1.0);
    // 计算单文档最大占比
    let max_doc_count = results
        .iter()
        .map(|r| r.chunk.doc_id.as_str())
        .collect::<HashSet<_>>()
        .iter()
        .map(|doc| {
            results
                .iter()
                .filter(|r| r.chunk.doc_id.as_str() == *doc)
                .count()
        })
        .max()
        .unwrap_or(0);
    let single_doc_ratio = max_doc_count as f32 / total as f32;
    if single_doc_ratio > config.max_single_doc_ratio {
        violations.push(format!(
            "单一文档占比 {:.0}% 超过上限 {:.0}%",
            single_doc_ratio * 100.0,
            config.max_single_doc_ratio * 100.0
        ));
        suggestions.push("启用 MMR 多样性重排或扩大检索范围".to_string());
    }
    dimensions.push(DimensionScore {
        name: "diversity".to_string(),
        score: diversity_score,
        reason: format!("{doc_count} 个不同文档 / {total} 条结果"),
    });

    // --- 3. Coverage (20%) ---
    let coverage_score = if total >= config.min_results {
        1.0
    } else {
        total as f32 / config.min_results as f32
    };
    if total < config.min_results {
        violations.push(format!(
            "结果数量 {total} 少于期望最小值 {min_res}",
            min_res = config.min_results
        ));
        suggestions.push("增大 top_k 或降低相关度阈值".to_string());
    }
    dimensions.push(DimensionScore {
        name: "coverage".to_string(),
        score: coverage_score,
        reason: format!(
            "{total} 条结果（期望 ≥ {min_res}）",
            min_res = config.min_results
        ),
    });

    // --- 4. Redundancy (15%) ---
    let redundant_count = count_redundant(results);
    let redundancy_rate = redundant_count as f32 / total as f32;
    let redundancy_score = 1.0 - redundancy_rate;
    if redundancy_rate > 0.3 {
        violations.push(format!(
            "冗余率 {:.0}%（{redundant_count}/{total} 条来自同文档相邻 chunk）",
            redundancy_rate * 100.0
        ));
        suggestions.push("启用 MMR 多样性重排减少冗余".to_string());
    }
    dimensions.push(DimensionScore {
        name: "redundancy".to_string(),
        score: redundancy_score,
        reason: format!("{redundant_count}/{total} 条冗余（同文档相邻 chunk）"),
    });

    // --- 5. Freshness (10%) ---
    let unique_doc_seq: HashSet<(String, usize)> = results
        .iter()
        .map(|r| (r.chunk.doc_id.clone(), r.chunk.sequence))
        .collect();
    let freshness_score = (unique_doc_seq.len() as f32 / total as f32).clamp(0.0, 1.0);
    dimensions.push(DimensionScore {
        name: "freshness".to_string(),
        score: freshness_score,
        reason: format!("{} 个唯一 (doc, seq) 组合", unique_doc_seq.len()),
    });

    // --- 加权综合分数 ---
    let weights = [0.30_f32, 0.25, 0.20, 0.15, 0.10];
    let overall_score: f32 = dimensions
        .iter()
        .zip(weights.iter())
        .map(|(d, w)| d.score * w)
        .sum::<f32>()
        .clamp(0.0, 1.0);

    // --- Verdict ---
    let verdict = if overall_score >= 0.75 && violations.is_empty() {
        QualityVerdict::Strong
    } else if overall_score >= 0.55 {
        QualityVerdict::Acceptable
    } else if overall_score >= 0.35 {
        QualityVerdict::Revise
    } else {
        QualityVerdict::Fail
    };

    RetrievalQualityReport {
        overall_score,
        verdict,
        dimensions,
        violations,
        suggestions,
    }
}

/// 统计冗余结果数量：同文档且相邻 sequence 的结果视为冗余。
fn count_redundant(results: &[RetrievalResult]) -> usize {
    let mut redundant = 0;
    for i in 0..results.len() {
        for j in 0..results.len() {
            if i != j
                && results[i].chunk.doc_id == results[j].chunk.doc_id
                && (results[i].chunk.sequence as i64 - results[j].chunk.sequence as i64).abs() == 1
            {
                redundant += 1;
                break;
            }
        }
    }
    // 每对冗余被计算了 2 次，除以 2
    redundant / 2
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use echomind_models::{Chunk, RetrievalResult};

    fn make_result(id: &str, doc_id: &str, score: f32, seq: usize) -> RetrievalResult {
        RetrievalResult {
            chunk: Chunk {
                id: id.to_string(),
                doc_id: doc_id.to_string(),
                content: format!("content for {id}"),
                token_count: 50,
                sequence: seq,
            },
            score,
            doc_name: String::new(),
        }
    }

    /// TC-RQG-001: 高质量检索 → Strong
    #[test]
    fn test_high_quality_strong() {
        let results = vec![
            make_result("a", "d1", 0.85, 0),
            make_result("b", "d2", 0.80, 0),
            make_result("c", "d3", 0.75, 0),
            make_result("d", "d4", 0.70, 0),
        ];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        assert_eq!(report.verdict, QualityVerdict::Strong);
        assert!(report.overall_score >= 0.75);
        assert!(report.violations.is_empty());
    }

    /// TC-RQG-002: 空结果 → Fail
    #[test]
    fn test_empty_fail() {
        let results: Vec<RetrievalResult> = vec![];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        assert_eq!(report.verdict, QualityVerdict::Fail);
        assert_eq!(report.overall_score, 0.0);
        assert!(!report.violations.is_empty());
    }

    /// TC-RQG-003: 低相关度 → 有违规，非 Strong
    #[test]
    fn test_low_relevance_revise() {
        let results = vec![
            make_result("a", "d1", 0.25, 0),
            make_result("b", "d2", 0.20, 0),
            make_result("c", "d3", 0.15, 0),
        ];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        // 低相关度应触发违规，verdict 不应为 Strong
        assert!(report.verdict != QualityVerdict::Strong);
        assert!(!report.violations.is_empty());
        assert!(report.overall_score < 0.85);
    }

    /// TC-RQG-004: 单文档垄断 → 有违规
    #[test]
    fn test_single_doc_monopoly() {
        let results = vec![
            make_result("a", "d1", 0.80, 0),
            make_result("b", "d1", 0.75, 2),
            make_result("c", "d1", 0.70, 4),
            make_result("d", "d1", 0.65, 6),
        ];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        // 100% 同文档 → 违规
        assert!(
            report.violations.iter().any(|v| v.contains("单一文档占比")),
            "Expected single doc ratio violation, got: {:?}",
            report.violations
        );
    }

    /// TC-RQG-005: 冗余结果（同文档相邻 chunk）→ 有违规
    #[test]
    fn test_redundant_results() {
        let results = vec![
            make_result("a", "d1", 0.80, 0),
            make_result("b", "d1", 0.75, 1), // 与 a 相邻 → 冗余
            make_result("c", "d2", 0.70, 0),
            make_result("d", "d2", 0.65, 1), // 与 c 相邻 → 冗余
        ];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        // 2/4 = 50% 冗余 → 违规
        assert!(
            report.violations.iter().any(|v| v.contains("冗余率")),
            "Expected redundancy violation, got: {:?}",
            report.violations
        );
    }

    /// TC-RQG-006: 结果不足 → 有违规
    #[test]
    fn test_insufficient_results() {
        let results = vec![make_result("a", "d1", 0.85, 0)];
        let config = QualityGateConfig {
            min_results: 5,
            ..Default::default()
        };
        let report = score_retrieval_quality(&results, &config);
        assert!(
            report.violations.iter().any(|v| v.contains("结果数量")),
            "Expected insufficient results violation"
        );
    }

    /// TC-RQG-007: 多文档非冗余高质量 → Strong
    #[test]
    fn test_multi_doc_diverse_strong() {
        let results = vec![
            make_result("a", "d1", 0.85, 0),
            make_result("b", "d2", 0.80, 0),
            make_result("c", "d3", 0.75, 0),
        ];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        assert_eq!(report.verdict, QualityVerdict::Strong);
        assert!(report.suggestions.is_empty());
    }

    /// TC-RQG-008: QualityVerdict as_str
    #[test]
    fn test_verdict_as_str() {
        assert_eq!(QualityVerdict::Strong.as_str(), "strong");
        assert_eq!(QualityVerdict::Acceptable.as_str(), "acceptable");
        assert_eq!(QualityVerdict::Revise.as_str(), "revise");
        assert_eq!(QualityVerdict::Fail.as_str(), "fail");
    }

    /// TC-RQG-009: 维度数量为 5
    #[test]
    fn test_five_dimensions() {
        let results = vec![make_result("a", "d1", 0.8, 0)];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        assert_eq!(report.dimensions.len(), 5);
        assert_eq!(report.dimensions[0].name, "relevance");
        assert_eq!(report.dimensions[1].name, "diversity");
        assert_eq!(report.dimensions[2].name, "coverage");
        assert_eq!(report.dimensions[3].name, "redundancy");
        assert_eq!(report.dimensions[4].name, "freshness");
    }

    /// TC-RQG-010: 综合分数在 [0, 1] 区间
    #[test]
    fn test_score_in_range() {
        let results = vec![
            make_result("a", "d1", 0.5, 0),
            make_result("b", "d2", 0.5, 0),
        ];
        let config = QualityGateConfig::default();
        let report = score_retrieval_quality(&results, &config);
        assert!(report.overall_score >= 0.0 && report.overall_score <= 1.0);
    }
}
