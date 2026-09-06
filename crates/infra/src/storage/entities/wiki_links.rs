//! Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接）

use anyhow::Context;
use echomind_models::WikiLink;
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

/// 批量写入 wiki-link 索引。
pub(crate) async fn add_wiki_links(pool: &Pool, links: &[WikiLink]) -> anyhow::Result<()> {
    if links.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let links = links.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut stmt = conn
                .prepare(
                    "INSERT OR IGNORE INTO wiki_links (id, source_doc_id, target, chunk_id, created_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .context("预编译 wiki_links 批量写入语句失败")?;
            for link in &links {
                stmt.execute(params![link.id, link.source_doc_id, link.target, link.chunk_id, link.created_at])
                    .context("批量写入 wiki_links 失败")?;
            }
            Ok(())
        })
    })
    .await
}

/// 查询文档的正向链接。
pub(crate) async fn get_forward_links(pool: &Pool, doc_id: &str) -> anyhow::Result<Vec<WikiLink>> {
    let pool = pool.clone();
    let doc_id = doc_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, source_doc_id, target, chunk_id, created_at \
                 FROM wiki_links WHERE source_doc_id = ?1 ORDER BY created_at ASC",
            )
            .context("准备正向链接查询语句失败")?;
        let links = stmt
            .query_map(params![&doc_id], |row| {
                Ok(WikiLink {
                    id: row.get(0)?,
                    source_doc_id: row.get(1)?,
                    target: row.get(2)?,
                    chunk_id: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .context("查询正向链接失败")?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(links)
    })
    .await
}

/// 查询文档的反向链接。
pub(crate) async fn get_backlinks(pool: &Pool, doc_name: &str) -> anyhow::Result<Vec<WikiLink>> {
    let pool = pool.clone();
    let pattern = format!("%{doc_name}%");
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, source_doc_id, target, chunk_id, created_at \
                 FROM wiki_links WHERE target LIKE ?1 ORDER BY created_at DESC",
            )
            .context("准备反向链接查询语句失败")?;
        let links = stmt
            .query_map(params![&pattern], |row| {
                Ok(WikiLink {
                    id: row.get(0)?,
                    source_doc_id: row.get(1)?,
                    target: row.get(2)?,
                    chunk_id: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .context("查询反向链接失败")?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(links)
    })
    .await
}
