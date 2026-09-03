#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::llm_router::*;

use echomind_models::LlmMode;

/// TC-ROUTER-001：无请求时使用 fallback
#[tokio::test]
async fn tc_router_001_no_request_uses_fallback() {
    let router = LlmRouter::new(
        LlmChoice::new(LlmMode::Remote, "gpt-4o-mini".to_string()),
        [LlmMode::Remote].into_iter().collect(),
    );
    let (choice, verdict) = router.resolve("conv-1", None).await.unwrap();
    assert_eq!(choice.mode, LlmMode::Remote);
    assert_eq!(choice.model_id, "gpt-4o-mini");
    assert_eq!(verdict, RouterVerdict::Initial);
}

/// TC-ROUTER-002：请求 Remote 模式返回 Remote 选择
#[tokio::test]
async fn tc_router_002_request_remote_returns_remote() {
    let router = LlmRouter::new_free_default();
    let (choice, verdict) = router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "gpt-4o")))
        .await
        .unwrap();
    assert_eq!(choice.mode, LlmMode::Remote);
    assert_eq!(choice.model_id, "gpt-4o");
    assert_eq!(verdict, RouterVerdict::Initial);
}

// Local 模式已移除（Phase 1 R3 重构）
// tc_router_003/004/005/006 (Local 模式相关) 已删除

/// TC-ROUTER-007：set_fallback 更新默认选择
#[tokio::test]
async fn tc_router_007_set_fallback_updates_default() {
    let router = LlmRouter::new_free_default();

    // 初始 fallback 是 Remote，model_id 为空
    let (choice, _) = router.resolve("conv-1", None).await.unwrap();
    assert_eq!(choice.mode, LlmMode::Remote);
    assert_eq!(choice.model_id, "");

    // 更新 fallback
    router
        .set_fallback(LlmChoice::new(LlmMode::Remote, "claude-3-opus".to_string()))
        .await;

    // 新会话使用新 fallback
    let (choice2, _) = router.resolve("conv-2", None).await.unwrap();
    assert_eq!(choice2.model_id, "claude-3-opus");
}

/// TC-ROUTER-008：clear 清除指定会话记录
#[tokio::test]
async fn tc_router_008_clear_conversation() {
    let router = LlmRouter::new_free_default();

    router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "")))
        .await
        .unwrap();
    assert!(router.last_mode_for("conv-1").await.is_some());

    router.clear("conv-1").await;
    assert!(router.last_mode_for("conv-1").await.is_none());

    // 清除后再次路由应为 Initial
    let (_, verdict) = router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "")))
        .await
        .unwrap();
    assert_eq!(verdict, RouterVerdict::Initial);
}

// Local 模式已移除（Phase 1 R3 重构）
// tc_router_009 (set_available_modes 动态更新) 已删除

// Local 模式已移除（Phase 1 R3 重构）
// tc_router_010 (RouterError Display 含 Local) 已删除

/// TC-ROUTER-011：Default trait 返回默认配置
#[tokio::test]
async fn tc_router_011_default_is_free() {
    let router = LlmRouter::default();
    let (choice, _) = router.resolve("conv-1", None).await.unwrap();
    assert_eq!(choice.mode, LlmMode::Remote);
}

/// TC-ROUTER-012：便捷构造函数 remote()
#[tokio::test]
async fn tc_router_012_convenience_constructor_remote() {
    let r = LlmChoice::remote("gpt-4o");
    assert_eq!(r.mode, LlmMode::Remote);
    assert_eq!(r.model_id, "gpt-4o");
}

// ============================================================================
// P2-2: 远程 LLM 连续失败计数（Local 模式已移除，降级逻辑废弃）
// ============================================================================

/// TC-FALLBACK-001：初始远程失败计数为 0。
#[tokio::test]
async fn tc_fallback_001_initial_count_zero() {
    let router = LlmRouter::new_free_default();
    assert_eq!(router.remote_failure_count(), 0, "初始连续失败计数应为 0");
}

/// TC-FALLBACK-002：单次失败递增计数（Local 模式已移除，不再触发自动切换）。
#[tokio::test]
async fn tc_fallback_002_single_failure_no_switch() {
    let router = LlmRouter::new_free_default();
    let switched = router.record_remote_failure().await;
    assert!(!switched, "Local 模式已移除，不应触发自动切换");
    assert_eq!(router.remote_failure_count(), 1, "计数应为 1");
    // fallback 仍为 Remote
    let fb = router.fallback().await;
    assert_eq!(fb.mode, LlmMode::Remote, "fallback 不应改变");
}

/// TC-FALLBACK-004：成功调用重置失败计数。
#[tokio::test]
async fn tc_fallback_004_success_resets_count() {
    let router = LlmRouter::new_free_default();

    // 2 次失败
    router.record_remote_failure().await;
    router.record_remote_failure().await;
    assert_eq!(router.remote_failure_count(), 2);

    // 成功调用重置
    router.record_remote_success();
    assert_eq!(router.remote_failure_count(), 0, "成功后计数应重置为 0");

    // 再失败 1 次：不应触发（因为计数从 0 开始）
    let switched = router.record_remote_failure().await;
    assert!(!switched, "重置后 1 次失败不应触发切换");
}

/// TC-FALLBACK-007：中断（非失败）不计入失败计数。
///
/// `forward_stream` 返回 `Ok(ForwardResult { completed: false, .. })` 表示用户中断，
/// 不是 LLM 故障，不应递增失败计数。
/// 此测试验证：成功路径不调用 `record_remote_failure`，计数保持 0。
#[tokio::test]
async fn tc_fallback_007_no_failure_on_success() {
    let router = LlmRouter::new_free_default();

    // 模拟成功调用：调用 record_remote_success
    router.record_remote_success();
    assert_eq!(router.remote_failure_count(), 0, "成功调用后计数应保持 0");

    // 再次成功
    router.record_remote_success();
    assert_eq!(router.remote_failure_count(), 0, "再次成功计数仍为 0");
}
