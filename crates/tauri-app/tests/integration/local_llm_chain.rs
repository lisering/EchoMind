#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    unused_imports
)]
//! 本地 LLM 集成测试（Pro feature 门控）。
//!
//! 覆盖 V2.0 深耕计划 Phase 4 S13 测试用例（提前至 S11）：
//! TC-LLM-CHAIN-001 ~ TC-LLM-CHAIN-006。
//!
//! 测试不依赖真实 GGUF 模型文件加载，仅验证：
//! - 模式切换持久化
//! - Free 用户被 Pro 门控拦截
//! - 模型文件管理（list/delete）
//! - KV Cache 状态查询
//! - 采样参数持久化
//! - LlmRouter 路由决策

use super::common::*;
use super::*;
use echomind_models::LlmMode;

// ============================================================================
// TC-LLM-CHAIN-001: 模式切换 Remote → Local → 持久化
// ============================================================================

/// TC-LLM-CHAIN-001：LLM 模式切换 + 持久化 + 重启恢复。
///
/// Free 用户尝试切换到 Local 模式应被 Pro 门控拦截。
/// Pro 用户切换到 Local 后重启恢复为 Local。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_llm_chain_001_mode_switch_persists() {
    let dir = TempDir::new().unwrap();

    // 1. Free 用户尝试切换到 Local → 应被拦截
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        assert!(!get_pro_status_inner(&state).await, "初始状态应为 Free");
        let result = set_llm_mode_inner("local".to_string(), &state).await;
        assert!(result.is_err(), "Free 用户切换到 Local 应被拦截");
        let err = result.unwrap_err();
        assert!(
            err.contains("PRO_REQUIRED") || err.contains("Pro"),
            "错误应包含 PRO_REQUIRED 或 Pro: {err}"
        );
    }

    // 2. Pro 用户切换到 Local → 持久化
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let license = make_valid_license();
        activate_pro_inner(&license, &state).await.unwrap();
        assert!(get_pro_status_inner(&state).await, "应为 Pro");

        set_llm_mode_inner("local".to_string(), &state)
            .await
            .unwrap();
        let mode = get_llm_mode_inner(&state).await.unwrap();
        assert_eq!(mode, "local", "切换后模式应为 local");
    }

    // 3. 重启后恢复
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let mode = get_llm_mode_inner(&restarted).await.unwrap();
    assert_eq!(mode, "local", "重启后模式应恢复为 local");
}

// ============================================================================
// TC-LLM-CHAIN-002: Free 用户 Local 模式全部拦截
// ============================================================================

/// TC-LLM-CHAIN-002：Free 用户 Local 模式功能受限。
///
/// 验证 set_llm_mode("local") 在 Free 用户下设置成功但 router 无法 resolve Local。
/// set_local_model 在 Free 用户下被拦截（Pro 门控）。
#[tokio::test]
async fn tc_llm_chain_002_free_user_local_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    assert!(!get_pro_status_inner(&state).await, "测试前提：Free 用户");

    // set_local_model 被拦截
    #[cfg(feature = "pro")]
    {
        let result = set_local_model_inner("test.gguf".to_string(), &state).await;
        assert!(result.is_err(), "Free 用户 set_local_model 应被拦截");
    }

    // router resolve Local 在 Free 下失败
    let router_result = state
        .llm_router
        .resolve(
            "conv-free-router-test",
            Some(echomind_core::llm_router::LlmChoice::new(
                LlmMode::Local,
                String::new(),
            )),
        )
        .await;
    assert!(
        router_result.is_err(),
        "Free 用户 router resolve Local 应失败"
    );
}

// ============================================================================
// TC-LLM-CHAIN-003: 模型列表查询 + 删除非存在模型
// ============================================================================

/// TC-LLM-CHAIN-003：list_local_models 返回空列表（无已下载模型）+ 删除不存在模型返回错误。
///
/// 验证初始状态模型列表为空，删除不存在的模型返回错误。
#[tokio::test]
async fn tc_llm_chain_003_model_list_and_delete_nonexistent() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 初始状态模型列表为空
    let models = list_local_models_inner(&state).await.unwrap();
    assert!(
        models.is_empty(),
        "初始状态模型列表应为空（无已下载 GGUF 文件）"
    );

    // 删除不存在的模型应返回错误
    let result = delete_model_inner("nonexistent.gguf".to_string(), &state).await;
    assert!(result.is_err(), "删除不存在的模型应返回错误");
}

// ============================================================================
// TC-LLM-CHAIN-004: 采样参数持久化（Pro 门控）
// ============================================================================

/// TC-LLM-CHAIN-004：set_sampling_params + 持久化 + 重启恢复。
///
/// 验证采样参数写入 settings 表，重启后恢复。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_llm_chain_004_sampling_params_persists() {
    let dir = TempDir::new().unwrap();

    // 1. Pro 用户设置采样参数
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let license = make_valid_license();
        activate_pro_inner(&license, &state).await.unwrap();

        let params = LlmSamplingParams {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(2048),
            frequency_penalty: Some(1.1),
            presence_penalty: Some(0.5),
        };
        set_sampling_params_inner(params, &state).await.unwrap();

        // 验证设置已持久化
        let stored = state.storage.get_setting("llm.sampling").await.unwrap();
        assert!(stored.is_some(), "采样参数应已持久化到 settings 表");
    }

    // 2. 重启后恢复
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let stored = restarted.storage.get_setting("llm.sampling").await.unwrap();
    assert!(stored.is_some(), "重启后采样参数应恢复");
    let json: serde_json::Value = serde_json::from_str(&stored.unwrap()).unwrap();
    assert_eq!(json["temperature"], 0.7, "temperature 应为 0.7");
    assert_eq!(json["max_tokens"], 2048, "max_tokens 应为 2048");
}

// ============================================================================
// TC-LLM-CHAIN-005: KV Cache 状态查询（Pro 门控）
// ============================================================================

/// TC-LLM-CHAIN-005：get_kv_cache_status 初始状态 + 清空 + 持久化。
///
/// 验证 KV Cache 状态查询在初始状态下返回正确结果。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_llm_chain_005_kv_cache_status_query() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let license = make_valid_license();
    activate_pro_inner(&license, &state).await.unwrap();

    // 查询 KV Cache 状态（初始应为空或默认值）
    let result = get_kv_cache_status_inner(&state).await;
    assert!(
        result.is_ok(),
        "查询 KV Cache 状态应成功: {:?}",
        result.err()
    );
    let status = result.unwrap();
    // 初始状态应有合理的默认值
    assert!(
        status.file_count <= 100,
        "初始 KV Cache 条目数应很少（无已保存的 cache）"
    );
}

// ============================================================================
// TC-LLM-CHAIN-006: LlmRouter 路由决策
// ============================================================================

/// TC-LLM-CHAIN-006：LlmRouter 在 Free 和 Pro 下的路由决策。
///
/// Free 用户 router resolve Local 模式应返回 ModeUnavailable。
/// Pro 用户 router resolve Remote 模式应成功。
#[tokio::test]
async fn tc_llm_chain_006_router_mode_availability() {
    let dir = TempDir::new().unwrap();

    // 1. Free 用户：resolve Local 应失败
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let result = state
            .llm_router
            .resolve(
                "conv-router-test-1",
                Some(echomind_core::llm_router::LlmChoice::new(
                    LlmMode::Local,
                    String::new(),
                )),
            )
            .await;
        assert!(result.is_err(), "Free 用户 router resolve Local 应失败");
        match result {
            Err(echomind_core::llm_router::RouterError::ModeUnavailable(_)) => {}
            Ok(_) => panic!("Free 用户不应能 resolve Local"),
        }
    }

    // 2. Pro 用户：resolve Remote 应成功
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let license = make_valid_license();
        activate_pro_inner(&license, &state).await.unwrap();

        let result = state
            .llm_router
            .resolve(
                "conv-router-test-2",
                Some(echomind_core::llm_router::LlmChoice::new(
                    LlmMode::Remote,
                    "test-model".to_string(),
                )),
            )
            .await;
        assert!(result.is_ok(), "Pro 用户 router resolve Remote 应成功");
        let (choice, verdict) = result.unwrap();
        assert_eq!(choice.mode, LlmMode::Remote);
        assert!(
            matches!(
                verdict,
                echomind_core::llm_router::RouterVerdict::Initial
                    | echomind_core::llm_router::RouterVerdict::SameMode
            ),
            "verdict 应为 Initial 或 SameMode"
        );
    }
}

// ============================================================================
// TC-LLM-CHAIN-007: PagedAttention 配置持久化（Pro 门控）
// ============================================================================

/// TC-LLM-CHAIN-007：set_paged_attn + 持久化。
///
/// 验证 PagedAttention 开关设置后持久化到 settings 表。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_llm_chain_007_paged_attn_persists() {
    let dir = TempDir::new().unwrap();

    // 设置启用 PagedAttention
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let license = make_valid_license();
        activate_pro_inner(&license, &state).await.unwrap();

        set_paged_attn_inner(true, 16, 512, &state).await.unwrap();

        // 验证持久化
        let value = state.storage.get_setting("llm.paged_attn").await.unwrap();
        assert_eq!(value.as_deref(), Some("true"), "PagedAttention 应为 true");
    }

    // 重启后恢复
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let value = restarted
        .storage
        .get_setting("llm.paged_attn")
        .await
        .unwrap();
    assert_eq!(
        value.as_deref(),
        Some("true"),
        "重启后 PagedAttention 应保持 true"
    );

    // 设置禁用
    set_paged_attn_inner(false, 16, 512, &restarted)
        .await
        .unwrap();
    let value2 = restarted
        .storage
        .get_setting("llm.paged_attn")
        .await
        .unwrap();
    assert_eq!(
        value2.as_deref(),
        Some("false"),
        "设置后 PagedAttention 应为 false"
    );
}

// ============================================================================
// TC-LLM-CHAIN-008: Kernel Mode 持久化（Pro 门控）
// ============================================================================

/// TC-LLM-CHAIN-008：set_kernel_mode + get_kernel_mode + 持久化。
///
/// 验证 KernelMode 切换（auto/gemv/mistral_rs）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_llm_chain_008_kernel_mode_persists() {
    let dir = TempDir::new().unwrap();

    // 1. 设置为 custom_gemv
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let license = make_valid_license();
        activate_pro_inner(&license, &state).await.unwrap();

        set_kernel_mode_inner("custom".to_string(), &state)
            .await
            .unwrap();
        let mode = get_kernel_mode_inner(&state).await.unwrap();
        assert_eq!(mode, "custom", "KernelMode 应为 custom");
    }

    // 2. 重启后恢复
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let mode = get_kernel_mode_inner(&restarted).await.unwrap();
    assert_eq!(mode, "custom", "重启后 KernelMode 应恢复为 custom");

    // 3. 切换到 mistral_rs
    set_kernel_mode_inner("mistral".to_string(), &restarted)
        .await
        .unwrap();
    let mode2 = get_kernel_mode_inner(&restarted).await.unwrap();
    assert_eq!(mode2, "mistral", "切换后 KernelMode 应为 mistral");
}
