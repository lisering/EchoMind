#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! MemoryBudget 单元测试（Phase 3 Session 22）。
//!
//! 测试覆盖：预算管理器创建、容量检查、分配/释放、LRU 驱逐、touch、统计快照。
//! `TC-MEM-007` 验证 `get_system_memory()` 返回正值。

use super::memory_budget::*;

/// TC-MEM-001：创建预算管理器。
#[test]
fn test_budget_creation() {
    let budget = MemoryBudget::new(1024 * 1024 * 1024); // 1GB
    assert_eq!(budget.max_bytes(), 1024 * 1024 * 1024);
    assert_eq!(budget.current_usage(), 0);
    assert_eq!(budget.available(), 1024 * 1024 * 1024);
    assert_eq!(budget.layer_count(), 0);
}

/// TC-MEM-002：预算内 can_fit 返回 true。
#[test]
fn test_can_fit_within_budget() {
    let budget = MemoryBudget::new(1024);
    assert!(budget.can_fit(512));
    assert!(budget.can_fit(1024));
    assert!(budget.can_fit(0));
}

/// TC-MEM-003：超预算 can_fit 返回 false。
#[test]
fn test_can_fit_exceeds_budget() {
    let budget = MemoryBudget::new(1024);
    assert!(!budget.can_fit(1025));
    assert!(!budget.can_fit(2048));
}

/// TC-MEM-004：allocate 后 current_usage 增加。
#[test]
fn test_allocate_increases_usage() {
    let mut budget = MemoryBudget::new(4096);
    budget.allocate("layer0", 1024).expect("分配失败");
    assert_eq!(budget.current_usage(), 1024);
    assert!(budget.can_fit(3072));
    assert!(!budget.can_fit(3073));

    budget.allocate("layer1", 2048).expect("分配失败");
    assert_eq!(budget.current_usage(), 3072);
    assert_eq!(budget.layer_count(), 2);
}

/// TC-MEM-005：release 后 current_usage 减少。
#[test]
fn test_release_decreases_usage() {
    let mut budget = MemoryBudget::new(4096);
    budget.allocate("layer0", 1024).expect("分配失败");
    budget.allocate("layer1", 2048).expect("分配失败");
    assert_eq!(budget.current_usage(), 3072);

    budget.release("layer0");
    assert_eq!(budget.current_usage(), 2048);
    assert_eq!(budget.layer_count(), 1);
}

/// TC-MEM-006：LRU 驱逐释放足够空间。
#[test]
fn test_evict_lru_frees_space() {
    let mut budget = MemoryBudget::new(4096);
    // 填满 4 个层
    budget.allocate("layer0", 1024).expect("分配失败");
    budget.allocate("layer1", 1024).expect("分配失败");
    budget.allocate("layer2", 1024).expect("分配失败");
    budget.allocate("layer3", 1024).expect("分配失败");
    assert_eq!(budget.current_usage(), 4096);

    // 分配新层触发 LRU 驱逐
    budget.allocate("layer4", 2048).expect("分配失败");
    assert!(budget.current_usage() <= 4096);
    // layer0 和 layer1 应被驱逐（最久未使用）
    assert!(!budget.contains_layer("layer0"));
    assert!(!budget.contains_layer("layer1"));
    assert!(budget.contains_layer("layer4"));
}

/// TC-MEM-007：系统内存 > 0。
#[test]
fn test_get_system_memory_positive() {
    let mem = get_system_memory();
    assert!(mem > 0, "系统内存应大于 0");
    // 至少 1GB（CI 环境最低配置）
    assert!(
        mem >= 1024 * 1024 * 1024,
        "系统内存应至少 1GB，实际: {mem} bytes"
    );
}

/// TC-MEM-008：空间足够时不驱逐。
#[test]
fn test_evict_lru_no_evict_needed() {
    let mut budget = MemoryBudget::new(8192);
    budget.allocate("layer0", 1024).expect("分配失败");
    budget.allocate("layer1", 1024).expect("分配失败");
    assert_eq!(budget.current_usage(), 2048);

    // 驱逐只需要 1024，当前可用 6144，不需要驱逐
    budget.evict_lru(1024).expect("驱逐失败");
    assert_eq!(budget.current_usage(), 2048, "空间足够时不应驱逐");
    assert_eq!(budget.layer_count(), 2, "不应驱逐任何层");
}

/// TC-MEM-009：touch 标记最近使用。
#[test]
fn test_touch_prevents_eviction() {
    let mut budget = MemoryBudget::new(4096);
    budget.allocate("layer0", 1024).expect("分配失败");
    budget.allocate("layer1", 1024).expect("分配失败");
    budget.allocate("layer2", 1024).expect("分配失败");
    budget.allocate("layer3", 1024).expect("分配失败");

    // touch layer0 使其成为最近使用
    budget.touch("layer0");

    // 分配新层触发驱逐，layer0 不应被驱逐
    budget.allocate("layer4", 1024).expect("分配失败");
    assert!(
        budget.contains_layer("layer0"),
        "layer0 被 touch 后不应被驱逐"
    );
}

/// TC-MEM-010：stats 快照。
#[test]
fn test_stats_snapshot() {
    let mut budget = MemoryBudget::new(2048);
    budget.allocate("layer0", 512).expect("分配失败");

    let stats = budget.stats();
    assert_eq!(stats.max_budget, 2048);
    assert_eq!(stats.current_usage, 512);
    assert_eq!(stats.available, 1536);
    assert!(stats.total_system > 0);

    // summary 不应 panic
    let _ = stats.summary();
}

/// TC-MEM-011：超出总预算返回错误。
#[test]
fn test_allocate_exceeds_total_budget() {
    let mut budget = MemoryBudget::new(1024);
    let result = budget.allocate("layer0", 2048);
    assert!(result.is_err(), "超过总预算应返回错误");
}

/// TC-MEM-012：重复分配同一层先释放旧分配。
#[test]
fn test_reallocate_same_layer() {
    let mut budget = MemoryBudget::new(4096);
    budget.allocate("layer0", 1024).expect("分配失败");
    assert_eq!(budget.current_usage(), 1024);

    // 重新分配不同大小
    budget.allocate("layer0", 2048).expect("分配失败");
    assert_eq!(budget.current_usage(), 2048, "重新分配应先释放旧分配");
    assert_eq!(budget.layer_count(), 1, "不应有重复层");
}

/// TC-MEM-013：clear 清空所有分配。
#[test]
fn test_clear() {
    let mut budget = MemoryBudget::new(4096);
    budget.allocate("layer0", 1024).expect("分配失败");
    budget.allocate("layer1", 1024).expect("分配失败");
    assert_eq!(budget.current_usage(), 2048);

    budget.clear();
    assert_eq!(budget.current_usage(), 0);
    assert_eq!(budget.layer_count(), 0);
    assert_eq!(budget.available(), 4096);
}
