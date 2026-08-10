//! Session Run Coordinator（B06 会话运行协调器）。
//!
//! 借鉴 OpenCode `packages/core/src/session/run-coordinator.ts`：
//! 每个 Session 串行执行，不同 Session 并发执行。支持 `wake`（合并唤醒）
//! 和 `interrupt`（中断）。
//!
//! ## 设计目标
//!
//! - **同会话串行**：同一 `conversation_id` 的多次执行请求自动排队，
//!   防止并发写入同一会话的消息历史导致数据损坏。
//! - **跨会话并发**：不同 `conversation_id` 的执行互不阻塞。
//! - **wake 合并**：多次 `wake` 调用合并为一次后续执行，避免重复执行。
//! - **interrupt**：标记停止状态，正在执行的闭包可检查并优雅退出。
//!
//! ## 对象安全
//!
//! `SessionCoordinator` 使用 `Arc<Mutex<HashMap>>` 管理活跃会话，
//! 可安全跨线程共享（`Send + Sync`）。

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, Notify};

/// 会话运行协调器。
///
/// 管理活跃会话的执行状态，确保同一会话串行执行，
/// 不同会话并发执行。支持 `wake`（合并唤醒）和 `interrupt`（中断）。
#[derive(Debug, Clone)]
pub struct SessionCoordinator {
    /// 活跃会话：conversation_id → 协调条目
    active: Arc<Mutex<HashMap<String, CoordEntry>>>,
}

/// 协调条目：记录单个会话的执行状态。
#[derive(Debug)]
struct CoordEntry {
    /// 完成通知器：正在排队的后续执行通过此 Notify 等待当前执行完成。
    done: Arc<Notify>,
    /// 是否有待处理的唤醒（合并多次 wake 为一次）。
    pending_wake: bool,
    /// 是否正在停止（interrupt 标记）。
    stopping: bool,
}

impl Default for SessionCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionCoordinator {
    /// 创建新的会话运行协调器。
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 启动或加入执行。
    ///
    /// 如果 `session_id` 已有活跃执行，则等待其完成后再执行（串行）。
    /// 如果没有活跃执行，则直接执行闭包。
    ///
    /// 执行完成后检查 `pending_wake`：如果有待处理唤醒则递归执行一次。
    /// `stopping` 状态下不执行，直接返回。
    ///
    /// # 参数
    /// - `session_id`: 会话标识（通常为 conversation_id）
    /// - `f`: 要执行的异步闭包
    ///
    /// # 返回
    /// 闭包的执行结果。
    pub async fn run<F, Fut, T>(&self, session_id: &str, f: F) -> anyhow::Result<T>
    where
        F: Fn() -> Fut + Sync,
        Fut: std::future::Future<Output = anyhow::Result<T>> + Send,
        T: Send,
    {
        self.run_inner(session_id, &f).await
    }

    /// `run` 的内部实现：通过引用调用闭包，支持递归重执行。
    fn run_inner<'a, F, Fut, T>(
        &'a self,
        session_id: &'a str,
        f: &'a F,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send + 'a>>
    where
        F: Fn() -> Fut + Sync,
        Fut: std::future::Future<Output = anyhow::Result<T>> + Send,
        T: Send + 'a,
    {
        Box::pin(async move {
            loop {
                let mut active = self.active.lock().await;

                if let Some(entry) = active.get(session_id) {
                    if entry.stopping {
                        // 正在停止：等待完成后重试
                        let done = entry.done.clone();
                        drop(active);
                        done.notified().await;
                        continue;
                    } else {
                        // 加入已有执行：等待当前执行完成
                        let done = entry.done.clone();
                        drop(active);
                        done.notified().await;
                        continue;
                    }
                }

                // 没有活跃执行：启动新执行
                let entry = CoordEntry {
                    done: Arc::new(Notify::new()),
                    pending_wake: false,
                    stopping: false,
                };
                active.insert(session_id.to_string(), entry);
                drop(active);

                // 执行闭包
                let result = f().await;

                // 检查是否有待处理唤醒
                let mut active = self.active.lock().await;
                if let Some(entry) = active.get(session_id)
                    && entry.pending_wake
                    && !entry.stopping
                {
                    // 有待处理唤醒：重置标记并递归执行
                    let done = entry.done.clone();
                    done.notify_waiters();
                    active.remove(session_id);
                    drop(active);
                    // 递归执行
                    return self.run_inner(session_id, f).await;
                }
                // 没有待处理唤醒：从活跃列表移除
                if let Some(entry) = active.remove(session_id) {
                    entry.done.notify_waiters();
                }
                drop(active);

                return result;
            }
        })
    }

    /// 注册后续工作（合并唤醒）。
    ///
    /// 如果会话有活跃执行，标记 `pending_wake = true`。
    /// 执行完成后会检查此标记并重新执行一次。
    /// 多次 `wake` 合并为一次（幂等）。
    pub async fn wake(&self, session_id: &str) {
        let mut active = self.active.lock().await;
        if let Some(entry) = active.get_mut(session_id) {
            entry.pending_wake = true;
        }
    }

    /// 中断执行。
    ///
    /// 标记会话的 `stopping = true`。
    /// 正在执行的闭包可通过 `is_stopping` 检查此状态并优雅退出。
    pub async fn interrupt(&self, session_id: &str) {
        let mut active = self.active.lock().await;
        if let Some(entry) = active.get_mut(session_id) {
            entry.stopping = true;
        }
    }

    /// 检查会话是否正在停止。
    ///
    /// 正在执行的闭包可定期调用此方法检查中断状态。
    pub async fn is_stopping(&self, session_id: &str) -> bool {
        let active = self.active.lock().await;
        active.get(session_id).is_some_and(|e| e.stopping)
    }

    /// 获取活跃会话列表。
    pub async fn active_sessions(&self) -> Vec<String> {
        self.active.lock().await.keys().cloned().collect()
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// TC-COORD-001：同一会话的两次 run 串行执行。
    #[tokio::test]
    async fn tc_coord_001_serial_execution() {
        let coord = SessionCoordinator::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let current = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        let m1 = max_concurrent.clone();
        let cur1 = current.clone();
        let coord1 = coord.clone();
        let h1 = tokio::spawn(async move {
            coord1
                .run("session-1", || {
                    let c = c1.clone();
                    let m = m1.clone();
                    let cur = cur1.clone();
                    async move {
                        let _g = ActiveGuard::new(cur.clone());
                        c.fetch_add(1, Ordering::SeqCst);
                        let now = cur.load(Ordering::SeqCst);
                        m.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        let c2 = counter.clone();
        let m2 = max_concurrent.clone();
        let cur2 = current.clone();
        let coord2 = coord.clone();
        let h2 = tokio::spawn(async move {
            coord2
                .run("session-1", || {
                    let c = c2.clone();
                    let m = m2.clone();
                    let cur = cur2.clone();
                    async move {
                        let _g = ActiveGuard::new(cur.clone());
                        c.fetch_add(1, Ordering::SeqCst);
                        let now = cur.load(Ordering::SeqCst);
                        m.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        let _ = h1.await;
        let _ = h2.await;

        // 两次都执行了
        assert_eq!(counter.load(Ordering::SeqCst), 2, "两次 run 都应执行");
        // 最大并发数为 1（串行）
        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            1,
            "同一会话应串行执行，最大并发=1"
        );
    }

    /// TC-COORD-002：不同会话的 run 并发执行。
    #[tokio::test]
    async fn tc_coord_002_concurrent_different_sessions() {
        let coord = SessionCoordinator::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let current = Arc::new(AtomicUsize::new(0));

        let c1 = counter.clone();
        let m1 = max_concurrent.clone();
        let cur1 = current.clone();
        let coord1 = coord.clone();
        let h1 = tokio::spawn(async move {
            coord1
                .run("session-A", || {
                    let c = c1.clone();
                    let m = m1.clone();
                    let cur = cur1.clone();
                    async move {
                        let _g = ActiveGuard::new(cur.clone());
                        c.fetch_add(1, Ordering::SeqCst);
                        let now = cur.load(Ordering::SeqCst);
                        m.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        let c2 = counter.clone();
        let m2 = max_concurrent.clone();
        let cur2 = current.clone();
        let coord2 = coord.clone();
        let h2 = tokio::spawn(async move {
            coord2
                .run("session-B", || {
                    let c = c2.clone();
                    let m = m2.clone();
                    let cur = cur2.clone();
                    async move {
                        let _g = ActiveGuard::new(cur.clone());
                        c.fetch_add(1, Ordering::SeqCst);
                        let now = cur.load(Ordering::SeqCst);
                        m.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        let _ = h1.await;
        let _ = h2.await;

        // 两次都执行了
        assert_eq!(counter.load(Ordering::SeqCst), 2);
        // 最大并发数 >= 2（不同会话可并发）
        assert!(
            max_concurrent.load(Ordering::SeqCst) >= 2,
            "不同会话应并发执行，最大并发 >= 2"
        );
    }

    /// TC-COORD-003：wake 合并多次唤醒为一次。
    #[tokio::test]
    async fn tc_coord_003_wake_merge() {
        let coord = SessionCoordinator::new();
        let exec_count = Arc::new(AtomicUsize::new(0));

        let ec1 = exec_count.clone();
        let coord1 = coord.clone();
        let h = tokio::spawn(async move {
            coord1
                .run("wake-session", || {
                    let ec = ec1.clone();
                    async move {
                        ec.fetch_add(1, Ordering::SeqCst);
                        // 模拟长时间执行
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        // 等待执行开始
        tokio::time::sleep(Duration::from_millis(20)).await;

        // 多次 wake（应合并）
        coord.wake("wake-session").await;
        coord.wake("wake-session").await;
        coord.wake("wake-session").await;

        // 等待执行完成
        let _ = h.await;

        // 初始执行 + 1 次 wake 重执行 = 2 次
        assert_eq!(
            exec_count.load(Ordering::SeqCst),
            2,
            "多次 wake 应合并为一次重执行"
        );
    }

    /// TC-COORD-004：interrupt 标记停止状态。
    #[tokio::test]
    async fn tc_coord_004_interrupt() {
        let coord = SessionCoordinator::new();

        let coord1 = coord.clone();
        let h = tokio::spawn(async move {
            coord1
                .run("interrupt-session", || async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<_, anyhow::Error>(())
                })
                .await
        });

        // 等待执行开始
        tokio::time::sleep(Duration::from_millis(20)).await;

        // interrupt
        coord.interrupt("interrupt-session").await;

        // 检查 stopping 状态
        assert!(
            coord.is_stopping("interrupt-session").await,
            "interrupt 后应标记为 stopping"
        );

        let _ = h.await;

        // 执行完成后 stopping 状态应清除（条目已移除）
        assert!(!coord.is_stopping("interrupt-session").await);
    }

    /// TC-COORD-005：执行完成后从活跃列表移除。
    #[tokio::test]
    async fn tc_coord_005_remove_after_completion() {
        let coord = SessionCoordinator::new();

        // 执行前：无活跃会话
        assert!(coord.active_sessions().await.is_empty());

        coord
            .run("remove-session", || async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                Ok::<_, anyhow::Error>(())
            })
            .await
            .unwrap();

        // 执行后：无活跃会话（已移除）
        assert!(
            coord.active_sessions().await.is_empty(),
            "执行完成后应从活跃列表移除"
        );
    }

    /// 辅助：RAII guard 用于跟踪并发执行数。
    struct ActiveGuard {
        current: Arc<AtomicUsize>,
    }

    impl ActiveGuard {
        fn new(current: Arc<AtomicUsize>) -> Self {
            current.fetch_add(1, Ordering::SeqCst);
            Self { current }
        }
    }

    impl Drop for ActiveGuard {
        fn drop(&mut self) {
            self.current.fetch_sub(1, Ordering::SeqCst);
        }
    }

    // ========================================================================
    // S5: SessionCoordinator 生产集成测试（TC-COORD-INTEGRATE-001~003）
    //
    // 验证 SessionCoordinator 在 chat_inner 集成场景下的行为：
    // 同会话串行化 / interrupt 中断清理 / wake 合并唤醒。
    // ========================================================================

    /// TC-COORD-INTEGRATE-001：会话级串行执行（同会话不并发）。
    ///
    /// 模拟两个 chat_inner 调用同时到达同一 conversation_id：
    /// 第二个请求等待第一个完成后才执行，最大并发数为 1。
    #[tokio::test]
    async fn tc_coord_integrate_001_serial_same_session() {
        let coord = SessionCoordinator::new();
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        let current = Arc::new(AtomicUsize::new(0));

        // 模拟两个并发 chat 请求（同一 conversation_id）
        let mc1 = max_concurrent.clone();
        let cur1 = current.clone();
        let coord1 = coord.clone();
        let h1 = tokio::spawn(async move {
            coord1
                .run("conv-001", move || {
                    let mc = mc1.clone();
                    let cur = cur1.clone();
                    async move {
                        let _g = ActiveGuard::new(cur.clone());
                        let now = cur.load(Ordering::SeqCst);
                        mc.fetch_max(now, Ordering::SeqCst);
                        // 模拟 RAG 检索 + LLM 生成耗时
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        let mc2 = max_concurrent.clone();
        let cur2 = current.clone();
        let coord2 = coord.clone();
        let h2 = tokio::spawn(async move {
            coord2
                .run("conv-001", move || {
                    let mc = mc2.clone();
                    let cur = cur2.clone();
                    async move {
                        let _g = ActiveGuard::new(cur.clone());
                        let now = cur.load(Ordering::SeqCst);
                        mc.fetch_max(now, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        let _ = h1.await;
        let _ = h2.await;

        // 最大并发 = 1（串行执行）
        assert_eq!(
            max_concurrent.load(Ordering::SeqCst),
            1,
            "同一 conversation_id 的 chat 请求应串行执行"
        );
    }

    /// TC-COORD-INTEGRATE-002：interrupt 中断后清理。
    ///
    /// 模拟 abort_chat 调用 interrupt，之后会话从活跃列表移除。
    #[tokio::test]
    async fn tc_coord_integrate_002_interrupt_cleanup() {
        let coord = SessionCoordinator::new();

        let coord1 = coord.clone();
        let h = tokio::spawn(async move {
            coord1
                .run("conv-abort", || async {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<_, anyhow::Error>(())
                })
                .await
        });

        // 等待执行开始
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !coord.active_sessions().await.is_empty(),
            "执行中应有活跃会话"
        );

        // 模拟 abort_chat → interrupt
        coord.interrupt("conv-abort").await;
        assert!(
            coord.is_stopping("conv-abort").await,
            "interrupt 后应标记 stopping"
        );

        let _ = h.await;

        // 执行完成后会话从活跃列表移除
        assert!(
            coord.active_sessions().await.is_empty(),
            "interrupt + 完成后会话应从活跃列表移除"
        );
    }

    /// TC-COORD-INTEGRATE-003：wake 合并唤醒。
    ///
    /// 模拟多次新消息到达同一会话（如用户快速连续发送），
    /// wake 合并为一次重执行，而非多次。
    #[tokio::test]
    async fn tc_coord_integrate_003_wake_merge() {
        let coord = SessionCoordinator::new();
        let exec_count = Arc::new(AtomicUsize::new(0));

        let ec = exec_count.clone();
        let coord1 = coord.clone();
        let h = tokio::spawn(async move {
            coord1
                .run("conv-wake", move || {
                    let ec = ec.clone();
                    async move {
                        ec.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(80)).await;
                        Ok::<_, anyhow::Error>(())
                    }
                })
                .await
        });

        // 等待执行开始
        tokio::time::sleep(Duration::from_millis(20)).await;

        // 模拟 3 次快速 wake（应合并为 1 次重执行）
        coord.wake("conv-wake").await;
        coord.wake("conv-wake").await;
        coord.wake("conv-wake").await;

        let _ = h.await;

        // 初始执行 + 1 次 wake 合并重执行 = 2 次
        assert_eq!(
            exec_count.load(Ordering::SeqCst),
            2,
            "多次 wake 应合并为 1 次重执行（总执行 2 次）"
        );
    }
}
