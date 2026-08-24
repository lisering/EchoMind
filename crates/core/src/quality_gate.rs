//! RAG 质量门控系统（借鉴 StoryMoss 加权评分门控，REQ-RAG-028 + REQ-PERF-014）。
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
//!
//! ## 主动干预（REQ-PERF-014）
//!
//! 在原有评估+日志的基础上，新增主动干预能力：
//! - `should_retry()`：评估质量评分是否低于阈值，决定是否需要增强重试
//! - `build_enhanced_config()`：生成增强检索参数（扩大 top_k + 启用 HyDE + 启用 Rerank）
//! - `RetryDecision`：重试决策（是否重试 + 原因）
//! - `RetryOutcome`：重试结果（原始分数 → 重试分数 + 改善幅度）
//! - 最大重试次数 = 1（防无限重试 + 控制延迟）

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

// ============================================================================
// 主动干预模块（REQ-PERF-014）
// ============================================================================

/// 主动门控配置（REQ-PERF-014）。
///
/// 控制何时触发增强重试以及增强策略的参数。
#[derive(Debug, Clone)]
pub struct ActiveGateConfig {
    /// 触发重试的质量分数阈值（低于此值时触发）。
    /// 默认 0.6，与 `GATE_PASS_THRESHOLD` 一致。
    pub retry_threshold: f32,
    /// top_k 扩大倍数（如 3.0 = 扩大到 3 倍）。
    /// 默认 3.0，扩大检索范围以找到更多候选。
    pub enhanced_top_k_multiplier: f32,
    /// 重试时是否启用 HyDE 查询改写。
    /// 默认 true，通过假设性答案改写查询提升语义匹配。
    pub enable_hyde: bool,
    /// 重试时是否启用 Cross-Encoder 重排序。
    /// 默认 true，对扩大后的候选集精排。
    pub enable_rerank: bool,
    /// 最大重试次数（默认 1，防无限重试 + 控制延迟）。
    pub max_retries: usize,
}

impl Default for ActiveGateConfig {
    fn default() -> Self {
        Self {
            retry_threshold: GATE_PASS_THRESHOLD,
            enhanced_top_k_multiplier: 3.0,
            enable_hyde: true,
            enable_rerank: true,
            max_retries: 1,
        }
    }
}

/// 重试决策。
///
/// 评估质量评分后，决定是否需要增强重试。
#[derive(Debug, Clone)]
pub struct RetryDecision {
    /// 是否应该重试
    pub should_retry: bool,
    /// 决策原因（人类可读）
    pub reason: String,
}

/// 增强检索配置。
///
/// 由 `build_enhanced_config()` 生成，用于指导增强重试的检索参数。
#[derive(Debug, Clone)]
pub struct EnhancedRetrievalConfig {
    /// 增强后的 top_k（原始 top_k × multiplier）
    pub enhanced_top_k: usize,
    /// 是否启用 HyDE 查询改写
    pub enable_hyde: bool,
    /// 是否启用 Cross-Encoder 重排序
    pub enable_rerank: bool,
}

/// 重试结果。
///
/// 记录原始检索和增强重试的质量对比，用于日志和可观测性。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RetryOutcome {
    /// 原始检索的加权质量分数
    pub original_score: f32,
    /// 重试后的加权质量分数
    pub retry_score: f32,
    /// 质量改善幅度（retry_score - original_score）
    pub improvement: f32,
    /// 是否实际执行了重试
    pub retried: bool,
}

/// 判断是否应该触发增强重试。
///
/// 当质量评分未通过门控（`!score.passed`）且未超过最大重试次数时，返回 `should_retry = true`。
///
/// # 参数
/// - `score`: 质量评分结果（由 `evaluate()` 产生）
/// - `config`: 主动门控配置
///
/// # 返回
/// `RetryDecision` 包含 `should_retry` 布尔值和原因说明。
pub fn should_retry(score: &GateScore, config: &ActiveGateConfig) -> RetryDecision {
    should_retry_with_count(score, config, 0)
}

/// 判断是否应该触发增强重试（带已重试计数）。
///
/// 在 `should_retry()` 基础上增加 `current_retry` 参数，用于防止无限重试。
///
/// # 参数
/// - `score`: 质量评分结果
/// - `config`: 主动门控配置
/// - `current_retry`: 当前已重试次数（0 = 首次，1 = 已重试一次）
///
/// # 返回
/// `RetryDecision`：当 `current_retry >= max_retries` 时返回 `should_retry = false`。
pub fn should_retry_with_count(
    score: &GateScore,
    config: &ActiveGateConfig,
    current_retry: usize,
) -> RetryDecision {
    if current_retry >= config.max_retries {
        return RetryDecision {
            should_retry: false,
            reason: format!("已达最大重试次数 {}，不再重试", config.max_retries),
        };
    }

    if !score.passed {
        return RetryDecision {
            should_retry: true,
            reason: format!(
                "质量分数 {:.3} 低于阈值 {:.3}，触发增强重试",
                score.weighted, config.retry_threshold
            ),
        };
    }

    RetryDecision {
        should_retry: false,
        reason: format!("质量分数 {:.3} 通过门控", score.weighted),
    }
}

/// 构建增强检索配置。
///
/// 根据主动门控配置生成增强检索参数：扩大 top_k + 启用 HyDE + 启用 Rerank。
///
/// # 参数
/// - `config`: 主动门控配置
/// - `original_top_k`: 原始 top_k 值
///
/// # 返回
/// `EnhancedRetrievalConfig` 包含增强后的参数。
pub fn build_enhanced_config(
    config: &ActiveGateConfig,
    original_top_k: usize,
) -> EnhancedRetrievalConfig {
    let enhanced_top_k = (original_top_k as f32 * config.enhanced_top_k_multiplier) as usize;
    EnhancedRetrievalConfig {
        enhanced_top_k: enhanced_top_k.max(original_top_k),
        enable_hyde: config.enable_hyde,
        enable_rerank: config.enable_rerank,
    }
}

/// 构建重试结果。
///
/// 比较原始检索和增强重试的质量分数，计算改善幅度。
///
/// # 参数
/// - `original_score`: 原始检索的加权质量分数
/// - `retry_score`: 重试后的加权质量分数
/// - `retried`: 是否实际执行了重试
///
/// # 返回
/// `RetryOutcome` 包含对比信息。
pub fn build_retry_outcome(original_score: f32, retry_score: f32, retried: bool) -> RetryOutcome {
    RetryOutcome {
        original_score,
        retry_score,
        improvement: retry_score - original_score,
        retried,
    }
}
