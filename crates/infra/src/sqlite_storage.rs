//! SQLite 持久化适配器（REQ-DB-001）：rusqlite(bundled) + r2d2 连接池 + WAL 模式。
//! 体系三：rusqlite 为具体实现，仅允许存在于 infra 层；
//! rusqlite 为同步 API，全部数据库操作经 `spawn_blocking` 执行，严禁阻塞 async executor。
//!
//! v2.0 S01 拆分：schema 常量、migration 函数、crypto 辅助已移至 `storage/` 子模块。
//! v2.0 S02 拆分：文档/会话/消息表 CRUD 操作已移至 `storage::documents` / `storage::conversations` / `storage::messages`。

#[path = "storage/mod.rs"]
pub(crate) mod storage;

use std::path::Path;
use tracing::{error, info, warn};

// 重导出子模块（保持外部引用路径不变）
pub(crate) use storage::{PRAGMAS, Pool, ensure_dir_0700, init_schema, load_or_create_cipher};
// crypto 辅助函数（接收 &Aes256Gcm 参数的自由函数）
use storage::{
    decrypt as crypto_decrypt, decrypt_bytes as crypto_decrypt_bytes, encrypt as crypto_encrypt,
    encrypt_bytes as crypto_encrypt_bytes,
};
// S02 拆分：CRUD 子模块
// S03 拆分：新增 vectors / entities / misc 子模块
use storage::{conversations, documents, entities, messages, misc, vectors};

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

// ============================================================================
// v2.2：向量快照缓存（REQ-PERF-016）
// ============================================================================
//
// 原 `LruVectorCache`（容量 5000 硬上限）在知识库向量数超过上限时
// 静默驱逐后段向量，导致检索结果缺失 chunk（正确性 bug，v2.2 修复）。
//
// 快照语义：
// - 缓存命中路径仅克隆 Arc 指针（O(1)），消除每次查询的全量深拷贝
// - max_vectors 仅作「内存预算守卫」：0 = 自动（缓存全部向量），
//   正数 = 向量总数超过预算时跳过缓存（检索仍完整，仅性能回退）
// - 写操作（add_embedding / 删除）仍整体失效缓存，下次检索重建快照

/// 向量快照缓存：`Option<Arc<快照>>`。命中时克隆 Arc（零深拷贝）。
type VectorCache =
    std::sync::Arc<std::sync::RwLock<Option<std::sync::Arc<Vec<(String, Vec<f32>)>>>>>;

/// 向量快照（`(chunk_id, 归一化向量)` 列表，Arc 共享，REQ-PERF-016）。
type VectorSnapshot = std::sync::Arc<Vec<(String, Vec<f32>)>>;

/// HNSW 自动启用阈值（REQ-PERF-013）。
///
/// 当知识库向量数 > 此阈值时，`vector_search` 自动构建并使用 HNSW 索引（O(log n)）。
/// 向量数 ≤ 此阈值时保持全量扫描（O(n)，小数据量更快，无构建开销）。
///
/// 阈值 500 基于性能交叉点推算：
/// - 500 chunks 全量扫描 ~5ms，HNSW 构建开销 ~50ms（首次查询反而更慢）
/// - 10k chunks 全量扫描 ~160ms，HNSW 查询 ~1ms（构建一次后永久受益）
const HNSW_AUTO_THRESHOLD: usize = 500;

// S01 拆分：aes_gcm / base64 加密辅助已迁移至 `storage::crypto` 模块。
// `Aes256Gcm` 类型仍作为 `SqliteStorage` 字段类型保留。
use aes_gcm::Aes256Gcm;
use anyhow::{Context, bail};
use echomind_core::Storage;
use echomind_core::privacy::{AuditEntry, AuditLogger};
use echomind_core::retrieval_memory::{
    MemoryRecord, QueryType, RetrievalMemoryStore, RetrievalMethod,
};
use echomind_models::{
    BudgetStats, ChatMessage, Chunk, CodeSymbol, Conversation, DocStatus, Document, EntityRelation,
    GlobalSearchResults, MemoryEntry, MemorySource, MemoryTier, MessageSearchResult, PendingInput,
    Proposition, RetrievalResult, ScratchLogEntry, SessionTodo, SummaryNode, SymbolKind,
    TodoStatus, TurnActiveVersion, WikiLink,
};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

// S01 拆分：PRAGMAS / SCHEMA_TABLES / SCHEMA_INDEXES / SCHEMA_FTS / SCHEMA_MESSAGES_FTS /
// DOC_COLS / NONCE_LEN / SECRET_KEY_FILE / SCHEMA_AUDIT_LOG / KNOWN_TABLES /
// IntegrityCheckResult / Pool 已迁移至 `storage::schema` 模块。

/// SQLite 存储适配器。克隆廉价（共享连接池与加密器）。
#[derive(Clone)]
pub struct SqliteStorage {
    pool: Pool,
    cipher: Aes256Gcm,
    /// HNSW 近似最近邻索引（REQ-NFR-005 + REQ-PERF-013）。
    /// 懒构建 + 脏标记：文档变更后置脏，下次检索自动重建。
    /// 大知识库（>HNSW_AUTO_THRESHOLD chunks）下将向量检索从全表扫描 O(n) 降为 O(log n)。
    /// **已从 Pro 下沉到 Free**（REQ-PERF-013）。
    hnsw: std::sync::Arc<std::sync::Mutex<Option<crate::hnsw_index::HnswIndex>>>,
    /// HNSW 索引是否因文档变更而失效（需要重建）。
    hnsw_dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// REQ-PERF-015：HNSW 索引磁盘持久化路径（`hnsw_index.bin`）。
    ///
    /// 首次构建后落盘；后续启动首次检索时从磁盘加载（跳过 DB 全量读取），
    /// 损坏/维度不匹配时回退全量构建。启动不加载（懒）。
    hnsw_path: std::path::PathBuf,
    /// **性能优化（秒出答案）**：内存向量快照缓存（REQ-PERF-016）。
    ///
    /// 首次 `vector_search` 时全量加载所有 (chunk_id, vector) 对到内存并归一化
    /// （REQ-PERF-017），后续检索直接在内存中计算点积（等价余弦），
    /// 跳过 SQLite BLOB 读取 + 反序列化。Arc 快照语义：命中路径零深拷贝。
    ///
    /// - 1000 chunks (384-dim): ~1.5MB 内存，检索从 ~20ms → ~1ms
    /// - 10K chunks: ~15MB 内存，检索从 ~200ms → ~10ms
    /// - 100K chunks: ~150MB 内存，检索从 ~5s → ~100ms
    ///
    /// 写操作（add_embedding / delete_chunks_by_doc）时自动失效，下次检索重建。
    /// 快照内容经归一化（单位向量），HNSW 构建与全量扫描共用同一份数据。
    vector_cache: VectorCache,
    /// 内存预算守卫：`0` = 自动（缓存全部向量）；正数 = 向量总数超过预算时
    /// 跳过缓存（检索结果仍完整，仅性能回退）。默认 0（REQ-PERF-016 AC-4）。
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
        let cipher = load_or_create_cipher(data_dir)?;
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
            hnsw: std::sync::Arc::new(std::sync::Mutex::new(None)),
            hnsw_dirty: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            vector_cache: std::sync::Arc::new(std::sync::RwLock::new(None)),
            max_vectors: 0,
            hnsw_path: data_dir.join("hnsw_index.bin"),
        };
        init_schema(&storage.pool, db_path)?;
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
        let cipher = load_or_create_cipher(data_dir)?;

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
            hnsw: std::sync::Arc::new(std::sync::Mutex::new(None)),
            hnsw_dirty: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            vector_cache: std::sync::Arc::new(std::sync::RwLock::new(None)),
            max_vectors: 0,
            hnsw_path: data_dir.join("hnsw_index.bin"),
        };
        init_schema(&storage.pool, db_path)?;
        Ok(storage)
    }

    /// 返回连接池的克隆（供 `SqliteCache` 共享同一数据库，REQ-PERF-001）。
    pub fn pool_clone(&self) -> Pool {
        self.pool.clone()
    }

    /// REQ-PERF-016：设置向量缓存内存预算守卫。
    ///
    /// - `0`：自动预算（缓存全部向量，推荐默认）
    /// - 正数：向量总数超过预算时跳过缓存（检索结果仍完整，仅性能回退）
    ///
    /// 更新后会使现有缓存失效（下次检索按新预算重建）。
    pub fn set_max_vectors(&mut self, max: usize) {
        self.max_vectors = max;
        self.invalidate_vector_cache();
    }

    /// REQ-PERF-016：获取向量缓存内存预算守卫（0 = 自动）。
    #[must_use]
    pub fn max_vectors(&self) -> usize {
        self.max_vectors
    }

    /// **性能优化（秒出答案）**：使内存向量缓存失效。
    ///
    /// 在嵌入写入/删除时调用，下次 `vector_search` 自动重建缓存。
    /// 同时使 HNSW 索引失效（REQ-PERF-013）。
    fn invalidate_vector_cache(&self) {
        if let Ok(mut guard) = self.vector_cache.write() {
            *guard = None;
        }
        self.mark_hnsw_dirty();
    }
}
/// 在阻塞线程池执行数据库任务。
pub(crate) async fn run_db<T>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T>
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
pub(crate) fn with_transaction<F, T>(conn: &rusqlite::Connection, f: F) -> anyhow::Result<T>
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

pub(crate) fn status_to_row(status: &DocStatus) -> (&'static str, Option<String>) {
    match status {
        DocStatus::Pending => ("pending", None),
        DocStatus::Processing => ("processing", None),
        DocStatus::Indexed => ("indexed", None),
        DocStatus::Failed(reason) => ("failed", Some(reason.clone())),
    }
}

pub(crate) fn row_to_status(status: &str, reason: Option<String>) -> DocStatus {
    match status {
        "pending" => DocStatus::Pending,
        "processing" => DocStatus::Processing,
        "indexed" => DocStatus::Indexed,
        "failed" => DocStatus::Failed(reason.unwrap_or_default()),
        other => DocStatus::Failed(format!("未知状态标识: {other}")),
    }
}

pub(crate) fn row_to_document(row: &rusqlite::Row<'_>) -> rusqlite::Result<Document> {
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
pub(crate) fn vec_to_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// 小端字节 → f32 向量（长度校验，绝不 Panic）。
#[allow(clippy::chunks_exact_to_as_chunks)] // Rust 1.98+ 新 lint，as_chunks 返回元组类型不兼容
pub(crate) fn bytes_to_vec(bytes: &[u8]) -> anyhow::Result<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        bail!("向量字节长度非法（非 4 的倍数）: {}", bytes.len());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// 高性能余弦相似度计算（单次遍历 + 4x 循环展开）。
///
/// 合并 dot product + 两个 norm 平方和为单次遍历，减少 2/3 内存访问。
/// 手动展开帮助编译器自动向量化（SSE/NEON），384 维向量可达 4x 加速。
#[inline]
pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let n = a.len();
    let mut dot = 0.0f32;
    let mut norm_a_sq = 0.0f32;
    let mut norm_b_sq = 0.0f32;

    let chunks = n / 4;
    let remainder = n % 4;
    for i in 0..chunks {
        let base = i * 4;
        let (a0, a1, a2, a3) = (a[base], a[base + 1], a[base + 2], a[base + 3]);
        let (b0, b1, b2, b3) = (b[base], b[base + 1], b[base + 2], b[base + 3]);
        dot += a0 * b0 + a1 * b1 + a2 * b2 + a3 * b3;
        norm_a_sq += a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
        norm_b_sq += b0 * b0 + b1 * b1 + b2 * b2 + b3 * b3;
    }
    for i in (n - remainder)..n {
        dot += a[i] * b[i];
        norm_a_sq += a[i] * a[i];
        norm_b_sq += b[i] * b[i];
    }

    let denom = (norm_a_sq * norm_b_sq).sqrt();
    if denom == 0.0 { 0.0 } else { dot / denom }
}

/// REQ-PERF-017：原地归一化向量为单位向量。
///
/// 零向量防御：模长为 0 时保持原样（全零），检索得分 0.0，不产生 NaN。
/// 归一化仅作用于内存缓存副本；DB 存储的 BLOB 保持原始值（向后兼容）。
pub(crate) fn normalize_in_place(v: &mut [f32]) {
    let norm_sq: f32 = v.iter().fold(0.0, |acc, x| acc + x * x);
    if norm_sq <= 0.0 {
        return;
    }
    let inv = 1.0 / norm_sq.sqrt();
    for x in v.iter_mut() {
        *x *= inv;
    }
}

/// REQ-PERF-017：归一化向量的点积（等价余弦相似度，1 次乘加/dim）。
///
/// 与 `cosine_similarity` 的 3 次乘加/dim 相比 FLOPs 降低约 3×。
/// 仅用于归一化后的向量（内存快照 + 归一化查询）；维度不匹配/空向量返回 0.0。
/// 手动 4 路展开辅助自动向量化（SSE/NEON）。
#[inline]
pub(crate) fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let n = a.len();
    let mut dot = 0.0f32;
    let chunks = n / 4;
    let remainder = n % 4;
    for i in 0..chunks {
        let base = i * 4;
        dot += a[base] * b[base]
            + a[base + 1] * b[base + 1]
            + a[base + 2] * b[base + 2]
            + a[base + 3] * b[base + 3];
    }
    for i in (n - remainder)..n {
        dot += a[i] * b[i];
    }
    dot
}

impl SqliteStorage {
    // ---- HNSW 索引支持（REQ-NFR-005 / F1-2）----

    /// 标记 HNSW 索引失效：分块/向量变更（导入、删除、重索引）后调用，
    /// 下次 `vector_search` 自动全量重建（REQ-PERF-013）。
    pub fn mark_hnsw_dirty(&self) {
        self.hnsw_dirty
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // REQ-PERF-015 AC-4：写入后磁盘索引已陈旧，删除落盘文件。
        // 防止崩溃重启后（内存 dirty 标志丢失）加载陈旧索引。
        // 删除失败不致命（下次搜索会用当前 DB 重建并覆盖）。
        if let Err(e) = std::fs::remove_file(&self.hnsw_path) {
            // 文件不存在（首次写入前）属正常，其余情况记录 debug 日志
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::debug!("删除陈旧 HNSW 索引文件失败: {e:#}");
            }
        }
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

    // ========================================================================
    // P1-1: 磁盘空间检查 + 弹性写入
    // ========================================================================

    /// 数据库文件路径（用于磁盘空间检查）。
    fn db_path(&self) -> std::path::PathBuf {
        // 从连接池获取数据库路径
        // 注：r2d2 不直接暴露路径，但 SqliteStorage::new 时已知道路径
        // 这里通过连接的 DBName 获取
        let conn = self.pool.get().ok();
        if let Some(conn) = conn {
            let db_name: String = conn
                .query_row("PRAGMA database_list", [], |row| row.get(2))
                .unwrap_or_default();
            std::path::PathBuf::from(db_name)
        } else {
            std::path::PathBuf::from("echomind.db")
        }
    }

    /// 检查磁盘空间是否充足（P1-1：磁盘满弹性设计）。
    ///
    /// 在关键写入操作（文档导入、消息持久化）前调用此方法，
    /// 如果可用空间低于阈值，返回 `DISK_FULL:` 前缀错误。
    ///
    /// # 参数
    /// - `required_bytes`: 写入操作预估所需空间（字节）。如果为 0，仅检查是否低于阈值。
    ///
    /// # 返回
    /// - `Ok(())`：空间充足，可以继续写入
    /// - `Err`：空间不足，错误消息包含 `DISK_FULL:` 前缀
    pub async fn check_disk_space(&self, required_bytes: u64) -> anyhow::Result<()> {
        let db_path = self.db_path();
        let path_for_check = if db_path.exists() {
            db_path
        } else {
            // 如果数据库文件不存在（新建场景），检查父目录
            db_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        };

        let result = tokio::task::spawn_blocking(move || {
            crate::disk_space::check_disk_space(&path_for_check, required_bytes)
        })
        .await
        .context("磁盘空间检查任务失败")?;

        if let Err(e) = result {
            // 返回带 DISK_FULL 前缀的错误
            return Err(anyhow::anyhow!(
                "{}: {}",
                echomind_core::errors::ERR_DISK_FULL,
                e
            ));
        }
        Ok(())
    }

    /// 清理磁盘空间（P1-1：磁盘满弹性设计）。
    ///
    /// 当磁盘空间不足时，调用此方法清理临时文件释放空间。
    /// 清理范围：.partial / .tmp 文件、孤立 .meta.json、旧日志文件。
    ///
    /// # 参数
    /// - `data_dir`: 应用数据目录
    ///
    /// # 返回
    /// 释放的字节数。
    pub async fn cleanup_for_disk_space(&self, data_dir: &Path) -> u64 {
        let data_dir = data_dir.to_path_buf();
        let freed =
            tokio::task::spawn_blocking(move || crate::disk_space::cleanup_temp_files(&data_dir))
                .await
                .unwrap_or(0);

        if freed > 0 {
            info!(
                "磁盘空间清理：释放了 {} 字节 ({:.1} MB)",
                freed,
                freed as f64 / (1024.0 * 1024.0)
            );
        }

        // 执行 WAL checkpoint 收缩 WAL 文件
        let pool = self.pool.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(conn) = pool.get() {
                let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
            }
        })
        .await;

        freed
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
        // P1-1: 写入前检查磁盘空间（文档记录约 1KB）
        self.check_disk_space(1024).await?;
        documents::add_document(&self.pool, doc.clone()).await
    }

    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> anyhow::Result<()> {
        documents::update_doc_status(&self.pool, doc_id.to_string(), status).await
    }

    async fn add_chunk(&self, chunk: &Chunk) -> anyhow::Result<()> {
        vectors::add_chunk(&self.pool, chunk).await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    /// 批量写入分块（单事务，性能优化）。S03 拆分委托 `vectors::add_chunks_batch`。
    async fn add_chunks_batch(&self, chunks: &[Chunk]) -> anyhow::Result<()> {
        // P1-1: 写入前检查磁盘空间（每个 chunk 约 1-4KB，保守估计 4KB/chunk）
        let estimated_bytes = chunks.len() as u64 * 4096;
        self.check_disk_space(estimated_bytes).await?;
        vectors::add_chunks_batch(&self.pool, chunks).await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    async fn add_embedding(&self, chunk_id: &str, embedding: &[f32]) -> anyhow::Result<()> {
        vectors::add_embedding(&self.pool, chunk_id, embedding).await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    /// 批量写入向量（性能优化：单事务 + 仅一次缓存失效）。S03 拆分委托。
    async fn add_embeddings_batch(&self, embeddings: &[(String, Vec<f32>)]) -> anyhow::Result<()> {
        // P1-1: 写入前检查磁盘空间（每个向量 384 维 × 4 字节 = 1536 字节，保守估计 2KB/向量）
        let estimated_bytes = embeddings.len() as u64 * 2048;
        self.check_disk_space(estimated_bytes).await?;
        vectors::add_embeddings_batch(&self.pool, embeddings).await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    async fn vector_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        // V3.1 P2-5：热路径计时埋点（tracing debug 级别，生产 INFO 下不输出）
        let _search_span = std::time::Instant::now();
        // REQ-PERF-017：查询向量归一化一次（内存快照向量已在加载时归一化）。
        // 归一化后余弦相似度退化为点积（1 次乘加/dim，~3× FLOPs 降低）。
        // HNSW 的 DistCosine 与归一化输入组合结果等价（1 - dot = 1 - cos）。
        let mut query_norm = query_embedding.to_vec();
        normalize_in_place(&mut query_norm);

        // ---- HNSW 快速路径（REQ-NFR-005 + REQ-PERF-013）----
        // 已从 Pro 下沉到 Free。阈值切换：向量数 > HNSW_AUTO_THRESHOLD 用 HNSW O(log n)，
        // 否则走内存全量扫描 O(n)（小数据量全量扫描更快，无构建开销）。
        let use_hnsw = {
            let idx = self.hnsw.lock().unwrap_or_else(|e| e.into_inner());
            idx.is_some() && !self.hnsw_dirty.load(std::sync::atomic::Ordering::SeqCst)
        };
        if use_hnsw {
            // 索引已构建且未脏：直接搜索
            // use_hnsw 已确认索引存在；此处防御性匹配，None（竞态）→ 降级全表扫描
            let search_hits = {
                let idx = self.hnsw.lock().unwrap_or_else(|e| e.into_inner());
                idx.as_ref().map(|idx| {
                    idx.search(&query_norm, top_k * 4)
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
                    tracing::debug!(
                        path = "hnsw",
                        top_k,
                        hits = results.len(),
                        elapsed_ms = _search_span.elapsed().as_millis() as u64,
                        "vector_search 完成"
                    );
                    return Ok(results);
                }
            }
        }

        // ---- 向量快照获取（REQ-PERF-016：Arc 快照，命中零深拷贝）----
        let cached: Option<VectorSnapshot> = {
            let guard = self.vector_cache.read();
            guard.ok().and_then(|g| g.clone())
        };

        let vectors: VectorSnapshot = match cached {
            Some(snapshot) => snapshot,
            None => {
                // 缓存未命中：全量加载 + 归一化（REQ-PERF-017）+ 内存预算守卫
                let mut loaded = self.load_all_embeddings().await?;
                for (_, v) in &mut loaded {
                    normalize_in_place(v);
                }
                let snapshot: VectorSnapshot = std::sync::Arc::new(loaded);
                // 内存预算守卫（REQ-PERF-016 AC-4）：0 = 自动（缓存全部）；
                // 正数 = 向量总数超出预算时跳过缓存（检索结果仍完整，仅下次查询回退 DB）。
                if (self.max_vectors == 0 || snapshot.len() <= self.max_vectors)
                    && let Ok(mut guard) = self.vector_cache.write()
                {
                    *guard = Some(std::sync::Arc::clone(&snapshot));
                }
                snapshot
            }
        };

        // 阈值切换：向量数 > 阈值且索引 dirty → 构建/加载 HNSW 索引
        if vectors.len() > HNSW_AUTO_THRESHOLD && !vectors.is_empty() {
            // 防御：维度不匹配的向量（旧 schema 迁移遗留数据）不参与 HNSW 图构建
            let query_dim = query_norm.len();
            let (matched, excluded): (Vec<_>, Vec<_>) = vectors
                .iter()
                .cloned()
                .partition(|(_, v)| v.len() == query_dim);
            let (hnsw, hnsw_dirty) = (self.hnsw.clone(), self.hnsw_dirty.clone());
            let hnsw_path = self.hnsw_path.clone();
            let query = query_norm.clone();
            let cipher = self.cipher.clone();
            let mut built_hits = tokio::task::spawn_blocking(move || {
                // REQ-PERF-015：首次构建（非 dirty + 内存索引为空）优先从磁盘加载（加密），
                // 校验失败（缺失/损坏/维度不匹配/解密失败）回退全量构建。
                let (idx, from_disk) = {
                    let guard = hnsw.lock().unwrap_or_else(|e| e.into_inner());
                    if guard.is_none() && !hnsw_dirty.load(std::sync::atomic::Ordering::SeqCst) {
                        let loaded = std::fs::read(&hnsw_path)
                            .map_err(anyhow::Error::from)
                            .and_then(|enc| crypto_decrypt_bytes(&cipher, &enc))
                            .and_then(|plain| {
                                crate::hnsw_index::HnswIndex::deserialize_binary(
                                    &plain,
                                    query.len(),
                                )
                            });
                        match loaded {
                            Ok(idx) => (idx, true),
                            Err(e) => {
                                warn!("HNSW 磁盘索引加载失败，回退全量构建: {e:#}");
                                (crate::hnsw_index::HnswIndex::build(&matched)?, false)
                            }
                        }
                    } else {
                        (crate::hnsw_index::HnswIndex::build(&matched)?, false)
                    }
                };
                let hits = idx.search(&query, top_k * 4);
                // REQ-PERF-015 AC-1：全量构建后加密落盘（磁盘加载的索引已最新，跳过重写）。
                if !from_disk {
                    match idx
                        .serialize_binary()
                        .and_then(|plain| crypto_encrypt_bytes(&cipher, &plain))
                    {
                        Ok(enc) => {
                            if let Err(e) = std::fs::write(&hnsw_path, enc) {
                                warn!("HNSW 索引落盘失败: {e:#}");
                            }
                        }
                        Err(e) => warn!("HNSW 索引序列化/加密失败: {e:#}"),
                    }
                }
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
            // HNSW 无结果：落入下方内存全量扫描路径（vectors 仍可用，因 HNSW 路径用 clone）
        }

        // ---- 内存全量扫描路径（小知识库或 HNSW 降级）----
        // 归一化向量点积（等价余弦，REQ-PERF-017），O(N) select_nth 取 top-k。
        // Arc 快照仅克隆指针（REQ-PERF-016 AC-3：零全量深拷贝）。
        let query_for_scan = query_norm.clone();
        let snapshot = std::sync::Arc::clone(&vectors);
        let top_k_val = top_k;
        let top_hits: Vec<(String, f32)> = tokio::task::spawn_blocking(move || {
            // 性能优化：先计算所有分数（不带 chunk_id clone），再用 select_nth_unstable_by
            // 取 top-k（O(N) 而非 O(N log N) 全量排序），最后仅 clone top-k 的 chunk_id
            let vectors = &snapshot;
            let n = vectors.len();
            // 用索引数组避免 chunk_id clone
            let mut scores: Vec<(usize, f32)> = Vec::with_capacity(n);
            for (i, (_, vector)) in vectors.iter().enumerate() {
                let score = dot_product(&query_for_scan, vector);
                scores.push((i, score));
            }

            // select_nth_unstable_by: O(N) 部分排序，第 k 个元素处于最终排序位置，
            // 前 k 个元素是最大的 k 个（但内部无序），后 N-k 个更小
            if top_k_val < n {
                let (before, _pivot, _after) = scores.select_nth_unstable_by(top_k_val, |a, b| {
                    b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
                });
                // 仅对 top-k 部分排序（k 很小，O(k log k)）
                let mut top = before.to_vec();
                top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                // 仅 clone top-k 的 chunk_id（而非全部 N 个）
                top.into_iter()
                    .map(|(idx, score)| (vectors[idx].0.clone(), score))
                    .collect::<Vec<_>>()
            } else {
                // n <= top_k：全部排序
                scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                scores
                    .into_iter()
                    .map(|(idx, score)| (vectors[idx].0.clone(), score))
                    .collect::<Vec<_>>()
            }
        })
        .await
        .context("内存向量检索任务执行失败")?;

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
        tracing::debug!(
            path = "scan",
            top_k,
            hits = results.len(),
            elapsed_ms = _search_span.elapsed().as_millis() as u64,
            "vector_search 完成"
        );
        Ok(results)
    }

    async fn find_document_by_hash(&self, hash: &str) -> anyhow::Result<Option<Document>> {
        documents::find_document_by_hash(&self.pool, hash.to_string()).await
    }

    async fn count_documents(&self) -> anyhow::Result<usize> {
        documents::count_documents(&self.pool).await
    }

    async fn count_chunks(&self) -> anyhow::Result<usize> {
        documents::count_chunks(&self.pool).await
    }

    async fn count_embeddings(&self) -> anyhow::Result<usize> {
        documents::count_embeddings(&self.pool).await
    }

    async fn cleanup_zombies(&self) -> anyhow::Result<usize> {
        documents::cleanup_zombies(&self.pool).await
    }

    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()> {
        let encrypted = crypto_encrypt(&self.cipher, value)?;
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

    /// 批量写入设置项：单次事务 + 批量 INSERT OR REPLACE。
    ///
    /// 性能优化：N 次 set_setting = N 次 spawn_blocking + N 次 DB 连接获取 + N 次 SQL 执行。
    /// 批量写入 = 1 次 spawn_blocking + 1 次 DB 连接 + 1 次事务 + N 条 INSERT。
    async fn set_settings_batch(&self, pairs: &[(&str, &str)]) -> anyhow::Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }
        // 预加密所有值（encrypt 返回 base64(nonce‖ciphertext) 字符串）
        let encrypted_pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| {
                let enc = crypto_encrypt(&self.cipher, value)?;
                Ok(((*key).to_string(), enc))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let pool = self.pool.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            with_transaction(&conn, |conn| {
                for (key, encrypted) in &encrypted_pairs {
                    conn.execute(
                        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                        params![key, encrypted],
                    )
                    .with_context(|| format!("写入设置项 {key} 失败"))?;
                }
                Ok(())
            })
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
            Some(v) => Ok(Some(crypto_decrypt(&self.cipher, &v)?)),
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
            match crypto_decrypt(&self.cipher, &encoded) {
                Ok(value) => results.push((key, value)),
                Err(e) => {
                    warn!("设置项 {key} 解密失败，跳过: {e}");
                }
            }
        }
        Ok(results)
    }

    async fn create_conversation(&self, conversation: &Conversation) -> anyhow::Result<()> {
        conversations::create_conversation(&self.pool, conversation.clone()).await
    }

    async fn list_conversations(&self, workspace_id: &str) -> anyhow::Result<Vec<Conversation>> {
        conversations::list_conversations(&self.pool, workspace_id.to_string()).await
    }

    async fn list_conversations_paginated(
        &self,
        workspace_id: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<Conversation>> {
        conversations::list_conversations_paginated(
            &self.pool,
            workspace_id.to_string(),
            limit,
            offset,
        )
        .await
    }

    async fn count_conversations(&self, workspace_id: &str) -> anyhow::Result<usize> {
        conversations::count_conversations(&self.pool, workspace_id.to_string()).await
    }

    async fn delete_conversation(&self, id: &str) -> anyhow::Result<()> {
        conversations::delete_conversation(&self.pool, id.to_string()).await
    }

    // ========================================================================
    // 工作空间管理（REQ-WS-001/003 多知识库）
    // ========================================================================

    async fn create_workspace(&self, workspace: &echomind_models::Workspace) -> anyhow::Result<()> {
        conversations::create_workspace(&self.pool, workspace.clone()).await
    }

    async fn list_workspaces(&self) -> anyhow::Result<Vec<echomind_models::Workspace>> {
        conversations::list_workspaces(&self.pool).await
    }

    async fn rename_workspace(&self, id: &str, name: &str) -> anyhow::Result<()> {
        conversations::rename_workspace(&self.pool, id.to_string(), name.to_string()).await
    }

    async fn delete_workspace(&self, id: &str) -> anyhow::Result<()> {
        conversations::delete_workspace(&self.pool, id.to_string()).await
    }

    async fn get_workspace_stats(
        &self,
        id: &str,
    ) -> anyhow::Result<echomind_models::WorkspaceStats> {
        conversations::get_workspace_stats(&self.pool, id.to_string()).await
    }

    async fn count_documents_in_workspace(&self, workspace_id: &str) -> anyhow::Result<usize> {
        documents::count_documents_in_workspace(&self.pool, workspace_id.to_string()).await
    }

    async fn list_documents_in_workspace(
        &self,
        workspace_id: &str,
    ) -> anyhow::Result<Vec<Document>> {
        documents::list_documents_in_workspace(&self.pool, workspace_id.to_string()).await
    }

    /// 迁移文档到目标工作空间（REQ-WS-004 跨知识库迁移）。
    ///
    /// 事务 UPDATE `documents.workspace_id`，chunks / 向量等通过外键关联自动归属。
    async fn migrate_document(
        &self,
        doc_id: &str,
        target_workspace_id: &str,
    ) -> anyhow::Result<()> {
        documents::migrate_document(
            &self.pool,
            doc_id.to_string(),
            target_workspace_id.to_string(),
        )
        .await
    }

    /// 按 ID 查找单个会话（REQ-EXP-001 导出功能）。
    /// 直接 SQL 查询，无需 workspace_id 过滤（会话 ID 全局唯一）。
    async fn get_conversation(&self, id: &str) -> anyhow::Result<Option<Conversation>> {
        conversations::get_conversation(&self.pool, id.to_string()).await
    }

    async fn update_conversation_title(&self, id: &str, title: &str) -> anyhow::Result<()> {
        conversations::update_conversation_title(&self.pool, id.to_string(), title.to_string())
            .await
    }

    async fn reorder_conversations(&self, ordered_ids: &[String]) -> anyhow::Result<()> {
        conversations::reorder_conversations(&self.pool, ordered_ids.to_vec()).await
    }

    async fn add_message(
        &self,
        conversation_id: &str,
        message: &ChatMessage,
    ) -> anyhow::Result<()> {
        messages::add_message(&self.pool, conversation_id.to_string(), message.clone()).await
    }

    async fn list_messages(&self, conversation_id: &str) -> anyhow::Result<Vec<ChatMessage>> {
        messages::list_messages(&self.pool, conversation_id.to_string()).await
    }

    async fn list_messages_paginated(
        &self,
        conversation_id: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<ChatMessage>> {
        messages::list_messages_paginated(&self.pool, conversation_id.to_string(), limit, offset)
            .await
    }

    async fn count_messages(&self, conversation_id: &str) -> anyhow::Result<usize> {
        messages::count_messages(&self.pool, conversation_id.to_string()).await
    }

    async fn delete_messages_by_ids(
        &self,
        conversation_id: &str,
        message_ids: &[String],
    ) -> anyhow::Result<usize> {
        messages::delete_messages_by_ids(
            &self.pool,
            conversation_id.to_string(),
            message_ids.to_vec(),
        )
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
        messages::promote_original_turn(
            &self.pool,
            conversation_id.to_string(),
            original_message_id.to_string(),
            turn_group.to_string(),
        )
        .await
    }

    async fn set_turn_active_version(
        &self,
        conversation_id: &str,
        turn_group: &str,
        active_version: i32,
    ) -> anyhow::Result<()> {
        messages::set_turn_active_version(
            &self.pool,
            conversation_id.to_string(),
            turn_group.to_string(),
            active_version,
        )
        .await
    }

    async fn get_turn_active_versions(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<Vec<TurnActiveVersion>> {
        messages::get_turn_active_versions(&self.pool, conversation_id.to_string()).await
    }

    /// 设置消息的安全标记（Q05 安全态势分层，S03 委托）。
    async fn set_entry_security_tainted(
        &self,
        message_id: &str,
        tainted: bool,
    ) -> anyhow::Result<()> {
        messages::set_entry_security_tainted(&self.pool, message_id.to_string(), tainted).await
    }

    /// 查询消息的安全标记（Q05 安全态势分层，S03 委托）。
    async fn get_entry_security_tainted(&self, message_id: &str) -> anyhow::Result<bool> {
        messages::get_entry_security_tainted(&self.pool, message_id.to_string()).await
    }

    async fn list_documents(&self) -> anyhow::Result<Vec<Document>> {
        documents::list_documents(&self.pool).await
    }

    async fn list_documents_paginated(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<Document>> {
        documents::list_documents_paginated(&self.pool, limit, offset).await
    }

    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()> {
        documents::delete_document(&self.pool, doc_id.to_string()).await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    async fn list_chunks(&self, doc_id: &str) -> anyhow::Result<Vec<Chunk>> {
        vectors::list_chunks(&self.pool, doc_id).await
    }

    /// 删除指定文档的全部分块（S03 委托）。外键级联自动清理 embeddings。
    async fn delete_chunks_by_doc(&self, doc_id: &str) -> anyhow::Result<()> {
        vectors::delete_chunks_by_doc(&self.pool, doc_id).await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    /// 关键词全文检索（FTS5 BM25 排序，REQ-RAG-010）。
    ///
    /// 使用 SQLite FTS5 trigram 分词器执行子串匹配，BM25 算法排序。
    /// 支持英文单词精确匹配和中日韩文本子串匹配。
    /// 空查询或无匹配时返回空 Vec，不返回 Err。
    ///
    /// **短查询回退**：trigram 分词器需要 ≥3 字符才能提取 trigram。
    /// 对于 <3 字符的查询（如中文 2 字词），回退为 SQL LIKE 全表扫描。
    /// 关键词全文检索（S03 委托 `vectors::keyword_search`）。
    async fn keyword_search(
        &self,
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        vectors::keyword_search(&self.pool, query, top_k).await
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
        messages::search_messages(&self.pool, query, limit).await
    }

    /// 全局搜索（REQ-IX-008）：统一搜索消息 + 文档 + 实体。
    ///
    /// 并行执行三路搜索：消息 FTS5 + 文档 LIKE + 实体 LIKE。
    /// 每组结果按相关性排序后截断到 `limit` 条。
    async fn global_search(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<GlobalSearchResults> {
        let trimmed = query.trim();
        if trimmed.is_empty() || limit == 0 {
            return Ok(GlobalSearchResults {
                messages: Vec::new(),
                documents: Vec::new(),
                entities: Vec::new(),
            });
        }

        // 并行执行三路搜索
        let (messages, documents, entities) = tokio::join!(
            messages::search_messages(&self.pool, trimmed, limit),
            documents::search_documents(&self.pool, trimmed, limit),
            entities::search_entities(&self.pool, trimmed, limit),
        );

        Ok(GlobalSearchResults {
            messages: messages?,
            documents: documents?,
            entities: entities?,
        })
    }

    // ---- 嵌入缓存：按内容指纹去重（全尺度性能优化）----

    /// 按内容哈希查找缓存的嵌入向量。
    ///
    /// 命中则返回 `Some(Vec<f32>)`，未命中返回 `None`。
    /// 封装 `SqliteStorage::find_cached_embedding_sync` 为 async 接口。
    async fn lookup_embedding_cache(&self, content_hash: &str) -> anyhow::Result<Option<Vec<f32>>> {
        vectors::lookup_embedding_cache(&self.pool, content_hash).await
    }

    /// 将嵌入向量写入缓存（S03 委托）。
    async fn put_embedding_cache(
        &self,
        content_hash: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        vectors::put_embedding_cache(&self.pool, content_hash, embedding).await
    }

    /// 批量查找缓存的嵌入向量（S03 委托）。
    async fn lookup_embedding_cache_batch(
        &self,
        hashes: &[String],
    ) -> anyhow::Result<Vec<(usize, Vec<f32>)>> {
        vectors::lookup_embedding_cache_batch(&self.pool, hashes).await
    }

    /// 批量写入嵌入向量缓存（S03 委托）。
    async fn put_embedding_cache_batch(&self, items: &[(String, Vec<f32>)]) -> anyhow::Result<()> {
        vectors::put_embedding_cache_batch(&self.pool, items).await
    }

    /// 按 chunk ID 查找单个 chunk（S03 委托，REQ-RAG-027 图遍历用）。
    async fn get_chunk_by_id(&self, chunk_id: &str) -> anyhow::Result<Option<Chunk>> {
        vectors::get_chunk_by_id(&self.pool, chunk_id).await
    }

    /// 重建 BM25 全文索引（S03 委托，REQ-PERF-005 Contextual BM25）。
    async fn rebuild_bm25_index(&self) -> anyhow::Result<()> {
        vectors::rebuild_bm25_index(&self.pool).await
    }

    /// 按源文件路径精确查找文档（S03 委托）。
    async fn find_document_by_original_path(&self, path: &str) -> anyhow::Result<Option<Document>> {
        documents::find_document_by_original_path(&self.pool, path.to_string()).await
    }

    /// 按源文件路径前缀查找文档（S03 委托）。
    async fn find_documents_by_original_path_prefix(
        &self,
        prefix: &str,
    ) -> anyhow::Result<Vec<Document>> {
        documents::find_documents_by_original_path_prefix(&self.pool, prefix.to_string()).await
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
        documents::add_document_tag(&self.pool, doc_id.to_string(), tag.to_string()).await
    }

    /// 移除文档标签（REQ-ING-022）。
    ///
    /// 读取当前 tags JSON 数组 → 移除指定标签 → 写回。
    /// 标签不存在时幂等返回 Ok。
    async fn remove_document_tag(&self, doc_id: &str, tag: &str) -> anyhow::Result<()> {
        documents::remove_document_tag(&self.pool, doc_id.to_string(), tag.to_string()).await
    }

    /// 列出所有文档标签（REQ-ING-022）。
    ///
    /// 全表扫描 documents.tags 列，解析 JSON 数组并统计每个标签的文档数。
    /// 返回 `(tag_name, count)` 列表，按计数降序排列。
    async fn list_all_tags(&self) -> anyhow::Result<Vec<(String, usize)>> {
        documents::list_all_tags(&self.pool).await
    }

    /// 按标签筛选文档（REQ-ING-022）。
    ///
    /// 使用 SQL `WHERE tags LIKE` 查询包含指定标签的文档。
    /// LIKE 模式 `"tag"` 可匹配 JSON 数组中的标签值。
    async fn filter_documents_by_tag(&self, tag: &str) -> anyhow::Result<Vec<Document>> {
        documents::filter_documents_by_tag(&self.pool, tag).await
    }

    // ------------------------------------------------------------------
    // 文档批量操作（REQ-ING-024 文档批量操作）
    // ------------------------------------------------------------------

    /// 批量删除文档（REQ-ING-024 AC-3）。
    ///
    /// 在单个事务中批量 DELETE documents + chunks_fts，全部成功或全部回滚。
    /// 删除后失效向量缓存。
    async fn batch_delete_documents(&self, doc_ids: &[String]) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let ids: Vec<String> = doc_ids.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let tx = conn.unchecked_transaction().context("开启事务失败")?;
            {
                let mut stmt_fts = tx.prepare("DELETE FROM chunks_fts WHERE doc_id = ?1")?;
                let mut stmt_doc = tx.prepare("DELETE FROM documents WHERE id = ?1")?;
                for id in &ids {
                    stmt_fts
                        .execute(params![id])
                        .context("清理 FTS5 索引失败")?;
                    stmt_doc.execute(params![id]).context("删除文档失败")?;
                }
            }
            tx.commit().context("提交事务失败")?;
            Ok(())
        })
        .await?;
        self.invalidate_vector_cache();
        Ok(())
    }

    /// 批量移动文档到目标工作空间（REQ-ING-024 AC-4）。
    ///
    /// 在单个事务中批量 UPDATE `workspace_id`，全部成功或全部回滚。
    async fn batch_move_documents(
        &self,
        doc_ids: &[String],
        target_workspace_id: &str,
    ) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let ids: Vec<String> = doc_ids.to_vec();
        let target = target_workspace_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let tx = conn.unchecked_transaction().context("开启事务失败")?;
            {
                let mut stmt =
                    tx.prepare("UPDATE documents SET workspace_id = ?1 WHERE id = ?2")?;
                for id in &ids {
                    stmt.execute(params![target, id])
                        .context("更新工作空间失败")?;
                }
            }
            tx.commit().context("提交事务失败")?;
            Ok(())
        })
        .await
    }

    /// 批量添加文档标签（REQ-ING-024 AC-5）。
    ///
    /// 在单个事务中为多个文档批量添加多个标签（去重幂等）。
    async fn batch_add_tags(&self, doc_ids: &[String], tags: &[String]) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let ids: Vec<String> = doc_ids.to_vec();
        let tags_cleaned: Vec<String> = tags
            .iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let tx = conn.unchecked_transaction().context("开启事务失败")?;
            {
                let mut stmt = tx.prepare("SELECT tags FROM documents WHERE id = ?1")?;
                let mut stmt_update = tx.prepare("UPDATE documents SET tags = ?1 WHERE id = ?2")?;
                for doc_id in &ids {
                    let tags_json: String = stmt
                        .query_row(params![doc_id], |row| row.get(0))
                        .unwrap_or_else(|_| "[]".to_string());
                    let mut current_tags: Vec<String> =
                        serde_json::from_str(&tags_json).unwrap_or_default();
                    for tag in &tags_cleaned {
                        if !current_tags.iter().any(|t| t == tag) {
                            current_tags.push(tag.clone());
                        }
                    }
                    let new_json =
                        serde_json::to_string(&current_tags).unwrap_or_else(|_| "[]".to_string());
                    stmt_update.execute(params![new_json, doc_id])?;
                }
            }
            tx.commit().context("提交事务失败")?;
            Ok(())
        })
        .await
    }

    // ========================================================================
    // S03 拆分：以下方法委托至 storage::entities / storage::misc 子模块
    // ========================================================================

    // ---- 实体索引（REQ-PERF-006）----
    async fn add_entities(&self, entities: &[(String, String, String)]) -> anyhow::Result<()> {
        entities::add_entities(&self.pool, entities).await
    }

    async fn entity_search(
        &self,
        query_entities: &[String],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        entities::entity_search(&self.pool, query_entities, top_k).await
    }

    // ---- Proposition（REQ-PERF-007）----
    async fn add_propositions(&self, propositions: &[Proposition]) -> anyhow::Result<()> {
        entities::add_propositions(&self.pool, propositions).await
    }

    async fn add_proposition_embeddings(
        &self,
        embeddings: &[(String, Vec<f32>)],
    ) -> anyhow::Result<()> {
        entities::add_proposition_embeddings(&self.pool, embeddings).await
    }

    async fn list_propositions_by_doc(&self, doc_id: &str) -> anyhow::Result<Vec<Proposition>> {
        entities::list_propositions_by_doc(&self.pool, doc_id).await
    }

    async fn proposition_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        entities::proposition_search(&self.pool, query_embedding, top_k).await
    }

    async fn rebuild_proposition_index(&self) -> anyhow::Result<()> {
        entities::rebuild_proposition_index(&self.pool).await
    }

    // ---- RAPTOR 摘要树（REQ-PERF-009）----
    async fn add_summary_nodes(&self, nodes: &[SummaryNode]) -> anyhow::Result<()> {
        entities::add_summary_nodes(&self.pool, nodes).await
    }

    async fn update_summary_embedding(
        &self,
        node_id: &str,
        embedding: &[f32],
    ) -> anyhow::Result<()> {
        entities::update_summary_embedding(&self.pool, node_id, embedding).await
    }

    async fn list_summary_nodes(&self, doc_id: &str) -> anyhow::Result<Vec<SummaryNode>> {
        entities::list_summary_nodes(&self.pool, doc_id).await
    }

    async fn summary_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        entities::summary_search(&self.pool, query_embedding, top_k).await
    }

    async fn rebuild_summary_tree(&self) -> anyhow::Result<()> {
        entities::rebuild_summary_tree(&self.pool).await
    }

    // ---- 实体关系图谱（REQ-RAG-026）----
    async fn add_relation(&self, relation: &EntityRelation) -> anyhow::Result<()> {
        entities::add_relation(&self.pool, relation).await
    }

    async fn add_relations_batch(&self, relations: &[EntityRelation]) -> anyhow::Result<()> {
        entities::add_relations_batch(&self.pool, relations).await
    }

    async fn get_relations_for_entity(
        &self,
        entity_text: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        entities::get_relations_for_entity(&self.pool, entity_text).await
    }

    async fn get_relations_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<EntityRelation>> {
        entities::get_relations_for_chunk(&self.pool, chunk_id).await
    }

    async fn search_by_relation(
        &self,
        subject: &str,
        relation_type: &str,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        entities::search_by_relation(&self.pool, subject, relation_type).await
    }

    async fn list_all_relations(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<EntityRelation>> {
        entities::list_all_relations(&self.pool, limit, offset).await
    }

    async fn count_relations(&self) -> anyhow::Result<usize> {
        entities::count_relations(&self.pool).await
    }

    async fn get_entity_types(
        &self,
        entities_list: &[String],
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        entities::get_entity_types(&self.pool, entities_list).await
    }

    async fn get_entity_graph(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, Vec<String>>> {
        entities::get_entity_graph(&self.pool).await
    }

    // ---- 代码符号索引（REQ-RAG-031）----
    async fn add_symbols(&self, symbols: &[CodeSymbol]) -> anyhow::Result<()> {
        entities::add_symbols(&self.pool, symbols).await
    }

    async fn search_by_symbol(
        &self,
        name: &str,
        kind: Option<&SymbolKind>,
    ) -> anyhow::Result<Vec<CodeSymbol>> {
        entities::search_by_symbol(&self.pool, name, kind).await
    }

    async fn get_symbols_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<CodeSymbol>> {
        entities::get_symbols_for_chunk(&self.pool, chunk_id).await
    }

    async fn search_symbols_fuzzy(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<CodeSymbol>> {
        entities::search_symbols_fuzzy(&self.pool, query, limit).await
    }

    // ---- 对话记忆系统（REQ-RAG-032）----
    fn add_memory_entry(
        &self,
        entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        misc::add_memory_entry(&self.pool, entry)
    }

    fn get_memory_entries(
        &self,
        tier: Option<&MemoryTier>,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
        misc::get_memory_entries(&self.pool, tier)
    }

    fn update_memory_entry(
        &self,
        entry: &MemoryEntry,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        misc::update_memory_entry(&self.pool, entry)
    }

    fn delete_memory_entry(
        &self,
        id: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        misc::delete_memory_entry(&self.pool, id)
    }

    fn clear_memory_entries(
        &self,
        tier: Option<&MemoryTier>,
    ) -> impl std::future::Future<Output = anyhow::Result<usize>> + Send {
        misc::clear_memory_entries(&self.pool, tier)
    }

    fn search_memory_entries(
        &self,
        query: &str,
        limit: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send {
        misc::search_memory_entries(&self.pool, query, limit)
    }

    // ---- Wiki 双向链接（REQ-ING-020）----
    async fn add_wiki_links(&self, links: &[WikiLink]) -> anyhow::Result<()> {
        entities::add_wiki_links(&self.pool, links).await
    }

    async fn get_forward_links(&self, doc_id: &str) -> anyhow::Result<Vec<WikiLink>> {
        entities::get_forward_links(&self.pool, doc_id).await
    }

    async fn get_backlinks(&self, doc_name: &str) -> anyhow::Result<Vec<WikiLink>> {
        entities::get_backlinks(&self.pool, doc_name).await
    }

    // ---- Durable Prompt Admission（B05）----
    async fn admit_input(
        &self,
        conversation_id: &str,
        content: &str,
        delivery: &str,
    ) -> anyhow::Result<String> {
        misc::admit_input(&self.pool, conversation_id, content, delivery).await
    }

    async fn promote_input(&self, input_id: &str) -> anyhow::Result<()> {
        misc::promote_input(&self.pool, input_id).await
    }

    async fn get_pending_inputs(&self, conversation_id: &str) -> anyhow::Result<Vec<PendingInput>> {
        misc::get_pending_inputs(&self.pool, conversation_id).await
    }

    // ---- Scratch-Promote（Q01）----
    async fn add_scratch_log(&self, entry: &ScratchLogEntry) -> anyhow::Result<()> {
        misc::add_scratch_log(&self.pool, entry).await
    }

    async fn get_scratch_logs(&self, limit: Option<usize>) -> anyhow::Result<Vec<ScratchLogEntry>> {
        misc::get_scratch_logs(&self.pool, limit).await
    }

    async fn delete_scratch_log(&self, id: &str) -> anyhow::Result<()> {
        misc::delete_scratch_log(&self.pool, id).await
    }

    async fn cleanup_expired_scratch_logs(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        misc::cleanup_expired_scratch_logs(&self.pool, before_timestamp).await
    }

    // ---- 幂等性存储（Q07）----
    async fn record_idempotency(&self, key: &str, timestamp: i64) -> anyhow::Result<()> {
        misc::record_idempotency(&self.pool, key, timestamp).await
    }

    async fn list_idempotency_records(&self) -> anyhow::Result<Vec<(String, i64)>> {
        misc::list_idempotency_records(&self.pool).await
    }

    async fn cleanup_expired_idempotency(&self, before_timestamp: i64) -> anyhow::Result<usize> {
        misc::cleanup_expired_idempotency(&self.pool, before_timestamp).await
    }

    // ---- Session Todo（B08）----
    async fn add_session_todo(&self, todo: &SessionTodo) -> anyhow::Result<()> {
        misc::add_session_todo(&self.pool, todo).await
    }

    async fn update_todo_status(&self, todo_id: &str, status: &TodoStatus) -> anyhow::Result<()> {
        misc::update_todo_status(&self.pool, todo_id, status).await
    }

    async fn get_session_todos(&self, conversation_id: &str) -> anyhow::Result<Vec<SessionTodo>> {
        misc::get_session_todos(&self.pool, conversation_id).await
    }

    async fn delete_session_todo(&self, todo_id: &str) -> anyhow::Result<()> {
        misc::delete_session_todo(&self.pool, todo_id).await
    }

    async fn delete_session_todos(&self, conversation_id: &str) -> anyhow::Result<()> {
        misc::delete_session_todos(&self.pool, conversation_id).await
    }

    // ---- Budget 预算追踪 ----
    async fn record_budget_usage(
        &self,
        principal: &str,
        input_tokens: usize,
        output_tokens: usize,
        cost_usd: f64,
        model_name: &str,
    ) -> anyhow::Result<()> {
        misc::record_budget_usage(
            &self.pool,
            principal,
            input_tokens,
            output_tokens,
            cost_usd,
            model_name,
        )
        .await
    }

    async fn get_budget_stats(&self, principal: &str) -> anyhow::Result<BudgetStats> {
        let daily_limit = self
            .get_setting("budget.daily_limit_usd")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        misc::get_budget_stats(&self.pool, principal, daily_limit).await
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
        documents::add_import_log(
            &self.pool,
            file_name.to_string(),
            format.to_string(),
            result.to_string(),
            error_message.map(|s| s.to_string()),
            file_size,
        )
        .await
    }

    async fn get_import_logs(
        &self,
        result_filter: Option<&str>,
    ) -> anyhow::Result<Vec<echomind_models::ImportLogEntry>> {
        documents::get_import_logs(&self.pool, result_filter.map(|s| s.to_string())).await
    }

    async fn clear_import_logs(&self) -> anyhow::Result<()> {
        documents::clear_import_logs(&self.pool).await
    }

    // ------------------------------------------------------------------
    // 对话书签（REQ-RAG-047）
    // ------------------------------------------------------------------

    /// 添加对话书签（REQ-RAG-047 AC-1/AC-2 + REQ-RAG-053 消息级）。
    async fn add_bookmark(
        &self,
        conversation_id: &str,
        note: Option<&str>,
        message_id: Option<&str>,
        summary: Option<&str>,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().timestamp();
        conversations::add_bookmark(
            &self.pool,
            conversation_id.to_string(),
            note.map(|s| s.to_string()),
            now,
            message_id.map(|s| s.to_string()),
            summary.map(|s| s.to_string()),
        )
        .await
    }

    /// 移除对话书签（REQ-RAG-047 AC-5）。
    async fn remove_bookmark(&self, conversation_id: &str) -> anyhow::Result<()> {
        conversations::remove_bookmark(&self.pool, conversation_id.to_string()).await
    }

    /// 列出全部书签（REQ-RAG-047 AC-3/AC-4 + REQ-RAG-053）。
    async fn list_bookmarks(&self) -> anyhow::Result<Vec<echomind_models::ConversationBookmark>> {
        conversations::list_bookmarks(&self.pool).await
    }

    /// 检查指定会话是否已加书签（REQ-RAG-047 AC-2）。
    async fn is_bookmarked(&self, conversation_id: &str) -> anyhow::Result<bool> {
        conversations::is_bookmarked(&self.pool, conversation_id.to_string()).await
    }

    /// 查询指定消息的书签（REQ-RAG-053）。
    async fn get_message_bookmark(
        &self,
        message_id: &str,
    ) -> anyhow::Result<Option<echomind_models::ConversationBookmark>> {
        conversations::get_message_bookmark(&self.pool, message_id.to_string()).await
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
pub(crate) fn build_fts5_or_query(query: &str) -> String {
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
    /// S03 拆分委托：检索记忆 CRUD 委托至 `misc` 子模块。
    async fn get_memory(
        &self,
        query_type: QueryType,
        method: RetrievalMethod,
    ) -> anyhow::Result<Option<MemoryRecord>> {
        misc::get_memory(&self.pool, query_type.as_str(), method.as_str()).await
    }

    async fn upsert_memory(&self, record: &MemoryRecord) -> anyhow::Result<()> {
        misc::upsert_memory(&self.pool, record).await
    }

    async fn list_memories(&self, query_type: QueryType) -> anyhow::Result<Vec<MemoryRecord>> {
        misc::list_memories(&self.pool, query_type.as_str()).await
    }

    async fn list_all_memories(&self) -> anyhow::Result<Vec<MemoryRecord>> {
        misc::list_all_memories(&self.pool).await
    }

    async fn clear_all_memories(&self) -> anyhow::Result<()> {
        misc::clear_all_memories(&self.pool).await
    }
}

#[cfg(test)]
mod fts5_query_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::build_fts5_or_query;
    use crate::sqlite_storage::SqliteStorage;
    use echomind_core::Storage;

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
    // 性能优化: set_settings_batch 批量写入测试（TC-BATCH-SET-001~003）
    // ========================================================================

    /// TC-BATCH-SET-001：批量写入后读取值一致。
    #[tokio::test]
    async fn tc_batch_set_001_write_read_consistency() {
        let dir = tempfile::TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("batch-test.db")).unwrap();

        // 批量写入 3 个设置项
        storage
            .set_settings_batch(&[
                ("test.key1", "value1"),
                ("test.key2", "value2"),
                ("test.key3", "value3"),
            ])
            .await
            .unwrap();

        // 逐个读取验证
        assert_eq!(
            storage.get_setting("test.key1").await.unwrap().as_deref(),
            Some("value1")
        );
        assert_eq!(
            storage.get_setting("test.key2").await.unwrap().as_deref(),
            Some("value2")
        );
        assert_eq!(
            storage.get_setting("test.key3").await.unwrap().as_deref(),
            Some("value3")
        );
    }

    /// TC-BATCH-SET-002：批量写入覆盖已有值。
    #[tokio::test]
    async fn tc_batch_set_002_overwrite_existing() {
        let dir = tempfile::TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("batch-overwrite.db")).unwrap();

        // 先写入旧值
        storage.set_setting("test.key", "old").await.unwrap();

        // 批量写入新值（覆盖）
        storage
            .set_settings_batch(&[("test.key", "new"), ("test.key2", "val2")])
            .await
            .unwrap();

        assert_eq!(
            storage.get_setting("test.key").await.unwrap().as_deref(),
            Some("new"),
            "批量写入应覆盖旧值"
        );
        assert_eq!(
            storage.get_setting("test.key2").await.unwrap().as_deref(),
            Some("val2")
        );
    }

    /// TC-BATCH-SET-003：空批量写入是 no-op。
    #[tokio::test]
    async fn tc_batch_set_003_empty_noop() {
        let dir = tempfile::TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("batch-empty.db")).unwrap();

        // 空批量写入不应报错
        storage.set_settings_batch(&[]).await.unwrap();
    }
}
