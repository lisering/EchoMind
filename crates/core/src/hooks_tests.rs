#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Agent 生命周期 Hooks 系统单元测试（TC-HOOK-001~007，REQ-RAG-029）。

use crate::hooks::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// 测试用 Hook：记录执行次数。
struct CountingHook {
    name: String,
    phase: HookPhase,
    call_count: Arc<AtomicU32>,
    /// 执行序号记录（用于验证执行顺序）。
    order: Option<Arc<AtomicU32>>,
}

impl CountingHook {
    fn new(name: &str, phase: HookPhase, call_count: Arc<AtomicU32>) -> Self {
        Self {
            name: name.to_string(),
            phase,
            call_count,
            order: None,
        }
    }

    /// 带序号记录的构造（用于验证执行顺序）。
    fn with_order(
        name: &str,
        phase: HookPhase,
        call_count: Arc<AtomicU32>,
        order: Arc<AtomicU32>,
    ) -> Self {
        Self {
            name: name.to_string(),
            phase,
            call_count,
            order: Some(order),
        }
    }
}

impl AgentHook for CountingHook {
    fn name(&self) -> &str {
        &self.name
    }
    fn phase(&self) -> HookPhase {
        self.phase
    }
    fn execute<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if let Some(ref order) = self.order {
                order.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        })
    }
}

/// 测试用 Hook：修改查询文本。
struct QueryRewriteHook {
    new_query: String,
}

impl AgentHook for QueryRewriteHook {
    fn name(&self) -> &str {
        "query_rewriter"
    }
    fn phase(&self) -> HookPhase {
        HookPhase::BeforeRetrieval
    }
    fn execute<'a>(
        &'a self,
        ctx: &'a mut HookContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        let new_query = self.new_query.clone();
        Box::pin(async move {
            ctx.query = new_query;
            Ok(())
        })
    }
}

/// 测试用 Hook：返回错误。
struct FailingHook;

impl AgentHook for FailingHook {
    fn name(&self) -> &str {
        "failing"
    }
    fn phase(&self) -> HookPhase {
        HookPhase::BeforeRetrieval
    }
    fn execute<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async move { anyhow::bail!("intentional failure") })
    }
}

/// 测试用 Hook：禁用。
struct DisabledHook;

impl AgentHook for DisabledHook {
    fn name(&self) -> &str {
        "disabled"
    }
    fn phase(&self) -> HookPhase {
        HookPhase::BeforeRetrieval
    }
    fn execute<'a>(
        &'a self,
        _ctx: &'a mut HookContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async move {
            panic!("disabled hook should not execute");
        })
    }
    fn enabled(&self) -> bool {
        false
    }
}

// ============================================================
// TC-HOOK-001: 注册 hook 后 run_phase 正确调用
// ============================================================
#[tokio::test]
async fn tc_hook_001() {
    let call_count = Arc::new(AtomicU32::new(0));
    let mut registry = HookRegistry::new();
    registry.register(Box::new(CountingHook::new(
        "test_hook",
        HookPhase::BeforeRetrieval,
        Arc::clone(&call_count),
    )));

    let mut ctx = HookContext::new(
        "conv-1".to_string(),
        "original query".to_string(),
        HookPhase::BeforeRetrieval,
    );

    registry
        .run_phase(HookPhase::BeforeRetrieval, &mut ctx)
        .await
        .unwrap();

    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ============================================================
// TC-HOOK-002: 多个同阶段 hook 按注册顺序执行
// ============================================================
#[tokio::test]
async fn tc_hook_002() {
    let count1 = Arc::new(AtomicU32::new(0));
    let count2 = Arc::new(AtomicU32::new(0));
    let count3 = Arc::new(AtomicU32::new(0));
    let order_counter = Arc::new(AtomicU32::new(0));

    let mut registry = HookRegistry::new();
    registry.register(Box::new(CountingHook::with_order(
        "hook_1",
        HookPhase::BeforeRetrieval,
        Arc::clone(&count1),
        Arc::clone(&order_counter),
    )));
    registry.register(Box::new(CountingHook::with_order(
        "hook_2",
        HookPhase::BeforeRetrieval,
        Arc::clone(&count2),
        Arc::clone(&order_counter),
    )));
    registry.register(Box::new(CountingHook::with_order(
        "hook_3",
        HookPhase::BeforeRetrieval,
        Arc::clone(&count3),
        Arc::clone(&order_counter),
    )));

    let mut ctx = HookContext::new(
        String::new(),
        "query".to_string(),
        HookPhase::BeforeRetrieval,
    );

    registry
        .run_phase(HookPhase::BeforeRetrieval, &mut ctx)
        .await
        .unwrap();

    // 全部 3 个 hook 都执行了
    assert_eq!(count1.load(Ordering::SeqCst), 1);
    assert_eq!(count2.load(Ordering::SeqCst), 1);
    assert_eq!(count3.load(Ordering::SeqCst), 1);
    // order_counter 最终为 3（3 个 hook 依次递增）
    assert_eq!(order_counter.load(Ordering::SeqCst), 3);
}

// ============================================================
// TC-HOOK-003: disabled hook 不执行
// ============================================================
#[tokio::test]
async fn tc_hook_003() {
    let call_count = Arc::new(AtomicU32::new(0));
    let mut registry = HookRegistry::new();
    registry.register(Box::new(DisabledHook));
    registry.register(Box::new(CountingHook::new(
        "active_hook",
        HookPhase::BeforeRetrieval,
        Arc::clone(&call_count),
    )));

    let mut ctx = HookContext::new(
        String::new(),
        "query".to_string(),
        HookPhase::BeforeRetrieval,
    );

    // 不应 panic（DisabledHook 被跳过）
    registry
        .run_phase(HookPhase::BeforeRetrieval, &mut ctx)
        .await
        .unwrap();

    // 只有 active_hook 执行了
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

// ============================================================
// TC-HOOK-004: hook 可修改 HookContext
// ============================================================
#[tokio::test]
async fn tc_hook_004() {
    let mut registry = HookRegistry::new();
    registry.register(Box::new(QueryRewriteHook {
        new_query: "改写后的查询".to_string(),
    }));

    let mut ctx = HookContext::new(
        "conv-123".to_string(),
        "原始查询".to_string(),
        HookPhase::BeforeRetrieval,
    );

    registry
        .run_phase(HookPhase::BeforeRetrieval, &mut ctx)
        .await
        .unwrap();

    assert_eq!(ctx.query, "改写后的查询");
}

// ============================================================
// TC-HOOK-005: hook 返回 Err 时中断后续 hook
// ============================================================
#[tokio::test]
async fn tc_hook_005() {
    let call_count = Arc::new(AtomicU32::new(0));
    let mut registry = HookRegistry::new();
    registry.register(Box::new(FailingHook));
    registry.register(Box::new(CountingHook::new(
        "after_failing",
        HookPhase::BeforeRetrieval,
        Arc::clone(&call_count),
    )));

    let mut ctx = HookContext::new(
        String::new(),
        "query".to_string(),
        HookPhase::BeforeRetrieval,
    );

    let result = registry
        .run_phase(HookPhase::BeforeRetrieval, &mut ctx)
        .await;

    // run_phase 返回 Err
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("intentional failure")
    );
    // 后续 hook 未执行
    assert_eq!(call_count.load(Ordering::SeqCst), 0);
}

// ============================================================
// TC-HOOK-006: 不同阶段的 hook 互不干扰
// ============================================================
#[tokio::test]
async fn tc_hook_006() {
    let before_count = Arc::new(AtomicU32::new(0));
    let after_count = Arc::new(AtomicU32::new(0));

    let mut registry = HookRegistry::new();
    registry.register(Box::new(CountingHook::new(
        "before_hook",
        HookPhase::BeforeRetrieval,
        Arc::clone(&before_count),
    )));
    registry.register(Box::new(CountingHook::new(
        "after_hook",
        HookPhase::AfterRetrieval,
        Arc::clone(&after_count),
    )));

    // 运行 BeforeRetrieval 阶段
    let mut ctx1 = HookContext::new(
        String::new(),
        "query".to_string(),
        HookPhase::BeforeRetrieval,
    );
    registry
        .run_phase(HookPhase::BeforeRetrieval, &mut ctx1)
        .await
        .unwrap();

    assert_eq!(before_count.load(Ordering::SeqCst), 1);
    assert_eq!(after_count.load(Ordering::SeqCst), 0);

    // 运行 AfterRetrieval 阶段
    let mut ctx2 = HookContext::new(
        String::new(),
        "query".to_string(),
        HookPhase::AfterRetrieval,
    );
    registry
        .run_phase(HookPhase::AfterRetrieval, &mut ctx2)
        .await
        .unwrap();

    assert_eq!(before_count.load(Ordering::SeqCst), 1);
    assert_eq!(after_count.load(Ordering::SeqCst), 1);
}

// ============================================================
// TC-HOOK-007: HookRegistry is_empty / count 正确
// ============================================================
#[tokio::test]
async fn tc_hook_007() {
    let registry = HookRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.count(), 0);

    let mut registry = HookRegistry::new();
    registry.register(Box::new(CountingHook::new(
        "h1",
        HookPhase::BeforeRetrieval,
        Arc::new(AtomicU32::new(0)),
    )));
    registry.register(Box::new(CountingHook::new(
        "h2",
        HookPhase::AfterRetrieval,
        Arc::new(AtomicU32::new(0)),
    )));
    registry.register(Box::new(CountingHook::new(
        "h3",
        HookPhase::BeforeGeneration,
        Arc::new(AtomicU32::new(0)),
    )));

    assert!(!registry.is_empty());
    assert_eq!(registry.count(), 3);
}
