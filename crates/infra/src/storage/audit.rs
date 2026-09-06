//! 审计日志存储（从 `sqlite_storage.rs` 拆分）。
//!
//! 实现 `AuditLogger` trait，提供哈希链审计日志的持久化。

use anyhow::Context;
use echomind_core::privacy::{AuditEntry, AuditLogger};
use rusqlite::params;

use super::migration::Pool;
use crate::sqlite_storage::run_db;

/// 写入审计日志条目（哈希链）。
pub(crate) async fn log_audit_entry(pool: &Pool, entry: AuditEntry) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO audit_log (id, action, details, pii_count, timestamp, prev_hash, curr_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.id,
                entry.action,
                entry.details,
                entry.pii_count as i64,
                entry.timestamp,
                entry.prev_hash,
                entry.curr_hash,
            ],
        )
        .context("写入审计日志失败")?;
        Ok(())
    })
    .await
}

/// 列出最近的审计日志条目（按时间倒序）。
pub(crate) async fn list_audit_entries(
    pool: &Pool,
    limit: usize,
) -> anyhow::Result<Vec<AuditEntry>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, action, details, pii_count, timestamp, prev_hash, curr_hash
                 FROM audit_log ORDER BY timestamp DESC LIMIT ?1",
            )
            .context("准备审计日志查询失败")?;
        let entries = stmt
            .query_map(params![limit as i64], |row| {
                Ok(AuditEntry {
                    id: row.get(0)?,
                    action: row.get(1)?,
                    details: row.get(2)?,
                    pii_count: row.get::<_, i64>(3)? as usize,
                    timestamp: row.get(4)?,
                    prev_hash: row.get(5)?,
                    curr_hash: row.get(6)?,
                })
            })
            .context("查询审计日志失败")?;
        let mut result = Vec::new();
        for entry in entries {
            result.push(entry.context("解析审计日志行失败")?);
        }
        Ok(result)
    })
    .await
}

/// 清空审计日志。
pub(crate) async fn clear_audit_log(pool: &Pool) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM audit_log", [])
            .context("清空审计日志失败")?;
        Ok(())
    })
    .await
}

/// 清理过期的审计日志条目（按天数）。
pub(crate) async fn purge_old_audit_entries(
    pool: &Pool,
    max_age_days: usize,
) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let now = chrono::Utc::now().timestamp();
        let cutoff = now - (max_age_days as i64) * 86_400;
        let deleted = conn
            .execute(
                "DELETE FROM audit_log WHERE timestamp < ?1",
                params![cutoff],
            )
            .context("轮转审计日志失败")?;
        Ok(deleted)
    })
    .await
}

/// 统计审计日志条目总数。
pub(crate) async fn count_audit_entries(pool: &Pool) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
            .context("统计审计日志失败")?;
        Ok(count as usize)
    })
    .await
}

/// `AuditLogger` trait 的默认实现，委托给上述函数。
pub(crate) struct SqliteAuditLogger {
    pool: Pool,
}

impl SqliteAuditLogger {
    pub(crate) fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

impl AuditLogger for SqliteAuditLogger {
    fn log<'a>(
        &'a self,
        entry: AuditEntry,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();
        Box::pin(async move { log_audit_entry(&pool, entry).await })
    }

    fn list_entries<'a>(
        &'a self,
        limit: usize,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<AuditEntry>>> + Send + 'a>,
    > {
        let pool = self.pool.clone();
        Box::pin(async move { list_audit_entries(&pool, limit).await })
    }

    fn clear<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();
        Box::pin(async move { clear_audit_log(&pool).await })
    }

    fn purge_old_entries<'a>(
        &'a self,
        max_age_days: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<usize>> + Send + 'a>>
    {
        let pool = self.pool.clone();
        Box::pin(async move { purge_old_audit_entries(&pool, max_age_days).await })
    }

    fn count_entries<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<usize>> + Send + 'a>>
    {
        let pool = self.pool.clone();
        Box::pin(async move { count_audit_entries(&pool).await })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::sqlite_storage::SqliteStorage;
    use echomind_core::privacy::AuditLogger;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_audit_logger_basic() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("audit-test.db")).unwrap();
        let logger = SqliteAuditLogger::new(storage.pool_clone());

        let entry = AuditEntry {
            id: "test-1".to_string(),
            action: "test_action".to_string(),
            details: "test details".to_string(),
            pii_count: 0,
            timestamp: chrono::Utc::now().timestamp(),
            prev_hash: Some("prev".to_string()),
            curr_hash: Some("curr".to_string()),
        };

        logger.log(entry.clone()).await.unwrap();

        let entries = logger.list_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "test-1");
        assert_eq!(entries[0].action, "test_action");
    }

    #[tokio::test]
    async fn test_audit_logger_clear() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("audit-clear.db")).unwrap();
        let logger = SqliteAuditLogger::new(storage.pool_clone());

        let entry = AuditEntry {
            id: "test-1".to_string(),
            action: "test".to_string(),
            details: "d".to_string(),
            pii_count: 0,
            timestamp: chrono::Utc::now().timestamp(),
            prev_hash: Some("p".to_string()),
            curr_hash: Some("c".to_string()),
        };
        logger.log(entry).await.unwrap();

        logger.clear().await.unwrap();
        let entries = logger.list_entries(10).await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn test_audit_logger_purge() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("audit-purge.db")).unwrap();
        let logger = SqliteAuditLogger::new(storage.pool_clone());

        let now = chrono::Utc::now().timestamp();
        let old_entry = AuditEntry {
            id: "old".to_string(),
            action: "old".to_string(),
            details: "d".to_string(),
            pii_count: 0,
            timestamp: now - 100_000,
            prev_hash: Some("p".to_string()),
            curr_hash: Some("c".to_string()),
        };
        let new_entry = AuditEntry {
            id: "new".to_string(),
            action: "new".to_string(),
            details: "d".to_string(),
            pii_count: 0,
            timestamp: now,
            prev_hash: Some("p".to_string()),
            curr_hash: Some("c".to_string()),
        };
        logger.log(old_entry).await.unwrap();
        logger.log(new_entry).await.unwrap();

        let deleted = logger.purge_old_entries(1).await.unwrap();
        assert_eq!(deleted, 1);

        let entries = logger.list_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "new");
    }

    #[tokio::test]
    async fn test_audit_logger_count() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("audit-count.db")).unwrap();
        let logger = SqliteAuditLogger::new(storage.pool_clone());

        assert_eq!(logger.count_entries().await.unwrap(), 0);

        let entry = AuditEntry {
            id: "test-1".to_string(),
            action: "test".to_string(),
            details: "d".to_string(),
            pii_count: 0,
            timestamp: chrono::Utc::now().timestamp(),
            prev_hash: Some("p".to_string()),
            curr_hash: Some("c".to_string()),
        };
        logger.log(entry).await.unwrap();
        assert_eq!(logger.count_entries().await.unwrap(), 1);
    }
}
