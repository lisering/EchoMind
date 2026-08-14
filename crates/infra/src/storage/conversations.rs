//! 会话与工作空间表 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S02 拆分：会话创建/列表/分页/删除/标题更新/排序、
//! 工作空间创建/列表/重命名/删除/统计、对话书签管理。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

use anyhow::Context;
use echomind_models::{Conversation, Workspace, WorkspaceStats};
use rusqlite::params;

use crate::sqlite_storage::run_db;

// ============================================================================
// 会话 CRUD
// ============================================================================

/// 创建会话（INSERT OR IGNORE）。
pub(crate) async fn create_conversation(
    pool: &super::migration::Pool,
    conversation: Conversation,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let c = conversation;
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR IGNORE INTO conversations (id, workspace_id, title, created_at, sort_order) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![c.id, c.workspace_id, c.title, c.created_at, c.sort_order],
        )
        .context("写入会话失败")?;
        Ok(())
    })
    .await
}

/// 列出工作空间下全部会话（按 sort_order ASC, created_at DESC）。
pub(crate) async fn list_conversations(
    pool: &super::migration::Pool,
    workspace_id: String,
) -> anyhow::Result<Vec<Conversation>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, title, created_at, sort_order FROM conversations WHERE workspace_id = ?1 ORDER BY sort_order ASC, created_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map(params![workspace_id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                sort_order: row.get(4)?,
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

/// 分页列出会话。
pub(crate) async fn list_conversations_paginated(
    pool: &super::migration::Pool,
    workspace_id: String,
    limit: usize,
    offset: usize,
) -> anyhow::Result<Vec<Conversation>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, title, created_at, sort_order FROM conversations WHERE workspace_id = ?1 ORDER BY sort_order ASC, created_at DESC, rowid DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![workspace_id, limit as i64, offset as i64], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                sort_order: row.get(4)?,
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

/// 统计工作空间会话数。
pub(crate) async fn count_conversations(
    pool: &super::migration::Pool,
    workspace_id: String,
) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE workspace_id = ?1",
                params![workspace_id],
                |row| row.get(0),
            )
            .context("统计会话数失败")?;
        Ok(count as usize)
    })
    .await
}

/// 删除会话（外键级联清理 messages）。
pub(crate) async fn delete_conversation(
    pool: &super::migration::Pool,
    id: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])
            .context("删除会话失败")?;
        Ok(())
    })
    .await
}

/// 按 ID 查找单个会话。
pub(crate) async fn get_conversation(
    pool: &super::migration::Pool,
    id: String,
) -> anyhow::Result<Option<Conversation>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, workspace_id, title, created_at, sort_order FROM conversations WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                workspace_id: row.get(1)?,
                title: row.get(2)?,
                created_at: row.get(3)?,
                sort_order: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    })
    .await
}

/// 更新会话标题。
pub(crate) async fn update_conversation_title(
    pool: &super::migration::Pool,
    id: String,
    title: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "UPDATE conversations SET title = ?1 WHERE id = ?2",
            params![title, id],
        )
        .context("更新会话标题失败")?;
        Ok(())
    })
    .await
}

/// 批量重排会话顺序。
pub(crate) async fn reorder_conversations(
    pool: &super::migration::Pool,
    ordered_ids: Vec<String>,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let ids: Vec<(String, i64)> = ordered_ids
        .iter()
        .enumerate()
        .map(|(idx, id)| (id.clone(), (idx as i64) + 1))
        .collect();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let tx = conn.unchecked_transaction()?;
        for (id, sort_order) in &ids {
            tx.execute(
                "UPDATE conversations SET sort_order = ?1 WHERE id = ?2",
                params![sort_order, id],
            )
            .context("更新会话排序失败")?;
        }
        tx.commit().context("提交会话排序事务失败")?;
        Ok(())
    })
    .await
}

// ============================================================================
// 工作空间管理
// ============================================================================

/// 创建工作空间（INSERT OR IGNORE）。
pub(crate) async fn create_workspace(
    pool: &super::migration::Pool,
    workspace: Workspace,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let ws = workspace;
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR IGNORE INTO workspaces (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![ws.id, ws.name, ws.created_at],
        )
        .context("写入工作空间失败")?;
        Ok(())
    })
    .await
}

/// 列出全部工作空间。
pub(crate) async fn list_workspaces(
    pool: &super::migration::Pool,
) -> anyhow::Result<Vec<Workspace>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at FROM workspaces ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Workspace {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
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

/// 重命名工作空间。
pub(crate) async fn rename_workspace(
    pool: &super::migration::Pool,
    id: String,
    name: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let affected = conn
            .execute(
                "UPDATE workspaces SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .context("重命名工作空间失败")?;
        if affected == 0 {
            anyhow::bail!("工作空间不存在: {id}");
        }
        Ok(())
    })
    .await
}

/// 删除工作空间（事务级联清理文档 + 会话 + 元数据）。
pub(crate) async fn delete_workspace(
    pool: &super::migration::Pool,
    id: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM documents WHERE workspace_id = ?1",
            params![&id],
        )
        .context("删除工作空间文档失败")?;
        tx.execute(
            "DELETE FROM conversations WHERE workspace_id = ?1",
            params![&id],
        )
        .context("删除工作空间会话失败")?;
        tx.execute("DELETE FROM workspaces WHERE id = ?1", params![&id])
            .context("删除工作空间元数据失败")?;
        tx.commit().context("提交工作空间删除事务失败")?;
        Ok(())
    })
    .await
}

/// 获取工作空间统计（文档数 + 会话数）。
pub(crate) async fn get_workspace_stats(
    pool: &super::migration::Pool,
    id: String,
) -> anyhow::Result<WorkspaceStats> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let doc_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM documents WHERE workspace_id = ?1",
                params![&id],
                |row| row.get(0),
            )
            .context("统计工作空间文档数失败")?;
        let conv_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE workspace_id = ?1",
                params![&id],
                |row| row.get(0),
            )
            .context("统计工作空间会话数失败")?;
        Ok(WorkspaceStats {
            document_count: doc_count as usize,
            conversation_count: conv_count as usize,
        })
    })
    .await
}

// ============================================================================
// 对话书签（REQ-RAG-047）
// ============================================================================

/// 添加对话书签（INSERT OR REPLACE）。
pub(crate) async fn add_bookmark(
    pool: &super::migration::Pool,
    conversation_id: String,
    note: Option<String>,
    created_at: i64,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO conversation_bookmarks (conversation_id, note, created_at) VALUES (?1, ?2, ?3)",
            params![conversation_id, note, created_at],
        )
        .context("写入 conversation_bookmarks 失败")?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}

/// 移除对话书签。
pub(crate) async fn remove_bookmark(
    pool: &super::migration::Pool,
    conversation_id: String,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        conn.execute(
            "DELETE FROM conversation_bookmarks WHERE conversation_id = ?1",
            params![conversation_id],
        )
        .context("删除 conversation_bookmarks 失败")?;
        Ok::<_, anyhow::Error>(())
    })
    .await??;
    Ok(())
}

/// 列出全部书签（按创建时间降序）。
pub(crate) async fn list_bookmarks(
    pool: &super::migration::Pool,
) -> anyhow::Result<Vec<echomind_models::ConversationBookmark>> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT conversation_id, note, created_at FROM conversation_bookmarks ORDER BY created_at DESC",
        )
        .context("准备 conversation_bookmarks 查询失败")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(echomind_models::ConversationBookmark {
                    conversation_id: row.get(0)?,
                    note: row.get(1)?,
                    created_at: row.get(2)?,
                })
            })
            .context("查询 conversation_bookmarks 失败")?;
        let mut bookmarks = Vec::new();
        for row in rows {
            bookmarks.push(row?);
        }
        Ok::<_, anyhow::Error>(bookmarks)
    })
    .await?
}

/// 检查指定会话是否已加书签。
pub(crate) async fn is_bookmarked(
    pool: &super::migration::Pool,
    conversation_id: String,
) -> anyhow::Result<bool> {
    let pool = pool.clone();
    let count: i64 = tokio::task::spawn_blocking(move || {
        let conn = pool.get()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM conversation_bookmarks WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .context("查询 conversation_bookmarks COUNT 失败")?;
        Ok::<_, anyhow::Error>(count)
    })
    .await??;
    Ok(count > 0)
}
