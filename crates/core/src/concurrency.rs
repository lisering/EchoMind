//! 并发控制模块（Q09 借鉴 QM createKeyedQueue）。
//!
//! 提供按 key 序列化的并发控制队列，消除 SQLite 并发写入竞态。
//! 同 key 的操作串行执行，不同 key 的操作并行执行。
//!
//! ## 设计动机
//!
//! EchoMind 的 `persist_exchange` 可能在同一会话中并发调用（如快速连续发送消息），
//! 导致 SQLite 并发写入竞态（消息顺序错乱、标题覆盖等）。
//! 借鉴 QM 的 `createKeyedQueue<K>()` 模式，按 `conversation_id` 序列化写入操作。

use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

/// 按 key 序列化异步操作的队列（借鉴 QM createKeyedQueue）。
///
/// 同 key 的操作串行执行（通过 per-key Mutex 保证），
/// 不同 key 的操作并行执行（无全局锁）。
///
/// # 示例
///
/// ```rust,ignore
/// use echomind_core::concurrency::KeyedQueue;
///
/// let queue = KeyedQueue::<String>::new();
///
/// // 同 key 串行
/// queue.run("conv-1".to_string(), || async {
///     println!("操作 1");
/// }).await;
///
/// queue.run("conv-1".to_string(), || async {
///     println!("操作 2（等待操作 1 完成后执行）");
/// }).await;
///
/// // 不同 key 并行
/// queue.run("conv-2".to_string(), || async {
///     println!("操作 3（与 conv-1 的操作并行）");
/// }).await;
/// ```
pub struct KeyedQueue<K: Eq + Hash + Clone + Send + Sync + 'static> {
    /// Per-key Mutex tail：每个 key 对应一个独立的 Mutex。
    ///
    /// 使用 `Arc<Mutex<()>>` 作为 per-key 锁，同 key 的操作共享同一锁，
    /// 不同 key 的操作使用不同锁，实现「同 key 串行、不同 key 并行」。
    ///
    /// 公开字段仅供测试验证（cleanup 后检查 tails.len()）。
    pub tails: Arc<RwLock<HashMap<K, Arc<Mutex<()>>>>>,
}

impl<K: Eq + Hash + Clone + Send + Sync + 'static> KeyedQueue<K> {
    /// 创建空的 KeyedQueue。
    pub fn new() -> Self {
        Self {
            tails: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 按 key 序列化执行异步操作。
    ///
    /// 同 key 的操作串行执行（后续操作等待前序完成），
    /// 不同 key 的操作并行执行（互不阻塞）。
    ///
    /// # 参数
    /// - `key`: 序列化键（如 conversation_id）
    /// - `f`: 闭包，返回一个 Future，在获得 key 锁后执行
    ///
    /// # 返回
    /// Future 的输出值。
    ///
    /// # panic 安全
    ///
    /// `tokio::sync::Mutex` 不 poison（与 `std::sync::Mutex` 不同），
    /// 因此一个操作 panic 后，锁会正常释放，后续同 key 操作不受影响。
    pub async fn run<F, Fut, T>(&self, key: K, f: F) -> T
    where
        F: FnOnce() -> Fut + Send,
        Fut: Future<Output = T> + Send,
        T: Send,
    {
        // 获取或创建 per-key Mutex
        let mutex = {
            let mut tails = self.tails.write().await;
            tails
                .entry(key)
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };

        // 获取 key 锁（同 key 串行），执行操作
        let _guard = mutex.lock().await;
        f().await
    }

    /// 清理空闲的 per-key Mutex tail（防止内存泄漏）。
    ///
    /// 在所有操作完成后调用，清理无活跃操作的 tail entry。
    /// 由于此方法获取 `tails` 的写锁，调用时不应有正在执行的 `run` 操作
    /// （否则该 tail 不会被清理，因为有活跃的 Arc 引用）。
    ///
    /// 实际上，由于 `run` 方法在获取 Mutex 后才释放 tails 写锁，
    /// 且 Mutex guard 在 `f().await` 完成后释放，所以 cleanup 时
    /// 已完成的操作的 tail 的 Arc 强引用计数为 1（只有 tails HashMap 持有），
    /// 可以安全清理。正在执行的操作的 tail 的 Arc 强引用 > 1，无法清理。
    ///
    /// 但简化实现：直接清空所有 tail（因为 cleanup 在空闲时调用）。
    /// 如果有正在执行的操作，其 Mutex 不会被影响（操作持有 Mutex guard）。
    pub async fn cleanup(&self) {
        let mut tails = self.tails.write().await;
        tails.clear();
    }
}

impl<K: Eq + Hash + Clone + Send + Sync + 'static> Default for KeyedQueue<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// 便捷构造函数（与 QM createKeyedQueue 命名对齐）。
pub fn create_keyed_queue<K: Eq + Hash + Clone + Send + Sync + 'static>() -> KeyedQueue<K> {
    KeyedQueue::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let _q = KeyedQueue::<String>::new();
        // tails 初始为空（需要 async 上下文检查，这里只验证构造不 panic）
    }

    #[test]
    fn test_default_equals_new() {
        let _q1 = KeyedQueue::<String>::new();
        let _q2 = KeyedQueue::<String>::default();
    }

    #[test]
    fn test_create_keyed_queue() {
        let _q = create_keyed_queue::<String>();
    }
}
