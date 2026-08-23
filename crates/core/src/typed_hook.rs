//! 类型安全 Hook 系统（B-04 借鉴 Rig `AgentHook` 每事件一 Action 类型）。
//!
//! ## 核心设计
//!
//! 每个生命周期事件有独立的 Action 返回类型，编译期拒绝不合法组合：
//!
//! - `on_before_retrieval() -> BeforeRetrievalAction`（Continue / PatchQuery / Stop）
//! - `on_after_retrieval() -> AfterRetrievalAction`（Continue / FilterResults / Stop）
//! - `on_before_generation() -> BeforeGenerationAction`（Continue / PatchContext / Stop）
//! - `on_model_response() -> ModelResponseAction`（Continue / RetryWithFeedback / Stop）
//!
//! 与现有 `AgentHook`（单方法 trait + HookPhase 枚举）共存：
//! - 旧的 `AgentHook` 保持向后兼容
//! - 新的 `TypedHook` 提供类型安全的 action 返回
//! - `HookRegistry` 同时支持两种 hook

use echomind_models::RetrievalResult;
use std::pin::Pin;

use crate::hooks::HookContext;

/// BeforeRetrieval 阶段的 Action 类型。
#[derive(Debug, Clone)]
pub enum BeforeRetrievalAction {
    /// 继续，不修改
    Continue,
    /// 修改查询文本
    PatchQuery(String),
    /// 停止执行
    Stop(String),
}

/// AfterRetrieval 阶段的 Action 类型。
#[derive(Debug, Clone)]
pub enum AfterRetrievalAction {
    /// 继续，不修改
    Continue,
    /// 过滤/修改检索结果
    FilterResults(Vec<RetrievalResult>),
    /// 停止执行
    Stop(String),
}

/// BeforeGeneration 阶段的 Action 类型。
#[derive(Debug, Clone, Default)]
pub struct RequestPatch {
    /// 覆盖 temperature（None = 不修改）
    pub temperature: Option<f32>,
    /// 覆盖 max_tokens（None = 不修改）
    pub max_tokens: Option<u32>,
    /// 追加上下文文本
    pub extra_context: Option<String>,
}

/// BeforeGeneration 阶段的 Action 类型。
#[derive(Debug, Clone)]
pub enum BeforeGenerationAction {
    /// 继续，不修改
    Continue,
    /// 修改请求参数
    PatchContext(RequestPatch),
    /// 停止执行
    Stop(String),
}

/// ModelResponse 阶段的 Action 类型。
#[derive(Debug, Clone)]
pub enum ModelResponseAction {
    /// 继续，接受响应
    Continue,
    /// 带反馈重试（将反馈文本追加到 prompt 后重新调用 LLM）
    RetryWithFeedback(String),
    /// 停止执行
    Stop(String),
}

/// 类型安全 Hook trait（B-04 借鉴 Rig AgentHook）。
///
/// 每个生命周期事件有独立方法 + 独立 Action 返回类型。
/// 不支持的方法默认返回 `Continue`（向后兼容）。
///
/// # 与 `AgentHook` 的关系
///
/// `AgentHook`（旧）：单方法 trait + HookPhase 枚举，只能修改上下文或返回错误。
/// `TypedHook`（新）：多方法 trait + 类型安全 Action，支持 Patch/Retry/Stop。
///
/// 两者在 `HookRegistry` 中共存，旧代码无需修改。
pub trait TypedHook: Send + Sync {
    /// Hook 名称。
    fn name(&self) -> &str;

    /// 检索前：可修改查询文本。
    fn on_before_retrieval<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<BeforeRetrievalAction>> + Send + 'a>>
    {
        Box::pin(async { Ok(BeforeRetrievalAction::Continue) })
    }

    /// 检索后：可过滤/修改检索结果。
    fn on_after_retrieval<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<AfterRetrievalAction>> + Send + 'a>>
    {
        Box::pin(async { Ok(AfterRetrievalAction::Continue) })
    }

    /// 生成前：可修改请求参数。
    fn on_before_generation<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<BeforeGenerationAction>> + Send + 'a>,
    > {
        Box::pin(async { Ok(BeforeGenerationAction::Continue) })
    }

    /// 模型响应后：可请求重试。
    fn on_model_response<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<ModelResponseAction>> + Send + 'a>>
    {
        Box::pin(async { Ok(ModelResponseAction::Continue) })
    }

    /// 是否启用（默认 true）。
    fn enabled(&self) -> bool {
        true
    }
}

/// 应用 `BeforeRetrievalAction` 到上下文。
pub fn apply_before_retrieval_action(
    ctx: &mut HookContext,
    action: BeforeRetrievalAction,
) -> anyhow::Result<bool> {
    match action {
        BeforeRetrievalAction::Continue => Ok(true),
        BeforeRetrievalAction::PatchQuery(new_query) => {
            ctx.query = new_query;
            Ok(true)
        }
        BeforeRetrievalAction::Stop(reason) => {
            tracing::warn!("Hook 停止执行 (BeforeRetrieval): {reason}");
            Ok(false)
        }
    }
}

/// 应用 `AfterRetrievalAction` 到上下文。
pub fn apply_after_retrieval_action(
    ctx: &mut HookContext,
    action: AfterRetrievalAction,
) -> anyhow::Result<bool> {
    match action {
        AfterRetrievalAction::Continue => Ok(true),
        AfterRetrievalAction::FilterResults(results) => {
            ctx.retrieval_results = results;
            Ok(true)
        }
        AfterRetrievalAction::Stop(reason) => {
            tracing::warn!("Hook 停止执行 (AfterRetrieval): {reason}");
            Ok(false)
        }
    }
}

/// 应用 `BeforeGenerationAction` 到上下文。
pub fn apply_before_generation_action(
    _ctx: &mut HookContext,
    action: BeforeGenerationAction,
) -> anyhow::Result<Option<RequestPatch>> {
    match action {
        BeforeGenerationAction::Continue => Ok(None),
        BeforeGenerationAction::PatchContext(patch) => Ok(Some(patch)),
        BeforeGenerationAction::Stop(reason) => {
            tracing::warn!("Hook 停止执行 (BeforeGeneration): {reason}");
            Err(anyhow::anyhow!("Hook stopped: {reason}"))
        }
    }
}

/// 应用 `ModelResponseAction`。
///
/// 返回 `Some(feedback)` 表示需要带反馈重试，`None` 表示继续。
pub fn apply_model_response_action(
    _ctx: &mut HookContext,
    action: ModelResponseAction,
) -> anyhow::Result<Option<String>> {
    match action {
        ModelResponseAction::Continue => Ok(None),
        ModelResponseAction::RetryWithFeedback(feedback) => Ok(Some(feedback)),
        ModelResponseAction::Stop(reason) => {
            tracing::warn!("Hook 停止执行 (ModelResponse): {reason}");
            Err(anyhow::anyhow!("Hook stopped: {reason}"))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::hooks::{HookContext, HookPhase};

    // ─── Action 类型构造 ───

    #[test]
    fn tc_th_001_before_retrieval_continue() {
        let action = BeforeRetrievalAction::Continue;
        let mut ctx = HookContext::new("conv1".into(), "query".into(), HookPhase::BeforeRetrieval);
        assert!(apply_before_retrieval_action(&mut ctx, action).unwrap());
        assert_eq!(ctx.query, "query");
    }

    #[test]
    fn tc_th_002_before_retrieval_patch_query() {
        let action = BeforeRetrievalAction::PatchQuery("rewritten query".into());
        let mut ctx = HookContext::new(
            "conv1".into(),
            "original".into(),
            HookPhase::BeforeRetrieval,
        );
        assert!(apply_before_retrieval_action(&mut ctx, action).unwrap());
        assert_eq!(ctx.query, "rewritten query");
    }

    #[test]
    fn tc_th_003_before_retrieval_stop() {
        let action = BeforeRetrievalAction::Stop("blocked".into());
        let mut ctx = HookContext::new("conv1".into(), "query".into(), HookPhase::BeforeRetrieval);
        assert!(!apply_before_retrieval_action(&mut ctx, action).unwrap());
    }

    #[test]
    fn tc_th_004_after_retrieval_filter_results() {
        let results = vec![RetrievalResult {
            chunk: echomind_models::Chunk {
                id: "c1".into(),
                doc_id: "d1".into(),
                content: "content".into(),
                token_count: 10,
                sequence: 0,
            },
            doc_name: "doc1".into(),
            score: 0.9,
        }];
        let action = AfterRetrievalAction::FilterResults(results.clone());
        let mut ctx = HookContext::new("conv1".into(), "query".into(), HookPhase::AfterRetrieval);
        ctx.retrieval_results = vec![];
        assert!(apply_after_retrieval_action(&mut ctx, action).unwrap());
        assert_eq!(ctx.retrieval_results.len(), 1);
        assert_eq!(ctx.retrieval_results[0].chunk.doc_id, "d1");
    }

    #[test]
    fn tc_th_005_before_generation_patch_context() {
        let patch = RequestPatch {
            temperature: Some(0.5),
            max_tokens: Some(2048),
            extra_context: Some("extra info".into()),
        };
        let action = BeforeGenerationAction::PatchContext(patch.clone());
        let mut ctx = HookContext::new("conv1".into(), "query".into(), HookPhase::BeforeGeneration);
        let result = apply_before_generation_action(&mut ctx, action).unwrap();
        assert!(result.is_some());
        let p = result.unwrap();
        assert_eq!(p.temperature, Some(0.5));
        assert_eq!(p.max_tokens, Some(2048));
        assert_eq!(p.extra_context.as_deref(), Some("extra info"));
    }

    #[test]
    fn tc_th_006_model_response_retry_with_feedback() {
        let action = ModelResponseAction::RetryWithFeedback("answer was incomplete".into());
        let mut ctx = HookContext::new("conv1".into(), "query".into(), HookPhase::AfterGeneration);
        let result = apply_model_response_action(&mut ctx, action).unwrap();
        assert_eq!(result.as_deref(), Some("answer was incomplete"));
    }

    // ─── RequestPatch 默认值 ───

    #[test]
    fn tc_th_007_request_patch_default() {
        let patch = RequestPatch::default();
        assert!(patch.temperature.is_none());
        assert!(patch.max_tokens.is_none());
        assert!(patch.extra_context.is_none());
    }

    // ─── TypedHook trait 默认实现 ───

    struct NoOpHook;

    impl TypedHook for NoOpHook {
        fn name(&self) -> &str {
            "noop"
        }
    }

    #[tokio::test]
    async fn tc_th_008_typed_hook_default_returns_continue() {
        let hook = NoOpHook;
        let mut ctx = HookContext::new("c1".into(), "q".into(), HookPhase::BeforeRetrieval);
        let action = hook.on_before_retrieval(&mut ctx).await.unwrap();
        assert!(matches!(action, BeforeRetrievalAction::Continue));

        let action = hook.on_after_retrieval(&mut ctx).await.unwrap();
        assert!(matches!(action, AfterRetrievalAction::Continue));

        let action = hook.on_before_generation(&mut ctx).await.unwrap();
        assert!(matches!(action, BeforeGenerationAction::Continue));

        let action = hook.on_model_response(&mut ctx).await.unwrap();
        assert!(matches!(action, ModelResponseAction::Continue));
    }

    // ─── TypedHook 自定义实现 ───

    struct QueryRewriteHook;

    impl TypedHook for QueryRewriteHook {
        fn name(&self) -> &str {
            "query_rewriter"
        }

        fn on_before_retrieval<'a>(
            &'a self,
            _ctx: &'a mut HookContext,
        ) -> Pin<
            Box<
                dyn std::future::Future<Output = anyhow::Result<BeforeRetrievalAction>> + Send + 'a,
            >,
        > {
            Box::pin(async { Ok(BeforeRetrievalAction::PatchQuery("rewritten".into())) })
        }
    }

    #[tokio::test]
    async fn tc_th_009_custom_hook_patches_query() {
        let hook = QueryRewriteHook;
        let mut ctx = HookContext::new("c1".into(), "original".into(), HookPhase::BeforeRetrieval);
        let action = hook.on_before_retrieval(&mut ctx).await.unwrap();
        assert!(matches!(action, BeforeRetrievalAction::PatchQuery(_)));
        apply_before_retrieval_action(&mut ctx, action).unwrap();
        assert_eq!(ctx.query, "rewritten");
    }

    #[test]
    fn tc_th_010_typed_hook_enabled_default() {
        let hook = NoOpHook;
        assert!(hook.enabled());
    }
}
