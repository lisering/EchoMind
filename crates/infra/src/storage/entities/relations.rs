//! 实体关系图谱（REQ-RAG-026 知识图谱实体关系检索）

use anyhow::Context;
use echomind_models::EntityRelation;
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

/// 写入单条实体关系。
pub(crate) async fn add_relation(pool: &Pool, relation: &EntityRelation) -> anyhow::Result<()> {
    let pool = pool.clone();
    let rel = relation.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR IGNORE INTO entity_relations (id, subject, relation_type, object, chunk_id, confidence) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![rel.id, rel.subject, rel.relation_type, rel.object, rel.chunk_id, rel.confidence],
        )
        .context("写入 entity_relation 失败")?;
        Ok(())
    })
    .await
}

/// 批量写入实体关系。
pub(crate) async fn add_relations_batch(
    pool: &Pool,
    relations: &[EntityRelation],
) -> anyhow::Result<()> {
    if relations.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let rels = relations.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut stmt = conn
                .prepare(
                    "INSERT OR IGNORE INTO entity_relations (id, subject, relation_type, object, chunk_id, confidence) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .context("预编译 entity_relations 批量写入语句失败")?;
            for rel in &rels {
                stmt.execute(params![
                    rel.id,
                    rel.subject,
                    rel.relation_type,
                    rel.object,
                    rel.chunk_id,
                    rel.confidence,
                ])
                .context("批量写入 entity_relations 失败")?;
            }
            Ok(())
        })
    })
    .await
}

/// 查询指定实体参与的所有关系（subject 或 object 等于 entity_text）。
pub(crate) async fn get_relations_for_entity(
    pool: &Pool,
    entity_text: &str,
) -> anyhow::Result<Vec<EntityRelation>> {
    let pool = pool.clone();
    let entity = entity_text.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, subject, relation_type, object, chunk_id, confidence \
             FROM entity_relations \
             WHERE subject = ?1 OR object = ?1",
        )?;
        let rows = stmt.query_map(params![entity], |row| {
            Ok(EntityRelation {
                id: row.get(0)?,
                subject: row.get(1)?,
                relation_type: row.get(2)?,
                object: row.get(3)?,
                chunk_id: row.get(4)?,
                confidence: row.get(5)?,
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

/// 查询指定 chunk 的所有关系。
pub(crate) async fn get_relations_for_chunk(
    pool: &Pool,
    chunk_id: &str,
) -> anyhow::Result<Vec<EntityRelation>> {
    let pool = pool.clone();
    let chunk_id = chunk_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, subject, relation_type, object, chunk_id, confidence \
             FROM entity_relations \
             WHERE chunk_id = ?1",
        )?;
        let rows = stmt.query_map(params![chunk_id], |row| {
            Ok(EntityRelation {
                id: row.get(0)?,
                subject: row.get(1)?,
                relation_type: row.get(2)?,
                object: row.get(3)?,
                chunk_id: row.get(4)?,
                confidence: row.get(5)?,
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

/// 按主体 + 关系类型查询关系。
pub(crate) async fn search_by_relation(
    pool: &Pool,
    subject: &str,
    relation_type: &str,
) -> anyhow::Result<Vec<EntityRelation>> {
    let pool = pool.clone();
    let subject = subject.to_string();
    let rel_type = relation_type.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, subject, relation_type, object, chunk_id, confidence \
             FROM entity_relations \
             WHERE subject = ?1 AND relation_type = ?2",
        )?;
        let rows = stmt.query_map(params![subject, rel_type], |row| {
            Ok(EntityRelation {
                id: row.get(0)?,
                subject: row.get(1)?,
                relation_type: row.get(2)?,
                object: row.get(3)?,
                chunk_id: row.get(4)?,
                confidence: row.get(5)?,
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

/// 分页查询全部实体关系。
pub(crate) async fn list_all_relations(
    pool: &Pool,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<EntityRelation>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, subject, relation_type, object, chunk_id, confidence \
             FROM entity_relations \
             ORDER BY id \
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
            Ok(EntityRelation {
                id: row.get(0)?,
                subject: row.get(1)?,
                relation_type: row.get(2)?,
                object: row.get(3)?,
                chunk_id: row.get(4)?,
                confidence: row.get(5)?,
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

/// 统计实体关系总数。
pub(crate) async fn count_relations(pool: &Pool) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM entity_relations", [], |row| {
            row.get(0)
        })?;
        Ok(count as usize)
    })
    .await
}
