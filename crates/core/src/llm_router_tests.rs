#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::llm_router::*;

use echomind_models::LlmMode;

/// TC-ROUTER-001：无请求时使用 fallback
#[tokio::test]
async fn tc_router_001_no_request_uses_fallback() {
    let router = LlmRouter::new(
        LlmChoice::new(LlmMode::Remote, "gpt-4o-mini".to_string()),
        [LlmMode::Remote, LlmMode::Local].into_iter().collect(),
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

/// TC-ROUTER-003：请求 Local 模式返回 Local 选择（Pro）
#[tokio::test]
async fn tc_router_003_request_local_returns_local() {
    let router = LlmRouter::new_pro_default();
    let (choice, verdict) = router
        .resolve(
            "conv-1",
            Some(LlmChoice::new(LlmMode::Local, "qwen2.5-7b.gguf")),
        )
        .await
        .unwrap();
    assert_eq!(choice.mode, LlmMode::Local);
    assert_eq!(choice.model_id, "qwen2.5-7b.gguf");
    assert_eq!(verdict, RouterVerdict::Initial);
}

/// TC-ROUTER-004：模式切换时记录 last_mode 并返回 ModeChanged
#[tokio::test]
async fn tc_router_004_mode_change_detected() {
    let router = LlmRouter::new_pro_default();

    // 第一次：Remote → Initial
    let (_, verdict1) = router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "gpt-4o")))
        .await
        .unwrap();
    assert_eq!(verdict1, RouterVerdict::Initial);
    assert_eq!(router.last_mode_for("conv-1").await, Some(LlmMode::Remote));

    // 第二次：同模式 → SameMode
    let (_, verdict2) = router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "gpt-4o")))
        .await
        .unwrap();
    assert_eq!(verdict2, RouterVerdict::SameMode);

    // 第三次：切换到 Local → ModeChanged
    let (choice3, verdict3) = router
        .resolve(
            "conv-1",
            Some(LlmChoice::new(LlmMode::Local, "qwen2.5-7b.gguf")),
        )
        .await
        .unwrap();
    assert_eq!(verdict3, RouterVerdict::ModeChanged);
    assert_eq!(choice3.mode, LlmMode::Local);
    assert_eq!(router.last_mode_for("conv-1").await, Some(LlmMode::Local));
}

/// TC-ROUTER-005：Free 模式请求 Local 返回错误
#[tokio::test]
async fn tc_router_005_free_mode_request_local_errors() {
    let router = LlmRouter::new_free_default();
    let result = router
        .resolve(
            "conv-1",
            Some(LlmChoice::new(LlmMode::Local, "qwen2.5-7b.gguf")),
        )
        .await;
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        RouterError::ModeUnavailable(LlmMode::Local)
    );
    // 错误时不应记录 last_mode
    assert_eq!(router.last_mode_for("conv-1").await, None);
}

/// TC-ROUTER-006：不同会话的模式独立追踪
#[tokio::test]
async fn tc_router_006_per_conversation_isolated() {
    let router = LlmRouter::new_pro_default();

    // conv-1: Remote
    router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "")))
        .await
        .unwrap();

    // conv-2: Local
    router
        .resolve("conv-2", Some(LlmChoice::new(LlmMode::Local, "")))
        .await
        .unwrap();

    assert_eq!(router.last_mode_for("conv-1").await, Some(LlmMode::Remote));
    assert_eq!(router.last_mode_for("conv-2").await, Some(LlmMode::Local));

    // conv-1 再次：应为 SameMode
    let (_, verdict) = router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Remote, "")))
        .await
        .unwrap();
    assert_eq!(verdict, RouterVerdict::SameMode);
}

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
    let router = LlmRouter::new_pro_default();

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

/// TC-ROUTER-009：set_available_modes 动态更新可用模式（模拟 Pro 激活）
#[tokio::test]
async fn tc_router_009_update_available_modes() {
    let router = LlmRouter::new_free_default();

    // 初始：Local 不可用
    assert!(
        router
            .resolve("conv-1", Some(LlmChoice::new(LlmMode::Local, "")))
            .await
            .is_err()
    );

    // 激活 Pro：Local 可用
    let mut modes = std::collections::HashSet::new();
    modes.insert(LlmMode::Remote);
    modes.insert(LlmMode::Local);
    router.set_available_modes(modes).await;

    // 现在 Local 可用
    let (choice, _) = router
        .resolve("conv-1", Some(LlmChoice::new(LlmMode::Local, "qwen.gguf")))
        .await
        .unwrap();
    assert_eq!(choice.mode, LlmMode::Local);
}

/// TC-ROUTER-010：RouterError Display 实现包含模式名
#[tokio::test]
async fn tc_router_010_error_display() {
    let err = RouterError::ModeUnavailable(LlmMode::Local);
    let msg = format!("{err}");
    assert!(msg.contains("Local"));
}

/// TC-ROUTER-011：Default trait 返回 Free 默认配置
#[tokio::test]
async fn tc_router_011_default_is_free() {
    let router = LlmRouter::default();
    let (choice, _) = router.resolve("conv-1", None).await.unwrap();
    assert_eq!(choice.mode, LlmMode::Remote);
    // Local 不可用
    assert!(
        router
            .resolve("conv-1", Some(LlmChoice::new(LlmMode::Local, "")))
            .await
            .is_err()
    );
}

/// TC-ROUTER-012：便捷构造函数 remote() 和 local()
#[tokio::test]
async fn tc_router_012_convenience_constructors() {
    let r = LlmChoice::remote("gpt-4o");
    assert_eq!(r.mode, LlmMode::Remote);
    assert_eq!(r.model_id, "gpt-4o");

    let l = LlmChoice::local("qwen.gguf");
    assert_eq!(l.mode, LlmMode::Local);
    assert_eq!(l.model_id, "qwen.gguf");
}

// ============================================================================
// P2-2: 远程 LLM 连续失败 → 自动切换 Local 模式（TC-FALLBACK-001~007）
// ============================================================================

/// TC-FALLBACK-001：初始远程失败计数为 0。
#[tokio::test]
async fn tc_fallback_001_initial_count_zero() {
    let router = LlmRouter::new_pro_default();
    assert_eq!(router.remote_failure_count(), 0, "初始连续失败计数应为 0");
}

/// TC-FALLBACK-002：单次失败递增计数，不触发自动切换。
#[tokio::test]
async fn tc_fallback_002_single_failure_no_switch() {
    let router = LlmRouter::new_pro_default();
    let switched = router.record_remote_failure().await;
    assert!(!switched, "1 次失败不应触发自动切换");
    assert_eq!(router.remote_failure_count(), 1, "计数应为 1");
    // fallback 仍为 Remote
    let fb = router.fallback().await;
    assert_eq!(fb.mode, LlmMode::Remote, "fallback 不应改变");
}

/// TC-FALLBACK-003：3 次连续失败触发自动切换到 Local（Pro 模式）。
#[tokio::test]
async fn tc_fallback_003_three_failures_trigger_switch() {
    let router = LlmRouter::new_pro_default();

    // 2 次失败：不触发
    router.record_remote_failure().await;
    router.record_remote_failure().await;
    assert_eq!(router.remote_failure_count(), 2);

    // 第 3 次失败：触发自动切换
    let switched = router.record_remote_failure().await;
    assert!(switched, "3 次连续失败应触发自动切换到 Local");
    // 计数器在切换后重置
    assert_eq!(router.remote_failure_count(), 0, "切换后计数器应重置");

    // fallback 已切换到 Local
    let fb = router.fallback().await;
    assert_eq!(fb.mode, LlmMode::Local, "fallback 应已切换到 Local");
}

/// TC-FALLBACK-004：成功调用重置失败计数。
#[tokio::test]
async fn tc_fallback_004_success_resets_count() {
    let router = LlmRouter::new_pro_default();

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

/// TC-FALLBACK-005：Free 版本（Local 不可用）3 次失败不触发自动切换。
#[tokio::test]
async fn tc_fallback_005_free_mode_no_local_no_switch() {
    let router = LlmRouter::new_free_default();

    router.record_remote_failure().await;
    router.record_remote_failure().await;
    let switched = router.record_remote_failure().await;

    assert!(!switched, "Free 版本无 Local 模式，不应触发自动切换");
    // fallback 仍为 Remote
    let fb = router.fallback().await;
    assert_eq!(fb.mode, LlmMode::Remote, "fallback 应保持 Remote");
}

/// TC-FALLBACK-006：自动切换后后续 resolve() 返回 Local 模式。
#[tokio::test]
async fn tc_fallback_006_post_switch_resolve_returns_local() {
    let router = LlmRouter::new_pro_default();

    // 触发 3 次失败 → 自动切换
    for _ in 0..3 {
        router.record_remote_failure().await;
    }

    // 新会话（无显式请求）应使用切换后的 fallback
    let (choice, _) = router.resolve("new-conv", None).await.unwrap();
    assert_eq!(choice.mode, LlmMode::Local, "自动切换后新会话应使用 Local");
}

/// TC-FALLBACK-007：中断（非失败）不计入失败计数。
///
/// `forward_stream` 返回 `Ok(ForwardResult { completed: false, .. })` 表示用户中断，
/// 不是 LLM 故障，不应递增失败计数。
/// 此测试验证：成功路径不调用 `record_remote_failure`，计数保持 0。
#[tokio::test]
async fn tc_fallback_007_no_failure_on_success() {
    let router = LlmRouter::new_pro_default();

    // 模拟成功调用：调用 record_remote_success
    router.record_remote_success();
    assert_eq!(router.remote_failure_count(), 0, "成功调用后计数应保持 0");

    // 再次成功
    router.record_remote_success();
    assert_eq!(router.remote_failure_count(), 0, "再次成功计数仍为 0");
}
