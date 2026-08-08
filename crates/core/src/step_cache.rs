//! StepCache 步骤级缓存（P2-1，arXiv 2026-03 "StepCache: Step-Level Reuse with Lightweight
//! Verification and Selective Patching for LLM Serving"）。
//!
//! ## 思路
//!
//! Agent / Coordinator 多步推理中，相同子任务（如「检索 X 文档」）的中间步骤结果可复用，
//! 避免重复的 LLM 调用与检索计算。本模块提供：
//!
//! - `StepCache` 端口 Trait：`get` / `put` / `stats` / `clear`
//! - `InMemoryStepCache` 默认实现：线程安全、容量上限（FIFO 淘汰）、命中统计
//! - `step_cache_key` 键派生：归一化 `(tool, input)` 的 SHA-256 哈希，轻量验证 =
//!   「相同工具 + 相同查询文本（忽略大小写/空白差异）」→ 相同键 → 命中
//!
//! ## 失效策略
//!
//! - 文档导入/删除 → `clear()` 全量清空（与 L0/L1/L3 语义缓存一致，防止引用过期文档）
//! - 容量超限 → FIFO 淘汰最旧条目
//! - `cache.enabled = false`（含隐私模式）→ 命令层不注入缓存，引擎回退为直接执行

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use sha2::{Digest, Sha256};

use crate::cache::normalize_query;

/// 默认容量上限（条目数）。
const DEFAULT_CAPACITY: usize = 256;

/// 步骤缓存统计。
#[derive(Debug, Clone, Default)]
pub struct StepCacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 当前条目数
    pub entries: usize,
}

/// 步骤级缓存端口（P2-1）。
///
/// 键为 `step_cache_key` 派生的 SHA-256 十六进制字符串，值为引擎自定义的
/// 序列化载荷（JSON 文本）。实现必须线程安全（多个并行 Action 共享同一实例）。
pub trait StepCache: Send + Sync {
    /// 查询缓存条目，未命中返回 `None`。
    fn get(&self, key: &str) -> Option<String>;
    /// 写入缓存条目（覆盖同名键，等价于刷新）。
    fn put(&self, key: String, value: String);
    /// 获取命中/未命中统计。
    fn stats(&self) -> StepCacheStats;
    /// 清空全部条目（文档导入/删除时调用）。
    fn clear(&self);
}

/// 计算步骤缓存键：归一化 `tool|input` 的 SHA-256 十六进制。
///
/// 轻量验证 = 查询文本一致（忽略大小写与空白差异）：相同工具 + 相同查询 →
/// 相同键 → 命中；不同查询 → 不同键 → 未命中（重新执行并写缓存）。
///
/// # 示例
///
/// ```
/// # use echomind_core::step_cache::step_cache_key;
/// // 相同工具 + 相同输入（大小写/空白差异）→ 相同键
/// assert_eq!(step_cache_key("search_kb", "Rust  语言"), step_cache_key("search_kb", "rust 语言"));
/// // 不同输入 → 不同键
/// assert_ne!(step_cache_key("search_kb", "Rust"), step_cache_key("search_kb", "Python"));
/// // 不同工具 → 不同键
/// assert_ne!(step_cache_key("search_kb", "Rust"), step_cache_key("decompose", "Rust"));
/// ```
pub fn step_cache_key(tool: &str, input: &str) -> String {
    let normalized = format!("{}|{}", normalize_query(tool), normalize_query(input));
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 内存步骤缓存：`HashMap` + FIFO 淘汰，容量上限默认 256 条。
///
/// 使用 `Mutex` 保证线程安全（并行 Action / 多引擎共享同一实例）。
pub struct InMemoryStepCache {
    inner: Mutex<Inner>,
}

struct Inner {
    entries: HashMap<String, String>,
    /// FIFO 淘汰队列（记录插入顺序）
    order: VecDeque<String>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl InMemoryStepCache {
    /// 创建指定容量的缓存。
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                entries: HashMap::new(),
                order: VecDeque::new(),
                capacity,
                hits: 0,
                misses: 0,
            }),
        }
    }
}

impl Default for InMemoryStepCache {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

impl StepCache for InMemoryStepCache {
    fn get(&self, key: &str) -> Option<String> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.entries.contains_key(key) {
            inner.hits += 1;
            inner.entries.get(key).cloned()
        } else {
            inner.misses += 1;
            None
        }
    }

    fn put(&self, key: String, value: String) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !inner.entries.contains_key(&key) {
            // 新条目：容量超限 → FIFO 淘汰最旧条目，再入队
            while inner.entries.len() >= inner.capacity {
                if let Some(oldest) = inner.order.pop_front() {
                    inner.entries.remove(&oldest);
                } else {
                    break;
                }
            }
            inner.order.push_back(key.clone());
        }
        // 更新已有条目（刷新值，不重复入队）或插入新条目
        inner.entries.insert(key, value);
    }

    fn stats(&self) -> StepCacheStats {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        StepCacheStats {
            hits: inner.hits,
            misses: inner.misses,
            entries: inner.entries.len(),
        }
    }

    fn clear(&self) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.entries.clear();
        inner.order.clear();
    }
}
