//! 幂等性存储 + 统一周期任务抽象（借鉴 QM IdempotencyStore + Sweeper）
//!
//! 提供跨会话的幂等性保证，防止重复操作（文件同步、AutoDream 重复触发等）。
//! 提供统一的周期任务管理，替代各自为政的后台任务实现。

use anyhow;
use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// 幂等性存储（借鉴 QM IdempotencyStore）
///
/// 支持内存缓存 + 可选持久化后端，提供 once(key, fn) 语义：
/// - 首次调用：执行 fn，返回 true
/// - 重复调用：跳过 fn，返回 false
/// - 支持 inflight 标记防止并发重复
/// - 自动清理过期记录
pub struct IdempotencyStore {
    /// 已完成的记录：key -> 完成时间戳（秒）
    done: Arc<RwLock<HashMap<String, i64>>>,
    /// 进行中的记录
    inflight: Arc<RwLock<HashSet<String>>>,
    /// 保留时间（秒），默认 14 天
    retention_secs: u64,
    /// LRU 缓存限制，防止无限增长
    max_cache_entries: usize,
}

impl Default for IdempotencyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl IdempotencyStore {
    /// 创建无持久化的内存版本
    pub fn new() -> Self {
        Self {
            done: Arc::new(RwLock::new(HashMap::new())),
            inflight: Arc::new(RwLock::new(HashSet::new())),
            retention_secs: 14 * 24 * 3600, // 14 天
            max_cache_entries: 1000,
        }
    }

    /// 创建测试版本（自定义保留期）
    pub fn new_test(retention_secs: u64) -> Self {
        Self {
            done: Arc::new(RwLock::new(HashMap::new())),
            inflight: Arc::new(RwLock::new(HashSet::new())),
            retention_secs,
            max_cache_entries: 1000,
        }
    }

    /// 执行一次操作（如果 key 未处理过）
    ///
    /// 返回 true = 执行了 fn，false = 已处理过或正在处理
    pub async fn once<F>(&self, key: &str, f: F) -> bool
    where
        F: FnOnce() -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>,
    {
        // 1. 检查 inflight
        {
            let inflight = self.inflight.read().await;
            if inflight.contains(key) {
                return false;
            }
        }

        // 2. 检查 done（带过期检查）
        let should_execute = {
            let done = self.done.read().await;
            match done.get(key) {
                Some(&timestamp) => {
                    let now = current_timestamp();
                    now - timestamp >= self.retention_secs as i64
                }
                None => true,
            }
        };

        if !should_execute {
            return false;
        }

        // 3. 标记 inflight
        {
            let mut inflight = self.inflight.write().await;
            if inflight.contains(key) {
                return false; // 并发检查
            }
            inflight.insert(key.to_string());
        }

        // 4. 执行用户函数
        let result = f().await;

        // 5. 清理 expired 记录（在较少执行路径上触发）
        if result.is_ok() {
            self.prune_expired();
        }

        // 6. 更新 done 标记
        if result.is_ok() {
            let mut done = self.done.write().await;

            // LRU 驱逐：如果超出限制，删除最旧的记录
            if done.len() >= self.max_cache_entries
                && let Some(oldest_key) = done
                    .iter()
                    .min_by_key(|(_, ts)| **ts)
                    .map(|(k, _)| (*k).clone())
            {
                done.remove(&oldest_key);
            }

            done.insert(key.to_string(), current_timestamp());

            // 持久化（如果启用）
            // 注意：当前版本只支持内存缓存，持久化需要额外的 trait 约束支持
            // 将在 tauri-app 层使用具体类型时实现
            let _ = key; // 避免未使用变量警告
        }

        // 7. 清理 inflight
        {
            let mut inflight = self.inflight.write().await;
            inflight.remove(key);
        }

        result.is_ok()
    }

    /// 手动清理过期记录
    pub fn prune_expired(&self) {
        let done = self.done.clone();
        let retention_secs = self.retention_secs;

        tokio::spawn(async move {
            let now = current_timestamp();
            let mut done_guard = done.write().await;

            done_guard.retain(|_, &mut timestamp| now - timestamp < retention_secs as i64);
        });
    }

    /// 获取缓存统计
    pub async fn stats(&self) -> IdempotencyStats {
        let done = self.done.read().await;
        let inflight = self.inflight.read().await;

        IdempotencyStats {
            done_count: done.len(),
            inflight_count: inflight.len(),
        }
    }

    /// 从持久化存储加载记录（内存版本为空实现）
    pub async fn load_from_storage(&self) -> anyhow::Result<()> {
        // 内存版本：无需从存储加载
        Ok(())
    }
}

/// 幂等性统计
#[derive(Debug, Clone)]
pub struct IdempotencyStats {
    pub done_count: usize,
    pub inflight_count: usize,
}

/// 统一周期任务管理器（借鉴 QM createSweeper）
///
/// 提供错误吞咽、标签化、后台执行的周期任务抽象。
pub struct Sweeper {
    handle: Option<tokio::task::JoinHandle<()>>,
    label: String,
}

impl Sweeper {
    /// 创建周期任务
    ///
    /// # 参数
    /// - `task`: 异步任务闭包
    /// - `interval`: 执行间隔
    /// - `label`: 任务标签（用于日志）
    pub fn new<F>(task: F, interval: Duration, label: &str) -> Self
    where
        F: Fn() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send + 'static,
    {
        let label_string = label.to_string();
        let label_for_task = label_string.clone();
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                // 错误吞咽 + 上下文标记
                match std::panic::AssertUnwindSafe(task()).catch_unwind().await {
                    Ok(()) => {}
                    Err(e) => {
                        let err_msg = if let Some(s) = e.downcast_ref::<String>() {
                            s.clone()
                        } else if let Some(s) = e.downcast_ref::<&str>() {
                            (*s).to_string()
                        } else {
                            "unknown panic".to_string()
                        };
                        tracing::warn!("[{}] sweep failed: {}", label_for_task, err_msg);
                    }
                }
            }
        });

        Self {
            handle: Some(handle),
            label: label_string,
        }
    }

    /// 停止周期任务
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }

    /// 获取任务标签
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Drop for Sweeper {
    fn drop(&mut self) {
        self.stop();
    }
}

/// 辅助函数：获取当前时间戳（秒）
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
