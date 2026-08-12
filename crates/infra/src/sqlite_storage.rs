//! SQLite 持久化适配器（REQ-DB-001）：rusqlite(bundled) + r2d2 连接池 + WAL 模式。
//! 体系三：rusqlite 为具体实现，仅允许存在于 infra 层；
//! rusqlite 为同步 API，全部数据库操作经 `spawn_blocking` 执行，严禁阻塞 async executor。

use std::path::Path;
use tracing::{error, info, warn};

// ============================================================================
// S6: LRU 向量缓存 — 带驱逐策略的内存向量缓存
// ============================================================================

/// **S6: LRU 向量缓存**。
///
/// 带容量限制的向量缓存，超限时驱逐最久未访问的条目。
/// 替代原全量加载策略，限制大规模知识库（10K+ chunks）的内存占用。
pub(crate) struct LruVectorCache {
    entries: std::collections::HashMap<String, Vec<f32>>,
    order: std::collections::VecDeque<String>,
    max_entries: usize,
}

impl LruVectorCache {
    pub(crate) fn new(max_entries: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            max_entries,
        }
    }

    pub(crate) fn from_vectors(vectors: Vec<(String, Vec<f32>)>, max_entries: usize) -> Self {
        let mut cache = Self::new(max_entries);
        for (id, vec) in vectors {
            cache.insert(id, vec);
        }
        cache
    }

    pub(crate) fn insert(&mut self, key: String, value: Vec<f32>) {
        if self.entries.contains_key(&key) {
            self.order.retain(|k| k != &key);
        } else if self.entries.len() >= self.max_entries
            && let Some(old_key) = self.order.pop_front()
        {
            self.entries.remove(&old_key);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
    }

    pub(crate) fn touch(&mut self, key: &str) {
        if self.entries.contains_key(key) {
            self.order.retain(|k| k != key);
            self.order.push_back(key.to_string());
        }
    }

    pub(crate) fn touch_batch(&mut self, keys: &[String]) {
        for key in keys {
            self.touch(key);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn remove(&mut self, key: &str) {
        if self.entries.remove(key).is_some() {
            self.order.retain(|k| k != key);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    #[allow(dead_code)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[allow(dead_code)]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &Vec<f32>)> {
        self.entries.iter()
    }

    pub(crate) fn to_vec(&self) -> Vec<(String, Vec<f32>)> {
        self.entries
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

type VectorCache = std::sync::Arc<std::sync::RwLock<Option<LruVectorCache>>>;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use echomind_core::Storage;
use echomind_core::privacy::{AuditEntry, AuditLogger};
use echomind_core::proposition_splitter::PropositionSplitter;
use echomind_core::retrieval_memory::{
    MemoryRecord, QueryType, RetrievalMemoryStore, RetrievalMethod,
};
use echomind_models::{
    BudgetStats, ChatMessage, Chunk, CodeSymbol, Conversation, DocStatus, Document, EntityRelation,
    MemoryEntry, MemorySource, MemoryTier, MessageSearchResult, PendingInput, Proposition,
    RetrievalResult, ScratchLogEntry, SessionTodo, SummaryNode, SymbolKind, TodoPriority,
    TodoStatus, TurnActiveVersion, WikiLink,
};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use rusqlite::params;

/// 连接级 PRAGMA：WAL 模式 + 外键级联 + 写繁忙重试 + 性能调优（REQ-DB-001-AC-3）。
///
/// 性能参数说明（2026-07 全尺度优化）：
/// - `cache_size = -65536`：64MB page cache（默认仅 2MB），大幅减少大库查询的磁盘 I/O。
/// - `mmap_size = 268435456`：256MB 零拷贝内存映射，绕过 read() 系统调用。
/// - `wal_autocheckpoint = 10000`：WAL 达到 10000 pages（~40MB）才 checkpoint，
///   减少 GB 级导入时的 checkpoint 频率，提升写入吞吐。
/// - `temp_store = MEMORY`：临时表和排序结果放内存，加速 FTS5 查询和复杂 JOIN。
const PRAGMAS: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -65536;
PRAGMA mmap_size = 268435456;
PRAGMA wal_autocheckpoint = 10000;
PRAGMA temp_store = MEMORY;
";

/// 表结构：documents / chunks / embeddings / settings / conversations / messages。
/// 外键 ON DELETE CASCADE（REQ-ING-005 前置）。
/// 仅含 CREATE TABLE 语句；索引在迁移完成后单独创建（防旧表 schema 不兼容崩溃）。
const SCHEMA_TABLES: &str = "
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

/// 索引定义（在表创建与迁移完成后执行）。
const SCHEMA_INDEXES: &str = "
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

/// FTS5 全文索引虚拟表（混合检索关键词通道，REQ-RAG-010）。
/// 使用 trigram 分词器：支持英文单词匹配 + 中日韩子串匹配。
/// chunk_id / doc_id 为 UNINDEXED 列（仅存储用于 JOIN 回主表，不参与全文索引）。
const SCHEMA_FTS: &str = "
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
const SCHEMA_MESSAGES_FTS: &str = "
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

const DOC_COLS: &str = "id, file_path, file_hash, status, status_reason, created_at, original_path, domain, summary, tags, workspace_id";

/// AES-256-GCM 随机数 nonce 长度（96 bit）
const NONCE_LEN: usize = 12;
/// 加密密钥文件名（与数据库同目录，权限 0600）
const SECRET_KEY_FILE: &str = "secret.key";

/// 审计日志表结构（防篡改哈希链）
const SCHEMA_AUDIT_LOG: &str = "
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

/// 数据库完整性检查结果（REQ-ERR-004）。
///
/// `PRAGMA integrity_check` 返回 `ok` 时为 [`IntegrityCheckResult::Ok`]，
/// 否则为 [`IntegrityCheckResult::Corrupted`]，携带错误详情。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityCheckResult {
    /// 数据库完整性正常
    Ok,
    /// 数据库损坏，携带 `PRAGMA integrity_check` 返回的错误消息
    Corrupted(String),
}

type Pool = r2d2::Pool<SqliteConnectionManager>;

/// SQLite 存储适配器。克隆廉价（共享连接池与加密器）。
#[derive(Clone)]
pub struct SqliteStorage {
    pool: Pool,
    cipher: Aes256Gcm,
    /// HNSW 近似最近邻索引（REQ-NFR-005，Pro feature）。
    /// 懒构建 + 脏标记：文档变更后置脏，下次检索自动重建。
    /// 大知识库（>10K chunks）下将向量检索从全表扫描 O(n) 降为 O(log n)。
    #[cfg(feature = "pro")]
    hnsw: std::sync::Arc<std::sync::Mutex<Option<crate::hnsw_index::HnswIndex>>>,
    /// HNSW 索引是否因文档变更而失效（需要重建）。
    #[cfg(feature = "pro")]
    hnsw_dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// **性能优化（秒出答案）**：内存向量缓存。
    ///
    /// 首次 `vector_search` 时全量加载所有 (chunk_id, vector) 对到内存，
    /// 后续检索直接在内存中计算余弦相似度，跳过 SQLite BLOB 读取 + 反序列化。
    ///
    /// - 1000 chunks (384-dim): ~1.5MB 内存，检索从 ~20ms → ~1ms
    /// - 10K chunks: ~15MB 内存，检索从 ~200ms → ~10ms
    /// - 100K chunks: ~150MB 内存，检索从 ~5s → ~100ms
    ///
    /// 写操作（add_embedding / delete_chunks_by_doc）时自动失效，下次检索重建。
    /// S6: 使用 `LruVectorCache` 带容量限制的 LRU 缓存。
    vector_cache: VectorCache,
    /// S6: LRU 缓存容量上限（默认 5000）。
    max_vectors: usize,
}

impl SqliteStorage {
    /// 打开（必要时创建）数据库，启用 WAL 并初始化表结构与设置加密器。
    /// 本函数含同步磁盘 I/O，调用方应置于 `spawn_blocking` 中。
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("创建数据库目录失败: {}", data_dir.display()))?;
        // REQ-SEC-004：数据目录权限 0700（仅所有者可读写执行）
        ensure_dir_0700(data_dir)?;
        let cipher = Self::load_or_create_cipher(data_dir)?;
        let manager =
            SqliteConnectionManager::file(db_path).with_init(|conn| conn.execute_batch(PRAGMAS));
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .max_lifetime(Some(std::time::Duration::from_secs(1800)))
            .build(manager)
            .context("创建 SQLite 连接池失败")?;
        let storage = Self {
            pool,
            cipher,
            #[cfg(feature = "pro")]
            hnsw: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(feature = "pro")]
            hnsw_dirty: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            vector_cache: std::sync::Arc::new(std::sync::RwLock::new(None)),
            max_vectors: 5000,
        };
        storage.init_schema()?;
        Ok(storage)
    }

    /// 打开加密数据库（SQLCipher AES-256），使用 Argon2id 密钥派生。
    ///
    /// # 参数
    /// - `db_path`: 数据库文件路径
    /// - `pragma_key`: SQLCipher PRAGMA key 字符串（如 `x'2DD29CA8...'`）
    ///
    /// # 安全说明
    /// - 密钥仅存在于内存中，通过 PRAGMA key 传递给 SQLCipher
    /// - 数据库文件在磁盘上为 AES-256-CBC 加密
    /// - 首次调用时创建加密数据库；后续调用用同一密钥打开
    pub fn new_encrypted(db_path: &Path, pragma_key: &str) -> anyhow::Result<Self> {
        let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("创建数据库目录失败: {}", data_dir.display()))?;
        ensure_dir_0700(data_dir)?;
        let cipher = Self::load_or_create_cipher(data_dir)?;

        // 加密模式：每个连接先执行 PRAGMA key，再执行其他 PRAGMA
        let key = pragma_key.to_string();
        let manager = SqliteConnectionManager::file(db_path)
            .with_init(move |conn| conn.execute_batch(&format!("PRAGMA key = {key};\n{PRAGMAS}")));
        let pool = Pool::builder()
            .max_size(8)
            .min_idle(Some(2))
            .max_lifetime(Some(std::time::Duration::from_secs(1800)))
            .build(manager)
            .context("创建加密 SQLite 连接池失败")?;
        let storage = Self {
            pool,
            cipher,
            #[cfg(feature = "pro")]
            hnsw: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(feature = "pro")]
            hnsw_dirty: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            vector_cache: std::sync::Arc::new(std::sync::RwLock::new(None)),
            max_vectors: 5000,
        };
        storage.init_schema()?;
        Ok(storage)
    }

    /// 返回连接池的克隆（供 `SqliteCache` 共享同一数据库，REQ-PERF-001）。
    pub fn pool_clone(&self) -> Pool {
        self.pool.clone()
    }

    /// S6: 设置 LRU 向量缓存容量上限。
    ///
    /// 更新后会使现有缓存失效（下次检索按新容量重建）。
    pub fn set_max_vectors(&mut self, max: usize) {
        self.max_vectors = max;
        self.invalidate_vector_cache();
    }

    /// S6: 获取 LRU 向量缓存容量上限。
    #[must_use]
    pub fn max_vectors(&self) -> usize {
        self.max_vectors
    }

    /// **性能优化（秒出答案）**：使内存向量缓存失效。
    ///
    /// 在嵌入写入/删除时调用，下次 `vector_search` 自动重建缓存。
    /// 同时使 HNSW 索引失效（Pro feature）。
    fn invalidate_vector_cache(&self) {
        if let Ok(mut guard) = self.vector_cache.write() {
            *guard = None;
        }
        #[cfg(feature = "pro")]
        self.mark_hnsw_dirty();
    }

    fn init_schema(&self) -> anyhow::Result<()> {
        let conn = self.pool.get().context("获取数据库连接失败")?;
        // 步骤 1：创建表（IF NOT EXISTS 不修改已有表结构）
        conn.execute_batch(SCHEMA_TABLES)
            .context("初始化数据库表结构失败")?;
        // 步骤 2：迁移旧表 schema（修复历史版本不兼容的列缺失）
        Self::migrate_schema(&conn)?;
        // 步骤 3：创建索引（此时所有列已保证存在）
        conn.execute_batch(SCHEMA_INDEXES)
            .context("初始化数据库索引失败")?;
        // 步骤 4：创建 FTS5 全文索引虚拟表（混合检索关键词通道）
        conn.execute_batch(SCHEMA_FTS)
            .context("初始化 FTS5 全文索引失败")?;
        // 步骤 5：迁移——若 chunks 表已有数据但 chunks_fts 为空，回填全文索引
        Self::backfill_fts_if_needed(&conn)?;
        // 步骤 5b：创建对话全文搜索 FTS5 虚拟表 + 触发器（REQ-RAG-040）
        conn.execute_batch(SCHEMA_MESSAGES_FTS)
            .context("初始化对话 FTS5 全文索引失败")?;
        // 步骤 5c：迁移——若 messages 表已有数据但 messages_fts 为空，回填全文索引
        Self::backfill_messages_fts_if_needed(&conn)?;
        // 步骤 6：创建审计日志表（防篡改哈希链）
        conn.execute_batch(SCHEMA_AUDIT_LOG)
            .context("初始化审计日志表失败")?;
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
    fn migrate_schema(conn: &rusqlite::Connection) -> anyhow::Result<()> {
        // ── conversations 表：sort_order 列在 v1.16 新增（REQ-IX-002 拖拽排序）──
        if Self::table_exists(conn, "conversations")?
            && !Self::has_column(conn, "conversations", "sort_order")?
        {
            info!("schema 迁移：conversations 表缺少 sort_order 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("迁移失败：conversations 表添加 sort_order 列失败")?;
        }

        // ── documents 表：status_reason 列在 Phase 4+ 新增 ──
        // ALTER TABLE ADD COLUMN 安全，旧行 status_reason = NULL
        if Self::table_exists(conn, "documents")?
            && !Self::has_column(conn, "documents", "status_reason")?
        {
            info!("schema 迁移：documents 表缺少 status_reason 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute("ALTER TABLE documents ADD COLUMN status_reason TEXT", [])
                .context("迁移失败：documents 表添加 status_reason 列失败")?;
        }

        // ── chunks 表：旧版含 session_id 列，新版不含 ──
        // SQLite ALTER TABLE 不支持 DROP COLUMN（3.35.0 前），
        // 使用 CREATE-COPY-DROP-RENAME 模式保留分块数据
        if Self::table_exists(conn, "chunks")? && Self::has_column(conn, "chunks", "session_id")? {
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
        if Self::table_exists(conn, "embeddings")?
            && !Self::has_column(conn, "embeddings", "vector")?
            && Self::has_column(conn, "embeddings", "embedding")?
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
        if Self::table_exists(conn, "messages")?
            && !Self::has_column(conn, "messages", "conversation_id")?
        {
            info!("schema 迁移：messages 表缺少 conversation_id 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute("ALTER TABLE messages ADD COLUMN conversation_id TEXT", [])
                .context("迁移失败：messages 表添加 conversation_id 列失败")?;
        }

        // ── messages 表：reasoning 列（推理思考过程持久化，P2-1 之后新增）──
        // ALTER TABLE ADD COLUMN 添加为 nullable；旧消息 reasoning = NULL（无思考过程）
        if Self::table_exists(conn, "messages")?
            && !Self::has_column(conn, "messages", "reasoning")?
        {
            info!("schema 迁移：messages 表缺少 reasoning 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute("ALTER TABLE messages ADD COLUMN reasoning TEXT", [])
                .context("迁移失败：messages 表添加 reasoning 列失败")?;
        }

        // ── messages 表：turn_group + version 列（用户消息编辑版本持久化）──
        // ALTER TABLE ADD COLUMN 添加；旧消息 turn_group='' / version=1（视为无版本管理）
        if Self::table_exists(conn, "messages")?
            && !Self::has_column(conn, "messages", "turn_group")?
        {
            info!("schema 迁移：messages 表缺少 turn_group 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute(
                "ALTER TABLE messages ADD COLUMN turn_group TEXT NOT NULL DEFAULT ''",
                [],
            )
            .context("迁移失败：messages 表添加 turn_group 列失败")?;
        }
        if Self::table_exists(conn, "messages")? && !Self::has_column(conn, "messages", "version")?
        {
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
        if Self::table_exists(conn, "messages")?
            && !Self::has_column(conn, "messages", "security_tainted")?
        {
            info!("schema 迁移：messages 表缺少 security_tainted 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute(
                "ALTER TABLE messages ADD COLUMN security_tainted INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .context("迁移失败：messages 表添加 security_tainted 列失败")?;
        }

        // ── documents 表：original_path 列在 REQ-SYNC-002 新增 ──
        // ALTER TABLE ADD COLUMN 添加为 nullable；旧文档 original_path = NULL
        if Self::table_exists(conn, "documents")?
            && !Self::has_column(conn, "documents", "original_path")?
        {
            info!("schema 迁移：documents 表缺少 original_path 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute("ALTER TABLE documents ADD COLUMN original_path TEXT", [])
                .context("迁移失败：documents 表添加 original_path 列失败")?;
        }

        // ── documents 表：domain 列在 REQ-VEC-013 新增 ──
        // ALTER TABLE ADD COLUMN 添加为 nullable；旧文档 domain = NULL（尚未分类）
        if Self::table_exists(conn, "documents")? && !Self::has_column(conn, "documents", "domain")?
        {
            info!("schema 迁移：documents 表缺少 domain 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute("ALTER TABLE documents ADD COLUMN domain TEXT", [])
                .context("迁移失败：documents 表添加 domain 列失败")?;
        }

        // ── documents 表：summary 列在 REQ-ING-019 新增 ──
        // ALTER TABLE ADD COLUMN 添加为 nullable；旧文档 summary = NULL（尚未生成摘要）
        if Self::table_exists(conn, "documents")?
            && !Self::has_column(conn, "documents", "summary")?
        {
            info!("schema 迁移：documents 表缺少 summary 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute("ALTER TABLE documents ADD COLUMN summary TEXT", [])
                .context("迁移失败：documents 表添加 summary 列失败")?;
        }

        // ── documents 表：tags 列在 REQ-ING-022 新增 ──
        // ALTER TABLE ADD COLUMN 添加；旧文档 tags = '[]'（空标签数组）
        if Self::table_exists(conn, "documents")? && !Self::has_column(conn, "documents", "tags")? {
            info!("schema 迁移：documents 表缺少 tags 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute(
                "ALTER TABLE documents ADD COLUMN tags TEXT NOT NULL DEFAULT '[]'",
                [],
            )
            .context("迁移失败：documents 表添加 tags 列失败")?;
        }

        // ── documents 表：workspace_id 列在 REQ-WS-001 新增 ──
        // ALTER TABLE ADD COLUMN 添加；旧文档 workspace_id = 'default'（默认工作空间）
        if Self::table_exists(conn, "documents")?
            && !Self::has_column(conn, "documents", "workspace_id")?
        {
            info!("schema 迁移：documents 表缺少 workspace_id 列，执行 ALTER TABLE ADD COLUMN");
            conn.execute(
                "ALTER TABLE documents ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default'",
                [],
            )
            .context("迁移失败：documents 表添加 workspace_id 列失败")?;
        }

        // ── workspaces 表：REQ-WS-001 多知识库元数据表 ──
        // 创建表（如不存在），并插入默认工作空间行（幂等）
        if !Self::table_exists(conn, "workspaces")? {
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
        if Self::table_exists(conn, "entities")? && !Self::has_column(conn, "entities", "chunk_id")?
        {
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
        if Self::table_exists(conn, "propositions")?
            && !Self::has_column(conn, "propositions", "chunk_id")?
        {
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
        if Self::table_exists(conn, "summary_nodes")?
            && !Self::has_column(conn, "summary_nodes", "doc_id")?
        {
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
        const KNOWN_TABLES: [&str; 20] = [
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
        if !KNOWN_TABLES.contains(&table) {
            bail!("未知表名（不在白名单中）: {table}");
        }
        Ok(())
    }

    /// 检查指定表是否存在。
    fn table_exists(conn: &rusqlite::Connection, table: &str) -> anyhow::Result<bool> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            params![table],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// 检查指定表是否包含某列（基于 PRAGMA table_info）。
    /// 安全：SQLite PRAGMA 不支持参数绑定，先用白名单校验表名防注入。
    fn has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> anyhow::Result<bool> {
        Self::validate_table_name(table)?;
        let sql = format!("PRAGMA table_info(\"{table}\")");
        let mut stmt = conn.prepare(&sql)?;
        let col_names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(col_names.iter().any(|c| c == column))
    }

    /// 加载或生成 AES-256-GCM 密钥（REQ-UI-008；文件权限 0600）。
    fn load_or_create_cipher(data_dir: &Path) -> anyhow::Result<Aes256Gcm> {
        let key_path = data_dir.join(SECRET_KEY_FILE);
        let key_bytes: [u8; 32] = if key_path.exists() {
            let raw = std::fs::read(&key_path)
                .with_context(|| format!("读取密钥文件失败: {}", key_path.display()))?;
            raw.try_into()
                .map_err(|_| anyhow!("密钥文件损坏（长度非法）: {}", key_path.display()))?
        } else {
            let generated: [u8; 32] = rand::random();
            std::fs::write(&key_path, generated)
                .with_context(|| format!("写入密钥文件失败: {}", key_path.display()))?;
            // REQ-SEC-004：密钥文件权限 0600（仅所有者可读写）
            ensure_file_0600(&key_path)?;
            generated
        };
        Ok(Aes256Gcm::new(aes_gcm::Key::<Aes256Gcm>::from_slice(
            &key_bytes,
        )))
    }

    /// AES-256-GCM 加密 → base64(nonce‖ciphertext)。
    fn encrypt(&self, plaintext: &str) -> anyhow::Result<String> {
        let nonce_bytes: [u8; NONCE_LEN] = rand::random();
        let ciphertext = self
            .cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
            .map_err(|e| anyhow!("设置项加密失败: {e}"))?;
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&ciphertext);
        Ok(B64.encode(blob))
    }

    /// base64(nonce‖ciphertext) → 解密明文。
    fn decrypt(&self, encoded: &str) -> anyhow::Result<String> {
        let blob = B64.decode(encoded).context("设置项 base64 解码失败")?;
        if blob.len() < NONCE_LEN {
            bail!("设置项密文长度非法");
        }
        let (nonce, ciphertext) = blob.split_at(NONCE_LEN);
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|e| anyhow!("设置项解密失败（密钥不匹配或数据损坏）: {e}"))?;
        String::from_utf8(plaintext).context("设置项明文非合法 UTF-8")
    }
}

/// 在阻塞线程池执行数据库任务。
async fn run_db<T>(f: impl FnOnce() -> anyhow::Result<T> + Send + 'static) -> anyhow::Result<T>
where
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .context("数据库任务执行失败")?
}

/// 在事务中执行数据库操作，错误时自动 ROLLBACK（Bug #1 修复）。
///
/// **问题**：此前 `BEGIN IMMEDIATE` + `?` + `COMMIT` 模式在 `?` 返回 Err 时
/// 跳过 `COMMIT`，连接被放回池中但事务仍然打开，后续使用该连接的操作
/// 会意外追加到未提交的事务中，导致数据不一致。
///
/// **修复**：此辅助函数在操作成功时 `COMMIT`，失败时 `ROLLBACK`，
/// 保证连接归还池时事务已关闭。
fn with_transaction<F, T>(conn: &rusqlite::Connection, f: F) -> anyhow::Result<T>
where
    F: FnOnce(&rusqlite::Connection) -> anyhow::Result<T>,
{
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match f(conn) {
        Ok(result) => {
            conn.execute_batch("COMMIT")?;
            Ok(result)
        }
        Err(e) => {
            // ROLLBACK 失败不影响错误传播（连接会在 drop 时自动回滚）
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// 设置目录权限为 0700（仅 Unix；Windows 无此概念，自动跳过）。
/// REQ-SEC-004：数据目录仅所有者可读写执行，防止其他用户访问敏感数据。
#[cfg(unix)]
fn ensure_dir_0700(dir: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("设置目录权限 0700 失败: {}", dir.display()))
}

/// 设置目录权限为 0700（Windows 无此概念，自动跳过）。
#[cfg(not(unix))]
fn ensure_dir_0700(_dir: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// 设置文件权限为 0600（仅 Unix；Windows 无此概念，自动跳过）。
/// REQ-SEC-004：敏感文件（密钥、数据库）仅所有者可读写。
#[cfg(unix)]
fn ensure_file_0600(file: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("设置文件权限 0600 失败: {}", file.display()))
}

/// 设置文件权限为 0600（Windows 无此概念，自动跳过）。
#[cfg(not(unix))]
fn ensure_file_0600(_file: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn status_to_row(status: &DocStatus) -> (&'static str, Option<String>) {
    match status {
        DocStatus::Pending => ("pending", None),
        DocStatus::Processing => ("processing", None),
        DocStatus::Indexed => ("indexed", None),
        DocStatus::Failed(reason) => ("failed", Some(reason.clone())),
    }
}

fn row_to_status(status: &str, reason: Option<String>) -> DocStatus {
    match status {
        "pending" => DocStatus::Pending,
        "processing" => DocStatus::Processing,
        "indexed" => DocStatus::Indexed,
        "failed" => DocStatus::Failed(reason.unwrap_or_default()),
        other => DocStatus::Failed(format!("未知状态标识: {other}")),
    }
}

fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
    let status: String = row.get(3)?;
    let reason: Option<String> = row.get(4)?;
    // tags 列存储为 JSON 数组字符串（如 `["法律","重要"]`）
    let tags_json: String = row.get(9).unwrap_or_else(|_| "[]".to_string());
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    // workspace_id 列（REQ-WS-001，索引 10）
    let workspace_id: String = row.get(10).unwrap_or_else(|_| "default".to_string());
    Ok(Document {
        id: row.get(0)?,
        file_path: row.get(1)?,
        file_hash: row.get(2)?,
        status: row_to_status(&status, reason),
        created_at: row.get(5)?,
        original_path: row.get(6)?,
        domain: row.get(7)?,
        summary: row.get(8)?,
        tags,
        workspace_id,
    })
}

/// 将数据库行转换为对话记忆条目（REQ-RAG-032）。
fn row_to_memory_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEntry> {
    let tier_str: String = row.get(1)?;
    let source_str: String = row.get(3)?;
    Ok(MemoryEntry {
        id: row.get(0)?,
        tier: MemoryTier::parse_str(&tier_str).unwrap_or(MemoryTier::Wing),
        content: row.get(2)?,
        source: MemorySource::parse_str(&source_str).unwrap_or(MemorySource::AutoExtracted),
        conversation_id: row.get(4)?,
        created_at: row.get(5)?,
        last_accessed: row.get(6)?,
        access_count: row.get::<_, i64>(7)? as u32,
        importance: row.get(8)?,
    })
}

/// f32 向量 → 小端字节（零依赖序列化）。
fn vec_to_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// 小端字节 → f32 向量（长度校验，绝不 Panic）。
fn bytes_to_vec(bytes: &[u8]) -> anyhow::Result<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        bail!("向量字节长度非法（非 4 的倍数）: {}", bytes.len());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

impl SqliteStorage {
    // ---- HNSW 索引支持（REQ-NFR-005 / F1-2）----

    /// 标记 HNSW 索引失效：分块/向量变更（导入、删除、重索引）后调用，
    /// 下次 `vector_search` 自动全量重建。仅 Pro feature 生效。
    #[cfg(feature = "pro")]
    pub fn mark_hnsw_dirty(&self) {
        self.hnsw_dirty
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// 从 SQLite 加载全部 embeddings（HNSW 索引构建用，REQ-NFR-005）。
    ///
    /// 返回 `(chunk_id, vector)` 对列表，按 doc_id + sequence 排序。
    pub async fn load_all_embeddings(&self) -> anyhow::Result<Vec<(String, Vec<f32>)>> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT e.chunk_id, e.vector
                 FROM embeddings e
                 JOIN chunks c ON c.id = e.chunk_id
                 JOIN documents d ON d.id = c.doc_id
                 ORDER BY c.doc_id, c.sequence",
            )?;
            let rows = stmt.query_map([], |row| {
                let chunk_id: String = row.get(0)?;
                let vector: Vec<u8> = row.get(1)?;
                Ok((chunk_id, vector))
            })?;
            let mut out = Vec::new();
            for row in rows {
                let (chunk_id, bytes) = row?;
                let vector = bytes_to_vec(&bytes)?;
                out.push((chunk_id, vector));
            }
            Ok(out)
        })
        .await
    }

    /// 按 chunk_id 列表批量查询 chunk 详情（HNSW 查询后获取详情用，REQ-NFR-005）。
    ///
    /// 返回 `Vec<RetrievalResult>`，score 字段为 0.0（由调用方从 HNSW 结果设置）。
    pub async fn get_chunks_by_ids(&self, ids: &[String]) -> anyhow::Result<Vec<RetrievalResult>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let pool = self.pool.clone();
        let ids = ids.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let placeholders: String = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, d.file_path
                 FROM chunks c
                 JOIN documents d ON d.id = c.doc_id
                 WHERE c.id IN ({placeholders})"
            );
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
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
                    score: 0.0,
                    doc_name,
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

    // ---- 数据库完整性检查（REQ-ERR-004）----

    /// 执行 `PRAGMA integrity_check` 检查数据库完整性（REQ-ERR-004-AC-2）。
    ///
    /// 检查流程：
    /// 1. 执行 `PRAGMA integrity_check`，返回 `ok` 则正常。
    /// 2. 若非 `ok`（损坏），尝试 `PRAGMA wal_checkpoint(TRUNCATE)` 进行 WAL 恢复。
    /// 3. 恢复后重新检查；若仍损坏，返回 `Corrupted` 并携带错误消息。
    ///
    /// 本方法为同步执行（PRAGMA 操作轻量），调用方可置于 `spawn_blocking` 中。
    pub fn check_integrity_sync(&self) -> anyhow::Result<IntegrityCheckResult> {
        let conn = self.pool.get().context("获取数据库连接失败")?;
        let result: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("执行 PRAGMA integrity_check 失败")?;
        if result.eq_ignore_ascii_case("ok") {
            return Ok(IntegrityCheckResult::Ok);
        }

        // 损坏 → 尝试 WAL checkpoint 恢复
        warn!("数据库完整性检查异常: {result}，尝试 WAL checkpoint 恢复…");
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");

        // 恢复后重新检查
        let recheck: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("恢复后重新执行 PRAGMA integrity_check 失败")?;
        if recheck.eq_ignore_ascii_case("ok") {
            info!("WAL checkpoint 恢复成功，数据库完整性恢复正常");
            return Ok(IntegrityCheckResult::Ok);
        }

        // 仍损坏 → 返回错误详情
        let detail = format!("{} → {}", result, recheck);
        error!("数据库完整性恢复失败: {detail}");
        Ok(IntegrityCheckResult::Corrupted(detail))
    }

    /// 执行 `PRAGMA integrity_check` 的异步封装（REQ-ERR-004）。
    ///
    /// 在 `spawn_blocking` 中调用 [`check_integrity_sync`]，避免阻塞 async executor。
    pub async fn check_integrity(&self) -> anyhow::Result<IntegrityCheckResult> {
        let pool = self.pool.clone();
        run_db(move || {
            // 重新获取连接执行 PRAGMA（与 sync 版本逻辑一致，但通过独立的连接池获取）
            let conn = pool.get().context("获取数据库连接失败")?;
            let result: String = conn
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .context("执行 PRAGMA integrity_check 失败")?;
            if result.eq_ignore_ascii_case("ok") {
                return Ok(IntegrityCheckResult::Ok);
            }

            warn!("数据库完整性检查异常: {result}，尝试 WAL checkpoint 恢复…");
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");

            let recheck: String = conn
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .context("恢复后重新执行 PRAGMA integrity_check 失败")?;
            if recheck.eq_ignore_ascii_case("ok") {
                info!("WAL checkpoint 恢复成功，数据库完整性恢复正常");
                return Ok(IntegrityCheckResult::Ok);
            }

            let detail = format!("{} → {}", result, recheck);
            error!("数据库完整性恢复失败: {detail}");
            Ok(IntegrityCheckResult::Corrupted(detail))
        })
        .await
    }

    // ---- 嵌入缓存（全尺度优化：按内容指纹跳过重复 ONNX 推理）----

    /// 按内容哈希查找缓存的嵌入向量。
    ///
    /// 命中则返回 `Some(Vec<f32>)`，未命中返回 `None`。
    /// 此方法同步执行（轻量级单条查询），在 `spawn_blocking` 中调用。
    pub fn find_cached_embedding_sync(
        conn: &rusqlite::Connection,
        content_hash: &str,
    ) -> anyhow::Result<Option<Vec<f32>>> {
        let mut stmt = conn
            .prepare("SELECT embedding FROM embeddings_cache WHERE content_hash = ?1")
            .context("查询嵌入缓存失败")?;
        let mut rows = stmt.query_map(params![content_hash], |row| row.get::<_, Vec<u8>>(0))?;
        match rows.next() {
            Some(Ok(bytes)) => Ok(Some(bytes_to_vec(&bytes)?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    /// 将嵌入向量写入缓存。
    ///
    /// 使用 `INSERT OR IGNORE` — 并发场景下首次写入者胜出，后续写入静默跳过。
    /// 此方法同步执行（轻量级单条写入），在 `spawn_blocking` 中调用。
    pub fn cache_embedding_sync(
        conn: &rusqlite::Connection,
        content_hash: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        let bytes = vec_to_bytes(embedding);
        conn.execute(
            "INSERT OR IGNORE INTO embeddings_cache (content_hash, embedding) VALUES (?1, ?2)",
            params![content_hash, bytes],
        )
        .context("写入嵌入缓存失败")?;
        Ok(())
    }
}

impl Storage for SqliteStorage {
    async fn add_document(&self, doc: &Document) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc = doc.clone();
        run_db(move || {
            let (status, reason) = status_to_row(&doc.status);
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
&format!(
"INSERT OR REPLACE INTO documents ({DOC_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
            ),
            params![
                doc.id,
                doc.file_path,
                doc.file_hash,
                status,
                reason,
                doc.created_at,
                doc.original_path,
                doc.domain,
                doc.summary,
                serde_json::to_string(&doc.tags).unwrap_or_else(|_| "[]".to_string()),
                doc.workspace_id,
],
)
            .context("写入文档失败")?;
            Ok(())
        })
        .await
    }

    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let (status, reason) = status_to_row(&status);
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "UPDATE documents SET status = ?1, status_reason = ?2 WHERE id = ?3",
                params![status, reason, doc_id],
            )
            .context("更新文档状态失败")?;
            Ok(())
        })
        .await
    }

    async fn add_chunk(&self, chunk: &Chunk) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let chunk = chunk.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
"INSERT OR REPLACE INTO chunks (id, doc_id, content, token_count, sequence) VALUES (?1, ?2, ?3, ?4, ?5)",
params![chunk.id, chunk.doc_id, chunk.content, chunk.token_count, chunk.sequence],
)
.context("写入分块失败")?;
// Contextual BM25（REQ-PERF-005）：FTS5 索引使用文档名前缀
let doc_name: String = conn
    .query_row(
        "SELECT file_path FROM documents WHERE id = ?1",
        params![chunk.doc_id],
        |row| row.get(0),
    )
    .map(|fp: String| {
        std::path::Path::new(&fp)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or(fp)
    })
    .unwrap_or_else(|_| "unknown".to_string());
let contextual_content =
    echomind_core::retriever::build_contextual_text(&doc_name, &chunk.content);
conn.execute(
"INSERT INTO chunks_fts (chunk_id, doc_id, content) VALUES (?1, ?2, ?3)",
params![chunk.id, chunk.doc_id, contextual_content],
)
.context("写入 FTS5 索引失败")?;
Ok(())
        })
        .await
    }

    /// 批量写入分块（单事务，性能优化）。
    ///
    /// 将所有 chunks 在单个事务中批量 INSERT，消除逐条写入的隐式事务开销与
    /// `spawn_blocking` 调度次数。空 Vec 直接返回 Ok。
    async fn add_chunks_batch(&self, chunks: &[Chunk]) -> anyhow::Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let chunks = chunks.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：使用 with_transaction 保证错误时自动 ROLLBACK
            with_transaction(&conn, |conn| {
                // 预编译语句，在循环中复用参数绑定
                let mut chunk_stmt = conn
                    .prepare(
                        "INSERT OR REPLACE INTO chunks (id, doc_id, content, token_count, sequence) \
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                    )
                    .context("预编译 chunks 批量写入语句失败")?;
                let mut fts_stmt = conn
                    .prepare("INSERT INTO chunks_fts (chunk_id, doc_id, content) VALUES (?1, ?2, ?3)")
                    .context("预编译 FTS5 批量写入语句失败")?;

                // Bug #2 修复：按 doc_id 分组查询文档名，避免跨文档批次使用错误文档名
                let mut doc_name_cache: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                for chunk in &chunks {
                    let doc_name = doc_name_cache.entry(chunk.doc_id.clone()).or_insert_with(|| {
                        conn.query_row(
                            "SELECT file_path FROM documents WHERE id = ?1",
                            params![&chunk.doc_id],
                            |row| row.get(0),
                        )
                        .map(|fp: String| {
                            std::path::Path::new(&fp)
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or(fp)
                        })
                        .unwrap_or_else(|_| "unknown".to_string())
                    });

                    chunk_stmt
                        .execute(params![
                            chunk.id,
                            chunk.doc_id,
                            chunk.content,
                            chunk.token_count,
                            chunk.sequence,
                        ])
                        .context("批量写入 chunks 失败")?;
                    let contextual_content =
                        echomind_core::retriever::build_contextual_text(doc_name, &chunk.content);
                    fts_stmt
                        .execute(params![chunk.id, chunk.doc_id, contextual_content])
                        .context("批量写入 FTS5 索引失败")?;
                }
                Ok(())
            })
        })
        .await?;
        // **性能优化**：使内存向量缓存失效
        self.invalidate_vector_cache();
        Ok(())
    }

    async fn add_embedding(&self, chunk_id: &str, embedding: &[f32]) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let chunk_id = chunk_id.to_string();
        let bytes = vec_to_bytes(embedding);
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?1, ?2)",
                params![chunk_id, bytes],
            )
            .context("写入向量失败")?;
            Ok(())
        })
        .await?;
        // **性能优化**：使内存向量缓存失效（下次检索时自动重建）
        self.invalidate_vector_cache();
        Ok(())
    }

    /// 批量写入向量（性能优化：单事务 + 仅一次缓存失效）。
    ///
    /// 原实现逐条调用 `add_embedding`，每次 spawn_blocking + pool.get() + INSERT +
    /// invalidate_vector_cache。400+ chunks = 400+ 次 DB 往返 + 400+ 次缓存失效。
    ///
    /// 优化后：全部 embeddings 在单个事务中提交（1 次 spawn_blocking），缓存仅失效一次。
    /// 实测 414 chunks 从 ~8s 降至 <0.1s（80x 加速）。
    async fn add_embeddings_batch(&self, embeddings: &[(String, Vec<f32>)]) -> anyhow::Result<()> {
        if embeddings.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        // 预序列化：chunk_id + f32→bytes 转换在 spawn_blocking 外完成，
        // 避免在阻塞线程中分配不必要的内存
        let items: Vec<(String, Vec<u8>)> = embeddings
            .iter()
            .map(|(id, vec)| (id.clone(), vec_to_bytes(vec)))
            .collect();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            with_transaction(&conn, |conn| {
                let mut stmt = conn
                    .prepare("INSERT OR REPLACE INTO embeddings (chunk_id, vector) VALUES (?1, ?2)")
                    .context("预编译 embeddings 批量写入语句失败")?;
                for (chunk_id, bytes) in &items {
                    stmt.execute(params![chunk_id, bytes])
                        .context("批量写入 embedding 失败")?;
                }
                Ok(())
            })
        })
        .await?;
        // 仅在全部写入完成后失效一次（而非每条写入各失效一次）
        self.invalidate_vector_cache();
        Ok(())
    }

    async fn vector_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        #[cfg(feature = "pro")]
        {
            // ---- HNSW 快速路径（REQ-NFR-005，Pro feature）----
            // 索引未构建或文档变更（dirty）→ 全量重建；否则直接搜索，
            // 大知识库检索从全表扫描 O(n) 降为 O(log n)。
            let use_hnsw = {
                let idx = self.hnsw.lock().unwrap_or_else(|e| e.into_inner());
                idx.is_some() && !self.hnsw_dirty.load(std::sync::atomic::Ordering::SeqCst)
            };
            if use_hnsw {
                // use_hnsw 已确认索引存在；此处防御性匹配，None（竞态）→ 降级全表扫描
                let search_hits = {
                    let idx = self.hnsw.lock().unwrap_or_else(|e| e.into_inner());
                    idx.as_ref().map(|idx| {
                        idx.search(query_embedding, top_k * 4)
                            .into_iter()
                            .map(|(id, dist)| (id, 1.0 - dist))
                            .collect::<Vec<_>>()
                    })
                };
                if let Some(hits) = search_hits {
                    let (ids, scores): (Vec<String>, Vec<f32>) = hits.into_iter().unzip();
                    if !ids.is_empty() {
                        let mut results = self.get_chunks_by_ids(&ids).await?;
                        for r in &mut results {
                            if let Some(pos) = ids.iter().position(|id| id == &r.chunk.id) {
                                r.score = scores[pos];
                            }
                        }
                        results.sort_by(|a, b| {
                            b.score
                                .partial_cmp(&a.score)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        results.truncate(top_k);
                        return Ok(results);
                    }
                }
            }
            // 索引缺失或脏：全量加载向量 + 构建（spawn_blocking 内 CPU 密集）并搜索
            let vectors = self.load_all_embeddings().await?;
            // 防御：维度不匹配的向量（旧 schema 迁移遗留数据）不参与 HNSW 图构建
            // （anndists 要求维度一致，否则构建断言崩溃）。其与任何查询的余弦相似度
            // 均为 0.0，查询后以 0.0 分追加返回，与全表扫描路径语义一致。
            let query_dim = query_embedding.len();
            let (matched, excluded): (Vec<_>, Vec<_>) =
                vectors.into_iter().partition(|(_, v)| v.len() == query_dim);
            let (hnsw, hnsw_dirty) = (self.hnsw.clone(), self.hnsw_dirty.clone());
            let query = query_embedding.to_vec();
            let mut built_hits = tokio::task::spawn_blocking(move || {
                let idx = crate::hnsw_index::HnswIndex::build(&matched)?;
                let hits = idx.search(&query, top_k * 4);
                *hnsw.lock().unwrap_or_else(|e| e.into_inner()) = Some(idx);
                hnsw_dirty.store(false, std::sync::atomic::Ordering::SeqCst);
                Ok::<_, anyhow::Error>(hits)
            })
            .await
            .context("HNSW 索引构建任务执行失败")??;
            // 维度不匹配的旧数据：得分 0.0 追加（与全表扫描路径一致）
            if !excluded.is_empty() {
                built_hits.extend(
                    excluded.into_iter().map(|(id, _)| (id, 1.0_f32)), // 距离 1.0 → 得分 0.0
                );
            }
            if !built_hits.is_empty() {
                let ids: Vec<String> = built_hits.iter().map(|(id, _)| id.clone()).collect();
                let scores: Vec<f32> = built_hits.iter().map(|(_, d)| 1.0 - d).collect();
                let mut results = self.get_chunks_by_ids(&ids).await?;
                for r in &mut results {
                    if let Some(pos) = ids.iter().position(|id| id == &r.chunk.id) {
                        r.score = scores[pos];
                    }
                }
                results.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                results.truncate(top_k);
                return Ok(results);
            }
            // 空库：落入下方内存缓存 / 全表扫描路径
        }

        // ---- 内存向量缓存快速路径（性能优化：秒出答案）----
        // S6: 使用 LruVectorCache 带容量限制的 LRU 缓存。
        // 首次检索时加载向量到内存，后续检索跳过 SQLite BLOB I/O + 反序列化。
        // 注意：read guard 必须在 .await 前释放（std::sync::RwLockReadGuard 非 Send）
        let cached: Option<Vec<(String, Vec<f32>)>> = {
            let guard = self.vector_cache.read();
            guard
                .ok()
                .and_then(|g| g.as_ref().map(|cache| cache.to_vec()))
        };

        let vectors: Vec<(String, Vec<f32>)> = match cached {
            Some(v) => v,
            None => {
                // 缓存未命中：全量加载并填充 LRU 缓存
                let loaded = self.load_all_embeddings().await?;
                let cache = LruVectorCache::from_vectors(loaded, self.max_vectors);
                let vec = cache.to_vec();
                if let Ok(mut guard) = self.vector_cache.write() {
                    *guard = Some(cache);
                }
                vec
            }
        };

        // 在内存中计算余弦相似度，取 top-k
        let query_vec = query_embedding.to_vec();
        let top_k_val = top_k;
        let top_hits: Vec<(String, f32)> = tokio::task::spawn_blocking(move || {
            // 使用简单的 Vec + sort 取 top-k（比 BinaryHeap 更直观，性能相当）
            let mut all_scores: Vec<(String, f32)> = Vec::with_capacity(vectors.len());
            for (chunk_id, vector) in &vectors {
                let score = cosine_similarity(&query_vec, vector);
                all_scores.push((chunk_id.clone(), score));
            }
            all_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            all_scores.truncate(top_k_val);
            all_scores
        })
        .await
        .context("内存向量检索任务执行失败")?;

        // S6: 搜索后 touch top-k 结果（更新 LRU 访问顺序）
        if !top_hits.is_empty() {
            let touch_keys: Vec<String> = top_hits.iter().map(|(id, _)| id.clone()).collect();
            if let Ok(mut guard) = self.vector_cache.write()
                && let Some(cache) = guard.as_mut()
            {
                cache.touch_batch(&touch_keys);
            }
        }

        if top_hits.is_empty() {
            return Ok(vec![]);
        }

        // 加载 top-k chunk 元数据（仅查 k 行，非全表扫描）
        let (ids, scores): (Vec<String>, Vec<f32>) = top_hits.into_iter().unzip();
        let mut results = self.get_chunks_by_ids(&ids).await?;
        // 按内存计算的分数更新 score（get_chunks_by_ids 不返回分数）
        for r in &mut results {
            if let Some(pos) = ids.iter().position(|id| id == &r.chunk.id) {
                r.score = scores[pos];
            }
        }
        // 确保按分数降序
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }

    async fn find_document_by_hash(&self, hash: &str) -> anyhow::Result<Option<Document>> {
        let pool = self.pool.clone();
        let hash = hash.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents WHERE file_hash = ?1 LIMIT 1"
            ))?;
            let mut rows = stmt.query_map(params![hash], row_to_document)?;
            match rows.next() {
                Some(row) => Ok(Some(row?)),
                None => Ok(None),
            }
        })
        .await
    }

    async fn count_documents(&self) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
                .context("统计文档数失败")?;
            Ok(count as usize)
        })
        .await
    }

    async fn count_chunks(&self) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
                .context("统计分块数失败")?;
            Ok(count as usize)
        })
        .await
    }

    async fn count_embeddings(&self) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))
                .context("统计向量数失败")?;
            Ok(count as usize)
        })
        .await
    }

    async fn cleanup_zombies(&self) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let affected = conn.execute(
                "UPDATE documents SET status = 'failed', status_reason = ?1 WHERE status = 'processing'",
                params!["崩溃恢复：上次会话中断"],
            )?;
            Ok(affected)
        })
        .await
    }

    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let encrypted = self.encrypt(value)?;
        let pool = self.pool.clone();
        let key = key.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, encrypted],
            )
            .context("写入设置项失败")?;
            Ok(())
        })
        .await
    }

    async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>> {
        let pool = self.pool.clone();
        let key = key.to_string();
        let encoded = run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
            let mut rows = stmt.query_map(params![key], |row| row.get::<_, String>(0))?;
            match rows.next() {
                Some(row) => Ok(Some(row?)),
                None => Ok(None),
            }
        })
        .await?;
        match encoded {
            Some(v) => Ok(Some(self.decrypt(&v)?)),
            None => Ok(None),
        }
    }

    /// 批量读取设置项：单次 SQL `WHERE key IN (...)` 替代 N 次串行 `get_setting`。
    ///
    /// 性能优化：`chat_inner` 原先串行调用 8+ 次 `get_setting`，每次触发独立的
    /// `spawn_blocking` + DB 连接获取 + SQL 执行。批量读取将 8 次往返压缩为 1 次。
    async fn get_settings_batch(&self, keys: &[&str]) -> anyhow::Result<Vec<(String, String)>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let pool = self.pool.clone();
        let keys: Vec<String> = keys.iter().map(|s| s.to_string()).collect();
        let encoded_pairs = run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let placeholders: String = (0..keys.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("SELECT key, value FROM settings WHERE key IN ({placeholders})");
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                keys.iter().map(|k| k as &dyn rusqlite::ToSql).collect();
            let rows = stmt.query_map(params.as_slice(), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut result = Vec::new();
            for row in rows {
                result.push(row?);
            }
            Ok(result)
        })
        .await?;
        // 解密每个值
        let mut results = Vec::with_capacity(encoded_pairs.len());
        for (key, encoded) in encoded_pairs {
            match self.decrypt(&encoded) {
                Ok(value) => results.push((key, value)),
                Err(e) => {
                    warn!("设置项 {key} 解密失败，跳过: {e}");
                }
            }
        }
        Ok(results)
    }

    async fn create_conversation(&self, conversation: &Conversation) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let c = conversation.clone();
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

    async fn list_conversations(&self, workspace_id: &str) -> anyhow::Result<Vec<Conversation>> {
        let pool = self.pool.clone();
        let workspace_id = workspace_id.to_string();
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

    async fn list_conversations_paginated(
        &self,
        workspace_id: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<Conversation>> {
        let pool = self.pool.clone();
        let workspace_id = workspace_id.to_string();
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

    async fn count_conversations(&self, workspace_id: &str) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        let workspace_id = workspace_id.to_string();
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

    async fn delete_conversation(&self, id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let id = id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 外键级联：messages 随会话一并清理（PRAGMA foreign_keys = ON）
            conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])
                .context("删除会话失败")?;
            Ok(())
        })
        .await
    }

    // ========================================================================
    // 工作空间管理（REQ-WS-001/003 多知识库）
    // ========================================================================

    async fn create_workspace(&self, workspace: &echomind_models::Workspace) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let ws = workspace.clone();
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

    async fn list_workspaces(&self) -> anyhow::Result<Vec<echomind_models::Workspace>> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at FROM workspaces ORDER BY created_at ASC, rowid ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(echomind_models::Workspace {
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

    async fn rename_workspace(&self, id: &str, name: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let id = id.to_string();
        let name = name.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let affected = conn
                .execute(
                    "UPDATE workspaces SET name = ?1 WHERE id = ?2",
                    params![name, id],
                )
                .context("重命名工作空间失败")?;
            if affected == 0 {
                bail!("工作空间不存在: {id}");
            }
            Ok(())
        })
        .await
    }

    async fn delete_workspace(&self, id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let id = id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let tx = conn.unchecked_transaction()?;

            // 1. 删除该工作空间的全部文档（级联清理 chunks/embeddings/entities 等）
            tx.execute(
                "DELETE FROM documents WHERE workspace_id = ?1",
                params![&id],
            )
            .context("删除工作空间文档失败")?;

            // 2. 删除该工作空间的全部会话（级联清理 messages）
            tx.execute(
                "DELETE FROM conversations WHERE workspace_id = ?1",
                params![&id],
            )
            .context("删除工作空间会话失败")?;

            // 3. 删除工作空间元数据行
            tx.execute("DELETE FROM workspaces WHERE id = ?1", params![&id])
                .context("删除工作空间元数据失败")?;

            tx.commit().context("提交工作空间删除事务失败")?;
            Ok(())
        })
        .await
    }

    async fn get_workspace_stats(
        &self,
        id: &str,
    ) -> anyhow::Result<echomind_models::WorkspaceStats> {
        let pool = self.pool.clone();
        let id = id.to_string();
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
            Ok(echomind_models::WorkspaceStats {
                document_count: doc_count as usize,
                conversation_count: conv_count as usize,
            })
        })
        .await
    }

    async fn count_documents_in_workspace(&self, workspace_id: &str) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        let workspace_id = workspace_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM documents WHERE workspace_id = ?1",
                    params![&workspace_id],
                    |row| row.get(0),
                )
                .context("统计工作空间文档数失败")?;
            Ok(count as usize)
        })
        .await
    }

    async fn list_documents_in_workspace(
        &self,
        workspace_id: &str,
    ) -> anyhow::Result<Vec<Document>> {
        let pool = self.pool.clone();
        let workspace_id = workspace_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents WHERE workspace_id = ?1 ORDER BY created_at DESC, rowid DESC"
            ))?;
            let rows = stmt.query_map(params![&workspace_id], row_to_document)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    /// 迁移文档到目标工作空间（REQ-WS-004 跨知识库迁移）。
    ///
    /// 事务 UPDATE `documents.workspace_id`，chunks / 向量等通过外键关联自动归属。
    async fn migrate_document(
        &self,
        doc_id: &str,
        target_workspace_id: &str,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        let target_ws = target_workspace_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "UPDATE documents SET workspace_id = ?1 WHERE id = ?2",
                params![&target_ws, &doc_id],
            )
            .context("迁移文档失败")?;
            Ok(())
        })
        .await
    }

    /// 按 ID 查找单个会话（REQ-EXP-001 导出功能）。
    /// 直接 SQL 查询，无需 workspace_id 过滤（会话 ID 全局唯一）。
    async fn get_conversation(&self, id: &str) -> anyhow::Result<Option<Conversation>> {
        let pool = self.pool.clone();
        let id = id.to_string();
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

    async fn update_conversation_title(&self, id: &str, title: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let id = id.to_string();
        let title = title.to_string();
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

    async fn reorder_conversations(&self, ordered_ids: &[String]) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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

    async fn add_message(
        &self,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
        let message = message.clone();
        run_db(move || {
            let sources_json = match &message.sources {
                Some(sources) => Some(serde_json::to_string(sources).context("序列化引用来源失败")?),
                None => None,
            };
            let reasoning = message.reasoning.clone();
            let turn_group = message.turn_group.clone().unwrap_or_default();
            let version = message.version.unwrap_or(1);
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, sources, reasoning, turn_group, version, created_at, security_tainted) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    conversation_id,
                    message.role,
                    message.content,
                    sources_json,
                    reasoning,
                    turn_group,
                    version,
                    chrono::Utc::now().timestamp(),
                    0, // security_tainted 默认 0（未标记）
                ],
            )
            .context("写入消息失败")?;
            Ok(())
        })
        .await
    }

    async fn list_messages(&self, conversation_id: &str) -> anyhow::Result<Vec<ChatMessage>> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
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

    async fn list_messages_paginated(
        &self,
        conversation_id: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<ChatMessage>> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // ORDER BY rowid DESC：从最新消息向前取；结果反转恢复正序（最旧在前）
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
            // 反转：DESC 查询返回最新在前，翻转为正序（最旧在前）
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

    async fn count_messages(&self, conversation_id: &str) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
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

    async fn delete_messages_by_ids(
        &self,
        conversation_id: &str,
        message_ids: &[String],
    ) -> anyhow::Result<usize> {
        if message_ids.is_empty() {
            return Ok(0);
        }
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
        let ids = message_ids.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 构建 IN (?, ?, ...) 占位符
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
            let param_refs: Vec<&dyn rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();
            let deleted = conn
                .execute(&sql, param_refs.as_slice())
                .context("批量删除消息失败")?;
            Ok(deleted)
        })
        .await
    }

    /// 首次编辑升级：把原始 user 消息行及紧随其后的 assistant 行原地标记为
    /// 指定 turn_group 的 version=1（REQ-QA 首次编辑分页）。
    ///
    /// 事务语义：user 行 + assistant 行同时升级，任一步失败整体 ROLLBACK。
    /// assistant 行定位规则：同一 conversation 中 rowid 大于 user 行 rowid、
    /// 且位于下一个 user 行之前的第一条 assistant 消息（保证属于同一问答对）。
    /// 幂等性：若原始行已属于同一 turn_group（重复调用/重试），UPDATE 幂等成功；
    /// 若属于其他 turn_group 则报错，防止误覆盖。
    async fn promote_original_turn(
        &self,
        conversation_id: &str,
        original_message_id: &str,
        turn_group: &str,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
        let original_message_id = original_message_id.to_string();
        let turn_group = turn_group.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            with_transaction(&conn, |conn| {
                // 1) 升级原始 user 行（未升级或已属于同一 turn_group 均可；属于其他 turn_group 报错）
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
                // 2) 升级紧随其后的 assistant 行（若存在；原问答未生成回答时跳过）。
                //    约束：assistant 行 rowid 必须大于 user 行，且小于下一个 user 行（同一问答对）。
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

    async fn set_turn_active_version(
        &self,
        conversation_id: &str,
        turn_group: &str,
        active_version: i32,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
        let turn_group = turn_group.to_string();
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

    async fn get_turn_active_versions(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<Vec<TurnActiveVersion>> {
        let pool = self.pool.clone();
        let conversation_id = conversation_id.to_string();
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

    async fn list_documents(&self) -> anyhow::Result<Vec<Document>> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents ORDER BY created_at DESC, rowid DESC"
            ))?;
            let rows = stmt.query_map([], row_to_document)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    async fn list_documents_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<Document>> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents ORDER BY created_at DESC, rowid DESC LIMIT ?1 OFFSET ?2"
            ))?;
            let rows = stmt.query_map(params![limit as i64, offset as i64], row_to_document)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 先清理 FTS5 索引（外键级联不触发虚拟表清理，需手动）
            conn.execute("DELETE FROM chunks_fts WHERE doc_id = ?1", params![doc_id])
                .context("清理 FTS5 索引失败")?;
            // 外键级联链：documents → chunks → embeddings（PRAGMA foreign_keys = ON）
            conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
                .context("删除文档失败")?;
            Ok(())
        })
        .await?;
        // **性能优化**：使内存向量缓存失效
        self.invalidate_vector_cache();
        Ok(())
    }

    async fn list_chunks(&self, doc_id: &str) -> anyhow::Result<Vec<Chunk>> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT id, doc_id, content, token_count, sequence FROM chunks WHERE doc_id = ?1 ORDER BY sequence ASC",
            )?;
            let rows = stmt.query_map(params![doc_id], |row| {
                Ok(Chunk {
                    id: row.get(0)?,
                    doc_id: row.get(1)?,
                    content: row.get(2)?,
                    token_count: row.get(3)?,
                    sequence: row.get(4)?,
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

    /// 删除指定文档的全部分块（外键级联自动清理 embeddings，REQ-VEC-005）。
    /// 文档记录本身保留，仅清理分块与向量数据，供重试索引重建。
    async fn delete_chunks_by_doc(&self, doc_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 先清理 FTS5 索引（虚拟表不受外键级联约束，需手动）
            conn.execute("DELETE FROM chunks_fts WHERE doc_id = ?1", params![doc_id])
                .context("清理 FTS5 索引失败")?;
            // 外键级联链：chunks → embeddings（PRAGMA foreign_keys = ON）
            conn.execute("DELETE FROM chunks WHERE doc_id = ?1", params![doc_id])
                .context("删除分块失败")?;
            Ok(())
        })
        .await
    }

    /// 关键词全文检索（FTS5 BM25 排序，REQ-RAG-010）。
    ///
    /// 使用 SQLite FTS5 trigram 分词器执行子串匹配，BM25 算法排序。
    /// 支持英文单词精确匹配和中日韩文本子串匹配。
    /// 空查询或无匹配时返回空 Vec，不返回 Err。
    ///
    /// **短查询回退**：trigram 分词器需要 ≥3 字符才能提取 trigram。
    /// 对于 <3 字符的查询（如中文 2 字词），回退为 SQL LIKE 全表扫描。
    async fn keyword_search(
        &self,
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        let trimmed = query.trim();
        if trimmed.is_empty() || top_k == 0 {
            return Ok(vec![]);
        }
        let pool = self.pool.clone();
        let query = trimmed.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // trigram 分词器要求 ≥3 字符；短查询回退为 LIKE 全表扫描
            if query.chars().count() < 3 {
                let pattern = format!("%{query}%");
                let mut stmt = conn.prepare(
                    "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, d.file_path
                     FROM chunks c
                     JOIN documents d ON d.id = c.doc_id
                     WHERE c.content LIKE ?1
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![pattern, top_k], |row| {
                    let file_path: String = row.get(5)?;
                    let doc_name = std::path::Path::new(&file_path)
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
                return Ok(results);
            }
            // FTS5 分词查询：将查询按空格/CJK 边界分词后逐词 OR 查询。
            //
            // 设计原因：之前将整个查询包裹为精确短语（"trampoline 是什么？"），
            // 导致文档中不存在该完整短语时返回 0 条结果——即使文档中含 "trampoline"。
            // 分词后逐词 OR 查询可让 "trampoline 是什么？" 匹配到含 "trampoline" 的 chunk。
            //
            // 安全：每个 token 包裹在双引号中转义，防 FTS5 操作符注入。
            let fts_query = build_fts5_or_query(&query);
            if fts_query.is_empty() {
                return Ok(vec![]);
            }
            let mut stmt = conn.prepare(
                "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, d.file_path
                 FROM chunks_fts fts
                 JOIN chunks c ON c.id = fts.chunk_id
                 JOIN documents d ON d.id = c.doc_id
                 WHERE chunks_fts MATCH ?1
                 ORDER BY bm25(chunks_fts)
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![fts_query, top_k], |row| {
                let file_path: String = row.get(5)?;
                let doc_name = std::path::Path::new(&file_path)
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
                    score: 1.0, // 关键词匹配 score=1.0，RRF 融合时仅依赖排名
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

    /// 对话全文搜索（REQ-RAG-040）。
    ///
    /// 使用 FTS5 trigram 分词器搜索 messages 表的 content 列。
    /// 返回按 BM25 分数降序排列的搜索结果，包含消息内容和所属会话标题。
    /// 空查询返回空列表，不返回 Err。
    ///
    /// **短查询回退**：trigram 分词器需要 ≥3 字符；对于 <3 字符的查询，
    /// 回退为 SQL LIKE 全表扫描（与 keyword_search 一致策略）。
    async fn search_messages(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<MessageSearchResult>> {
        let trimmed = query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let pool = self.pool.clone();
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
                    // bm25() 返回负值（越小越好），取负转为正值（越大越好）
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

    // ---- 嵌入缓存：按内容指纹去重（全尺度性能优化）----

    /// 按内容哈希查找缓存的嵌入向量。
    ///
    /// 命中则返回 `Some(Vec<f32>)`，未命中返回 `None`。
    /// 封装 `SqliteStorage::find_cached_embedding_sync` 为 async 接口。
    async fn lookup_embedding_cache(&self, content_hash: &str) -> anyhow::Result<Option<Vec<f32>>> {
        let pool = self.pool.clone();
        let hash = content_hash.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            SqliteStorage::find_cached_embedding_sync(&conn, &hash)
        })
        .await
    }

    /// 将嵌入向量写入缓存。
    ///
    /// 封装 `SqliteStorage::cache_embedding_sync` 为 async 接口。
    /// 使用 `INSERT OR IGNORE` — 并发场景下首次写入者胜出。
    async fn put_embedding_cache(
        &self,
        content_hash: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let hash = content_hash.to_string();
        let emb = embedding.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            SqliteStorage::cache_embedding_sync(&conn, &hash, &emb)
        })
        .await
    }

    /// 批量查找缓存的嵌入向量（性能优化：单次 DB 查询替代 N 次串行查询）。
    ///
    /// 使用临时表 + JOIN 实现批量查询，避免 SQLite 的 `IN (...)` 参数限制（SQLITE_MAX_VARIABLE_NUMBER=999）。
    /// 返回 `(batch_index, embedding)` 列表（仅命中项），batch_index 对应输入 hashes 中的位置。
    async fn lookup_embedding_cache_batch(
        &self,
        hashes: &[String],
    ) -> anyhow::Result<Vec<(usize, Vec<f32>)>> {
        if hashes.is_empty() {
            return Ok(Vec::new());
        }
        let pool = self.pool.clone();
        let hashes = hashes.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 使用临时表存储待查 hashes，避免 IN 子句参数限制
            with_transaction(&conn, |conn| {
                conn.execute(
                    "CREATE TEMP TABLE IF NOT EXISTS _batch_lookup (idx INTEGER PRIMARY KEY, hash TEXT NOT NULL)",
                    [],
                )?;
                conn.execute("DELETE FROM _batch_lookup", [])?;
                {
                    let mut stmt = conn.prepare(
                        "INSERT INTO _batch_lookup (idx, hash) VALUES (?1, ?2)",
                    )?;
                    for (i, hash) in hashes.iter().enumerate() {
                        stmt.execute(params![i as i64, hash])?;
                    }
                }
                Ok(())
            })?;

            let mut stmt = conn.prepare(
                "SELECT b.idx, e.embedding
                 FROM _batch_lookup b
                 JOIN embeddings_cache e ON e.content_hash = b.hash",
            )?;
            let rows = stmt.query_map([], |row| {
                let idx: i64 = row.get(0)?;
                let bytes: Vec<u8> = row.get(1)?;
                Ok((idx as usize, bytes))
            })?;
            let mut hits = Vec::new();
            for row in rows {
                let (idx, bytes) = row?;
                hits.push((idx, bytes_to_vec(&bytes)?));
            }
            // 清理临时表
            conn.execute("DELETE FROM _batch_lookup", []).ok();
            Ok(hits)
        })
        .await
    }

    /// 批量写入嵌入向量缓存（性能优化：单事务批量 INSERT）。
    ///
    /// 全部缓存项在单个事务中提交（1 次 spawn_blocking），替代逐条写入的 N 次往返。
    async fn put_embedding_cache_batch(&self, items: &[(String, Vec<f32>)]) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        // 预序列化：hash + f32→bytes 转换在 spawn_blocking 外完成
        let items: Vec<(String, Vec<u8>)> = items
            .iter()
            .map(|(hash, vec)| (hash.clone(), vec_to_bytes(vec)))
            .collect();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            with_transaction(&conn, |conn| {
                let mut stmt = conn
                    .prepare(
                        "INSERT OR IGNORE INTO embeddings_cache (content_hash, embedding) VALUES (?1, ?2)",
                    )
                    .context("预编译 embeddings_cache 批量写入语句失败")?;
                for (hash, bytes) in &items {
                    stmt.execute(params![hash, bytes])
                        .context("批量写入嵌入缓存失败")?;
                }
                Ok(())
            })
        })
        .await
    }

    /// 按源文件路径精确查找文档（REQ-SYNC-002 增量同步用）。
    ///
    /// 使用 `idx_documents_original_path` 索引加速查询，避免全表扫描。
    async fn find_document_by_original_path(&self, path: &str) -> anyhow::Result<Option<Document>> {
        let pool = self.pool.clone();
        let path = path.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents WHERE original_path = ?1 LIMIT 1"
            ))?;
            let mut rows = stmt.query_map(params![path], row_to_document)?;
            match rows.next() {
                Some(row) => Ok(Some(row?)),
                None => Ok(None),
            }
        })
        .await
    }

    /// 按源文件路径前缀查找文档（REQ-SYNC-002 增量同步用）。
    ///
    /// 使用 `idx_documents_original_path` 索引加速 LIKE 前缀查询。
    /// 用于同步文件夹时找出所有 `original_path` 以 `prefix` 开头的文档，
    /// 以检测哪些文件已被删除（在文件夹中不存在但在 DB 中存在）。
    async fn find_documents_by_original_path_prefix(
        &self,
        prefix: &str,
    ) -> anyhow::Result<Vec<Document>> {
        let pool = self.pool.clone();
        let prefix = prefix.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // LIKE 'prefix%' 可利用索引（前缀匹配），比默认实现的全表遍历高效
            let pattern = format!("{prefix}%");
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents WHERE original_path LIKE ?1"
            ))?;
            let rows = stmt.query_map(params![pattern], row_to_document)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    /// 更新文档领域分类标签（REQ-VEC-013 领域画像）。
    ///
    /// 将 `EmbeddingDomainClassifier` 的分类结果持久化到 `documents.domain` 列。
    /// 使用 `idx_documents_domain` 索引加速后续按领域筛选查询。
    async fn update_document_domain(&self, doc_id: &str, domain: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        let domain = domain.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "UPDATE documents SET domain = ?1 WHERE id = ?2",
                params![domain, doc_id],
            )
            .context("更新文档领域分类失败")?;
            Ok(())
        })
        .await
    }

    /// 更新文档摘要（REQ-ING-019 文档摘要自动生成）。
    ///
    /// 将 LLM 生成的摘要持久化到 `documents.summary` 列。
    /// 摘要在导入完成后异步生成，失败时保持 None（优雅降级）。
    async fn update_document_summary(&self, doc_id: &str, summary: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        let summary = summary.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "UPDATE documents SET summary = ?1 WHERE id = ?2",
                params![summary, doc_id],
            )
            .context("更新文档摘要失败")?;
            Ok(())
        })
        .await
    }

    // ------------------------------------------------------------------
    // 文档标签系统（REQ-ING-022 用户自定义标签管理）
    // ------------------------------------------------------------------

    /// 添加文档标签（REQ-ING-022）。
    ///
    /// 读取当前 tags JSON 数组 → 追加新标签（去重） → 写回。
    /// 标签为空字符串时直接返回 Ok（空标签无意义）。
    async fn add_document_tag(&self, doc_id: &str, tag: &str) -> anyhow::Result<()> {
        let tag = tag.trim().to_string();
        if tag.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let tags_json: String = conn
                .query_row(
                    "SELECT tags FROM documents WHERE id = ?1",
                    params![doc_id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "[]".to_string());
            let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            if !tags.iter().any(|t| t == &tag) {
                tags.push(tag);
                let new_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
                conn.execute(
                    "UPDATE documents SET tags = ?1 WHERE id = ?2",
                    params![new_json, doc_id],
                )
                .context("更新文档标签失败")?;
            }
            Ok(())
        })
        .await
    }

    /// 移除文档标签（REQ-ING-022）。
    ///
    /// 读取当前 tags JSON 数组 → 移除指定标签 → 写回。
    /// 标签不存在时幂等返回 Ok。
    async fn remove_document_tag(&self, doc_id: &str, tag: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        let tag = tag.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let tags_json: String = conn
                .query_row(
                    "SELECT tags FROM documents WHERE id = ?1",
                    params![doc_id],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| "[]".to_string());
            let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            let before_len = tags.len();
            tags.retain(|t| t != &tag);
            if tags.len() != before_len {
                let new_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
                conn.execute(
                    "UPDATE documents SET tags = ?1 WHERE id = ?2",
                    params![new_json, doc_id],
                )
                .context("更新文档标签失败")?;
            }
            Ok(())
        })
        .await
    }

    /// 列出所有文档标签（REQ-ING-022）。
    ///
    /// 全表扫描 documents.tags 列，解析 JSON 数组并统计每个标签的文档数。
    /// 返回 `(tag_name, count)` 列表，按计数降序排列。
    async fn list_all_tags(&self) -> anyhow::Result<Vec<(String, usize)>> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare("SELECT tags FROM documents")?;
            let rows = stmt.query_map([], |row| {
                let tags_json: String = row.get(0).unwrap_or_else(|_| "[]".to_string());
                let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
                Ok(tags)
            })?;
            // 统计每个标签的文档数
            let mut tag_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for row in rows {
                let tags = row?;
                for tag in tags {
                    *tag_counts.entry(tag).or_insert(0) += 1;
                }
            }
            // 按计数降序排列
            let mut result: Vec<(String, usize)> = tag_counts.into_iter().collect();
            result.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            Ok(result)
        })
        .await
    }

    /// 按标签筛选文档（REQ-ING-022）。
    ///
    /// 使用 SQL `WHERE tags LIKE` 查询包含指定标签的文档。
    /// LIKE 模式 `%"tag"%` 可匹配 JSON 数组中的标签值。
    async fn filter_documents_by_tag(&self, tag: &str) -> anyhow::Result<Vec<Document>> {
        let pool = self.pool.clone();
        // 使用 JSON 数组中的子串匹配：匹配 `"tag"` 模式
        let pattern = format!("\"{}\"", tag);
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(&format!(
                "SELECT {DOC_COLS} FROM documents WHERE tags LIKE ?1 ORDER BY created_at DESC, rowid DESC"
            ))?;
            let rows = stmt.query_map(params![format!("%{pattern}%")], row_to_document)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
        .await
    }

    /// 批量写入实体索引（REQ-PERF-006 实体链接增强）。
    ///
    /// 导入文档时抽取实体并批量写入 `entities` 表。
    /// 使用 `INSERT OR IGNORE` 避免重复实体导致唯一约束冲突。
    async fn add_entities(&self, entities: &[(String, String, String)]) -> anyhow::Result<()> {
        if entities.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let entities = entities.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：with_transaction 保证错误时自动 ROLLBACK
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

    /// 实体检索（REQ-PERF-006 实体链接增强）。
    ///
    /// 从查询中抽取实体后，在 `entities` 表中精确匹配，
    /// 返回包含匹配实体的 chunk 列表（按命中实体数降序排列）。
    async fn entity_search(
        &self,
        query_entities: &[String],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        if query_entities.is_empty() || top_k == 0 {
            return Ok(vec![]);
        }
        let pool = self.pool.clone();
        let entities = query_entities.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 构建 IN 占位符
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
                let doc_name = std::path::Path::new(&file_path)
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
                    score: 1.0, // 实体匹配 score=1.0，RRF 融合时仅依赖排名
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

    /// 重建 BM25 全文索引（REQ-PERF-005 Contextual BM25）。
    ///
    /// 清空 FTS5 索引后，使用 `build_contextual_text()` 拼接文档名前缀重建。
    /// 旧数据库升级到 Contextual BM25 时由用户通过 `rebuild_bm25_index` IPC 命令触发。
    async fn rebuild_bm25_index(&self) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：with_transaction 保证错误时自动 ROLLBACK
            with_transaction(&conn, |conn| {
                // 清空旧 FTS5 索引
                conn.execute_batch("DELETE FROM chunks_fts;")
                    .context("清空 FTS5 索引失败")?;
                // 使用 Contextual BM25 重建
                conn.execute_batch(
                    "INSERT INTO chunks_fts (chunk_id, doc_id, content)
                     SELECT c.id, c.doc_id,
                       '文档《' ||
                       COALESCE(substr(d.file_path, instr(replace(d.file_path, '\\', '/'), '/') + 1), d.file_path)
                       || '》：\n' || c.content
                     FROM chunks c
                     JOIN documents d ON d.id = c.doc_id;",
                )
                .context("重建 FTS5 索引失败")?;
                Ok(())
            })
        })
        .await
    }

    // ------------------------------------------------------------------
    // Proposition 级原子分割（REQ-PERF-007）
    // ------------------------------------------------------------------

    /// 批量写入 proposition 索引（REQ-PERF-007）。
    async fn add_propositions(&self, propositions: &[Proposition]) -> anyhow::Result<()> {
        if propositions.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let props = propositions.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：with_transaction 保证错误时自动 ROLLBACK
            with_transaction(&conn, |conn| {
                let mut stmt = conn
                    .prepare(
                        "INSERT OR REPLACE INTO propositions (id, chunk_id, content, sequence) \
                         VALUES (?1, ?2, ?3, ?4)",
                    )
                    .context("预编译 propositions 批量写入语句失败")?;
                for prop in &props {
                    stmt.execute(params![
                        prop.id,
                        prop.chunk_id,
                        prop.content,
                        prop.sequence as i64
                    ])
                    .context("批量写入 propositions 失败")?;
                }
                Ok(())
            })
        })
        .await
    }

    /// 批量写入 proposition 嵌入向量（REQ-PERF-007）。
    async fn add_proposition_embeddings(
        &self,
        embeddings: &[(String, Vec<f32>)],
    ) -> anyhow::Result<()> {
        if embeddings.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let embeddings = embeddings.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：with_transaction 保证错误时自动 ROLLBACK
            with_transaction(&conn, |conn| {
                let mut stmt = conn
                    .prepare("UPDATE propositions SET embedding = ?1 WHERE id = ?2")
                    .context("预编译 proposition 嵌入更新语句失败")?;
                for (id, vector) in &embeddings {
                    let bytes = vec_to_bytes(vector);
                    stmt.execute(params![bytes, id])
                        .context("更新 proposition 嵌入失败")?;
                }
                Ok(())
            })
        })
        .await
    }

    /// 列出文档的所有 proposition（REQ-PERF-007）。
    async fn list_propositions_by_doc(&self, doc_id: &str) -> anyhow::Result<Vec<Proposition>> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT p.id, p.chunk_id, p.content, p.sequence \
                 FROM propositions p \
                 JOIN chunks c ON c.id = p.chunk_id \
                 WHERE c.doc_id = ?1 \
                 ORDER BY c.sequence, p.sequence",
            )?;
            let rows = stmt.query_map(params![doc_id], |row| {
                Ok(Proposition {
                    id: row.get(0)?,
                    chunk_id: row.get(1)?,
                    content: row.get(2)?,
                    sequence: row.get::<_, i64>(3)? as usize,
                })
            })?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
        .await
    }

    /// Proposition 向量检索（REQ-PERF-007）。
    ///
    /// 在 proposition 嵌入表上执行 top-k 余弦相似度检索，
    /// 返回命中的 proposition 对应的 chunk（已去重）。
    async fn proposition_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }
        let pool = self.pool.clone();
        let query = query_embedding.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT p.id, p.chunk_id, p.content, p.sequence, p.embedding, \
                 c.id, c.doc_id, c.content, c.token_count, c.sequence, \
                 d.file_path \
                 FROM propositions p \
                 JOIN chunks c ON c.id = p.chunk_id \
                 JOIN documents d ON d.id = c.doc_id \
                 WHERE p.embedding IS NOT NULL",
            )?;
            let rows = stmt.query_map([], |row| {
                let prop_id: String = row.get(0)?;
                let prop_embedding: Vec<u8> = row.get(4)?;
                Ok((
                    prop_id,
                    Chunk {
                        id: row.get(5)?,
                        doc_id: row.get(6)?,
                        content: row.get(7)?,
                        token_count: row.get(8)?,
                        sequence: row.get::<_, i64>(9)? as usize,
                    },
                    row.get::<_, String>(10)?, // file_path
                    prop_embedding,
                ))
            })?;

            // 计算每个 proposition 的余弦相似度，取每个 chunk 的最高分
            use std::collections::HashMap;
            let mut best_per_chunk: HashMap<String, RetrievalResult> = HashMap::new();
            for row in rows {
                let (_prop_id, chunk, file_path, bytes) = row?;
                let vector = bytes_to_vec(&bytes)?;
                let score = cosine_similarity(&query, &vector);
                let doc_name = Path::new(&file_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_path.clone());
                // 每个 chunk 只保留最高分的 proposition
                best_per_chunk
                    .entry(chunk.id.clone())
                    .and_modify(|existing| {
                        if score > existing.score {
                            existing.score = score;
                        }
                    })
                    .or_insert_with(|| RetrievalResult {
                        chunk: chunk.clone(),
                        score,
                        doc_name: doc_name.clone(),
                    });
            }

            // 降序排序，截取 top_k
            let mut all_hits: Vec<RetrievalResult> = best_per_chunk.into_values().collect();
            all_hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            all_hits.truncate(top_k);
            Ok(all_hits)
        })
        .await
    }

    /// 重建 proposition 索引（REQ-PERF-007）。
    ///
    /// 清空 propositions 表后，遍历所有 chunk → 分割为 proposition → 重新写入。
    /// 注意：此操作仅重建 proposition 内容，不计算嵌入向量。
    /// 嵌入向量需要由嵌入管线单独计算（通过 `add_proposition_embeddings`）。
    async fn rebuild_proposition_index(&self) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：with_transaction 保证错误时自动 ROLLBACK
            with_transaction(&conn, |conn| {
                // 清空旧 propositions
                conn.execute_batch("DELETE FROM propositions;")
                    .context("清空 propositions 表失败")?;

                // 查询所有 chunk + doc_name
                let mut stmt = conn.prepare(
                    "SELECT c.id, c.doc_id, c.content, c.sequence, d.file_path \
                     FROM chunks c \
                     JOIN documents d ON d.id = c.doc_id \
                     ORDER BY c.doc_id, c.sequence",
                )?;
                let chunks: Vec<(String, String, String, usize, String)> = stmt
                    .query_map([], |row| {
                        Ok((
                            row.get(0)?,                    // chunk_id
                            row.get(1)?,                    // doc_id
                            row.get(2)?,                    // content
                            row.get::<_, i64>(3)? as usize, // sequence
                            row.get(4)?,                    // file_path
                        ))
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                // 为每个 chunk 分割 proposition 并写入
                let mut insert_stmt = conn
                    .prepare(
                        "INSERT INTO propositions (id, chunk_id, content, sequence) \
                         VALUES (?1, ?2, ?3, ?4)",
                    )
                    .context("预编译 proposition 写入语句失败")?;

                for (chunk_id, _doc_id, content, _seq, file_path) in &chunks {
                    let doc_name = Path::new(file_path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file_path.clone());
                    let propositions = PropositionSplitter::split(content, chunk_id, &doc_name);
                    for prop in propositions {
                        insert_stmt
                            .execute(params![
                                prop.id,
                                prop.chunk_id,
                                prop.content,
                                prop.sequence as i64
                            ])
                            .context("写入 proposition 失败")?;
                    }
                }
                Ok(())
            })
        })
        .await
    }

    // ------------------------------------------------------------------
    // RAPTOR 摘要树（REQ-PERF-009）
    // ------------------------------------------------------------------

    /// 批量写入摘要树节点（REQ-PERF-009）。
    async fn add_summary_nodes(&self, nodes: &[SummaryNode]) -> anyhow::Result<()> {
        if nodes.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let nodes = nodes.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // Bug #1 修复：with_transaction 保证错误时自动 ROLLBACK
            with_transaction(&conn, |conn| {
                let mut stmt = conn
                    .prepare(
                        "INSERT OR REPLACE INTO summary_nodes (id, doc_id, level, content, child_ids, embedding) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    )
                    .context("预编译 summary_nodes 批量写入语句失败")?;
                for node in &nodes {
                    let child_ids_json = serde_json::to_string(&node.child_ids)
                        .context("序列化 child_ids 失败")?;
                    let embedding_bytes = node.embedding.as_ref().map(|v| vec_to_bytes(v));
                    stmt.execute(params![
                        node.id,
                        node.doc_id,
                        node.level as i64,
                        node.content,
                        child_ids_json,
                        embedding_bytes,
                    ])
                    .context("批量写入 summary_nodes 失败")?;
                }
                Ok(())
            })
        })
        .await
    }

    /// 更新摘要节点嵌入向量（REQ-PERF-009）。
    async fn update_summary_embedding(
        &self,
        node_id: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let node_id = node_id.to_string();
        let embedding = embedding.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let bytes = vec_to_bytes(&embedding);
            conn.execute(
                "UPDATE summary_nodes SET embedding = ?1 WHERE id = ?2",
                params![bytes, node_id],
            )
            .context("更新 summary_node 嵌入失败")?;
            Ok(())
        })
        .await
    }

    /// 列出文档的所有摘要节点（REQ-PERF-009）。
    async fn list_summary_nodes(&self, doc_id: &str) -> anyhow::Result<Vec<SummaryNode>> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT id, doc_id, level, content, child_ids, embedding \
                 FROM summary_nodes \
                 WHERE doc_id = ?1 \
                 ORDER BY level",
            )?;
            let rows = stmt.query_map(params![doc_id], |row| {
                let child_ids_json: String = row.get(4)?;
                let child_ids: Vec<String> =
                    serde_json::from_str(&child_ids_json).unwrap_or_default();
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.as_ref().and_then(|b| bytes_to_vec(b).ok());
                Ok(SummaryNode {
                    id: row.get(0)?,
                    doc_id: row.get(1)?,
                    level: row.get::<_, i64>(2)? as usize,
                    content: row.get(3)?,
                    child_ids,
                    embedding,
                })
            })?;
            let mut nodes = Vec::new();
            for row in rows {
                nodes.push(row?);
            }
            Ok(nodes)
        })
        .await
    }

    /// 摘要树向量检索（REQ-PERF-009）。
    async fn summary_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        let pool = self.pool.clone();
        let query_vec = query_embedding.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;

            // 全表扫描 + Rust 余弦相似度计算（与 vector_search 一致）
            let mut stmt = conn.prepare(
                "SELECT sn.id, sn.doc_id, sn.content, sn.child_ids, sn.embedding, d.file_path \
                 FROM summary_nodes sn \
                 JOIN documents d ON d.id = sn.doc_id \
                 WHERE sn.embedding IS NOT NULL",
            )?;
            let rows = stmt.query_map([], |row| {
                let child_ids_json: String = row.get(3)?;
                let _child_ids: Vec<String> =
                    serde_json::from_str(&child_ids_json).unwrap_or_default();
                let embedding_blob: Vec<u8> = row.get(4)?;
                let doc_embedding = bytes_to_vec(&embedding_blob).unwrap_or_default();
                let doc_name: String = row.get(5)?;
                let content: String = row.get(2)?;
                let node_id: String = row.get(0)?;
                Ok((node_id, doc_name, content, doc_embedding))
            })?;

            let mut hits: Vec<RetrievalResult> = Vec::new();
            for row in rows {
                let (node_id, doc_name, content, doc_embedding) = row?;
                if doc_embedding.is_empty() {
                    continue;
                }
                let score = cosine_similarity(&query_vec, &doc_embedding);
                // 摘要节点作为 "chunk" 返回（复用 RetrievalResult 结构）
                hits.push(RetrievalResult {
                    chunk: echomind_models::Chunk {
                        id: node_id,
                        doc_id: String::new(), // 摘要节点的 doc_id 不直接返回
                        content,
                        token_count: 0,
                        sequence: 0,
                    },
                    score,
                    doc_name,
                });
            }

            // 降序排序，截取 top_k
            hits.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            hits.truncate(top_k);
            Ok(hits)
        })
        .await
    }

    /// 重建摘要树索引（REQ-PERF-009）：清空 summary_nodes 表。
    async fn rebuild_summary_tree(&self) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute_batch("DELETE FROM summary_nodes;")
                .context("清空 summary_nodes 表失败")?;
            Ok(())
        })
        .await
    }

    // ------------------------------------------------------------------
    // 实体关系图谱（REQ-RAG-026 知识图谱实体关系检索）
    // ------------------------------------------------------------------

    /// 写入单条实体关系（REQ-RAG-026）。
    async fn add_relation(&self, relation: &EntityRelation) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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

    /// 批量写入实体关系（REQ-RAG-026）。
    async fn add_relations_batch(&self, relations: &[EntityRelation]) -> anyhow::Result<()> {
        if relations.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
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

    /// 查询指定实体参与的所有关系（REQ-RAG-026 图遍历）。
    ///
    /// 返回 subject 或 object 等于 `entity_text` 的所有关系。
    async fn get_relations_for_entity(
        &self,
        entity_text: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        let pool = self.pool.clone();
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

    /// 查询指定 chunk 的所有关系（REQ-RAG-026）。
    async fn get_relations_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<EntityRelation>> {
        let pool = self.pool.clone();
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

    /// 按主体 + 关系类型查询关系（REQ-RAG-026）。
    async fn search_by_relation(
        &self,
        subject: &str,
        relation_type: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        let pool = self.pool.clone();
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

    /// 按 chunk ID 查找单个 chunk（REQ-RAG-027 图遍历检索用）。
    async fn get_chunk_by_id(&self, chunk_id: &str) -> anyhow::Result<Option<Chunk>> {
        let pool = self.pool.clone();
        let chunk_id = chunk_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT id, doc_id, content, token_count, sequence \
                 FROM chunks WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map(params![chunk_id], |row| {
                Ok(Chunk {
                    id: row.get(0)?,
                    doc_id: row.get(1)?,
                    content: row.get(2)?,
                    token_count: row.get(3)?,
                    sequence: row.get(4)?,
                })
            })?;
            match rows.next() {
                Some(row) => Ok(Some(row?)),
                None => Ok(None),
            }
        })
        .await
    }

    /// 分页查询全部实体关系（REQ-RAG-027 前端图谱可视化用）。
    ///
    /// 返回 `entity_relations` 表中前 `limit` 条记录（跳过 `offset` 条），
    /// 按 `id` 排序保证分页稳定性。
    async fn list_all_relations(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        let pool = self.pool.clone();
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

    /// 统计实体关系总数（REQ-RAG-027 前端图谱可视化用）。
    async fn count_relations(&self) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM entity_relations", [], |row| {
                    row.get(0)
                })?;
            Ok(count as usize)
        })
        .await
    }

    /// 批量查询实体类型（REQ-RAG-027 前端图谱可视化增强）。
    ///
    /// 从 `entities` 表批量查询指定实体文本列表的 `entity_type`，
    /// 返回 `HashMap<entity_text, entity_type>` 映射。
    /// 使用 SQL `WHERE entity_text IN (...)` 单次查询，避免 N+1 问题。
    async fn get_entity_types(
        &self,
        entities: &[String],
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        if entities.is_empty() {
            return Ok(std::collections::HashMap::new());
        }
        let pool = self.pool.clone();
        let entities = entities.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 构建 IN 占位符
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

    /// 获取全量实体邻接表（REQ-RAG-027 Session 5 高级分析用）。
    ///
    /// 一次 SQL 查询返回全量邻接表 `HashMap<entity, Vec<neighbor>>`，
    /// 供 GraphAnalyzer 在后端内存中完成路径分析、社区检测等高级分析。
    ///
    /// 邻接表构建逻辑：
    /// - 遍历 `entity_relations` 表所有记录
    /// - 对每条 `(subject, object)` 关系，同时添加 subject→object 和 object→subject（无向图）
    /// - 去重邻居列表（同一条边不重复添加）
    async fn get_entity_graph(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare("SELECT subject, object FROM entity_relations")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;

            let mut adjacency: std::collections::HashMap<
                String,
                std::collections::HashSet<String>,
            > = std::collections::HashMap::new();
            for row in rows {
                let (subject, object) = row?;
                // 无向图：双向添加
                adjacency
                    .entry(subject.clone())
                    .or_default()
                    .insert(object.clone());
                adjacency.entry(object).or_default().insert(subject);
            }

            // HashSet → Vec 转换
            let result: std::collections::HashMap<String, Vec<String>> = adjacency
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().collect()))
                .collect();
            Ok(result)
        })
        .await
    }

    // ------------------------------------------------------------------
    // 代码符号索引（REQ-RAG-031 代码感知 RAG）
    // ------------------------------------------------------------------

    /// 批量写入代码符号索引（REQ-RAG-031）。
    async fn add_symbols(&self, symbols: &[CodeSymbol]) -> anyhow::Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
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

    /// 按符号名精确搜索（REQ-RAG-031）。
    async fn search_by_symbol(
        &self,
        name: &str,
        kind: Option<&SymbolKind>,
    ) -> anyhow::Result<Vec<CodeSymbol>> {
        let pool = self.pool.clone();
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
            // 如果有 kind 过滤，在 Rust 侧过滤（避免 SQL 参数复杂性）
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

    /// 获取指定 chunk 的所有符号（REQ-RAG-031）。
    async fn get_symbols_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<CodeSymbol>> {
        let pool = self.pool.clone();
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

    /// 模糊搜索符号（REQ-RAG-031）。
    async fn search_symbols_fuzzy(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CodeSymbol>> {
        if query.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let pool = self.pool.clone();
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

    // ============================================================
    // 对话记忆系统（REQ-RAG-032）
    // ============================================================

    fn add_memory_entry(
        &self,
        entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        let entry = entry.clone();
        let pool = self.pool.clone();
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

    fn get_memory_entries(
        &self,
        tier: Option<&MemoryTier>,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
        let tier_str = tier.map(|t| t.as_str().to_string());
        let pool = self.pool.clone();
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

    fn update_memory_entry(
        &self,
        entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        let entry = entry.clone();
        let pool = self.pool.clone();
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

    fn delete_memory_entry(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        let id = id.to_string();
        let pool = self.pool.clone();
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

    fn clear_memory_entries(
        &self,
        tier: Option<&MemoryTier>,
    ) -> impl std::future::Future<Output = anyhow::Result<usize>> + Send {
        let tier_str = tier.map(|t| t.as_str().to_string());
        let pool = self.pool.clone();
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

    fn search_memory_entries(
        &self,
        query: &str,
        limit: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
        let query = format!("%{query}%");
        let limit = limit as i64;
        let pool = self.pool.clone();
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

    // ------------------------------------------------------------------
    // Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接）
    // ------------------------------------------------------------------

    /// 批量写入 wiki-link 索引（REQ-ING-020）。
    async fn add_wiki_links(&self, links: &[WikiLink]) -> anyhow::Result<()> {
        if links.is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
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

    /// 查询文档的正向链接（REQ-ING-020）。
    async fn get_forward_links(&self, doc_id: &str) -> anyhow::Result<Vec<WikiLink>> {
        let pool = self.pool.clone();
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

    /// 查询文档的反向链接（REQ-ING-020）。
    async fn get_backlinks(&self, doc_name: &str) -> anyhow::Result<Vec<WikiLink>> {
        let pool = self.pool.clone();
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

    // ------------------------------------------------------------------
    // Durable Prompt Admission（B05 持久化提示接纳）
    // ------------------------------------------------------------------

    /// 接纳用户输入（B05 Durable Prompt Admission）。
    async fn admit_input(
        &self,
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
        let pool = self.pool.clone();
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

    /// 提升接纳记录为正式消息（B05 Durable Prompt Admission）。
    async fn promote_input(&self, input_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let input_id = input_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 使用当前时间戳作为 promoted_seq（标记提升时刻）
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

    /// 获取会话的待处理输入列表（B05 Durable Prompt Admission）。
    async fn get_pending_inputs(&self, conversation_id: &str) -> anyhow::Result<Vec<PendingInput>> {
        let pool = self.pool.clone();
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

    // ------------------------------------------------------------------
    // Scratch-Promote 记忆整合（Q01 借鉴 QM scratch-promote + consolidation）
    // ------------------------------------------------------------------

    /// 追加一条 scratch 日志条目（Q01）。
    async fn add_scratch_log(&self, entry: &ScratchLogEntry) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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
    async fn get_scratch_logs(&self, limit: Option<usize>) -> anyhow::Result<Vec<ScratchLogEntry>> {
        let pool = self.pool.clone();
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
    async fn delete_scratch_log(&self, id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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
    async fn cleanup_expired_scratch_logs(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
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

    // ------------------------------------------------------------------
    // 幂等性存储支持（Q07 幂等性存储）
    // ------------------------------------------------------------------

    /// 记录幂等性操作（Q07）。
    async fn record_idempotency(&self, key: &str, timestamp: i64) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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
    async fn list_idempotency_records(&self) -> anyhow::Result<Vec<(String, i64)>> {
        let pool = self.pool.clone();
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
    async fn cleanup_expired_idempotency(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        let pool = self.pool.clone();
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

    // ------------------------------------------------------------------
    // Session Todo 持久化（B08 会话待办持久化）
    // ------------------------------------------------------------------

    /// 创建 Todo 项（B08 Session Todo 持久化）。
    async fn add_session_todo(&self, todo: &SessionTodo) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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

    /// 更新 Todo 状态（B08 Session Todo 持久化）。
    async fn update_todo_status(&self, todo_id: &str, status: &TodoStatus) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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

    /// 获取会话的 Todo 列表（B08 Session Todo 持久化），按 position 升序排列。
    async fn get_session_todos(&self, conversation_id: &str) -> anyhow::Result<Vec<SessionTodo>> {
        let pool = self.pool.clone();
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

    /// 删除单个 Todo 项（B08 Session Todo 持久化）。
    async fn delete_session_todo(&self, todo_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let todo_id = todo_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute("DELETE FROM session_todos WHERE id = ?1", params![&todo_id])
                .context("删除 session_todo 失败")?;
            Ok(())
        })
        .await
    }

    /// 删除会话的全部 Todo 项（B08 Session Todo 持久化）。
    async fn delete_session_todos(&self, conversation_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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

    // --- Security-Tainted 条目标记（Q05）---

    async fn set_entry_security_tainted(
        &self,
        message_id: &str,
        tainted: bool,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let message_id = message_id.to_string();
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

    async fn get_entry_security_tainted(&self, message_id: &str) -> anyhow::Result<bool> {
        let pool = self.pool.clone();
        let message_id = message_id.to_string();
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

    async fn record_budget_usage(
        &self,
        principal: &str,
        input_tokens: usize,
        output_tokens: usize,
        cost_usd: f64,
        model_name: &str,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
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

    async fn get_budget_stats(&self, principal: &str) -> anyhow::Result<BudgetStats> {
        let pool = self.pool.clone();
        let principal = principal.to_string();

        // Get daily limit from settings table
        let daily_limit = self
            .get_setting("budget.daily_limit_usd")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);

        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;

            // Calculate total spending in the last 24 hours
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

    async fn set_budget_limit(&self, principal: &str, daily_limit_usd: f64) -> anyhow::Result<()> {
        let key = format!("budget.daily_limit_usd.{}", principal);
        self.set_setting(&key, &daily_limit_usd.to_string()).await
    }

    // ------------------------------------------------------------------
    // 导入历史记录（REQ-ING-011）
    // ------------------------------------------------------------------

    async fn add_import_log(
        &self,
        file_name: &str,
        format: &str,
        result: &str,
        error_message: Option<&str>,
        file_size: Option<i64>,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let file_name = file_name.to_string();
        let format = format.to_string();
        let result = result.to_string();
        let error_message = error_message.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO import_logs (timestamp, file_name, format, result, error_message, file_size) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![now, &file_name, &format, &result, &error_message, file_size],
            )?;
            // 淘汰最旧记录（保留最近 100 条）
            conn.execute(
                "DELETE FROM import_logs WHERE id NOT IN (SELECT id FROM import_logs ORDER BY id DESC LIMIT 100)",
                [],
            )?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }

    async fn get_import_logs(
        &self,
        result_filter: Option<&str>,
    ) -> anyhow::Result<Vec<echomind_models::ImportLogEntry>> {
        let pool = self.pool.clone();
        let filter = result_filter.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut entries = Vec::new();
            if let Some(ref f) = filter {
                let mut stmt = conn.prepare(
                    "SELECT id, timestamp, file_name, format, result, error_message, file_size FROM import_logs WHERE result = ?1 ORDER BY id DESC LIMIT 100",
                )?;
                let rows = stmt.query_map(rusqlite::params![f], |row| {
                    Ok(echomind_models::ImportLogEntry {
                        id: row.get(0)?,
                        timestamp: row.get(1)?,
                        file_name: row.get(2)?,
                        format: row.get(3)?,
                        result: row.get(4)?,
                        error_message: row.get(5)?,
                        file_size: row.get(6)?,
                    })
                })?;
                for row in rows {
                    entries.push(row?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, timestamp, file_name, format, result, error_message, file_size FROM import_logs ORDER BY id DESC LIMIT 100",
                )?;
                let rows = stmt.query_map([], |row| {
                    Ok(echomind_models::ImportLogEntry {
                        id: row.get(0)?,
                        timestamp: row.get(1)?,
                        file_name: row.get(2)?,
                        format: row.get(3)?,
                        result: row.get(4)?,
                        error_message: row.get(5)?,
                        file_size: row.get(6)?,
                    })
                })?;
                for row in rows {
                    entries.push(row?);
                }
            }
            Ok(entries)
        })
        .await?
    }

    async fn clear_import_logs(&self) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            conn.execute("DELETE FROM import_logs", [])?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }

    // ------------------------------------------------------------------
    // 对话书签（REQ-RAG-047）
    // ------------------------------------------------------------------

    /// 添加对话书签（REQ-RAG-047 AC-1/AC-2）。
    async fn add_bookmark(&self, conversation_id: &str, note: Option<&str>) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let conv_id = conversation_id.to_string();
        let note_val = note.map(|s| s.to_string());
        let now = chrono::Utc::now().timestamp();
        tokio::task::spawn_blocking(move || {
let conn = pool.get()?;
conn.execute(
"INSERT OR REPLACE INTO conversation_bookmarks (conversation_id, note, created_at) VALUES (?1, ?2, ?3)",
params![conv_id, note_val, now],
)
.context("写入 conversation_bookmarks 失败")?;
Ok::<_, anyhow::Error>(())
})
.await??;
        Ok(())
    }

    /// 移除对话书签（REQ-RAG-047 AC-5）。
    async fn remove_bookmark(&self, conversation_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let conv_id = conversation_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            conn.execute(
                "DELETE FROM conversation_bookmarks WHERE conversation_id = ?1",
                params![conv_id],
            )
            .context("删除 conversation_bookmarks 失败")?;
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        Ok(())
    }

    /// 列出全部书签（REQ-RAG-047 AC-3/AC-4）。
    async fn list_bookmarks(&self) -> anyhow::Result<Vec<echomind_models::ConversationBookmark>> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let mut stmt = conn.prepare(
"SELECT conversation_id, note, created_at FROM conversation_bookmarks ORDER BY created_at DESC",
).context("准备 conversation_bookmarks 查询失败")?;
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

    /// 检查指定会话是否已加书签（REQ-RAG-047 AC-2）。
    async fn is_bookmarked(&self, conversation_id: &str) -> anyhow::Result<bool> {
        let pool = self.pool.clone();
        let conv_id = conversation_id.to_string();
        let count: i64 = tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM conversation_bookmarks WHERE conversation_id = ?1",
                    params![conv_id],
                    |row| row.get(0),
                )
                .context("查询 conversation_bookmarks COUNT 失败")?;
            Ok::<_, anyhow::Error>(count)
        })
        .await??;
        Ok(count > 0)
    }
}

/// 构建 FTS5 OR 查询字符串：将查询分词后逐词包裹为短语，用 OR 连接。
///
/// 设计原因：FTS5 trigram 分词器对整句短语匹配过于严格——
/// `"trampoline 是什么？"` 只匹配包含该完整短语的文档，导致含 "trampoline"
/// 但不含整句的 chunk 无法被检索到。改为逐词 OR 查询后，任一 token 命中即可返回。
///
/// 分词策略：
/// - 英文/数字：按空格分割为单词
/// - 中日韩（CJK）：逐字符提取（trigram 分词器以 3 字符为最小匹配单位，
///   单个 CJK 字符也作为 token 参与匹配，因 trigram 会将其与相邻字符组合）
/// - 标点/停用词：过滤掉长度 <1 的 token
///
/// 安全：每个 token 用双引号包裹并转义内部双引号，防止 FTS5 操作符注入
/// （如 `*`、`NEAR`、`AND`、`OR`、`NOT` 被当作普通字符串而非操作符）。
fn build_fts5_or_query(query: &str) -> String {
    let mut tokens: Vec<String> = Vec::new();

    for segment in query.split_whitespace() {
        // 保留字母数字、下划线、连字符（编程标识符常见字符），
        // 以及 CJK 字符。过滤标点和 FTS5 语法字符。
        let cleaned: String = segment
            .chars()
            .filter(|c| {
                c.is_alphanumeric()
                    || *c == '_'
                    || *c == '-'
                    || ('\u{4e00}'..='\u{9fff}').contains(c)
                    || ('\u{3040}'..='\u{30ff}').contains(c)
            })
            .collect();
        if !cleaned.is_empty() {
            tokens.push(cleaned);
        }
    }

    // 纯 CJK 无空格场景：trigram 分词器需要 ≥3 字符才能提取 trigram，
    // 所以将 CJK 文本拆分为 3 字符滑窗作为 token。
    if tokens.len() == 1 && tokens[0].chars().count() > 3 {
        let token = tokens[0].clone();
        let has_cjk = token.chars().any(|c| {
            ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3040}'..='\u{30ff}').contains(&c)
        });
        if has_cjk {
            let chars: Vec<char> = token.chars().collect();
            tokens.clear();
            for window in chars.windows(3) {
                let s: String = window.iter().collect();
                if !s.is_empty() {
                    tokens.push(s);
                }
            }
            // 如果原文不足 3 字符，回退为完整 token
            if tokens.is_empty() && !chars.is_empty() {
                tokens.push(token);
            }
        }
    }

    // 去重 + 转义 + OR 连接
    let mut seen = std::collections::HashSet::new();
    let mut parts: Vec<String> = Vec::new();
    for token in &tokens {
        if seen.insert(token.clone()) {
            let escaped = token.replace('"', "\"\"");
            parts.push(format!("\"{escaped}\""));
        }
    }

    parts.join(" OR ")
}

// ============================================================================
// AuditLogger trait 实现（SqliteStorage 作为审计日志持久化适配器）
// ============================================================================

impl AuditLogger for SqliteStorage {
    fn log<'a>(
        &'a self,
        entry: AuditEntry,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let conn = pool.get().context("获取数据库连接失败")?;
                conn.execute(
                    "INSERT INTO audit_log (id, action, details, pii_count, timestamp, prev_hash, curr_hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        entry.id,
                        entry.action,
                        entry.details,
                        entry.pii_count as i64,
                        entry.timestamp,
                        entry.prev_hash,
                        entry.curr_hash,
                    ],
                )
                .context("写入审计日志失败")?;
                Ok(())
            })
            .await
            .context("审计日志写入任务失败")?
        })
    }

    fn list_entries<'a>(
        &'a self,
        limit: usize,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<AuditEntry>>> + Send + 'a>,
    > {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let conn = pool.get().context("获取数据库连接失败")?;
                let mut stmt = conn
                    .prepare(
                        "SELECT id, action, details, pii_count, timestamp, prev_hash, curr_hash
                         FROM audit_log ORDER BY timestamp DESC LIMIT ?1",
                    )
                    .context("准备审计日志查询失败")?;
                let entries = stmt
                    .query_map(params![limit as i64], |row| {
                        Ok(AuditEntry {
                            id: row.get(0)?,
                            action: row.get(1)?,
                            details: row.get(2)?,
                            pii_count: row.get::<_, i64>(3)? as usize,
                            timestamp: row.get(4)?,
                            prev_hash: row.get(5)?,
                            curr_hash: row.get(6)?,
                        })
                    })
                    .context("查询审计日志失败")?;
                let mut result = Vec::new();
                for entry in entries {
                    result.push(entry.context("解析审计日志行失败")?);
                }
                Ok(result)
            })
            .await
            .context("审计日志查询任务失败")?
        })
    }

    fn clear<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let conn = pool.get().context("获取数据库连接失败")?;
                conn.execute("DELETE FROM audit_log", [])
                    .context("清空审计日志失败")?;
                Ok(())
            })
            .await
            .context("审计日志清空任务失败")?
        })
    }

    fn purge_old_entries<'a>(
        &'a self,
        max_age_days: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<usize>> + Send + 'a>>
    {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let conn = pool.get().context("获取数据库连接失败")?;
                let now = chrono::Utc::now().timestamp();
                let cutoff = now - (max_age_days as i64) * 86_400;
                let deleted = conn
                    .execute(
                        "DELETE FROM audit_log WHERE timestamp < ?1",
                        params![cutoff],
                    )
                    .context("轮转审计日志失败")?;
                Ok(deleted)
            })
            .await
            .context("审计日志轮转任务失败")?
        })
    }

    fn count_entries<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<usize>> + Send + 'a>>
    {
        let pool = self.pool.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let conn = pool.get().context("获取数据库连接失败")?;
                let count: i64 = conn
                    .query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get(0))
                    .context("统计审计日志失败")?;
                Ok(count as usize)
            })
            .await
            .context("审计日志统计任务失败")?
        })
    }
}

// ============================================================
// RetrievalMemoryStore 实现（REQ-PERF-012 自进化检索记忆）
// ============================================================

impl RetrievalMemoryStore for SqliteStorage {
    async fn get_memory(
        &self,
        query_type: QueryType,
        method: RetrievalMethod,
    ) -> anyhow::Result<Option<MemoryRecord>> {
        let qt = query_type.as_str().to_string();
        let m = method.as_str().to_string();
        let pool = self.pool.clone();
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
                        method: RetrievalMethod::parse_str(&m_str)
                            .unwrap_or(RetrievalMethod::Hybrid),
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

    async fn upsert_memory(&self, record: &MemoryRecord) -> anyhow::Result<()> {
        let qt = record.query_type.as_str().to_string();
        let m = record.method.as_str().to_string();
        let hit = record.hit_count as i64;
        let miss = record.miss_count as i64;
        let avg = record.avg_score;
        let pool = self.pool.clone();
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

    async fn list_memories(&self, query_type: QueryType) -> anyhow::Result<Vec<MemoryRecord>> {
        let qt = query_type.as_str().to_string();
        let pool = self.pool.clone();
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
                        method: RetrievalMethod::parse_str(&m_str)
                            .unwrap_or(RetrievalMethod::Hybrid),
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

    async fn list_all_memories(&self) -> anyhow::Result<Vec<MemoryRecord>> {
        let pool = self.pool.clone();
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
                        method: RetrievalMethod::parse_str(&m_str)
                            .unwrap_or(RetrievalMethod::Hybrid),
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

    async fn clear_all_memories(&self) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute("DELETE FROM retrieval_memory", [])
                .context("清空检索记忆失败")?;
            Ok(())
        })
        .await
        .context("检索记忆清空任务失败")?
    }
}

#[cfg(test)]
mod fts5_query_tests {
    use super::LruVectorCache;
    use super::build_fts5_or_query;

    #[test]
    fn single_english_word() {
        assert_eq!(build_fts5_or_query("trampoline"), "\"trampoline\"");
    }

    #[test]
    fn mixed_en_cjk_sentence() {
        let q = build_fts5_or_query("trampoline 是什么？");
        assert!(
            q.contains("\"trampoline\""),
            "必须包含 trampoline token: {q}"
        );
    }

    #[test]
    fn empty_query() {
        assert_eq!(build_fts5_or_query(""), "");
        assert_eq!(build_fts5_or_query("   "), "");
    }

    #[test]
    fn dedup_tokens() {
        let q = build_fts5_or_query("tokio tokio tokio");
        assert_eq!(q, "\"tokio\"");
    }

    #[test]
    fn fts5_operator_injection_prevention() {
        let q = build_fts5_or_query("test NEAR water");
        // "NEAR" 被双引号包裹后视为普通字符串，不作为 FTS5 操作符
        assert!(q.contains("\"NEAR\""));
    }

    // ========================================================================
    // S6: LRU 向量缓存 TDD 测试（TC-LRU-001~003）
    // ========================================================================

    /// TC-LRU-001：缓存满时驱逐最旧条目。
    ///
    /// 向容量为 3 的 LRU 缓存插入 5 个条目，
    /// 验证仅保留最后 3 个，最旧的 2 个被驱逐。
    #[test]
    fn tc_lru_001_evict_oldest_when_full() {
        let mut cache = LruVectorCache::new(3);
        cache.insert("a".to_string(), vec![1.0]);
        cache.insert("b".to_string(), vec![2.0]);
        cache.insert("c".to_string(), vec![3.0]);
        cache.insert("d".to_string(), vec![4.0]);
        cache.insert("e".to_string(), vec![5.0]);

        assert_eq!(cache.len(), 3, "容量 3 应仅保留 3 个条目");
        // "a" 和 "b" 应被驱逐（最旧）
        let entries: Vec<(String, Vec<f32>)> = cache.to_vec();
        let has = |key: &str| entries.iter().any(|(k, _)| k == key);
        assert!(!has("a"), "\"a\" 应被驱逐");
        assert!(!has("b"), "\"b\" 应被驱逐");
        assert!(has("c"), "\"c\" 应保留");
        assert!(has("d"), "\"d\" 应保留");
        assert!(has("e"), "\"e\" 应保留");
    }

    /// TC-LRU-002：检索操作更新访问顺序。
    ///
    /// 插入 a, b, c（容量 3），touch "a"（移到 MRU 端），
    /// 再插入 "d" → "b"（最旧）被驱逐而非 "a"。
    #[test]
    fn tc_lru_002_touch_updates_access_order() {
        let mut cache = LruVectorCache::new(3);
        cache.insert("a".to_string(), vec![1.0]);
        cache.insert("b".to_string(), vec![2.0]);
        cache.insert("c".to_string(), vec![3.0]);

        // touch "a" → 移到 MRU 端（最新）
        cache.touch("a");

        // 插入 "d" → "b" 被驱逐（"a" 已被 touch，不是最旧了）
        cache.insert("d".to_string(), vec![4.0]);

        assert_eq!(cache.len(), 3);
        let entries: Vec<(String, Vec<f32>)> = cache.to_vec();
        let has = |key: &str| entries.iter().any(|(k, _)| k == key);
        assert!(has("a"), "\"a\" 被 touch 后应保留");
        assert!(!has("b"), "\"b\" 应被驱逐（最旧）");
        assert!(has("c"), "\"c\" 应保留");
        assert!(has("d"), "\"d\" 应保留");
    }

    /// TC-LRU-003：写操作失效对应条目。
    ///
    /// 插入 a, b, c，remove "b" → "b" 被移除，其余保留。
    /// 再插入 "d" 不会驱逐任何条目（有空位）。
    #[test]
    fn tc_lru_003_remove_invalidates_entry() {
        let mut cache = LruVectorCache::new(5);
        cache.insert("a".to_string(), vec![1.0]);
        cache.insert("b".to_string(), vec![2.0]);
        cache.insert("c".to_string(), vec![3.0]);

        // remove "b"
        cache.remove("b");

        assert_eq!(cache.len(), 2, "remove 后应剩 2 个条目");
        let entries: Vec<(String, Vec<f32>)> = cache.to_vec();
        let has = |key: &str| entries.iter().any(|(k, _)| k == key);
        assert!(has("a"), "\"a\" 应保留");
        assert!(!has("b"), "\"b\" 应被移除");
        assert!(has("c"), "\"c\" 应保留");

        // 插入 "d" 不驱逐（有空位）
        cache.insert("d".to_string(), vec![4.0]);
        assert_eq!(cache.len(), 3, "有空位时插入不驱逐");
    }

    /// 额外：from_vectors 超量截断验证。
    #[test]
    fn tc_lru_extra_from_vectors_truncation() {
        let vectors = vec![
            ("a".to_string(), vec![1.0]),
            ("b".to_string(), vec![2.0]),
            ("c".to_string(), vec![3.0]),
            ("d".to_string(), vec![4.0]),
            ("e".to_string(), vec![5.0]),
        ];
        let cache = LruVectorCache::from_vectors(vectors, 3);
        assert_eq!(cache.len(), 3, "from_vectors 应截断到 max_entries");
    }

    /// 额外：clear 清空全部。
    #[test]
    fn tc_lru_extra_clear() {
        let mut cache = LruVectorCache::new(5);
        cache.insert("a".to_string(), vec![1.0]);
        cache.insert("b".to_string(), vec![2.0]);
        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }
}
