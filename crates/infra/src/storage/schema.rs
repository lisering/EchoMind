//! 数据库 schema 常量定义（S01 拆分自 `sqlite_storage.rs`）。
//!
//! 本模块集中管理所有 SQL DDL 常量：表结构、索引、FTS5 虚拟表、触发器、
//! 以及列名白名单和完整性检查结果枚举。
//! 常量按逻辑分组：PRAGMA → 表 → 索引 → FTS → 审计 → 加密常量。

// ============================================================================
// PRAGMA
// ============================================================================

/// 连接级 PRAGMA：WAL 模式 + 外键级联 + 写繁忙重试 + 性能调优（REQ-DB-001-AC-3）。
///
/// 性能参数说明（2026-07 全尺度优化）：
/// - `cache_size = -65536`：64MB page cache（默认仅 2MB），大幅减少大库查询的磁盘 I/O。
/// - `mmap_size = 268435456`：256MB 零拷贝内存映射，绕过 read() 系统调用。
/// - `wal_autocheckpoint = 10000`：WAL 达到 10000 pages（~40MB）才 checkpoint，
///   减少 GB 级导入时的 checkpoint 频率，提升写入吞吐。
/// - `temp_store = MEMORY`：临时表和排序结果放内存，加速 FTS5 查询和复杂 JOIN。
pub(crate) const PRAGMAS: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -65536;
PRAGMA mmap_size = 268435456;
PRAGMA wal_autocheckpoint = 10000;
PRAGMA temp_store = MEMORY;
";

// ============================================================================
// 表结构
// ============================================================================

/// 表结构：documents / chunks / embeddings / settings / conversations / messages。
/// 外键 ON DELETE CASCADE（REQ-ING-005 前置）。
/// 仅含 CREATE TABLE 语句；索引在迁移完成后单独创建（防旧表 schema 不兼容崩溃）。
pub(crate) const SCHEMA_TABLES: &str = "
CREATE TABLE IF NOT EXISTS documents (
id TEXT PRIMARY KEY,
file_path TEXT NOT NULL,
file_hash TEXT NOT NULL,
status TEXT NOT NULL,
status_reason TEXT,
created_at INTEGER NOT NULL,
original_path TEXT,
domain TEXT,
summary TEXT,
tags TEXT NOT NULL DEFAULT '[]',
workspace_id TEXT NOT NULL DEFAULT 'default'
);

CREATE TABLE IF NOT EXISTS workspaces (
id TEXT PRIMARY KEY,
name TEXT NOT NULL,
created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    token_count INTEGER NOT NULL,
    sequence INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS embeddings (
    chunk_id TEXT PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    vector BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    sources TEXT,
    reasoning TEXT,
    turn_group TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    security_tainted INTEGER NOT NULL DEFAULT 0
);

-- 嵌入缓存表（全尺度性能优化：按内容指纹去重，避免重复 ONNX 推理）
CREATE TABLE IF NOT EXISTS embeddings_cache (
    content_hash TEXT PRIMARY KEY,
    embedding BLOB NOT NULL
);

-- 实体索引表（REQ-PERF-006 实体链接增强）：三路 RRF 实体匹配通道
CREATE TABLE IF NOT EXISTS entities (
id TEXT PRIMARY KEY,
chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
entity_text TEXT NOT NULL,
entity_type TEXT NOT NULL
);

-- Proposition 索引表（REQ-PERF-007 Proposition 级原子分割）：
-- 将 chunk 分解为自包含的原子事实，proposition 级检索精度优于 chunk 级。
CREATE TABLE IF NOT EXISTS propositions (
id TEXT PRIMARY KEY,
chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
content TEXT NOT NULL,
embedding BLOB,
sequence INTEGER NOT NULL
);

-- RAPTOR 摘要树表（REQ-PERF-009 多级摘要树索引）：
-- 将原始 chunks 组织为多级摘要树，提供从局部事实到全局主题的多层次检索。
CREATE TABLE IF NOT EXISTS summary_nodes (
id TEXT PRIMARY KEY,
doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
level INTEGER NOT NULL,
content TEXT NOT NULL,
child_ids TEXT NOT NULL,
embedding BLOB
);

-- 检索记忆表（REQ-PERF-012 自进化检索记忆）：
-- 记录每种查询类型 × 检索方法的累计效果统计，自适应选择最佳策略。
CREATE TABLE IF NOT EXISTS retrieval_memory (
query_type TEXT NOT NULL,
method TEXT NOT NULL,
hit_count INTEGER DEFAULT 0,
miss_count INTEGER DEFAULT 0,
avg_score REAL DEFAULT 0,
PRIMARY KEY (query_type, method)
);

-- 实体关系图表（REQ-RAG-026 知识图谱实体关系检索）：
-- 存储实体间的有向关系边，供图遍历检索使用。
CREATE TABLE IF NOT EXISTS entity_relations (
id TEXT PRIMARY KEY,
subject TEXT NOT NULL,
relation_type TEXT NOT NULL,
object TEXT NOT NULL,
chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
confidence REAL NOT NULL
);

-- 轮次活跃版本表（分支切换状态持久化）：
-- 用户在分页器中切换查看不同编辑版本时，活跃版本号被持久化，
-- 下次加载会话时恢复到最后一次查看的版本。
CREATE TABLE IF NOT EXISTS turn_active_versions (
conversation_id TEXT NOT NULL,
turn_group TEXT NOT NULL,
active_version INTEGER NOT NULL,
PRIMARY KEY (conversation_id, turn_group)
);

-- 代码符号索引表（REQ-RAG-031 代码感知 RAG）：
-- tree-sitter AST 抽取的函数/类/结构体等符号，使代码查询能精确定位到函数定义。
CREATE TABLE IF NOT EXISTS code_symbols (
id TEXT PRIMARY KEY,
chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
name TEXT NOT NULL,
kind TEXT NOT NULL,
language TEXT NOT NULL,
start_line INTEGER NOT NULL,
end_line INTEGER NOT NULL,
signature TEXT
);

-- 对话记忆表（REQ-RAG-032 持久化记忆系统增强）：
-- 借鉴 IfAI 三层记忆 Wing/Hall/Room 空间隐喻，存储对话中提取的关键事实。
CREATE TABLE IF NOT EXISTS memory_entries (
id TEXT PRIMARY KEY,
tier TEXT NOT NULL,
content TEXT NOT NULL,
source TEXT NOT NULL,
conversation_id TEXT,
created_at INTEGER NOT NULL,
last_accessed INTEGER NOT NULL,
access_count INTEGER NOT NULL DEFAULT 0,
importance REAL NOT NULL DEFAULT 0.5
);

-- Wiki 双向链接表（REQ-ING-020 Markdown 笔记双向链接）：
-- 存储 Markdown 文档中 [[wiki-link]] 语法的链接关系，支持正向链接和反向链接查询。
CREATE TABLE IF NOT EXISTS wiki_links (
id TEXT PRIMARY KEY,
source_doc_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
target TEXT NOT NULL,
chunk_id TEXT NOT NULL REFERENCES chunks(id) ON DELETE CASCADE,
created_at INTEGER NOT NULL
);

-- Durable Prompt Admission 表（B05 持久化提示接纳）：
-- 存储已接纳但未提升为正式消息的用户输入。
-- delivery: 'steer'（优先中断当前生成）或 'queue'（排队等待）。
-- promoted_seq: NULL = 未提升；非 NULL = 已提升为正式消息的 seq。
CREATE TABLE IF NOT EXISTS pending_inputs (
id TEXT PRIMARY KEY,
conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
content TEXT NOT NULL,
delivery TEXT NOT NULL DEFAULT 'queue',
created_at INTEGER NOT NULL,
promoted_seq INTEGER
);

-- Session Todo 持久化表（B08 会话待办持久化）：
-- 存储 Agent 的 Todo 列表，支持跨会话恢复。
-- status: 'pending' / 'in_progress' / 'completed'
-- priority: 'low' / 'medium' / 'high'
-- position: 排序位置（升序）
CREATE TABLE IF NOT EXISTS session_todos (
id TEXT PRIMARY KEY,
conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
content TEXT NOT NULL,
status TEXT NOT NULL DEFAULT 'pending',
priority TEXT NOT NULL DEFAULT 'medium',
position INTEGER NOT NULL,
created_at INTEGER NOT NULL
);

--- Scratch-Promote 记忆整合表（Q01 借鉴 QM scratch-promote）：
--- 存储临时事实，等待 LLM 审查后 promote 到长期记忆层。
--- date: YYYY-MM-DD 格式，用于按日聚合
--- created_at: Unix 秒级时间戳，用于过期清理（默认 14 天）
CREATE TABLE IF NOT EXISTS scratch_logs (
    id TEXT PRIMARY KEY,
    date TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS idempotency_records (
    key TEXT PRIMARY KEY,
    timestamp INTEGER NOT NULL
);

-- 预算追踪表（QM 借鉴）：
-- 记录 LLM API 使用情况，用于费用追踪和预算控制。
-- LLM_COST_PER_MTOK 环境变量控制每百万 token 价格。
CREATE TABLE IF NOT EXISTS budget_records (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    principal TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    cost_usd REAL NOT NULL,
    model_name TEXT NOT NULL
);

-- 导入历史记录表（REQ-ING-011）：
-- 记录每次导入操作的时间戳、文件名、格式、结果（成功/失败/跳过）。
-- 上限 100 条，超过自动淘汰最旧记录。
CREATE TABLE IF NOT EXISTS import_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    file_name TEXT NOT NULL,
    format TEXT NOT NULL,
    result TEXT NOT NULL,
    error_message TEXT,
    file_size INTEGER
);
";

// ============================================================================
// 索引
// ============================================================================

/// 索引定义（在表创建与迁移完成后执行）。
pub(crate) const SCHEMA_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS idx_documents_hash ON documents(file_hash);
CREATE INDEX IF NOT EXISTS idx_documents_original_path ON documents(original_path);
CREATE INDEX IF NOT EXISTS idx_documents_domain ON documents(domain);
CREATE INDEX IF NOT EXISTS idx_chunks_doc ON chunks(doc_id);
CREATE INDEX IF NOT EXISTS idx_conversations_workspace ON conversations(workspace_id);
CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id);
CREATE INDEX IF NOT EXISTS idx_messages_turn ON messages(conversation_id, turn_group, version);
CREATE INDEX IF NOT EXISTS idx_entities_chunk ON entities(chunk_id);
CREATE INDEX IF NOT EXISTS idx_entities_text ON entities(entity_text);
CREATE INDEX IF NOT EXISTS idx_propositions_chunk ON propositions(chunk_id);
CREATE INDEX IF NOT EXISTS idx_summary_nodes_doc ON summary_nodes(doc_id);
CREATE INDEX IF NOT EXISTS idx_summary_nodes_level ON summary_nodes(level);
CREATE INDEX IF NOT EXISTS idx_relations_subject ON entity_relations(subject);
CREATE INDEX IF NOT EXISTS idx_relations_object ON entity_relations(object);
CREATE INDEX IF NOT EXISTS idx_relations_chunk ON entity_relations(chunk_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON code_symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_chunk ON code_symbols(chunk_id);
CREATE INDEX IF NOT EXISTS idx_symbols_kind ON code_symbols(kind);
CREATE INDEX IF NOT EXISTS idx_memory_tier ON memory_entries(tier);
CREATE INDEX IF NOT EXISTS idx_memory_content ON memory_entries(content);
CREATE INDEX IF NOT EXISTS idx_wiki_links_source ON wiki_links(source_doc_id);
CREATE INDEX IF NOT EXISTS idx_wiki_links_target ON wiki_links(target);
CREATE INDEX IF NOT EXISTS idx_wiki_links_chunk ON wiki_links(chunk_id);
CREATE INDEX IF NOT EXISTS idx_pending_inputs_conv ON pending_inputs(conversation_id);
CREATE INDEX IF NOT EXISTS idx_pending_inputs_promoted ON pending_inputs(promoted_seq);
CREATE INDEX IF NOT EXISTS idx_session_todos_conv ON session_todos(conversation_id);
CREATE INDEX IF NOT EXISTS idx_session_todos_position ON session_todos(conversation_id, position);
CREATE INDEX IF NOT EXISTS idx_scratch_logs_date ON scratch_logs(date);
CREATE INDEX IF NOT EXISTS idx_scratch_logs_created ON scratch_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_budget_records_principal ON budget_records(principal);
CREATE INDEX IF NOT EXISTS idx_budget_records_timestamp ON budget_records(timestamp);
CREATE INDEX IF NOT EXISTS idx_import_logs_timestamp ON import_logs(timestamp);

-- 对话书签表（REQ-RAG-047）
CREATE TABLE IF NOT EXISTS conversation_bookmarks (
  conversation_id TEXT PRIMARY KEY,
  note TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_bookmarks_created ON conversation_bookmarks(created_at);
";

// ============================================================================
// FTS5 全文索引
// ============================================================================

/// FTS5 全文索引虚拟表（混合检索关键词通道，REQ-RAG-010）。
/// 使用 trigram 分词器：支持英文单词匹配 + 中日韩子串匹配。
/// chunk_id / doc_id 为 UNINDEXED 列（仅存储用于 JOIN 回主表，不参与全文索引）。
pub(crate) const SCHEMA_FTS: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    chunk_id UNINDEXED,
    doc_id UNINDEXED,
    content,
    tokenize='trigram'
);
";

/// 对话全文搜索 FTS5 虚拟表（REQ-RAG-040）。
///
/// 索引 messages 表的 content 列，使用 trigram 分词器（与 chunks_fts 一致）。
/// 通过触发器自动同步 messages 表 INSERT/UPDATE/DELETE。
/// message_id / conversation_id 为 UNINDEXED 列（仅存储用于回查，不参与全文索引）。
pub(crate) const SCHEMA_MESSAGES_FTS: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    message_id UNINDEXED,
    conversation_id UNINDEXED,
    content,
    tokenize='trigram'
);

-- 触发器：messages INSERT → messages_fts INSERT
CREATE TRIGGER IF NOT EXISTS messages_fts_ai AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(message_id, conversation_id, content)
    VALUES (new.id, new.conversation_id, new.content);
END;

-- 触发器：messages DELETE → messages_fts DELETE
CREATE TRIGGER IF NOT EXISTS messages_fts_ad AFTER DELETE ON messages BEGIN
    DELETE FROM messages_fts WHERE message_id = old.id;
END;

-- 触发器：messages UPDATE(content) → messages_fts UPDATE
CREATE TRIGGER IF NOT EXISTS messages_fts_au AFTER UPDATE ON messages WHEN new.content != old.content BEGIN
    DELETE FROM messages_fts WHERE message_id = old.id;
    INSERT INTO messages_fts(message_id, conversation_id, content)
    VALUES (new.id, new.conversation_id, new.content);
END;
";

// ============================================================================
// 审计日志
// ============================================================================

/// 审计日志表结构（防篡改哈希链）
pub(crate) const SCHEMA_AUDIT_LOG: &str = "
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    action TEXT NOT NULL,
    details TEXT NOT NULL,
    pii_count INTEGER NOT NULL DEFAULT 0,
    timestamp INTEGER NOT NULL,
    prev_hash TEXT,
    curr_hash TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
";

// ============================================================================
// 列名常量
// ============================================================================

/// documents 表查询列名（与 `row_to_document` 映射器一一对应）。
pub(crate) const DOC_COLS: &str = "id, file_path, file_hash, status, status_reason, created_at, original_path, domain, summary, tags, workspace_id";

// ============================================================================
// 加密常量
// ============================================================================

/// AES-256-GCM 随机数 nonce 长度（96 bit）
pub(crate) const NONCE_LEN: usize = 12;
/// 加密密钥文件名（与数据库同目录，权限 0600）
pub(crate) const SECRET_KEY_FILE: &str = "secret.key";

// ============================================================================
// 表名白名单
// ============================================================================

/// 安全：表名白名单校验，防止 PRAGMA table_info SQL 注入。
pub(crate) const KNOWN_TABLES: [&str; 20] = [
    "documents",
    "chunks",
    "embeddings",
    "settings",
    "conversations",
    "messages",
    "embeddings_cache",
    "entities",
    "propositions",
    "summary_nodes",
    "retrieval_memory",
    "entity_relations",
    "wiki_links",
    "pending_inputs",
    "session_todos",
    "scratch_logs",
    "conversation_bookmarks",
    "idempotency_records",
    "budget_records",
    "workspaces",
];

// ============================================================================
// 注：IntegrityCheckResult 枚举保留在 sqlite_storage.rs 中定义，
// 因为它是 pub API 类型，外部 crate（tauri-app）直接引用。
// 详见 sqlite_storage.rs 中的 pub enum IntegrityCheckResult。
