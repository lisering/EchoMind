//! 数据库 schema 迁移与初始化（S01 拆分自 `sqlite_storage.rs`）。
//!
//! 本模块负责：
//! - `init_schema`：创建表、索引、FTS5 虚拟表、审计日志表
//! - `backfill_fts_if_needed` / `backfill_messages_fts_if_needed`：旧库升级回填全文索引
//! - `migrate_schema`：增量迁移旧表列结构（绝不丢弃用户数据）
//! - `safe_migrate_schema`：安全迁移（REQ-DB-008）— 备份 → 迁移 → 完整性检查 → 恢复
//! - `validate_table_name` / `table_exists` / `has_column`：表名安全校验与 schema 检测

use anyhow::Context;
use rusqlite::params;
use std::path::Path;
use tracing::{error, info, warn};

use super::schema::{
    KNOWN_TABLES, SCHEMA_AUDIT_LOG, SCHEMA_FTS, SCHEMA_INDEXES, SCHEMA_MESSAGES_FTS, SCHEMA_TABLES,
};

/// 连接池类型别名（与 `SqliteStorage` 一致）。
pub(crate) type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

/// 初始化数据库 schema：表 → 安全迁移 → 索引 → FTS → 回填 → 审计日志 → 完整性检查。
///
/// 步骤顺序有严格依赖关系：
/// 1. 创建表（`IF NOT EXISTS` 不修改已有表结构）
/// 2. 安全迁移旧表 schema（REQ-DB-008：备份 → 迁移 → 完整性检查 → 恢复）
/// 3. 创建索引（此时所有列已保证存在）
/// 4. 创建 FTS5 虚拟表
/// 5. 回填 FTS5 索引（旧库升级场景）
/// 6. 创建审计日志表
/// 7. 迁移后完整性检查
pub(crate) fn init_schema(pool: &Pool, db_path: &Path) -> anyhow::Result<()> {
    let conn = pool.get().context("获取数据库连接失败")?;
    // 步骤 1：创建表（IF NOT EXISTS 不修改已有表结构）
    conn.execute_batch(SCHEMA_TABLES)
        .context("初始化数据库表结构失败")?;
    // 步骤 2：安全迁移旧表 schema（REQ-DB-008）
    // 先关闭此连接（drop），让 safe_migrate_schema 用自己的连接操作
    drop(conn);
    safe_migrate_schema(pool, db_path)?;
    let conn = pool.get().context("获取数据库连接失败")?;
    // 步骤 3：创建索引（此时所有列已保证存在）
    conn.execute_batch(SCHEMA_INDEXES)
        .context("初始化数据库索引失败")?;
    // 步骤 4：创建 FTS5 全文索引虚拟表（混合检索关键词通道）
    conn.execute_batch(SCHEMA_FTS)
        .context("初始化 FTS5 全文索引失败")?;
    // 步骤 5：迁移——若 chunks 表已有数据但 chunks_fts 为空，回填全文索引
    backfill_fts_if_needed(&conn)?;
    // 步骤 5b：创建对话全文搜索 FTS5 虚拟表 + 触发器（REQ-RAG-040）
    conn.execute_batch(SCHEMA_MESSAGES_FTS)
        .context("初始化对话 FTS5 全文索引失败")?;
    // 步骤 5c：迁移——若 messages 表已有数据但 messages_fts 为空，回填全文索引
    backfill_messages_fts_if_needed(&conn)?;
    // 步骤 6：创建审计日志表（防篡改哈希链）
    conn.execute_batch(SCHEMA_AUDIT_LOG)
        .context("初始化审计日志表失败")?;
    // 步骤 7：迁移后完整性检查（REQ-DB-008 AC-3）
    let integrity = run_integrity_check(pool)?;
    if integrity != "ok" {
        error!("迁移后完整性检查失败: {integrity}");
        anyhow::bail!("数据库迁移后完整性检查失败: {integrity}");
    }
    info!("迁移后完整性检查通过");
    Ok(())
}

/// 若 chunks 表已有数据但 chunks_fts 为空（旧数据库升级场景），回填全文索引。
///
/// 避免旧版用户升级后 FTS5 索引为空导致关键词搜索无效。
fn backfill_fts_if_needed(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let chunk_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .context("统计 chunks 数量失败")?;
    let fts_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks_fts", [], |row| row.get(0))
        .context("统计 chunks_fts 数量失败")?;
    if chunk_count > 0 && fts_count == 0 {
        info!("FTS5 回填：chunks 表有 {chunk_count} 条数据但 chunks_fts 为空，执行回填");
        // Contextual BM25（REQ-PERF-005）：FTS5 索引使用文档名前缀
        conn.execute_batch(
            "INSERT INTO chunks_fts (chunk_id, doc_id, content)
                 SELECT c.id, c.doc_id, 
                   '文档《' || 
                   COALESCE(substr(d.file_path, instr(replace(d.file_path, '\\', '/'), '/') + 1), d.file_path)
                   || '》：\n' || c.content
                 FROM chunks c
                 JOIN documents d ON d.id = c.doc_id;",
        )
        .context("FTS5 回填失败")?;
    }
    Ok(())
}

/// 若 messages 表已有数据但 messages_fts 为空（旧数据库升级场景），回填全文索引。
///
/// 避免旧版用户升级后 FTS5 索引为空导致对话搜索无效。
fn backfill_messages_fts_if_needed(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    let msg_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .context("统计 messages 数量失败")?;
    let fts_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages_fts", [], |row| row.get(0))
        .context("统计 messages_fts 数量失败")?;
    if msg_count > 0 && fts_count == 0 {
        info!("FTS5 回填：messages 表有 {msg_count} 条数据但 messages_fts 为空，执行回填");
        conn.execute_batch(
            "INSERT INTO messages_fts (message_id, conversation_id, content)
                 SELECT id, conversation_id, content FROM messages;",
        )
        .context("messages_fts 回填失败")?;
    }
    Ok(())
}

/// 旧数据库 schema 迁移：增量迁移，**绝不丢弃用户数据**。
///
/// 背景：`CREATE TABLE IF NOT EXISTS` 不会修改已有表的列结构。
/// 若用户从旧版本升级，表可能缺少新增列或列名变更。
///
/// 迁移策略：
/// - **简单加列**（documents.status_reason、messages.conversation_id）：
///   使用 `ALTER TABLE ADD COLUMN`，保留全部行数据。
/// - **列名变更/列移除**（embeddings.embedding→vector、chunks.session_id）：
///   使用 CREATE-COPY-DROP-RENAME 模式，先建新表、拷贝数据、删旧表、改名。
pub(crate) fn migrate_schema(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    // ── conversations 表：sort_order 列在 v1.16 新增（REQ-IX-002 拖拽排序）──
    if table_exists(conn, "conversations")? && !has_column(conn, "conversations", "sort_order")? {
        info!("schema 迁移：conversations 表缺少 sort_order 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute(
            "ALTER TABLE conversations ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .context("迁移失败：conversations 表添加 sort_order 列失败")?;
    }

    // ── documents 表：status_reason 列在 Phase 4+ 新增 ──
    // ALTER TABLE ADD COLUMN 安全，旧行 status_reason = NULL
    if table_exists(conn, "documents")? && !has_column(conn, "documents", "status_reason")? {
        info!("schema 迁移：documents 表缺少 status_reason 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute("ALTER TABLE documents ADD COLUMN status_reason TEXT", [])
            .context("迁移失败：documents 表添加 status_reason 列失败")?;
    }

    // ── chunks 表：旧版含 session_id 列，新版不含 ──
    // SQLite ALTER TABLE 不支持 DROP COLUMN（3.35.0 前），
    // 使用 CREATE-COPY-DROP-RENAME 模式保留分块数据
    if table_exists(conn, "chunks")? && has_column(conn, "chunks", "session_id")? {
        info!("schema 迁移：chunks 表含旧版 session_id 列，执行 CREATE-COPY-DROP-RENAME");
        conn.execute_batch(
            "CREATE TABLE chunks_new (
                    id TEXT PRIMARY KEY,
                    doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                    content TEXT NOT NULL,
                    token_count INTEGER NOT NULL,
                    sequence INTEGER NOT NULL
                );
                INSERT INTO chunks_new (id, doc_id, content, token_count, sequence)
                    SELECT id, doc_id, content, token_count, sequence FROM chunks;
                DROP TABLE chunks;
                ALTER TABLE chunks_new RENAME TO chunks;",
        )
        .context("迁移失败：chunks 表 schema 重建失败（数据已保留）")?;
    }

    // ── embeddings 表：旧版列名 embedding（非 vector），可能含 session_id/model_name ──
    // CREATE-COPY-DROP-RENAME，保留全部向量数据
    if table_exists(conn, "embeddings")?
        && !has_column(conn, "embeddings", "vector")?
        && has_column(conn, "embeddings", "embedding")?
    {
        info!(
            "schema 迁移：embeddings 表列名不匹配（embedding → vector），执行 CREATE-COPY-DROP-RENAME"
        );
        conn.execute_batch(
            "CREATE TABLE embeddings_new (
                    chunk_id TEXT PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
                    vector BLOB NOT NULL
                );
                INSERT INTO embeddings_new (chunk_id, vector)
                    SELECT chunk_id, embedding FROM embeddings;
                DROP TABLE embeddings;
                ALTER TABLE embeddings_new RENAME TO embeddings;",
        )
        .context("迁移失败：embeddings 表 schema 重建失败（向量数据已保留）")?;
    }

    // ── messages 表：conversation_id 列在 Phase 6 新增 ──
    // ALTER TABLE ADD COLUMN 添加为 nullable；旧消息 conversation_id = NULL
    // 应用层只通过 conversation_id 查询消息，NULL 的旧消息不会被返回
    if table_exists(conn, "messages")? && !has_column(conn, "messages", "conversation_id")? {
        info!("schema 迁移：messages 表缺少 conversation_id 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute("ALTER TABLE messages ADD COLUMN conversation_id TEXT", [])
            .context("迁移失败：messages 表添加 conversation_id 列失败")?;
    }

    // ── messages 表：reasoning 列（推理思考过程持久化，P2-1 之后新增）──
    // ALTER TABLE ADD COLUMN 添加为 nullable；旧消息 reasoning = NULL（无思考过程）
    if table_exists(conn, "messages")? && !has_column(conn, "messages", "reasoning")? {
        info!("schema 迁移：messages 表缺少 reasoning 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute("ALTER TABLE messages ADD COLUMN reasoning TEXT", [])
            .context("迁移失败：messages 表添加 reasoning 列失败")?;
    }

    // ── messages 表：turn_group + version 列（用户消息编辑版本持久化）──
    // ALTER TABLE ADD COLUMN 添加；旧消息 turn_group='' / version=1（视为无版本管理）
    if table_exists(conn, "messages")? && !has_column(conn, "messages", "turn_group")? {
        info!("schema 迁移：messages 表缺少 turn_group 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute(
            "ALTER TABLE messages ADD COLUMN turn_group TEXT NOT NULL DEFAULT ''",
            [],
        )
        .context("迁移失败：messages 表添加 turn_group 列失败")?;
    }
    if table_exists(conn, "messages")? && !has_column(conn, "messages", "version")? {
        info!("schema 迁移：messages 表缺少 version 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute(
            "ALTER TABLE messages ADD COLUMN version INTEGER NOT NULL DEFAULT 1",
            [],
        )
        .context("迁移失败：messages 表添加 version 列失败")?;
    }
    // 索引：按 (conversation_id, turn_group, version) 查询版本树
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_messages_turn ON messages(conversation_id, turn_group, version)",
        [],
    )
    .context("迁移失败：创建 idx_messages_turn 索引失败")?;

    // ── messages 表：security_tainted 列（Q05 安全态势分层）──
    // ALTER TABLE ADD COLUMN 添加；旧消息 security_tainted = 0（未标记）
    if table_exists(conn, "messages")? && !has_column(conn, "messages", "security_tainted")? {
        info!("schema 迁移：messages 表缺少 security_tainted 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute(
            "ALTER TABLE messages ADD COLUMN security_tainted INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .context("迁移失败：messages 表添加 security_tainted 列失败")?;
    }

    // ── documents 表：original_path 列在 REQ-SYNC-002 新增 ──
    // ALTER TABLE ADD COLUMN 添加为 nullable；旧文档 original_path = NULL
    if table_exists(conn, "documents")? && !has_column(conn, "documents", "original_path")? {
        info!("schema 迁移：documents 表缺少 original_path 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute("ALTER TABLE documents ADD COLUMN original_path TEXT", [])
            .context("迁移失败：documents 表添加 original_path 列失败")?;
    }

    // ── documents 表：domain 列在 REQ-VEC-013 新增 ──
    // ALTER TABLE ADD COLUMN 添加为 nullable；旧文档 domain = NULL（尚未分类）
    if table_exists(conn, "documents")? && !has_column(conn, "documents", "domain")? {
        info!("schema 迁移：documents 表缺少 domain 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute("ALTER TABLE documents ADD COLUMN domain TEXT", [])
            .context("迁移失败：documents 表添加 domain 列失败")?;
    }

    // ── documents 表：summary 列在 REQ-ING-019 新增 ──
    // ALTER TABLE ADD COLUMN 添加为 nullable；旧文档 summary = NULL（尚未生成摘要）
    if table_exists(conn, "documents")? && !has_column(conn, "documents", "summary")? {
        info!("schema 迁移：documents 表缺少 summary 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute("ALTER TABLE documents ADD COLUMN summary TEXT", [])
            .context("迁移失败：documents 表添加 summary 列失败")?;
    }

    // ── documents 表：tags 列在 REQ-ING-022 新增 ──
    // ALTER TABLE ADD COLUMN 添加；旧文档 tags = '[]'（空标签数组）
    if table_exists(conn, "documents")? && !has_column(conn, "documents", "tags")? {
        info!("schema 迁移：documents 表缺少 tags 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute(
            "ALTER TABLE documents ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .context("迁移失败：documents 表添加 tags 列失败")?;
    }

    // ── documents 表：workspace_id 列在 REQ-WS-001 新增 ──
    // ALTER TABLE ADD COLUMN 添加；旧文档 workspace_id = 'default'（默认工作空间）
    if table_exists(conn, "documents")? && !has_column(conn, "documents", "workspace_id")? {
        info!("schema 迁移：documents 表缺少 workspace_id 列，执行 ALTER TABLE ADD COLUMN");
        conn.execute(
            "ALTER TABLE documents ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default'",
            [],
        )
        .context("迁移失败：documents 表添加 workspace_id 列失败")?;
    }

    // ── workspaces 表：REQ-WS-001 多知识库元数据表 ──
    // 创建表（如不存在），并插入默认工作空间行（幂等）
    if !table_exists(conn, "workspaces")? {
        info!("schema 迁移：创建 workspaces 表");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspaces (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );",
        )
        .context("迁移失败：创建 workspaces 表失败")?;
    }
    // 插入默认工作空间（幂等，旧库已有 default 数据但无 workspaces 表行）
    conn.execute(
        "INSERT OR IGNORE INTO workspaces (id, name, created_at) VALUES (?1, ?2, ?3)",
        params!["default", "Default", chrono::Utc::now().timestamp()],
    )
    .context("迁移失败：插入默认工作空间失败")?;

    // ── documents 表：workspace_id 索引（REQ-WS-001 数据隔离查询加速）──
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_documents_workspace ON documents(workspace_id)",
        [],
    )
    .context("迁移失败：创建 idx_documents_workspace 索引失败")?;

    // ── entities 表：旧版 schema 可能缺少 chunk_id 列 ──
    if table_exists(conn, "entities")? && !has_column(conn, "entities", "chunk_id")? {
        info!("schema 迁移：entities 表 schema 不兼容（缺少 chunk_id 列），重建表");
        conn.execute("DROP TABLE IF EXISTS entities", [])
            .context("迁移失败：DROP TABLE entities 失败")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entities (
                    id TEXT PRIMARY KEY,
                    chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
                    entity_text TEXT NOT NULL,
                    entity_type TEXT NOT NULL
                );",
        )
        .context("迁移失败：重建 entities 表失败")?;
    }

    // ── propositions 表：旧版 schema 可能缺少 chunk_id 列 ──
    // propositions 是派生索引表（可从 chunk 重新分割），schema 不兼容时直接重建。
    if table_exists(conn, "propositions")? && !has_column(conn, "propositions", "chunk_id")? {
        info!("schema 迁移：propositions 表 schema 不兼容（缺少 chunk_id 列），重建表");
        conn.execute("DROP TABLE IF EXISTS propositions", [])
            .context("迁移失败：DROP TABLE propositions 失败")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS propositions (
                    id TEXT PRIMARY KEY,
                    chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
                    content TEXT NOT NULL,
                    embedding BLOB,
                    sequence INTEGER NOT NULL
                );",
        )
        .context("迁移失败：重建 propositions 表失败")?;
    }

    // ── summary_nodes 表：旧版 schema 可能缺少 doc_id 列 ──
    // summary_nodes 是派生索引表（可从 chunk 重新构建摘要树），schema 不兼容时直接重建。
    if table_exists(conn, "summary_nodes")? && !has_column(conn, "summary_nodes", "doc_id")? {
        info!("schema 迁移：summary_nodes 表 schema 不兼容（缺少 doc_id 列），重建表");
        conn.execute("DROP TABLE IF EXISTS summary_nodes", [])
            .context("迁移失败：DROP TABLE summary_nodes 失败")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS summary_nodes (
                    id TEXT PRIMARY KEY,
                    doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                    level INTEGER NOT NULL,
                    content TEXT NOT NULL,
                    child_ids TEXT NOT NULL,
                    embedding BLOB
                );",
        )
        .context("迁移失败：重建 summary_nodes 表失败")?;
    }

    Ok(())
}

/// 安全：表名白名单校验，防止 PRAGMA table_info SQL 注入。
fn validate_table_name(table: &str) -> anyhow::Result<()> {
    if !KNOWN_TABLES.contains(&table) {
        anyhow::bail!("未知表名（不在白名单中）: {table}");
    }
    Ok(())
}

/// 检查指定表是否存在。
pub(crate) fn table_exists(conn: &rusqlite::Connection, table: &str) -> anyhow::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// 检查指定表是否包含某列（基于 PRAGMA table_info）。
/// 安全：SQLite PRAGMA 不支持参数绑定，先用白名单校验表名防注入。
pub(crate) fn has_column(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
) -> anyhow::Result<bool> {
    validate_table_name(table)?;
    let sql = format!("PRAGMA table_info(\"{table}\")");
    let mut stmt = conn.prepare(&sql)?;
    let col_names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(col_names.iter().any(|c| c == column))
}

// ============================================================================
// 安全迁移函数（REQ-DB-008 数据库迁移安全防护）
// ============================================================================

/// 检查是否有需要迁移的旧 schema。
///
/// 返回 `true` 表示存在需要迁移的旧表结构，需要备份。
fn needs_migration(conn: &rusqlite::Connection) -> bool {
    // chunks 表含旧版 session_id 列
    if table_exists(conn, "chunks").unwrap_or(false)
        && has_column(conn, "chunks", "session_id").unwrap_or(false)
    {
        return true;
    }
    // embeddings 表列名不匹配（embedding → vector）
    if table_exists(conn, "embeddings").unwrap_or(false)
        && !has_column(conn, "embeddings", "vector").unwrap_or(false)
        && has_column(conn, "embeddings", "embedding").unwrap_or(false)
    {
        return true;
    }
    // entities 表缺少 chunk_id 列
    if table_exists(conn, "entities").unwrap_or(false)
        && !has_column(conn, "entities", "chunk_id").unwrap_or(false)
    {
        return true;
    }
    // propositions 表缺少 chunk_id 列
    if table_exists(conn, "propositions").unwrap_or(false)
        && !has_column(conn, "propositions", "chunk_id").unwrap_or(false)
    {
        return true;
    }
    // summary_nodes 表缺少 doc_id 列
    if table_exists(conn, "summary_nodes").unwrap_or(false)
        && !has_column(conn, "summary_nodes", "doc_id").unwrap_or(false)
    {
        return true;
    }
    false
}

/// 备份数据库文件（REQ-DB-008 AC-1）。
///
/// 在迁移前将数据库文件复制为 `.bak` 后缀的备份文件。
/// 备份文件权限设为 0600（Unix）。
/// 备份失败时返回 `Err`，但调用方 `safe_migrate_schema` 会降级为无备份模式。
pub(crate) fn backup_database_file(db_path: &Path) -> anyhow::Result<()> {
    let bak_path = db_path.with_extension("db.bak");
    info!(
        "迁移安全：备份数据库文件 {} → {}",
        db_path.display(),
        bak_path.display()
    );
    std::fs::copy(db_path, &bak_path).context("备份数据库文件失败")?;

    // 设置备份文件权限（Unix 0600）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&bak_path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

/// 执行 `PRAGMA integrity_check` 并返回结果字符串（REQ-DB-008 AC-3）。
///
/// 返回 `"ok"` 表示数据库完整性正常。
pub(crate) fn run_integrity_check(pool: &Pool) -> anyhow::Result<String> {
    let conn = pool.get().context("获取数据库连接失败")?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("执行 PRAGMA integrity_check 失败")?;
    Ok(result)
}

/// 从备份恢复数据库文件（REQ-DB-008 AC-4）。
///
/// 将 `.bak` 备份文件覆盖回数据库文件。
/// 恢复后删除备份文件。
pub(crate) fn restore_from_backup(db_path: &Path) -> anyhow::Result<()> {
    let bak_path = db_path.with_extension("db.bak");
    error!(
        "迁移安全：从备份恢复数据库 {} → {}",
        bak_path.display(),
        db_path.display()
    );
    if !bak_path.exists() {
        anyhow::bail!("备份文件不存在: {}", bak_path.display());
    }
    std::fs::copy(&bak_path, db_path).context("从备份恢复数据库失败")?;
    // 清理备份文件
    let _ = std::fs::remove_file(&bak_path);
    Ok(())
}

/// 清理备份文件（REQ-DB-008 AC-5）。
///
/// 迁移成功后删除 `.bak` 备份文件。
pub(crate) fn cleanup_backup(db_path: &Path) {
    let bak_path = db_path.with_extension("db.bak");
    if bak_path.exists() {
        info!("迁移安全：清理备份文件 {}", bak_path.display());
        let _ = std::fs::remove_file(&bak_path);
    }
}

/// 安全迁移：备份 → 迁移 → 完整性检查 → 恢复/清理（REQ-DB-008）。
///
/// 1. 检查是否有需要迁移的旧 schema（无迁移则跳过备份，AC-7）
/// 2. 备份数据库文件（AC-1）
/// 3. 执行 `migrate_schema`（AC-2，DDL 在 SQLite 中自动事务）
/// 4. 执行 `PRAGMA integrity_check`（AC-3）
/// 5. 完整性检查失败 → 从备份恢复（AC-4）
/// 6. 完整性检查通过 → 清理备份（AC-5）
///
/// 备份失败时降级为无备份模式（AC-9），仅 warning 日志不阻塞迁移。
pub(crate) fn safe_migrate_schema(pool: &Pool, db_path: &Path) -> anyhow::Result<()> {
    // 检查是否有需要迁移的旧 schema
    {
        let conn = pool.get().context("获取数据库连接失败")?;
        if !needs_migration(&conn) {
            info!("迁移安全：无需迁移，跳过备份");
            return Ok(());
        }
    }

    // AC-1: 备份数据库文件
    let backup_result = backup_database_file(db_path);
    let has_backup = if let Err(ref e) = backup_result {
        // AC-9: 备份失败降级为无备份模式
        warn!("迁移安全：备份失败，降级为无备份模式: {e}");
        false
    } else {
        info!("迁移安全：备份成功");
        true
    };

    // AC-2: 执行迁移（DDL 在 SQLite 中自动包裹在隐式事务中）
    info!("迁移安全：执行 schema 迁移");
    {
        let conn = pool.get().context("获取数据库连接失败")?;
        migrate_schema(&conn)?;
    }

    // AC-3: 迁移后完整性检查
    info!("迁移安全：执行完整性检查");
    let integrity = run_integrity_check(pool)?;

    if integrity == "ok" {
        // 完整性检查通过
        info!("迁移安全：完整性检查通过");
        if has_backup {
            // AC-5: 清理备份
            cleanup_backup(db_path);
        }
        Ok(())
    } else {
        // AC-4: 完整性检查失败，从备份恢复
        error!("迁移安全：完整性检查失败: {integrity}");
        if has_backup {
            info!("迁移安全：从备份恢复数据库");
            restore_from_backup(db_path)?;
            anyhow::bail!("数据库迁移后完整性检查失败，已从备份恢复: {integrity}");
        } else {
            anyhow::bail!("数据库迁移后完整性检查失败（无备份可用）: {integrity}");
        }
    }
}
