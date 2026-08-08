//! 网页搜索融合引擎（REQ-RAG-036）：阈值触发判断 + RRF 融合本地与 Web 结果。
//!
//! ## 架构
//!
//! ```text
//! 本地检索结果 → should_search() 阈值判断
//!   → true: WebSearchProvider.search(query) → SearchResult 列表
//!           → convert_to_retrieval_results() → 转换为 RetrievalResult
//!           → rrf_fuse_with_web() → RRF 融合本地 + Web 结果
//!   → false: 仅使用本地结果（不触发搜索）
//! ```
//!
//! ## RRF 融合权重
//!
//! Web 搜索结果权重 0.4（低于向量 1.0 / BM25 0.8 / Entity 0.6 / Graph 0.5），
//! 因为网页搜索结果不如本地知识库精确，仅作为补充 context。

use std::sync::Arc;

use echomind_models::{Chunk, RetrievalResult, SearchResult};

use crate::WebSearchProvider;

/// 默认阈值：本地检索 top-1 score 低于此值时触发网页搜索。
///
/// 0.3 是经验值——cosine similarity 0.3 以下通常意味着知识库中无强相关内容，
/// 适合补充互联网搜索结果。用户可在设置中调整。
pub const DEFAULT_SEARCH_THRESHOLD: f32 = 0.3;

/// Web 搜索结果在 RRF 融合中的权重（低于本地检索各通道）。
const WEB_WEIGHT: f32 = 0.4;

/// RRF 常量 k（与 hybrid_retriever 保持一致）。
const RRF_K: f32 = 60.0;

/// 判断是否应该触发网页搜索。
///
/// 当本地检索结果为空，或 top-1 score 低于阈值时返回 `true`。
///
/// # 参数
/// - `local_results`: 本地检索结果（按 score 降序）
/// - `threshold`: 触发搜索的分数阈值
///
/// # 返回
/// `true` 表示应该触发网页搜索，`false` 表示本地结果足够。
pub fn should_search(local_results: &[RetrievalResult], threshold: f32) -> bool {
    if local_results.is_empty() {
        return true;
    }
    // top-1 score（结果已按 score 降序排列，取第一个）
    let top_score = local_results[0].score;
    top_score < threshold
}

/// 将 `SearchResult` 列表转换为 `RetrievalResult` 列表。
///
/// 网页搜索结果没有 chunk_id / doc_id，使用 URL 作为唯一标识。
/// `chunk.content` 设为 `snippet`，`doc_name` 设为 `title`。
/// `score` 设为递减序列（按搜索结果顺序，越靠前越相关）。
///
/// # 参数
/// - `results`: 网页搜索结果列表
///
/// # 返回
/// 转换后的 `RetrievalResult` 列表，可直接用于 RRF 融合。
pub fn convert_to_retrieval_results(results: &[SearchResult]) -> Vec<RetrievalResult> {
    results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            // 使用 URL 作为 chunk_id 和 doc_id，确保 RRF 融合时能正确去重
            let chunk = Chunk {
                id: format!("web:{}", r.url),
                doc_id: format!("web:{}", r.url),
                content: r.snippet.clone(),
                sequence: i,
                token_count: r.snippet.chars().count() / 4,
            };
            // score 递减：第一条 1.0，后续依次降低
            let score = 1.0 / (1.0 + i as f32 * 0.1);
            RetrievalResult {
                chunk,
                score,
                doc_name: format!("🌐 {}", r.title),
            }
        })
        .collect()
}

/// 将本地检索结果与网页搜索结果通过 RRF 融合。
///
/// 本地结果权重 1.0（与向量检索一致），网页结果权重 0.4（低于所有本地通道）。
/// RRF 算法与 `hybrid_retriever::rrf_fuse` 一致，但仅融合两个列表。
///
/// # 参数
/// - `local_results`: 本地检索结果（按 score 降序）
/// - `web_results`: 网页搜索转换后的 RetrievalResult（按搜索顺序）
/// - `top_k`: 返回结果数量上限
///
/// # 返回
/// RRF 融合后的结果列表，按 RRF 分数降序排列，截取 top_k 条。
pub fn rrf_fuse_with_web(
    local_results: Vec<RetrievalResult>,
    web_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult> {
    if top_k == 0 {
        return Vec::new();
    }

    let mut scores: std::collections::HashMap<String, (f32, RetrievalResult)> =
        std::collections::HashMap::new();

    // 本地检索结果（权重 1.0）
    for (rank, result) in local_results.iter().enumerate() {
        let score = 1.0 / (RRF_K + rank as f32 + 1.0);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert_with(|| (score, result.clone()));
    }

    // 网页搜索结果（权重 0.4）
    for (rank, result) in web_results.iter().enumerate() {
        let score = WEB_WEIGHT / (RRF_K + rank as f32 + 1.0);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert_with(|| (score, result.clone()));
    }

    // 按 RRF 分数降序排序
    let mut fused: Vec<(f32, RetrievalResult)> = scores.into_values().collect();
    fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    fused.into_iter().map(|(_, r)| r).take(top_k).collect()
}

/// 执行网页搜索并将结果融合到本地检索结果中。
///
/// 这是 `chat_inner` 中调用的主入口函数，封装了完整的搜索→转换→融合流程。
///
/// # 参数
/// - `provider`: 网页搜索 provider（实现 `WebSearchProvider` trait）
/// - `query`: 搜索查询文本
/// - `local_results`: 本地检索结果
/// - `top_k`: 返回结果数量上限
///
/// # 返回
/// 融合后的结果列表。如果搜索失败，返回原始本地结果（优雅降级）。
pub async fn search_and_fuse(
    provider: &Arc<dyn WebSearchProvider>,
    query: &str,
    local_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult> {
    // 执行搜索（失败时优雅降级）
    let search_results = match provider.search(query).await {
        Ok(results) if !results.is_empty() => {
            eprintln!(
                "[WEB] 网页搜索返回 {} 条结果，融合到本地检索",
                results.len()
            );
            results
        }
        Ok(_) => {
            eprintln!("[WEB] 网页搜索无结果，仅使用本地检索");
            return local_results;
        }
        Err(e) => {
            eprintln!("[WEB] 网页搜索失败，降级为仅本地检索: {e:#}");
            return local_results;
        }
    };

    // 转换为 RetrievalResult 并 RRF 融合
    let web_results = convert_to_retrieval_results(&search_results);
    rrf_fuse_with_web(local_results, web_results, top_k)
}
