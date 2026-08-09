//! Q10 LLMProvider Trait 扩展 — 辅助模型方法 TDD 测试。
//!
//! 借鉴 QM `HarnessModelUtilities` 接口，为 `LLMProvider` trait 增加
//! `generate_title` / `one_shot` / `judge` / `context_token_budget` 可选方法。
//! 默认实现返回 `None`，保证现有实现者无需修改（向后兼容）。

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::Result;
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::LLMProvider;
use echomind_models::ChatMessage;

// ---------------------------------------------------------------------------
// Mock Providers
// ---------------------------------------------------------------------------

/// 最小 stub：只实现 `chat_stream`，不覆盖任何新方法。
/// 用于验证默认实现返回 None（向后兼容）。
struct MinimalStub;

impl LLMProvider for MinimalStub {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::empty().boxed())
    }
}

/// 可配置 mock：覆盖 `generate_title` / `one_shot` / `judge` / `context_token_budget`。
struct RichMock {
    title: Option<String>,
    one_shot_resp: Option<String>,
    judge_resp: Option<String>,
    budget: Option<usize>,
    should_fail: bool,
    generate_title_called: AtomicBool,
    one_shot_called: AtomicBool,
    judge_called: AtomicBool,
    call_count: AtomicUsize,
}

impl RichMock {
    fn new() -> Self {
        Self {
            title: None,
            one_shot_resp: None,
            judge_resp: None,
            budget: None,
            should_fail: false,
            generate_title_called: AtomicBool::new(false),
            one_shot_called: AtomicBool::new(false),
            judge_called: AtomicBool::new(false),
            call_count: AtomicUsize::new(0),
        }
    }

    fn with_title(mut self, title: &str) -> Self {
        self.title = Some(title.to_string());
        self
    }

    fn with_one_shot(mut self, resp: &str) -> Self {
        self.one_shot_resp = Some(resp.to_string());
        self
    }

    fn with_judge(mut self, verdict: &str) -> Self {
        self.judge_resp = Some(verdict.to_string());
        self
    }

    fn with_budget(mut self, budget: usize) -> Self {
        self.budget = Some(budget);
        self
    }

    fn with_failure(mut self) -> Self {
        self.should_fail = true;
        self
    }
}

impl LLMProvider for RichMock {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        Ok(futures::stream::empty().boxed())
    }

    async fn generate_title(&self, _transcript: &str) -> Result<Option<String>> {
        self.generate_title_called.store(true, Ordering::SeqCst);
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            return Err(anyhow::anyhow!("模拟标题生成失败"));
        }
        Ok(self.title.clone())
    }

    async fn one_shot(&self, _system: &str, _prompt: &str) -> Result<Option<String>> {
        self.one_shot_called.store(true, Ordering::SeqCst);
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            return Err(anyhow::anyhow!("模拟单次推理失败"));
        }
        Ok(self.one_shot_resp.clone())
    }

    async fn judge(&self, _system: &str, _prompt: &str) -> Result<Option<String>> {
        self.judge_called.store(true, Ordering::SeqCst);
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            return Err(anyhow::anyhow!("模拟判断失败"));
        }
        Ok(self.judge_resp.clone())
    }

    fn context_token_budget(&self) -> Option<usize> {
        self.budget
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// TC-LLM-AUX-001：默认实现返回 None（向后兼容）。
///
/// `MinimalStub` 只实现 `chat_stream`，不覆盖任何新方法。
/// 所有新方法应返回 `Ok(None)` 或 `None`，证明现有实现者无需修改。
#[tokio::test]
async fn tc_llm_aux_001_default_impl_returns_none() {
    let stub = MinimalStub;

    // generate_title 默认返回 Ok(None)
    let title = stub.generate_title("用户对话内容").await.unwrap();
    assert!(title.is_none(), "generate_title 默认应返回 None");

    // one_shot 默认返回 Ok(None)
    let one_shot = stub.one_shot("system", "prompt").await.unwrap();
    assert!(one_shot.is_none(), "one_shot 默认应返回 None");

    // judge 默认返回 Ok(None)
    let judge = stub.judge("system", "prompt").await.unwrap();
    assert!(judge.is_none(), "judge 默认应返回 None");

    // context_token_budget 默认返回 None
    let budget = stub.context_token_budget();
    assert!(budget.is_none(), "context_token_budget 默认应返回 None");
}

/// TC-LLM-AUX-002：generate_title 返回有意义的标题。
///
/// `RichMock` 覆盖 `generate_title`，返回指定标题。
/// 验证 trait 方法可被子实现正确覆盖。
#[tokio::test]
async fn tc_llm_aux_002_generate_title_returns_meaningful_title() {
    let mock = RichMock::new().with_title("RAG 架构设计讨论");

    let title = mock
        .generate_title("用户: 什么是 RAG?\n助手: RAG 是检索增强生成...")
        .await;

    assert!(title.is_ok(), "generate_title 不应返回 Err");
    let title = title.unwrap();
    assert!(title.is_some(), "generate_title 应返回 Some");
    let title = title.unwrap();
    assert_eq!(title, "RAG 架构设计讨论");
    assert!(
        mock.generate_title_called.load(Ordering::SeqCst),
        "generate_title 应被实际调用"
    );
}

/// TC-LLM-AUX-003：generate_title 失败时返回 Err，调用方可降级为 derive_title。
///
/// `RichMock` 设置 `should_fail = true`，`generate_title` 返回 Err。
/// 调用方应捕获错误并降级为字符串截取。
#[tokio::test]
async fn tc_llm_aux_003_generate_title_failure_returns_err() {
    let mock = RichMock::new().with_failure();

    let result = mock.generate_title("对话内容").await;

    assert!(result.is_err(), "generate_title 失败应返回 Err");

    // 模拟调用方降级逻辑
    let fallback_title = result
        .ok()
        .flatten()
        .unwrap_or_else(|| derive_title_fallback("什么是检索增强生成技术?"));

    assert_eq!(
        fallback_title, "什么是检索增强生成技术?",
        "降级标题应与 derive_title 一致"
    );
}

/// TC-LLM-AUX-004：one_shot 返回完整响应。
///
/// `RichMock` 覆盖 `one_shot`，返回指定响应文本。
#[tokio::test]
async fn tc_llm_aux_004_one_shot_returns_full_response() {
    let mock = RichMock::new().with_one_shot("这是一段完整的推理结果。");

    let result = mock.one_shot("你是一个助手", "总结以下内容: ...").await;

    assert!(result.is_ok(), "one_shot 不应返回 Err");
    let resp = result.unwrap();
    assert!(resp.is_some(), "one_shot 应返回 Some");
    let resp = resp.unwrap();
    assert_eq!(resp, "这是一段完整的推理结果。");
    assert!(
        mock.one_shot_called.load(Ordering::SeqCst),
        "one_shot 应被实际调用"
    );
}

/// TC-LLM-AUX-005：context_token_budget 返回模型上下文上限。
///
/// `RichMock` 覆盖 `context_token_budget`，返回 8192。
#[tokio::test]
async fn tc_llm_aux_005_context_token_budget_returns_max() {
    let mock = RichMock::new().with_budget(8192);

    let budget = mock.context_token_budget();

    assert_eq!(budget, Some(8192), "context_token_budget 应返回 8192");
}

/// TC-LLM-AUX-006：judge 返回裁决字符串。
///
/// `RichMock` 覆盖 `judge`，返回 "yes"。
#[tokio::test]
async fn tc_llm_aux_006_judge_returns_verdict() {
    let mock = RichMock::new().with_judge("yes");

    let result = mock.judge("你是安全审查员", "以下内容是否安全: ...").await;

    assert!(result.is_ok(), "judge 不应返回 Err");
    let verdict = result.unwrap();
    assert!(verdict.is_some(), "judge 应返回 Some");
    let verdict = verdict.unwrap();
    assert_eq!(verdict, "yes");
    assert!(
        mock.judge_called.load(Ordering::SeqCst),
        "judge 应被实际调用"
    );
}

/// TC-LLM-AUX-007：trait 对象安全验证 — 通过 Arc<dyn LLMProvider> 调用新方法。
///
/// 注意：`async fn in trait` 不支持 `dyn`，但通过泛型参数可验证 trait 兼容性。
/// 此测试通过泛型函数验证所有新方法可在泛型约束下调用。
#[tokio::test]
async fn tc_llm_aux_007_generic_dispatch_works() {
    async fn use_provider<P: LLMProvider>(provider: &P) {
        let _ = provider.generate_title("test").await;
        let _ = provider.one_shot("sys", "prompt").await;
        let _ = provider.judge("sys", "prompt").await;
        let _ = provider.context_token_budget();
    }

    let mock = RichMock::new()
        .with_title("测试标题")
        .with_one_shot("响应")
        .with_judge("no")
        .with_budget(4096);

    use_provider(&mock).await;

    assert_eq!(
        mock.call_count.load(Ordering::SeqCst),
        3,
        "generate_title + one_shot + judge 应各调用 1 次（共 3 次）"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 模拟 `derive_title` 降级逻辑（与 `commands/mod.rs` 中的实现一致）。
fn derive_title_fallback(query: &str) -> String {
    const TITLE_MAX_CHARS: usize = 24;
    let trimmed = query.trim();
    if trimmed.chars().count() <= TITLE_MAX_CHARS {
        trimmed.to_string()
    } else {
        format!(
            "{}…",
            trimmed.chars().take(TITLE_MAX_CHARS).collect::<String>()
        )
    }
}
