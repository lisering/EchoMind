//! 应用状态组装（Phase 3：注入 SqliteStorage，REQ-DB-001）。
//! InMemoryStorage 已按 TPM 冻结契约移除，持久化由 crates/infra 的 SQLite 适配器承担。

use std::path::PathBuf;

use anyhow::Context;
use echomind_core::Storage;
use echomind_infra::sqlite_storage::SqliteStorage;

/// Tauri 全局状态。
pub struct AppState {
    /// 存储端口实现（SQLite 持久化，WAL 模式）
    pub storage: SqliteStorage,
    /// 应用数据目录（文档副本与数据库根目录）
    pub data_dir: PathBuf,
}

impl AppState {
    /// 初始化：打开数据库并执行崩溃恢复（Processing 僵尸 → Failed，REQ-DB-001-AC-2）。
    pub async fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        let db_path = data_dir.join("echomind.db");
        let storage = tokio::task::spawn_blocking(move || SqliteStorage::new(&db_path))
            .await
            .context("存储初始化任务失败")?
            .context("初始化 SQLite 存储失败")?;

        let cleaned = storage
            .cleanup_zombies()
            .await
            .context("崩溃恢复清理失败")?;
        if cleaned > 0 {
            eprintln!("崩溃恢复：已将 {cleaned} 条中断的索引任务标记为失败");
        }

        Ok(Self { storage, data_dir })
    }
}
