//! RAM 预算管理（Phase 3 Session 22）。
//!
//! 在模型 > 可用 RAM 时自动管理内存：
//! - 分配/释放内存预算
//! - LRU 驱逐已使用的层
//! - 预取即将使用的层
//!
//! # 设计原理
//!
//! 大模型（如 Qwen2.5-7B Q4_K_M ~4GB）可能超过系统可用 RAM。
//! 内存预算管理器跟踪当前使用量，在需要新层时检查是否有足够空间：
//! - 有空间：直接分配
//! - 无空间：LRU 驱逐最久未使用的层，释放空间后分配
//!
//! 不依赖外部 crate（sysinfo），使用 `libc::sysconf` 获取系统内存（Unix）。

#![cfg(feature = "pro")]

use std::collections::VecDeque;

/// 内存统计快照。
#[derive(Debug, Clone)]
pub struct MemoryStats {
    /// 系统总 RAM（字节）
    pub total_system: usize,
    /// 预算上限（字节）
    pub max_budget: usize,
    /// 当前使用量（字节）
    pub current_usage: usize,
    /// 可用预算（字节）
    pub available: usize,
}

impl MemoryStats {
    /// 生成描述字符串。
    pub fn summary(&self) -> String {
        format!(
            "系统RAM: {:.1}GB | 预算: {:.1}GB | 已用: {:.1}GB | 可用: {:.1}GB",
            self.total_system as f64 / 1_073_741_824.0,
            self.max_budget as f64 / 1_073_741_824.0,
            self.current_usage as f64 / 1_073_741_824.0,
            self.available as f64 / 1_073_741_824.0,
        )
    }
}

/// LRU 缓存项（层 ID → 内存大小）。
#[derive(Debug, Clone)]
struct LruEntry {
    /// 层标识
    layer_id: String,
    /// 该层占用的字节数
    bytes: usize,
}

/// RAM 预算管理器。
///
/// 跟踪内存使用量，在需要新层时检查预算并 LRU 驱逐旧层。
pub struct MemoryBudget {
    /// 预算上限（字节）
    max_bytes: usize,
    /// 当前使用量（字节）
    current_usage: usize,
    /// LRU 队列（最近使用的在队尾，最久未使用的在队首）
    lru_queue: VecDeque<LruEntry>,
    /// 层 ID → 在队列中的索引（快速查找）
    layer_map: std::collections::HashMap<String, usize>,
}

impl MemoryBudget {
    /// 创建预算管理器。
    ///
    /// # 参数
    /// - `max_bytes` — 内存预算上限（字节）
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            current_usage: 0,
            lru_queue: VecDeque::new(),
            layer_map: std::collections::HashMap::new(),
        }
    }

    /// 创建基于系统 RAM 的预算管理器（自动设置为系统 RAM 的 80%）。
    pub fn from_system(percentage: f64) -> Self {
        let total = get_system_memory();
        let budget = (total as f64 * percentage) as usize;
        Self::new(budget)
    }

    /// 获取当前使用量（字节）。
    pub fn current_usage(&self) -> usize {
        self.current_usage
    }

    /// 获取预算上限（字节）。
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// 获取可用预算（字节）。
    pub fn available(&self) -> usize {
        self.max_bytes.saturating_sub(self.current_usage)
    }

    /// 检查是否能容纳指定字节。
    pub fn can_fit(&self, bytes: usize) -> bool {
        self.current_usage + bytes <= self.max_bytes
    }

    /// 分配内存预算。
    ///
    /// 如果空间不足，先调用 `evict_lru` 驱逐旧层。
    ///
    /// # 错误
    /// - 空间不足且驱逐后仍不足
    /// - 请求超过总预算
    pub fn allocate(&mut self, layer_id: &str, bytes: usize) -> anyhow::Result<()> {
        if bytes > self.max_bytes {
            anyhow::bail!(
                "层 {layer_id} 大小 ({bytes} bytes) 超过总预算 ({})",
                self.max_bytes
            );
        }

        // 如果该层已存在，先释放旧分配
        if self.layer_map.contains_key(layer_id) {
            self.release(layer_id);
        }

        // 空间不足时 LRU 驱逐
        if !self.can_fit(bytes) {
            self.evict_lru(bytes)?;
        }

        // 分配
        self.current_usage += bytes;
        self.lru_queue.push_back(LruEntry {
            layer_id: layer_id.to_string(),
            bytes,
        });
        self.layer_map
            .insert(layer_id.to_string(), self.lru_queue.len() - 1);

        Ok(())
    }

    /// 释放指定层的内存预算。
    pub fn release(&mut self, layer_id: &str) {
        if let Some(&idx) = self.layer_map.get(layer_id) {
            if let Some(entry) = self.lru_queue.get(idx) {
                self.current_usage = self.current_usage.saturating_sub(entry.bytes);
            }
            self.lru_queue.remove(idx);
            // 重建索引映射（remove 后索引变化）
            self.rebuild_index();
        }
    }

    /// 标记指定层为最近使用（移到 LRU 队尾）。
    pub fn touch(&mut self, layer_id: &str) {
        if let Some(&idx) = self.layer_map.get(layer_id)
            && let Some(entry) = self.lru_queue.get(idx).cloned()
        {
            self.lru_queue.remove(idx);
            self.lru_queue.push_back(entry);
            self.rebuild_index();
        }
    }

    /// LRU 驱逐直到释放足够空间。
    ///
    /// 如果当前可用空间已满足 `need_bytes`，直接返回（不驱逐任何层）。
    /// 否则从队首（最久未使用）开始驱逐，直到可用空间 ≥ `need_bytes`。
    pub fn evict_lru(&mut self, need_bytes: usize) -> anyhow::Result<()> {
        // 空间已足够，无需驱逐
        if self.available() >= need_bytes {
            return Ok(());
        }

        // 需要释放的字节数 = 需求 - 当前可用
        let need_to_free = need_bytes - self.available();
        let mut freed = 0usize;

        while freed < need_to_free && !self.lru_queue.is_empty() {
            let entry = self.lru_queue.pop_front();
            if let Some(entry) = entry {
                freed += entry.bytes;
                self.current_usage = self.current_usage.saturating_sub(entry.bytes);
                self.layer_map.remove(&entry.layer_id);
            }
        }

        if freed < need_to_free {
            anyhow::bail!("LRU 驱逐后仍不足: 需要释放 {need_to_free} bytes, 仅释放 {freed} bytes");
        }

        self.rebuild_index();
        Ok(())
    }

    /// 获取内存统计快照。
    pub fn stats(&self) -> MemoryStats {
        MemoryStats {
            total_system: get_system_memory(),
            max_budget: self.max_bytes,
            current_usage: self.current_usage,
            available: self.available(),
        }
    }

    /// 清空所有分配。
    pub fn clear(&mut self) {
        self.current_usage = 0;
        self.lru_queue.clear();
        self.layer_map.clear();
    }

    /// 获取当前管理的层数。
    pub fn layer_count(&self) -> usize {
        self.lru_queue.len()
    }

    /// 检查指定层是否已被分配。
    pub fn contains_layer(&self, layer_id: &str) -> bool {
        self.layer_map.contains_key(layer_id)
    }

    /// 重建索引映射（remove 操作后索引失效）。
    fn rebuild_index(&mut self) {
        self.layer_map.clear();
        for (idx, entry) in self.lru_queue.iter().enumerate() {
            self.layer_map.insert(entry.layer_id.clone(), idx);
        }
    }
}

/// 获取系统总 RAM（字节）。
///
/// Unix: 使用 `libc::sysconf(_SC_PHYS_PAGES) * _SC_PAGE_SIZE`。
/// 非 Unix: 返回默认值 8GB（保守估计）。
pub fn get_system_memory() -> usize {
    #[cfg(unix)]
    {
        // SAFETY: sysconf 是线程安全的 POSIX 函数
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        let phys_pages = unsafe { libc::sysconf(libc::_SC_PHYS_PAGES) };

        if page_size > 0 && phys_pages > 0 {
            return (page_size as usize) * (phys_pages as usize);
        }
    }

    // 回退：默认 8GB
    8 * 1024 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::needless_range_loop)]
    use super::*;

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
        assert!(!budget.layer_map.contains_key("layer0"));
        assert!(!budget.layer_map.contains_key("layer1"));
        assert!(budget.layer_map.contains_key("layer4"));
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
            budget.layer_map.contains_key("layer0"),
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
}
