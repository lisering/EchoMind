//! 全局搜索（REQ-IX-008）

use anyhow::Context;
use echomind_models::EntitySearchResult;
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::run_db;

/// 全局搜索实体（entity_text LIKE 匹配，REQ-IX-008）。
///
/// 搜索 entities 表的 `entity_text` 列，返回去重后的实体列表。
/// 每个实体关联其所在的 chunk_id 和 doc_id。
pub(crate) async fn search_entities(
    pool: &Pool,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<EntitySearchResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let pool = pool.clone();
    let pattern = format!("%{trimmed}%");
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT e.entity_text, e.entity_type, e.chunk_id, c.doc_id \
             FROM entities e \
             JOIN chunks c ON c.id = e.chunk_id \
             WHERE e.entity_text LIKE ?1 \
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |row| {
            Ok(EntitySearchResult {
                entity_text: row.get(0)?,
                entity_type: row.get(1)?,
                chunk_id: row.get(2)?,
                doc_id: row.get(3)?,
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
