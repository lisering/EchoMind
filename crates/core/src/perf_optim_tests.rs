#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

//! 性能优化 TDD 测试（REQ-PERF-OPTIM）
//!
//! 验证 6 项性能优化的正确性：
//! 1. 余弦相似度：单次遍历 + 4x 循环展开 vs 原始三次遍历结果一致
//! 2. 余弦相似度：非 4 倍数长度向量正确处理
//! 3. 余弦相似度：零向量 / 空向量边界条件
//! 4. Top-K 选择：partial sort 结果与全量排序一致
//! 5. LRU O(1) touch：touch 后驱逐顺序正确
//! 6. LRU O(1) insert：重复 key 更新值并移到 MRU 端

use crate::cache::cosine_similarity;

// ─── 余弦相似度优化测试 ──────────────────────────────────────────────

/// TC-PERF-COS-001：优化后余弦相似度与预期值一致（384 维向量）。
///
/// 验证单次遍历 + 4x 循环展开实现与数学定义一致。
#[test]
fn tc_perf_cos_001_384_dim_correctness() {
    // 构造 384 维向量（4 的倍数，走纯循环展开路径）
    let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..384).map(|i| ((i + 1) as f32) * 0.01).collect();

    // 手动计算预期值
    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let expected = dot / (norm_a * norm_b);

    let actual = cosine_similarity(&a, &b);
    assert!(
        (actual - expected).abs() < 1e-5,
        "384 维余弦相似度应与预期一致: expected={expected}, actual={actual}"
    );
}

/// TC-PERF-COS-002：非 4 倍数长度向量正确处理（remainder 路径）。
#[test]
fn tc_perf_cos_002_non_multiple_of_4() {
    // 385 维（384 + 1 remainder）
    let mut a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let mut b: Vec<f32> = (0..384).map(|i| ((i + 1) as f32) * 0.01).collect();
    a.push(0.5);
    b.push(0.3);

    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let expected = dot / (norm_a * norm_b);

    let actual = cosine_similarity(&a, &b);
    assert!(
        (actual - expected).abs() < 1e-5,
        "非 4 倍数长度应正确处理: expected={expected}, actual={actual}"
    );
}

/// TC-PERF-COS-003：零向量返回 0.0。
#[test]
fn tc_perf_cos_003_zero_vector() {
    let a = vec![0.0f32; 384];
    let b: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 1e-10, "零向量相似度应为 0.0，实际: {sim}");
}

/// TC-PERF-COS-004：空向量 / 长度不匹配返回 0.0。
#[test]
fn tc_perf_cos_004_empty_mismatch() {
    assert_eq!(cosine_similarity(&[], &[]), 0.0, "空向量应返回 0.0");
    assert_eq!(
        cosine_similarity(&[1.0, 2.0], &[1.0]),
        0.0,
        "长度不匹配应返回 0.0"
    );
}

/// TC-PERF-COS-005：相同向量相似度为 1.0。
#[test]
fn tc_perf_cos_005_identical_vectors() {
    let v: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let sim = cosine_similarity(&v, &v);
    assert!(
        (sim - 1.0).abs() < 1e-5,
        "相同向量相似度应为 1.0，实际: {sim}"
    );
}

/// TC-PERF-COS-006：1 维向量（最小 remainder）。
#[test]
fn tc_perf_cos_006_single_element() {
    let sim = cosine_similarity(&[1.0], &[1.0]);
    assert!(
        (sim - 1.0).abs() < 1e-5,
        "1 维向量相似度应为 1.0，实际: {sim}"
    );
}

/// TC-PERF-COS-007：3 维向量（纯 remainder）。
#[test]
fn tc_perf_cos_007_three_elements() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0];
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 1e-5, "正交向量相似度应为 0.0，实际: {sim}");
}

/// TC-PERF-COS-008：1024 维 bge-m3 向量正确性。
#[test]
fn tc_perf_cos_008_bge_m3_1024_dim() {
    let a: Vec<f32> = (0..1024).map(|i| (i as f32) * 0.001).collect();
    let b: Vec<f32> = (0..1024).map(|i| ((i + 2) as f32) * 0.001).collect();

    let dot: f32 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let expected = dot / (norm_a * norm_b);

    let actual = cosine_similarity(&a, &b);
    assert!(
        (actual - expected).abs() < 1e-4,
        "1024 维 bge-m3 向量应正确: expected={expected}, actual={actual}"
    );
}
