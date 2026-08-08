#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use crate::cache::{
    cosine_similarity, embedding_from_bytes, embedding_to_bytes, estimate_rag_token_cost,
    is_expired, normalize_query, query_hash,
};

/// TC-CACHE-001: normalize_query 去除多余空白 + 小写化
#[test]
fn tc_cache_001_normalize_query_strips_whitespace_and_lowercases() {
    assert_eq!(normalize_query("  Hello   World  "), "hello world");
    assert_eq!(normalize_query("  Multiple   Spaces  "), "multiple spaces");
    assert_eq!(normalize_query("Already Clean"), "already clean");
    assert_eq!(normalize_query(""), "");
    assert_eq!(normalize_query("   "), "");
}

/// TC-CACHE-002: query_hash 相同归一化查询产生相同哈希
#[test]
fn tc_cache_002_query_hash_consistent_for_normalized_equivalents() {
    // 相同语义不同格式产生相同哈希
    assert_eq!(query_hash("  Hello   World  "), query_hash("hello world"));
    assert_eq!(query_hash("Hello\nWorld"), query_hash("HELLO WORLD"));
    assert_eq!(query_hash("RAG  优化"), query_hash("rag 优化"));

    // 不同查询产生不同哈希
    assert_ne!(query_hash("Hello World"), query_hash("Goodbye World"));

    // 空查询产生确定哈希
    let empty_hash = query_hash("");
    assert_eq!(empty_hash.len(), 64); // SHA-256 hex = 64 chars
}

/// TC-CACHE-003: cosine_similarity 相同向量为 1.0
#[test]
fn tc_cache_003_cosine_similarity_identical_vectors() {
    let v = vec![1.0, 2.0, 3.0, 4.0];
    let sim = cosine_similarity(&v, &v);
    assert!((sim - 1.0).abs() < 1e-6);
}

/// TC-CACHE-004: cosine_similarity 正交向量为 0.0
#[test]
fn tc_cache_004_cosine_similarity_orthogonal_vectors() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 1e-6);
}

/// TC-CACHE-005: cosine_similarity 长度不匹配返回 0.0
#[test]
fn tc_cache_005_cosine_similarity_mismatched_lengths() {
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![1.0, 2.0];
    let sim = cosine_similarity(&a, &b);
    assert!(sim.abs() < 1e-6);
}

/// TC-CACHE-006: cosine_similarity 空向量返回 0.0
#[test]
fn tc_cache_006_cosine_similarity_empty_vectors() {
    let empty: Vec<f32> = vec![];
    let v = vec![1.0, 2.0];
    assert!(cosine_similarity(&empty, &v).abs() < 1e-6);
    assert!(cosine_similarity(&v, &empty).abs() < 1e-6);
    assert!(cosine_similarity(&empty, &empty).abs() < 1e-6);
}

/// TC-CACHE-007: cosine_similarity 相似向量高分数
#[test]
fn tc_cache_007_cosine_similarity_similar_vectors_high_score() {
    let a = vec![1.0, 0.5, 0.3, 0.8];
    let b = vec![1.1, 0.48, 0.29, 0.82];
    let sim = cosine_similarity(&a, &b);
    assert!(
        sim > 0.99,
        "similar vectors should have high cosine sim, got {sim}"
    );
}

/// TC-CACHE-008: embedding_to_bytes / embedding_from_bytes roundtrip
#[test]
fn tc_cache_008_embedding_bytes_roundtrip() {
    let original = vec![1.0, -2.5, 3.15, 0.0, 42.0];
    let bytes = embedding_to_bytes(&original);
    let recovered = embedding_from_bytes(&bytes);
    assert_eq!(recovered.len(), original.len());
    for (a, b) in original.iter().zip(recovered.iter()) {
        assert!((a - b).abs() < 1e-6);
    }
}

/// TC-CACHE-009: embedding_from_bytes 非法长度返回空向量
#[test]
fn tc_cache_009_embedding_from_bytes_invalid_length() {
    let bad_bytes = [1, 2, 3]; // 3 bytes = not divisible by 4
    let result = embedding_from_bytes(&bad_bytes);
    assert!(result.is_empty());
}

/// TC-CACHE-010: is_expired 正确判断过期
#[test]
fn tc_cache_010_is_expired_logic() {
    let now = 1000;
    let ttl = 3600; // 1 hour

    // 刚创建，未过期
    assert!(!is_expired(now, ttl, now));
    assert!(!is_expired(now - 100, ttl, now));

    // TTL 临界点
    assert!(!is_expired(now - 3600, ttl, now)); // exactly TTL seconds ago → not expired
    assert!(is_expired(now - 3601, ttl, now)); // one second past TTL → expired

    // 很久以前创建，已过期
    assert!(is_expired(now - 100000, ttl, now));
}

/// TC-CACHE-011: estimate_rag_token_cost 返回正值
#[test]
fn tc_cache_011_estimate_rag_token_cost_positive() {
    let cost = estimate_rag_token_cost();
    assert!(cost > 0);
    assert!(cost >= 1000); // 至少 1000 token 的估算
}

/// TC-CACHE-012: normalize_query 处理中文和特殊字符
#[test]
fn tc_cache_012_normalize_query_chinese_and_special_chars() {
    // 中文不受影响（仅小写化不影响中文）
    assert_eq!(normalize_query("你好 World"), "你好 world");
    assert_eq!(normalize_query("  RAG  优化  "), "rag 优化");

    // 换行和制表符被当作空白
    assert_eq!(normalize_query("Hello\tWorld\nRAG"), "hello world rag");

    // 标点符号保留
    assert_eq!(normalize_query("What is RAG?"), "what is rag?");
}
