#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! StepCache 步骤级缓存单元测试（TC-STEPCACHE-001~006，P2-1）。
//!
//! 验证：
//! - 键派生：归一化（大小写/空白差异）→ 相同键；不同输入/工具 → 不同键
//! - put/get 往返与命中/未命中统计
//! - 容量上限 FIFO 淘汰
//! - 同名键更新不改变 FIFO 顺序
//! - clear 全量清空

use crate::step_cache::{InMemoryStepCache, StepCache, step_cache_key};

/// TC-STEPCACHE-001：键派生 — 相同工具+相同输入（大小写/空白差异）→ 相同键。
#[test]
fn key_derivation_normalization() {
    assert_eq!(
        step_cache_key("search_kb", "Rust  语言"),
        step_cache_key("search_kb", "rust 语言")
    );
    assert_eq!(
        step_cache_key("search_kb", "Hello\nWorld"),
        step_cache_key("search_kb", "HELLO WORLD")
    );
}

/// TC-STEPCACHE-002：键派生 — 不同输入 / 不同工具 → 不同键。
#[test]
fn key_derivation_distinct() {
    assert_ne!(
        step_cache_key("search_kb", "Rust"),
        step_cache_key("search_kb", "Python")
    );
    assert_ne!(
        step_cache_key("search_kb", "Rust"),
        step_cache_key("decompose", "Rust")
    );
    assert_ne!(
        step_cache_key("search_kb", "Rust"),
        step_cache_key("synthesis", "Rust")
    );
}

/// TC-STEPCACHE-003：put/get 往返 + 命中统计。
#[test]
fn put_get_roundtrip() {
    let cache = InMemoryStepCache::default();
    assert!(cache.get("missing").is_none());
    let stats = cache.stats();
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits, 0);

    cache.put(
        step_cache_key("search_kb", "Rust"),
        "obs|sources".to_string(),
    );
    let hit = cache.get(&step_cache_key("search_kb", "rust"));
    assert_eq!(hit.as_deref(), Some("obs|sources"));
    let stats = cache.stats();
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.entries, 1);
}

/// TC-STEPCACHE-004：容量上限 — 超出后 FIFO 淘汰最旧条目。
#[test]
fn capacity_eviction_fifo() {
    let cache = InMemoryStepCache::new(2);
    cache.put("k1".to_string(), "v1".to_string());
    cache.put("k2".to_string(), "v2".to_string());
    cache.put("k3".to_string(), "v3".to_string());

    assert!(cache.get("k1").is_none(), "最旧条目 k1 应被淘汰");
    assert_eq!(cache.get("k2").as_deref(), Some("v2"));
    assert_eq!(cache.get("k3").as_deref(), Some("v3"));
    assert_eq!(cache.stats().entries, 2);
}

/// TC-STEPCACHE-005：同名键更新不改变 FIFO 顺序、不重复计数。
#[test]
fn put_overwrite_keeps_order() {
    let cache = InMemoryStepCache::new(2);
    cache.put("k1".to_string(), "v1".to_string());
    cache.put("k2".to_string(), "v2".to_string());
    // 更新 k1（不应重新入队，否则 k1 变成最“新”、k2 变成最旧被淘汰）
    cache.put("k1".to_string(), "v1-updated".to_string());
    cache.put("k3".to_string(), "v3".to_string());

    // k1 仍是最旧（更新不改变位置）→ 应被淘汰；k2/k3 保留
    assert!(cache.get("k1").is_none(), "k1 仍是最旧，应被 FIFO 淘汰");
    assert_eq!(cache.get("k2").as_deref(), Some("v2"));
    assert_eq!(cache.get("k3").as_deref(), Some("v3"));
    assert_eq!(cache.stats().entries, 2);
}

/// TC-STEPCACHE-006：clear 清空全部条目。
#[test]
fn clear_resets() {
    let cache = InMemoryStepCache::default();
    cache.put("k1".to_string(), "v1".to_string());
    cache.clear();
    assert!(cache.get("k1").is_none());
    assert_eq!(cache.stats().entries, 0);
}
