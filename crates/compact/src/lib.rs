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

use echomind_models::{ChatMessage, CompactionInfo, CompactionResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use echomind_core::LLMProvider;
use echomind_core::splitter::bpe;

/// Token 预算驱动的上下文压缩增强。
pub mod token_budget;

// 重导出常用类型
pub use token_budget::{
    IncrementalCompactor, build_summary_input, estimate_messages_tokens, estimate_tokens,
    extract_summary_from_result, find_compaction_boundary, needs_compaction,
};

/// 结构化压缩模板。
///
/// 使用 6 章节结构化 Markdown 模板引导 LLM 生成格式一致的摘要。
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

    /// 使用 TokenBudgetConfig 进行可配置压缩。
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
}

#[cfg(test)]
mod compact_tests;
