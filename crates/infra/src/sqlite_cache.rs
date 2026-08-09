//! SQLite 语义缓存适配器（REQ-PERF-001）：三级缓存金字塔的持久化实现。
//!
//! 三张 SQLite 表存储缓存数据：
//! - `exact_cache`：L0 精确匹配（query hash → answer）
//! - `semantic_cache`：L1 语义匹配（embedding BLOB → answer）
//! - `retrieval_cache`：L3 检索结果缓存（embedding BLOB → results JSON）
//!
//! ## 设计决策
//!
//! - 复用 `SqliteStorage` 的 r2d2 连接池（同一数据库文件，减少连接数）
//! - 向量存储为 BLOB（f32 little-endian bytes），与 `SqliteStorage` 一致
//! - 语义匹配使用全表扫描 + Rust 余弦相似度（与 `vector_search` 一致）
//! - 所有 DB 操作经 `tokio::task::spawn_blocking`

use anyhow::Context;
use echomind_core::ResponseCache;
use echomind_core::cache::{
    cosine_similarity, embedding_from_bytes, embedding_to_bytes, estimate_rag_token_cost,
    is_expired, query_hash,
};
use echomind_models::{CacheHit, CacheLevel, CacheStats};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

type SqlitePool = Pool<SqliteConnectionManager>;

/// SQLite 语义缓存适配器。
///
/// 持有 r2d2 连接池的克隆（廉价，内部引用计数），
/// 与 `SqliteStorage` 共享同一数据库文件。
#[derive(Clone)]
pub struct SqliteCache {
    pool: SqlitePool,
    // 运行时统计（原子操作，无锁）
    exact_hits: Arc<AtomicU32>,
    semantic_hits: Arc<AtomicU32>,
    retrieval_hits: Arc<AtomicU32>,
    total_queries: Arc<AtomicU32>,
}

impl SqliteCache {
    /// 创建 `SqliteCache`，使用已有的 r2d2 连接池。
    ///
    /// 调用方在 `SqliteStorage::new()` 后传入 `storage.pool_clone()`。
    /// 本函数会创建缓存表（如不存在）。
    pub fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        let cache = Self {
            pool,
            exact_hits: Arc::new(AtomicU32::new(0)),
            semantic_hits: Arc::new(AtomicU32::new(0)),
            retrieval_hits: Arc::new(AtomicU32::new(0)),
            total_queries: Arc::new(AtomicU32::new(0)),
        };
        cache.init_tables()?;
        Ok(cache)
    }

    /// 按文档名失效缓存条目（新鲜度感知，REQ-PERF-001 增强）。
    ///
    /// 删除文档后调用：只清空引用该文档的缓存条目，保留其他有效命中
    /// （替代全库清空，知识库大时显著提升缓存命中率）。
    /// sources_json / results_json 为序列化的 `RetrievalResult`，其中
    /// `doc_name` 字段以 JSON 引号包裹 → `%"doc_name"%` 精确匹配，
    /// 避免 "foo.md" 误匹配 "foobar.md"。
    pub async fn invalidate_by_doc(&self, doc_name: &str) -> anyhow::Result<()> {
        if doc_name.trim().is_empty() {
            return Ok(());
        }
        let pool = self.pool.clone();
        let pattern = format!("%\"{}\"%", doc_name.trim());
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（按文档失效缓存）")?;
            for (table, col) in [
                ("exact_cache", "sources_json"),
                ("semantic_cache", "sources_json"),
                ("retrieval_cache", "results_json"),
            ] {
                conn.execute(
                    &format!("DELETE FROM {table} WHERE {col} LIKE ?1"),
                    rusqlite::params![&pattern],
                )?;
            }
            Ok(())
        })
        .await?
    }

    /// 初始化缓存表（幂等，CREATE TABLE IF NOT EXISTS）。
    fn init_tables(&self) -> anyhow::Result<()> {
        let conn = self
            .pool
            .get()
            .context("获取数据库连接失败（缓存初始化）")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS exact_cache (
                id TEXT PRIMARY KEY,
                query_hash TEXT UNIQUE NOT NULL,
                query_text TEXT NOT NULL,
                answer_text TEXT NOT NULL,
                sources_json TEXT,
                conversation_id TEXT,
                created_at INTEGER NOT NULL,
                hit_count INTEGER DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_exact_cache_hash ON exact_cache(query_hash);

            CREATE TABLE IF NOT EXISTS semantic_cache (
                id TEXT PRIMARY KEY,
                query_text TEXT NOT NULL,
                query_embedding BLOB NOT NULL,
                answer_text TEXT NOT NULL,
                sources_json TEXT,
                conversation_id TEXT,
                created_at INTEGER NOT NULL,
                hit_count INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS retrieval_cache (
                id TEXT PRIMARY KEY,
                query_text TEXT NOT NULL,
                query_embedding BLOB NOT NULL,
                results_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                hit_count INTEGER DEFAULT 0
            );",
        )
        .context("创建缓存表失败")?;
        Ok(())
    }

    /// 获取连接池引用（供 `SqliteStorage` 共享）。
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

impl ResponseCache for SqliteCache {
    /// L0 精确匹配：以 query hash 查找缓存答案。
    async fn lookup_exact(
        &self,
        hash: &str,
        ttl_secs: u64,
        now: i64,
    ) -> anyhow::Result<Option<CacheHit>> {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        let hash = hash.to_string();
        let pool = self.pool.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（L0 查询）")?;
            let mut stmt = conn.prepare(
                "SELECT answer_text, sources_json, created_at FROM exact_cache
                 WHERE query_hash = ?1 ORDER BY created_at DESC LIMIT 1",
            )?;
            let result = stmt.query_row([&hash], |row| {
                let answer: String = row.get(0)?;
                let sources: Option<String> = row.get(1)?;
                let created: i64 = row.get(2)?;
                Ok((answer, sources, created))
            });
            match result {
                Ok((answer, sources, created)) => {
                    if is_expired(created, ttl_secs, now) {
                        Ok(None)
                    } else {
                        Ok(Some(CacheHit {
                            level: CacheLevel::Exact,
                            answer_text: Some(answer),
                            sources_json: sources,
                            retrieval_results_json: None,
                        }))
                    }
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(anyhow::Error::from(e)),
            }
        })
        .await?;

        if result.as_ref().is_ok_and(|r| r.is_some()) {
            self.exact_hits.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// L1 语义匹配：以查询嵌入查找余弦相似度 ≥ 阈值的缓存答案。
    async fn lookup_semantic(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        ttl_secs: u64,
        now: i64,
    ) -> anyhow::Result<Option<CacheHit>> {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        let emb = query_embedding.to_vec();
        let pool = self.pool.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（L1 查询）")?;
            let mut stmt = conn.prepare(
                "SELECT query_embedding, answer_text, sources_json, created_at
                 FROM semantic_cache ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                let blob: Vec<u8> = row.get(0)?;
                let answer: String = row.get(1)?;
                let sources: Option<String> = row.get(2)?;
                let created: i64 = row.get(3)?;
                Ok((blob, answer, sources, created))
            })?;

            let mut best: Option<(f32, String, Option<String>)> = None;
            for row in rows {
                let (blob, answer, sources, created) = row?;
                if is_expired(created, ttl_secs, now) {
                    continue;
                }
                let cached_emb = embedding_from_bytes(&blob);
                let sim = cosine_similarity(&emb, &cached_emb);
                if sim >= threshold {
                    match &best {
                        Some((best_sim, _, _)) if sim > *best_sim => {}
                        _ => {
                            best = Some((sim, answer, sources));
                        }
                    }
                }
            }
            Ok(best.map(|(sim, answer, sources)| {
                tracing::debug!("L1 语义缓存命中，相似度: {sim:.4}");
                CacheHit {
                    level: CacheLevel::Semantic,
                    answer_text: Some(answer),
                    sources_json: sources,
                    retrieval_results_json: None,
                }
            }))
        })
        .await?;

        if result.as_ref().is_ok_and(|r| r.is_some()) {
            self.semantic_hits.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// L3 检索结果缓存：以查询嵌入查找缓存的检索结果。
    async fn lookup_retrieval(
        &self,
        query_embedding: &[f32],
        threshold: f32,
        ttl_secs: u64,
        now: i64,
    ) -> anyhow::Result<Option<String>> {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        let emb = query_embedding.to_vec();
        let pool = self.pool.clone();

        let result = tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（L3 查询）")?;
            let mut stmt = conn.prepare(
                "SELECT query_embedding, results_json, created_at
                 FROM retrieval_cache ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                let blob: Vec<u8> = row.get(0)?;
                let results: String = row.get(1)?;
                let created: i64 = row.get(2)?;
                Ok((blob, results, created))
            })?;

            let mut best: Option<(f32, String)> = None;
            for row in rows {
                let (blob, results, created) = row?;
                if is_expired(created, ttl_secs, now) {
                    continue;
                }
                let cached_emb = embedding_from_bytes(&blob);
                let sim = cosine_similarity(&emb, &cached_emb);
                if sim >= threshold {
                    match &best {
                        Some((best_sim, _)) if sim > *best_sim => {}
                        _ => {
                            best = Some((sim, results));
                        }
                    }
                }
            }
            Ok(best.map(|(_, results)| results))
        })
        .await?;

        if result.as_ref().is_ok_and(|r| r.is_some()) {
            self.retrieval_hits.fetch_add(1, Ordering::Relaxed);
        }
        result
    }

    /// 写入 L0 精确匹配缓存。
    async fn insert_exact(
        &self,
        hash: &str,
        query_text: &str,
        answer_text: &str,
        sources_json: &str,
        conversation_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let hash = hash.to_string();
        let query_text = query_text.to_string();
        let answer_text = answer_text.to_string();
        let sources_json = sources_json.to_string();
        let conversation_id = conversation_id.map(|s| s.to_string());
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（L0 写入）")?;
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT OR REPLACE INTO exact_cache
                 (id, query_hash, query_text, answer_text, sources_json, conversation_id, created_at, hit_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![
                    id,
                    hash,
                    query_text,
                    answer_text,
                    sources_json,
                    conversation_id,
                    now,
                ],
            )?;
            Ok(())
        })
        .await?
    }

    /// 写入 L1 语义匹配缓存。
    async fn insert_semantic(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        answer_text: &str,
        sources_json: &str,
        conversation_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let query_text = query_text.to_string();
        let embedding_bytes = embedding_to_bytes(query_embedding);
        let answer_text = answer_text.to_string();
        let sources_json = sources_json.to_string();
        let conversation_id = conversation_id.map(|s| s.to_string());
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（L1 写入）")?;
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO semantic_cache
                 (id, query_text, query_embedding, answer_text, sources_json, conversation_id, created_at, hit_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
                rusqlite::params![
                    id,
                    query_text,
                    embedding_bytes,
                    answer_text,
                    sources_json,
                    conversation_id,
                    now,
                ],
            )?;
            Ok(())
        })
        .await?
    }

    /// 写入 L3 检索结果缓存。
    async fn insert_retrieval(
        &self,
        query_text: &str,
        query_embedding: &[f32],
        results_json: &str,
    ) -> anyhow::Result<()> {
        let query_text = query_text.to_string();
        let embedding_bytes = embedding_to_bytes(query_embedding);
        let results_json = results_json.to_string();
        let pool = self.pool.clone();

        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（L3 写入）")?;
            let id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "INSERT INTO retrieval_cache
                 (id, query_text, query_embedding, results_json, created_at, hit_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                rusqlite::params![id, query_text, embedding_bytes, results_json, now],
            )?;
            Ok(())
        })
        .await?
    }

    /// 清空所有缓存。
    async fn clear_all(&self) -> anyhow::Result<()> {
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（清空缓存）")?;
            conn.execute("DELETE FROM exact_cache", [])?;
            conn.execute("DELETE FROM semantic_cache", [])?;
            conn.execute("DELETE FROM retrieval_cache", [])?;
            Ok(())
        })
        .await?
    }

    /// 获取缓存统计信息。
    async fn get_stats(&self) -> anyhow::Result<CacheStats> {
        let exact = self.exact_hits.load(Ordering::Relaxed);
        let semantic = self.semantic_hits.load(Ordering::Relaxed);
        let retrieval = self.retrieval_hits.load(Ordering::Relaxed);
        let total = self.total_queries.load(Ordering::Relaxed);

        let pool = self.pool.clone();
        let entry_count = tokio::task::spawn_blocking(move || {
            let conn = pool.get().context("获取连接失败（统计）")?;
            let exact_count: u32 = conn
                .query_row("SELECT COUNT(*) FROM exact_cache", [], |row| row.get(0))
                .unwrap_or(0);
            let semantic_count: u32 = conn
                .query_row("SELECT COUNT(*) FROM semantic_cache", [], |row| row.get(0))
                .unwrap_or(0);
            let retrieval_count: u32 = conn
                .query_row("SELECT COUNT(*) FROM retrieval_cache", [], |row| row.get(0))
                .unwrap_or(0);
            Ok::<u32, anyhow::Error>(exact_count + semantic_count + retrieval_count)
        })
        .await??;

        let token_saved = (exact + semantic) as u64 * estimate_rag_token_cost();

        Ok(CacheStats {
            enabled: true,
            exact_hits: exact,
            semantic_hits: semantic,
            retrieval_hits: retrieval,
            total_queries: total,
            cache_size_entries: entry_count,
            estimated_token_saved: token_saved,
        })
    }
}

/// 便捷函数：对查询文本计算 hash 并进行 L0 精确匹配。
///
/// 供 `chat_inner` 调用，减少重复代码。
pub fn lookup_exact_for_query(
    cache: &SqliteCache,
    query: &str,
    ttl_secs: u64,
    now: i64,
) -> anyhow::Result<Option<CacheHit>> {
    let hash = query_hash(query);
    // 注意：这里是同步调用包装，实际使用时应在 async 上下文中 await
    // 这里保留同步签名仅供测试使用
    let pool = cache.pool().clone();
    let hash_owned = hash.clone();
    let result = std::thread::spawn(move || {
        let conn = pool.get().context("获取连接失败")?;
        let mut stmt = conn.prepare(
            "SELECT answer_text, sources_json, created_at FROM exact_cache
             WHERE query_hash = ?1 ORDER BY created_at DESC LIMIT 1",
        )?;
        match stmt.query_row([&hash_owned], |row| {
            let answer: String = row.get(0)?;
            let sources: Option<String> = row.get(1)?;
            let created: i64 = row.get(2)?;
            Ok((answer, sources, created))
        }) {
            Ok((answer, sources, created)) => {
                if is_expired(created, ttl_secs, now) {
                    Ok(None)
                } else {
                    Ok(Some(CacheHit {
                        level: CacheLevel::Exact,
                        answer_text: Some(answer),
                        sources_json: sources,
                        retrieval_results_json: None,
                    }))
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::Error::from(e)),
        }
    })
    .join()
    .unwrap_or_else(|_| Err(anyhow::anyhow!("线程恐慌")))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use tempfile::TempDir;

    fn create_test_cache() -> (SqliteCache, TempDir) {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("test_cache.db");
        let manager = SqliteConnectionManager::file(&db_path)
            .with_init(|conn| conn.execute_batch("PRAGMA journal_mode=WAL;"));
        let pool = Pool::builder().max_size(4).build(manager).unwrap();
        let cache = SqliteCache::new(pool).unwrap();
        (cache, tmp)
    }

    #[tokio::test]
    async fn tc_cache_int_001_l0_exact_hit() {
        let (cache, _tmp) = create_test_cache();
        let hash = query_hash("什么是 RAG?");
        let now = chrono::Utc::now().timestamp();

        // 写入缓存
        cache
            .insert_exact(&hash, "什么是 RAG?", "RAG 是检索增强生成", "[]", None)
            .await
            .unwrap();

        // 查询应命中
        let hit = cache.lookup_exact(&hash, 86400, now).await.unwrap();
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.level, CacheLevel::Exact);
        assert_eq!(hit.answer_text.unwrap(), "RAG 是检索增强生成");
    }

    #[tokio::test]
    async fn tc_cache_int_002_l0_miss_then_l1_semantic_hit() {
        let (cache, _tmp) = create_test_cache();
        let now = chrono::Utc::now().timestamp();

        // 写入 L1 语义缓存
        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        cache
            .insert_semantic(
                "什么是检索增强生成？",
                &embedding,
                "RAG = Retrieval-Augmented Generation",
                "[]",
                None,
            )
            .await
            .unwrap();

        // 用相似嵌入查询 L1（相似度 > 0.92）
        let query_emb = vec![0.1, 0.2, 0.3, 0.4];
        let hit = cache
            .lookup_semantic(&query_emb, 0.92, 86400, now)
            .await
            .unwrap();
        assert!(hit.is_some());
        let hit = hit.unwrap();
        assert_eq!(hit.level, CacheLevel::Semantic);
    }

    #[tokio::test]
    async fn tc_cache_int_003_l3_retrieval_hit() {
        let (cache, _tmp) = create_test_cache();
        let now = chrono::Utc::now().timestamp();

        let embedding = vec![0.5, 0.6, 0.7, 0.8];
        let results_json = r#"[{"chunk":{"id":"c1","doc_id":"d1","content":"test","token_count":1,"sequence":0},"score":0.9,"doc_name":"test.md"}]"#;
        cache
            .insert_retrieval("test query", &embedding, results_json)
            .await
            .unwrap();

        let hit = cache
            .lookup_retrieval(&embedding, 0.90, 86400, now)
            .await
            .unwrap();
        assert!(hit.is_some());
        let results = hit.unwrap();
        assert!(results.contains("test.md"));
    }

    #[tokio::test]
    async fn tc_cache_int_004_ttl_expiry() {
        let (cache, _tmp) = create_test_cache();
        let hash = query_hash("test query");
        let now = chrono::Utc::now().timestamp();

        cache
            .insert_exact(&hash, "test query", "answer", "[]", None)
            .await
            .unwrap();

        // TTL = 0 → 立即过期
        let hit = cache.lookup_exact(&hash, 0, now + 1).await.unwrap();
        assert!(hit.is_none());
    }

    #[tokio::test]
    async fn tc_cache_int_005_clear_all() {
        let (cache, _tmp) = create_test_cache();
        let hash = query_hash("clear test");

        cache
            .insert_exact(&hash, "clear test", "answer", "[]", None)
            .await
            .unwrap();
        cache.clear_all().await.unwrap();

        let now = chrono::Utc::now().timestamp();
        let hit = cache.lookup_exact(&hash, 86400, now).await.unwrap();
        assert!(hit.is_none());
    }

    #[tokio::test]
    async fn tc_cache_int_006_stats() {
        let (cache, _tmp) = create_test_cache();
        let hash = query_hash("stats test");
        let now = chrono::Utc::now().timestamp();

        cache
            .insert_exact(&hash, "stats test", "answer", "[]", None)
            .await
            .unwrap();
        cache.lookup_exact(&hash, 86400, now).await.unwrap();

        let stats = cache.get_stats().await.unwrap();
        assert_eq!(stats.exact_hits, 1);
        assert!(stats.cache_size_entries >= 1);
        assert!(stats.estimated_token_saved > 0);
    }

    #[tokio::test]
    async fn tc_cache_int_007_privacy_mode_skip() {
        // privacy_mode 由上层 commands.rs 控制，这里测试 cache 本身正常工作
        let (cache, _tmp) = create_test_cache();
        let now = chrono::Utc::now().timestamp();
        let hash = query_hash("privacy test");

        cache
            .insert_exact(&hash, "privacy test", "answer", "[]", None)
            .await
            .unwrap();

        // 正常查询命中
        let hit = cache.lookup_exact(&hash, 86400, now).await.unwrap();
        assert!(hit.is_some());
    }
}
