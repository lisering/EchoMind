//! 对话编排引擎（REQ-RAG-001/003/008）：检索 → 空上下文拦截 → 组装分段 Prompt → 调用 LLM。
//! 空上下文直接返回固定提示，不发起任何 LLM 网络请求（防止低质量上下文污染与无效计费）。
//! 系统提示词包含语言跟随引导指令（REQ-RAG-008），使 LLM 以用户提问语言回答，
//! 代码/术语/API 名称保留原文不翻译。
//!
//! ## Prompt Caching 三段式架构
//!
//! 提示词构建已迁移至 `echomind-prompt` crate（`build_rag_prompt_segmented()`），
//! 将系统提示词拆分为静态前缀 + 动态上下文两段，使 API 端 prompt caching 能命中静态前缀。
//! `ChatEngine::chat()` 调用 `LLMProvider::chat_stream_segmented()` 传递两段。

use std::sync::Arc;
use tracing::warn;

use echomind_models::{ChatMessage, RetrievalResult, TokenUsage};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::memory_store::MemoryRetriever;
use crate::quality_gate::{GateConfig, GateScore};
use crate::splitter::bpe;
use crate::web_search::{self, DEFAULT_SEARCH_THRESHOLD};
use crate::{LLMProvider, Retriever, WebSearchProvider};
use echomind_prompt::build_rag_prompt_segmented;

/// 空上下文固定拒答文案（REQ-RAG-003-AC-1）
pub const NO_CONTEXT_MESSAGE: &str = "知识库中未找到相关内容。请尝试换个问题，或先导入相关文档。";

/// 对话编排输出。
pub enum ChatOutcome {
    /// 正常回答：引用来源 + token 流 + token 用量
    Answered {
        /// 实际注入上下文的引用来源（与 chat_sources 事件一致）
        sources: Vec<RetrievalResult>,
        /// token 增量流
        stream: BoxStream<'static, anyhow::Result<String>>,
        /// token 用量统计（远程 API 模式下来自 SSE usage 字段；本地推理模式为 None）
        token_usage: Option<TokenUsage>,
        /// 质量门控评分结果（None 表示门控未启用，REQ-RAG-028）
        gate_score: Option<GateScore>,
    },
    /// 知识库未命中：固定提示流（未调用 LLM）
    NoContext {
        /// 固定拒答文案流
        stream: BoxStream<'static, anyhow::Result<String>>,
    },
}

/// RAG 对话编排引擎（六边形架构：仅依赖端口 Trait）。
pub struct ChatEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
    /// 质量门控配置（None = 已禁用，REQ-RAG-028）
    gate_config: Option<GateConfig>,
    /// 质量门控评分结果（None = 门控未启用或未评估）
    gate_score: Option<GateScore>,
    /// 对话记忆检索器（None = 记忆注入已禁用，REQ-RAG-032）
    memory: Option<Arc<dyn MemoryRetriever>>,
    /// 网页搜索 provider（None = 网页搜索已禁用，REQ-RAG-036）
    ///
    /// 当本地检索 top-1 score < threshold 时触发网页搜索，
    /// 搜索结果通过 RRF 融合到本地结果中。
    web_search_provider: Option<Arc<dyn WebSearchProvider>>,
}

impl<R: Retriever, L: LLMProvider> ChatEngine<R, L> {
    pub fn new(retriever: R, llm: L) -> Self {
        Self {
            retriever,
            llm,
            gate_config: None,
            gate_score: None,
            memory: None,
            web_search_provider: None,
        }
    }

    /// 启用质量门控（REQ-RAG-028）。
    ///
    /// 启用后，`chat_with_sources()` 在压缩前评估检索结果质量。
    /// 当前版本仅做评估 + 日志记录（PassThrough 策略），不阻断生成。
    /// 降级策略（ExpandTopK）由 `chat_inner` 在外部处理（因为需要重新检索）。
    pub fn with_quality_gate(mut self, config: GateConfig) -> Self {
        self.gate_config = Some(config);
        self
    }

    /// 启用对话记忆注入（REQ-RAG-032）。
    ///
    /// 启用后，`chat_with_sources()` 在构建 system prompt 前检索相关记忆，
    /// 将记忆以 `[相关记忆]` 块注入到 system prompt 静态前缀之前。
    /// 记忆仅作为额外上下文，LLM 自行决定是否引用。
    ///
    /// 向后兼容：未调用此方法时 `memory` 为 `None`，行为与之前完全一致。
    pub fn with_memory(mut self, memory: Arc<dyn MemoryRetriever>) -> Self {
        self.memory = Some(memory);
        self
    }

    /// 启用网页搜索补充（REQ-RAG-036）。
    ///
    /// 启用后，当本地检索 top-1 score < `DEFAULT_SEARCH_THRESHOLD` (0.3) 时，
    /// 自动调用 `WebSearchProvider` 搜索互联网，将搜索结果通过 RRF 融合到本地结果中。
    /// 搜索失败时优雅降级为仅使用本地结果。
    ///
    /// 向后兼容：未调用此方法时 `web_search_provider` 为 `None`，行为与之前完全一致。
    pub fn with_web_search(mut self, provider: Arc<dyn WebSearchProvider>) -> Self {
        self.web_search_provider = Some(provider);
        self
    }

    /// 获取质量门控评分结果（REQ-RAG-028）。
    ///
    /// 返回最近一次 `chat_with_sources()` 的质量评分。
    /// `None` 表示门控未启用或尚未评估。
    pub fn gate_score(&self) -> Option<&GateScore> {
        self.gate_score.as_ref()
    }

    /// 执行一轮对话：检索 → 空上下文拦截 → 组装分段提示词 → 流式调用 LLM。
    ///
    /// 使用 `build_rag_prompt_segmented()` 将系统提示词拆分为静态前缀 + 动态上下文，
    /// 通过 `chat_stream_segmented()` 传递给 LLM Provider，使 API 端可命中 prompt caching。
    pub async fn chat(
        &self,
        history: &[ChatMessage],
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<ChatOutcome> {
        let mut sources = self.retriever.retrieve(query, top_k).await?;

        // REQ-RAG-036：网页搜索补充
        //
        // 当本地检索 top-1 score < 阈值时，触发网页搜索补充 context。
        // 搜索结果通过 RRF 融合到本地结果中，在 prompt 中标注来源（🌐 Web）。
        // 搜索失败时优雅降级为仅使用本地结果。
        if let Some(ref web_provider) = self.web_search_provider
            && web_search::should_search(&sources, DEFAULT_SEARCH_THRESHOLD)
        {
            warn!(
                "本地检索 top-1 score 低于阈值 {:.1}，触发网页搜索",
                DEFAULT_SEARCH_THRESHOLD
            );
            sources = web_search::search_and_fuse(web_provider, query, sources, top_k).await;
        }

        self.chat_with_sources(history, query, sources).await
    }

    /// 使用外部预检索结果执行一轮对话（跳过内部检索）。
    ///
    /// **性能优化（出答案速度）**：调用方可将检索与上下文压缩（compaction）并行执行，
    /// 检索不依赖压缩后的历史，二者互不阻塞，节省串行等待时间。
    pub async fn chat_with_sources(
        &self,
        history: &[ChatMessage],
        query: &str,
        sources: Vec<RetrievalResult>,
    ) -> anyhow::Result<ChatOutcome> {
        if sources.is_empty() {
            // 空上下文拦截：固定提示，不调用 LLM（REQ-RAG-003-AC-2）
            let stream = futures::stream::once(async move {
                Ok::<String, anyhow::Error>(NO_CONTEXT_MESSAGE.to_string())
            })
            .boxed();
            return Ok(ChatOutcome::NoContext { stream });
        }

        // REQ-RAG-028：质量门控评估（在压缩前评估原始检索结果质量）
        //
        // 当前版本仅做评估 + 日志记录（PassThrough 策略），不阻断生成。
        // 低质量时记录告警日志，供 chat_inner 外部读取 gate_score 做降级决策。
        let gate_score = if let Some(ref gate) = self.gate_config {
            let score = crate::quality_gate::evaluate(&sources, gate);
            if !score.passed {
                warn!(
                    "检索质量偏低: weighted={:.3} coverage={:.3} diversity={:.3} variance={:.3}",
                    score.weighted, score.coverage, score.diversity, score.score_variance
                );
            }
            Some(score)
        } else {
            None
        };
        // 存储评分结果供外部读取（gate_score() 方法）
        // 注意：ChatEngine 是不可变引用（&self），无法写入字段。
        // 评分结果通过返回值传递给调用方。

        let segmented = build_rag_prompt_segmented(&sources);

        // REQ-RAG-032：对话记忆注入
        //
        // 若启用记忆，检索相关记忆并注入到 system prompt 静态前缀之前。
        // 记忆以 `[相关记忆]` 块格式注入，LLM 自行决定是否引用。
        // 记忆仅作为额外上下文，不修改 RAG context 部分（检索结果不受记忆影响）。
        let static_prefix = if let Some(ref mem) = self.memory {
            let memories = mem
                .retrieve_relevant_memories(query, 5)
                .await
                .unwrap_or_default();
            if memories.is_empty() {
                segmented.static_prefix.clone()
            } else {
                let memory_text = memories
                    .iter()
                    .map(|m| format!("- {}", m.content))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("[相关记忆]\n{memory_text}\n\n{}", segmented.static_prefix)
            }
        } else {
            segmented.static_prefix.clone()
        };

        let stream = self
            .llm
            .chat_stream_segmented(&static_prefix, &segmented.dynamic_context, history, query)
            .await?;
        Ok(ChatOutcome::Answered {
            sources,
            stream,
            token_usage: None,
            gate_score,
        })
    }
}

// ============================================================
// 对话上下文长度管理（REQ-RAG-017）
// ============================================================

/// 历史截断结果。
pub struct TruncationResult {
    /// 截断后的历史消息（保留首轮 + 最近 N 轮）
    pub history: Vec<ChatMessage>,
    /// 截断信息（None 表示无需截断）
    pub info: Option<echomind_models::HistoryTruncationPayload>,
}

/// 截断历史消息：保留首轮 + 最近 N 轮，截断中间消息（REQ-RAG-017 AC-1/AC-5）。
///
/// 策略：
/// 1. 计算所有历史消息的总 token 数
/// 2. 若未超限，返回原始历史
/// 3. 若超限，保留前 2 条（首轮 Q&A）+ 从末尾往前保留尽可能多的消息
/// 4. 中间消息被截断，不发送给 LLM
///
/// # 参数
/// - `history` — 完整历史消息列表
/// - `token_limit` — 上下文 token 限制（REQ-RAG-017 AC-4：可配置，默认 4096）
pub fn truncate_history(
    history: &[ChatMessage],
    token_limit: usize,
) -> anyhow::Result<TruncationResult> {
    // 空历史或单条消息无需截断
    if history.len() <= 2 {
        return Ok(TruncationResult {
            history: history.to_vec(),
            info: None,
        });
    }

    let encoder = bpe()?;

    // 计算总 token 数
    let msg_tokens: Vec<usize> = history
        .iter()
        .map(|m| encoder.encode_with_special_tokens(&m.content).len())
        .collect();
    let total_tokens: usize = msg_tokens.iter().sum();

    // 未超限，无需截断
    if total_tokens <= token_limit {
        return Ok(TruncationResult {
            history: history.to_vec(),
            info: None,
        });
    }

    // 保留首轮（前 2 条：user + assistant）
    let first_turn_end = 2.min(history.len());
    let mut retained: Vec<ChatMessage> = history[..first_turn_end].to_vec();
    let mut retained_tokens: usize = msg_tokens[..first_turn_end].iter().sum();

    // 从后往前保留消息，直到接近 token 限制
    let mut recent: Vec<ChatMessage> = Vec::new();
    for i in (first_turn_end..history.len()).rev() {
        let msg_tok = msg_tokens[i];
        if retained_tokens + msg_tok > token_limit {
            break;
        }
        recent.push(history[i].clone());
        retained_tokens += msg_tok;
    }
    recent.reverse();
    retained.extend(recent);

    let truncated_count = history.len() - retained.len();
    if truncated_count == 0 {
        return Ok(TruncationResult {
            history: history.to_vec(),
            info: None,
        });
    }

    Ok(TruncationResult {
        history: retained,
        info: Some(echomind_models::HistoryTruncationPayload {
            truncated_count,
            total_tokens,
            retained_tokens,
            token_limit,
        }),
    })
}
