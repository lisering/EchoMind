#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：REQ-PERF-016 向量缓存正确性修复（TC-VEC-CACHE-001~006）
//! + REQ-PERF-017 余弦相似度归一化预计算（TC-COS-001~004）。
//!
//! 背景：LruVectorCache 容量硬编码 5000，知识库向量数超过上限时
//! `from_vectors()` 静默驱逐后段向量，导致检索结果缺失 chunk（正确性 bug）。
//! v2.2 修复：缓存改为 Arc 快照语义（零深拷贝），容量仅作内存预算守卫，
//! 归一化预计算将余弦相似度降为点积（3×乘加/dim → 1×）。

use echomind_core::Storage;
use echomind_models::{Chunk, DocStatus, Document};
use tempfile::TempDir;

use crate::sqlite_storage::SqliteStorage;

/// 辅助：创建带 N 个文档 + N 个 chunk + N 个嵌入的临时存储。
/// 返回 (TempDir 保活, SqliteStorage, chunk_ids)。
async fn setup_with_vectors(n: usize) -> (TempDir, SqliteStorage, Vec<String>) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("echomind.db");
    let storage = SqliteStorage::new(&db_path).unwrap();

    let mut chunk_ids = Vec::with_capacity(n);
    for i in 0..n {
        let doc = Document::new(format!("doc-{i}.md"), format!("hash-{i}"));
        storage.add_document(&doc).await.unwrap();
        let chunk = Chunk::new(doc.id.clone(), format!("内容 {i}"), 4, 0);
        storage.add_chunk(&chunk).await.unwrap();
        // 每个向量沿第 i 维突出（第 0 维梯度便于排序断言）
        let mut v = vec![0.05; 8];
        v[0] = 0.5 + i as f32 * 0.1;
        storage.add_embedding(&chunk.id, &v).await.unwrap();
        chunk_ids.push(chunk.id);
    }
    (dir, storage, chunk_ids)
}

/// 辅助：构造 8 维已知向量（便于精确断言）。
fn vec_with_first(x: f32) -> Vec<f32> {
    let mut v = vec![0.1; 8];
    v[0] = x;
    v
}

// ============================================================================
// REQ-PERF-016 TC-VEC-CACHE-001~006
// ============================================================================

/// TC-VEC-CACHE-001：容量上限不截断检索结果（正确性修复核心）。
///
/// 3 个向量 + max_vectors=2（< 总数）。旧实现 `from_vectors` 会驱逐 1 个向量，
/// 使检索静默缺失。新实现容量仅作内存预算守卫：不缓存但检索仍完整。
#[tokio::test]
async fn tc_vec_cache_001_cap_does_not_drop_vectors() {
    let (_dir, mut storage, _ids) = setup_with_vectors(3).await;
    storage.set_max_vectors(2);

    // 查询偏向第 3 个向量（得分最高）
    let hits = storage.vector_search(&vec_with_first(0.8), 5).await.unwrap();

    assert_eq!(hits.len(), 3, "容量上限不得导致检索结果缺失 chunk");
    // 得分降序
    for w in hits.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "结果必须按分数降序: {} >= {}",
            w[0].score,
            w[1].score
        );
    }
    // 最高分应属于第一维 0.8 最接近的向量（i=2 → v[0]=0.7）
    assert_eq!(hits[0].doc_name, "doc-2.md", "查询 0.8 最接近第 3 个向量");
}

/// TC-VEC-CACHE-002：max_vectors=0（自动预算）= 缓存全部向量。
#[tokio::test]
async fn tc_vec_cache_002_auto_capacity_zero() {
    let (_dir, mut storage, _ids) = setup_with_vectors(3).await;
    storage.set_max_vectors(0);

    let hits = storage.vector_search(&vec_with_first(0.55), 3).await.unwrap();
    assert_eq!(hits.len(), 3, "自动预算应检索全部向量");
}

/// TC-VEC-CACHE-003：缓存命中路径语义等价（Arc 快照，零深拷贝）。
///
/// 首次检索填充缓存，第二次检索命中缓存。两次结果必须一致。
#[tokio::test]
async fn tc_vec_cache_003_hit_path_equivalence() {
    let (_dir, storage, _ids) = setup_with_vectors(3).await;

    let first = storage.vector_search(&vec_with_first(0.6), 3).await.unwrap();
    let second = storage.vector_search(&vec_with_first(0.6), 3).await.unwrap();

    assert_eq!(first.len(), second.len(), "缓存命中与未命中结果数量一致");
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.chunk.id, b.chunk.id, "缓存命中路径 chunk 序列一致");
        assert!(
            (a.score - b.score).abs() < 1e-6,
            "缓存命中路径分数一致: {} vs {}",
            a.score,
            b.score
        );
    }
}

/// TC-VEC-CACHE-004：top-k 精确排序（select_nth + 部分排序路径）。
///
/// 10 个向量第一维梯度 0.5..=1.4，查询 1.0 → 期望 i=5..9 依次为 top-5。
#[tokio::test]
async fn tc_vec_cache_004_top_k_exact_ordering() {
    let (_dir, storage, _ids) = setup_with_vectors(10).await;

    let hits = storage.vector_search(&vec_with_first(1.0), 5).await.unwrap();
    assert_eq!(hits.len(), 5);

    // 第一维 0.5 + i*0.1，查询 1.0 → 距离 |1.0 - (0.5 + i*0.1)| 升序：i=5,4,6,3,7
    let expected = ["doc-5.md", "doc-4.md", "doc-6.md", "doc-3.md", "doc-7.md"];
    for (i, exp) in expected.iter().enumerate() {
        assert_eq!(&hits[i].doc_name, exp, "top-{} 应为 {exp}", i + 1);
    }
}

/// TC-VEC-CACHE-005：max_vectors getter/setter 往返。
#[test]
fn tc_vec_cache_005_max_vectors_roundtrip() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("echomind.db");
    let mut storage = SqliteStorage::new(&db_path).unwrap();

    assert_eq!(storage.max_vectors(), 0, "默认应为 0（自动预算）");
    storage.set_max_vectors(1234);
    assert_eq!(storage.max_vectors(), 1234, "set_max_vectors 往返一致");
    storage.set_max_vectors(0);
    assert_eq!(storage.max_vectors(), 0, "恢复自动预算");
}

/// TC-VEC-CACHE-006：写入后缓存失效 + 检索包含新向量。
#[tokio::test]
async fn tc_vec_cache_006_write_invalidates_cache() {
    let (_dir, storage, _ids) = setup_with_vectors(2).await;
    // 填充缓存
    let before = storage.vector_search(&vec_with_first(0.6), 5).await.unwrap();
    assert_eq!(before.len(), 2);

    // 新增文档 + 向量（第一维 0.95，最接近查询 1.0）
    let doc = Document::new("doc-new.md".to_string(), "hash-new".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = Chunk::new(doc.id.clone(), "新内容".to_string(), 4, 0);
    storage.add_chunk(&chunk).await.unwrap();
    storage.add_embedding(&chunk.id, &vec_with_first(0.95)).await.unwrap();

    let after = storage.vector_search(&vec_with_first(1.0), 5).await.unwrap();
    assert_eq!(after.len(), 3, "写入后缓存失效，新向量参与检索");
    assert_eq!(after[0].doc_name, "doc-new.md", "新向量最接近查询");
}

// ============================================================================
// REQ-PERF-017 TC-COS-001~004
// ============================================================================

/// TC-COS-001：归一化点积与余弦相似度等价（误差 ≤ 1e-6）。
#[test]
fn tc_cos_001_normalized_dot_equals_cosine() {
    let pairs: Vec<(Vec<f32>, Vec<f32>)> = vec![
        (vec![1.0, 0.0], vec![1.0, 0.0]),
        (vec![1.0, 0.0], vec![0.0, 1.0]),
        (vec![3.0, 4.0], vec![4.0, 3.0]),
        (vec![0.5, -0.5, 0.25], vec![-0.25, 0.75, 0.5]),
        (vec![0.1; 384], vec![0.2; 384]),
    ];
    for (a, b) in pairs {
        let mut na = a.clone();
        let mut nb = b.clone();
        crate::sqlite_storage::normalize_in_place(&mut na);
        crate::sqlite_storage::normalize_in_place(&mut nb);
        let dot = crate::sqlite_storage::dot_product(&na, &nb);
        let cos = crate::sqlite_storage::cosine_similarity(&a, &b);
        assert!(
            (dot - cos).abs() < 1e-6,
            "归一化点积 {dot} 应等于余弦 {cos}"
        );
    }
}

/// TC-COS-002：零向量防御 — 不产生 NaN，得分为 0.0。
#[test]
fn tc_cos_002_zero_vector_no_nan() {
    let mut zero = vec![0.0; 8];
    crate::sqlite_storage::normalize_in_place(&mut zero);
    let mut q = vec_with_first(0.9);
    crate::sqlite_storage::normalize_in_place(&mut q);
    let score = crate::sqlite_storage::dot_product(&q, &zero);
    assert!(score.is_finite(), "零向量不得产生 NaN");
    assert_eq!(score, 0.0, "零向量得分应为 0.0");
}

/// TC-COS-003：查询向量非单位时结果与余弦等价（归一化后比较）。
#[test]
fn tc_cos_003_non_unit_query_equivalence() {
    // 查询向量放大 5 倍 + 平移不影响归一化后余弦
    let cand = vec![0.3, 0.4, 0.5, 0.6];
    let q_raw = vec![1.5, -2.0, 3.5, -4.0];
    let q_scaled = q_raw.iter().map(|x| x * 5.0 + 1.0).collect::<Vec<_>>();

    let mut nq1 = q_raw.clone();
    let mut nq2 = q_scaled.clone();
    let mut nc = cand.clone();
    crate::sqlite_storage::normalize_in_place(&mut nq1);
    crate::sqlite_storage::normalize_in_place(&mut nq2);
    crate::sqlite_storage::normalize_in_place(&mut nc);

    let d1 = crate::sqlite_storage::dot_product(&nq1, &nc);
    let d2 = crate::sqlite_storage::dot_product(&nq2, &nc);
    let cos = crate::sqlite_storage::cosine_similarity(&q_raw, &cand);
    assert!((d1 - cos).abs() < 1e-6, "归一化点积等于余弦: {d1} vs {cos}");
    assert!((d2 - cos).abs() < 1e-6, "查询缩放不影响余弦: {d2} vs {cos}");
}

/// TC-COS-004：DB 存储格式不变（load_all_embeddings 返回原始未归一化向量）。
#[tokio::test]
async fn tc_cos_004_load_all_embeddings_stays_raw() {
    let (_dir, storage, _ids) = setup_with_vectors(1).await;

    let loaded = storage.load_all_embeddings().await.unwrap();
    assert_eq!(loaded.len(), 1);
    let raw = &loaded[0].1;
    // 原始向量第一维应为 0.5（未归一化），模长 > 1
    assert!(
        (raw[0] - 0.5).abs() < 1e-6,
        "load_all_embeddings 必须返回 DB 原始值，第一维 0.5 实际 {}",
        raw[0]
    );
    let norm: f32 = raw.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() > 1e-3,
        "原始向量模长应 ≠ 1（未归一化），实际 {norm}"
    );
}

// ============================================================================
// 附加：文档状态护栏（setup 辅助的 sanity）
// ============================================================================

/// 辅助断言：setup_with_vectors 创建 Pending 状态文档（与导入管线一致）。
#[tokio::test]
async fn tc_vec_cache_extra_doc_status_pending() {
    let (_dir, storage, _ids) = setup_with_vectors(1).await;
    let docs = storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].status, DocStatus::Pending);
}
