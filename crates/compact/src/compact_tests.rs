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

// ============================================================
// Q04: Token 级预算 + Entry Token 缓存 + Dangling Call 修复
// TC-TOKEN-CACHE-001 ~ TC-TOKEN-CACHE-003
// TC-HEAL-DANGLING-001 ~ TC-HEAL-DANGLING-003
// ============================================================

use crate::{TokenEstimator, heal_dangling_calls};

/// TC-TOKEN-CACHE-001：相同内容第二次估算命中缓存（BPE 只调用一次）。
///
/// 借鉴 QM `estimateEntryTokens()` 的 LRU 缓存：对相同内容的消息，
/// 避免重复 BPE 编码，直接返回缓存的 token 数。
#[tokio::test]
async fn tc_token_cache_001_same_content_hits_cache() {
    let estimator = TokenEstimator::new(100);
    let content = "这是一段测试内容，用于验证 token 缓存功能是否正常工作。";

    // 第一次估算：BPE 编码并缓存
    let tokens1 = estimator.estimate(content).await;
    assert!(tokens1 > 0, "token 数应大于 0");

    // 第二次估算相同内容：应命中缓存，返回相同值
    let tokens2 = estimator.estimate(content).await;
    assert_eq!(
        tokens1, tokens2,
        "相同内容的 token 估算结果应一致（缓存命中）"
    );

    // 缓存条目数应为 1（同一内容只缓存一次）
    assert_eq!(estimator.cache_len().await, 1, "缓存中应只有 1 个条目");
}

/// TC-TOKEN-CACHE-002：缓存满时 LRU 驱逐最旧条目。
///
/// 借鉴 QM 的 50k LRU 缓存：当缓存达到上限时，移除最久未使用的条目，
/// 为新条目腾出空间。
#[tokio::test]
async fn tc_token_cache_002_lru_eviction_when_full() {
    let estimator = TokenEstimator::new(3); // 小缓存便于测试

    // 填满缓存（3 个不同内容）
    estimator.estimate("content A").await;
    estimator.estimate("content B").await;
    estimator.estimate("content C").await;
    assert_eq!(estimator.cache_len().await, 3, "缓存应已满");

    // 插入第 4 个内容 → 驱逐最旧的 "content A"
    estimator.estimate("content D").await;
    assert_eq!(
        estimator.cache_len().await,
        3,
        "缓存满后插入新条目应保持上限"
    );

    // "content A" 应已被驱逐（重新估算应产生相同结果但缓存条目数不变）
    // 注意：被驱逐后重新估算会重新加入缓存，驱逐 "content B"
    estimator.estimate("content A").await;
    assert_eq!(
        estimator.cache_len().await,
        3,
        "驱逐后重新估算应保持缓存上限"
    );
}

/// TC-TOKEN-CACHE-003：`recent_entry_count_within_budget` 从末尾向前保留消息。
///
/// 借鉴 QM `recentEntryCountWithinBudget()`：给定 token 预算，
/// 从消息列表末尾向前累积，返回能在预算内保留的消息条数。
#[tokio::test]
async fn tc_token_cache_003_recent_entry_count_within_budget() {
    let estimator = TokenEstimator::new(100);

    // 构造 5 条消息，每条约 10-15 tokens
    let messages: Vec<ChatMessage> = (0..5)
        .map(|i| {
            user_msg(&format!(
                "这是第 {} 条消息，包含足够的文本内容用于测试 token 预算。",
                i + 1
            ))
        })
        .collect();

    // 预算足够容纳全部 5 条 → 返回 5
    let count_all = estimator
        .recent_entry_count_within_budget(&messages, 10000)
        .await;
    assert_eq!(count_all, 5, "预算充足时应保留全部消息");

    // 预算只能容纳约 2-3 条 → 返回值应小于 5
    let count_partial = estimator
        .recent_entry_count_within_budget(&messages, 30)
        .await;
    assert!(
        count_partial < 5,
        "预算不足时应只保留部分消息，实际保留 {} 条",
        count_partial
    );
    assert!(
        count_partial >= 1,
        "至少应保留最后 1 条消息，实际保留 {} 条",
        count_partial
    );

    // 预算为 0 → 至少保留最后 1 条（防边界情况）
    let count_min = estimator
        .recent_entry_count_within_budget(&messages, 0)
        .await;
    assert_eq!(count_min, 1, "预算为 0 时至少保留最后 1 条消息");
}

/// TC-HEAL-DANGLING-001：单个未配对 tool_call 被修复。
///
/// 借鉴 QM `healDanglingCalls()`：当 assistant 消息包含 Action 但
/// 后续没有 Observation 时，自动插入 `[interrupted]` 占位 observation 消息。
#[tokio::test]
async fn tc_heal_dangling_001_single_unpaired_action_healed() {
    let mut history = vec![
        user_msg("请搜索 EchoMind 的架构文档"),
        assistant_msg(
            "Thought: 我需要搜索知识库中的架构文档。\n\
             Action: search_kb\n\
             Action Input: EchoMind 架构文档",
        ),
        // ← 缺少 Observation，Agent 被中断
    ];

    let original_len = history.len();
    heal_dangling_calls(&mut history);

    // 应插入 1 条 observation 占位消息
    assert_eq!(
        history.len(),
        original_len + 1,
        "应插入 1 条 [interrupted] observation 消息"
    );

    // 插入的消息应为 system 角色，包含 [interrupted] 标记
    let obs_msg = &history[history.len() - 1];
    assert_eq!(
        obs_msg.role, "system",
        "占位 observation 消息应为 system 角色"
    );
    assert!(
        obs_msg.content.contains("[interrupted]"),
        "占位消息应包含 [interrupted] 标记，实际: {}",
        obs_msg.content
    );
}

/// TC-HEAL-DANGLING-002：多个未配对 tool_call 全部修复。
///
/// 多轮 ReAct 循环中，若多轮的 Action 均未获得 Observation，
/// `heal_dangling_calls` 应为每个未配对的 Action 插入占位消息。
#[tokio::test]
async fn tc_heal_dangling_002_multiple_unpaired_actions_all_healed() {
    let mut history = vec![
        user_msg("请搜索并总结"),
        assistant_msg(
            "Thought: 先搜索。\n\
             Action: search_kb\n\
             Action Input: Rust 生态",
        ),
        // ← 缺少 Observation #1
        assistant_msg(
            "Thought: 再搜索另一个主题。\n\
             Action: search_kb\n\
             Action Input: Tauri v2",
        ),
        // ← 缺少 Observation #2
    ];

    let original_len = history.len();
    heal_dangling_calls(&mut history);

    // 应插入 2 条 observation 占位消息
    assert_eq!(
        history.len(),
        original_len + 2,
        "应插入 2 条 [interrupted] observation 消息，实际插入 {} 条",
        history.len() - original_len
    );

    // 验证两条占位消息
    let obs_count = history
        .iter()
        .filter(|m| m.role == "system" && m.content.contains("[interrupted]"))
        .count();
    assert_eq!(obs_count, 2, "应有 2 条包含 [interrupted] 的 system 消息");
}

/// TC-HEAL-DANGLING-003：已配对的 tool_call 不受影响。
///
/// 当 Action 后面已有 Observation 时，`heal_dangling_calls` 不应
/// 插入额外的占位消息。
#[tokio::test]
async fn tc_heal_dangling_003_paired_action_not_affected() {
    let mut history = vec![
        user_msg("请搜索"),
        assistant_msg(
            "Thought: 搜索知识库。\n\
             Action: search_kb\n\
             Action Input: 架构设计",
        ),
        // 已有 Observation
        ChatMessage {
            id: None,
            role: "system".to_string(),
            content: "Observation: 找到了 3 篇相关文档。".to_string(),
            sources: None,
            ..Default::default()
        },
        assistant_msg("Final Answer: 根据搜索结果，架构设计包括..."),
    ];

    let original_len = history.len();
    heal_dangling_calls(&mut history);

    // 不应插入任何消息
    assert_eq!(
        history.len(),
        original_len,
        "已配对的 Action+Observation 不应被修改"
    );
}

// ============================================================
// 跨 Phase 依赖整合测试（S68: 后台压缩用 Sweeper 统一管理）
// ============================================================

use tokio::sync::Mutex;

use crate::schedule_background_compaction;

/// TC-SWEEPER-001：schedule_background_compaction 成功获取并执行。
#[tokio::test]
async fn tc_sweeper_001_schedule_succeeds_and_releases() {
    let pending: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let llm = SummarySpyLlm::new("压缩摘要");
    let history = vec![
        ChatMessage {
            id: None,
            role: "user".to_string(),
            content: "旧消息1".to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: Some(1),
        },
        ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: "回复1".to_string(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: Some(1),
        },
    ];

    // 调度后台压缩
    let future = schedule_background_compaction(
        "conv-sweeper-001".to_string(),
        history,
        100,
        llm,
        pending.clone(),
    )
    .await;

    assert!(future.is_some(), "首次调度应返回 Some");
    let future = future.unwrap();

    // 执行 future
    future.await;

    // 验证 pending 已释放
    let pending_set = pending.lock().await;
    assert!(
        !pending_set.contains("conv-sweeper-001"),
        "执行完成后 pending 应被释放"
    );
}

/// TC-SWEEPER-002：同一会话重复调度返回 None（去重）。
#[tokio::test]
async fn tc_sweeper_002_duplicate_schedule_returns_none() {
    let pending: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    // 手动模拟已有 pending
    {
        let mut set = pending.lock().await;
        set.insert("conv-dup".to_string());
    }

    let llm = SummarySpyLlm::new("摘要");
    let history = vec![ChatMessage {
        id: None,
        role: "user".to_string(),
        content: "test".to_string(),
        sources: None,
        reasoning: None,
        turn_group: None,
        version: Some(1),
    }];

    let future =
        schedule_background_compaction("conv-dup".to_string(), history, 100, llm, pending.clone())
            .await;

    assert!(future.is_none(), "已有 pending 时应返回 None（跳过）");
}

/// TC-SWEEPER-003：LLM 失败时 pending 仍被释放（错误吞咽）。
#[tokio::test]
async fn tc_sweeper_003_llm_failure_releases_pending() {
    let pending: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    /// 失败 LLM：chat_stream 返回 Err
    struct FailingLlm;
    impl LLMProvider for FailingLlm {
        async fn chat_stream(
            &self,
            _: &str,
            _: &[ChatMessage],
            _: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            Err(anyhow::anyhow!("LLM 不可用"))
        }
    }

    let history = vec![ChatMessage {
        id: None,
        role: "user".to_string(),
        content: "test failure".to_string(),
        sources: None,
        reasoning: None,
        turn_group: None,
        version: Some(1),
    }];

    let future = schedule_background_compaction(
        "conv-fail".to_string(),
        history,
        100,
        FailingLlm,
        pending.clone(),
    )
    .await;

    assert!(future.is_some(), "首次调度应返回 Some");
    let future = future.unwrap();
    future.await;

    // 验证 pending 已释放（即使 LLM 失败）
    let pending_set = pending.lock().await;
    assert!(
        !pending_set.contains("conv-fail"),
        "LLM 失败后 pending 仍应被释放"
    );
}

/// TC-SWEEPER-004：schedule 返回的 future 自动释放 pending（不需手动 release）。
#[tokio::test]
async fn tc_sweeper_004_future_auto_releases_pending() {
    let pending: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let llm = SummarySpyLlm::new("摘要");

    let history = vec![ChatMessage {
        id: None,
        role: "user".to_string(),
        content: "auto release test".to_string(),
        sources: None,
        reasoning: None,
        turn_group: None,
        version: Some(1),
    }];

    let future = schedule_background_compaction(
        "conv-auto".to_string(),
        history,
        10000, // 大限制，不触发实际压缩
        llm,
        pending.clone(),
    )
    .await;

    assert!(future.is_some());
    let future = future.unwrap();

    // 执行前 pending 应存在
    {
        let set = pending.lock().await;
        assert!(set.contains("conv-auto"), "执行前 pending 应存在");
    }

    future.await;

    // 执行后 pending 应释放
    {
        let set = pending.lock().await;
        assert!(!set.contains("conv-auto"), "执行后 pending 应释放");
    }
}
