//! 关键词检索（FTS5 BM25）

use std::path::Path;

use anyhow::Context;
use echomind_models::{Chunk, RetrievalResult};
use rusqlite::params;

use super::super::migration::Pool;
use super::fts5::build_fts5_or_query;
use crate::sqlite_storage::run_db;

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
