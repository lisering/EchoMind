#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 对话历史压缩引擎测试（TC-COMPACT-001~010）。
//!
//! 验证 `CompactionEngine` 在历史超过阈值时将旧消息压缩为摘要（而非丢弃），
//! 替代 `truncate_history` 的纯截断策略。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::ChatMessage;
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::CompactionEngine;
use echomind_core::LLMProvider;

// ============================================================
// Mock LLM Provider
// ============================================================

/// 摘要间谍 LLM：调用 `chat_stream` 时返回固定摘要文本，记录调用次数。
#[derive(Clone, Default)]
struct SummarySpyLlm {
    calls: Arc<AtomicUsize>,
    /// 返回的摘要文本（按 token 分割模拟流式输出）
    summary_text: Arc<String>,
}

impl SummarySpyLlm {
    fn new(summary: &str) -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            summary_text: Arc::new(summary.to_string()),
        }
    }
}

impl LLMProvider for SummarySpyLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let text = self.summary_text.clone();
        // 模拟流式输出：一次性返回全文
        Ok(futures::stream::once(async move { Ok(text.to_string()) }).boxed())
    }
}

/// 错误 LLM：调用时返回错误，用于测试降级路径。
#[derive(Clone, Default)]
struct ErrorLlm;

impl LLMProvider for ErrorLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::once(async move {
            Err(anyhow::anyhow!("LLM 摘要生成失败（模拟）"))
        })
        .boxed())
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 构造用户消息
fn user_msg(content: &str) -> ChatMessage {
    ChatMessage {
        id: None,
        role: "user".to_string(),
        content: content.to_string(),
        sources: None,
        ..Default::default()
    }
}

/// 构造助手消息
fn assistant_msg(content: &str) -> ChatMessage {
    ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: content.to_string(),
        sources: None,
        ..Default::default()
    }
}

/// 构造多轮对话历史（每条消息约 10-15 tokens）
fn make_history(turns: usize) -> Vec<ChatMessage> {
    let mut history = Vec::new();
    for i in 0..turns {
        history.push(user_msg(&format!(
            "这是第 {} 轮对话的用户问题，内容较长用于测试压缩功能。",
            i + 1
        )));
        history.push(assistant_msg(&format!(
            "这是第 {} 轮对话的助手回答，提供了详细的解答和相关的上下文信息。",
            i + 1
        )));
    }
    history
}

// ============================================================
// 测试用例
// ============================================================

/// TC-COMPACT-001：空历史无需压缩，info 为 None。
#[tokio::test]
async fn tc_compact_001_empty_history_no_compaction() {
    let llm = SummarySpyLlm::new("摘要内容");
    let engine = CompactionEngine::new(&llm);
    let result = engine.compact(&[], 4096).await.unwrap();

    assert!(result.history.is_empty(), "空历史应返回空");
    assert!(result.info.is_none(), "空历史无需压缩");
    assert_eq!(llm.calls.load(Ordering::SeqCst), 0, "不应调用 LLM");
}

/// TC-COMPACT-002：历史未超 token 限制时无需压缩，info 为 None。
#[tokio::test]
async fn tc_compact_002_under_limit_no_compaction() {
    let llm = SummarySpyLlm::new("摘要内容");
    let engine = CompactionEngine::new(&llm);
    let history = make_history(3); // 6 条消息，约 60-90 tokens
    let result = engine.compact(&history, 4096).await.unwrap();

    assert_eq!(result.history.len(), history.len(), "未超限应保留全部历史");
    assert!(result.info.is_none(), "未超限无需压缩");
    assert_eq!(llm.calls.load(Ordering::SeqCst), 0, "不应调用 LLM");
}

/// TC-COMPACT-003：短历史（≤ 2 条消息）无需压缩。
#[tokio::test]
async fn tc_compact_003_short_history_no_compaction() {
    let llm = SummarySpyLlm::new("摘要内容");
    let engine = CompactionEngine::new(&llm);
    let history = vec![user_msg("你好"), assistant_msg("你好！有什么可以帮你的？")];
    let result = engine.compact(&history, 100).await.unwrap();

    assert_eq!(result.history.len(), 2, "短历史应保留全部");
    assert!(result.info.is_none(), "短历史无需压缩");
    assert_eq!(llm.calls.load(Ordering::SeqCst), 0, "不应调用 LLM");
}

/// TC-COMPACT-004：历史超过限制时触发压缩，生成摘要 system 消息。
#[tokio::test]
async fn tc_compact_004_exceeds_limit_triggers_compaction() {
    let llm = SummarySpyLlm::new("用户讨论了EchoMind的架构设计和功能实现。");
    let engine = CompactionEngine::new(&llm);
    let history = make_history(20); // 40 条消息，远超 200 token 限制
    let result = engine.compact(&history, 200).await.unwrap();

    let info = result.info.expect("超限应触发压缩，info 不为 None");
    assert!(
        info.compacted_count > 0,
        "应有消息被压缩，实际 compacted_count = {}",
        info.compacted_count
    );
    assert!(
        info.total_tokens > info.compacted_tokens,
        "压缩后 token 数应小于原始（{} > {}）",
        info.total_tokens,
        info.compacted_tokens
    );
    assert_eq!(
        llm.calls.load(Ordering::SeqCst),
        1,
        "应调用 LLM 恰好一次生成摘要"
    );
}

/// TC-COMPACT-005：压缩后历史首条消息为 system 角色的摘要。
#[tokio::test]
async fn tc_compact_005_summary_is_system_message() {
    let llm = SummarySpyLlm::new("这是对话摘要。");
    let engine = CompactionEngine::new(&llm);
    let history = make_history(15);
    let result = engine.compact(&history, 200).await.unwrap();

    assert!(!result.history.is_empty(), "压缩后历史不应为空");
    let first = &result.history[0];
    assert_eq!(
        first.role, "system",
        "首条消息应为 system 角色（摘要），实际为 {}",
        first.role
    );
    assert!(
        first.content.contains("对话摘要") || first.content.contains("这是对话摘要"),
        "摘要消息内容应包含摘要文本，实际: {}",
        first.content
    );
}

/// TC-COMPACT-006：压缩后最近的消息被保留。
#[tokio::test]
async fn tc_compact_006_recent_messages_preserved() {
    let llm = SummarySpyLlm::new("摘要");
    let engine = CompactionEngine::new(&llm);
    let history = make_history(15);
    let original_len = history.len();
    let result = engine.compact(&history, 200).await.unwrap();

    // 压缩后应包含摘要 + 部分最近消息
    assert!(
        result.history.len() > 1,
        "压缩后应包含摘要 + 至少一条最近消息"
    );

    // 最后一条消息应与原始历史最后一条一致
    let last_original = history.last().unwrap();
    let last_compacted = result.history.last().unwrap();
    assert_eq!(
        last_compacted.role, last_original.role,
        "最后一条消息角色应一致"
    );
    assert_eq!(
        last_compacted.content, last_original.content,
        "最后一条消息内容应一致（最近消息原样保留）"
    );

    // 压缩后总消息数应小于原始
    assert!(
        result.history.len() < original_len,
        "压缩后消息数应小于原始（{} < {}）",
        result.history.len(),
        original_len
    );
}

/// TC-COMPACT-007：压缩后总 token 数低于限制阈值。
#[tokio::test]
async fn tc_compact_007_compacted_tokens_under_limit() {
    let llm = SummarySpyLlm::new("简短摘要");
    let engine = CompactionEngine::new(&llm);
    let history = make_history(30);
    let token_limit = 300;
    let result = engine.compact(&history, token_limit).await.unwrap();

    let info = result.info.expect("应触发压缩");
    assert!(
        info.compacted_tokens <= token_limit,
        "压缩后 token 数应 ≤ 限制（{} ≤ {}）",
        info.compacted_tokens,
        token_limit
    );
}

/// TC-COMPACT-008：LLM 摘要失败时降级为截断（不丢失最近消息）。
#[tokio::test]
async fn tc_compact_008_llm_failure_fallback_to_truncation() {
    let llm = ErrorLlm;
    let engine = CompactionEngine::new(&llm);
    let history = make_history(20);
    let result = engine.compact(&history, 200).await;

    // LLM 失败时应降级为截断，不返回 Err
    let result = result.expect("LLM 失败应降级为截断，不返回错误");
    assert!(!result.history.is_empty(), "降级截断后历史不应为空");

    // 降级时 info 应为 None（截断不是压缩）
    // 或者 info 存在但 compacted_count 反映截断行为
    // 设计决策：降级时仍返回 info，但摘要为截断提示
    let info = result.info.as_ref();
    if let Some(info) = info {
        // 如果有 info，说明触发了处理
        assert!(info.compacted_count > 0, "降级截断也应记录被处理的旧消息数");
    }
}

/// TC-COMPACT-009：CompactionInfo 字段完整且正确。
#[tokio::test]
async fn tc_compact_009_compaction_info_fields_correct() {
    let llm = SummarySpyLlm::new("摘要文本");
    let engine = CompactionEngine::new(&llm);
    let history = make_history(25);
    let token_limit = 250;
    let result = engine.compact(&history, token_limit).await.unwrap();

    let info = result.info.expect("应触发压缩");
    assert_eq!(info.token_limit, token_limit, "token_limit 应与输入一致");
    assert!(
        info.total_tokens > token_limit,
        "total_tokens 应大于 token_limit（{} > {}）",
        info.total_tokens,
        token_limit
    );
    assert!(info.compacted_count > 0, "compacted_count 应大于 0");
    assert!(info.compacted_tokens > 0, "compacted_tokens 应大于 0");
    assert!(
        info.compacted_tokens < info.total_tokens,
        "compacted_tokens 应小于 total_tokens（压缩生效）"
    );
}

/// TC-COMPACT-010：CompactionResult 可序列化/反序列化（IPC 传输兼容）。
#[test]
fn tc_compact_010_compaction_result_serde_roundtrip() {
    use echomind_models::{CompactionInfo, CompactionResult};

    let result = CompactionResult {
        history: vec![
            ChatMessage {
                id: None,
                role: "system".to_string(),
                content: "[对话摘要] 用户讨论了架构设计。".to_string(),
                sources: None,
                ..Default::default()
            },
            ChatMessage {
                id: None,
                role: "user".to_string(),
                content: "最近的问题".to_string(),
                sources: None,
                ..Default::default()
            },
        ],
        info: Some(CompactionInfo {
            compacted_count: 10,
            total_tokens: 5000,
            compacted_tokens: 800,
            token_limit: 4096,
            compaction_kind: None,
        }),
    };

    let json = serde_json::to_string(&result).expect("serialize");
    let back: CompactionResult = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.history.len(), 2);
    let info = back.info.expect("info should exist");
    assert_eq!(info.compacted_count, 10);
    assert_eq!(info.total_tokens, 5000);
    assert_eq!(info.compacted_tokens, 800);
    assert_eq!(info.token_limit, 4096);
    assert!(
        info.compaction_kind.is_none(),
        "旧版兼容 compaction_kind 应为 None"
    );
}

// ============================================================
// Q03: 双阈值压缩 + 后台异步压缩 TDD 测试
// TC-COMPACT-DUAL-001 ~ TC-COMPACT-DUAL-007
// ============================================================

use crate::{
    CompactionTrigger, DualThresholdConfig, release_background_compaction,
    run_background_compaction, try_acquire_background_compaction,
};
use std::collections::HashSet;

/// 构造指定条目数的历史（每条约 20 字符 → ~5 tokens）。
fn make_history_n(turns: usize) -> Vec<ChatMessage> {
    let mut history = Vec::new();
    for i in 0..turns {
        history.push(user_msg(&format!("第 {} 轮用户问题内容测试", i + 1)));
        history.push(assistant_msg(&format!("第 {} 轮助手回答内容详细", i + 1)));
    }
    history
}

/// TC-COMPACT-DUAL-001：历史 < 70% → CompactionTrigger::None。
#[tokio::test]
async fn tc_compact_dual_001_under_soft_threshold_returns_none() {
    let llm = SummarySpyLlm::new("摘要");
    let engine = CompactionEngine::new(&llm);
    let config = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 50,
        max_tokens: 8192,
    };
    // 10 条消息 → entry_ratio = 10/50 = 0.2 < 0.7
    let history = make_history_n(5);
    let trigger = engine.check_compaction_needed(&history, &config);
    assert_eq!(trigger, CompactionTrigger::None, "历史 < 70% 应返回 None");
    assert!(trigger.is_none());
}

/// TC-COMPACT-DUAL-002：历史 70%–90% → CompactionTrigger::Background。
#[tokio::test]
async fn tc_compact_dual_002_between_soft_and_hard_returns_background() {
    let llm = SummarySpyLlm::new("摘要");
    let engine = CompactionEngine::new(&llm);
    let config = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 50,
        max_tokens: 8192,
    };
    // 40 条消息 → entry_ratio = 40/50 = 0.8, 0.7 <= 0.8 < 0.9
    let history = make_history_n(20);
    let trigger = engine.check_compaction_needed(&history, &config);
    assert_eq!(
        trigger,
        CompactionTrigger::Background,
        "历史 70%-90% 应返回 Background"
    );
    assert!(trigger.is_background());
}

/// TC-COMPACT-DUAL-003：历史 > 90% → CompactionTrigger::Synchronous。
#[tokio::test]
async fn tc_compact_dual_003_above_hard_threshold_returns_synchronous() {
    let llm = SummarySpyLlm::new("摘要");
    let engine = CompactionEngine::new(&llm);
    let config = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 50,
        max_tokens: 8192,
    };
    // 48 条消息 → entry_ratio = 48/50 = 0.96 > 0.9
    let history = make_history_n(24);
    let trigger = engine.check_compaction_needed(&history, &config);
    assert_eq!(
        trigger,
        CompactionTrigger::Synchronous,
        "历史 > 90% 应返回 Synchronous"
    );
    assert!(trigger.is_synchronous());
}

/// TC-COMPACT-DUAL-004：token_ratio 超阈值优先返回更高等级。
#[tokio::test]
async fn tc_compact_dual_004_token_ratio_drives_trigger() {
    let llm = SummarySpyLlm::new("摘要");
    let engine = CompactionEngine::new(&llm);
    let config = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 1000, // 高 max_entries → entry_ratio 很低
        max_tokens: 10000, // 高 max_tokens → token_ratio 低
    };
    // 10 条消息（中文 UTF-8 ~30 bytes/条 → ~7 tokens/条）
    // 20 条 ≈ 140 tokens, token_ratio = 140/10000 = 0.014 < 0.7 → None
    let history = make_history_n(5);
    let trigger = engine.check_compaction_needed(&history, &config);
    assert_eq!(
        trigger,
        CompactionTrigger::None,
        "token_ratio < 0.7 应返回 None"
    );

    // 增大消息或降低 max_tokens → token_ratio > 0.9
    let config2 = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 1000,
        max_tokens: 100, // 低 max_tokens → token_ratio 高
    };
    // 40 条消息 ≈ 280 tokens, token_ratio = 280/100 = 2.8 > 0.9
    let history = make_history_n(20);
    let trigger = engine.check_compaction_needed(&history, &config2);
    assert_eq!(
        trigger,
        CompactionTrigger::Synchronous,
        "token_ratio > 0.9 应返回 Synchronous"
    );
}

/// TC-COMPACT-DUAL-005：同步压缩返回压缩后历史（复用 compact() 逻辑）。
#[tokio::test]
async fn tc_compact_dual_005_synchronous_compaction_returns_result() {
    let llm = SummarySpyLlm::new("同步压缩摘要");
    let engine = CompactionEngine::new(&llm);
    let config = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 50,
        max_tokens: 8192,
    };
    // 制造超 token_limit 的历史 → Synchronous
    let history = make_history_n(30);
    let trigger = engine.check_compaction_needed(&history, &config);
    assert_eq!(trigger, CompactionTrigger::Synchronous);

    // 执行同步压缩
    let result = engine.compact(&history, 200).await.unwrap();
    let info = result.info.expect("同步压缩应返回 info");
    assert!(info.compacted_count > 0, "应有消息被压缩");
    assert!(
        info.compacted_tokens < info.total_tokens,
        "压缩后 token 应减少"
    );
    // 验证 CompactionKind 转换
    assert!(trigger.to_kind().is_some(), "Synchronous 应有 kind");
    assert_eq!(
        trigger.to_kind(),
        Some(echomind_models::CompactionKind::Synchronous)
    );
}

/// TC-COMPACT-DUAL-006：后台压缩失败不影响对话（错误吞咽）。
#[tokio::test]
async fn tc_compact_dual_006_background_compaction_failure_swallowed() {
    let llm = ErrorLlm;
    // run_background_compaction 应吞咽错误，不 panic / 不返回 Err
    let history = make_history_n(20);
    run_background_compaction("test-conv-fail", &history, 200, &llm).await;
    // 如果到这里没 panic，说明错误被正确吞咽
}

/// TC-COMPACT-DUAL-007：同一会话不重复触发后台压缩（compaction_pending 去重）。
#[tokio::test]
async fn tc_compact_dual_007_dedup_prevents_duplicate_background_compaction() {
    let pending = Arc::new(tokio::sync::Mutex::new(HashSet::new()));

    // 第一次获取 → true
    let acquired1 = try_acquire_background_compaction("conv-dedup", Arc::clone(&pending)).await;
    assert!(acquired1, "首次获取应成功");

    // 第二次获取（同一会话）→ false（去重）
    let acquired2 = try_acquire_background_compaction("conv-dedup", Arc::clone(&pending)).await;
    assert!(!acquired2, "同一会话应被去重跳过");

    // 不同会话 → true
    let acquired3 = try_acquire_background_compaction("conv-other", Arc::clone(&pending)).await;
    assert!(acquired3, "不同会话应获取成功");

    // 释放后可以再次获取
    release_background_compaction("conv-dedup", Arc::clone(&pending)).await;
    let acquired4 = try_acquire_background_compaction("conv-dedup", Arc::clone(&pending)).await;
    assert!(acquired4, "释放后应可再次获取");
}

/// TC-COMPACT-DUAL-008：DualThresholdConfig 默认值正确。
#[test]
fn tc_compact_dual_008_default_config_values() {
    let config = DualThresholdConfig::default();
    assert_eq!(config.soft_threshold, 0.7);
    assert_eq!(config.hard_threshold, 0.9);
    assert_eq!(config.max_entries, 50);
    assert_eq!(config.max_tokens, 8192);
}

/// TC-COMPACT-DUAL-009：CompactionTrigger 序列化/反序列化（IPC 兼容）。
#[test]
fn tc_compact_dual_009_trigger_serde_roundtrip() {
    let triggers = vec![
        CompactionTrigger::None,
        CompactionTrigger::Background,
        CompactionTrigger::Synchronous,
    ];
    for trigger in &triggers {
        let json = serde_json::to_string(trigger).expect("serialize");
        let back: CompactionTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, trigger, "往返序列化应保持一致");
    }
}

/// TC-COMPACT-DUAL-010：from_settings 使用默认值填充缺失项。
#[test]
fn tc_compact_dual_010_from_settings_defaults() {
    let config = DualThresholdConfig::from_settings(None, None, None, None);
    assert_eq!(config.soft_threshold, 0.7);
    assert_eq!(config.hard_threshold, 0.9);
    assert_eq!(config.max_entries, 50);
    assert_eq!(config.max_tokens, 8192);

    let config2 = DualThresholdConfig::from_settings(Some(0.6), Some(0.85), Some(100), Some(4096));
    assert_eq!(config2.soft_threshold, 0.6);
    assert_eq!(config2.hard_threshold, 0.85);
    assert_eq!(config2.max_entries, 100);
    assert_eq!(config2.max_tokens, 4096);
}

/// TC-COMPACT-DUAL-011：max_entries=0 或 max_tokens=0 防除零返回 None。
#[tokio::test]
async fn tc_compact_dual_011_zero_max_returns_none() {
    let llm = SummarySpyLlm::new("摘要");
    let engine = CompactionEngine::new(&llm);
    let config = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 0,
        max_tokens: 8192,
    };
    let history = make_history_n(10);
    let trigger = engine.check_compaction_needed(&history, &config);
    assert_eq!(
        trigger,
        CompactionTrigger::None,
        "max_entries=0 应返回 None"
    );

    let config2 = DualThresholdConfig {
        soft_threshold: 0.7,
        hard_threshold: 0.9,
        max_entries: 50,
        max_tokens: 0,
    };
    let trigger2 = engine.check_compaction_needed(&history, &config2);
    assert_eq!(
        trigger2,
        CompactionTrigger::None,
        "max_tokens=0 应返回 None"
    );
}

/// TC-COMPACT-DUAL-012：CompactionKind as_str + serde 测试。
#[test]
fn tc_compact_dual_012_compaction_kind_as_str() {
    use echomind_models::CompactionKind;

    assert_eq!(CompactionKind::Background.as_str(), "background");
    assert_eq!(CompactionKind::Synchronous.as_str(), "synchronous");

    // serde
    let json = serde_json::to_string(&CompactionKind::Background).unwrap();
    assert_eq!(json, "\"background\"");
    let back: CompactionKind = serde_json::from_str("\"synchronous\"").unwrap();
    assert_eq!(back, CompactionKind::Synchronous);
}
