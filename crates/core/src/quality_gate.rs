//! RAG 质量门控系统（借鉴 StoryMoss 加权评分门控，REQ-RAG-028）。
//!
//! 检索后评估结果质量，低质量时触发降级策略：
//! - 检索覆盖率（top-1 分数归一化）：最高分 chunk 的分数是否足够高
//! - 来源多样性（不同文档占比）：检索结果是否来自多个文档
//! - 分数方差（top-k 分数标准差）：是否有明显优劣区分
//!
//! ## 加权评分公式
//!
//! `weighted = coverage * w_coverage + diversity * w_diversity + score_variance * w_score`
//!
//! 默认权重：coverage 0.4 + diversity 0.3 + score_variance 0.3
//! 默认阈值：0.6（低于此值时标记为低质量）
//!
//! ## 降级策略
//!
//! 当前版本仅做评估 + 日志记录（`PassThrough`）。
//! `ExpandTopK`（扩大 top-k 后重试）和 `WarnUser`（提示用户查询不够明确）
//! 由 `chat_inner` 在外部处理，因为需要重新检索或前端交互。

use echomind_models::RetrievalResult;

/// 质量门控配置。
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// 通过阈值（默认 0.6，范围 0.3-0.9）
    pub threshold: f32,
    /// 检索覆盖率权重（默认 0.4）
    pub weight_coverage: f32,
    /// 来源多样性权重（默认 0.3）
    pub weight_diversity: f32,
    /// 分数方差权重（默认 0.3）
    pub weight_score: f32,
    /// 降级策略
    pub degradation: DegradationStrategy,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            threshold: GATE_PASS_THRESHOLD,
            weight_coverage: 0.4,
            weight_diversity: 0.3,
            weight_score: 0.3,
            degradation: DegradationStrategy::PassThrough,
        }
    }
}

/// 降级策略。
#[derive(Debug, Clone)]
pub enum DegradationStrategy {
    /// 扩大 top_k 后重试（factor = 扩大倍数，max_retry = 最大重试次数）
    ExpandTopK {
        /// top_k 扩大倍数（如 2.0 = 扩大到 2 倍）
        factor: f32,
        /// 最大重试次数
        max_retry: usize,
    },
    /// 提示用户查询不够明确
    WarnUser,
    /// 直接放行（仅记录分数，默认行为）
    PassThrough,
}

/// 质量评分结果。
#[derive(Debug, Clone)]
pub struct GateScore {
    /// 检索覆盖率：top-1 分数归一化（0-1+，>1.0 表示高分）
    pub coverage: f32,
    /// 来源多样性：不同文档数 / 总结果数（0-1）
    pub diversity: f32,
    /// 分数方差：top-k 分数的标准差归一化（0-1+）
    pub score_variance: f32,
    /// 加权总分
    pub weighted: f32,
    /// 是否通过门控
    pub passed: bool,
}

/// 默认通过阈值。
pub const GATE_PASS_THRESHOLD: f32 = 0.6;

/// 分数归一化基准（top-1 score / NORMALIZATION_BASE）。
/// 余弦相似度 0.3 对应归一化 1.0，低于 0.3 为低质量。
const NORMALIZATION_BASE: f32 = 0.3;

/// 评估检索结果质量。
///
/// # 参数
/// - `results`: 检索结果列表（已按分数降序排列）
/// - `config`: 门控配置
///
/// # 返回
/// 质量评分，包含各维度分数和加权总分。
///
/// # 算法
///
/// - **coverage** = `results[0].score / 0.3`，clamp 到 `[0, 2.0]`，
///   使 0.3 分对应 1.0（满分），低于 0.3 为低覆盖率
/// - **diversity** = `unique_doc_count / results.len()`，1.0 = 每个结果来自不同文档
/// - **score_variance** = `stddev(scores)` 归一化到 `[0, 1+]`，
///   高方差 = 有明显优劣区分（好），低方差 = 无区分度（差）
/// - **weighted** = `coverage * w_c + diversity * w_d + score_variance * w_s`
/// - **passed** = `weighted >= threshold`
pub fn evaluate(results: &[RetrievalResult], config: &GateConfig) -> GateScore {
    if results.is_empty() {
        return GateScore {
            coverage: 0.0,
            diversity: 0.0,
            score_variance: 0.0,
            weighted: 0.0,
            passed: false,
        };
    }

    let coverage = compute_coverage(results);
    let diversity = compute_diversity(results);
    let score_variance = compute_score_variance(results);

    let weighted = coverage * config.weight_coverage
        + diversity * config.weight_diversity
        + score_variance * config.weight_score;

    let passed = weighted >= config.threshold;

    GateScore {
        coverage,
        diversity,
        score_variance,
        weighted,
        passed,
    }
}

/// 计算检索覆盖率：top-1 分数归一化。
///
/// `score / 0.3`，clamp 到 `[0, 2.0]`。
/// - score = 0.3 → coverage = 1.0（合格线）
/// - score = 0.6 → coverage = 2.0（满分+）
/// - score = 0.15 → coverage = 0.5（低质量）
fn compute_coverage(results: &[RetrievalResult]) -> f32 {
    let top_score = results[0].score;
    let normalized = top_score / NORMALIZATION_BASE;
    normalized.clamp(0.0, 2.0)
}

/// 计算来源多样性：不同文档数 / 总结果数。
///
/// - 3 个结果来自 3 个不同文档 → 1.0
/// - 3 个结果来自 1 个文档 → 0.333
fn compute_diversity(results: &[RetrievalResult]) -> f32 {
    let total = results.len() as f32;
    if total == 0.0 {
        return 0.0;
    }
    let unique_docs: std::collections::HashSet<&str> =
        results.iter().map(|r| r.doc_name.as_str()).collect();
    unique_docs.len() as f32 / total
}

/// 计算分数方差：top-k 分数的标准差。
///
/// 高标准差 = 有明显优劣区分（好），
/// 低标准差 = 无区分度（差）。
///
/// 归一化：`stddev` 本身在 `[0, 0.5]` 范围内（余弦相似度范围 `[0, 1]`），
/// 直接作为 `[0, 0.5+]` 使用，权重 0.3 使其在加权总分中贡献合理比例。
fn compute_score_variance(results: &[RetrievalResult]) -> f32 {
    let n = results.len() as f32;
    if n < 2.0 {
        return 0.0;
    }
    let scores: Vec<f32> = results.iter().map(|r| r.score).collect();
    let mean = scores.iter().sum::<f32>() / n;
    let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / n;
    variance.sqrt()
}
