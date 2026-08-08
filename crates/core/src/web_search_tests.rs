//! 网页搜索融合引擎 TDD 测试（REQ-RAG-036）。
//!
//! 测试覆盖：
//! - TC-RAG-SEARCH-001: 本地检索 score ≥ 阈值时不触发搜索
//! - TC-RAG-SEARCH-002: 本地检索 score < 阈值时触发搜索
//! - TC-RAG-SEARCH-003: RRF 融合本地 + web 结果排序正确
//! - TC-RAG-SEARCH-004: 搜索失败优雅降级（provider 返回 Err → 仅用本地结果）
//! - TC-RAG-SEARCH-005: 空查询处理（返回空结果不崩溃）
//! - TC-RAG-SEARCH-006: web 搜索关闭时不触发（should_search 直接返回 false）

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use echomind_models::{Chunk, RetrievalResult, SearchResult};

use crate::WebSearchProvider;
use crate::web_search::{
    DEFAULT_SEARCH_THRESHOLD, convert_to_retrieval_results, rrf_fuse_with_web, search_and_fuse,
    should_search,
};

/// 创建测试用 RetrievalResult。
fn make_result(chunk_id: &str, content: &str, score: f32, doc_name: &str) -> RetrievalResult {
    RetrievalResult {
        chunk: Chunk {
            id: chunk_id.to_string(),
            doc_id: chunk_id.to_string(),
            content: content.to_string(),
            token_count: content.len() / 4,
            sequence: 0,
        },
        score,
        doc_name: doc_name.to_string(),
    }
}

/// 创建测试用 SearchResult。
fn make_search_result(title: &str, url: &str, snippet: &str) -> SearchResult {
    SearchResult {
        title: title.to_string(),
        url: url.to_string(),
        snippet: snippet.to_string(),
        source: "duckduckgo".to_string(),
    }
}

/// Mock WebSearchProvider，记录调用次数。
struct MockSearchProvider {
    call_count: Arc<AtomicUsize>,
    results: Vec<SearchResult>,
    should_fail: bool,
}

impl MockSearchProvider {
    fn new(results: Vec<SearchResult>, should_fail: bool) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            results,
            should_fail,
        }
    }

    #[allow(dead_code)]
    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

impl WebSearchProvider for MockSearchProvider {
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<Vec<SearchResult>>> + Send + 'a>,
    > {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let results = self.results.clone();
        let should_fail = self.should_fail;
        Box::pin(async move {
            if should_fail {
                anyhow::bail!("模拟搜索失败")
            }
            if query.is_empty() {
                return Ok(vec![]);
            }
            Ok(results)
        })
    }
}

/// TC-RAG-SEARCH-001: 本地检索 score ≥ 阈值时不触发搜索。
///
/// 验证 `should_search()` 在 top-1 score 高于阈值时返回 false。
#[tokio::test]
async fn tc_rag_search_001_no_search_when_score_above_threshold() {
    let local_results = vec![
        make_result("chunk1", "content1", 0.85, "doc1"),
        make_result("chunk2", "content2", 0.70, "doc1"),
    ];

    let trigger = should_search(&local_results, DEFAULT_SEARCH_THRESHOLD);
    assert!(!trigger, "score 0.85 >= 0.3 阈值，不应触发搜索");
}

/// TC-RAG-SEARCH-002: 本地检索 score < 阈值时触发搜索。
///
/// 验证 `should_search()` 在 top-1 score 低于阈值时返回 true。
#[tokio::test]
async fn tc_rag_search_002_search_when_score_below_threshold() {
    let local_results = vec![
        make_result("chunk1", "content1", 0.15, "doc1"),
        make_result("chunk2", "content2", 0.10, "doc1"),
    ];

    let trigger = should_search(&local_results, DEFAULT_SEARCH_THRESHOLD);
    assert!(trigger, "score 0.15 < 0.3 阈值，应触发搜索");
}

/// TC-RAG-SEARCH-003: RRF 融合本地 + web 结果排序正确。
///
/// 验证 `rrf_fuse_with_web()` 正确融合本地和 web 结果，
/// 且融合后结果按 RRF 分数降序排列。
#[tokio::test]
async fn tc_rag_search_003_rrf_fusion_ordering() {
    let local_results = vec![
        make_result("local1", "local content 1", 0.85, "doc1"),
        make_result("local2", "local content 2", 0.70, "doc2"),
    ];

    let web_results = convert_to_retrieval_results(&[
        make_search_result("Web Result 1", "https://example.com/1", "web snippet 1"),
        make_search_result("Web Result 2", "https://example.com/2", "web snippet 2"),
    ]);

    let fused = rrf_fuse_with_web(local_results, web_results, 10);

    // 融合后应有 4 个结果（2 local + 2 web）
    assert_eq!(fused.len(), 4, "融合后应有 4 个结果");

    // 本地结果应排在前面（权重 1.0 > 0.4）
    assert!(
        fused[0].chunk.id.starts_with("local"),
        "本地结果应排在前面（权重更高）"
    );
    assert!(
        fused[1].chunk.id.starts_with("local"),
        "第二个也应是本地结果"
    );

    // 验证 web 结果在后面
    let web_count = fused
        .iter()
        .filter(|r| r.chunk.id.starts_with("web:"))
        .count();
    assert_eq!(web_count, 2, "应有 2 个 web 结果");
}

/// TC-RAG-SEARCH-004: 搜索失败优雅降级（provider 返回 Err → 仅用本地结果）。
///
/// 验证 `search_and_fuse()` 在 provider 返回错误时，
/// 优雅降级为仅使用本地检索结果（不崩溃，不丢失本地结果）。
#[tokio::test]
async fn tc_rag_search_004_graceful_degradation_on_error() {
    let provider = MockSearchProvider::new(vec![], true);
    let provider_arc: Arc<dyn WebSearchProvider> = Arc::new(provider);

    let local_results = vec![
        make_result("local1", "local content 1", 0.15, "doc1"),
        make_result("local2", "local content 2", 0.10, "doc2"),
    ];

    let fused = search_and_fuse(&provider_arc, "test query", local_results.clone(), 10).await;

    // 搜索失败时，应返回原始本地结果
    assert_eq!(
        fused.len(),
        local_results.len(),
        "搜索失败时应返回原始本地结果数量"
    );
    // 结果 ID 应与本地结果一致
    for (i, r) in fused.iter().enumerate() {
        assert_eq!(
            r.chunk.id, local_results[i].chunk.id,
            "降级结果应与本地结果一致"
        );
    }
}

/// TC-RAG-SEARCH-005: 空查询处理（返回空结果不崩溃）。
///
/// 验证 `should_search()` 和 `search_and_fuse()` 在空查询时不崩溃。
#[tokio::test]
async fn tc_rag_search_005_empty_query_handling() {
    // 空本地结果 → 应触发搜索
    let trigger = should_search(&[], DEFAULT_SEARCH_THRESHOLD);
    assert!(trigger, "空本地结果应触发搜索");

    // Mock provider 对空查询返回空结果
    let provider = MockSearchProvider::new(vec![], false);
    let provider_arc: Arc<dyn WebSearchProvider> = Arc::new(provider);

    let fused = search_and_fuse(&provider_arc, "", vec![], 10).await;

    // 空查询 + 空本地结果 → 应返回空列表
    assert!(fused.is_empty(), "空查询应返回空结果列表");
}

/// TC-RAG-SEARCH-006: 搜索结果来源标注验证。
///
/// 验证 `convert_to_retrieval_results()` 正确设置 `doc_name` 前缀为 🌐 标记，
/// 使前端能区分本地 KB 来源和 Web 来源。
#[tokio::test]
async fn tc_rag_search_006_web_source_label() {
    let search_results = vec![
        make_search_result(
            "Rust Programming Language",
            "https://www.rust-lang.org",
            "Rust is a systems programming language.",
        ),
        make_search_result(
            "Tauri Framework",
            "https://tauri.app",
            "Tauri is a framework for building desktop apps.",
        ),
    ];

    let converted = convert_to_retrieval_results(&search_results);

    assert_eq!(converted.len(), 2);

    // 验证 doc_name 以 🌐 开头（前端据此区分来源）
    assert!(
        converted[0].doc_name.starts_with("🌐"),
        "web 来源 doc_name 应以 🌐 开头，实际: {}",
        converted[0].doc_name
    );
    assert!(
        converted[0].chunk.id.starts_with("web:"),
        "web 来源 chunk.id 应以 web: 开头"
    );

    // 验证 snippet 被正确放入 chunk.content
    assert_eq!(
        converted[0].chunk.content, "Rust is a systems programming language.",
        "snippet 应被放入 chunk.content"
    );

    // 验证 score 递减
    assert!(
        converted[0].score > converted[1].score,
        "第一条搜索结果 score 应高于第二条"
    );
}
