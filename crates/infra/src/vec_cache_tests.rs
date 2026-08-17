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
//!
//! 向量编码约定：8 维空间中仅前 2 维非零，向量 = [cos(a), sin(a), 0..0]。
//! 余弦相似度 = cos(a_i - a_query)，随 |a_i - a_query| 单调递减，排序确定无并列。

use echomind_core::Storage;
use echomind_models::{Chunk, DocStatus, Document};
use tempfile::TempDir;

use crate::sqlite_storage::SqliteStorage;

/// 构造 8 维向量，仅前 2 维非零：[cos(angle), sin(angle), 0..0]。
fn angle_vec(angle: f32) -> Vec<f32> {
    let mut v = vec![0.0f32; 8];
    v[0] = angle.cos();
    v[1] = angle.sin();
    v
}

/// 辅助：创建带 N 个文档 + N 个 chunk + N 个嵌入的临时存储。
/// 第 i 个向量角度 = i * 0.1 rad。返回 (TempDir 保活, SqliteStorage, chunk_ids)。
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
        storage
            .add_embedding(&chunk.id, &angle_vec(i as f32 * 0.1))
            .await
            .unwrap();
        chunk_ids.push(chunk.id);
    }
    (dir, storage, chunk_ids)
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

    // 查询角度 0.19 → 距离排序：i=2(0.2) 0.01 < i=1(0.1) 0.09 < i=0(0) 0.19
    let hits = storage.vector_search(&angle_vec(0.19), 5).await.unwrap();

    assert_eq!(hits.len(), 3, "容量上限不得导致检索结果缺失 chunk");
    for w in hits.windows(2) {
        assert!(
            w[0].score >= w[1].score,
            "结果必须按分数降序: {} >= {}",
            w[0].score,
            w[1].score
        );
    }
    assert_eq!(hits[0].doc_name, "doc-2.md", "查询 0.19 最接近第 3 个向量");
}

/// TC-VEC-CACHE-002：max_vectors=0（自动预算）= 缓存全部向量。
#[tokio::test]
async fn tc_vec_cache_002_auto_capacity_zero() {
    let (_dir, mut storage, _ids) = setup_with_vectors(3).await;
    storage.set_max_vectors(0);

    let hits = storage.vector_search(&angle_vec(0.05), 3).await.unwrap();
    assert_eq!(hits.len(), 3, "自动预算应检索全部向量");
}

/// TC-VEC-CACHE-003：缓存命中路径语义等价（Arc 快照，零深拷贝）。
#[tokio::test]
async fn tc_vec_cache_003_hit_path_equivalence() {
    let (_dir, storage, _ids) = setup_with_vectors(3).await;

    let first = storage.vector_search(&angle_vec(0.15), 3).await.unwrap();
    let second = storage.vector_search(&angle_vec(0.15), 3).await.unwrap();

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
/// 10 个向量角度 0..0.9 step 0.1，查询 0.53 → top-5 依次 i=5,6,4,7,3。
#[tokio::test]
async fn tc_vec_cache_004_top_k_exact_ordering() {
    let (_dir, storage, _ids) = setup_with_vectors(10).await;

    let hits = storage.vector_search(&angle_vec(0.53), 5).await.unwrap();
    assert_eq!(hits.len(), 5);

    let expected = ["doc-5.md", "doc-6.md", "doc-4.md", "doc-7.md", "doc-3.md"];
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
    // 填充缓存（角度 0 / 0.1）
    let before = storage.vector_search(&angle_vec(0.05), 5).await.unwrap();
    assert_eq!(before.len(), 2);

    // 新增向量角度 0.03（比 0 / 0.1 都更接近查询 0.02）
    let doc = Document::new("doc-new.md".to_string(), "hash-new".to_string());
    storage.add_document(&doc).await.unwrap();
    let chunk = Chunk::new(doc.id.clone(), "新内容".to_string(), 4, 0);
    storage.add_chunk(&chunk).await.unwrap();
    storage
        .add_embedding(&chunk.id, &angle_vec(0.03))
        .await
        .unwrap();

    let after = storage.vector_search(&angle_vec(0.02), 5).await.unwrap();
    assert_eq!(after.len(), 3, "写入后缓存失效，新向量参与检索");
    assert_eq!(after[0].doc_name, "doc-new.md", "新向量最接近查询");
}

// ============================================================================
// REQ-PERF-017 TC-COS-001~004
// ============================================================================

/// TC-COS-001：归一化点积与余弦相似度等价（误差 ≤ 1e-4，f32 累积）。
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
            (dot - cos).abs() < 1e-4,
            "归一化点积 {dot} 应等于余弦 {cos}"
        );
    }
}

/// TC-COS-002：零向量防御 — 不产生 NaN，得分为 0.0。
#[test]
fn tc_cos_002_zero_vector_no_nan() {
    let mut zero = vec![0.0; 8];
    crate::sqlite_storage::normalize_in_place(&mut zero);
    let mut q = angle_vec(0.9);
    crate::sqlite_storage::normalize_in_place(&mut q);
    let score = crate::sqlite_storage::dot_product(&q, &zero);
    assert!(score.is_finite(), "零向量不得产生 NaN");
    assert_eq!(score, 0.0, "零向量得分应为 0.0");
}

/// TC-COS-003：查询向量非单位时结果与余弦等价（正标量缩放不改变余弦）。
#[test]
fn tc_cos_003_non_unit_query_equivalence() {
    let cand = vec![0.3, 0.4, 0.5, 0.6];
    let q_raw = vec![1.5, -2.0, 3.5, -4.0];
    // 纯正标量缩放（方向不变），归一化后余弦不变
    let q_scaled = q_raw.iter().map(|x| x * 5.0).collect::<Vec<_>>();

    let mut nq1 = q_raw.clone();
    let mut nq2 = q_scaled.clone();
    let mut nc = cand.clone();
    crate::sqlite_storage::normalize_in_place(&mut nq1);
    crate::sqlite_storage::normalize_in_place(&mut nq2);
    crate::sqlite_storage::normalize_in_place(&mut nc);

    let d1 = crate::sqlite_storage::dot_product(&nq1, &nc);
    let d2 = crate::sqlite_storage::dot_product(&nq2, &nc);
    let cos = crate::sqlite_storage::cosine_similarity(&q_raw, &cand);
    assert!((d1 - cos).abs() < 1e-4, "归一化点积等于余弦: {d1} vs {cos}");
    assert!((d2 - cos).abs() < 1e-4, "查询缩放不影响余弦: {d2} vs {cos}");
}

/// TC-COS-004：DB 存储格式不变（load_all_embeddings 返回原始未归一化向量）。
#[tokio::test]
async fn tc_cos_004_load_all_embeddings_stays_raw() {
    let (_dir, storage, _ids) = setup_with_vectors(1).await;

    let loaded = storage.load_all_embeddings().await.unwrap();
    assert_eq!(loaded.len(), 1);
    let raw = &loaded[0].1;
    // 原始向量 [cos(0), sin(0), 0..] = [1.0, 0.0, 0..]，模长 = 1，但方向已知
    assert!(
        (raw[0] - 1.0).abs() < 1e-6,
        "load_all_embeddings 必须返回 DB 原始值，第一维 1.0 实际 {}",
        raw[0]
    );
    assert!(
        (raw[1] - 0.0).abs() < 1e-6,
        "第二维应为 0.0 实际 {}",
        raw[1]
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
