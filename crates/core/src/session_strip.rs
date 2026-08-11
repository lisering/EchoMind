//! Session Strip（会话条带化）— REQ-RAG-046。
//!
//! 借鉴 ds4 (DwarfStar) `/strip` 命令：从已保存的会话中移除数据以减少上下文消耗。
//!
//! ## ds4 原始概念
//!
//! ds4 的 `/strip <sha>` 命令从已保存的会话中移除重型 KV 缓存载荷（payload_bytes=0），
//! 同时保留渲染的文本转录（text + title）。加载 stripped 会话时，从文本重建 KV。
//!
//! ## EchoMind 适配
//!
//! EchoMind 不存储 KV 缓存载荷，但消息历史本身占用上下文窗口。
//! Session Strip 允许用户：
//!
//! 1. **按范围移除**：删除 `[from_index, to_index]` 闭区间内的消息
//! 2. **保留近期**：只保留最后 N 条消息，移除其余全部
//! 3. **摘要替代**：可选地插入一条 system 消息作为被移除内容的摘要
//! 4. **预览**：查看将被移除的消息列表和 token 估算，不执行实际删除
//!
//! ## 零新增依赖
//!
//! 复用 `Storage` trait + `echomind_models` 数据模型。

use anyhow::Result;
use echomind_models::{ChatMessage, StripConfig, StripPreview, StripResult};

use crate::Storage;

/// Session Stripper — 会话消息条带化引擎。
///
/// 借鉴 ds4 (DwarfStar) `/strip` 命令，适配为 EchoMind 的消息级操作。
/// 所有方法均为异步泛型函数，接受 `Storage` trait 实现。
pub struct SessionStripper;

impl SessionStripper {
    /// 按索引范围 strip 消息（REQ-RAG-046 AC-1/AC-2）。
    ///
    /// 删除 `conversation_id` 会话中 `[from_index, to_index]` 闭区间内的消息。
    /// 如果 `config.replace_with_summary` 为 `true` 且 `summary_text` 非空，
    /// 则插入一条 system 消息作为摘要替代。
    ///
    /// # 参数
    /// - `storage`：存储实现
    /// - `conversation_id`：会话 ID
    /// - `config`：strip 配置
    ///
    /// # 返回
    /// `StripResult` 记录删除数量、摘要插入状态、被删除消息 ID、token 估算。
    ///
    /// # 错误
    /// - 索引越界（`from_index > to_index` 或 `to_index >= total_messages`）
    pub async fn strip_range<S: Storage>(
        storage: &S,
        conversation_id: &str,
        config: &StripConfig,
    ) -> Result<StripResult> {
        let messages = storage.list_messages(conversation_id).await?;
        let total = messages.len();

        // 空对话：返回空结果
        if total == 0 {
            return Ok(StripResult::empty());
        }

        // 范围校验
        Self::validate_range(total, config.from_index, config.to_index)?;

        // 收集要删除的消息
        let to_strip: Vec<&ChatMessage> = messages[config.from_index..=config.to_index]
            .iter()
            .collect();

        let stripped_count = to_strip.len();
        let stripped_message_ids: Vec<String> =
            to_strip.iter().filter_map(|m| m.id.clone()).collect();

        // 估算 token 节省
        let estimated_tokens_saved: usize = to_strip
            .iter()
            .map(|m| Self::estimate_tokens(&m.content))
            .sum();

        // 执行删除
        if !stripped_message_ids.is_empty() {
            storage
                .delete_messages_by_ids(conversation_id, &stripped_message_ids)
                .await?;
        }

        // 可选：插入摘要 system 消息
        let summary_inserted = if config.replace_with_summary {
            if let Some(summary) = &config.summary_text {
                let summary_msg = ChatMessage {
                    id: None,
                    role: "system".to_string(),
                    content: format!("[摘要] {summary}"),
                    sources: None,
                    reasoning: None,
                    turn_group: None,
                    version: None,
                };
                storage.add_message(conversation_id, &summary_msg).await?;
                true
            } else {
                false
            }
        } else {
            false
        };

        Ok(StripResult {
            stripped_count,
            summary_inserted,
            stripped_message_ids,
            estimated_tokens_saved,
            summary: config.summary_text.clone().unwrap_or_default(),
            kept_count: total.saturating_sub(stripped_count),
        })
    }

    /// 保留最后 N 条消息，strip 其余全部（REQ-RAG-046 AC-3/AC-4）。
    ///
    /// 删除 `conversation_id` 会话中除最后 `keep_last_n` 条以外的所有消息。
    /// 如果 `replace_with_summary` 为 `true` 且 `summary_text` 非空，
    /// 则插入一条 system 消息作为摘要。
    ///
    /// # 参数
    /// - `storage`：存储实现
    /// - `conversation_id`：会话 ID
    /// - `keep_last_n`：保留的最后消息数量（0 = 删除全部）
    /// - `replace_with_summary`：是否插入摘要
    /// - `summary_text`：摘要文本（`None` 时不插入）
    pub async fn strip_keeping_recent<S: Storage>(
        storage: &S,
        conversation_id: &str,
        keep_last_n: usize,
        replace_with_summary: bool,
        summary_text: Option<&str>,
    ) -> Result<StripResult> {
        let messages = storage.list_messages(conversation_id).await?;
        let total = messages.len();

        // 空对话或保留全部：返回空结果
        if total == 0 || keep_last_n >= total {
            return Ok(StripResult::empty());
        }

        // 计算要删除的范围 [0, total - keep_last_n - 1]
        let to_index = total.saturating_sub(keep_last_n).saturating_sub(1);

        let config = StripConfig {
            from_index: 0,
            to_index,
            replace_with_summary,
            summary_text: summary_text.map(|s| s.to_string()),
        };

        Self::strip_range(storage, conversation_id, &config).await
    }

    /// 预览 strip 效果，不执行实际删除（REQ-RAG-046 AC-5）。
    ///
    /// 返回将被 strip 的消息列表和 token 估算。
    ///
    /// # 参数
    /// - `storage`：存储实现
    /// - `conversation_id`：会话 ID
    /// - `from_index`：起始消息索引（包含）
    /// - `to_index`：结束消息索引（包含）
    ///
    /// # 返回
    /// `StripPreview` 包含将被 strip 的消息、总消息数、token 估算。
    pub async fn preview<S: Storage>(
        storage: &S,
        conversation_id: &str,
        from_index: usize,
        to_index: usize,
    ) -> Result<StripPreview> {
        let messages = storage.list_messages(conversation_id).await?;
        let total = messages.len();

        // 空对话：返回空预览
        if total == 0 {
            return Ok(StripPreview::empty(total));
        }

        // 范围校验
        Self::validate_range(total, from_index, to_index)?;

        // 收集预览消息
        let preview_messages: Vec<ChatMessage> = messages[from_index..=to_index].to_vec();

        let estimated_tokens_saved: usize = preview_messages
            .iter()
            .map(|m| Self::estimate_tokens(&m.content))
            .sum();

        Ok(StripPreview {
            messages: preview_messages,
            total_messages: total,
            estimated_tokens_saved,
        })
    }

    /// 估算 token 数量（REQ-RAG-046 AC-8）。
    ///
    /// 使用粗略估算：4 字符 ≈ 1 token。
    /// 这是 GPT tokenizer 的常见近似值，适用于快速估算。
    ///
    /// # 参数
    /// - `text`：要估算的文本
    ///
    /// # 返回
    /// 估算的 token 数量（至少 0）
    pub fn estimate_tokens(text: &str) -> usize {
        text.len() / 4
    }

    /// 验证 strip 范围（REQ-RAG-046 AC-6）。
    ///
    /// # 参数
    /// - `total`：消息总数
    /// - `from_index`：起始索引
    /// - `to_index`：结束索引
    ///
    /// # 错误
    /// - `from_index > to_index`
    /// - `to_index >= total`
    fn validate_range(total: usize, from_index: usize, to_index: usize) -> Result<()> {
        if from_index > to_index {
            anyhow::bail!("无效的 strip 范围：from_index ({from_index}) > to_index ({to_index})");
        }
        if to_index >= total {
            anyhow::bail!("索引越界：to_index ({to_index}) >= 消息总数 ({total})");
        }
        Ok(())
    }
}
