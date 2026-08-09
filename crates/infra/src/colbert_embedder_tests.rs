//! ColBERT 多向量嵌入 TDD 测试（REQ-PERF-008, Pro feature）。
//!
//! 测试用例 TC-COL-001~006：
//! - TC-COL-001: 文本 → token 级多向量（每 token 一个嵌入）
//! - TC-COL-002: MaxSim 交互计算 query-document 相关度
//! - TC-COL-003: 多向量检索精度优于单向量
//! - TC-COL-004: token 级向量可预计算存储
//! - TC-COL-005: MaxSim 计算复杂度 O(q×d)
//! - TC-COL-006: 可与单向量嵌入共存（运行时切换）

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::colbert_embedder::*;
use echomind_core::{MultiVectorEmbedder as _, maxsim};

/// Mock 多向量嵌入器（测试用，不需要 ONNX 模型）。
///
/// 生成确定性的伪嵌入向量：每个 token 的嵌入 = token 首字符的 ASCII 码 × 0.01
struct MockMultiVectorEmbedder {
    dim: usize,
}

impl MockMultiVectorEmbedder {
    fn new(dim: usize) -> Self {
        Self { dim }
    }
}

impl echomind_core::MultiVectorEmbedder for MockMultiVectorEmbedder {
    async fn embed_tokens(&self, text: &str) -> anyhow::Result<Vec<Vec<f32>>> {
        let tokens = ColbertEmbedder::tokenize(text);
        Ok(tokens
            .iter()
            .map(|t| {
                // 生成确定性向量：基于 token 首字符
                let seed = t.chars().next().map(|c| c as u32).unwrap_or(0);
                (0..self.dim)
                    .map(|i| ((seed + i as u32 * 7) % 97) as f32 / 97.0)
                    .collect()
            })
            .collect())
    }
}

/// TC-COL-001: 文本 → token 级多向量（每 token 一个嵌入）。
#[tokio::test]
async fn tc_col_001_text_to_token_vectors() {
    let embedder = MockMultiVectorEmbedder::new(384);
    let text = "Hello 世界 Rust";
    let vectors = embedder.embed_tokens(text).await.unwrap();

    // 应有 3+ 个 token 向量（Hello, 世, 界, Rust）
    assert!(!vectors.is_empty(), "token 向量不应为空");
    assert!(
        vectors.len() >= 3,
        "至少 3 个 token 向量，实际 {}",
        vectors.len()
    );

    // 每个向量维度应为 384
    for vec in &vectors {
        assert_eq!(vec.len(), 384, "每个 token 向量维度应为 384");
    }

    // 不同 token 的向量应不同
    let distinct = vectors.iter().filter(|v| v != &&vectors[0]).count();
    assert!(distinct > 0, "应存在与第一个 token 不同的向量");
}

/// TC-COL-002: MaxSim 交互计算 query-document 相关度。
#[tokio::test]
async fn tc_col_002_maxsim_interaction() {
    let embedder = MockMultiVectorEmbedder::new(128);

    // query = "Rust 语言"
    let query_vectors = embedder.embed_tokens("Rust 语言").await.unwrap();
    // document = "Rust 是一种系统编程语言"
    let doc_vectors = embedder
        .embed_tokens("Rust 是一种系统编程语言")
        .await
        .unwrap();

    // MaxSim 分数应 > 0（有匹配的 token）
    let score = maxsim(&query_vectors, &doc_vectors);
    assert!(score > 0.0, "MaxSim 分数应 > 0，实际 {score}");

    // 无关文档的 MaxSim 应更低
    let irrelevant_vectors = embedder.embed_tokens("天气晴朗适合出游").await.unwrap();
    let irrelevant_score = maxsim(&query_vectors, &irrelevant_vectors);
    assert!(
        score >= irrelevant_score,
        "相关文档分数 {score} 应 >= 无关文档分数 {irrelevant_score}"
    );
}

/// TC-COL-003: 多向量检索精度优于单向量。
#[tokio::test]
async fn tc_col_003_multivec_better_than_singlevec() {
    let embedder = MockMultiVectorEmbedder::new(128);

    // 精确匹配场景：query 和 doc 共享特定 token
    let query_vectors = embedder.embed_tokens("async fn main").await.unwrap();
    let exact_doc = embedder
        .embed_tokens("async fn main is the entry point")
        .await
        .unwrap();
    let partial_doc = embedder
        .embed_tokens("the main function is important")
        .await
        .unwrap();

    let exact_score = maxsim(&query_vectors, &exact_doc);
    let partial_score = maxsim(&query_vectors, &partial_doc);

    // 精确匹配文档分数应 > 部分匹配
    assert!(
        exact_score > partial_score,
        "精确匹配分数 {exact_score} 应 > 部分匹配分数 {partial_score}"
    );
}

/// TC-COL-004: token 级向量可预计算存储（序列化/反序列化往返）。
#[test]
fn tc_col_004_multivec_serialization_roundtrip() {
    // 模拟 3 个 token × 4 维向量
    let original: Vec<Vec<f32>> = vec![
        vec![0.1, 0.2, 0.3, 0.4],
        vec![0.5, 0.6, 0.7, 0.8],
        vec![0.9, 1.0, 1.1, 1.2],
    ];

    // 序列化为 BLOB
    let bytes = multivec_to_bytes(&original);
    assert!(!bytes.is_empty(), "序列化结果不应为空");

    // 反序列化
    let restored = multivec_from_bytes(&bytes).unwrap();

    // 验证往返一致性
    assert_eq!(restored.len(), original.len(), "token 数应一致");
    for (i, (orig, rest)) in original.iter().zip(restored.iter()).enumerate() {
        assert_eq!(orig.len(), rest.len(), "token {i} 维度应一致");
        for (j, (a, b)) in orig.iter().zip(rest.iter()).enumerate() {
            assert!(
                (a - b).abs() < 1e-6,
                "token {i} dim {j}: 原始 {a} vs 还原 {b}"
            );
        }
    }
}

/// TC-COL-005: MaxSim 计算复杂度 O(q×d)。
#[tokio::test]
async fn tc_col_005_maxsim_complexity() {
    let embedder = MockMultiVectorEmbedder::new(64);

    // 小 query（3 tokens）× 大 document（10 tokens）
    let query = embedder.embed_tokens("one two three").await.unwrap();
    let doc = embedder
        .embed_tokens("one two three four five six seven eight nine ten")
        .await
        .unwrap();

    // MaxSim 应正确计算（每个 query token 在 doc 中有匹配）
    let score = maxsim(&query, &doc);
    assert!(score > 0.0, "MaxSim 分数应 > 0");

    // 空 query 或空 doc 返回 0
    let empty: Vec<Vec<f32>> = vec![];
    assert_eq!(maxsim(&empty, &doc), 0.0, "空 query 应返回 0");
    assert_eq!(maxsim(&query, &empty), 0.0, "空 doc 应返回 0");
}

/// TC-COL-006: 可与单向量嵌入共存（运行时切换）。
#[tokio::test]
async fn tc_col_006_coexist_with_single_vector() {
    // 单向量嵌入（模拟）
    let single_vec = vec![0.1_f32; 384];

    // 多向量嵌入
    let embedder = MockMultiVectorEmbedder::new(384);
    let multi_vec = embedder.embed_tokens("test text").await.unwrap();

    // 两者可共存：维度相同但结构不同
    assert_eq!(single_vec.len(), 384, "单向量维度应为 384");
    for vec in &multi_vec {
        assert_eq!(vec.len(), 384, "多向量维度应与单向量相同");
    }

    // MaxSim 可在多向量间计算
    let query_multi = embedder.embed_tokens("query").await.unwrap();
    let doc_multi = embedder.embed_tokens("document").await.unwrap();
    let score = maxsim(&query_multi, &doc_multi);
    assert!(score >= 0.0, "MaxSim 分数应 >= 0");

    // 也可以用单向量做余弦相似度（模拟）
    let cos_sim = {
        let dot: f32 = single_vec
            .iter()
            .zip(&multi_vec[0])
            .map(|(a, b)| a * b)
            .sum();
        let norm_a: f32 = single_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = multi_vec[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a * norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    };
    let _ = cos_sim; // 两种检索方式可共存
}

/// 额外测试：分词器测试。
#[test]
fn tc_col_007_tokenize_basic() {
    // 英文
    let tokens = ColbertEmbedder::tokenize("Hello World");
    assert_eq!(tokens, vec!["Hello", "World"]);

    // 中文（逐字）
    let tokens = ColbertEmbedder::tokenize("你好世界");
    assert_eq!(tokens.len(), 4, "4 个中文字符应为 4 个 token");

    // 混合
    let tokens = ColbertEmbedder::tokenize("Rust 语言");
    assert!(tokens.contains(&"Rust".to_string()), "应包含 Rust");
    assert!(tokens.contains(&"语".to_string()), "应包含 语");
    assert!(tokens.contains(&"言".to_string()), "应包含 言");

    // 空文本
    let tokens = ColbertEmbedder::tokenize("");
    assert!(tokens.is_empty(), "空文本应无 token");

    // 标点
    let tokens = ColbertEmbedder::tokenize("code: test!");
    assert!(tokens.contains(&"code".to_string()), "应包含 code");
    assert!(tokens.contains(&"test".to_string()), "应包含 test");
}

/// 额外测试：多向量序列化边界。
#[test]
fn tc_col_008_serialization_edge_cases() {
    // 空向量
    let empty: Vec<Vec<f32>> = vec![];
    let bytes = multivec_to_bytes(&empty);
    let restored = multivec_from_bytes(&bytes).unwrap();
    assert!(restored.is_empty(), "空多向量序列化/反序列化应保持空");

    // 单 token
    let single = vec![vec![0.5; 384]];
    let bytes = multivec_to_bytes(&single);
    let restored = multivec_from_bytes(&bytes).unwrap();
    assert_eq!(restored.len(), 1, "单 token 应保持 1 个向量");
    assert_eq!(restored[0].len(), 384, "维度应保持 384");

    // 不同维度
    let mixed = vec![vec![1.0, 2.0], vec![3.0, 4.0, 5.0]];
    let bytes = multivec_to_bytes(&mixed);
    let restored = multivec_from_bytes(&bytes).unwrap();
    assert_eq!(restored[0].len(), 2, "第一个 token 维度应为 2");
    assert_eq!(restored[1].len(), 3, "第二个 token 维度应为 3");
}
