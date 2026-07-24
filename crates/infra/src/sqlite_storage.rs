//! SQLite 持久化适配器（REQ-DB-001）：rusqlite(bundled) + r2d2 连接池 + WAL 模式。
//! 体系三：rusqlite 为具体实现，仅允许存在于 infra 层；
//! rusqlite 为同步 API，全部数据库操作经 `spawn_blocking` 执行，严禁阻塞 async executor。

use std::path::Path;

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, anyhow, bail};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use echomind_core::Storage;
use echomind_models::{ChatMessage, Chunk, Conversation, DocStatus, Document, RetrievalResult};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

/// 连接级 PRAGMA：WAL 模式 + 外键级联 + 写繁忙重试（REQ-DB-001-AC-3）
const PRAGMAS: &str = "
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
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
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    sources TEXT,
    created_at INTEGER NOT NULL
);
";

/// 索引定义（在表创建与迁移完成后执行）。
const SCHEMA_INDEXES: &str = "
CREATE INDEX IF NOT EXISTS idx_documents_hash ON documents(file_hash);
CREATE INDEX IF NOT EXISTS idx_chunks_doc ON chunks(doc_id);
CREATE INDEX IF NOT EXISTS idx_conversations_workspace ON conversations(workspace_id);
CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id);
";

const DOC_COLS: &str = "id, file_path, file_hash, status, status_reason, created_at";

/// AES-256-GCM 随机数 nonce 长度（96 bit）
const NONCE_LEN: usize = 12;
/// 加密密钥文件名（与数据库同目录，权限 0600）
const SECRET_KEY_FILE: &str = "secret.key";

type Pool = r2d2::Pool<SqliteConnectionManager>;

/// SQLite 存储适配器。克隆廉价（共享连接池与加密器）。
#[derive(Clone)]
pub struct SqliteStorage {
    pool: Pool,
    cipher: Aes256Gcm,
}

impl SqliteStorage {
    /// 打开（必要时创建）数据库，启用 WAL 并初始化表结构与设置加密器。
    /// 本函数含同步磁盘 I/O，调用方应置于 `spawn_blocking` 中。
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let data_dir = db_path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(data_dir)
            .with_context(|| format!("创建数据库目录失败: {}", data_dir.display()))?;
        let cipher = Self::load_or_create_cipher(data_dir)?;
        let manager =
            SqliteConnectionManager::file(db_path).with_init(|conn| conn.execute_batch(PRAGMAS));
        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .context("创建 SQLite 连接池失败")?;
        let storage = Self { pool, cipher };
        storage.init_schema()?;
        Ok(storage)
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
        Ok(())
    }

    /// 旧数据库 schema 迁移：检测已有表是否缺少必要列，若缺少则 DROP + 重建。
    ///
    /// 背景：`CREATE TABLE IF NOT EXISTS` 不会修改已有表的列结构。
    /// 若用户从旧版本升级，表可能缺少新增列，导致后续 CREATE INDEX / 查询引用
    /// 不存在列时崩溃。迁移策略：对每个表检查关键列是否存在，缺失则 DROP 重建
    /// （开发阶段数据可丢弃；生产环境应改用 ALTER TABLE 增量迁移 + 数据导出）。
    fn migrate_schema(conn: &rusqlite::Connection) -> anyhow::Result<()> {
        // documents 表：status_reason 列在 Phase 4+ 新增，旧库可能缺失
        if Self::table_exists(conn, "documents")?
            && !Self::has_column(conn, "documents", "status_reason")?
        {
            eprintln!("schema 迁移：documents 表缺少 status_reason 列，重建表");
            conn.execute_batch("DROP TABLE IF EXISTS documents;")
                .context("迁移失败：删除旧 documents 表失败")?;
        }

        // messages 表：conversation_id 列在 Phase 6 新增，旧库可能缺失
        if Self::table_exists(conn, "messages")?
            && !Self::has_column(conn, "messages", "conversation_id")?
        {
            eprintln!("schema 迁移：messages 表缺少 conversation_id 列，重建表");
            conn.execute_batch("DROP TABLE IF EXISTS messages;")
                .context("迁移失败：删除旧 messages 表失败")?;
        }

        // 重建被删除的表（此时 IF NOT EXISTS 会真正执行建表）
        conn.execute_batch(SCHEMA_TABLES)
            .context("迁移失败：重建表失败")?;
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
    fn has_column(conn: &rusqlite::Connection, table: &str, column: &str) -> anyhow::Result<bool> {
        let sql = format!("PRAGMA table_info({table})");
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
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                    .with_context(|| format!("设置密钥文件权限失败: {}", key_path.display()))?;
            }
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
    Ok(Document {
        id: row.get(0)?,
        file_path: row.get(1)?,
        file_hash: row.get(2)?,
        status: row_to_status(&status, reason),
        created_at: row.get(5)?,
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

impl Storage for SqliteStorage {
    async fn add_document(&self, doc: &Document) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc = doc.clone();
        run_db(move || {
            let (status, reason) = status_to_row(&doc.status);
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                &format!(
                    "INSERT OR REPLACE INTO documents ({DOC_COLS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
                ),
                params![
                    doc.id,
                    doc.file_path,
                    doc.file_hash,
                    status,
                    reason,
                    doc.created_at
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
            Ok(())
        })
        .await
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
        .await
    }

    async fn vector_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        let pool = self.pool.clone();
        let query = query_embedding.to_vec();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            let mut stmt = conn.prepare(
                "SELECT c.id, c.doc_id, c.content, c.token_count, c.sequence, d.file_path, e.vector
                 FROM embeddings e
                 JOIN chunks c ON c.id = e.chunk_id
                 JOIN documents d ON d.id = c.doc_id",
            )?;
            let rows = stmt.query_map([], |row| {
                let file_path: String = row.get(5)?;
                let vector: Vec<u8> = row.get(6)?;
                Ok((
                    Chunk {
                        id: row.get(0)?,
                        doc_id: row.get(1)?,
                        content: row.get(2)?,
                        token_count: row.get(3)?,
                        sequence: row.get(4)?,
                    },
                    file_path,
                    vector,
                ))
            })?;

            let mut hits = Vec::new();
            for row in rows {
                let (chunk, file_path, bytes) = row?;
                let vector = bytes_to_vec(&bytes)?;
                let score = cosine_similarity(&query, &vector);
                let doc_name = Path::new(&file_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| file_path.clone());
                hits.push(RetrievalResult {
                    chunk,
                    score,
                    doc_name,
                });
            }
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

    async fn create_conversation(&self, conversation: &Conversation) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let c = conversation.clone();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "INSERT OR IGNORE INTO conversations (id, workspace_id, title, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![c.id, c.workspace_id, c.title, c.created_at],
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
                "SELECT id, workspace_id, title, created_at FROM conversations WHERE workspace_id = ?1 ORDER BY created_at DESC, rowid DESC",
            )?;
            let rows = stmt.query_map(params![workspace_id], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    workspace_id: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
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
            let conn = pool.get().context("获取数据库连接失败")?;
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, sources, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    conversation_id,
                    message.role,
                    message.content,
                    sources_json,
                    chrono::Utc::now().timestamp(),
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
                "SELECT role, content, sources FROM messages WHERE conversation_id = ?1 ORDER BY rowid ASC",
            )?;
            let rows = stmt.query_map(params![conversation_id], |row| {
                let sources_json: Option<String> = row.get(2)?;
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, sources_json))
            })?;
            let mut out = Vec::new();
            for row in rows {
                let (role, content, sources_json) = row?;
                let sources = match sources_json {
                    Some(json) => Some(
                        serde_json::from_str(&json).context("反序列化引用来源失败")?,
                    ),
                    None => None,
                };
                out.push(ChatMessage { role, content, sources });
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

    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        let doc_id = doc_id.to_string();
        run_db(move || {
            let conn = pool.get().context("获取数据库连接失败")?;
            // 外键级联链：documents → chunks → embeddings（PRAGMA foreign_keys = ON）
            conn.execute("DELETE FROM documents WHERE id = ?1", params![doc_id])
                .context("删除文档失败")?;
            Ok(())
        })
        .await
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
}
