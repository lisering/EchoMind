//! 实体/关系/摘要/命题/wiki链接/代码符号 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S03 拆分：实体索引、实体检索、关系图谱、RAPTOR 摘要树、
//! Wiki 双向链接、代码符号索引。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

use std::path::Path;

use anyhow::Context;
use echomind_models::{
    Chunk, CodeSymbol, EntityRelation, EntitySearchResult, RetrievalResult, SymbolKind, WikiLink,
};
use rusqlite::params;

use super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

// ============================================================================
// 实体索引（REQ-PERF-006 实体链接增强）
// ============================================================================

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

// ============================================================================
// 实体关系图谱（REQ-RAG-026 知识图谱实体关系检索）
// ============================================================================

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

// ============================================================================
// Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接）
// ============================================================================

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

// ============================================================================
// 代码符号索引（REQ-RAG-031 代码感知 RAG）
// ============================================================================

/// 批量写入代码符号索引。
pub(crate) async fn add_symbols(pool: &Pool, symbols: &[CodeSymbol]) -> anyhow::Result<()> {
    if symbols.is_empty() {
        return Ok(());
    }
    let pool = pool.clone();
    let syms = symbols.to_vec();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let mut stmt = conn
                .prepare(
                    "INSERT OR REPLACE INTO code_symbols \
                     (id, chunk_id, name, kind, language, start_line, end_line, signature) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .context("预编译 code_symbols 批量写入语句失败")?;
            for sym in &syms {
                stmt.execute(params![
                    sym.id,
                    sym.chunk_id,
                    sym.name,
                    sym.kind.as_str(),
                    sym.language,
                    sym.start_line as i64,
                    sym.end_line as i64,
                    sym.signature
                ])
                .context("批量写入 code_symbols 失败")?;
            }
            Ok(())
        })
    })
    .await
}

/// 按符号名精确搜索（可选 kind 过滤）。
pub(crate) async fn search_by_symbol(
    pool: &Pool,
    name: &str,
    kind: Option<&echomind_models::SymbolKind>,
) -> anyhow::Result<Vec<CodeSymbol>> {
    let pool = pool.clone();
    let name = name.to_string();
    let kind_str = kind
        .map(echomind_models::SymbolKind::as_str)
        .map(String::from);
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let (sql, kind_param): (String, Option<String>) = match &kind_str {
            Some(k) => (
                "SELECT id, chunk_id, name, kind, language, start_line, end_line, signature \
                 FROM code_symbols WHERE name = ?1 AND kind = ?2"
                    .to_string(),
                Some(k.clone()),
            ),
            None => (
                "SELECT id, chunk_id, name, kind, language, start_line, end_line, signature \
                 FROM code_symbols WHERE name = ?1"
                    .to_string(),
                None,
            ),
        };
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![name], |row| {
            let kind_str: String = row.get(3)?;
            Ok(CodeSymbol {
                id: row.get(0)?,
                chunk_id: row.get(1)?,
                name: row.get(2)?,
                kind: SymbolKind::parse_str(&kind_str),
                language: row.get(4)?,
                start_line: row.get::<_, i64>(5)? as usize,
                end_line: row.get::<_, i64>(6)? as usize,
                signature: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            let sym = row?;
            if let Some(ref k) = kind_param
                && sym.kind.as_str() != k
            {
                continue;
            }
            result.push(sym);
        }
        Ok(result)
    })
    .await
}

/// 获取指定 chunk 的所有符号。
pub(crate) async fn get_symbols_for_chunk(
    pool: &Pool,
    chunk_id: &str,
) -> anyhow::Result<Vec<CodeSymbol>> {
    let pool = pool.clone();
    let chunk_id = chunk_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, chunk_id, name, kind, language, start_line, end_line, signature \
             FROM code_symbols WHERE chunk_id = ?1 ORDER BY start_line",
        )?;
        let rows = stmt.query_map(params![chunk_id], |row| {
            let kind_str: String = row.get(3)?;
            Ok(CodeSymbol {
                id: row.get(0)?,
                chunk_id: row.get(1)?,
                name: row.get(2)?,
                kind: SymbolKind::parse_str(&kind_str),
                language: row.get(4)?,
                start_line: row.get::<_, i64>(5)? as usize,
                end_line: row.get::<_, i64>(6)? as usize,
                signature: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    })
    .await
}

/// 模糊搜索符号（LIKE 匹配）。
pub(crate) async fn search_symbols_fuzzy(
    pool: &Pool,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<CodeSymbol>> {
    if query.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let pool = pool.clone();
    let pattern = format!("%{query}%");
    let limit = limit as i64;
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, chunk_id, name, kind, language, start_line, end_line, signature \
             FROM code_symbols WHERE name LIKE ?1 LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit], |row| {
            let kind_str: String = row.get(3)?;
            Ok(CodeSymbol {
                id: row.get(0)?,
                chunk_id: row.get(1)?,
                name: row.get(2)?,
                kind: SymbolKind::parse_str(&kind_str),
                language: row.get(4)?,
                start_line: row.get::<_, i64>(5)? as usize,
                end_line: row.get::<_, i64>(6)? as usize,
                signature: row.get(7)?,
            })
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    })
    .await
}

// ============================================================================
// 全局搜索（REQ-IX-008）
// ============================================================================

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
