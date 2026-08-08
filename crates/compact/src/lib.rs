//! 对话历史压缩引擎：当历史超过阈值时将旧消息压缩为摘要（而非丢弃）。
//!
//! 替代 `echomind_core::chat::truncate_history` 的纯截断策略——截断直接丢弃中间消息，
//! 导致 LLM 丢失早期对话的关键上下文。压缩引擎通过 LLM 将旧消息
//! 生成简洁摘要，保留关键信息的同时控制 token 数量。
//!
//! ## 压缩算法
//!
//! 1. 计算全部历史消息的 token 总数（tiktoken BPE）
//! 2. 若未超限（≤ `token_limit`），返回原始历史（`info = None`）
//! 3. 若超限，将历史分为「待压缩旧消息」+「保留的最近消息」：
//!    - 最近消息预算 = `token_limit * 2/3`，从末尾向前保留尽可能多的消息
//!    - 旧消息 = 预算之外的头部消息
//! 4. 调用 LLM 将旧消息压缩为一条摘要 system 消息
//! 5. 返回 `[摘要 system 消息] + [最近消息]`
//!
//! ## 降级策略
//!
//! 当 LLM 摘要生成失败（网络错误、超时等）时，降级为截断策略
//! （保留最近消息，丢弃旧消息），并在摘要位置插入截断提示 system 消息，
//! 确保不返回 Err 中断对话流程。
//!
//! ## 与 `truncate_history` 的对比
//!
//! | 维度 | `truncate_history` | `CompactionEngine` |
//! |---|---|---|
//! | 旧消息处理 | 丢弃 | LLM 压缩为摘要 |
//! | 上下文保留 | 仅首轮 + 最近 N 轮 | 摘要 + 最近 N 轮 |
//! | 信息损失 | 高（中间消息完全丢失） | 低（摘要保留关键信息） |
//! | LLM 调用 | 无 | 一次（摘要生成） |
//! | 失败降级 | N/A | 退化为截断 |

use std::collections::HashSet;
use std::sync::Arc;

use echomind_models::{ChatMessage, CompactionInfo, CompactionKind, CompactionResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use echomind_core::LLMProvider;
use echomind_core::splitter::bpe;

/// Token 预算驱动的上下文压缩增强（S69：Cherry Studio 借鉴）。
pub mod token_budget;

// 重导出常用类型
pub use token_budget::{
    IncrementalCompactor, build_summary_input, estimate_messages_tokens, estimate_tokens,
    extract_summary_from_result, find_compaction_boundary, needs_compaction,
};

// ============================================================
// Q03: 双阈值压缩 + 后台异步压缩
// ============================================================

/// 压缩触发类型（借鉴 QM `CompactionTrigger`）。
///
/// 双阈值判断结果：
/// - `None`：历史未达 Soft 阈值（70%），无需压缩
/// - `Background`：达到 Soft 阈值但未达 Hard 阈值，后台异步压缩
/// - `Synchronous`：达到 Hard 阈值（90%），同步阻塞压缩
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompactionTrigger {
    /// 未触发
    #[default]
    None,
    /// Soft 阈值（70%）— 后台异步压缩
    Background,
    /// Hard 阈值（90%）— 同步压缩
    Synchronous,
}

impl CompactionTrigger {
    /// 是否未触发压缩。
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// 是否为后台异步压缩。
    pub fn is_background(&self) -> bool {
        matches!(self, Self::Background)
    }

    /// 是否为同步压缩。
    pub fn is_synchronous(&self) -> bool {
        matches!(self, Self::Synchronous)
    }

    /// 转为 `CompactionKind`（`None` 返回 `None`）。
    pub fn to_kind(&self) -> Option<CompactionKind> {
        match self {
            Self::None => None,
            Self::Background => Some(CompactionKind::Background),
            Self::Synchronous => Some(CompactionKind::Synchronous),
        }
    }
}

/// 双阈值压缩配置（借鉴 QM `COMPACT_SOFT_FRACTION` / `COMPACT_HARD_FRACTION`）。
///
/// 控制 `check_compaction_needed()` 的阈值行为。
/// 可通过设置面板配置，适配不同对话长度和 LLM context window。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DualThresholdConfig {
    /// Soft 阈值（0.0-1.0），达到时触发后台异步压缩（默认 0.7）
    #[serde(default = "default_soft_threshold")]
    pub soft_threshold: f64,
    /// Hard 阈值（0.0-1.0），达到时触发同步压缩（默认 0.9）
    #[serde(default = "default_hard_threshold")]
    pub hard_threshold: f64,
    /// 历史条目上限（entry_ratio = history.len() / max_entries）
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
    /// Token 上限（token_ratio = estimated_tokens / max_tokens）
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
}

fn default_soft_threshold() -> f64 {
    0.7
}
fn default_hard_threshold() -> f64 {
    0.9
}
fn default_max_entries() -> usize {
    50
}
fn default_max_tokens() -> usize {
    8192
}

impl Default for DualThresholdConfig {
    fn default() -> Self {
        Self {
            soft_threshold: default_soft_threshold(),
            hard_threshold: default_hard_threshold(),
            max_entries: default_max_entries(),
            max_tokens: default_max_tokens(),
        }
    }
}

impl DualThresholdConfig {
    /// 从设置键值构建配置（用于从 SQLite settings 表读取）。
    pub fn from_settings(
        soft: Option<f64>,
        hard: Option<f64>,
        max_entries: Option<usize>,
        max_tokens: Option<usize>,
    ) -> Self {
        let defaults = Self::default();
        Self {
            soft_threshold: soft.unwrap_or(defaults.soft_threshold),
            hard_threshold: hard.unwrap_or(defaults.hard_threshold),
            max_entries: max_entries.unwrap_or(defaults.max_entries),
            max_tokens: max_tokens.unwrap_or(defaults.max_tokens),
        }
    }
}

/// 粗估历史消息的 token 数（避免每次调用 BPE 编码器）。
///
/// 使用 `content.len() / 4` 作为快速估算（4 字符 ≈ 1 token，适用于英文）。
/// 中文文本略低估（2-3 字符 ≈ 1 token），但阈值判断的误差可接受。
fn estimate_history_tokens(history: &[ChatMessage]) -> usize {
    history.iter().map(|m| m.content.len() / 4).sum()
}

/// 结构化压缩模板（借鉴 OpenCode SessionCompaction）。
///
/// 替代旧版自由文本提示词，使用 6 章节结构化 Markdown 模板引导 LLM 生成
/// 格式一致的摘要，保留关键工作状态信息。
const STRUCTURED_SUMMARY_TEMPLATE: &str = r#"请按以下 Markdown 结构输出对话历史摘要，保持章节顺序不变。不要包含 <template> 标签。

<template>
## 目标
- [一到两句话描述用户试图完成的任务]

## 重要细节
- [约束/偏好、决策及原因、重要事实/假设、继续所需的确切上下文，或"(无)"]

## 工作状态
### 已完成
- [已完成的工作、已验证的事实、或所做的更改；否则"(无)"]

### 进行中
- [当前工作、部分更改、或调查状态；否则"(无)"]

### 阻塞
- [障碍、失败命令、或未知问题；否则"(无)"]

## 下一步
1. [立即要做的具体操作，或"(无)"]
2. [已知的下一步操作，或"(无)"]

## 相关文件
- [文件或文档路径：为何重要，或"(无)"]
</template>

规则：
- 保留每个章节，即使为空。
- 使用简洁的要点，不要写成段落。
- 保留确切的文件路径、符号、命令、错误字符串、URL 和标识符。
- 不要提及摘要过程或上下文被压缩。"#;

/// 降级截断时插入的提示消息前缀。
const FALLBACK_NOTICE: &str = "[早期对话历史已截断] ";

/// 对话历史压缩引擎。
///
/// 泛型参数 `L` 为 `LLMProvider` trait 的具体实现（远程 API 或本地推理引擎）。
/// 通过引用借用 LLM Provider，不获取所有权，使调用方可在压缩后继续使用 Provider。
///
/// # 用法
///
/// ```ignore
/// let compaction = CompactionEngine::new(&llm);
/// let result = compaction.compact(&history, 4096).await?;
/// // result.history 可直接传给 ChatEngine::chat()
/// ```
pub struct CompactionEngine<'a, L: LLMProvider> {
    /// 借用的 LLM Provider，用于生成摘要。
    llm: &'a L,
}

impl<'a, L: LLMProvider> CompactionEngine<'a, L> {
    /// 创建压缩引擎，借用 LLM Provider 引用。
    pub fn new(llm: &'a L) -> Self {
        Self { llm }
    }

    /// 压缩对话历史：超限时将旧消息压缩为摘要，未超限时原样返回。
    ///
    /// # 参数
    /// - `history` — 完整历史消息列表（按时间正序）
    /// - `token_limit` — 上下文 token 限制（默认 4096）
    ///
    /// # 返回
    /// `CompactionResult` 包含压缩后的历史和压缩信息。
    /// `info` 为 `None` 表示无需压缩（历史未超限）。
    ///
    /// # 降级
    /// LLM 摘要失败时降级为截断策略（保留最近消息），不返回 Err。
    pub async fn compact(
        &self,
        history: &[ChatMessage],
        token_limit: usize,
    ) -> anyhow::Result<CompactionResult> {
        // 空历史或短历史（≤ 2 条）无需压缩
        if history.len() <= 2 {
            return Ok(CompactionResult {
                history: history.to_vec(),
                info: None,
            });
        }

        let encoder = bpe()?;

        // 计算每条消息的 token 数
        let msg_tokens: Vec<usize> = history
            .iter()
            .map(|m| encoder.encode_with_special_tokens(&m.content).len())
            .collect();
        let total_tokens: usize = msg_tokens.iter().sum();

        // 未超限，无需压缩
        if total_tokens <= token_limit {
            return Ok(CompactionResult {
                history: history.to_vec(),
                info: None,
            });
        }

        // 最近消息预算：token_limit 的 2/3
        let recent_budget = token_limit * 2 / 3;

        // 从末尾向前保留尽可能多的最近消息
        let mut recent_start = history.len();
        let mut recent_tokens = 0usize;
        for i in (0..history.len()).rev() {
            if recent_tokens.saturating_add(msg_tokens[i]) > recent_budget {
                break;
            }
            recent_start = i;
            recent_tokens += msg_tokens[i];
        }

        // 旧消息 = [0..recent_start)，待压缩
        let old_messages = &history[..recent_start];

        // 至少保留最后一条消息（防止 recent_start == history.len() 的边界情况）
        if recent_start >= history.len() {
            recent_start = history.len().saturating_sub(1);
        }
        let recent_messages = &history[recent_start..];

        // 尝试 LLM 摘要生成，失败时降级为截断提示
        let summary_msg = match self.generate_summary(old_messages).await {
            Ok(summary) => ChatMessage {
                id: None,
                role: "system".to_string(),
                content: format!("[对话历史摘要] {summary}"),
                sources: None,
                reasoning: None,
                turn_group: None,
                version: None,
            },
            Err(e) => {
                // 降级：LLM 摘要失败，使用截断提示替代
                eprintln!("CompactionEngine: LLM 摘要生成失败，降级为截断: {e:#}");
                ChatMessage {
                    id: None,
                    role: "system".to_string(),
                    content: format!(
                        "{FALLBACK_NOTICE}已省略 {old_count} 条早期对话消息（摘要生成失败）。",
                        old_count = old_messages.len()
                    ),
                    sources: None,
                    reasoning: None,
                    turn_group: None,
                    version: None,
                }
            }
        };

        // 计算压缩后 token 数
        let summary_tokens = encoder
            .encode_with_special_tokens(&summary_msg.content)
            .len();
        let compacted_tokens = summary_tokens + recent_tokens;

        // 组装压缩后历史：[摘要 system 消息] + [最近消息]
        let mut compacted_history = vec![summary_msg];
        compacted_history.extend(recent_messages.iter().cloned());

        Ok(CompactionResult {
            history: compacted_history,
            info: Some(CompactionInfo {
                compacted_count: old_messages.len(),
                total_tokens,
                compacted_tokens,
                token_limit,
                compaction_kind: None,
            }),
        })
    }

    /// 使用 TokenBudgetConfig 进行可配置压缩（S69 增强）。
    ///
    /// 与 `compact` 的区别：
    /// - 使用 `TokenBudgetConfig` 控制阈值/保留比/最小消息数
    /// - 支持增量压缩（合并已有摘要）
    /// - 适用于 Agent in-loop compaction 场景
    ///
    /// # 参数
    /// - `history` — 完整历史消息列表
    /// - `config` — Token 预算配置
    /// - `prev_summary` — 上一次压缩的摘要（None = 首次压缩，增量压缩时传入已有摘要）
    pub async fn compact_with_config(
        &self,
        history: &[ChatMessage],
        config: &echomind_models::TokenBudgetConfig,
        prev_summary: Option<&str>,
    ) -> anyhow::Result<CompactionResult> {
        // 空历史或短历史无需压缩
        if history.len() < config.min_messages_to_compact {
            return Ok(CompactionResult {
                history: history.to_vec(),
                info: None,
            });
        }

        let encoder = bpe()?;
        let total_tokens: usize = history
            .iter()
            .map(|m| encoder.encode_with_special_tokens(&m.content).len() + 4)
            .sum();

        // 未达压缩阈值，无需压缩
        if !config.should_compact(total_tokens, history.len()) {
            return Ok(CompactionResult {
                history: history.to_vec(),
                info: None,
            });
        }

        // 使用 config 的 recent_keep_ratio 计算保留预算
        let recent_budget = config.recent_budget();
        let recent_start = crate::token_budget::find_compaction_boundary(history, recent_budget)?;

        let old_messages = &history[..recent_start];
        let recent_messages = &history[recent_start..];

        if old_messages.is_empty() {
            return Ok(CompactionResult {
                history: history.to_vec(),
                info: None,
            });
        }

        // 构建摘要输入（合并已有摘要）
        let summary_input = crate::token_budget::build_summary_input(prev_summary, old_messages);

        // 尝试 LLM 摘要生成
        let summary_msg = match self.generate_summary_text(&summary_input).await {
            Ok(summary) => ChatMessage {
                id: None,
                role: "system".to_string(),
                content: format!("[对话历史摘要] {summary}"),
                sources: None,
                reasoning: None,
                turn_group: None,
                version: None,
            },
            Err(e) => {
                eprintln!("CompactionEngine: LLM 摘要生成失败，降级为截断: {e:#}");
                ChatMessage {
                    id: None,
                    role: "system".to_string(),
                    content: format!(
                        "{FALLBACK_NOTICE}已省略 {old_count} 条早期对话消息（摘要生成失败）。",
                        old_count = old_messages.len()
                    ),
                    sources: None,
                    reasoning: None,
                    turn_group: None,
                    version: None,
                }
            }
        };

        let summary_tokens = encoder
            .encode_with_special_tokens(&summary_msg.content)
            .len();
        let recent_tokens: usize = recent_messages
            .iter()
            .map(|m| encoder.encode_with_special_tokens(&m.content).len() + 4)
            .sum();
        let compacted_tokens = summary_tokens + recent_tokens;

        let mut compacted_history = vec![summary_msg];
        compacted_history.extend(recent_messages.iter().cloned());

        Ok(CompactionResult {
            history: compacted_history,
            info: Some(CompactionInfo {
                compacted_count: old_messages.len(),
                total_tokens,
                compacted_tokens,
                token_limit: config.max_tokens,
                compaction_kind: None,
            }),
        })
    }

    /// 调用 LLM 生成对话历史摘要（内部方法，接收文本输入）。
    async fn generate_summary_text(&self, conversation_text: &str) -> anyhow::Result<String> {
        let stream: BoxStream<'static, anyhow::Result<String>> = self
            .llm
            .chat_stream(
                STRUCTURED_SUMMARY_TEMPLATE,
                &[],
                &format!("请压缩以下对话历史：\n\n{conversation_text}"),
            )
            .await?;

        let mut summary = String::new();
        tokio::pin!(stream);
        while let Some(token_result) = stream.next().await {
            match token_result {
                Ok(token) => summary.push_str(&token),
                Err(e) => return Err(e),
            }
        }

        let trimmed = summary.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("LLM 摘要为空"));
        }
        Ok(trimmed.to_string())
    }

    /// 调用 LLM 生成对话历史摘要。
    ///
    /// 将旧消息拼接为文本，通过 `chat_stream` 发送给 LLM，
    /// 收集流式输出为完整摘要字符串。
    async fn generate_summary(&self, messages: &[ChatMessage]) -> anyhow::Result<String> {
        let mut conversation_text = String::new();
        for msg in messages {
            conversation_text.push_str(&format!("{}: {}\n", msg.role, msg.content));
        }

        let stream: BoxStream<'static, anyhow::Result<String>> = self
            .llm
            .chat_stream(
                STRUCTURED_SUMMARY_TEMPLATE,
                &[],
                &format!("请压缩以下对话历史：\n\n{conversation_text}"),
            )
            .await?;

        // 收集流式输出为完整字符串
        let mut summary = String::new();
        tokio::pin!(stream);
        while let Some(token_result) = stream.next().await {
            match token_result {
                Ok(token) => summary.push_str(&token),
                Err(e) => return Err(e),
            }
        }

        let trimmed = summary.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("LLM 摘要为空"));
        }
        Ok(trimmed.to_string())
    }

    // ============================================================
    // Q03: 双阈值压缩判断 + 后台异步压缩调度
    // ============================================================

    /// 检查是否需要压缩（借鉴 QM `overBudgetFraction` + `COMPACT_SOFT/HARD_FRACTION`）。
    ///
    /// 双阈值判断逻辑：
    /// - `entry_ratio` = history.len() / max_entries
    /// - `token_ratio` = estimated_tokens / max_tokens（粗估，4 字符 ≈ 1 token）
    /// - `max_ratio` = max(entry_ratio, token_ratio)
    /// - `max_ratio >= hard_threshold` → `Synchronous`
    /// - `max_ratio >= soft_threshold` → `Background`
    /// - 否则 → `None`
    ///
    /// # 参数
    /// - `history` — 完整历史消息列表
    /// - `config` — 双阈值配置
    ///
    /// # 返回
    /// `CompactionTrigger` 枚举，指示是否需要压缩及压缩类型。
    pub fn check_compaction_needed(
        &self,
        history: &[ChatMessage],
        config: &DualThresholdConfig,
    ) -> CompactionTrigger {
        // 防止除零（max_entries 或 max_tokens 为 0 时返回 None）
        if config.max_entries == 0 || config.max_tokens == 0 {
            return CompactionTrigger::None;
        }

        let entry_ratio = history.len() as f64 / config.max_entries as f64;
        let estimated_tokens = estimate_history_tokens(history);
        let token_ratio = estimated_tokens as f64 / config.max_tokens as f64;
        let max_ratio = entry_ratio.max(token_ratio);

        if max_ratio >= config.hard_threshold {
            CompactionTrigger::Synchronous
        } else if max_ratio >= config.soft_threshold {
            CompactionTrigger::Background
        } else {
            CompactionTrigger::None
        }
    }
}

/// 后台异步压缩：去重获取（借鉴 QM `scheduleBackgroundCompaction()`）。
///
/// 检查是否已有同一会话的后台压缩在进行。若无则标记为进行中。
///
/// **设计决策**：不在 compact crate 中 `tokio::spawn`，因为 `LLMProvider` trait
/// 使用 native `async fn`（Edition 2024），其返回 future 不保证 `Send`。
/// 调用方（如 `chat_inner`）知道具体的 LLM 类型（如 `OpenAIProvider`），
/// 可以在那些类型上安全地 `tokio::spawn`。
///
/// # 参数
/// - `conversation_id` — 会话 ID（用于去重）
/// - `pending` — 去重集合
///
/// # 返回
/// - `true` — 成功获取，调用方应执行压缩并在完成后调用 `release_background_compaction`
/// - `false` — 已有后台压缩在进行，跳过
pub async fn try_acquire_background_compaction(
    conversation_id: &str,
    pending: Arc<tokio::sync::Mutex<HashSet<String>>>,
) -> bool {
    let mut pending_set = pending.lock().await;
    if pending_set.contains(conversation_id) {
        eprintln!("[COMPACTION] 会话 {conversation_id} 已有后台压缩在进行，跳过");
        return false;
    }
    pending_set.insert(conversation_id.to_string());
    true
}

/// 释放后台压缩 pending 标记（压缩完成后调用）。
pub async fn release_background_compaction(
    conversation_id: &str,
    pending: Arc<tokio::sync::Mutex<HashSet<String>>>,
) {
    let mut pending_set = pending.lock().await;
    pending_set.remove(conversation_id);
}

/// 执行后台压缩（不 spawn，直接 await）。
///
/// 错误被吞咽（仅 `eprintln!` 日志），不传播错误，不影响对话流程。
/// 调用方负责：
/// 1. 先调用 `try_acquire_background_compaction()` 去重
/// 2. 获取成功后 spawn 本函数（或直接 await）
/// 3. 完成后调用 `release_background_compaction()` 清除标记
///
/// # 参数
/// - `conversation_id` — 会话 ID（用于日志）
/// - `history` — 完整历史消息（切片引用）
/// - `max_tokens` — Token 上限
/// - `llm` — LLM Provider 引用
pub async fn run_background_compaction<L: LLMProvider>(
    conversation_id: &str,
    history: &[ChatMessage],
    max_tokens: usize,
    llm: &L,
) {
    let engine = CompactionEngine::new(llm);
    match engine.compact(history, max_tokens).await {
        Ok(result) => {
            if let Some(info) = &result.info {
                eprintln!(
                    "[COMPACTION] 后台压缩完成（会话 {}）：{} 条消息被压缩，{} → {} tokens",
                    conversation_id, info.compacted_count, info.total_tokens, info.compacted_tokens
                );
            } else {
                eprintln!(
                    "[COMPACTION] 后台压缩完成（会话 {}）：无需压缩",
                    conversation_id
                );
            }
        }
        Err(e) => {
            eprintln!(
                "[COMPACTION] 后台压缩失败（会话 {}，不影响对话）：{e:#}",
                conversation_id
            );
        }
    }
}

#[cfg(test)]
mod compact_tests;
