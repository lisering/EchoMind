#![allow(clippy::unwrap_used)]
//! QA 红灯测试：TC-VEC-001 本地向量化批量推理（REQ-VEC-002-AC-2）。

use std::path::PathBuf;

use echomind_core::Embedder;

use crate::local_embedder::LocalEmbedder;

/// 模型缓存目录：workspace target 下（已 gitignore），避免每次测试重复下载。
fn model_cache_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fastembed-cache")
}

/// TC-VEC-001 批量推理：数量匹配、维度 384、非全零（all-MiniLM-L6-v2 量化版）。
#[tokio::test(flavor = "multi_thread")]
async fn tc_vec_001_local_embed_batch() {
    let embedder = LocalEmbedder::new(model_cache_dir()).await.unwrap();

    let texts = vec![
        "灵犀知识库".to_string(),
        "local-first AI knowledge base".to_string(),
        "Rust 极速检索".to_string(),
    ];
    let vectors = embedder.embed_batch(&texts).await.unwrap();

    assert_eq!(vectors.len(), texts.len(), "向量数量必须与输入匹配");
    for (i, v) in vectors.iter().enumerate() {
        assert_eq!(v.len(), 384, "第 {i} 条向量维度必须为 384");
        assert!(v.iter().any(|x| x.abs() > 1e-6), "第 {i} 条向量不得全零");
    }

    let single = embedder.embed("单条文本验证").await.unwrap();
    assert_eq!(single.len(), 384, "单条 embed 维度同样为 384");
}
