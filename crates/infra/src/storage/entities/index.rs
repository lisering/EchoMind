//! 实体索引（REQ-PERF-006 实体链接增强）

use std::path::Path;

use anyhow::Context;
use echomind_models::{Chunk, RetrievalResult};
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

/// 批量写入实体索引（INSERT OR IGNORE 避免重复实体）。
pub(crate) async fn add_entities(
    pool: &Pool,
    entities: &[(String, String, String)],
) -> anyhow::Result<()> {
    if entities.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let entities = entities.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut stmt = conn
                .prepare(
                    "INSERT OR IGNORE INTO entities (id, chunk_id, entity_text, entity_type) \
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .context("预编译 entities 批量写入语句失败")?;
            for (text, etype, chunk_id) in &entities {
                let id = uuid::Uuid::new_v4().to_string();
                stmt.execute(params![id, chunk_id, text, etype])
                    .context("批量写入 entities 失败")?;
            }
            Ok(())
        })
    })
    .await
}

/// 实体检索（精确匹配 entity_text，返回按命中数降序的 chunk 列表）。
pub(crate) async fn entity_search(
    pool: &Pool,
    query_entities: &[String],
    top_k: usize,
) -> anyhow::Result<Vec<RetrievalResult>> {
    if query_entities.is_empty() || top_k == 0 {
        return Ok(vec![]);
    }
    let pool = pool.clone();
    let entities = query_entities.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let placeholders: Vec<String> = (0..entities.len()).map(|_| "?".to_string()).collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, \
             d.file_path, COUNT(*) as entity_hit_count \
             FROM entities e \
             JOIN chunks c ON c.id = e.chunk_id \
             JOIN documents d ON d.id = c.doc_id \
             WHERE e.entity_text IN ({in_clause}) \
             GROUP BY c.id \
             ORDER BY entity_hit_count DESC \
             LIMIT ?"
        );
        let mut stmt = conn.prepare(&sql)?;
        let limit_val = top_k as i64;
        let params: Vec<&dyn rusqlite::ToSql> = entities
            .iter()
            .map(|e| e as &dyn rusqlite::ToSql)
            .chain(std::iter::once(&limit_val as &dyn rusqlite::ToSql))
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
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

/// 批量查询实体类型，返回 `HashMap<entity_text, entity_type>`。
pub(crate) async fn get_entity_types(
    pool: &Pool,
    entities: &[String],
) -> anyhow::Result<std::collections::HashMap<String, String>> {
    if entities.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let pool = pool.clone();
    let entities = entities.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let placeholders: Vec<String> = (0..entities.len()).map(|_| "?".to_string()).collect();
        let in_clause = placeholders.join(", ");
        let sql = format!(
            "SELECT DISTINCT entity_text, entity_type FROM entities WHERE entity_text IN ({in_clause})"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = entities
            .iter()
            .map(|e| e as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut result: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for row in rows {
            let (text, etype) = row?;
            result.insert(text, etype);
        }
        Ok(result)
    })
    .await
}

/// 获取全量实体邻接表（无向图，用于 GraphAnalyzer 高级分析）。
pub(crate) async fn get_entity_graph(
    pool: &Pool,
) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare("SELECT subject, object FROM entity_relations")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut adjacency: std::collections::HashMap<String, std::collections::HashSet<String>> =
            std::collections::HashMap::new();
        for row in rows {
            let (subject, object) = row?;
            adjacency
                .entry(subject.clone())
                .or_default()
                .insert(object.clone());
            adjacency.entry(object).or_default().insert(subject);
        }

        let result: std::collections::HashMap<String, Vec<String>> = adjacency
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();
        Ok(result)
    })
    .await
}
