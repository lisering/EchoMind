//! 核心工具函数（从 `sqlite_storage.rs` 拆分）。
//!
//! 包含：数据库任务执行、事务管理、状态转换、向量序列化、
//! 相似度计算、内存条目转换等基础设施。

use anyhow::{Context, bail};
use echomind_models::{DocStatus, Document, MemoryEntry, MemorySource, MemoryTier};
use tokio::task;

/// 在阻塞线程池执行数据库任务。
pub(crate) async fn run_db<T>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T>
where
    T: Send + 'static,
{
    task::spawn_blocking(f)
        .await
        .context("数据库任务执行失败")?
}

/// 在事务中执行数据库操作，错误时自动 ROLLBACK。
///
/// 修复 Bug #1：此前 `BEGIN IMMEDIATE` + `?` + `COMMIT` 模式在 `?` 返回 Err 时
/// 跳过 `COMMIT`，连接被放回池中但事务仍然打开，后续使用该连接的操作
/// 会意外追加到未提交的事务中，导致数据不一致。
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
    let tags_json: String = row.get(9).unwrap_or_else(|_| "[]".to_string());
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
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

pub(crate) fn row_to_memory_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryEntry> {
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
#[allow(clippy::chunks_exact_to_as_chunks)]
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
#[allow(dead_code)] // 保留供未来优化使用，当前未在生产代码中调用
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_vec_to_bytes_roundtrip() {
        let v = vec![1.0, -2.5, std::f32::consts::PI, 0.0, -0.0];
        let bytes = vec_to_bytes(&v);
        let restored = bytes_to_vec(&bytes).unwrap();
        assert_eq!(v, restored);
    }

    #[test]
    fn test_bytes_to_vec_invalid_len() {
        let bytes = vec![1, 2, 3]; // 非 4 的倍数
        assert!(bytes_to_vec(&bytes).is_err());
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 1e-6);

        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_normalize_in_place() {
        let mut v = vec![3.0, 4.0];
        normalize_in_place(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);

        let mut v = vec![0.0, 0.0];
        normalize_in_place(&mut v);
        assert_eq!(v, vec![0.0, 0.0]);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![0.5, 0.5, 0.5, 0.5];
        assert!((dot_product(&a, &b) - 5.0).abs() < 1e-6);
    }
}
