//! 向量与分块表 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S03 拆分：分块写入、向量写入、关键词检索（FTS5 BM25）、
//! 嵌入缓存、BM25 索引重建、分块查询/删除、路径查找。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。
//! `vector_search` 保留在 `sqlite_storage.rs` 主文件（依赖 HNSW / 内存缓存 / self 状态）。

use std::path::Path;

use anyhow::Context;
use echomind_models::{Chunk, RetrievalResult};
use rusqlite::params;

use super::migration::Pool;
use crate::sqlite_storage::{
    SqliteStorage, build_fts5_or_query, bytes_to_vec, cosine_similarity, run_db, vec_to_bytes,
    with_transaction,
};

// ============================================================================
// 分块写入
// ============================================================================

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
        // Contextual BM25（REQ-PERF-005）：FTS5 索引使用文档名前缀
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

// ============================================================================
// 向量写入
// ============================================================================

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

// ============================================================================
// 分块查询与删除
// ============================================================================

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

// ============================================================================
// 关键词检索（FTS5 BM25）
// ============================================================================

/// 关键词全文检索（FTS5 BM25 排序，REQ-RAG-010）。
///
/// trigram 分词器执行子串匹配，BM25 算法排序。
/// 短查询（<3 字符）回退为 SQL LIKE 全表扫描。
pub(crate) async fn keyword_search(
    pool: &Pool,
    query: &str,
    top_k: usize,
) -> anyhow::Result<Vec<RetrievalResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() || top_k == 0 {
        return Ok(vec![]);
    }
    let pool = pool.clone();
    let query = trimmed.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        if query.chars().count() < 3 {
            let pattern = format!("%{query}%");
            let mut stmt = conn.prepare(
                "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, d.file_path
                 FROM chunks c
                 JOIN documents d ON d.id = c.doc_id
                 WHERE c.content LIKE ?1
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![pattern, top_k], |row| {
                let file_path: String = row.get(5)?;
                let doc_name = Path::new(&file_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_path.clone());
                Ok(RetrievalResult {
                    chunk: Chunk {
                        id: row.get(0)?,
                        doc_id: row.get(1)?,
                        content: row.get(2)?,
                        token_count: row.get(3)?,
                        sequence: row.get(4)?,
                    },
                    score: 1.0,
                    doc_name,
                })
            })?;
            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            return Ok(results);
        }
        let fts_query = build_fts5_or_query(&query);
        if fts_query.is_empty() {
            return Ok(vec![]);
        }
        let mut stmt = conn.prepare(
            "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, d.file_path
             FROM chunks_fts fts
             JOIN chunks c ON c.id = fts.chunk_id
             JOIN documents d ON d.id = c.doc_id
             WHERE chunks_fts MATCH ?1
             ORDER BY bm25(chunks_fts)
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, top_k], |row| {
            let file_path: String = row.get(5)?;
            let doc_name = Path::new(&file_path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| file_path.clone());
            Ok(RetrievalResult {
                chunk: Chunk {
                    id: row.get(0)?,
                    doc_id: row.get(1)?,
                    content: row.get(2)?,
                    token_count: row.get(3)?,
                    sequence: row.get(4)?,
                },
                score: 1.0,
                doc_name,
            })
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    })
    .await
}

// ============================================================================
// 嵌入缓存
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
        SqliteStorage::find_cached_embedding_sync(&conn, &hash)
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
        SqliteStorage::cache_embedding_sync(&conn, &hash, &emb)
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

// ============================================================================
// BM25 索引重建
// ============================================================================

/// 重建 BM25 全文索引（REQ-PERF-005 Contextual BM25）。
pub(crate) async fn rebuild_bm25_index(pool: &Pool) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            conn.execute_batch("DELETE FROM chunks_fts;")
                .context("清空 FTS5 索引失败")?;
            conn.execute_batch(
                "INSERT INTO chunks_fts (chunk_id, doc_id, content)
                 SELECT c.id, c.doc_id,
                   '文档《' ||
                   COALESCE(substr(d.file_path, instr(replace(d.file_path, '\\', '/'), '/') + 1), d.file_path)
                   || '》：\n' || c.content
                 FROM chunks c
                 JOIN documents d ON d.id = c.doc_id;",
            )
            .context("重建 FTS5 索引失败")?;
            Ok(())
        })
    })
    .await
}

// ============================================================================
// 路径查找
// ============================================================================

/// Proposition 向量检索辅助：计算余弦相似度。
///
/// 此函数供 `proposition_search` 在 entities.rs 中使用，
/// 避免在 entities.rs 中重复引用 `cosine_similarity`。
#[allow(dead_code)]
pub(crate) fn _cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    cosine_similarity(a, b)
}
