//! 设置存储（从 `sqlite_storage.rs` 拆分）。
//!
//! 提供加密的键值设置存储，支持单条和批量读写。

use anyhow::Context;
use rusqlite::params;

use super::migration::Pool;
use crate::sqlite_storage::{crypto_decrypt, crypto_encrypt, run_db};

/// 写入单个设置项（加密存储）。
pub(crate) async fn set_setting(
    pool: &Pool,
    cipher: &aes_gcm::Aes256Gcm,
    key: &str,
    value: &str,
) -> anyhow::Result<()> {
    let encrypted = crypto_encrypt(cipher, value)?;
    let pool = pool.clone();
    let key = key.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, encrypted],
        )
        .context("写入设置项失败")?;
        Ok(())
    })
    .await
}

/// 批量写入设置项：单次事务 + 批量 INSERT OR REPLACE。
///
/// 性能优化：N 次 set_setting = N 次 spawn_blocking + N 次 DB 连接获取 + N 次 SQL 执行。
/// 批量写入 = 1 次 spawn_blocking + 1 次 DB 连接 + 1 次事务 + N 条 INSERT。
pub(crate) async fn set_settings_batch(
    pool: &Pool,
    cipher: &aes_gcm::Aes256Gcm,
    pairs: &[(&str, &str)],
) -> anyhow::Result<()> {
    if pairs.is_empty() {
        return Ok(());
    }
    let encrypted_pairs: Vec<(String, String)> = pairs
        .iter()
        .map(|(key, value)| {
            let enc = crypto_encrypt(cipher, value)?;
            Ok(((*key).to_string(), enc))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        crate::sqlite_storage::with_transaction(&conn, |conn| {
            for (key, encrypted) in &encrypted_pairs {
                conn.execute(
                    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                    params![key, encrypted],
                )
                .with_context(|| format!("写入设置项 {key} 失败"))?;
            }
            Ok(())
        })
    })
    .await
}

/// 读取单个设置项（解密返回）。
pub(crate) async fn get_setting(
    pool: &Pool,
    cipher: &aes_gcm::Aes256Gcm,
    key: &str,
) -> anyhow::Result<Option<String>> {
    let pool = pool.clone();
    let key = key.to_string();
    let encoded = run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    })
    .await?;
    match encoded {
        Some(v) => Ok(Some(crypto_decrypt(cipher, &v)?)),
        None => Ok(None),
    }
}

/// 批量读取设置项：单次 SQL `WHERE key IN (...)` 替代 N 次串行 `get_setting`。
///
/// 性能优化：`chat_inner` 原先串行调用 8+ 次 `get_setting`，每次触发独立的
/// `spawn_blocking` + DB 连接获取 + SQL 执行。批量读取将 8 次往返压缩为 1 次。
pub(crate) async fn get_settings_batch(
    pool: &Pool,
    cipher: &aes_gcm::Aes256Gcm,
    keys: &[&str],
) -> anyhow::Result<Vec<(String, String)>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let pool = pool.clone();
    let keys: Vec<String> = keys.iter().map(|s| s.to_string()).collect();
    let encoded_pairs = run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let placeholders: String = (0..keys.len()).map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT key, value FROM settings WHERE key IN ({placeholders})");
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            keys.iter().map(|k| k as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    })
    .await?;
    let mut results = Vec::with_capacity(encoded_pairs.len());
    for (key, encoded) in encoded_pairs {
        match crypto_decrypt(cipher, &encoded) {
            Ok(value) => results.push((key, value)),
            Err(e) => {
                tracing::warn!("设置项 {key} 解密失败，跳过: {e}");
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::sqlite_storage::SqliteStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_set_get_setting() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("settings-test.db")).unwrap();
        let pool = storage.pool_clone();
        let cipher = storage.cipher.clone();

        set_setting(&pool, &cipher, "test.key", "test_value")
            .await
            .unwrap();
        let value = get_setting(&pool, &cipher, "test.key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        let missing = get_setting(&pool, &cipher, "missing.key").await.unwrap();
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn test_batch_settings() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("batch-settings.db")).unwrap();
        let pool = storage.pool_clone();
        let cipher = storage.cipher.clone();

        set_settings_batch(&pool, &cipher, &[("k1", "v1"), ("k2", "v2"), ("k3", "v3")])
            .await
            .unwrap();

        let results = get_settings_batch(&pool, &cipher, &["k1", "k2", "k3"])
            .await
            .unwrap();
        assert_eq!(results.len(), 3);
        let map: std::collections::HashMap<_, _> = results.into_iter().collect();
        assert_eq!(map.get("k1"), Some(&"v1".to_string()));
        assert_eq!(map.get("k2"), Some(&"v2".to_string()));
        assert_eq!(map.get("k3"), Some(&"v3".to_string()));
    }

    #[tokio::test]
    async fn test_batch_overwrite() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("batch-overwrite.db")).unwrap();
        let pool = storage.pool_clone();
        let cipher = storage.cipher.clone();

        set_setting(&pool, &cipher, "key", "old").await.unwrap();
        set_settings_batch(&pool, &cipher, &[("key", "new"), ("key2", "val2")])
            .await
            .unwrap();

        let value = get_setting(&pool, &cipher, "key").await.unwrap();
        assert_eq!(value, Some("new".to_string()));
    }

    #[tokio::test]
    async fn test_empty_batch() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("empty-batch.db")).unwrap();
        let pool = storage.pool_clone();
        let cipher = storage.cipher.clone();

        set_settings_batch(&pool, &cipher, &[]).await.unwrap();
        let results = get_settings_batch(&pool, &cipher, &[]).await.unwrap();
        assert!(results.is_empty());
    }
}
