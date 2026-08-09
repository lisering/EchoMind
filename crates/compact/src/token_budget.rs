//! Token 预算驱动的上下文压缩增强（S69：Cherry Studio 借鉴）。
//!
//! 在现有 `CompactionEngine` 基础上增强：
//! 1. 可配置 `TokenBudgetConfig`（阈值/保留比/最小消息数）
//! 2. 增量压缩：不每次重新摘要全部旧消息，合并已有摘要 + 新旧消息
//! 3. In-loop compaction：Agent 多步推理中每步检查预算，超阈值时动态压缩
//! 4. Token 估算工具函数（公共 API）

use echomind_core::splitter::bpe;
use echomind_models::{ChatMessage, CompactionResult, TokenBudgetConfig};

/// 估算文本的 token 数（使用 tiktoken BPE cl100k_base）。
///
/// # 错误
/// BPE 编码器初始化失败时返回 Err。
pub fn estimate_tokens(text: &str) -> anyhow::Result<usize> {
    let encoder = bpe()?;
    Ok(encoder.encode_with_special_tokens(text).len())
}

/// 估算消息列表的总 token 数。
///
/// 每条消息的 token 数 = content 的 BPE token 数 + 4（角色/分隔符开销）。
pub fn estimate_messages_tokens(messages: &[ChatMessage]) -> anyhow::Result<usize> {
    let encoder = bpe()?;
    let mut total = 0usize;
    for msg in messages {
        // 每条消息额外 4 token 开销（role + delimiter，参照 OpenAI 公式）
        total += encoder.encode_with_special_tokens(&msg.content).len() + 4;
    }
    Ok(total)
}

/// 查找压缩边界：将历史分为「待压缩旧消息」+「保留的最近消息」。
///
/// 从末尾向前累积 token，直到达到 `recent_budget` 为止。
/// 返回 `(old_messages, recent_messages)` 的分割索引。
pub fn find_compaction_boundary(
    messages: &[ChatMessage],
    recent_budget: usize,
) -> anyhow::Result<usize> {
    let encoder = bpe()?;
    let msg_tokens: Vec<usize> = messages
        .iter()
        .map(|m| encoder.encode_with_special_tokens(&m.content).len() + 4)
        .collect();

    let mut recent_start = messages.len();
    let mut recent_tokens = 0usize;
    for i in (0..messages.len()).rev() {
        if recent_tokens.saturating_add(msg_tokens[i]) > recent_budget {
            break;
        }
        recent_start = i;
        recent_tokens += msg_tokens[i];
    }

    // 至少保留最后一条消息
    if recent_start >= messages.len() {
        recent_start = messages.len().saturating_sub(1);
    }

    Ok(recent_start)
}

/// 判断当前历史是否需要压缩（基于 TokenBudgetConfig）。
pub fn needs_compaction(
    history: &[ChatMessage],
    config: &TokenBudgetConfig,
) -> anyhow::Result<bool> {
    if history.len() < config.min_messages_to_compact {
        return Ok(false);
    }
    let total = estimate_messages_tokens(history)?;
    Ok(config.should_compact(total, history.len()))
}

/// 增量压缩管理器：跟踪已有摘要，避免每次重新摘要全部旧消息。
///
/// 在 Agent 多步推理循环中，每步可能新增消息。
/// 增量压缩器记住上一次压缩的摘要和边界，只对新产生的旧消息进行摘要，
/// 然后合并已有摘要，显著减少 LLM 调用次数。
#[derive(Debug, Clone)]
pub struct IncrementalCompactor {
    /// 上一次压缩生成的摘要文本（None = 尚未压缩过）
    prev_summary: Option<String>,
    /// 上一次压缩时保留的最近消息起始索引
    prev_boundary: usize,
    /// 压缩配置
    config: TokenBudgetConfig,
}

impl IncrementalCompactor {
    /// 创建增量压缩器。
    pub fn new(config: TokenBudgetConfig) -> Self {
        Self {
            prev_summary: None,
            prev_boundary: 0,
            config,
        }
    }

    /// 获取已有摘要引用（用于合并）。
    pub fn prev_summary(&self) -> Option<&str> {
        self.prev_summary.as_deref()
    }

    /// 判断是否需要压缩。
    pub fn should_compact(&self, history: &[ChatMessage]) -> anyhow::Result<bool> {
        needs_compaction(history, &self.config)
    }

    /// 更新压缩状态（压缩完成后调用）。
    pub fn update(&mut self, summary: String, boundary: usize) {
        self.prev_summary = Some(summary);
        self.prev_boundary = boundary;
    }

    /// 重置状态（新对话开始时调用）。
    pub fn reset(&mut self) {
        self.prev_summary = None;
        self.prev_boundary = 0;
    }
}

/// 构建 LLM 摘要请求的输入文本。
///
/// 如果已有上一次摘要，将旧摘要 + 新旧消息一起拼接，
/// 使 LLM 在已有摘要基础上增量更新。
pub fn build_summary_input(prev_summary: Option<&str>, new_old_messages: &[ChatMessage]) -> String {
    let mut text = String::new();

    // 如果有已有摘要，作为前缀注入
    if let Some(summary) = prev_summary {
        text.push_str("[已有对话摘要]\n");
        text.push_str(summary);
        text.push_str("\n\n[新增对话历史]\n");
    }

    for msg in new_old_messages {
        text.push_str(&format!("{}: {}\n", msg.role, msg.content));
    }

    text
}

/// 从 CompactionResult 中提取压缩信息摘要。
pub fn extract_summary_from_result(result: &CompactionResult) -> Option<&str> {
    if result.info.is_some() {
        // 摘要是 history[0] 的 content（去掉 "[对话历史摘要] " 前缀）
        result.history.first().map(|m| {
            m.content
                .strip_prefix("[对话历史摘要] ")
                .unwrap_or(&m.content)
        })
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use echomind_models::CompactionInfo;

    fn make_msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            id: None,
            role: role.to_string(),
            content: content.to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: None,
        }
    }

    #[test]
    fn test_estimate_tokens_non_empty() {
        let tokens = estimate_tokens("Hello world, this is a test.").unwrap();
        assert!(tokens > 0);
        assert!(tokens < 20);
    }

    #[test]
    fn test_estimate_tokens_empty() {
        let tokens = estimate_tokens("").unwrap();
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_messages_tokens() {
        let msgs = vec![
            make_msg("user", "Hello"),
            make_msg("assistant", "Hi there! How can I help you?"),
        ];
        let tokens = estimate_messages_tokens(&msgs).unwrap();
        // 每条消息至少 4 token 开销 + content token
        assert!(tokens > 8);
    }

    #[test]
    fn test_find_compaction_boundary_short_history() {
        let msgs = vec![make_msg("user", "Hi"), make_msg("assistant", "Hello!")];
        let boundary = find_compaction_boundary(&msgs, 1000).unwrap();
        // 短历史应全部保留
        assert_eq!(boundary, 0);
    }

    #[test]
    fn test_find_compaction_boundary_long_history() {
        let msgs: Vec<ChatMessage> = (0..10)
            .map(|i| {
                make_msg(
                    "user",
                    &format!("Message number {i} with some content to make it longer"),
                )
            })
            .collect();
        let boundary = find_compaction_boundary(&msgs, 50).unwrap();
        // 应该保留少于全部消息
        assert!(boundary > 0);
        assert!(boundary < 10);
    }

    #[test]
    fn test_find_compaction_boundary_at_least_one() {
        let msgs = vec![
            make_msg(
                "user",
                "This is a very long message that exceeds the budget on its own "
                    .repeat(10)
                    .as_str(),
            ),
            make_msg("assistant", "Short reply"),
        ];
        let boundary = find_compaction_boundary(&msgs, 5).unwrap();
        // 至少保留最后一条
        assert_eq!(boundary, 1);
    }

    #[test]
    fn test_needs_compaction_short_history() {
        let msgs = vec![make_msg("user", "Hi")];
        let config = TokenBudgetConfig::default();
        assert!(!needs_compaction(&msgs, &config).unwrap());
    }

    #[test]
    fn test_needs_compaction_below_threshold() {
        let msgs = vec![
            make_msg("user", "Short message"),
            make_msg("assistant", "Short reply"),
            make_msg("user", "Another short message"),
        ];
        let config = TokenBudgetConfig {
            max_tokens: 10000,
            ..Default::default()
        };
        assert!(!needs_compaction(&msgs, &config).unwrap());
    }

    #[test]
    fn test_needs_compaction_above_threshold() {
        let long_text = "This is a long message. ".repeat(100);
        let msgs = vec![
            make_msg("user", &long_text),
            make_msg("assistant", &long_text),
            make_msg("user", &long_text),
        ];
        let config = TokenBudgetConfig {
            max_tokens: 100,
            compaction_threshold: 0.5,
            min_messages_to_compact: 3,
            ..Default::default()
        };
        assert!(needs_compaction(&msgs, &config).unwrap());
    }

    #[test]
    fn test_token_budget_config_default() {
        let config = TokenBudgetConfig::default();
        assert_eq!(config.compaction_threshold, 0.8);
        assert_eq!(config.recent_keep_ratio, 0.67);
        assert_eq!(config.min_messages_to_compact, 3);
    }

    #[test]
    fn test_token_budget_config_trigger() {
        let config = TokenBudgetConfig {
            max_tokens: 1000,
            compaction_threshold: 0.8,
            ..Default::default()
        };
        assert_eq!(config.compaction_trigger(), 800);
    }

    #[test]
    fn test_token_budget_config_recent_budget() {
        let config = TokenBudgetConfig {
            max_tokens: 3000,
            recent_keep_ratio: 0.67,
            ..Default::default()
        };
        assert_eq!(config.recent_budget(), 2010);
    }

    #[test]
    fn test_should_compact_method() {
        let config = TokenBudgetConfig {
            max_tokens: 1000,
            compaction_threshold: 0.8,
            min_messages_to_compact: 3,
            ..Default::default()
        };
        // total_tokens = 700, message_count = 5 → 700 < 800, false
        assert!(!config.should_compact(700, 5));
        // total_tokens = 900, message_count = 5 → 900 > 800, true
        assert!(config.should_compact(900, 5));
        // total_tokens = 900, message_count = 2 → 2 < 3, false
        assert!(!config.should_compact(900, 2));
    }

    #[test]
    fn test_incremental_compactor_new() {
        let config = TokenBudgetConfig::default();
        let compactor = IncrementalCompactor::new(config);
        assert!(compactor.prev_summary().is_none());
        assert_eq!(compactor.prev_boundary, 0);
    }

    #[test]
    fn test_incremental_compactor_update_and_reset() {
        let config = TokenBudgetConfig::default();
        let mut compactor = IncrementalCompactor::new(config);
        compactor.update("Summary text".to_string(), 5);
        assert_eq!(compactor.prev_summary(), Some("Summary text"));
        assert_eq!(compactor.prev_boundary, 5);
        compactor.reset();
        assert!(compactor.prev_summary().is_none());
        assert_eq!(compactor.prev_boundary, 0);
    }

    #[test]
    fn test_build_summary_input_without_prev() {
        let msgs = vec![make_msg("user", "Hello"), make_msg("assistant", "Hi there")];
        let text = build_summary_input(None, &msgs);
        assert!(text.contains("user: Hello"));
        assert!(text.contains("assistant: Hi there"));
        assert!(!text.contains("[已有对话摘要]"));
    }

    #[test]
    fn test_build_summary_input_with_prev() {
        let msgs = vec![make_msg("user", "New question")];
        let text = build_summary_input(Some("Previous summary text"), &msgs);
        assert!(text.contains("[已有对话摘要]"));
        assert!(text.contains("Previous summary text"));
        assert!(text.contains("[新增对话历史]"));
        assert!(text.contains("user: New question"));
    }

    #[test]
    fn test_extract_summary_from_result() {
        let result = CompactionResult {
            history: vec![ChatMessage {
                id: None,
                role: "system".to_string(),
                content: "[对话历史摘要] This is the summary".to_string(),
                sources: None,
                reasoning: None,
                turn_group: None,
                version: None,
            }],
            info: Some(CompactionInfo {
                compacted_count: 3,
                total_tokens: 1000,
                compacted_tokens: 500,
                token_limit: 800,
                compaction_kind: None,
            }),
        };
        let summary = extract_summary_from_result(&result);
        assert_eq!(summary, Some("This is the summary"));
    }

    #[test]
    fn test_extract_summary_from_result_none() {
        let result = CompactionResult {
            history: vec![make_msg("user", "test")],
            info: None,
        };
        assert!(extract_summary_from_result(&result).is_none());
    }
}
