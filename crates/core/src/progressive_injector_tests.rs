#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 渐进式上下文注入 TDD 测试（TC-PROG-001~006）。
//!
//! 测试覆盖：
//! - 初始注入 top-2 chunks
//! - LLM 输出检测"需要更多信息"→ 追加 chunk
//! - 足够信息时不再追加
//! - 最大追加次数 = top_k
//! - 平均注入 chunk 数 < top_k
//! - 禁用时行为与当前一致

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::chat::{ChatEngine, ChatOutcome};
use crate::progressive_injector::{
    ProgressiveConfig, ProgressiveInjector, ProgressiveStats, detect_insufficient_info,
    split_sources,
};
use crate::{LLMProvider, Retriever};

// ============================================================
// 辅助 Mock
// ============================================================

/// 构造 N 条检索结果（按 score 降序）。
fn make_sources(n: usize) -> Vec<RetrievalResult> {
    (0..n)
        .map(|i| RetrievalResult {
            chunk: Chunk::new(
                format!("doc-{i}"),
                format!("这是第 {i} 条文档片段，包含一些用于测试的文本内容。"),
                10,
                i,
            ),
            score: 0.95 - i as f32 * 0.05,
            doc_name: format!("doc_{i}.md"),
        })
        .collect()
}

/// 间谍 LLM：覆盖 `chat_stream_segmented`，捕获 dynamic_context 参数。
#[derive(Clone, Default)]
struct CapturingLlm {
    segmented_calls: Arc<AtomicUsize>,
    captured_dynamic_context: Arc<Mutex<Option<String>>>,
}

impl LLMProvider for CapturingLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::empty().boxed())
    }

    async fn chat_stream_segmented(
        &self,
        _static_prefix: &str,
        dynamic_context: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        self.segmented_calls.fetch_add(1, Ordering::SeqCst);
        *self.captured_dynamic_context.lock().unwrap() = Some(dynamic_context.to_string());
        Ok(futures::stream::empty().boxed())
    }
}

/// 多条命中 Retriever。
struct MultiHitRetriever {
    sources: Vec<RetrievalResult>,
}

impl Retriever for MultiHitRetriever {
    async fn retrieve(&self, _query: &str, top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(self.sources.iter().take(top_k).cloned().collect())
    }
}

// ============================================================
// TC-PROG-001: 初始注入 top-2 chunks
// ============================================================

/// TC-PROG-001：ProgressiveInjector 初始注入 top-2 chunks。
///
/// 验证：
/// - initial_indices() 返回 [0, 1]
/// - injected_count() == 2
/// - 初始不触发任何追加
#[test]
fn tc_prog_001_initial_inject_top_2() {
    let injector = ProgressiveInjector::with_defaults(5);

    assert_eq!(
        injector.initial_indices(),
        vec![0, 1],
        "初始应注入前 2 个 chunk（索引 0 和 1）"
    );
    assert_eq!(injector.injected_count(), 2, "初始注入后已注入数量应为 2");
    assert_eq!(injector.append_rounds(), 0, "初始注入后追加轮次应为 0");
}

/// TC-PROG-001b：ChatEngine 启用渐进式注入后，prompt 仅包含 top-2 chunks。
///
/// 使用 CapturingLlm 捕获 dynamic_context，验证其中仅包含 2 个来源编号，
/// 而非全部 top_k 个。
#[tokio::test]
async fn tc_prog_001b_chat_engine_initial_prompt_has_2_chunks() {
    let llm = CapturingLlm::default();
    let retriever = MultiHitRetriever {
        sources: make_sources(8),
    };
    let engine = ChatEngine::new(retriever, llm.clone()).with_progressive(2);

    let outcome = engine.chat(&[], "测试问题", 8).await.unwrap();

    let ChatOutcome::Answered { .. } = outcome else {
        panic!("有检索结果必须走 Answered 分支");
    };

    let captured = llm
        .captured_dynamic_context
        .lock()
        .unwrap()
        .clone()
        .expect("应捕获 dynamic_context");

    // prompt 中仅包含 [1] 和 [2]，不包含 [3]~[8]
    assert!(captured.contains("[1]"), "prompt 应包含来源 [1]");
    assert!(captured.contains("[2]"), "prompt 应包含来源 [2]");
    assert!(
        !captured.contains("[3]"),
        "渐进式注入：prompt 不应包含 [3]（仅初始注入 2 个）"
    );
    assert!(!captured.contains("[8]"), "渐进式注入：prompt 不应包含 [8]");
}

// ============================================================
// TC-PROG-002: LLM 生成中检测"需要更多信息"→ 追加 chunk
// ============================================================

/// TC-PROG-002：检测到"不确定"信号后追加 chunk。
///
/// 验证：
/// - needs_more_info() 对包含"不确定"的文本返回 true
/// - can_append() 返回 true（还有未注入的 source）
/// - next_batch() 返回下一批索引
/// - injected_count 更新
#[test]
fn tc_prog_002_detect_insufficient_then_append() {
    let mut injector = ProgressiveInjector::with_defaults(5);

    // 初始注入 2 个
    assert_eq!(injector.injected_count(), 2);

    // 检测到"不确定"信号
    let llm_output = "我不确定这个问题是否在文档中有答案。";
    assert!(
        injector.needs_more_info(llm_output),
        "LLM 输出包含'不确定'应触发 needs_more_info"
    );

    // 可以追加
    assert!(injector.can_append(), "还有 3 个未注入的 source，应可追加");

    // 追加一批
    let batch = injector.next_batch();
    assert_eq!(batch, vec![2], "应追加索引 2");
    assert_eq!(injector.injected_count(), 3, "追加后已注入 3 个");
    assert_eq!(injector.append_rounds(), 1, "追加轮次为 1");
}

/// TC-PROG-002b：英文"insufficient"也能被检测到。
#[test]
fn tc_prog_002b_english_insufficient_detection() {
    let injector = ProgressiveInjector::with_defaults(5);

    assert!(
        injector.needs_more_info("The information is insufficient to answer."),
        "英文'insufficient'应被检测到"
    );
    assert!(
        injector.needs_more_info("Not enough context to determine."),
        "英文'not enough'应被检测到"
    );
    assert!(
        injector.needs_more_info("unable to find relevant information"),
        "英文'unable to'应被检测到"
    );
}

/// TC-PROG-002c：多次追加逐步扩展。
#[test]
fn tc_prog_002c_multiple_appends() {
    let mut injector = ProgressiveInjector::with_defaults(5);

    // 第一次追加
    let batch1 = injector.next_batch();
    assert_eq!(batch1, vec![2]);
    assert_eq!(injector.injected_count(), 3);

    // 第二次追加
    let batch2 = injector.next_batch();
    assert_eq!(batch2, vec![3]);
    assert_eq!(injector.injected_count(), 4);

    // 第三次追加
    let batch3 = injector.next_batch();
    assert_eq!(batch3, vec![4]);
    assert_eq!(injector.injected_count(), 5);
}

// ============================================================
// TC-PROG-003: 足够信息时不再追加
// ============================================================

/// TC-PROG-003：LLM 输出无"不确定"信号时不追加。
///
/// 验证：
/// - needs_more_info() 对正常回答文本返回 false
/// - 不调用 next_batch()
/// - injected_count 保持初始值
#[test]
fn tc_prog_003_sufficient_info_no_append() {
    let injector = ProgressiveInjector::with_defaults(5);

    // LLM 正常回答，无"不确定"信号
    let normal_output = "根据文档内容，这个功能的实现方式如下...";
    assert!(
        !injector.needs_more_info(normal_output),
        "正常回答不应触发 needs_more_info"
    );

    // 不追加
    assert_eq!(injector.injected_count(), 2, "不追加时保持初始注入 2 个");
    assert_eq!(injector.append_rounds(), 0, "追加轮次为 0");

    // 虽然可以追加，但不追加
    assert!(injector.can_append(), "仍有可追加的 source");
    // 主动不调用 next_batch，验证 injected_count 不变
    assert_eq!(injector.injected_count(), 2);
}

/// TC-PROG-003b：detect_insufficient_info 独立函数测试。
#[test]
fn tc_prog_003b_detect_function_normal_text() {
    // 正常回答文本不触发
    assert!(!detect_insufficient_info("根据文档第三段的内容..."));
    assert!(!detect_insufficient_info("The answer is 42."));
    assert!(!detect_insufficient_info("以下是相关步骤的说明。"));

    // 空文本不触发
    assert!(!detect_insufficient_info(""));
}

// ============================================================
// TC-PROG-004: 最大追加次数 = top_k
// ============================================================

/// TC-PROG-004：追加次数达到上限后不再追加。
///
/// 验证：
/// - 注入全部 source 后 can_append() 返回 false
/// - next_batch() 返回空 vec
/// - injected_count 不超过 total_sources
#[test]
fn tc_prog_004_max_append_reached() {
    let mut injector = ProgressiveInjector::with_defaults(3);

    // 初始注入 2，剩余 1 个
    assert_eq!(injector.injected_count(), 2);
    assert!(injector.can_append());

    // 追加最后一个
    let batch = injector.next_batch();
    assert_eq!(batch, vec![2]);
    assert_eq!(injector.injected_count(), 3);

    // 已注入全部，不能继续追加
    assert!(!injector.can_append(), "全部注入后不能追加");
    assert_eq!(
        injector.next_batch(),
        Vec::<usize>::new(),
        "已注入全部 source 后 next_batch 返回空"
    );
    assert_eq!(injector.injected_count(), 3, "不超过 total_sources");
}

/// TC-PROG-004b：max_rounds 限制追加轮次。
#[test]
fn tc_prog_004b_max_rounds_limit() {
    let config = ProgressiveConfig {
        initial_count: 1,
        max_rounds: 2, // 只允许 2 次追加
    };
    let mut injector = ProgressiveInjector::new(config, 10);

    // 初始注入 1
    assert_eq!(injector.injected_count(), 1);

    // 第一次追加
    let batch1 = injector.next_batch();
    assert_eq!(batch1, vec![1]);
    assert_eq!(injector.injected_count(), 2);

    // 第二次追加
    let batch2 = injector.next_batch();
    assert_eq!(batch2, vec![2]);
    assert_eq!(injector.injected_count(), 3);

    // 第三次追加被 max_rounds 拦截（虽然还有 source）
    assert!(!injector.can_append(), "max_rounds=2 时第三次追加被拦截");
    assert_eq!(
        injector.next_batch(),
        Vec::<usize>::new(),
        "超过 max_rounds 后返回空"
    );
}

// ============================================================
// TC-PROG-005: 平均注入 chunk 数 < top_k
// ============================================================

/// TC-PROG-005：模拟多次查询，验证平均注入 chunk 数 < top_k。
///
/// 场景：100 次查询
/// - 60% 仅需初始 2 个 chunk（LLM 回答充分）
/// - 25% 需追加 1 次（共 3 个）
/// - 15% 需追加 2 次（共 4 个）
///
/// 平均 = (60×2 + 25×3 + 15×4) / 100 = 2.55 << 8 (top_k)
#[test]
fn tc_prog_005_avg_injected_less_than_top_k() {
    let top_k = 8;
    let mut stats = ProgressiveStats::default();

    // 60% 仅需初始
    for _ in 0..60 {
        stats.record(2, false);
    }

    // 25% 追加 1 次
    for _ in 0..25 {
        stats.record(3, true);
    }

    // 15% 追加 2 次
    for _ in 0..15 {
        stats.record(4, true);
    }

    let avg = stats.avg_injected();
    assert!(avg < top_k as f64, "平均注入 {avg} 应 < top_k={top_k}");
    assert!(avg < 5.0, "平均注入 {avg} 应 < 5.0（期望 2-3 范围）");
    assert_eq!(stats.total_queries, 100);
    assert_eq!(stats.append_rate(), 0.40); // 40% 触发追加
}

/// TC-PROG-005b：split_sources 正确分割。
#[test]
fn tc_prog_005b_split_sources() {
    let sources = make_sources(5);
    let (initial, remaining) = split_sources(&sources, 2);

    assert_eq!(initial.len(), 2, "初始批次 2 个");
    assert_eq!(remaining.len(), 3, "剩余批次 3 个");
    assert_eq!(initial[0].doc_name, "doc_0.md");
    assert_eq!(initial[1].doc_name, "doc_1.md");
    assert_eq!(remaining[0].doc_name, "doc_2.md");
    assert_eq!(remaining[2].doc_name, "doc_4.md");
}

/// TC-PROG-005c：split_sources 边界——initial_count > sources.len()。
#[test]
fn tc_prog_005c_split_sources_overflow() {
    let sources = make_sources(2);
    let (initial, remaining) = split_sources(&sources, 5);

    assert_eq!(initial.len(), 2, "initial_count > len 时取全部");
    assert_eq!(remaining.len(), 0, "remaining 为空");
}

// ============================================================
// TC-PROG-006: 禁用时行为与当前一致
// ============================================================

/// TC-PROG-006：ChatEngine 未启用渐进式注入时，prompt 包含全部 top_k chunks。
///
/// 验证禁用渐进式注入时，ChatEngine 的行为与当前完全一致：
/// - prompt 包含全部 top_k 个来源编号
/// - LLM 被调用一次
#[tokio::test]
async fn tc_prog_006_disabled_uses_all_chunks() {
    let llm = CapturingLlm::default();
    let retriever = MultiHitRetriever {
        sources: make_sources(8),
    };
    // 不调用 with_progressive() = 禁用
    let engine = ChatEngine::new(retriever, llm.clone());

    let outcome = engine.chat(&[], "测试问题", 8).await.unwrap();

    let ChatOutcome::Answered { .. } = outcome else {
        panic!("有检索结果必须走 Answered 分支");
    };

    let captured = llm
        .captured_dynamic_context
        .lock()
        .unwrap()
        .clone()
        .expect("应捕获 dynamic_context");

    // 禁用时 prompt 包含全部 8 个来源
    for i in 1..=8 {
        assert!(
            captured.contains(&format!("[{i}]")),
            "禁用渐进式注入时 prompt 应包含来源 [{i}]"
        );
    }

    assert_eq!(
        llm.segmented_calls.load(Ordering::SeqCst),
        1,
        "LLM 应被调用一次"
    );
}

/// TC-PROG-006b：启用 vs 禁用对比——启用的 prompt 更短。
#[tokio::test]
async fn tc_prog_006b_enabled_vs_disabled_prompt_size() {
    let sources = make_sources(8);

    // 禁用
    let llm_disabled = CapturingLlm::default();
    let engine_disabled = ChatEngine::new(
        MultiHitRetriever {
            sources: sources.clone(),
        },
        llm_disabled.clone(),
    );
    engine_disabled.chat(&[], "测试", 8).await.unwrap();
    let disabled_context = llm_disabled
        .captured_dynamic_context
        .lock()
        .unwrap()
        .clone()
        .unwrap();

    // 启用
    let llm_enabled = CapturingLlm::default();
    let engine_enabled = ChatEngine::new(
        MultiHitRetriever {
            sources: sources.clone(),
        },
        llm_enabled.clone(),
    )
    .with_progressive(2);
    engine_enabled.chat(&[], "测试", 8).await.unwrap();
    let enabled_context = llm_enabled
        .captured_dynamic_context
        .lock()
        .unwrap()
        .clone()
        .unwrap();

    // 启用渐进式注入的 prompt 更短
    assert!(
        enabled_context.len() < disabled_context.len(),
        "渐进式注入的 prompt 应更短（{} < {}）",
        enabled_context.len(),
        disabled_context.len()
    );
}
