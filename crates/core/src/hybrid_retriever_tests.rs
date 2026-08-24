#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-RRF-* RRF 融合算法（REQ-RAG-010）。

use echomind_models::{Chunk, RetrievalResult};

use crate::hybrid_retriever::rrf_fuse;

/// RRF 常量（必须与 `rrf_fuse` 中的常量保持一致，用于分数验证）。
const RRF_K: f32 = 60.0;
const KEYWORD_WEIGHT: f32 = 0.8;
const ENTITY_WEIGHT: f32 = 0.6;

/// 创建测试用 `RetrievalResult`。
fn make_result(chunk_id: &str, content: &str, score: f32) -> RetrievalResult {
    let mut chunk = Chunk::new("doc1".to_string(), content.to_string(), content.len(), 0);
    chunk.id = chunk_id.to_string();
    RetrievalResult {
        chunk,
        score,
        doc_name: "test.md".to_string(),
    }
}

/// 创建指定 doc_name 的 `RetrievalResult`。
fn make_result_with_doc(
    chunk_id: &str,
    content: &str,
    score: f32,
    doc_name: &str,
) -> RetrievalResult {
    let mut chunk = Chunk::new("doc1".to_string(), content.to_string(), content.len(), 0);
    chunk.id = chunk_id.to_string();
    RetrievalResult {
        chunk,
        score,
        doc_name: doc_name.to_string(),
    }
}

// ───────────────────────── 边界条件 ─────────────────────────

/// TC-RRF-001：两个空输入返回空结果。
#[test]
fn tc_rrf_001_both_empty() {
    let result = rrf_fuse(vec![], vec![], 5);
    assert!(result.is_empty(), "两个空输入应返回空结果");
}

/// TC-RRF-002：空关键词结果时仅返回向量结果。
#[test]
fn tc_rrf_002_empty_keyword_keeps_vector_only() {
    let vector_results = vec![
        make_result("c1", "content1", 0.9),
        make_result("c2", "content2", 0.8),
    ];
    let result = rrf_fuse(vector_results, vec![], 5);
    assert_eq!(result.len(), 2, "空关键词结果时应仅返回向量结果");
    assert_eq!(result[0].chunk.id, "c1", "rank 0 的向量结果应排第一");
    assert_eq!(result[1].chunk.id, "c2", "rank 1 的向量结果应排第二");
}

/// TC-RRF-003：空向量结果时仅返回关键词结果。
#[test]
fn tc_rrf_003_empty_vector_keeps_keyword_only() {
    let keyword_results = vec![
        make_result("c1", "content1", 0.8),
        make_result("c2", "content2", 0.7),
    ];
    let result = rrf_fuse(vec![], keyword_results, 5);
    assert_eq!(result.len(), 2, "空向量结果时应仅返回关键词结果");
    assert_eq!(result[0].chunk.id, "c1", "rank 0 的关键词结果应排第一");
}

/// TC-RRF-004：top_k=0 返回空结果。
#[test]
fn tc_rrf_004_top_k_zero() {
    let vector_results = vec![make_result("c1", "content1", 0.9)];
    let result = rrf_fuse(vector_results, vec![], 0);
    assert!(result.is_empty(), "top_k=0 应返回空结果");
}

// ───────────────────────── RRF 排序验证 ─────────────────────────

/// TC-RRF-005：重叠结果（在两个列表中都出现）应排在非重叠结果前面。
#[test]
fn tc_rrf_005_overlapping_ranks_higher() {
    let vector_results = vec![
        make_result("overlap", "shared", 0.5),
        make_result("v_only", "vector_only", 0.9),
    ];
    let keyword_results = vec![make_result("overlap", "shared", 0.4)];

    let result = rrf_fuse(vector_results, keyword_results, 5);

    assert_eq!(result.len(), 2, "应去重为 2 个结果");
    assert_eq!(
        result[0].chunk.id, "overlap",
        "重叠结果 RRF 分数更高应排第一（即使原始 score 较低）"
    );
}

/// TC-RRF-006：相同 chunk ID 的结果应去重。
#[test]
fn tc_rrf_006_dedup_by_chunk_id() {
    let vector_results = vec![make_result("shared_id", "from_vector", 0.9)];
    let keyword_results = vec![make_result("shared_id", "from_graph", 0.8)];

    let result = rrf_fuse(vector_results, keyword_results, 5);

    assert_eq!(result.len(), 1, "相同 chunk ID 应去重");
}

/// TC-RRF-007：top_k 限制返回数量。
#[test]
fn tc_rrf_007_respects_top_k() {
    let vector_results = vec![
        make_result("c1", "content1", 0.9),
        make_result("c2", "content2", 0.8),
        make_result("c3", "content3", 0.7),
    ];
    let keyword_results = vec![
        make_result("c4", "content4", 0.6),
        make_result("c5", "content5", 0.5),
    ];

    let result = rrf_fuse(vector_results, keyword_results, 3);
    assert_eq!(result.len(), 3, "top_k=3 应返回恰好 3 个结果");
}

/// TC-RRF-008：rank 影响排序（rank 越靠后，RRF 分数越低）。
#[test]
fn tc_rrf_008_rank_affects_ordering() {
    let vector_results = vec![
        make_result("c1", "first", 0.5),
        make_result("c2", "second", 0.5),
        make_result("c3", "third", 0.5),
    ];
    let result = rrf_fuse(vector_results, vec![], 5);

    assert_eq!(result[0].chunk.id, "c1", "rank 0 应排第一");
    assert_eq!(result[1].chunk.id, "c2", "rank 1 应排第二");
    assert_eq!(result[2].chunk.id, "c3", "rank 2 应排第三");
}

/// TC-RRF-009：向量通道权重高于关键词通道（向量 rank 0 应排在关键词 rank 0 前面）。
#[test]
fn tc_rrf_009_vector_weight_higher_than_keyword() {
    let vector_results = vec![make_result("v1", "vector_content", 0.5)];
    let keyword_results = vec![make_result("g1", "graph_content", 0.5)];

    let result = rrf_fuse(vector_results, keyword_results, 2);

    // 向量 rank 0 RRF = 1/(61) ≈ 0.0164
    // 关键词 rank 0 RRF = 0.8/(61) ≈ 0.0131
    // 向量应排前面
    assert_eq!(
        result[0].chunk.id, "v1",
        "向量 rank 0 的 RRF 分数应高于关键词 rank 0，应排第一"
    );
    assert_eq!(result[1].chunk.id, "g1");
}

/// TC-RRF-010：多个重叠项验证分数叠加正确。
#[test]
fn tc_rrf_010_multiple_overlaps() {
    let vector_results = vec![
        make_result("a", "content_a", 0.0),
        make_result("b", "content_b", 0.0),
        make_result("c", "content_c", 0.0),
    ];
    // "a" 和 "c" 在关键词结果中也出现
    let keyword_results = vec![
        make_result("a", "content_a", 0.0),
        make_result("c", "content_c", 0.0),
    ];

    let result = rrf_fuse(vector_results, keyword_results, 10);
    assert_eq!(result.len(), 3, "去重后应有 3 个结果");

    // "a" 的分数 = 1/(61) + 0.8/(61) = 1.8/61
    // "b" 的分数 = 1/(62)
    // "c" 的分数 = 1/(63) + 0.8/(62)
    let score_a = 1.0 / (RRF_K + 1.0) + KEYWORD_WEIGHT / (RRF_K + 1.0);
    let score_b = 1.0 / (RRF_K + 2.0);
    let score_c = 1.0 / (RRF_K + 3.0) + KEYWORD_WEIGHT / (RRF_K + 2.0);

    assert!(score_a > score_c, "a 的分数应高于 c");
    assert!(score_c > score_b, "c 的分数应高于 b（关键词加成）");

    assert_eq!(result[0].chunk.id, "a", "a 应排第一");
    assert_eq!(result[1].chunk.id, "c", "c 应排第二");
    assert_eq!(result[2].chunk.id, "b", "b 应排第三");
}

/// TC-RRF-011：融合后的结果保留正确的字段。
#[test]
fn tc_rrf_011_preserves_result_fields() {
    let vector_results = vec![make_result_with_doc("c1", "hello world", 0.95, "readme.md")];
    let result = rrf_fuse(vector_results, vec![], 5);

    assert_eq!(result[0].chunk.content, "hello world", "内容应保留");
    assert_eq!(result[0].doc_name, "readme.md", "文档名应保留");
}

/// TC-RRF-012：无重叠时保留所有结果。
#[test]
fn tc_rrf_012_non_overlapping_preserves_all() {
    let vector_results = vec![make_result("c1", "v1", 0.9), make_result("c2", "v2", 0.8)];
    let keyword_results = vec![make_result("c3", "g1", 0.7), make_result("c4", "g2", 0.6)];

    let result = rrf_fuse(vector_results, keyword_results, 10);
    assert_eq!(result.len(), 4, "无重叠时应保留所有 4 个结果");

    let ids: Vec<&str> = result.iter().map(|r| r.chunk.id.as_str()).collect();
    assert!(ids.contains(&"c1"));
    assert!(ids.contains(&"c2"));
    assert!(ids.contains(&"c3"));
    assert!(ids.contains(&"c4"));
}

/// TC-RRF-013：top_k 超过总数时返回所有结果。
#[test]
fn tc_rrf_013_top_k_exceeds_total() {
    let vector_results = vec![make_result("c1", "v1", 0.9)];
    let keyword_results = vec![make_result("c2", "g1", 0.8)];
    let result = rrf_fuse(vector_results, keyword_results, 100);
    assert_eq!(result.len(), 2, "top_k 超过总数时应返回所有结果");
}

/// TC-RRF-014：全部重叠时正确去重。
#[test]
fn tc_rrf_014_all_overlap() {
    let vector_results = vec![
        make_result("c1", "content1", 0.0),
        make_result("c2", "content2", 0.0),
    ];
    let keyword_results = vec![
        make_result("c1", "content1", 0.0),
        make_result("c2", "content2", 0.0),
    ];

    let result = rrf_fuse(vector_results, keyword_results, 5);
    assert_eq!(result.len(), 2, "全部重叠应去重为 2 个");
    assert_eq!(result[0].chunk.id, "c1", "c1 总分更高应排第一");
    assert_eq!(result[1].chunk.id, "c2");
}

// ───────────────────────── Cross-Encoder 重排序测试（REQ-RAG-020）─────────────────────────

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::NoReranker;

/// Mock Cross-Encoder Reranker：反转候选顺序 + 统一调用次数。
///
/// 模拟 Cross-Encoder 的行为：对 query-document 对打分后重排。
/// 这里用「反转顺序 + 分数递减赋值」模拟 reranker 的排序效果，
/// 并通过 `call_count` 统计 rerank 被调用的次数。
struct MockReranker {
    /// rerank 被调用的次数
    call_count: Arc<AtomicUsize>,
}

impl MockReranker {
    fn new() -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        let r = Self {
            call_count: Arc::clone(&counter),
        };
        (r, counter)
    }
}

impl Reranker for MockReranker {
    fn rerank<'a>(
        &'a self,
        _query: &'a str,
        candidates: &'a [RetrievalResult],
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<RetrievalResult>>> + Send + 'a>,
    > {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            // 反转顺序：最低分变最高分，模拟 reranker 重新排序
            let mut reranked: Vec<RetrievalResult> = candidates.iter().rev().cloned().collect();
            // 用递减分数赋值（模拟 Cross-Encoder 打分）
            let score = 1.0_f32;
            for (i, r) in reranked.iter_mut().enumerate() {
                r.score = score - (i as f32) * 0.01;
            }
            Ok(reranked)
        })
    }
}

/// TC-RERANK-001：NoReranker 原样返回候选列表（不重排不截断）。
#[tokio::test]
async fn tc_rerank_001_no_reranker_passthrough() {
    let candidates = vec![
        make_hit("c1", "content1", 0.9),
        make_hit("c2", "content2", 0.8),
        make_hit("c3", "content3", 0.7),
    ];
    let reranker = NoReranker;
    let result = reranker.rerank("query", &candidates).await.unwrap();
    assert_eq!(result.len(), 3, "NoReranker 应返回全部候选");
    assert_eq!(result[0].chunk.id, "c1", "顺序不变");
    assert_eq!(result[1].chunk.id, "c2");
    assert_eq!(result[2].chunk.id, "c3");
}

/// TC-RERANK-002：MockReranker 重排候选顺序并重新打分。
#[tokio::test]
async fn tc_rerank_002_mock_reranker_reorders() {
    let candidates = vec![
        make_hit("a", "content_a", 0.9),
        make_hit("b", "content_b", 0.8),
        make_hit("c", "content_c", 0.7),
    ];
    let (reranker, call_count) = MockReranker::new();
    let result = reranker.rerank("query", &candidates).await.unwrap();
    assert_eq!(call_count.load(Ordering::SeqCst), 1, "rerank 应被调用一次");
    assert_eq!(result.len(), 3, "应返回全部候选");
    // MockReranker 反转顺序
    assert_eq!(result[0].chunk.id, "c", "反转后 c 应排第一");
    assert_eq!(result[1].chunk.id, "b");
    assert_eq!(result[2].chunk.id, "a");
    // 分数应按递减赋值
    assert!(result[0].score > result[1].score, "分数应递减");
    assert!(result[1].score > result[2].score);
}

/// TC-RERANK-003：HybridRetriever 注入 reranker 后改变结果顺序（REQ-RAG-020-AC-1）。
///
/// AC-1：注入 reranker 后，检索结果按 Cross-Encoder 分数重排，而非原始 RRF 顺序。
#[tokio::test]
async fn tc_rerank_003_hybrid_with_reranker_changes_order() {
    // 向量检索返回 c1 > c2 > c3（按 score 降序）
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("c1", "content1", 0.9),
            make_hit("c2", "content2", 0.8),
            make_hit("c3", "content3", 0.7),
        ],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut retriever = HybridRetriever::new(MockEmbedder, storage);
    let (reranker, call_count) = MockReranker::new();
    retriever.set_reranker(Some(Arc::new(reranker)));

    let results = retriever.retrieve("query", 3).await.unwrap();

    // MockReranker 反转顺序，c3 应排第一
    assert_eq!(
        results[0].chunk.id, "c3",
        "reranker 重排后 c3 应排第一（反转）"
    );
    assert!(call_count.load(Ordering::SeqCst) > 0, "reranker 应被调用");
}

/// TC-RERANK-004：HybridRetriever 注入 reranker 后 top_k 被正确截断（REQ-RAG-020-AC-2）。
///
/// AC-2：reranker 返回全部候选后，仅取 top_k 注入 LLM prompt。
#[tokio::test]
async fn tc_rerank_004_top_k_respected_after_rerank() {
    // 5 个候选，top_k=2 → 仅取 reranker 排序后的前 2 个
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("c1", "content1", 0.9),
            make_hit("c2", "content2", 0.85),
            make_hit("c3", "content3", 0.8),
            make_hit("c4", "content4", 0.75),
            make_hit("c5", "content5", 0.7),
        ],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut retriever = HybridRetriever::new(MockEmbedder, storage);
    let (reranker, _) = MockReranker::new();
    retriever.set_reranker(Some(Arc::new(reranker)));

    let results = retriever.retrieve("query", 2).await.unwrap();
    // top_k=2 → 取 reranker 排序后的前 2 个
    assert_eq!(
        results.len(),
        2,
        "top_k=2 应返回 2 个结果（不含 Chunk Expansion）"
    );
    // MockReranker 反转后，c5 和 c4 排前
    assert_eq!(results[0].chunk.id, "c5", "反转后 c5 排第一");
    assert_eq!(results[1].chunk.id, "c4", "反转后 c4 排第二");
}

/// TC-RERANK-005：未注入 reranker 时行为与之前完全一致（向后兼容，REQ-RAG-020-AC-3）。
#[tokio::test]
async fn tc_rerank_005_no_reranker_backward_compat() {
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("c1", "content1", 0.9),
            make_hit("c2", "content2", 0.8),
        ],
        keyword_hits: vec![make_hit("c3", "content3", 1.0)],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    // 不注入 reranker
    let retriever = HybridRetriever::new(MockEmbedder, storage);
    let results = retriever.retrieve("query", 5).await.unwrap();
    // 行为应与之前一致：3 个去重结果
    assert_eq!(results.len(), 3, "未注入 reranker 时行为不变");
}

/// TC-RERANK-006：注入 reranker 但关闭混合检索时仍能正常工作（REQ-RAG-020-AC-4）。
#[tokio::test]
async fn tc_rerank_006_reranker_with_hybrid_disabled() {
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("c1", "content1", 0.9),
            make_hit("c2", "content2", 0.8),
            make_hit("c3", "content3", 0.7),
        ],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut retriever = HybridRetriever::new(MockEmbedder, storage);
    retriever.set_hybrid_enabled(false);
    let (reranker, call_count) = MockReranker::new();
    retriever.set_reranker(Some(Arc::new(reranker)));

    let results = retriever.retrieve("query", 3).await.unwrap();
    // hybrid 关闭 + reranker 启用 → 向量检索 → reranker 重排
    assert!(
        call_count.load(Ordering::SeqCst) > 0,
        "hybrid 关闭时 reranker 仍应被调用"
    );
    // MockReranker 反转顺序
    assert_eq!(results[0].chunk.id, "c3", "反转后 c3 应排第一");
}

/// TC-RERANK-007：空知识库时 reranker 不被调用（REQ-RAG-020-AC-5）。
#[tokio::test]
async fn tc_rerank_007_empty_kb_reranker_not_called() {
    let storage = HybridMockStorage::default();
    let mut retriever = HybridRetriever::new(MockEmbedder, storage);
    let (reranker, call_count) = MockReranker::new();
    retriever.set_reranker(Some(Arc::new(reranker)));

    let results = retriever.retrieve("query", 5).await.unwrap();
    assert!(results.is_empty(), "空知识库应返回空结果");
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "空知识库时 reranker 不应被调用"
    );
}

/// TC-RERANK-008：reranker 分数替换原始 RRF 分数（REQ-RAG-020-AC-6）。
#[tokio::test]
async fn tc_rerank_008_reranker_score_replaces_original() {
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("c1", "content1", 0.5)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut retriever = HybridRetriever::new(MockEmbedder, storage);
    let (reranker, _) = MockReranker::new();
    retriever.set_reranker(Some(Arc::new(reranker)));

    let results = retriever.retrieve("query", 1).await.unwrap();
    // MockReranker 将分数设为 1.0（第一个）
    assert!(
        (results[0].score - 1.0).abs() < 0.001,
        "reranker 分数应替换原始 RRF 分数"
    );
}

/// TC-RERANK-009：set_reranker(None) 可在运行时关闭重排序。
#[tokio::test]
async fn tc_rerank_009_disable_reranker_at_runtime() {
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("c1", "content1", 0.9),
            make_hit("c2", "content2", 0.8),
        ],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut retriever = HybridRetriever::new(MockEmbedder, storage);
    let (reranker, call_count) = MockReranker::new();
    // 启用 → 关闭
    retriever.set_reranker(Some(Arc::new(reranker)));
    retriever.set_reranker(None);

    let results = retriever.retrieve("query", 5).await.unwrap();
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "set_reranker(None) 后 reranker 不应被调用"
    );
    // 顺序应保持原始 RRF 顺序（c1 在前）
    assert_eq!(results[0].chunk.id, "c1", "关闭 reranker 后顺序不变");
}

/// 固定向量 Embedder（检索内容不影响本测试断言）。
struct MockEmbedder;

// ───────────────────────── HybridRetriever 集成测试 ─────────────────────────

use anyhow::Result;
use echomind_models::{ChatMessage, Conversation, DocStatus, Document};

use crate::hybrid_retriever::HybridRetriever;
use crate::{Embedder, Reranker, Retriever, Storage};

impl Embedder for MockEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Ok(vec![1.0, 0.0, 0.0])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
    }
}

/// 混合检索 MockStorage：可分别控制 vector_search 和 keyword_search 的返回结果。
#[derive(Default)]
struct HybridMockStorage {
    /// vector_search 返回的结果
    vector_hits: Vec<RetrievalResult>,
    /// keyword_search 返回的结果
    keyword_hits: Vec<RetrievalResult>,
    /// entity_search 返回的结果
    entity_hits: Vec<RetrievalResult>,
    /// list_chunks 返回的 chunk（Chunk Expansion 用）
    all_chunks: Vec<Chunk>,
}

impl Storage for HybridMockStorage {
    async fn add_document(&self, _doc: &Document) -> Result<()> {
        Ok(())
    }
    async fn update_doc_status(&self, _doc_id: &str, _status: DocStatus) -> Result<()> {
        Ok(())
    }
    async fn add_chunk(&self, _chunk: &Chunk) -> Result<()> {
        Ok(())
    }
    async fn add_embedding(&self, _chunk_id: &str, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }
    async fn vector_search(
        &self,
        _query_embedding: &[f32],
        _top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        Ok(self.vector_hits.clone())
    }
    async fn find_document_by_hash(&self, _hash: &str) -> Result<Option<Document>> {
        Ok(None)
    }
    async fn count_documents(&self) -> Result<usize> {
        Ok(0)
    }
    async fn count_chunks(&self) -> Result<usize> {
        Ok(0)
    }
    async fn cleanup_zombies(&self) -> Result<usize> {
        Ok(0)
    }
    async fn set_setting(&self, _key: &str, _value: &str) -> Result<()> {
        Ok(())
    }
    async fn get_setting(&self, _key: &str) -> Result<Option<String>> {
        Ok(None)
    }
    async fn create_conversation(&self, _conversation: &Conversation) -> Result<()> {
        Ok(())
    }
    async fn list_conversations(&self, _workspace_id: &str) -> Result<Vec<Conversation>> {
        Ok(vec![])
    }
    async fn delete_conversation(&self, _id: &str) -> Result<()> {
        Ok(())
    }
    async fn update_conversation_title(&self, _id: &str, _title: &str) -> Result<()> {
        Ok(())
    }
    async fn add_message(&self, _conversation_id: &str, _message: &ChatMessage) -> Result<()> {
        Ok(())
    }
    async fn list_messages(&self, _conversation_id: &str) -> Result<Vec<ChatMessage>> {
        Ok(vec![])
    }
    async fn list_documents(&self) -> Result<Vec<Document>> {
        Ok(vec![])
    }
    async fn delete_document(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn list_chunks(&self, _doc_id: &str) -> Result<Vec<Chunk>> {
        Ok(self.all_chunks.clone())
    }
    async fn delete_chunks_by_doc(&self, _doc_id: &str) -> Result<()> {
        Ok(())
    }
    async fn keyword_search(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(self.keyword_hits.clone())
    }

    async fn entity_search(
        &self,
        _query_entities: &[String],
        _top_k: usize,
    ) -> Result<Vec<RetrievalResult>> {
        Ok(self.entity_hits.clone())
    }
}

/// 创建带 ID 的 RetrievalResult（用于 HybridRetriever 测试）。
fn make_hit(chunk_id: &str, content: &str, score: f32) -> RetrievalResult {
    let mut chunk = Chunk::new("doc-1".to_string(), content.to_string(), 10, 0);
    chunk.id = chunk_id.to_string();
    RetrievalResult {
        chunk,
        score,
        doc_name: "test.md".to_string(),
    }
}

/// TC-HYBRID-001：混合检索融合向量 + 关键词结果（REQ-RAG-010-AC-1）。
///
/// AC-1：向量检索和关键词检索的结果通过 RRF 融合，重叠结果排名提升。
#[tokio::test]
async fn tc_hybrid_001_combines_vector_and_keyword() {
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("v1", "向量命中内容", 0.9),
            make_hit("shared", "重叠内容", 0.8),
        ],
        keyword_hits: vec![
            make_hit("k1", "关键词命中内容", 1.0),
            make_hit("shared", "重叠内容", 1.0),
        ],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let retriever = HybridRetriever::new(MockEmbedder, storage);

    let results = retriever.retrieve("任意查询", 5).await.unwrap();

    assert!(!results.is_empty(), "混合检索应返回结果");
    // "shared" 在两个通道都出现，RRF 分数最高，应排第一
    assert_eq!(results[0].chunk.id, "shared", "重叠结果应排第一");
    // 应包含所有 3 个唯一 chunk
    assert_eq!(results.len(), 3, "应返回 3 个去重后的结果");
}

/// TC-HYBRID-002：空知识库返回空结果（REQ-RAG-010-AC-2）。
#[tokio::test]
async fn tc_hybrid_002_empty_kb_returns_empty() {
    let storage = HybridMockStorage::default();
    let retriever = HybridRetriever::new(MockEmbedder, storage);

    let results = retriever.retrieve("任意查询", 5).await.unwrap();
    assert!(results.is_empty(), "空知识库应返回空结果");
}

/// TC-HYBRID-003：向量通道未命中但关键词命中时返回关键词结果（REQ-RAG-010-AC-3）。
///
/// AC-3：纯向量检索无法命中的精确匹配查询，关键词检索能补充召回。
#[tokio::test]
async fn tc_hybrid_003_keyword_only_hit() {
    let storage = HybridMockStorage {
        vector_hits: vec![],
        keyword_hits: vec![make_hit("k1", "精确匹配内容", 1.0)],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let retriever = HybridRetriever::new(MockEmbedder, storage);

    let results = retriever.retrieve("精确匹配", 5).await.unwrap();
    assert!(
        !results.is_empty(),
        "向量未命中但关键词命中时应返回关键词结果"
    );
    assert_eq!(results[0].chunk.id, "k1");
}

/// TC-HYBRID-004：低分向量结果被阈值过滤（REQ-RAG-010-AC-4）。
///
/// AC-4：score < 0.35 的向量结果被过滤，不参与 RRF 融合。
#[tokio::test]
async fn tc_hybrid_004_low_score_vector_filtered() {
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("high", "高分向量命中", 0.92),
            make_hit("low", "低分向量命中", 0.20), // 低于阈值 0.35
        ],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let retriever = HybridRetriever::new(MockEmbedder, storage);

    let results = retriever.retrieve("任意查询", 5).await.unwrap();
    assert_eq!(results.len(), 1, "低分向量结果应被过滤");
    assert_eq!(results[0].chunk.id, "high", "仅保留高分结果");
}

/// TC-HYBRID-005：Chunk Expansion 在混合检索中生效（REQ-RAG-010-AC-5）。
///
/// AC-5：混合检索结果同样进行 Chunk Expansion，扩展相邻 chunk。
#[tokio::test]
async fn tc_hybrid_005_chunk_expansion_works() {
    let doc_id = "expansion-doc".to_string();
    let chunk0 = Chunk::new(doc_id.clone(), "前一段落".to_string(), 10, 0);
    let mut chunk1 = Chunk::new(doc_id.clone(), "命中片段".to_string(), 10, 1);
    chunk1.id = "hit".to_string();
    let chunk2 = Chunk::new(doc_id, "后一段落".to_string(), 10, 2);

    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("hit", "命中片段", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![chunk0, chunk1, chunk2],
    };
    let retriever = HybridRetriever::new(MockEmbedder, storage);

    let results = retriever.retrieve("任意查询", 5).await.unwrap();
    assert_eq!(results.len(), 3, "Chunk Expansion 应扩展前后相邻 chunk");
    assert!(
        results.iter().any(|r| r.chunk.content.contains("前一段落")),
        "应包含前一个 chunk"
    );
    assert!(
        results.iter().any(|r| r.chunk.content.contains("后一段落")),
        "应包含后一个 chunk"
    );
}

// ───────────────────────── HyDE 查询改写测试（REQ-RAG-021）─────────────────────────

use crate::{NoRewriter, QueryRewriter};
use std::sync::Mutex;

/// TC-HYDE-001：NoRewriter 原样返回查询（REQ-RAG-021-AC-3）。
///
/// AC-3：未注入 rewriter 时行为与之前完全一致。
#[tokio::test]
async fn tc_hyde_001_no_rewriter_returns_original_query() {
    let rewriter = NoRewriter;
    let result = rewriter.rewrite("什么是 Rust 的所有权机制").await.unwrap();
    assert_eq!(
        result, "什么是 Rust 的所有权机制",
        "NoRewriter 应原样返回查询"
    );
}

/// 间谍 Embedder：记录最后一次 embed() 调用接收到的文本。
struct SpyEmbedder {
    last_text: Arc<Mutex<String>>,
}

impl SpyEmbedder {
    fn new() -> (Self, Arc<Mutex<String>>) {
        let text = Arc::new(Mutex::new(String::new()));
        let spy = Self {
            last_text: Arc::clone(&text),
        };
        (spy, text)
    }
}

impl Embedder for SpyEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        *self.last_text.lock().unwrap() = text.to_string();
        Ok(vec![1.0, 0.0, 0.0])
    }
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if let Some(last) = texts.last() {
            *self.last_text.lock().unwrap() = last.clone();
        }
        Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
    }
}

/// 固定返回改写文本的 Mock Rewriter。
struct MockRewriter {
    rewritten_text: String,
    call_count: Arc<AtomicUsize>,
}

impl MockRewriter {
    fn new(text: &str) -> (Self, Arc<AtomicUsize>) {
        let counter = Arc::new(AtomicUsize::new(0));
        let r = Self {
            rewritten_text: text.to_string(),
            call_count: Arc::clone(&counter),
        };
        (r, counter)
    }
}

impl QueryRewriter for MockRewriter {
    fn rewrite<'a>(
        &'a self,
        _query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let text = self.rewritten_text.clone();
        Box::pin(async move { Ok(text) })
    }
}

/// 始终失败的 Mock Rewriter（测试优雅降级）。
struct FailingRewriter;

impl QueryRewriter for FailingRewriter {
    fn rewrite<'a>(
        &'a self,
        _query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        Box::pin(async move { Err(anyhow::anyhow!("模拟 LLM 调用失败")) })
    }
}

/// 返回空字符串的 Mock Rewriter（测试空内容降级）。
struct EmptyRewriter;

impl QueryRewriter for EmptyRewriter {
    fn rewrite<'a>(
        &'a self,
        _query: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
    {
        Box::pin(async move { Ok(String::new()) })
    }
}

/// TC-HYDE-002：注入 rewriter 后向量检索使用改写文本（REQ-RAG-021-AC-1）。
///
/// AC-1：注入 rewriter 后，向量检索使用改写后的文本嵌入而非原始查询。
#[tokio::test]
async fn tc_hyde_002_rewriter_used_for_vector_search() {
    let (embedder, spy_text) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("v1", "Rust 所有权机制说明", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let (rewriter, call_count) =
        MockRewriter::new("Rust 的所有权机制通过借用检查器在编译期保证内存安全");
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(rewriter)));

    let _ = r.retrieve("什么是所有权", 5).await.unwrap();

    // rewriter 应被调用一次
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "rewriter 应被调用一次"
    );
    // embed 应接收到改写后的文本，而非原始查询
    let embedded = spy_text.lock().unwrap().clone();
    assert_eq!(
        embedded, "Rust 的所有权机制通过借用检查器在编译期保证内存安全",
        "向量检索应使用改写后的文本嵌入"
    );
    assert_ne!(embedded, "什么是所有权", "向量检索不应使用原始查询嵌入");
}

/// TC-HYDE-003：改写失败时优雅降级为原始查询（REQ-RAG-021-AC-2）。
///
/// AC-2：改写失败（LLM 错误）时不中断检索，使用原始查询嵌入。
#[tokio::test]
async fn tc_hyde_003_failing_rewriter_falls_back_to_original() {
    let (embedder, spy_text) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("v1", "命中内容", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(FailingRewriter)));

    let results = r.retrieve("原始查询", 5).await.unwrap();

    // 检索不应失败
    assert!(!results.is_empty(), "改写失败时检索应正常返回结果");
    // 应使用原始查询嵌入
    assert_eq!(
        spy_text.lock().unwrap().as_str(),
        "原始查询",
        "改写失败时应降级为原始查询"
    );
}

/// TC-HYDE-004：改写返回空字符串时降级为原始查询（REQ-RAG-021-AC-2 补充）。
///
/// HybridRetriever 在收到空改写结果时，应降级为原始查询嵌入，
/// 而非对空字符串进行嵌入（空字符串嵌入无语义意义）。
#[tokio::test]
async fn tc_hyde_004_empty_rewrite_falls_back_to_original() {
    let (embedder, spy_text) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("v1", "命中内容", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(EmptyRewriter)));

    let results = r.retrieve("原始查询", 5).await.unwrap();

    assert!(!results.is_empty(), "空改写时检索应正常返回结果");
    assert_eq!(
        spy_text.lock().unwrap().as_str(),
        "原始查询",
        "空改写结果应降级为原始查询嵌入"
    );
}

/// TC-HYDE-005：set_rewriter(None) 后 rewriter 不被调用（向后兼容，REQ-RAG-021-AC-3）。
#[tokio::test]
async fn tc_hyde_005_none_rewriter_not_called() {
    let (embedder, spy_text) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("v1", "命中内容", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let (rewriter, call_count) = MockRewriter::new("改写后的文本");
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(rewriter)));
    // 运行时关闭
    r.set_rewriter(None);

    let results = r.retrieve("原始查询", 5).await.unwrap();

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        0,
        "set_rewriter(None) 后 rewriter 不应被调用"
    );
    assert_eq!(
        spy_text.lock().unwrap().as_str(),
        "原始查询",
        "关闭 rewriter 后应使用原始查询嵌入"
    );
    assert!(!results.is_empty());
}

/// TC-HYDE-006：HyDE + 混合检索 + 关闭 reranker → 正常工作（REQ-RAG-021-AC-4）。
///
/// AC-4：注入 rewriter 但关闭混合检索时仍能正常工作。
#[tokio::test]
async fn tc_hyde_006_works_with_hybrid_disabled() {
    let (embedder, spy_text) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("v1", "命中内容", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let (rewriter, _) = MockRewriter::new("假设性答案文档");
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_hybrid_enabled(false);
    r.set_rewriter(Some(Arc::new(rewriter)));

    let results = r.retrieve("查询", 5).await.unwrap();

    assert!(!results.is_empty(), "HyDE + 关闭混合检索应正常返回结果");
    assert_eq!(
        spy_text.lock().unwrap().as_str(),
        "假设性答案文档",
        "关闭混合检索时 HyDE 仍应使用改写文本嵌入"
    );
}

/// TC-HYDE-007：HyDE + 混合检索 + reranker 三阶段协同工作。
///
/// 验证 HyDE 改写 → 向量检索 → RRF 融合 → reranker 精排 全链路不冲突。
#[tokio::test]
async fn tc_hyde_007_hyde_plus_reranker_chain() {
    let (embedder, _) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![
            make_hit("v1", "向量命中 A", 0.9),
            make_hit("v2", "向量命中 B", 0.8),
        ],
        keyword_hits: vec![make_hit("k1", "关键词命中", 1.0)],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let (rewriter, rewriter_calls) = MockRewriter::new("改写后的查询文本");
    let (reranker, reranker_calls) = MockReranker::new();
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(rewriter)));
    r.set_reranker(Some(Arc::new(reranker)));

    let results = r.retrieve("查询", 5).await.unwrap();

    assert!(!results.is_empty(), "三阶段链路应正常返回结果");
    assert_eq!(
        rewriter_calls.load(Ordering::SeqCst),
        1,
        "HyDE rewriter 应被调用一次"
    );
    assert_eq!(
        reranker_calls.load(Ordering::SeqCst),
        1,
        "Cross-Encoder reranker 应被调用一次"
    );
}

/// TC-HYDE-008：空知识库时 rewriter 仍被调用但不影响结果为空。
#[tokio::test]
async fn tc_hyde_008_empty_kb_with_rewriter() {
    let (embedder, _) = SpyEmbedder::new();
    let storage = HybridMockStorage::default();
    let (rewriter, call_count) = MockRewriter::new("改写文本");
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(rewriter)));

    let results = r.retrieve("查询", 5).await.unwrap();

    assert!(results.is_empty(), "空知识库应返回空结果");
    // rewriter 仍被调用（改写发生在检索之前）
    assert_eq!(
        call_count.load(Ordering::SeqCst),
        1,
        "空知识库时 rewriter 仍应被调用一次"
    );
}

/// TC-HYDE-009：多次检索 rewriter 每次都被调用（无缓存）。
#[tokio::test]
async fn tc_hyde_009_rewriter_called_each_time() {
    let (embedder, _) = SpyEmbedder::new();
    let storage = HybridMockStorage {
        vector_hits: vec![make_hit("v1", "命中", 0.9)],
        keyword_hits: vec![],
        entity_hits: vec![],
        all_chunks: vec![],
    };
    let (rewriter, call_count) = MockRewriter::new("改写文本");
    let mut r = HybridRetriever::new(embedder, storage);
    r.set_rewriter(Some(Arc::new(rewriter)));

    for _ in 0..3 {
        let _ = r.retrieve("查询", 5).await.unwrap();
    }

    assert_eq!(
        call_count.load(Ordering::SeqCst),
        3,
        "3 次检索应调用 rewriter 3 次"
    );
}

// ───────────────────────── 三路 RRF 融合测试（REQ-PERF-006）─────────────────────────

use crate::hybrid_retriever::rrf_fuse_three_way;

/// TC-3RRF-001：三路融合结果包含所有通道命中。
///
/// 向量、关键词、实体三路各返回不同的 chunk，融合后应包含全部唯一 chunk。
#[test]
fn tc_3rrf_001_includes_all_channels() {
    let vector_results = vec![make_result("v1", "向量命中", 0.9)];
    let keyword_results = vec![make_result("k1", "关键词命中", 0.8)];
    let entity_results = vec![make_result("e1", "实体命中", 0.7)];

    let result = rrf_fuse_three_way(vector_results, keyword_results, entity_results, 10);

    assert_eq!(result.len(), 3, "三路各不重叠时应返回 3 个结果");
    let ids: Vec<&str> = result.iter().map(|r| r.chunk.id.as_str()).collect();
    assert!(ids.contains(&"v1"), "应包含向量通道命中");
    assert!(ids.contains(&"k1"), "应包含关键词通道命中");
    assert!(ids.contains(&"e1"), "应包含实体通道命中");
}

/// TC-3RRF-002：实体匹配权重 0.6（低于 vector 1.0, keyword 0.8）。
///
/// 三路各返回一个不同的 chunk（rank 0），验证 RRF 分数排序：
/// - 向量 rank 0: 1.0 / 61 ≈ 0.0164
/// - 关键词 rank 0: 0.8 / 61 ≈ 0.0131
/// - 实体 rank 0: 0.6 / 61 ≈ 0.0098
///
/// 排序应为：向量 > 关键词 > 实体。
#[test]
fn tc_3rrf_002_entity_weight_06() {
    let vector_results = vec![make_result("v1", "向量内容", 0.5)];
    let keyword_results = vec![make_result("k1", "关键词内容", 0.5)];
    let entity_results = vec![make_result("e1", "实体内容", 0.5)];

    let result = rrf_fuse_three_way(vector_results, keyword_results, entity_results, 3);

    assert_eq!(result.len(), 3, "应返回 3 个结果");
    // 验证排序：向量 > 关键词 > 实体（权重递减）
    assert_eq!(result[0].chunk.id, "v1", "向量权重 1.0 应排第一");
    assert_eq!(result[1].chunk.id, "k1", "关键词权重 0.8 应排第二");
    assert_eq!(result[2].chunk.id, "e1", "实体权重 0.6 应排第三");

    // 验证 RRF 分数计算
    let score_v = 1.0 / (RRF_K + 1.0);
    let score_k = KEYWORD_WEIGHT / (RRF_K + 1.0);
    let score_e = ENTITY_WEIGHT / (RRF_K + 1.0);
    assert!(score_v > score_k, "向量分数应高于关键词");
    assert!(score_k > score_e, "关键词分数应高于实体");
}

/// TC-3RRF-003：实体通道单独命中时仍返回结果。
///
/// 向量和关键词都未命中，仅实体通道命中时，融合结果应包含实体命中。
#[test]
fn tc_3rrf_003_entity_only_hit() {
    let entity_results = vec![
        make_result("e1", "实体命中 A", 0.7),
        make_result("e2", "实体命中 B", 0.6),
    ];

    let result = rrf_fuse_three_way(vec![], vec![], entity_results, 5);

    assert_eq!(result.len(), 2, "仅实体命中时应返回 2 个结果");
    assert_eq!(result[0].chunk.id, "e1", "实体 rank 0 应排第一");
    assert_eq!(result[1].chunk.id, "e2");
}

/// TC-3RRF-004：三路同时命中的结果排名最高。
///
/// 向量和关键词和实体都返回相同 chunk "shared"（三路重叠），
/// 加上各路独有的 chunk。三路重叠的 "shared" 应排第一。
#[test]
fn tc_3rrf_004_triple_overlap_ranks_highest() {
    let vector_results = vec![
        make_result("shared", "三路重叠", 0.5),
        make_result("v_only", "仅向量", 0.9),
    ];
    let keyword_results = vec![
        make_result("shared", "三路重叠", 0.4),
        make_result("k_only", "仅关键词", 0.8),
    ];
    let entity_results = vec![
        make_result("shared", "三路重叠", 0.3),
        make_result("e_only", "仅实体", 0.7),
    ];

    let result = rrf_fuse_three_way(vector_results, keyword_results, entity_results, 10);

    assert_eq!(result.len(), 4, "应返回 4 个去重结果");
    // "shared" 三路重叠，RRF 分数最高
    assert_eq!(
        result[0].chunk.id, "shared",
        "三路重叠的 chunk 应排第一（RRF 分数最高）"
    );

    // 验证分数：shared = 1/61 + 0.8/61 + 0.6/61 = 2.4/61
    // v_only = 1/62
    // k_only = 0.8/62
    // e_only = 0.6/62
    let score_shared =
        1.0 / (RRF_K + 1.0) + KEYWORD_WEIGHT / (RRF_K + 1.0) + ENTITY_WEIGHT / (RRF_K + 1.0);
    let score_v_only = 1.0 / (RRF_K + 2.0);
    let score_k_only = KEYWORD_WEIGHT / (RRF_K + 2.0);
    let score_e_only = ENTITY_WEIGHT / (RRF_K + 2.0);

    assert!(score_shared > score_v_only, "shared 分数应高于 v_only");
    assert!(score_shared > score_k_only, "shared 分数应高于 k_only");
    assert!(score_shared > score_e_only, "shared 分数应高于 e_only");
    assert!(score_v_only > score_k_only, "v_only 应高于 k_only");
    assert!(score_k_only > score_e_only, "k_only 应高于 e_only");
}

// ================================================================
// P1-1 过检索倍数测试（2026-08-17）
// ================================================================

/// TC-ORETRIEVE-001：过检索倍数 5 使 RRF 融合候选池更大。
///
/// 验证倍数 5 比倍数 3 能让更多候选进入 RRF 融合，
/// 从而提升最终 top-k 中包含真正相关结果的概率。
#[test]
fn tc_oretrieve_001_larger_pool_better_rrf() {
    // 构造场景：向量检索 top-25 中有一个 rank-25 的相关结果
    // 在倍数 3（top-15）时会被遗漏，倍数 5（top-25）时进入 RRF 融合
    let mut vector_results = Vec::new();
    for i in 0..25 {
        let score = 0.9 - (i as f32 * 0.01); // 递减分数
        vector_results.push(make_result(&format!("v{i}"), &format!("content{i}"), score));
    }

    // 关键词检索也返回 25 条，其中 rank-20 与向量 rank-25 是同一 chunk
    let mut keyword_results = Vec::new();
    for i in 0..25 {
        let score = 0.8 - (i as f32 * 0.01);
        keyword_results.push(make_result(&format!("k{i}"), &format!("kw{i}"), score));
    }
    // 在 rank-20 位置插入与向量 rank-25 相同的 chunk
    keyword_results[20] = make_result("v24", "shared content", 0.6);

    // RRF 融合：倍数 5（25 条）vs 倍数 3（15 条）
    let fused_5 = rrf_fuse(vector_results.clone(), keyword_results.clone(), 5);

    // 倍数 3：仅取前 15 条
    let vector_15: Vec<_> = vector_results.into_iter().take(15).collect();
    let keyword_15: Vec<_> = keyword_results.into_iter().take(15).collect();
    let fused_3 = rrf_fuse(vector_15, keyword_15, 5);

    // 倍数 5 的结果中应包含 v24（从 keyword rank-20 进入），倍数 3 不包含
    let ids_5: Vec<&str> = fused_5.iter().map(|r| r.chunk.id.as_str()).collect();
    let ids_3: Vec<&str> = fused_3.iter().map(|r| r.chunk.id.as_str()).collect();

    assert!(
        ids_5.contains(&"v24"),
        "倍数 5 应包含 v24（从 keyword rank-20 进入 RRF）"
    );
    assert!(
        !ids_3.contains(&"v24"),
        "倍数 3 不应包含 v24（keyword rank-20 未进入 top-15）"
    );
}

/// TC-ORETRIEVE-002：过检索倍数不影响 RRF 融合排序顺序。
#[test]
fn tc_oretrieve_002_rrf_order_preserved() {
    let vector_results = vec![
        make_result("v1", "content1", 0.9),
        make_result("v2", "content2", 0.8),
        make_result("v3", "content3", 0.7),
    ];
    let keyword_results = vec![make_result("v2", "kw2", 1.0), make_result("v1", "kw1", 0.9)];

    let fused = rrf_fuse(vector_results, keyword_results, 3);

    // v1 和 v2 都在两个通道中出现，应该排在前面
    assert_eq!(fused[0].chunk.id, "v1", "v1 在两通道中排名高，应排第一");
    assert_eq!(fused[1].chunk.id, "v2", "v2 在两通道中排名高，应排第二");
    assert_eq!(fused[2].chunk.id, "v3", "v3 仅在向量通道，应排第三");
}

/// TC-ORETRIEVE-003：过检索倍数 5 在 top_k=1 时提供 5 条候选给 RRF。
#[test]
fn tc_oretrieve_003_top_k_one_with_five_candidates() {
    // 验证 top_k=1 时，过检索倍数 5 → 各通道 5 条候选
    // 这确保即使只请求 1 条结果，也有足够的候选池给 RRF 融合
    // v0 在两通道中都出现（向量 rank-0 + 关键词 rank-0），RRF 分数最高
    let vector_results: Vec<_> = (0..5)
        .map(|i| make_result(&format!("v{i}"), &format!("c{i}"), 0.9 - i as f32 * 0.1))
        .collect();
    // 关键词结果中 v0 排第一（与向量结果共享 chunk ID，两通道 rank-0）
    let keyword_results = vec![
        make_result("v0", "kw for v0", 1.0), // 关键词 rank-0
        make_result("v1", "kw for v1", 0.9), // 关键词 rank-1
    ];

    let fused = rrf_fuse(vector_results, keyword_results, 1);
    assert_eq!(fused.len(), 1, "top_k=1 应返回 1 条结果");
    // v0 在向量 rank-0 + 关键词 rank-0 → RRF 分数最高
    assert_eq!(
        fused[0].chunk.id, "v0",
        "top_k=1 时应返回 RRF 分数最高的 v0"
    );
}

/// TC-ORETRIEVE-004：空结果时 RRF 融合返回空。
#[test]
fn tc_oretrieve_004_empty_results_empty_fusion() {
    let fused = rrf_fuse(vec![], vec![], 5);
    assert!(fused.is_empty(), "空输入应返回空结果");
}

/// TC-ORETRIEVE-005：过检索倍数 5 允许 rank-25 结果通过 RRF 融合提升排名。
#[test]
fn tc_oretrieve_005_rank_25_promoted_by_rrf() {
    // 构造：向量 rank-25 的 chunk 在关键词检索中 rank-1
    // RRF 融合后应提升到 top-5 以内
    let mut vector_results = Vec::new();
    for i in 0..25 {
        vector_results.push(make_result(
            &format!("v{i}"),
            &format!("c{i}"),
            0.9 - i as f32 * 0.01,
        ));
    }

    let keyword_results = vec![
        make_result("v24", "kw for v24", 1.0), // 关键词 rank-1
    ];

    let fused = rrf_fuse(vector_results, keyword_results, 5);

    // v24 在向量中 rank-25 (分数低) 但关键词 rank-1 (分数高)
    // RRF 融合后应进入 top-5
    let ids: Vec<&str> = fused.iter().map(|r| r.chunk.id.as_str()).collect();
    assert!(
        ids.contains(&"v24"),
        "v24 通过关键词 rank-1 应进入 top-5 RRF 融合结果: {ids:?}"
    );
}
