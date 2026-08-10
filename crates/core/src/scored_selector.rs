//! LLM Provider 评分选择引擎（借鉴 OpenMontage lib/scoring.py）。
//!
//! ## 背景
//!
//! 当用户配置了多个 LLM 后端（如 OpenAI、Anthropic、本地 Ollama），当前 `LlmRouter`
//! 仅做简单的模式路由（Remote vs Local）。本模块引入多维度加权评分，自动选择
//! 最适合当前任务的 Provider，选择过程可解释、可审计。
//!
//! ## 评估维度
//!
//! | 维度 | 权重 | 说明 |
//! |------|------|------|
//! | `task_fit` | 30% | 任务匹配度（代码/对话/长文本/多语言） |
//! | `output_quality` | 20% | 输出质量（模型能力等级） |
//! | `cost_efficiency` | 15% | 成本效率（token 价格） |
//! | `latency` | 15% | 延迟（首 token / 完成时间） |
//! | `reliability` | 10% | 可靠性（历史成功率） |
//! | `context_window` | 5% | 上下文窗口大小 |
//! | `continuity` | 5% | 连续性（与已选 Provider 一致） |
//!
//! ## 借鉴来源
//!
//! OpenMontage `lib/scoring.py` → `ProviderScore` + `score_provider()` + `rank_providers()`

use std::collections::HashSet;

/// LLM 任务类型（影响 task_fit 评分）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TaskType {
    /// 通用对话
    Chat,
    /// 代码生成 / 代码问答
    Code,
    /// 长文档摘要 / 长上下文 RAG
    LongContext,
    /// 多语言翻译
    Multilingual,
    /// 推理 / 数学 / 逻辑
    Reasoning,
    /// Agent 多步推理
    Agentic,
}

impl TaskType {
    /// 从查询文本推断任务类型。
    pub fn infer(query: &str) -> Self {
        let lower = query.to_lowercase();
        if lower.contains("代码") || lower.contains("code") || lower.contains("函数") {
            Self::Code
        } else if lower.contains("翻译") || lower.contains("translate") {
            Self::Multilingual
        } else if lower.contains("推理") || lower.contains("证明") || lower.contains("数学") {
            Self::Reasoning
        } else if lower.contains("总结") || lower.contains("摘要") || lower.contains("summarize")
        {
            Self::LongContext
        } else if lower.contains("计划") || lower.contains("步骤") || lower.contains("plan") {
            Self::Agentic
        } else {
            Self::Chat
        }
    }
}

/// LLM Provider 候选配置。
#[derive(Debug, Clone)]
pub struct ProviderCandidate {
    /// Provider 名称（如 "openai", "anthropic", "ollama"）
    pub name: String,
    /// 模型名称（如 "gpt-4o", "claude-3.5-sonnet", "qwen2.5:7b"）
    pub model: String,
    /// 是否本地运行
    pub is_local: bool,
    /// 上下文窗口大小（token 数）
    pub context_window: usize,
    /// 输入 token 价格（美元 / 1M token，0 = 免费）
    pub price_input_per_m: f32,
    /// 输出 token 价格（美元 / 1M token，0 = 免费）
    pub price_output_per_m: f32,
    /// 模型能力等级（0.0-1.0，越高越强）
    pub quality_tier: f32,
    /// 历史成功率（0.0-1.0，None = 无数据）
    pub historical_success_rate: Option<f32>,
    /// 历史 P50 延迟（秒，None = 无数据）
    pub latency_p50_seconds: Option<f32>,
    /// 支持的任务类型
    pub supported_tasks: HashSet<TaskType>,
}

/// Provider 评分结果。
#[derive(Debug, Clone)]
pub struct ProviderScore {
    /// Provider 名称
    pub name: String,
    /// 模型名称
    pub model: String,
    /// task_fit 分数（0-1）
    pub task_fit: f32,
    /// output_quality 分数（0-1）
    pub output_quality: f32,
    /// cost_efficiency 分数（0-1）
    pub cost_efficiency: f32,
    /// latency 分数（0-1）
    pub latency: f32,
    /// reliability 分数（0-1）
    pub reliability: f32,
    /// context_window 分数（0-1）
    pub context_window: f32,
    /// continuity 分数（0-1）
    pub continuity: f32,
    /// 加权总分
    pub weighted_score: f32,
}

impl ProviderScore {
    /// 生成人类可读的评分说明。
    pub fn explain(&self) -> String {
        format!(
            "{name} ({model}): {score:.3}\n  task_fit={tf:.2} quality={q:.2} cost={c:.2} \
             latency={l:.2} reliability={r:.2} ctx={cw:.2} continuity={ct:.2}",
            name = self.name,
            model = self.model,
            score = self.weighted_score,
            tf = self.task_fit,
            q = self.output_quality,
            c = self.cost_efficiency,
            l = self.latency,
            r = self.reliability,
            cw = self.context_window,
            ct = self.continuity,
        )
    }
}

/// 评分配置。
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// 当前任务类型
    pub task_type: TaskType,
    /// 估算的输入 token 数
    pub estimated_input_tokens: usize,
    /// 估算的输出 token 数
    pub estimated_output_tokens: usize,
    /// 已选 Provider 名称（用于 continuity 评分）
    pub locked_providers: HashSet<String>,
}

impl ScoringConfig {
    /// 为指定任务创建评分配置。
    pub fn for_task(task_type: TaskType) -> Self {
        Self {
            task_type,
            estimated_input_tokens: 2000,
            estimated_output_tokens: 1000,
            locked_providers: HashSet::new(),
        }
    }

    /// 从查询文本推断任务类型并创建配置。
    pub fn from_query(query: &str) -> Self {
        Self::for_task(TaskType::infer(query))
    }
}

/// 估算单次调用的美元成本。
fn estimate_cost(candidate: &ProviderCandidate, config: &ScoringConfig) -> f32 {
    let input_cost =
        candidate.price_input_per_m * config.estimated_input_tokens as f32 / 1_000_000.0;
    let output_cost =
        candidate.price_output_per_m * config.estimated_output_tokens as f32 / 1_000_000.0;
    input_cost + output_cost
}

/// 计算成本效率分数（0-1，越高越好）。
fn score_cost_efficiency(cost: f32) -> f32 {
    if cost <= 0.0 {
        return 1.0; // 免费
    }
    if cost < 0.001 {
        0.95
    } else if cost < 0.01 {
        0.8
    } else if cost < 0.05 {
        0.6
    } else if cost < 0.15 {
        0.4
    } else {
        0.2
    }
}

/// 计算延迟分数（0-1，越高越好）。
fn score_latency(candidate: &ProviderCandidate) -> f32 {
    if let Some(p50) = candidate.latency_p50_seconds {
        if p50 <= 1.0 {
            1.0
        } else if p50 <= 5.0 {
            0.8
        } else if p50 <= 15.0 {
            0.6
        } else if p50 <= 30.0 {
            0.4
        } else {
            0.2
        }
    } else {
        // 无历史数据：本地快，远程慢
        if candidate.is_local { 0.85 } else { 0.5 }
    }
}

/// 计算可靠性分数（0-1，越高越好）。
fn score_reliability(candidate: &ProviderCandidate) -> f32 {
    if let Some(rate) = candidate.historical_success_rate {
        rate
    } else if candidate.is_local {
        0.85 // 本地无网络依赖
    } else {
        0.75 // 远程有网络波动
    }
}

/// 计算上下文窗口分数（0-1，越高越好）。
fn score_context_window(candidate: &ProviderCandidate, config: &ScoringConfig) -> f32 {
    let needed = config.estimated_input_tokens + config.estimated_output_tokens;
    if candidate.context_window == 0 {
        return 0.3; // 未知
    }
    if candidate.context_window >= needed * 2 {
        1.0 // 充裕
    } else if candidate.context_window >= needed {
        0.8 // 够用
    } else if candidate.context_window >= needed / 2 {
        0.5 // 紧张
    } else {
        0.2 // 不足
    }
}

/// 计算连续性分数（0-1，越高越好）。
fn score_continuity(candidate: &ProviderCandidate, config: &ScoringConfig) -> f32 {
    if config.locked_providers.is_empty() {
        0.5 // 无历史
    } else if config.locked_providers.contains(&candidate.name) {
        0.9 // 与已选一致
    } else {
        0.4 // 不一致
    }
}

/// 对单个 Provider 评分。
pub fn score_provider(candidate: &ProviderCandidate, config: &ScoringConfig) -> ProviderScore {
    // task_fit: 是否支持当前任务类型
    let task_fit = if candidate.supported_tasks.contains(&config.task_type) {
        1.0
    } else {
        0.3 // 不直接支持但可能勉强可用
    };

    // output_quality: 来自 quality_tier
    let output_quality = candidate.quality_tier.clamp(0.0, 1.0);

    // cost_efficiency
    let cost = estimate_cost(candidate, config);
    let cost_efficiency = score_cost_efficiency(cost);

    // latency
    let latency = score_latency(candidate);

    // reliability
    let reliability = score_reliability(candidate);

    // context_window
    let context_window = score_context_window(candidate, config);

    // continuity
    let continuity = score_continuity(candidate, config);

    // 加权总分
    let weighted_score = task_fit * 0.30
        + output_quality * 0.20
        + cost_efficiency * 0.15
        + latency * 0.15
        + reliability * 0.10
        + context_window * 0.05
        + continuity * 0.05;

    ProviderScore {
        name: candidate.name.clone(),
        model: candidate.model.clone(),
        task_fit,
        output_quality,
        cost_efficiency,
        latency,
        reliability,
        context_window,
        continuity,
        weighted_score,
    }
}

/// 对多个 Provider 排序，返回按分数降序排列的评分列表。
pub fn rank_providers(
    candidates: &[ProviderCandidate],
    config: &ScoringConfig,
) -> Vec<ProviderScore> {
    let mut scores: Vec<ProviderScore> = candidates
        .iter()
        .map(|c| score_provider(c, config))
        .collect();
    scores.sort_by(|a, b| {
        b.weighted_score
            .partial_cmp(&a.weighted_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scores
}

/// 选择最佳 Provider，返回评分最高的候选索引和评分。
///
/// 返回 `None` 当候选列表为空。
pub fn select_best(
    candidates: &[ProviderCandidate],
    config: &ScoringConfig,
) -> Option<(usize, ProviderScore)> {
    let ranked = rank_providers(candidates, config);
    ranked.into_iter().next().and_then(|score| {
        candidates
            .iter()
            .position(|c| c.name == score.name)
            .map(|idx| (idx, score))
    })
}

/// 格式化排名列表用于展示。
pub fn format_ranking(rankings: &[ProviderScore], top_n: usize) -> String {
    let lines: Vec<String> = rankings
        .iter()
        .take(top_n)
        .enumerate()
        .map(|(i, r)| {
            format!(
                "  {}. {} ({}) — score: {:.3} [fit={:.2} quality={:.2} cost={:.2} latency={:.2}]",
                i + 1,
                r.name,
                r.model,
                r.weighted_score,
                r.task_fit,
                r.output_quality,
                r.cost_efficiency,
                r.latency,
            )
        })
        .collect();
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn make_candidate(
        name: &str,
        model: &str,
        is_local: bool,
        ctx: usize,
        price_in: f32,
        price_out: f32,
        quality: f32,
        tasks: &[TaskType],
    ) -> ProviderCandidate {
        ProviderCandidate {
            name: name.to_string(),
            model: model.to_string(),
            is_local,
            context_window: ctx,
            price_input_per_m: price_in,
            price_output_per_m: price_out,
            quality_tier: quality,
            historical_success_rate: None,
            latency_p50_seconds: None,
            supported_tasks: tasks.iter().cloned().collect(),
        }
    }

    /// TC-SPS-001: 任务匹配的 Provider 得分高于不匹配的
    #[test]
    fn test_task_fit_dominates() {
        let candidates = vec![
            make_candidate(
                "openai",
                "gpt-4o",
                false,
                128_000,
                5.0,
                15.0,
                0.9,
                &[TaskType::Code, TaskType::Chat],
            ),
            make_candidate(
                "local",
                "qwen",
                true,
                32_000,
                0.0,
                0.0,
                0.6,
                &[TaskType::Chat],
            ),
        ];
        let config = ScoringConfig::for_task(TaskType::Code);
        let ranked = rank_providers(&candidates, &config);
        // openai 支持 Code → task_fit=1.0；local 不支持 → task_fit=0.3
        assert_eq!(ranked[0].name, "openai");
    }

    /// TC-SPS-002: 免费 Provider 成本效率满分
    #[test]
    fn test_free_provider_cost_perfect() {
        let candidate = make_candidate(
            "local",
            "qwen",
            true,
            32_000,
            0.0,
            0.0,
            0.6,
            &[TaskType::Chat],
        );
        let config = ScoringConfig::for_task(TaskType::Chat);
        let score = score_provider(&candidate, &config);
        assert!((score.cost_efficiency - 1.0).abs() < 0.001);
    }

    /// TC-SPS-003: 本地 Provider 延迟高于远程
    #[test]
    fn test_local_latency_higher() {
        let local = make_candidate(
            "local",
            "qwen",
            true,
            32_000,
            0.0,
            0.0,
            0.6,
            &[TaskType::Chat],
        );
        let remote = make_candidate(
            "openai",
            "gpt-4o",
            false,
            128_000,
            5.0,
            15.0,
            0.9,
            &[TaskType::Chat],
        );
        let config = ScoringConfig::for_task(TaskType::Chat);
        let local_score = score_provider(&local, &config);
        let remote_score = score_provider(&remote, &config);
        assert!(local_score.latency > remote_score.latency);
    }

    /// TC-SPS-004: 连续性——已选 Provider 获得更高 continuity 分
    #[test]
    fn test_continuity_bonus() {
        let candidate = make_candidate(
            "openai",
            "gpt-4o",
            false,
            128_000,
            5.0,
            15.0,
            0.9,
            &[TaskType::Chat],
        );
        let config_with_lock = ScoringConfig {
            task_type: TaskType::Chat,
            estimated_input_tokens: 2000,
            estimated_output_tokens: 1000,
            locked_providers: HashSet::from(["openai".to_string()]),
        };
        let config_without_lock = ScoringConfig::for_task(TaskType::Chat);
        let with_lock = score_provider(&candidate, &config_with_lock);
        let without_lock = score_provider(&candidate, &config_without_lock);
        assert!(with_lock.continuity > without_lock.continuity);
    }

    /// TC-SPS-005: 空候选列表 → select_best 返回 None
    #[test]
    fn test_empty_candidates() {
        let candidates: Vec<ProviderCandidate> = vec![];
        let config = ScoringConfig::for_task(TaskType::Chat);
        assert!(select_best(&candidates, &config).is_none());
    }

    /// TC-SPS-006: rank_providers 按分数降序
    #[test]
    fn test_rank_descending() {
        let candidates = vec![
            make_candidate(
                "weak",
                "small",
                true,
                4_000,
                0.0,
                0.0,
                0.3,
                &[TaskType::Chat],
            ),
            make_candidate(
                "strong",
                "large",
                false,
                128_000,
                5.0,
                15.0,
                0.9,
                &[TaskType::Chat],
            ),
            make_candidate(
                "medium",
                "mid",
                true,
                32_000,
                0.0,
                0.0,
                0.6,
                &[TaskType::Chat],
            ),
        ];
        let config = ScoringConfig::for_task(TaskType::Chat);
        let ranked = rank_providers(&candidates, &config);
        assert!(ranked[0].weighted_score >= ranked[1].weighted_score);
        assert!(ranked[1].weighted_score >= ranked[2].weighted_score);
    }

    /// TC-SPS-007: TaskType::infer 从查询推断
    #[test]
    fn test_task_type_infer() {
        assert_eq!(TaskType::infer("写一个函数"), TaskType::Code);
        assert_eq!(TaskType::infer("translate this"), TaskType::Multilingual);
        assert_eq!(TaskType::infer("请总结这段文字"), TaskType::LongContext);
        assert_eq!(TaskType::infer("证明这个定理"), TaskType::Reasoning);
        assert_eq!(TaskType::infer("制定计划步骤"), TaskType::Agentic);
        assert_eq!(TaskType::infer("你好"), TaskType::Chat);
    }

    /// TC-SPS-008: explain() 生成可读说明
    #[test]
    fn test_explain_readable() {
        let candidate = make_candidate(
            "openai",
            "gpt-4o",
            false,
            128_000,
            5.0,
            15.0,
            0.9,
            &[TaskType::Chat],
        );
        let config = ScoringConfig::for_task(TaskType::Chat);
        let score = score_provider(&candidate, &config);
        let explain = score.explain();
        assert!(explain.contains("openai"));
        assert!(explain.contains("gpt-4o"));
    }

    /// TC-SPS-009: format_ranking 输出
    #[test]
    fn test_format_ranking() {
        let candidates = vec![
            make_candidate(
                "a",
                "model-a",
                false,
                128_000,
                5.0,
                15.0,
                0.9,
                &[TaskType::Chat],
            ),
            make_candidate(
                "b",
                "model-b",
                true,
                32_000,
                0.0,
                0.0,
                0.6,
                &[TaskType::Chat],
            ),
        ];
        let config = ScoringConfig::for_task(TaskType::Chat);
        let ranked = rank_providers(&candidates, &config);
        let formatted = format_ranking(&ranked, 5);
        assert!(formatted.contains("1."));
        assert!(formatted.contains("2."));
    }

    /// TC-SPS-010: 历史成功率影响可靠性分数
    #[test]
    fn test_historical_success_rate() {
        let mut candidate = make_candidate(
            "flaky",
            "model",
            false,
            128_000,
            5.0,
            15.0,
            0.9,
            &[TaskType::Chat],
        );
        candidate.historical_success_rate = Some(0.5);
        let config = ScoringConfig::for_task(TaskType::Chat);
        let score = score_provider(&candidate, &config);
        assert!((score.reliability - 0.5).abs() < 0.001);
    }

    /// TC-SPS-011: 上下文窗口不足时扣分
    #[test]
    fn test_small_context_window() {
        let candidate = make_candidate(
            "tiny",
            "small-ctx",
            true,
            2_000,
            0.0,
            0.0,
            0.5,
            &[TaskType::Chat],
        );
        let config = ScoringConfig {
            task_type: TaskType::Chat,
            estimated_input_tokens: 10_000,
            estimated_output_tokens: 5_000,
            locked_providers: HashSet::new(),
        };
        let score = score_provider(&candidate, &config);
        assert!(score.context_window < 0.5);
    }

    /// TC-SPS-012: 加权总分在 [0, 1] 区间
    #[test]
    fn test_score_in_range() {
        let candidate = make_candidate("x", "m", false, 128_000, 5.0, 15.0, 0.9, &[TaskType::Chat]);
        let config = ScoringConfig::for_task(TaskType::Chat);
        let score = score_provider(&candidate, &config);
        assert!(score.weighted_score >= 0.0 && score.weighted_score <= 1.0);
    }
}
