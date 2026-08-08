//! 安全管理 Store — SecurityManager + ClipboardGuard。
//!
//! 借鉴 Zed `credentials_provider`/`askpass` 的 Store 模式：
//! 安全相关状态封装在独立 Store 中，减少与业务逻辑的耦合。

use std::sync::Arc;

use echomind_core::security::{ClipboardConfig, ClipboardGuard, SecurityManager};
use echomind_infra::sqlite_storage::SqliteStorage;

/// 安全管理 Store（自动锁屏 + 剪贴板清除 + 暴力破解防护）。
///
/// # 职责
/// - `SecurityManager` — 加密状态机、自动锁屏、暴力破解防护、紧急销毁
/// - `ClipboardGuard` — 敏感数据剪贴板自动清除
///
/// # 线程安全
/// 内部使用 `Arc` 共享，无额外锁需求。
pub struct SecurityStore {
    /// 共享存储引用
    #[allow(dead_code)]
    storage: Arc<SqliteStorage>,
    /// 安全管理器（自动锁屏、暴力破解防护、紧急销毁）
    pub security: Arc<SecurityManager>,
    /// 剪贴板清除管理器
    pub clipboard_guard: Arc<ClipboardGuard>,
}

impl SecurityStore {
    /// 创建新的安全管理 Store。
    pub fn new(storage: Arc<SqliteStorage>) -> Self {
        Self {
            storage,
            security: Arc::new(SecurityManager::new()),
            clipboard_guard: Arc::new(ClipboardGuard::new(ClipboardConfig::default())),
        }
    }
}
