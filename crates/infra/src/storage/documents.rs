//! 文档表 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S02 拆分：文档写入、状态更新、哈希查重、统计计数、僵尸清理、
//! 列表/分页、标签管理、导入日志、知识库统计、工作空间文档操作。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

use anyhow::Context;
use echomind_models::{DocStatus, Document, DocumentSearchResult};
use rusqlite::params;

use super::schema::DOC_COLS;
use crate::sqlite_storage::{row_to_document, run_db, status_to_row};

/// 拼接 `INSERT OR REPLACE INTO documents` SQL 语句。
fn insert_document_sql() -> String {
    format!(
        "INSERT OR REPLACE INTO documents ({DOC_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
    )
}

// ============================================================================
// 文档写入与状态更新
// ============================================================================

/// 写入文档行（INSERT OR REPLACE）。
pub(crate) async fn add_document(
    pool: &super::migration::Pool,
    doc: Document,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let (status, reason) = status_to_row(&doc.status);
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            &insert_document_sql(),
            params![
                doc.id,
                doc.file_path,
                doc.file_hash,
                status,
                reason,
                doc.created_at,
                doc.original_path,
                doc.domain,
                doc.summary,
                serde_json::to_string(&doc.tags).unwrap_or_else(|_| "[]".to_string()),
                doc.workspace_id,
            ],
        )
        .context("写入文档失败")?;
        Ok(())
    })
    .await
}

/// 更新文档状态。
pub(crate) async fn update_doc_status(
    pool: &super::migration::Pool,
    doc_id: String,
    status: DocStatus,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let (status_str, reason) = status_to_row(&status);
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "UPDATE documents SET status = ?1, status_reason = ?2 WHERE id = ?3",
            params![status_str, reason, doc_id],
        )
        .context("更新文档状态失败")?;
        Ok(())
    })
    .await
}

// ============================================================================
// 查重与统计
// ============================================================================

/// 按文件哈希查重。
pub(crate) async fn find_document_by_hash(
    pool: &super::migration::Pool,
    hash: String,
) -> anyhow::Result<Option<Document>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE file_hash = ?1 LIMIT 1"
        ))?;
        let mut rows = stmt.query_map(params![hash], row_to_document)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    })
    .await
}

/// 统计文档总数。
pub(crate) async fn count_documents(pool: &super::migration::Pool) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .context("统计文档数失败")?;
        Ok(count as usize)
    })
    .await
}

/// 统计分块总数。
pub(crate) async fn count_chunks(pool: &super::migration::Pool) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .context("统计分块数失败")?;
        Ok(count as usize)
    })
    .await
}

/// 统计向量总数。
pub(crate) async fn count_embeddings(pool: &super::migration::Pool) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
            .context("统计向量数失败")?;
        Ok(count as usize)
    })
    .await
}

/// 崩溃恢复：将 Processing 文档标记为 Failed。
pub(crate) async fn cleanup_zombies(pool: &super::migration::Pool) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let affected = conn.execute(
            "UPDATE documents SET status = 'failed', status_reason = ?1 WHERE status = 'processing'",
            params!["崩溃恢复：上次会话中断"],
        )?;
        Ok(affected)
    })
    .await
}

// ============================================================================
// 列表与分页
// ============================================================================

/// 列出全部文档（按创建时间降序）。
pub(crate) async fn list_documents(pool: &super::migration::Pool) -> anyhow::Result<Vec<Document>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents ORDER BY created_at DESC, rowid DESC"
        ))?;
        let rows = stmt.query_map([], row_to_document)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

/// 分页列出文档。
pub(crate) async fn list_documents_paginated(
    pool: &super::migration::Pool,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<Document>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents ORDER BY created_at DESC, rowid DESC LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], row_to_document)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

// ============================================================================
// 标签管理（REQ-ING-022）
// ============================================================================

/// 添加文档标签。
pub(crate) async fn add_document_tag(
    pool: &super::migration::Pool,
    doc_id: String,
    tag: String,
) -> anyhow::Result<()> {
    let tag = tag.trim().to_string();
    if tag.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let tags_json: String = conn
            .query_row(
                "SELECT tags FROM documents WHERE id = ?1",
                params![doc_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "[]".to_string());
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if !tags.iter().any(|t| t == &tag) {
            tags.push(tag);
            let new_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE documents SET tags = ?1 WHERE id = ?2",
                params![new_json, doc_id],
            )
            .context("更新文档标签失败")?;
        }
        Ok(())
    })
    .await
}

/// 移除文档标签。
pub(crate) async fn remove_document_tag(
    pool: &super::migration::Pool,
    doc_id: String,
    tag: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let tags_json: String = conn
            .query_row(
                "SELECT tags FROM documents WHERE id = ?1",
                params![doc_id],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "[]".to_string());
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        let before_len = tags.len();
        tags.retain(|t| t != &tag);
        if tags.len() != before_len {
            let new_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "UPDATE documents SET tags = ?1 WHERE id = ?2",
                params![new_json, doc_id],
            )
            .context("更新文档标签失败")?;
        }
        Ok(())
    })
    .await
}

/// 列出所有标签及其文档数（按计数降序）。
pub(crate) async fn list_all_tags(
    pool: &super::migration::Pool,
) -> anyhow::Result<Vec<(String, usize)>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare("SELECT tags FROM documents")?;
        let rows = stmt.query_map([], |row| {
            let tags_json: String = row.get(0).unwrap_or_else(|_| "[]".to_string());
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            Ok(tags)
        })?;
        let mut tag_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for row in rows {
            let tags = row?;
            for tag in tags {
                *tag_counts.entry(tag).or_insert(0) += 1;
            }
        }
        let mut result: Vec<(String, usize)> = tag_counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        Ok(result)
    })
    .await
}

/// 按标签筛选文档。
pub(crate) async fn filter_documents_by_tag(
    pool: &super::migration::Pool,
    tag: &str,
) -> anyhow::Result<Vec<Document>> {
    let pattern = format!("\"{}\"", tag);
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE tags LIKE ?1 ORDER BY created_at DESC, rowid DESC"
        ))?;
        let rows = stmt.query_map(params![format!("%{pattern}%")], row_to_document)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

// ============================================================================
// 导入历史记录（REQ-ING-011）
// ============================================================================

/// 添加导入日志（保留最近 100 条）。
pub(crate) async fn add_import_log(
    pool: &super::migration::Pool,
    file_name: String,
    format: String,
    result: String,
    error_message: Option<String>,
    file_size: Option<i64>,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "INSERT INTO import_logs (timestamp, file_name, format, result, error_message, file_size) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![now, &file_name, &format, &result, &error_message, file_size],
        )?;
        conn.execute(
            "DELETE FROM import_logs WHERE id NOT IN (SELECT id FROM import_logs ORDER BY id DESC LIMIT 100)",
            [],
        )?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}

/// 查询导入日志（可选结果过滤，最近 100 条）。
pub(crate) async fn get_import_logs(
    pool: &super::migration::Pool,
    result_filter: Option<String>,
) -> anyhow::Result<Vec<echomind_models::ImportLogEntry>> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        let mut entries = Vec::new();
        if let Some(ref f) = result_filter {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, file_name, format, result, error_message, file_size FROM import_logs WHERE result = ?1 ORDER BY id DESC LIMIT 100",
            )?;
            let rows = stmt.query_map(rusqlite::params![f], |row| {
                Ok(echomind_models::ImportLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    file_name: row.get(2)?,
                    format: row.get(3)?,
                    result: row.get(4)?,
                    error_message: row.get(5)?,
                    file_size: row.get(6)?,
                })
            })?;
            for row in rows {
                entries.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, timestamp, file_name, format, result, error_message, file_size FROM import_logs ORDER BY id DESC LIMIT 100",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(echomind_models::ImportLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    file_name: row.get(2)?,
                    format: row.get(3)?,
                    result: row.get(4)?,
                    error_message: row.get(5)?,
                    file_size: row.get(6)?,
                })
            })?;
            for row in rows {
                entries.push(row?);
            }
        }
        Ok(entries)
    })
    .await?
}

/// 清空导入日志。
pub(crate) async fn clear_import_logs(pool: &super::migration::Pool) -> anyhow::Result<()> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        conn.execute("DELETE FROM import_logs", [])?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}

// ============================================================================
// 工作空间文档操作
// ============================================================================

/// 统计工作空间文档数。
pub(crate) async fn count_documents_in_workspace(
    pool: &super::migration::Pool,
    workspace_id: String,
) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM documents WHERE workspace_id = ?1",
                params![&workspace_id],
                |row| row.get(0),
            )
            .context("统计工作空间文档数失败")?;
        Ok(count as usize)
    })
    .await
}

/// 列出工作空间文档。
pub(crate) async fn list_documents_in_workspace(
    pool: &super::migration::Pool,
    workspace_id: String,
) -> anyhow::Result<Vec<Document>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE workspace_id = ?1 ORDER BY created_at DESC, rowid DESC"
        ))?;
        let rows = stmt.query_map(params![&workspace_id], row_to_document)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

/// 迁移文档到目标工作空间。
pub(crate) async fn migrate_document(
    pool: &super::migration::Pool,
    doc_id: String,
    target_workspace_id: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "UPDATE documents SET workspace_id = ?1 WHERE id = ?2",
            params![&target_workspace_id, &doc_id],
        )
        .context("迁移文档失败")?;
        Ok(())
    })
    .await
}

/// 按源文件路径精确查找文档。
pub(crate) async fn find_document_by_original_path(
    pool: &super::migration::Pool,
    path: String,
) -> anyhow::Result<Option<Document>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE original_path = ?1 LIMIT 1"
        ))?;
        let mut rows = stmt.query_map(params![path], row_to_document)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    })
    .await
}

/// 按源文件路径前缀查找文档。
pub(crate) async fn find_documents_by_original_path_prefix(
    pool: &super::migration::Pool,
    prefix: String,
) -> anyhow::Result<Vec<Document>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let pattern = format!("{prefix}%");
        let mut stmt = conn.prepare(&format!(
            "SELECT {DOC_COLS} FROM documents WHERE original_path LIKE ?1"
        ))?;
        let rows = stmt.query_map(params![pattern], row_to_document)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    })
    .await
}

/// 删除文档（含 FTS5 清理）。
pub(crate) async fn delete_document(
    pool: &super::migration::Pool,
    doc_id: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM chunks_fts WHERE doc_id = ?1", params![doc_id])
            .context("清理 FTS5 索引失败")?;
        conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
            .context("删除文档失败")?;
        Ok(())
    })
    .await
}

// ============================================================================
// 全局搜索（REQ-IX-008）
// ============================================================================

/// 全局搜索文档（文件名 + 摘要 LIKE 匹配，REQ-IX-008）。
///
/// 搜索 documents 表的 `file_path` 和 `summary` 列。
/// 返回结果按文件名匹配优先排序，每组最多 `limit` 条。
pub(crate) async fn search_documents(
    pool: &super::migration::Pool,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<DocumentSearchResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let pool = pool.clone();
    let pattern = format!("%{trimmed}%");
    let trimmed_lower = trimmed.to_lowercase();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, file_path, summary FROM documents \
             WHERE file_path LIKE ?1 OR summary LIKE ?1 \
             ORDER BY CASE WHEN file_path LIKE ?1 THEN 0 ELSE 1 END \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            let file_path: String = row.get(1)?;
            let summary: Option<String> = row.get(2)?;
            let match_type = if file_path.to_lowercase().contains(&trimmed_lower) {
                "title"
            } else {
                "summary"
            };
            Ok(DocumentSearchResult {
                doc_id: row.get(0)?,
                file_path,
                summary,
                match_type: match_type.to_string(),
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
