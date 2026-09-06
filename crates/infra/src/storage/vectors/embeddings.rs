//! 向量写入

use anyhow::Context;
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, vec_to_bytes, with_transaction};

/// 写入单个嵌入向量（INSERT OR REPLACE）。
pub(crate) async fn add_embedding(
    pool: &Pool,
    chunk_id: &str,
    embedding: &[f32],
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let chunk_id = chunk_id.to_string();
    let bytes = vec_to_bytes(embedding);
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?1, ?2)",
            params![chunk_id, bytes],
        )
        .context("写入向量失败")?;
        Ok(())
    })
    .await
}

/// 批量写入向量（单事务 + 仅一次缓存失效）。
pub(crate) async fn add_embeddings_batch(
    pool: &Pool,
    embeddings: &[(String, Vec<f32>)],
) -> anyhow::Result<()> {
    if embeddings.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let items: Vec<(String, Vec<u8>)> = embeddings
        .iter()
        .map(|(id, vec)| (id.clone(), vec_to_bytes(vec)))
        .collect();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut stmt = conn
                .prepare("INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?1, ?2)")
                .context("预编译 embeddings 批量写入语句失败")?;
            for (chunk_id, bytes) in &items {
                stmt.execute(params![chunk_id, bytes])
                    .context("批量写入 embedding 失败")?;
            }
            Ok(())
        })
    })
    .await
}
