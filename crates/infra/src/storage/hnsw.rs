//! HNSW 向量索引与内存快照缓存（从 `sqlite_storage.rs` 拆分）。
//!
//! 包含：HNSW 索引构建/加载/搜索/持久化、向量快照缓存管理、
//! 向量搜索核心逻辑（HNSW 快速路径 + 内存全量扫描回退）。
//!
//! 注：此模块仅用于测试/参考，实际生产代码使用 `hnsw_index.rs` 实现。
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use echomind_models::{Chunk, RetrievalResult};

use super::core::{bytes_to_vec, dot_product, normalize_in_place, run_db};
use super::migration::Pool;
use crate::sqlite_storage::{
    VectorCache, VectorSnapshot, crypto_decrypt_bytes, crypto_encrypt_bytes,
};

/// 从 SQLite 加载全部 embeddings（HNSW 索引构建用）。
///
/// 返回 `(chunk_id, vector)` 对列表，按 doc_id + sequence 排序。
pub(crate) async fn load_all_embeddings(pool: &Pool) -> anyhow::Result<Vec<(String, Vec<f32>)>> {
    let pool = pool.clone();
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

/// 按 chunk_id 列表批量查询 chunk 详情（HNSW 查询后获取详情用）。
///
/// 返回 `Vec<RetrievalResult>`，score 字段为 0.0（由调用方从 HNSW 结果设置）。
pub(crate) async fn get_chunks_by_ids(
    pool: &Pool,
    ids: &[String],
) -> anyhow::Result<Vec<RetrievalResult>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let pool = pool.clone();
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

/// HNSW 索引搜索参数。
pub(crate) struct HnswSearchParams<'a> {
    pub query_embedding: &'a [f32],
    pub top_k: usize,
    pub vectors: VectorSnapshot,
    pub hnsw: Arc<std::sync::Mutex<Option<crate::hnsw_index::HnswIndex>>>,
    pub hnsw_dirty: Arc<std::sync::atomic::AtomicBool>,
    pub hnsw_path: std::path::PathBuf,
    pub cipher: aes_gcm::Aes256Gcm,
}

/// 执行 HNSW 搜索（构建/加载索引 + 搜索）。
///
/// 返回 `(chunk_id, score)` 对列表。
pub(crate) async fn search_hnsw(
    params: HnswSearchParams<'_>,
) -> anyhow::Result<Option<Vec<(String, f32)>>> {
    use crate::hnsw_index::HnswIndex;

    let HnswSearchParams {
        query_embedding,
        top_k,
        vectors,
        hnsw,
        hnsw_dirty,
        hnsw_path,
        cipher,
    } = params;

    let query_dim = query_embedding.len();
    let (matched, excluded): (Vec<_>, Vec<_>) = vectors
        .iter()
        .cloned()
        .partition(|(_, v)| v.len() == query_dim);

    if matched.is_empty() {
        return Ok(None);
    }

    let (hnsw_guard, from_disk) = {
        let guard = hnsw.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() && !hnsw_dirty.load(std::sync::atomic::Ordering::SeqCst) {
            let loaded = std::fs::read(&hnsw_path)
                .map_err(anyhow::Error::from)
                .and_then(|enc| crypto_decrypt_bytes(&cipher, &enc))
                .and_then(|plain| HnswIndex::deserialize_binary(&plain, query_dim));
            match loaded {
                Ok(idx) => (idx, true),
                Err(e) => {
                    tracing::warn!("HNSW 磁盘索引加载失败，回退全量构建: {e:#}");
                    (HnswIndex::build(&matched)?, false)
                }
            }
        } else {
            (HnswIndex::build(&matched)?, false)
        }
    };

    let hits = hnsw_guard.search(query_embedding, top_k * 4);
    let mut results: Vec<(String, f32)> = hits
        .into_iter()
        .map(|(id, dist)| (id, 1.0 - dist))
        .collect();

    if !excluded.is_empty() {
        results.extend(excluded.into_iter().map(|(id, _)| (id, 1.0_f32)));
    }

    if !from_disk {
        match hnsw_guard
            .serialize_binary()
            .and_then(|plain| crypto_encrypt_bytes(&cipher, &plain))
        {
            Ok(enc) => {
                if let Err(e) = std::fs::write(&hnsw_path, enc) {
                    tracing::warn!("HNSW 索引落盘失败: {e:#}");
                }
            }
            Err(e) => tracing::warn!("HNSW 索引序列化/加密失败: {e:#}"),
        }
    }

    *hnsw.lock().unwrap_or_else(|e| e.into_inner()) = Some(hnsw_guard);
    hnsw_dirty.store(false, std::sync::atomic::Ordering::SeqCst);

    Ok(Some(results))
}

/// 内存全量扫描搜索（归一化向量点积，O(N) select_nth 取 top-k）。
///
/// Arc 快照仅克隆指针，零全量深拷贝。
pub(crate) async fn search_memory_scan(
    query_embedding: &[f32],
    top_k: usize,
    vectors: VectorSnapshot,
) -> anyhow::Result<Vec<(String, f32)>> {
    let query_for_scan = query_embedding.to_vec();
    let snapshot = Arc::clone(&vectors);
    let top_k_val = top_k;

    let top_hits: Vec<(String, f32)> = tokio::task::spawn_blocking(move || {
        let vectors = &snapshot;
        let n = vectors.len();
        let mut scores: Vec<(usize, f32)> = Vec::with_capacity(n);
        for (i, (_, vector)) in vectors.iter().enumerate() {
            let score = dot_product(&query_for_scan, vector);
            scores.push((i, score));
        }

        if top_k_val < n {
            let (before, _pivot, _after) = scores.select_nth_unstable_by(top_k_val, |a, b| {
                b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut top = before.to_vec();
            top.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            top.into_iter()
                .map(|(idx, score)| (vectors[idx].0.clone(), score))
                .collect()
        } else {
            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            scores
                .into_iter()
                .map(|(idx, score)| (vectors[idx].0.clone(), score))
                .collect()
        }
    })
    .await
    .context("内存向量检索任务执行失败")?;

    Ok(top_hits)
}

/// 向量快照缓存管理。
pub(crate) struct VectorCacheManager {
    cache: VectorCache,
    max_vectors: usize,
}

impl VectorCacheManager {
    pub(crate) fn new(max_vectors: usize) -> Self {
        Self {
            cache: Arc::new(std::sync::RwLock::new(None)),
            max_vectors,
        }
    }

    /// 获取或构建向量快照。
    pub(crate) async fn get_or_build_snapshot(
        &self,
        load_fn: impl FnOnce() -> anyhow::Result<Vec<(String, Vec<f32>)>> + Send + 'static,
    ) -> anyhow::Result<VectorSnapshot> {
        let cached = {
            let guard = self.cache.read();
            guard.ok().and_then(|g| g.clone())
        };

        match cached {
            Some(snapshot) => Ok(snapshot),
            None => {
                let mut loaded = load_fn()?;
                for (_, v) in &mut loaded {
                    normalize_in_place(v);
                }
                let snapshot: VectorSnapshot = Arc::new(loaded);
                if (self.max_vectors == 0 || snapshot.len() <= self.max_vectors)
                    && let Ok(mut guard) = self.cache.write()
                {
                    *guard = Some(Arc::clone(&snapshot));
                }
                Ok(snapshot)
            }
        }
    }

    /// 使缓存失效。
    pub(crate) fn invalidate(&self) {
        if let Ok(mut guard) = self.cache.write() {
            *guard = None;
        }
    }

    /// 更新最大向量数限制。
    pub(crate) fn set_max_vectors(&mut self, max: usize) {
        self.max_vectors = max;
        self.invalidate();
    }

    pub(crate) fn max_vectors(&self) -> usize {
        self.max_vectors
    }
}

/// 数据库完整性检查结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IntegrityCheckResult {
    Ok,
    Corrupted(String),
}

/// 执行 `PRAGMA integrity_check` 检查数据库完整性（同步版本）。
pub(crate) fn check_integrity_sync(pool: &Pool) -> anyhow::Result<IntegrityCheckResult> {
    let conn = pool.get().context("获取数据库连接失败")?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("执行 PRAGMA integrity_check 失败")?;
    if result.eq_ignore_ascii_case("ok") {
        return Ok(IntegrityCheckResult::Ok);
    }

    tracing::warn!("数据库完整性检查异常: {result}，尝试 WAL checkpoint 恢复…");
    let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");

    let recheck: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("恢复后重新执行 PRAGMA integrity_check 失败")?;
    if recheck.eq_ignore_ascii_case("ok") {
        tracing::info!("WAL checkpoint 恢复成功，数据库完整性恢复正常");
        return Ok(IntegrityCheckResult::Ok);
    }

    let detail = format!("{} → {}", result, recheck);
    tracing::error!("数据库完整性恢复失败: {detail}");
    Ok(IntegrityCheckResult::Corrupted(detail))
}

/// 执行 `PRAGMA integrity_check` 的异步封装。
pub(crate) async fn check_integrity(pool: &Pool) -> anyhow::Result<IntegrityCheckResult> {
    let pool = pool.clone();
    run_db(move || {
        let conn = pool.get().context("获取数据库连接失败")?;
        let result: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("执行 PRAGMA integrity_check 失败")?;
        if result.eq_ignore_ascii_case("ok") {
            return Ok(IntegrityCheckResult::Ok);
        }

        tracing::warn!("数据库完整性检查异常: {result}，尝试 WAL checkpoint 恢复…");
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");

        let recheck: String = conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .context("恢复后重新执行 PRAGMA integrity_check 失败")?;
        if recheck.eq_ignore_ascii_case("ok") {
            tracing::info!("WAL checkpoint 恢复成功，数据库完整性恢复正常");
            return Ok(IntegrityCheckResult::Ok);
        }

        let detail = format!("{} → {}", result, recheck);
        tracing::error!("数据库完整性恢复失败: {detail}");
        Ok(IntegrityCheckResult::Corrupted(detail))
    })
    .await
}

/// 获取数据库文件路径（用于磁盘空间检查）。
pub(crate) fn get_db_path(pool: &Pool) -> std::path::PathBuf {
    let conn = pool.get().ok();
    if let Some(conn) = conn {
        let db_name: String = conn
            .query_row("PRAGMA database_list", [], |row| row.get(2))
            .unwrap_or_default();
        std::path::PathBuf::from(db_name)
    } else {
        std::path::PathBuf::from("echomind.db")
    }
}

/// 检查磁盘空间是否充足。
pub(crate) async fn check_disk_space(pool: &Pool, required_bytes: u64) -> anyhow::Result<()> {
    let db_path = get_db_path(pool);
    let path_for_check = if db_path.exists() {
        db_path
    } else {
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
        return Err(anyhow::anyhow!(
            "{}: {}",
            echomind_core::errors::ERR_DISK_FULL,
            e
        ));
    }
    Ok(())
}

/// 清理磁盘空间（临时文件 + WAL checkpoint）。
pub(crate) async fn cleanup_for_disk_space(pool: &Pool, data_dir: &Path) -> u64 {
    let data_dir = data_dir.to_path_buf();
    let freed =
        tokio::task::spawn_blocking(move || crate::disk_space::cleanup_temp_files(&data_dir))
            .await
            .unwrap_or(0);

    if freed > 0 {
        tracing::info!(
            "磁盘空间清理：释放了 {} 字节 ({:.1} MB)",
            freed,
            freed as f64 / (1024.0 * 1024.0)
        );
    }

    let pool = pool.clone();
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(conn) = pool.get() {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)");
        }
    })
    .await;

    freed
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::sqlite_storage::SqliteStorage;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_integrity_check() {
        let dir = TempDir::new().unwrap();
        let storage = SqliteStorage::new(&dir.path().join("integrity.db")).unwrap();
        let pool = storage.pool_clone();

        let result = check_integrity_sync(&pool).unwrap();
        assert_eq!(result, IntegrityCheckResult::Ok);

        let result = check_integrity(&pool).await.unwrap();
        assert_eq!(result, IntegrityCheckResult::Ok);
    }

    #[tokio::test]
    async fn test_vector_cache_manager() {
        let mut manager = VectorCacheManager::new(0);
        assert_eq!(manager.max_vectors(), 0);

        manager.set_max_vectors(100);
        assert_eq!(manager.max_vectors(), 100);
    }
}
