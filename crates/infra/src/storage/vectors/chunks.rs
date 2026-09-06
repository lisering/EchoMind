//! 分块写入、查询与删除

use anyhow::Context;
use echomind_models::Chunk;
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

/// 写入单个分块（INSERT OR REPLACE）+ FTS5 索引（Contextual BM25）。
pub(crate) async fn add_chunk(pool: &Pool, chunk: &Chunk) -> anyhow::Result<()> {
    let pool = pool.clone();
    let chunk = chunk.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR REPLACE INTO chunks (id, doc_id, content, token_count, sequence) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chunk.id, chunk.doc_id, chunk.content, chunk.token_count, chunk.sequence],
        )
        .context("写入分块失败")?;
        let doc_name: String = conn
            .query_row(
                "SELECT file_path FROM documents WHERE id = ?1",
                params![chunk.doc_id],
                |row| row.get(0),
            )
            .map(|fp: String| {
                std::path::Path::new(&fp)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or(fp)
            })
            .unwrap_or_else(|_| "unknown".to_string());
        let contextual_content =
            echomind_core::retriever::build_contextual_text(&doc_name, &chunk.content);
        conn.execute(
            "INSERT INTO chunks_fts (chunk_id, doc_id, content) VALUES (?1, ?2, ?3)",
            params![chunk.id, chunk.doc_id, contextual_content],
        )
        .context("写入 FTS5 索引失败")?;
        Ok(())
    })
    .await
}

/// 批量写入分块（单事务，性能优化）。
pub(crate) async fn add_chunks_batch(pool: &Pool, chunks: &[Chunk]) -> anyhow::Result<()> {
    if chunks.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let chunks = chunks.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut chunk_stmt = conn
                .prepare(
                    "INSERT OR REPLACE INTO chunks (id, doc_id, content, token_count, sequence) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .context("预编译 chunks 批量写入语句失败")?;
            let mut fts_stmt = conn
                .prepare("INSERT INTO chunks_fts (chunk_id, doc_id, content) VALUES (?1, ?2, ?3)")
                .context("预编译 FTS5 批量写入语句失败")?;

            let mut doc_name_cache: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for chunk in &chunks {
                let doc_name = doc_name_cache
                    .entry(chunk.doc_id.clone())
                    .or_insert_with(|| {
                        conn.query_row(
                            "SELECT file_path FROM documents WHERE id = ?1",
                            params![&chunk.doc_id],
                            |row| row.get(0),
                        )
                        .map(|fp: String| {
                            std::path::Path::new(&fp)
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or(fp)
                        })
                        .unwrap_or_else(|_| "unknown".to_string())
                    });

                chunk_stmt
                    .execute(params![
                        chunk.id,
                        chunk.doc_id,
                        chunk.content,
                        chunk.token_count,
                        chunk.sequence,
                    ])
                    .context("批量写入 chunks 失败")?;
                let contextual_content =
                    echomind_core::retriever::build_contextual_text(doc_name, &chunk.content);
                fts_stmt
                    .execute(params![chunk.id, chunk.doc_id, contextual_content])
                    .context("批量写入 FTS5 索引失败")?;
            }
            Ok(())
        })
    })
    .await
}

/// 列出指定文档的所有分块（按 sequence 升序）。
pub(crate) async fn list_chunks(pool: &Pool, doc_id: &str) -> anyhow::Result<Vec<Chunk>> {
    let pool = pool.clone();
    let doc_id = doc_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, doc_id, content, token_count, sequence FROM chunks WHERE doc_id = ?1 ORDER BY sequence ASC",
        )?;
        let rows = stmt.query_map(params![doc_id], |row| {
            Ok(Chunk {
                id: row.get(0)?,
                doc_id: row.get(1)?,
                content: row.get(2)?,
                token_count: row.get(3)?,
                sequence: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

/// 删除指定文档的全部分块（外键级联自动清理 embeddings，REQ-VEC-005）。
pub(crate) async fn delete_chunks_by_doc(pool: &Pool, doc_id: &str) -> anyhow::Result<()> {
    let pool = pool.clone();
    let doc_id = doc_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM chunks_fts WHERE doc_id = ?1", params![doc_id])
            .context("清理 FTS5 索引失败")?;
        conn.execute("DELETE FROM chunks WHERE doc_id = ?1", params![doc_id])
            .context("删除分块失败")?;
        Ok(())
    })
    .await
}

/// 按 chunk ID 查找单个 chunk（REQ-RAG-027 图遍历检索用）。
pub(crate) async fn get_chunk_by_id(pool: &Pool, chunk_id: &str) -> anyhow::Result<Option<Chunk>> {
    let pool = pool.clone();
    let chunk_id = chunk_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, doc_id, content, token_count, sequence \
             FROM chunks WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![chunk_id], |row| {
            Ok(Chunk {
                id: row.get(0)?,
                doc_id: row.get(1)?,
                content: row.get(2)?,
                token_count: row.get(3)?,
                sequence: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    })
    .await
}
