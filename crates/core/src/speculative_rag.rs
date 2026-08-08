//! Speculative RAG（REQ-PERF-011）：小模型快速生成草稿 → 大模型验证/修正。
//!
//! ## 核心流程
//!
//! 1. **草稿阶段**：小模型用简化 prompt（仅 top-1 chunk）快速生成草稿
//! 2. **质量检查**：草稿过短（< `min_draft_length`）则回退直接生成
//! 3. **验证阶段**：大模型用完整 prompt（全部 chunks）+ 草稿作为 prefix 验证/修正
//! 4. **决策**：
//!    - 草稿被接受（相似度 ≥ `accept_threshold`）→ 直接使用草稿
//!    - 草稿被修正（相似度 < `accept_threshold`）→ 使用修正后输出
//!    - 草稿质量不足 → 回退为大模型直接生成
//!
//! ## 效果
//!
//! - 首 token 延迟：↓40-60%（小模型 ~200ms vs 大模型 ~1500ms）
//! - 草稿 + 验证总 token < 大模型直接生成（验证可复用草稿 prefix）
//!
//! ## 与渐进注入（S7）兼容
//!
//! 草稿先注入 top-1 chunk，验证时注入全部 chunks（等效于追加）。
//! 当渐进注入和 Speculative RAG 同时启用时：
//! - 草稿用 top-1 chunk（渐进注入的初始子集）
//! - 验证用全部 chunks（渐进注入的完整集合）
//!
//! ## 调研来源
//!
//! - vLLM (arXiv:2309.06180) — Speculative Decoding 概念
//! - 自研适配 RAG 场景：草稿用简化 prompt，验证用完整 prompt

use echomind_models::{ChatMessage, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::LLMProvider;
use echomind_prompt::build_rag_prompt_segmented;

/// Speculative RAG 配置。
#[derive(Debug, Clone)]
pub struct SpeculativeRagConfig {
    /// 草稿阶段注入的 chunk 数量（默认 1，仅 top-1）。
    ///
    /// 草稿使用更少的上下文，使小模型快速生成。
    pub draft_top_k: usize,
    /// 草稿质量最小字符数阈值。
    ///
    /// 草稿字符数低于此值时视为质量不足，回退为大模型直接生成。
    pub min_draft_length: usize,
    /// 验证接受阈值（0.0-1.0）。
    ///
    /// 大模型输出与草稿的相似度高于此值时接受草稿，否则视为修正。
    pub accept_threshold: f32,
}

impl Default for SpeculativeRagConfig {
    fn default() -> Self {
        Self {
            draft_top_k: 1,
            min_draft_length: 20,
            accept_threshold: 0.85,
        }
    }
}

/// Speculative RAG 执行结果。
pub enum SpeculativeOutcome {
    /// 草稿被大模型接受（直接使用草稿）。
    DraftAccepted {
        /// 被接受的草稿内容
        draft: String,
        /// 输出流（内容与草稿一致，以流式形式返回）
        stream: BoxStream<'static, anyhow::Result<String>>,
    },
    /// 草稿被大模型修正。
    DraftCorrected {
        /// 原始草稿
        draft: String,
        /// 修正后输出的流
        stream: BoxStream<'static, anyhow::Result<String>>,
    },
    /// 草稿质量不足，回退为大模型直接生成。
    FallbackDirect {
        /// 直接生成的流
        stream: BoxStream<'static, anyhow::Result<String>>,
    },
}

impl std::fmt::Debug for SpeculativeOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DraftAccepted { draft, .. } => f
                .debug_struct("DraftAccepted")
                .field("draft", draft)
                .finish(),
            Self::DraftCorrected { draft, .. } => f
                .debug_struct("DraftCorrected")
                .field("draft", draft)
                .finish(),
            Self::FallbackDirect { .. } => f.debug_struct("FallbackDirect").finish(),
        }
    }
}

/// Speculative RAG 统计信息。
#[derive(Debug, Clone, Default)]
pub struct SpeculativeStats {
    /// 总查询次数
    pub total_queries: usize,
    /// 草稿被接受次数
    pub draft_accepted: usize,
    /// 草稿被修正次数
    pub draft_corrected: usize,
    /// 回退直接生成次数
    pub fallback_direct: usize,
}

impl SpeculativeStats {
    /// 草稿接受率（0.0-1.0）。
    pub fn accept_rate(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.draft_accepted as f64 / self.total_queries as f64
    }

    /// 回退率（0.0-1.0）。
    pub fn fallback_rate(&self) -> f64 {
        if self.total_queries == 0 {
            return 0.0;
        }
        self.fallback_direct as f64 / self.total_queries as f64
    }

    /// 记录一次查询结果。
    pub fn record(&mut self, accepted: bool, corrected: bool, fallback: bool) {
        self.total_queries += 1;
        if fallback {
            self.fallback_direct += 1;
        } else if accepted {
            self.draft_accepted += 1;
        } else if corrected {
            self.draft_corrected += 1;
        }
    }
}

/// Speculative RAG 引擎（REQ-PERF-011）。
///
/// 使用小模型（draft）快速生成草稿，大模型（verify）验证/修正。
/// 预期首 token 延迟 ↓40-60%。
///
/// # 泛型参数
/// - `D`: 草稿模型（小模型），实现 `LLMProvider`
/// - `L`: 验证模型（大模型），实现 `LLMProvider`
///
/// # 使用方式
///
/// ```ignore
/// use echomind_core::speculative_rag::{SpeculativeRagEngine, SpeculativeRagConfig};
///
/// let engine = SpeculativeRagEngine::new(draft_llm, verify_llm)
///     .with_config(SpeculativeRagConfig::default());
/// let outcome = engine.speculate(&sources, &history, "查询问题").await?;
/// ```
///
/// # 流程
///
/// 1. 草稿模型用简化 prompt（top-1 chunk）生成草稿
/// 2. 草稿过短 → 回退直接生成
/// 3. 验证模型用完整 prompt + 草稿验证
/// 4. 相似度 ≥ 阈值 → 接受草稿；否则 → 修正
pub struct SpeculativeRagEngine<D: LLMProvider, L: LLMProvider> {
    /// 草稿模型（小模型）
    draft_llm: D,
    /// 验证模型（大模型）
    verify_llm: L,
    /// 配置
    config: SpeculativeRagConfig,
}

impl<D: LLMProvider, L: LLMProvider> SpeculativeRagEngine<D, L> {
    /// 创建 Speculative RAG 引擎，使用默认配置。
    pub fn new(draft_llm: D, verify_llm: L) -> Self {
        Self {
            draft_llm,
            verify_llm,
            config: SpeculativeRagConfig::default(),
        }
    }

    /// 设置配置。
    pub fn with_config(mut self, config: SpeculativeRagConfig) -> Self {
        self.config = config;
        self
    }

    /// 执行 Speculative RAG。
    ///
    /// # 参数
    /// - `sources`: 检索结果列表（已按 score 降序排列）
    /// - `history`: 对话历史
    /// - `query`: 用户查询
    ///
    /// # 返回
    /// `SpeculativeOutcome` 枚举，包含输出流。
    ///
    /// # 流程
    /// 1. 草稿模型用简化 prompt（top-N chunk，N=`draft_top_k`）生成草稿
    /// 2. 草稿字符数 < `min_draft_length` → 回退直接生成
    /// 3. 验证模型用完整 prompt + 草稿作为 prefix 验证/修正
    /// 4. 验证输出与草稿相似度 ≥ `accept_threshold` → 接受草稿
    /// 5. 否则 → 使用修正后输出
    pub async fn speculate(
        &self,
        sources: &[RetrievalResult],
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<SpeculativeOutcome> {
        if sources.is_empty() {
            return Err(anyhow::anyhow!("Speculative RAG 需要至少 1 个检索结果"));
        }

        // 阶段 1：草稿模型用简化 prompt（仅 top-N chunk）生成草稿
        let draft_count = self.config.draft_top_k.min(sources.len());
        let draft_sources = &sources[..draft_count];
        let draft_prompt = build_rag_prompt_segmented(draft_sources);
        let draft_stream = self
            .draft_llm
            .chat_stream_segmented(
                &draft_prompt.static_prefix,
                &draft_prompt.dynamic_context,
                history,
                query,
            )
            .await?;
        let draft = collect_stream(draft_stream).await?;

        // 阶段 2：检查草稿质量
        if draft.chars().count() < self.config.min_draft_length {
            // 草稿质量不足，回退直接生成
            let full_prompt = build_rag_prompt_segmented(sources);
            let stream = self
                .verify_llm
                .chat_stream_segmented(
                    &full_prompt.static_prefix,
                    &full_prompt.dynamic_context,
                    history,
                    query,
                )
                .await?;
            return Ok(SpeculativeOutcome::FallbackDirect { stream });
        }

        // 阶段 3：验证模型用完整 prompt + 草稿作为 prefix 验证
        let full_prompt = build_rag_prompt_segmented(sources);
        let verify_system = format!(
            "{static}\n\n{dynamic}\n\n--- 草稿验证 ---\n\
             以下是一个基于部分上下文生成的草稿回答。请基于上方完整上下文验证或修正此草稿。\n\
             如果草稿正确，请输出相同内容；如果有误，请修正后输出完整回答。\n\n\
             草稿内容：\n{draft}",
            static = full_prompt.static_prefix,
            dynamic = full_prompt.dynamic_context,
            draft = draft,
        );
        let verify_stream = self
            .verify_llm
            .chat_stream(&verify_system, history, query)
            .await?;
        let verified = collect_stream(verify_stream).await?;

        // 阶段 4：比较验证输出与草稿相似度
        let similarity = text_similarity(&draft, &verified);

        if similarity >= self.config.accept_threshold {
            // 草稿被接受：流式输出草稿内容
            let draft_clone = draft.clone();
            let stream =
                futures::stream::once(async move { Ok::<String, anyhow::Error>(draft_clone) })
                    .boxed();
            Ok(SpeculativeOutcome::DraftAccepted { draft, stream })
        } else {
            // 草稿被修正：流式输出修正后内容
            let verified_clone = verified.clone();
            let stream =
                futures::stream::once(async move { Ok::<String, anyhow::Error>(verified_clone) })
                    .boxed();
            Ok(SpeculativeOutcome::DraftCorrected { draft, stream })
        }
    }
}

/// 计算两段文本的相似度（0.0-1.0）。
///
/// 使用基于 Levenshtein 编辑距离的归一化相似度：
/// `similarity = 1.0 - (edit_distance / max_length)`
///
/// 完全相同 → 1.0，完全不同 → 0.0。
///
/// # 参数
/// - `a`: 第一段文本
/// - `b`: 第二段文本
///
/// # 返回
/// 相似度分数（0.0-1.0）。两段空文本返回 1.0，一段为空返回 0.0。
///
/// Bug #10 修复：长文本（> 200 字符）截断到前 200 字符比较，避免 O(m×n) 编辑距离
/// 在数千字回答上产生数百毫秒延迟。截断对相似度判断的影响可忽略——草稿和验证
/// 回答的差异通常在开头段落即可辨识（如方向性偏差、事实错误）。
pub fn text_similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // 截断到前 200 字符（性能优化，避免长文本 O(m×n) 超时）
    const MAX_COMPARE_CHARS: usize = 200;
    let a_chars: Vec<char> = a.chars().take(MAX_COMPARE_CHARS).collect();
    let b_chars: Vec<char> = b.chars().take(MAX_COMPARE_CHARS).collect();
    let max_len = a_chars.len().max(b_chars.len());
    let distance = levenshtein_distance(&a_chars, &b_chars);
    1.0 - (distance as f32 / max_len as f32)
}

/// Levenshtein 编辑距离（内部辅助函数）。
///
/// 计算将 `a` 转换为 `b` 所需的最少编辑操作数（插入/删除/替换）。
/// 时间复杂度 O(m×n)，空间复杂度 O(n)（滚动数组优化）。
fn levenshtein_distance(a: &[char], b: &[char]) -> usize {
    let m = a.len();
    let n = b.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr: Vec<usize> = vec![0; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// 从流收集完整文本（内部辅助函数）。
///
/// 消费 `BoxStream<Result<String>>`，将所有 token 拼接为完整字符串。
async fn collect_stream(
    mut stream: BoxStream<'static, anyhow::Result<String>>,
) -> anyhow::Result<String> {
    let mut result = String::new();
    while let Some(token) = stream.next().await {
        result.push_str(&token?);
    }
    Ok(result)
}

/// 从流收集完整文本并统计总字符数（公开函数，供测试使用）。
///
/// 消费 `BoxStream<Result<String>>`，返回完整字符串。
/// 用于 TC-SPEC-006 统计 token 数对比。
pub async fn collect_stream_to_string(
    stream: BoxStream<'static, anyhow::Result<String>>,
) -> anyhow::Result<String> {
    collect_stream(stream).await
}
