//! 消息表 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S02 拆分：消息写入、列表/分页、批量删除、编辑分页（turn_group 升级）、
//! 活跃版本管理、安全标记、FTS5 全文搜索。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

use anyhow::Context;
use echomind_models::{ChatMessage, MessageSearchResult, TurnActiveVersion};
use rusqlite::params;

use crate::sqlite_storage::{build_fts5_or_query, run_db, with_transaction};

// ============================================================================
// 消息 CRUD
// ============================================================================

/// 写入消息行（INSERT，自动生成 UUID + 时间戳）。
pub(crate) async fn add_message(
    pool: &super::migration::Pool,
    conversation_id: String,
    message: ChatMessage,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let msg = message;
    run_db(move || {
        let sources_json = match &msg.sources {
            Some(sources) => Some(serde_json::to_string(sources).context("序列化引用来源失败")?),
            None => None,
        };
        let reasoning = msg.reasoning.clone();
        let turn_group = msg.turn_group.clone().unwrap_or_default();
        let version = msg.version.unwrap_or(1);
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, sources, reasoning, turn_group, version, created_at, security_tainted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                uuid::Uuid::new_v4().to_string(),
                conversation_id,
                msg.role,
                msg.content,
                sources_json,
                reasoning,
                turn_group,
                version,
                chrono::Utc::now().timestamp(),
                0,
            ],
        )
        .context("写入消息失败")?;
        Ok(())
    })
    .await
}

/// 列出会话全部消息（按 rowid ASC）。
pub(crate) async fn list_messages(
    pool: &super::migration::Pool,
    conversation_id: String,
) -> anyhow::Result<Vec<ChatMessage>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, role, content, sources, reasoning, turn_group, version FROM messages WHERE conversation_id = ?1 ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            let sources_json: Option<String> = row.get(3)?;
            let reasoning: Option<String> = row.get(4)?;
            let turn_group: Option<String> = row.get(5)?;
            let version: Option<i32> = row.get(6)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                sources_json,
                reasoning,
                turn_group,
                version,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, role, content, sources_json, reasoning, turn_group, version) = row?;
            let sources = match sources_json {
                Some(json) => Some(
                    serde_json::from_str(&json).context("反序列化引用来源失败")?,
                ),
                None => None,
            };
            let tg = turn_group.filter(|s| !s.is_empty());
            let ver = if tg.is_some() { Some(version.unwrap_or(1)) } else { None };
            out.push(ChatMessage {
                id: Some(id),
                role,
                content,
                sources,
                reasoning,
                turn_group: tg,
                version: ver,
            });
        }
        Ok(out)
    })
    .await
}

/// 分页列出消息（从最新向前取，反转恢复正序）。
pub(crate) async fn list_messages_paginated(
    pool: &super::migration::Pool,
    conversation_id: String,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<ChatMessage>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, role, content, sources, reasoning, turn_group, version FROM messages WHERE conversation_id = ?1 ORDER BY rowid DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![conversation_id, limit as i64, offset as i64], |row| {
            let sources_json: Option<String> = row.get(3)?;
            let reasoning: Option<String> = row.get(4)?;
            let turn_group: Option<String> = row.get(5)?;
            let version: Option<i32> = row.get(6)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                sources_json,
                reasoning,
                turn_group,
                version,
            ))
        })?;
        #[allow(clippy::type_complexity)]
        let mut out: Vec<(String, String, String, Option<String>, Option<String>, Option<String>, Option<i32>)> = Vec::new();
        for row in rows {
            out.push(row?);
        }
        out.reverse();
        let mut messages = Vec::with_capacity(out.len());
        for (id, role, content, sources_json, reasoning, turn_group, version) in out {
            let sources = match sources_json {
                Some(json) => Some(
                    serde_json::from_str(&json).context("反序列化引用来源失败")?,
                ),
                None => None,
            };
            let tg = turn_group.filter(|s| !s.is_empty());
            let ver = if tg.is_some() { Some(version.unwrap_or(1)) } else { None };
            messages.push(ChatMessage {
                id: Some(id),
                role,
                content,
                sources,
                reasoning,
                turn_group: tg,
                version: ver,
            });
        }
        Ok(messages)
    })
    .await
}

/// 统计消息数。
pub(crate) async fn count_messages(
    pool: &super::migration::Pool,
    conversation_id: String,
) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .context("统计消息数失败")?;
        Ok(count as usize)
    })
    .await
}

/// 批量删除消息（WHERE id IN (...)）。
pub(crate) async fn delete_messages_by_ids(
    pool: &super::migration::Pool,
    conversation_id: String,
    message_ids: Vec<String>,
) -> anyhow::Result<usize> {
    if message_ids.is_empty() {
        return Ok(0);
    }
    let pool = pool.clone();
    let ids = message_ids;
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let placeholders: Vec<String> = (0..ids.len()).map(|i| format!("?{}", i + 2)).collect();
        let sql = format!(
            "DELETE FROM messages WHERE conversation_id = ?1 AND id IN ({})",
            placeholders.join(", ")
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::with_capacity(ids.len() + 1);
        params_vec.push(Box::new(conversation_id.clone()));
        for id in &ids {
            params_vec.push(Box::new(id.clone()));
        }
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        let deleted = conn
            .execute(&sql, param_refs.as_slice())
            .context("批量删除消息失败")?;
        Ok(deleted)
    })
    .await
}

// ============================================================================
// 编辑分页（turn_group 升级）
// ============================================================================

/// 首次编辑升级：把原始 user+assistant 行标记为 turn_group version=1。
pub(crate) async fn promote_original_turn(
    pool: &super::migration::Pool,
    conversation_id: String,
    original_message_id: String,
    turn_group: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        with_transaction(&conn, |conn| {
            let user_updated = conn.execute(
                "UPDATE messages SET turn_group = ?1, version = 1 \
WHERE id = ?2 AND conversation_id = ?3 AND role = 'user' \
AND (turn_group = '' OR turn_group = ?1)",
                params![turn_group, original_message_id, conversation_id],
            )?;
            if user_updated == 0 {
                anyhow::bail!(
                    "原始消息行不存在或已被其他 turn_group 占用: id={original_message_id}"
                );
            }
            conn.execute(
                "UPDATE messages SET turn_group = ?1, version = 1 \
WHERE id = (SELECT id FROM messages WHERE conversation_id = ?2 \
AND role = 'assistant' \
AND (turn_group = '' OR turn_group = ?1) \
AND rowid > (SELECT rowid FROM messages WHERE id = ?3) \
AND rowid < COALESCE((SELECT MIN(rowid) FROM messages WHERE conversation_id = ?2 \
AND role = 'user' AND turn_group = '' \
AND rowid > (SELECT rowid FROM messages WHERE id = ?3)), 9223372036854775807) \
ORDER BY rowid ASC LIMIT 1)",
                params![turn_group, conversation_id, original_message_id],
            )?;
            Ok(())
        })
    })
    .await
}

/// 设置轮次活跃版本（UPSERT）。
pub(crate) async fn set_turn_active_version(
    pool: &super::migration::Pool,
    conversation_id: String,
    turn_group: String,
    active_version: i32,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO turn_active_versions (conversation_id, turn_group, active_version) \
VALUES (?1, ?2, ?3) \
ON CONFLICT(conversation_id, turn_group) DO UPDATE SET active_version = ?3",
            params![conversation_id, turn_group, active_version],
        )
        .context("设置轮次活跃版本失败")?;
        Ok(())
    })
    .await
}

/// 获取会话的全部活跃版本。
pub(crate) async fn get_turn_active_versions(
    pool: &super::migration::Pool,
    conversation_id: String,
) -> anyhow::Result<Vec<TurnActiveVersion>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT turn_group, active_version FROM turn_active_versions WHERE conversation_id = ?1",
        )?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(TurnActiveVersion {
                turn_group: row.get(0)?,
                active_version: row.get(1)?,
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

// ============================================================================
// 安全标记
// ============================================================================

/// 设置消息的 security_tainted 标记。
pub(crate) async fn set_entry_security_tainted(
    pool: &super::migration::Pool,
    message_id: String,
    tainted: bool,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "UPDATE messages SET security_tainted = ?1 WHERE id = ?2",
            params![if tainted { 1 } else { 0 }, &message_id],
        )
        .context("设置 security_tainted 标记失败")?;
        Ok(())
    })
    .await
}

/// 查询消息的 security_tainted 标记。
pub(crate) async fn get_entry_security_tainted(
    pool: &super::migration::Pool,
    message_id: String,
) -> anyhow::Result<bool> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let tainted: i64 = conn
            .query_row(
                "SELECT security_tainted FROM messages WHERE id = ?1",
                params![&message_id],
                |row| row.get(0),
            )
            .context("查询 security_tainted 标记失败")?;
        Ok(tainted != 0)
    })
    .await
}

// ============================================================================
// FTS5 全文搜索
// ============================================================================

/// 全文搜索消息（FTS5 + LIKE 回退）。
pub(crate) async fn search_messages(
    pool: &super::migration::Pool,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<MessageSearchResult>> {
    let trimmed = query.trim();
    if trimmed.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }
    let pool = pool.clone();
    let query = trimmed.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;

        // 短查询回退：trigram 分词器要求 ≥3 字符
        if query.chars().count() < 3 {
            let pattern = format!("%{query}%");
            let mut stmt = conn.prepare(
                "SELECT m.id, m.conversation_id, m.role, m.content, m.created_at,
                        COALESCE(c.title, '')
                 FROM messages m
                 LEFT JOIN conversations c ON c.id = m.conversation_id
                 WHERE m.content LIKE ?1
                 ORDER BY m.created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![pattern, limit as i64], |row| {
                Ok(MessageSearchResult {
                    message_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    conversation_title: row.get(5)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    score: 1.0,
                    created_at: row.get(4)?,
                })
            })?;
            let mut results = Vec::new();
            for row in rows {
                results.push(row?);
            }
            return Ok(results);
        }

        // FTS5 分词查询
        let fts_query = build_fts5_or_query(&query);
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = conn.prepare(
            "SELECT m.id, m.conversation_id, m.role, m.content, m.created_at,
                    COALESCE(c.title, ''), bm25(messages_fts)
             FROM messages_fts fts
             JOIN messages m ON m.id = fts.message_id
             LEFT JOIN conversations c ON c.id = m.conversation_id
             WHERE messages_fts MATCH ?1
             ORDER BY bm25(messages_fts)
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
            let score_val: f64 = row.get(6)?;
            Ok(MessageSearchResult {
                message_id: row.get(0)?,
                conversation_id: row.get(1)?,
                conversation_title: row.get(5)?,
                role: row.get(2)?,
                content: row.get(3)?,
                score: -score_val,
                created_at: row.get(4)?,
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
