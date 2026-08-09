#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

//! TDD 测试：Prompt Caching 自动放置策略（TC-CACTAG-001~006）
//!
//! 测试策略：
//! - TC-CACTAG-001~002: Auto 策略在静态前缀放置 After 断点
//! - TC-CACTAG-003: None 策略不放置任何断点
//! - TC-CACTAG-004: Custom 策略按自定义配置放置
//! - TC-CACTAG-005: 辅助函数正确报告策略行为
//! - TC-CACTAG-006: CachedSegmentedPrompt 拼接正确

use crate::cache_policy::*;

// ============================================================
// TC-CACTAG-001: Auto 策略在静态前缀放置 After 断点
// ============================================================

#[test]
fn tc_cactag_001_auto_places_after_on_static_prefix() {
    let result = apply_cache_policy("角色描述。", "检索片段。", &CachePolicy::Auto);
    assert_eq!(
        result.static_prefix_cache,
        CacheBreakpoint::After,
        "Auto 策略应在静态前缀放置 After 断点"
    );
}

// ============================================================
// TC-CACTAG-002: Auto 策略不在动态上下文放置断点
// ============================================================

#[test]
fn tc_cactag_002_auto_no_breakpoint_on_dynamic() {
    let result = apply_cache_policy("角色描述。", "检索片段。", &CachePolicy::Auto);
    assert_eq!(
        result.dynamic_context_cache,
        CacheBreakpoint::None,
        "Auto 策略不应在动态上下文放置断点"
    );
}

// ============================================================
// TC-CACTAG-003: None 策略不放置任何断点
// ============================================================

#[test]
fn tc_cactag_003_none_no_breakpoints() {
    let result = apply_cache_policy("角色描述。", "检索片段。", &CachePolicy::None);
    assert_eq!(
        result.static_prefix_cache,
        CacheBreakpoint::None,
        "None 策略不应在静态前缀放置断点"
    );
    assert_eq!(
        result.dynamic_context_cache,
        CacheBreakpoint::None,
        "None 策略不应在动态上下文放置断点"
    );
}

// ============================================================
// TC-CACTAG-004: Custom 策略按自定义配置放置
// ============================================================

#[test]
fn tc_cactag_004_custom_respects_config() {
    let custom = CachePolicy::Custom(CachePolicyObject {
        system: Some(false),
        messages: Some(CacheMessagePolicy::None),
    });
    let result = apply_cache_policy("角色描述。", "检索片段。", &custom);
    assert_eq!(
        result.static_prefix_cache,
        CacheBreakpoint::None,
        "Custom system=false 不应在静态前缀放置断点"
    );

    let custom_on = CachePolicy::Custom(CachePolicyObject {
        system: Some(true),
        messages: Some(CacheMessagePolicy::LatestUserMessage),
    });
    let result2 = apply_cache_policy("角色描述。", "检索片段。", &custom_on);
    assert_eq!(
        result2.static_prefix_cache,
        CacheBreakpoint::After,
        "Custom system=true 应在静态前缀放置 After 断点"
    );
}

// ============================================================
// TC-CACTAG-005: 辅助函数正确报告策略行为
// ============================================================

#[test]
fn tc_cactag_005_helper_functions_correct() {
    // Auto 策略
    assert!(caches_system_messages(&CachePolicy::Auto));
    assert!(caches_latest_user_message(&CachePolicy::Auto));
    assert!(!caches_all_user_messages(&CachePolicy::Auto));

    // None 策略
    assert!(!caches_system_messages(&CachePolicy::None));
    assert!(!caches_latest_user_message(&CachePolicy::None));
    assert!(!caches_all_user_messages(&CachePolicy::None));
}

// ============================================================
// TC-CACTAG-006: CachedSegmentedPrompt 拼接正确
// ============================================================

#[test]
fn tc_cactag_006_combined_string_correct() {
    let result = apply_cache_policy("前缀。", "上下文。", &CachePolicy::Auto);
    let combined = result.to_combined_string();
    assert_eq!(
        combined, "前缀。\n\n上下文。",
        "拼接结果应为 '前缀。\\n\\n上下文。'"
    );
}
