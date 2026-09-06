//! 代码符号索引（REQ-RAG-031 代码感知 RAG）

use anyhow::Context;
use echomind_models::{CodeSymbol, SymbolKind};
use rusqlite::params;

use super::super::migration::Pool;
use crate::sqlite_storage::{run_db, with_transaction};

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
    kind: Option<&SymbolKind>,
) -> anyhow::Result<Vec<CodeSymbol>> {
    let pool = pool.clone();
    let name = name.to_string();
    let kind_str = kind.map(SymbolKind::as_str).map(String::from);
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
