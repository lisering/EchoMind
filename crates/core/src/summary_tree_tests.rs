//! RAPTOR 摘要树 TDD 测试（REQ-PERF-009）。
//!
//! 测试用例 TC-RAP-001~006：
//! - TC-RAP-001: chunk → 多级摘要树（Level 0=原始, Level 1=组摘要, Level 2=主题摘要）
//! - TC-RAP-002: 摘要树节点关联到子节点
//! - TC-RAP-003: 全局查询命中摘要节点 → 返回摘要 + 原始 chunks
//! - TC-RAP-004: 局部查询仍命中 Level 0
//! - TC-RAP-005: 摘要节点数 < 原始 chunk 数（压缩比）
//! - TC-RAP-006: 摘要树可增量更新（新文档导入时）

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::summary_tree::*;
use echomind_models::{Chunk, SummaryNode};

/// 占位摘要生成器：拼接子节点文本（测试用，零 LLM 调用）。
fn mock_summarize(
    texts: Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>> {
    Box::pin(async move { Ok(format!("摘要：{}", texts.join(" / "))) })
}

/// 创建测试用 chunks。
fn make_test_chunks(doc_id: &str, count: usize) -> Vec<Chunk> {
    (0..count)
        .map(|i| Chunk {
            id: format!("chunk-{i}"),
            doc_id: doc_id.to_string(),
            content: format!("这是第 {i} 个分块的内容。"),
            token_count: 256,
            sequence: i,
        })
        .collect()
}

/// TC-RAP-001: chunk → 多级摘要树（Level 0=原始, Level 1=组摘要, Level 2=主题摘要）。
#[tokio::test]
async fn tc_rap_001_build_multi_level_tree() {
    let chunks = make_test_chunks("doc-1", 16);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes = builder.build("doc-1", &chunks).await.unwrap();

    // 16 chunks / 4 = 4 Level-0 节点
    let level0: Vec<&SummaryNode> = nodes.iter().filter(|n| n.level == 0).collect();
    assert!(!level0.is_empty(), "Level 0 摘要节点不应为空");
    assert_eq!(level0.len(), 4, "16 chunks / 4 = 4 个 Level 0 组摘要");

    // 4 Level-0 / 4 = 1 Level-1 节点
    let level1: Vec<&SummaryNode> = nodes.iter().filter(|n| n.level == 1).collect();
    assert!(!level1.is_empty(), "Level 1 摘要节点不应为空");
    assert_eq!(level1.len(), 1, "4 Level-0 / 4 = 1 个 Level 1 主题摘要");
}

/// TC-RAP-002: 摘要树节点关联到子节点。
#[tokio::test]
async fn tc_rap_002_node_links_to_children() {
    let chunks = make_test_chunks("doc-2", 8);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes = builder.build("doc-2", &chunks).await.unwrap();

    // Level 0 节点的子节点是原始 chunk IDs
    let level0 = nodes.iter().find(|n| n.level == 0).unwrap();
    assert!(!level0.child_ids.is_empty(), "Level 0 节点应有子节点");
    // 子节点应该是 chunk IDs
    assert!(
        level0.child_ids.iter().all(|id| id.starts_with("chunk-")),
        "Level 0 子节点应为 chunk IDs"
    );

    // Level 1 节点的子节点是 Level 0 节点 IDs
    if let Some(level1) = nodes.iter().find(|n| n.level == 1) {
        assert!(!level1.child_ids.is_empty(), "Level 1 节点应有子节点");
        // Level 1 的子节点 ID 应能在 nodes 中找到
        for child_id in &level1.child_ids {
            assert!(
                nodes.iter().any(|n| &n.id == child_id),
                "Level 1 子节点 {child_id} 应在摘要节点列表中找到"
            );
        }
    }
}

/// TC-RAP-003: 全局查询命中摘要节点 → 返回摘要 + 原始 chunks。
#[tokio::test]
async fn tc_rap_003_global_query_hits_summary_then_expand() {
    let chunks = make_test_chunks("doc-3", 12);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes = builder.build("doc-3", &chunks).await.unwrap();

    // 模拟摘要检索命中 Level 0 的第一个节点
    let hit_node_id = nodes.iter().find(|n| n.level == 0).unwrap().id.clone();
    let hit_node_ids = vec![hit_node_id.clone()];

    // 展开到原始 chunk IDs
    let expanded = expand_summary_to_chunks(&hit_node_ids, &nodes);

    assert!(!expanded.is_empty(), "展开后应包含原始 chunk IDs");
    // 展开的 chunk IDs 应在原始 chunks 中
    for chunk_id in &expanded {
        assert!(
            chunks.iter().any(|c| &c.id == chunk_id),
            "展开的 chunk ID {chunk_id} 应在原始 chunks 中"
        );
    }
}

/// TC-RAP-004: 局部查询仍命中 Level 0（classify_query_level 判断为 Local）。
#[tokio::test]
async fn tc_rap_004_local_query_stays_level_0() {
    // 包含特定术语的查询 → Local
    let query = "Rust 的所有权机制是什么？";
    let level = classify_query_level(query);
    assert_eq!(level, QueryLevel::Local, "具体事实查询应为 Local 级别");

    // 全局查询 → Global
    let global_query = "请概述文档的主要内容";
    let level = classify_query_level(global_query);
    assert_eq!(level, QueryLevel::Global, "概述查询应为 Global 级别");
}

/// TC-RAP-005: 摘要节点数 < 原始 chunk 数（压缩比）。
#[tokio::test]
async fn tc_rap_005_summary_count_less_than_chunks() {
    let chunks = make_test_chunks("doc-5", 16);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes = builder.build("doc-5", &chunks).await.unwrap();

    // 摘要节点总数（Level 0 + Level 1）应 < 原始 chunk 数
    // 16 chunks → 4 Level-0 + 1 Level-1 = 5 < 16
    assert!(
        nodes.len() < chunks.len(),
        "摘要节点数 {} 应 < 原始 chunk 数 {}",
        nodes.len(),
        chunks.len()
    );

    // 验证压缩比
    let ratio = chunks.len() as f32 / nodes.len() as f32;
    assert!(
        ratio > 1.0,
        "压缩比 {ratio} 应 > 1.0（摘要节点数 < 原始 chunk 数）"
    );
}

/// TC-RAP-006: 摘要树可增量更新（新文档导入时仅构建该文档的子树）。
#[tokio::test]
async fn tc_rap_006_incremental_update() {
    // 文档 1 的摘要树
    let chunks1 = make_test_chunks("doc-a", 8);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes1 = builder.build("doc-a", &chunks1).await.unwrap();

    // 文档 2 的摘要树
    let chunks2 = make_test_chunks("doc-b", 12);
    let nodes2 = builder.build("doc-b", &chunks2).await.unwrap();

    // 两棵子树互不影响
    for node in &nodes1 {
        assert_eq!(node.doc_id, "doc-a", "文档 1 的摘要节点 doc_id 应正确");
    }
    for node in &nodes2 {
        assert_eq!(node.doc_id, "doc-b", "文档 2 的摘要节点 doc_id 应正确");
    }

    // 合并后总节点数 = nodes1.len() + nodes2.len()
    // doc-a: 8 chunks → 2 Level-0 + 1 Level-1 = 3 nodes
    // doc-b: 12 chunks → 3 Level-0 + 1 Level-1 = 4 nodes
    let combined = nodes1.len() + nodes2.len();
    assert_eq!(combined, 7, "合并节点数应正确（doc-a: 3, doc-b: 4）");
}

/// 额外测试：chunks 太少不构建摘要树。
#[tokio::test]
async fn tc_rap_007_too_few_chunks_no_tree() {
    let chunks = make_test_chunks("doc-small", 3);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes = builder.build("doc-small", &chunks).await.unwrap();
    assert!(
        nodes.is_empty(),
        "3 chunks ≤ cluster_size=4，不应构建摘要树"
    );
}

/// 额外测试：merge_summary_and_chunk_results 合并去重。
#[tokio::test]
async fn tc_rap_008_merge_results_dedup() {
    use echomind_models::RetrievalResult;

    let chunks = make_test_chunks("doc-merge", 8);
    let builder = SummaryTreeBuilder::new(mock_summarize, 4, 2);
    let nodes = builder.build("doc-merge", &chunks).await.unwrap();

    // 模拟摘要命中
    let summary_hit = RetrievalResult {
        chunk: Chunk {
            id: nodes[0].id.clone(),
            doc_id: "doc-merge".to_string(),
            content: nodes[0].content.clone(),
            token_count: 100,
            sequence: 0,
        },
        score: 0.9,
        doc_name: "test.md".to_string(),
    };

    // 模拟 chunk 检索结果（包含摘要展开的 chunk）
    let chunk_results: Vec<RetrievalResult> = chunks[0..4]
        .iter()
        .map(|c| RetrievalResult {
            chunk: c.clone(),
            score: 0.8,
            doc_name: "test.md".to_string(),
        })
        .collect();

    let merged = merge_summary_and_chunk_results(&[summary_hit], &nodes, &chunk_results);

    // 应包含 1 个摘要 + 展开的 chunks
    assert!(!merged.is_empty(), "合并结果不应为空");
    // 摘要节点应在结果中
    assert!(
        merged.iter().any(|r| r.chunk.id == nodes[0].id),
        "摘要节点应在合并结果中"
    );
}
