//! 文档管理 Store — 导入取消 + 文件监听（REQ-ING-006, REQ-SYNC-003）。
//!
//! 借鉴 Zed `buffer_store`/`worktree_store` 的 Store 模式：
//! 领域状态封装在独立 Store 中，持有共享 Storage 引用。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use echomind_infra::file_watcher::FileWatcherHandle;
use echomind_infra::sqlite_storage::SqliteStorage;

/// 文档管理 Store（导入取消 + 文件监听器生命周期管理）。
///
/// # 职责
/// - 导入取消标志（全局单例，桌面应用一次只导入一批文件）
/// - 文件监听器句柄注册/注销/查询
///
/// # 线程安全
/// 内部使用 `tokio::sync::Mutex` 保护 HashMap，`Arc<AtomicBool>` 实现取消标志。
pub struct DocumentStore {
    /// 共享存储引用（所有 Store 共用同一 SqliteStorage 实例）
    storage: Arc<SqliteStorage>,
    /// 导入取消标志（全局单例，桌面应用一次只导入一批文件）
    import_cancel: Arc<AtomicBool>,
    /// 文件监听器句柄（路径 → FileWatcherHandle，Drop 自动停止监听）
    file_watchers: tokio::sync::Mutex<HashMap<String, FileWatcherHandle>>,
}

impl DocumentStore {
    /// 创建新的文档管理 Store。
    pub fn new(storage: Arc<SqliteStorage>) -> Self {
        Self {
            storage,
            import_cancel: Arc::new(AtomicBool::new(false)),
            file_watchers: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    /// 获取存储引用。
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }

    /// 获取导入取消标志的引用（供 import_files_inner 在循环中检查）。
    pub fn import_cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.import_cancel)
    }

    /// 触发导入取消（abort_import 命令行为）。
    /// 设置标志后，import_files_inner 循环在下一个文件边界退出。
    pub fn abort_import(&self) {
        self.import_cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// 重置导入取消标志（每次导入开始前调用）。
    pub fn reset_import_cancel(&self) {
        self.import_cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// 注册文件监听器句柄（REQ-SYNC-003）。
    ///
    /// 将 `FileWatcherHandle` 存入 `file_watchers` HashMap。
    /// 若同路径已存在句柄，旧句柄被替换（Drop 时停止监听）。
    pub async fn register_file_watcher(&self, path: &str, handle: FileWatcherHandle) {
        self.file_watchers
            .lock()
            .await
            .insert(path.to_string(), handle);
    }

    /// 注销文件监听器（REQ-SYNC-003）。
    ///
    /// 从 HashMap 中移除句柄，Drop 自动停止监听。
    /// 若路径不存在则静默忽略（幂等）。
    pub async fn unregister_file_watcher(&self, path: &str) {
        self.file_watchers.lock().await.remove(path);
    }

    /// 检查指定路径的监听器是否活跃（REQ-SYNC-001 AC-3）。
    pub async fn is_watcher_active(&self, path: &str) -> bool {
        self.file_watchers.lock().await.contains_key(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_import_cancel_lifecycle() {
        // 注意：真实测试需要 SqliteStorage 实例，此处仅测试标志逻辑
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(std::sync::atomic::Ordering::SeqCst));

        flag.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(flag.load(std::sync::atomic::Ordering::SeqCst));

        flag.store(false, std::sync::atomic::Ordering::SeqCst);
        assert!(!flag.load(std::sync::atomic::Ordering::SeqCst));
    }
}
