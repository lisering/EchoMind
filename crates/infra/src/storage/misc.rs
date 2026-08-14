//! 杂项表 CRUD 操作（从 `sqlite_storage.rs` 拆分）。
//!
//! v2.0 S03 拆分：Durable Prompt Admission、Scratch 日志、幂等性记录、
//! Session Todo、Budget 预算、对话记忆系统、检索记忆。
//!
//! 所有函数接收 `&Pool` 参数，不持有 `&self`，可见性 `pub(crate)`。

use anyhow::Context;
use echomind_models::{
    BudgetStats, MemoryEntry, MemoryTier, PendingInput, ScratchLogEntry, SessionTodo, TodoPriority,
    TodoStatus,
};
use rusqlite::OptionalExtension;
use rusqlite::params;

use super::migration::Pool;
use crate::sqlite_storage::row_to_memory_entry;
use crate::sqlite_storage::run_db;

// ============================================================================
// Durable Prompt Admission（B05 持久化提示接纳）
// ============================================================================

/// 接纳用户输入（B05）。
pub(crate) async fn admit_input(
    pool: &Pool,
    conversation_id: &str,
    content: &str,
    delivery: &str,
) -> anyhow::Result<String> {
    let input = PendingInput::new(
        conversation_id.to_string(),
        content.to_string(),
        delivery.to_string(),
    );
    let id = input.id.clone();
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO pending_inputs (id, conversation_id, content, delivery, created_at, promoted_seq) \
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
            params![input.id, input.conversation_id, input.content, input.delivery, input.created_at],
        )
        .context("写入 pending_inputs 失败")?;
        Ok(())
    })
    .await?;
    Ok(id)
}

/// 提升接纳记录为正式消息（B05）。
pub(crate) async fn promote_input(pool: &Pool, input_id: &str) -> anyhow::Result<()> {
    let pool = pool.clone();
    let input_id = input_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE pending_inputs SET promoted_seq = ?2 WHERE id = ?1",
            params![&input_id, now],
        )
        .context("提升 pending_inputs 失败")?;
        Ok(())
    })
    .await
}

/// 获取会话的待处理输入列表（B05）。
pub(crate) async fn get_pending_inputs(
    pool: &Pool,
    conversation_id: &str,
) -> anyhow::Result<Vec<PendingInput>> {
    let pool = pool.clone();
    let conversation_id = conversation_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, conversation_id, content, delivery, created_at, promoted_seq \
                 FROM pending_inputs WHERE conversation_id = ?1 AND promoted_seq IS NULL \
                 ORDER BY CASE delivery WHEN 'steer' THEN 0 ELSE 1 END, created_at ASC",
            )
            .context("准备待处理输入查询语句失败")?;
        let inputs = stmt
            .query_map(params![&conversation_id], |row| {
                Ok(PendingInput {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    content: row.get(2)?,
                    delivery: row.get(3)?,
                    created_at: row.get(4)?,
                    promoted_seq: row.get(5)?,
                })
            })
            .context("查询待处理输入失败")?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(inputs)
    })
    .await
}

// ============================================================================
// Scratch-Promote 记忆整合（Q01）
// ============================================================================

/// 追加一条 scratch 日志条目（Q01）。
pub(crate) async fn add_scratch_log(pool: &Pool, entry: &ScratchLogEntry) -> anyhow::Result<()> {
    let pool = pool.clone();
    let entry = entry.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO scratch_logs (id, date, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![entry.id, entry.date, entry.content, entry.created_at],
        )
        .context("写入 scratch_logs 失败")?;
        Ok(())
    })
    .await
}

/// 获取 scratch 日志条目列表（Q01）。
pub(crate) async fn get_scratch_logs(
    pool: &Pool,
    limit: Option<usize>,
) -> anyhow::Result<Vec<ScratchLogEntry>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let sql = match limit {
            Some(n) => format!(
                "SELECT id, date, content, created_at FROM scratch_logs ORDER BY created_at ASC LIMIT {n}"
            ),
            None =>
                "SELECT id, date, content, created_at FROM scratch_logs ORDER BY created_at ASC"
                    .to_string(),
        };
        let mut stmt = conn.prepare(&sql).context("准备 scratch_logs 查询失败")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ScratchLogEntry {
                    id: row.get(0)?,
                    date: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .context("查询 scratch_logs 失败")?;
        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    })
    .await
}

/// 删除指定的 scratch 日志条目（Q01）。
pub(crate) async fn delete_scratch_log(pool: &Pool, id: &str) -> anyhow::Result<()> {
    let pool = pool.clone();
    let id = id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM scratch_logs WHERE id = ?1", params![id])
            .context("删除 scratch_logs 失败")?;
        Ok(())
    })
    .await
}

/// 清理过期的 scratch 日志条目（Q01）。
pub(crate) async fn cleanup_expired_scratch_logs(
    pool: &Pool,
    before_timestamp: i64,
) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count = conn
            .execute(
                "DELETE FROM scratch_logs WHERE created_at < ?1",
                params![before_timestamp],
            )
            .context("清理过期 scratch_logs 失败")?;
        Ok(count)
    })
    .await
}

// ============================================================================
// 幂等性存储（Q07）
// ============================================================================

/// 记录幂等性操作（Q07）。
pub(crate) async fn record_idempotency(
    pool: &Pool,
    key: &str,
    timestamp: i64,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let key = key.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT OR REPLACE INTO idempotency_records (key, timestamp) VALUES (?1, ?2)",
            params![key, timestamp],
        )
        .context("写入 idempotency_records 失败")?;
        Ok(())
    })
    .await
}

/// 列出所有幂等性记录（Q07）。
pub(crate) async fn list_idempotency_records(pool: &Pool) -> anyhow::Result<Vec<(String, i64)>> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare("SELECT key, timestamp FROM idempotency_records ORDER BY timestamp DESC")
            .context("准备 idempotency_records 查询失败")?;
        let rows = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let timestamp: i64 = row.get(1)?;
                Ok((key, timestamp))
            })
            .context("查询 idempotency_records 失败")?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    })
    .await
}

/// 清理过期的幂等性记录（Q07）。
pub(crate) async fn cleanup_expired_idempotency(
    pool: &Pool,
    before_timestamp: i64,
) -> anyhow::Result<usize> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let count = conn
            .execute(
                "DELETE FROM idempotency_records WHERE timestamp < ?1",
                params![before_timestamp],
            )
            .context("清理过期 idempotency_records 失败")?;
        Ok(count)
    })
    .await
}

// ============================================================================
// Session Todo 持久化（B08）
// ============================================================================

/// 创建 Todo 项（B08）。
pub(crate) async fn add_session_todo(pool: &Pool, todo: &SessionTodo) -> anyhow::Result<()> {
    let pool = pool.clone();
    let todo = todo.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO session_todos (id, conversation_id, content, status, priority, position, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                todo.id,
                todo.conversation_id,
                todo.content,
                todo.status.as_str(),
                todo.priority.as_str(),
                todo.position,
                todo.created_at,
            ],
        )
        .context("写入 session_todos 失败")?;
        Ok(())
    })
    .await
}

/// 更新 Todo 状态（B08）。
pub(crate) async fn update_todo_status(
    pool: &Pool,
    todo_id: &str,
    status: &TodoStatus,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let todo_id = todo_id.to_string();
    let status_str = status.as_str().to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "UPDATE session_todos SET status = ?2 WHERE id = ?1",
            params![&todo_id, &status_str],
        )
        .context("更新 session_todos 状态失败")?;
        Ok(())
    })
    .await
}

/// 获取会话的 Todo 列表（B08）。
pub(crate) async fn get_session_todos(
    pool: &Pool,
    conversation_id: &str,
) -> anyhow::Result<Vec<SessionTodo>> {
    let pool = pool.clone();
    let conversation_id = conversation_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT id, conversation_id, content, status, priority, position, created_at \
                 FROM session_todos WHERE conversation_id = ?1 ORDER BY position ASC",
            )
            .context("准备 session_todos 查询语句失败")?;
        let todos = stmt
            .query_map(params![&conversation_id], |row| {
                let status_str: String = row.get(3)?;
                let priority_str: String = row.get(4)?;
                Ok(SessionTodo {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    content: row.get(2)?,
                    status: TodoStatus::from_db_str(&status_str).unwrap_or(TodoStatus::Pending),
                    priority: TodoPriority::from_db_str(&priority_str)
                        .unwrap_or(TodoPriority::Medium),
                    position: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .context("查询 session_todos 失败")?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(todos)
    })
    .await
}

/// 删除单个 Todo 项（B08）。
pub(crate) async fn delete_session_todo(pool: &Pool, todo_id: &str) -> anyhow::Result<()> {
    let pool = pool.clone();
    let todo_id = todo_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM session_todos WHERE id = ?1", params![&todo_id])
            .context("删除 session_todo 失败")?;
        Ok(())
    })
    .await
}

/// 删除会话的全部 Todo 项（B08）。
pub(crate) async fn delete_session_todos(pool: &Pool, conversation_id: &str) -> anyhow::Result<()> {
    let pool = pool.clone();
    let conversation_id = conversation_id.to_string();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "DELETE FROM session_todos WHERE conversation_id = ?1",
            params![&conversation_id],
        )
        .context("删除会话 session_todos 失败")?;
        Ok(())
    })
    .await
}

// ============================================================================
// Budget 预算追踪
// ============================================================================

/// 记录预算使用。
pub(crate) async fn record_budget_usage(
    pool: &Pool,
    principal: &str,
    input_tokens: usize,
    output_tokens: usize,
    cost_usd: f64,
    model_name: &str,
) -> anyhow::Result<()> {
    let pool = pool.clone();
    let principal = principal.to_string();
    let model_name = model_name.to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO budget_records (principal, timestamp, input_tokens, output_tokens, cost_usd, model_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![principal, now, input_tokens as i64, output_tokens as i64, cost_usd, model_name],
        )
        .context("记录预算使用失败")?;
        Ok(())
    })
    .await
}

/// 获取预算统计（daily_limit 由调用方从 settings 读取后传入）。
pub(crate) async fn get_budget_stats(
    pool: &Pool,
    principal: &str,
    daily_limit: f64,
) -> anyhow::Result<BudgetStats> {
    let pool = pool.clone();
    let principal = principal.to_string();

    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;

        let day_ago = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64 - 86400;

        let spent_today: f64 = conn
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM budget_records WHERE principal = ?1 AND timestamp > ?2",
                params![principal, day_ago],
                |row| row.get(0),
            )
            .context("查询预算统计失败")?;

        let remaining = if daily_limit > 0.0 {
            (daily_limit - spent_today).max(0.0)
        } else {
            f64::INFINITY
        };

        Ok(BudgetStats {
            daily_limit,
            spent_today,
            remaining,
        })
    })
    .await
}

// ============================================================================
// 对话记忆系统（REQ-RAG-032）
// ============================================================================

/// 写入对话记忆条目。
pub(crate) fn add_memory_entry(
    pool: &Pool,
    entry: &MemoryEntry,
) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
    let entry = entry.clone();
    let pool = pool.clone();
    async move {
        let tier = entry.tier.as_str().to_string();
        let source = entry.source.as_str().to_string();
        let conv_id = entry.conversation_id.clone();
        let id = entry.id.clone();
        let content = entry.content.clone();
        let created_at = entry.created_at;
        let last_accessed = entry.last_accessed;
        let access_count = entry.access_count as i64;
        let importance = entry.importance;
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "INSERT OR REPLACE INTO memory_entries \
                 (id, tier, content, source, conversation_id, created_at, last_accessed, access_count, importance) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![&id, &tier, &content, &source, &conv_id, created_at, last_accessed, access_count, importance],
            )
            .context("写入对话记忆失败")?;
            Ok(())
        })
        .await
        .context("对话记忆写入任务失败")?
    }
}

/// 获取对话记忆条目（可选 tier 过滤）。
pub(crate) fn get_memory_entries(
    pool: &Pool,
    tier: Option<&MemoryTier>,
) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
    let tier_str = tier.map(|t| t.as_str().to_string());
    let pool = pool.clone();
    async move {
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let entries = if let Some(ref tier) = tier_str {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, tier, content, source, conversation_id, created_at, \
                         last_accessed, access_count, importance \
                         FROM memory_entries WHERE tier = ?1 \
                         ORDER BY importance DESC, created_at DESC",
                    )
                    .context("准备查询语句失败")?;
                stmt.query_map(params![tier], row_to_memory_entry)
                    .context("查询对话记忆失败")?
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, tier, content, source, conversation_id, created_at, \
                         last_accessed, access_count, importance \
                         FROM memory_entries \
                         ORDER BY importance DESC, created_at DESC",
                    )
                    .context("准备查询语句失败")?;
                stmt.query_map([], row_to_memory_entry)
                    .context("查询对话记忆失败")?
                    .collect::<Result<Vec<_>, _>>()?
            };
            Ok(entries)
        })
        .await
        .context("对话记忆查询任务失败")?
    }
}

/// 更新对话记忆条目。
pub(crate) fn update_memory_entry(
    pool: &Pool,
    entry: &MemoryEntry,
) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
    let entry = entry.clone();
    let pool = pool.clone();
    async move {
        let tier = entry.tier.as_str().to_string();
        let last_accessed = entry.last_accessed;
        let access_count = entry.access_count as i64;
        let importance = entry.importance;
        let id = entry.id.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "UPDATE memory_entries SET tier = ?1, last_accessed = ?2, access_count = ?3, importance = ?4 \
                 WHERE id = ?5",
                params![&tier, last_accessed, access_count, importance, &id],
            )
            .context("更新对话记忆失败")?;
            Ok(())
        })
        .await
        .context("对话记忆更新任务失败")?
    }
}

/// 删除对话记忆条目。
pub(crate) fn delete_memory_entry(
    pool: &Pool,
    id: &str,
) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
    let id = id.to_string();
    let pool = pool.clone();
    async move {
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute("DELETE FROM memory_entries WHERE id = ?1", params![&id])
                .context("删除对话记忆失败")?;
            Ok(())
        })
        .await
        .context("对话记忆删除任务失败")?
    }
}

/// 清空对话记忆条目（可选 tier 过滤）。
pub(crate) fn clear_memory_entries(
    pool: &Pool,
    tier: Option<&MemoryTier>,
) -> impl std::future::Future<Output = anyhow::Result<usize>> + Send {
    let tier_str = tier.map(|t| t.as_str().to_string());
    let pool = pool.clone();
    async move {
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let deleted = if let Some(ref tier) = tier_str {
                conn.execute("DELETE FROM memory_entries WHERE tier = ?1", params![tier])
                    .context("清空对话记忆失败")?
            } else {
                conn.execute("DELETE FROM memory_entries", [])
                    .context("清空对话记忆失败")?
            };
            Ok(deleted)
        })
        .await
        .context("对话记忆清空任务失败")?
    }
}

/// 搜索对话记忆条目。
pub(crate) fn search_memory_entries(
    pool: &Pool,
    query: &str,
    limit: usize,
) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
    let query = format!("%{query}%");
    let limit = limit as i64;
    let pool = pool.clone();
    async move {
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn
                .prepare(
                    "SELECT id, tier, content, source, conversation_id, created_at, \
                     last_accessed, access_count, importance \
                     FROM memory_entries WHERE content LIKE ?1 \
                     ORDER BY importance DESC LIMIT ?2",
                )
                .context("准备搜索语句失败")?;
            let entries = stmt
                .query_map(params![&query, limit], row_to_memory_entry)
                .context("搜索对话记忆失败")?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(entries)
        })
        .await
        .context("对话记忆搜索任务失败")?
    }
}

// ============================================================================
// 检索记忆（REQ-PERF-012 自进化检索记忆）
// ============================================================================

/// 获取检索记忆记录。
pub(crate) async fn get_memory(
    pool: &Pool,
    query_type: &str,
    method: &str,
) -> anyhow::Result<Option<echomind_core::retrieval_memory::MemoryRecord>> {
    use echomind_core::retrieval_memory::{MemoryRecord, QueryType, RetrievalMethod};
    let qt = query_type.to_string();
    let m = method.to_string();
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT query_type, method, hit_count, miss_count, avg_score \
                 FROM retrieval_memory WHERE query_type = ?1 AND method = ?2",
            )
            .context("准备查询语句失败")?;
        let record = stmt
            .query_row(params![&qt, &m], |row| {
                let qt_str: String = row.get(0)?;
                let m_str: String = row.get(1)?;
                Ok(MemoryRecord {
                    query_type: QueryType::parse_str(&qt_str).unwrap_or(QueryType::Factual),
                    method: RetrievalMethod::parse_str(&m_str).unwrap_or(RetrievalMethod::Hybrid),
                    hit_count: row.get::<_, i64>(2)? as u32,
                    miss_count: row.get::<_, i64>(3)? as u32,
                    avg_score: row.get(4)?,
                })
            })
            .optional()
            .context("查询检索记忆失败")?;
        Ok(record)
    })
    .await
    .context("检索记忆查询任务失败")?
}

/// 写入/更新检索记忆记录。
pub(crate) async fn upsert_memory(
    pool: &Pool,
    record: &echomind_core::retrieval_memory::MemoryRecord,
) -> anyhow::Result<()> {
    let qt = record.query_type.as_str().to_string();
    let m = record.method.as_str().to_string();
    let hit = record.hit_count as i64;
    let miss = record.miss_count as i64;
    let avg = record.avg_score;
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute(
            "INSERT INTO retrieval_memory (query_type, method, hit_count, miss_count, avg_score) \
             VALUES (?1, ?2, ?3, ?4, ?5) \
             ON CONFLICT(query_type, method) DO UPDATE SET \
             hit_count = ?3, miss_count = ?4, avg_score = ?5",
            params![&qt, &m, hit, miss, avg],
        )
        .context("写入检索记忆失败")?;
        Ok(())
    })
    .await
    .context("检索记忆写入任务失败")?
}

/// 列出指定查询类型的检索记忆。
pub(crate) async fn list_memories(
    pool: &Pool,
    query_type: &str,
) -> anyhow::Result<Vec<echomind_core::retrieval_memory::MemoryRecord>> {
    use echomind_core::retrieval_memory::{MemoryRecord, QueryType, RetrievalMethod};
    let qt = query_type.to_string();
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT query_type, method, hit_count, miss_count, avg_score \
                 FROM retrieval_memory WHERE query_type = ?1",
            )
            .context("准备查询语句失败")?;
        let records = stmt
            .query_map(params![&qt], |row| {
                let qt_str: String = row.get(0)?;
                let m_str: String = row.get(1)?;
                Ok(MemoryRecord {
                    query_type: QueryType::parse_str(&qt_str).unwrap_or(QueryType::Factual),
                    method: RetrievalMethod::parse_str(&m_str).unwrap_or(RetrievalMethod::Hybrid),
                    hit_count: row.get::<_, i64>(2)? as u32,
                    miss_count: row.get::<_, i64>(3)? as u32,
                    avg_score: row.get(4)?,
                })
            })
            .context("查询检索记忆列表失败")?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    })
    .await
    .context("检索记忆列表任务失败")?
}

/// 列出全部检索记忆。
pub(crate) async fn list_all_memories(
    pool: &Pool,
) -> anyhow::Result<Vec<echomind_core::retrieval_memory::MemoryRecord>> {
    use echomind_core::retrieval_memory::{MemoryRecord, QueryType, RetrievalMethod};
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let mut stmt = conn
            .prepare(
                "SELECT query_type, method, hit_count, miss_count, avg_score \
                 FROM retrieval_memory",
            )
            .context("准备查询语句失败")?;
        let records = stmt
            .query_map([], |row| {
                let qt_str: String = row.get(0)?;
                let m_str: String = row.get(1)?;
                Ok(MemoryRecord {
                    query_type: QueryType::parse_str(&qt_str).unwrap_or(QueryType::Factual),
                    method: RetrievalMethod::parse_str(&m_str).unwrap_or(RetrievalMethod::Hybrid),
                    hit_count: row.get::<_, i64>(2)? as u32,
                    miss_count: row.get::<_, i64>(3)? as u32,
                    avg_score: row.get(4)?,
                })
            })
            .context("查询全部检索记忆失败")?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    })
    .await
    .context("检索记忆全部列表任务失败")?
}

/// 清空全部检索记忆。
pub(crate) async fn clear_all_memories(pool: &Pool) -> anyhow::Result<()> {
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        conn.execute("DELETE FROM retrieval_memory", [])
            .context("清空检索记忆失败")?;
        Ok(())
    })
    .await
    .context("检索记忆清空任务失败")?
}
