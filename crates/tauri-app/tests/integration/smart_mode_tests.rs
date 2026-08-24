#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! REQ-RAG-050 智能模式 RAG 参数调优 TDD 测试。
//!
//! 智能模式开启时，RAG 检索参数应自动使用优化值：
//! - top_k = 8（扩大候选池）
//! - score_threshold = 0.05（降低过滤门槛）
//! - chunk_expansion_enabled = true（增加上下文窗口）
//!
//! 智能模式关闭时恢复用户手动设置值。

use echomind_core::Storage;
use echomind_tauri_app::commands::{get_smart_mode_inner, set_smart_mode_inner};
use echomind_tauri_app::state::AppState;

/// 创建临时 AppState。
async fn make_state() -> (AppState, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    (state, dir)
}

/// TC-RAG-SMART-001: 智能模式默认开启（首次安装无设置项）
#[tokio::test]
async fn tc_rag_smart_001_default_enabled() {
    let (state, _dir) = make_state().await;
    let enabled = get_smart_mode_inner(&state).await.unwrap();
    assert!(enabled, "智能模式默认应开启");
}

/// TC-RAG-SMART-002: 智能模式开启时设置 RAG 优化参数
#[tokio::test]
async fn tc_rag_smart_002_enable_sets_optimized_params() {
    let (state, _dir) = make_state().await;

    // 开启智能模式
    set_smart_mode_inner(true, &state).await.unwrap();

    // 验证 RAG 参数已设为优化值
    let top_k = state.storage.get_setting("rag.top_k").await.unwrap();
    assert_eq!(top_k.as_deref(), Some("8"), "智能模式开启后 top_k 应为 8");

    let threshold = state
        .storage
        .get_setting("rag.score_threshold")
        .await
        .unwrap();
    assert_eq!(
        threshold.as_deref(),
        Some("0.05"),
        "智能模式开启后 score_threshold 应为 0.05"
    );

    let expansion = state
        .storage
        .get_setting("rag.chunk_expansion_enabled")
        .await
        .unwrap();
    assert_eq!(
        expansion.as_deref(),
        Some("true"),
        "智能模式开启后 chunk_expansion 应为 true"
    );
}

/// TC-RAG-SMART-003: 智能模式关闭时恢复用户手动设置值
#[tokio::test]
async fn tc_rag_smart_003_disable_restores_user_params() {
    let (state, _dir) = make_state().await;

    // 先设置用户自定义值
    state.storage.set_setting("rag.top_k", "3").await.unwrap();
    state
        .storage
        .set_setting("rag.score_threshold", "0.2")
        .await
        .unwrap();
    state
        .storage
        .set_setting("rag.chunk_expansion_enabled", "false")
        .await
        .unwrap();

    // 开启智能模式（备份用户值 + 设为优化值）
    set_smart_mode_inner(true, &state).await.unwrap();

    // 关闭智能模式（恢复用户值）
    set_smart_mode_inner(false, &state).await.unwrap();

    // 验证恢复
    let top_k = state.storage.get_setting("rag.top_k").await.unwrap();
    assert_eq!(top_k.as_deref(), Some("3"), "关闭后 top_k 应恢复为用户值 3");

    let threshold = state
        .storage
        .get_setting("rag.score_threshold")
        .await
        .unwrap();
    assert_eq!(
        threshold.as_deref(),
        Some("0.2"),
        "关闭后 score_threshold 应恢复为用户值 0.2"
    );

    let expansion = state
        .storage
        .get_setting("rag.chunk_expansion_enabled")
        .await
        .unwrap();
    assert_eq!(
        expansion.as_deref(),
        Some("false"),
        "关闭后 chunk_expansion 应恢复为用户值 false"
    );
}

/// TC-RAG-SMART-004: 智能模式切换可重复（开→关→开）
#[tokio::test]
async fn tc_rag_smart_004_toggle_repeatable() {
    let (state, _dir) = make_state().await;

    // 第一次开启
    set_smart_mode_inner(true, &state).await.unwrap();
    assert!(get_smart_mode_inner(&state).await.unwrap());

    // 关闭
    set_smart_mode_inner(false, &state).await.unwrap();
    assert!(!get_smart_mode_inner(&state).await.unwrap());

    // 第二次开启
    set_smart_mode_inner(true, &state).await.unwrap();
    assert!(get_smart_mode_inner(&state).await.unwrap());

    // 验证第二次开启后参数正确
    let top_k = state.storage.get_setting("rag.top_k").await.unwrap();
    assert_eq!(top_k.as_deref(), Some("8"), "第二次开启后 top_k 应为 8");
}

/// TC-RAG-SMART-005: 智能模式参数变更持久化到 settings 表
#[tokio::test]
async fn tc_rag_smart_005_params_persisted() {
    let (state, _dir) = make_state().await;

    set_smart_mode_inner(true, &state).await.unwrap();

    // 验证 smart_mode.enabled 持久化
    let enabled = state
        .storage
        .get_setting("smart_mode.enabled")
        .await
        .unwrap();
    assert_eq!(
        enabled.as_deref(),
        Some("true"),
        "smart_mode.enabled 应持久化为 true"
    );

    // 验证 rag.top_k 持久化
    let top_k = state.storage.get_setting("rag.top_k").await.unwrap();
    assert_eq!(top_k.as_deref(), Some("8"), "rag.top_k 应持久化为 8");
}
