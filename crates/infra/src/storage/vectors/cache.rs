//! 嵌入缓存（同步与异步版本）

use anyhow::Context;
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{bytes_to_vec, run_db, vec_to_bytes, with_transaction};

// ============================================================================
// 嵌入缓存（同步版本，供 embedder 调用）
// ============================================================================

/// 按内容哈希查找缓存的嵌入向量（同步版本，供 spawn_blocking 内调用）。
pub(crate) fn find_cached_embedding_sync(
    conn: &rusqlite::Connection,
    content_hash: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    let mut stmt = conn
        .prepare("SELECT embedding FROM embeddings_cache WHERE content_hash = ?1")
        .context("查询嵌入缓存失败")?;
    let mut rows = stmt.query_map(params![content_hash], |row| row.get::<_, Vec<u8>>(0))?;
    match rows.next() {
        Some(Ok(bytes)) => Ok(Some(bytes_to_vec(&bytes)?)),
        Some(Err(e)) => Err(e.into()),
        None => Ok(None),
    }
}

/// 将嵌入向量写入缓存（同步版本，供 spawn_blocking 内调用）。
///
/// 使用 `INSERT OR IGNORE` — 并发场景下首次写入者胜出，后续写入静默跳过。
pub(crate) fn cache_embedding_sync(
    conn: &rusqlite::Connection,
    content_hash: &str,
    embedding: &[f32],
) -> anyhow::Result<()> {
    let bytes = vec_to_bytes(embedding);
    conn.execute(
        "INSERT OR IGNORE INTO embeddings_cache (content_hash, embedding) VALUES (?1, ?2)",
        params![content_hash, bytes],
    )
    .context("写入嵌入缓存失败")?;
    Ok(())
}

// ============================================================================
// 嵌入缓存（异步版本，供 Storage trait 调用）
// ============================================================================

/// 按内容哈希查找缓存的嵌入向量。
pub(crate) async fn lookup_embedding_cache(
    pool: &Pool,
    content_hash: &str,
) -> anyhow::Result<Option<Vec<f32>>> {
    let pool = pool.clone();
    let hash = content_hash.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        find_cached_embedding_sync(&conn, &hash)
    })
    .await
}

/// 将嵌入向量写入缓存（INSERT OR IGNORE）。
pub(crate) async fn put_embedding_cache(
    pool: &Pool,
    content_hash: &str,
    embedding: &[f32],
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let hash = content_hash.to_string();
    let emb = embedding.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        cache_embedding_sync(&conn, &hash, &emb)
    })
    .await
}

/// 批量查找缓存的嵌入向量（临时表 + JOIN 避免 IN 参数限制）。
pub(crate) async fn lookup_embedding_cache_batch(
    pool: &Pool,
    hashes: &[String],
) -> anyhow::Result<Vec<(usize, Vec<f32>)>> {
    if hashes.is_empty() {
        return Ok(Vec::new());
    }
    let pool = pool.clone();
    let hashes = hashes.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            conn.execute(
                "CREATE TEMP TABLE IF NOT EXISTS _batch_lookup (idx INTEGER PRIMARY KEY, hash TEXT NOT NULL)",
                [],
            )?;
            conn.execute("DELETE FROM _batch_lookup", [])?;
            {
                let mut stmt = conn
                    .prepare("INSERT INTO _batch_lookup (idx, hash) VALUES (?1, ?2)")?;
                for (i, hash) in hashes.iter().enumerate() {
                    stmt.execute(params![i as i64, hash])?;
                }
            }
            Ok(())
        })?;

        let mut stmt = conn.prepare(
            "SELECT b.idx, e.embedding
             FROM _batch_lookup b
             JOIN embeddings_cache e ON e.content_hash = b.hash",
        )?;
        let rows = stmt.query_map([], |row| {
            let idx: i64 = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            Ok((idx as usize, bytes))
        })?;
        let mut hits = Vec::new();
        for row in rows {
            let (idx, bytes) = row?;
            hits.push((idx, bytes_to_vec(&bytes)?));
        }
        conn.execute("DELETE FROM _batch_lookup", []).ok();
        Ok(hits)
    })
    .await
}

/// 批量写入嵌入向量缓存（单事务批量 INSERT OR IGNORE）。
pub(crate) async fn put_embedding_cache_batch(
    pool: &Pool,
    items: &[(String, Vec<f32>)],
) -> anyhow::Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let items: Vec<(String, Vec<u8>)> = items
        .iter()
        .map(|(hash, vec)| (hash.clone(), vec_to_bytes(vec)))
        .collect();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut stmt = conn
                .prepare(
                    "INSERT OR IGNORE INTO embeddings_cache (content_hash, embedding) VALUES (?1, ?2)",
                )
                .context("预编译 embeddings_cache 批量写入语句失败")?;
            for (hash, bytes) in &items {
                stmt.execute(params![hash, bytes])
                    .context("批量写入嵌入缓存失败")?;
            }
            Ok(())
        })
    })
    .await
}
