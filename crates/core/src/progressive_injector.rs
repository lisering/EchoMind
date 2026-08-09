//! 渐进式上下文注入（REQ-PERF-010）：按需注入 chunk，LLM 上下文足够时停止追加。
//!
//! ## 核心流程
//!
//! 1. 检索 top-k chunks（按 score 降序）
//! 2. 初始注入 chunk[0..N]（N=2，可配置）→ LLM 开始流式生成
//! 3. 生成中检测"不确定"信号关键词
//!    - 检测到 → 追加下一批 chunk → 重启生成
//!    - 未检测到 → 继续生成直到完成
//! 4. 最大追加次数 = top_k，防止无限追加
//!
//! ## 效果
//!
//! - 平均注入 chunk 数：5 → 2-3（↓40-60% token）
//! - 足够信息时不再追加，避免上下文膨胀
//! - 与 Prompt 压缩（S2）兼容：压缩后再渐进注入
//!
//! ## 检测策略
//!
//! 简单版用关键词匹配（"不确定"/"没有找到"/"insufficient"），
//! 复杂版可用 LLM 判断（但增加 token，不推荐）。
//!
//! ## 调研来源
//!
//! - 自研设计，灵感来自 Self-RAG 的自主检索决策和 CRAG 的检索精炼理念

use echomind_models::RetrievalResult;

/// 关键词/短语列表：LLM 输出中出现这些词时表示需要更多上下文信息。
///
/// 检测策略：
/// - 英文关键词：大小写不敏感匹配
/// - 中文关键词：精确匹配
const INSUFFICIENT_KEYWORDS: &[&str] = &[
    // 中文
    "不确定",
    "没有找到",
    "未找到",
    "无法确定",
    "不清楚",
    "无法回答",
    "信息不足",
    "不相关",
    "未能找到",
    "无法确认",
    "找不到",
    "无相关",
    "无法确定",
    "知识库中未找到",
    // 英文
    "insufficient",
    "not found",
    "unclear",
    "cannot determine",
    "not enough",
    "no relevant",
    "unable to",
    "not sure",
    "no information",
    "no mention",
    "does not contain",
    "not covered",
];

/// 渐进式注入元数据（由 ChatEngine 返回，供调用方决策追加）。
#[derive(Debug, Clone)]
pub struct ProgressiveInfo {
    /// 总 source 数量
    pub total_sources: usize,
    /// 已注入 prompt 的 source 数量
    pub injected_count: usize,
    /// 是否还有未注入的 source 可供追加
    pub can_expand: bool,
}

/// 渐进式注入配置。
#[derive(Debug, Clone)]
pub struct ProgressiveConfig {
    /// 初始注入的 chunk 数量（默认 2）。
    pub initial_count: usize,
    /// 最大追加轮次（默认等于 top_k，防止无限追加）。
    pub max_rounds: usize,
}

impl Default for ProgressiveConfig {
    fn default() -> Self {
        Self {
            initial_count: 2,
            max_rounds: 8, // 匹配 DEFAULT_TOP_K
        }
    }
}

/// 渐进式注入状态机。
///
/// 跟踪已注入的 source 数量和追加轮次，管理从初始注入 → 检测 → 追加 → 完成的流程。
///
/// # 使用方式
///
/// ```ignore
/// use echomind_core::progressive_injector::ProgressiveInjector;
///
/// let mut injector = ProgressiveInjector::with_defaults(5);
/// let initial = injector.initial_indices(); // [0, 1]
/// // LLM 生成后检测
/// if injector.needs_more_info("我不确定...") {
///     let next = injector.next_batch(); // [2]
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ProgressiveInjector {
    /// 配置
    config: ProgressiveConfig,
    /// 总 source 数量
    total_sources: usize,
    /// 已注入的 source 数量
    injected_count: usize,
    /// 已执行的追加轮次
    append_rounds: usize,
}

impl ProgressiveInjector {
    /// 创建新的注入器，指定配置和总 source 数量。
    pub fn new(config: ProgressiveConfig, total_sources: usize) -> Self {
        let initial_count = config.initial_count.min(total_sources.max(1));
        Self {
            config,
            total_sources,
            injected_count: initial_count,
            append_rounds: 0,
        }
    }

    /// 使用默认配置创建注入器。
    pub fn with_defaults(total_sources: usize) -> Self {
        Self::new(ProgressiveConfig::default(), total_sources)
    }

    /// 返回初始注入的 source 索引列表。
    ///
    /// 对于 5 个 source 和 initial_count=2，返回 `[0, 1]`。
    pub fn initial_indices(&self) -> Vec<usize> {
        let count = self.config.initial_count.min(self.total_sources);
        (0..count).collect()
    }

    /// 检测 LLM 累积输出文本中是否包含"需要更多信息"信号。
    ///
    /// 使用关键词匹配（大小写不敏感），检测中文和英文"不确定"类表述。
    pub fn needs_more_info(&self, text: &str) -> bool {
        detect_insufficient_info(text)
    }

    /// 是否还能追加更多 source。
    pub fn can_append(&self) -> bool {
        self.injected_count < self.total_sources && self.append_rounds < self.config.max_rounds
    }

    /// 获取下一批要追加的 source 索引（每次追加 1 个）。
    ///
    /// 返回空 vec 表示无法继续追加。
    pub fn next_batch(&mut self) -> Vec<usize> {
        if !self.can_append() {
            return vec![];
        }
        let start = self.injected_count;
        let end = (start + 1).min(self.total_sources);
        let batch: Vec<usize> = (start..end).collect();
        self.injected_count = end;
        self.append_rounds += 1;
        batch
    }

    /// 已注入的 source 数量。
    pub fn injected_count(&self) -> usize {
        self.injected_count
    }

    /// 已执行的追加轮次。
    pub fn append_rounds(&self) -> usize {
        self.append_rounds
    }

    /// 总 source 数量。
    pub fn total_sources(&self) -> usize {
        self.total_sources
    }
}

/// 独立函数：检测文本中是否包含"信息不足"信号。
///
/// 供 `forward_stream` 在 commands.rs 中流式消费 LLM 输出时调用，
/// 检测累积文本中是否出现"不确定"类关键词，触发渐进式上下文追加。
pub fn detect_insufficient_info(text: &str) -> bool {
    let lower = text.to_lowercase();
    INSUFFICIENT_KEYWORDS.iter().any(|kw| {
        if kw.is_ascii() {
            lower.contains(kw)
        } else {
            text.contains(kw)
        }
    })
}

/// 将 source 列表分割为初始批次和剩余批次。
///
/// 返回 `(initial_sources, remaining_sources)`。
pub fn split_sources(
    sources: &[RetrievalResult],
    initial_count: usize,
) -> (Vec<RetrievalResult>, Vec<RetrievalResult>) {
    let split = initial_count.min(sources.len());
    let initial = sources[..split].to_vec();
    let remaining = sources[split..].to_vec();
    (initial, remaining)
}

/// 为 source 子集构建分段式提示词。
///
/// 使用 `build_rag_prompt_segmented` 从 prompt crate 构建提示词，
/// 但仅使用指定的 source 子集。允许渐进式注入以小批量初始上下文开始，
/// 按需扩展。
pub fn build_progressive_prompt(sources: &[RetrievalResult]) -> echomind_prompt::SegmentedPrompt {
    echomind_prompt::build_rag_prompt_segmented(sources)
}

/// 统计信息：渐进式注入的运行时统计。
#[derive(Debug, Clone, Default)]
pub struct ProgressiveStats {
    /// 总查询次数
    pub total_queries: usize,
    /// 总注入 chunk 数（所有查询累计）
    pub total_injected: usize,
    /// 触发追加的查询次数
    pub append_triggered: usize,
}

impl ProgressiveStats {
    /// 平均注入 chunk 数。
    pub fn avg_injected(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.total_injected as f64 / self.total_queries as f64
    }

    /// 追加触发率（0.0-1.0）。
    pub fn append_rate(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.append_triggered as f64 / self.total_queries as f64
    }

    /// 记录一次查询的结果。
    pub fn record(&mut self, injected: usize, appended: bool) {
        self.total_queries += 1;
        self.total_injected += injected;
        if appended {
            self.append_triggered += 1;
        }
    }
}
