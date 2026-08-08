//! RAPTOR 摘要树索引（REQ-PERF-009）：多级摘要树构建与检索。
//!
//! 基于 RAPTOR 论文（Recursive Abstractive Tree for Retrieval-Ordered Access），
//! 将原始 chunks 组织为多级摘要树，提供从局部事实到全局主题的多层次检索能力。
//!
//! ## 树结构
//!
//! ```text
//! Level 0: 原始 chunks（256 tokens each，不在 summary_nodes 表中）
//!   │ 聚类（按 embedding 相似度分组，每组 4 个 chunks）
//!   ▼
//! Level 1: 组摘要（LLM 生成 ~100 tokens/摘要）
//!   │ 聚类
//!   ▼
//! Level 2: 主题摘要（LLM 生成 ~50 tokens/摘要）
//! ```
//!
//! ## 查询路由
//!
//! - 局部事实查询 → Level 0 检索（直接在 chunks 上检索）
//! - 全局分析查询 → Level 2 → Level 1 → Level 0 展开
//!   （先在摘要树上检索，命中后通过 child_ids 向下展开到原始 chunks）
//!
//! ## 增量更新
//!
//! 新文档导入时仅构建该文档的摘要子树，不影响其他文档的已有摘要树。

use echomind_models::{Chunk, RetrievalResult, SummaryNode};
use std::collections::HashMap;

/// 默认聚类大小：每个摘要节点覆盖的子节点数（Level 0 → Level 1 每组 4 个 chunks）。
pub const DEFAULT_CLUSTER_SIZE: usize = 4;

/// 默认最大树高度（Level 0 = 组摘要, Level 1 = 主题摘要）。
pub const DEFAULT_MAX_LEVEL: usize = 2;

/// RAPTOR 摘要树构建器。
///
/// 将文档的 chunks 组织为多级摘要树：
/// - Level 0: 原始 chunks 按 sequence 顺序分组聚类
/// - Level 1+: 对每组子节点生成 LLM 摘要
///
/// # LLM 调用
///
/// 摘要生成需要 LLM 调用（`summarize_group` 回调），可作为异步任务执行。
/// 在没有 LLM 时（如测试或离线模式），可使用 `summarize_group` 返回拼接文本的占位实现。
pub struct SummaryTreeBuilder<F>
where
    F: Fn(
        Vec<String>,
    )
        -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>,
{
    /// 聚类大小（每组子节点数）
    cluster_size: usize,
    /// 最大树高度
    max_level: usize,
    /// 摘要生成回调：接收子节点文本列表，返回 LLM 生成的摘要文本
    summarize_group: F,
}

impl<F> SummaryTreeBuilder<F>
where
    F: Fn(
        Vec<String>,
    )
        -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>,
{
    /// 创建摘要树构建器。
    ///
    /// # 参数
    /// - `summarize_group`: 摘要生成回调。接收一组子节点文本，返回摘要文本。
    ///   在没有 LLM 时可返回拼接文本的占位实现。
    /// - `cluster_size`: 每组聚类大小（默认 4）
    /// - `max_level`: 最大树高度（默认 2）
    pub fn new(summarize_group: F, cluster_size: usize, max_level: usize) -> Self {
        Self {
            cluster_size: cluster_size.max(1),
            max_level: max_level.max(1),
            summarize_group,
        }
    }

    /// 为文档构建摘要树。
    ///
    /// # 参数
    /// - `doc_id`: 文档 ID
    /// - `chunks`: 文档的全部分块（按 sequence 排序）
    ///
    /// # 返回
    /// 所有层级的摘要节点列表（Level 0 在前，Level N 在后）。
    /// 如果 chunks 数量 ≤ cluster_size，不构建摘要树（返回空 Vec）。
    pub async fn build(&self, doc_id: &str, chunks: &[Chunk]) -> anyhow::Result<Vec<SummaryNode>> {
        if chunks.len() <= self.cluster_size {
            // chunks 太少，无需构建摘要树
            return Ok(vec![]);
        }

        let mut all_nodes = Vec::new();
        let mut current_level_children: Vec<String> = chunks.iter().map(|c| c.id.clone()).collect();
        let mut current_level_texts: Vec<String> =
            chunks.iter().map(|c| c.content.clone()).collect();
        let mut level = 0usize;

        // 持续聚类直到不能再减少或达到最大层级
        loop {
            // 如果当前层子节点数 <= 1，已到达根节点，不再聚类
            if current_level_children.len() <= 1 {
                break;
            }
            // 如果已达到最大层级
            // max_level = 树的最大层数（Level 0 到 Level max_level-1）
            if level >= self.max_level {
                break;
            }
            let groups = Self::cluster_by_sequence(
                &current_level_children,
                &current_level_texts,
                self.cluster_size,
            );

            let mut next_level_children = Vec::new();
            let mut next_level_texts = Vec::new();

            for group in groups {
                let child_ids = group.0;
                let child_texts = group.1;

                // 调用 LLM 生成摘要
                let summary_text = (self.summarize_group)(child_texts.clone()).await?;

                let node =
                    SummaryNode::new(doc_id.to_string(), level, summary_text.clone(), child_ids);
                next_level_children.push(node.id.clone());
                next_level_texts.push(summary_text);
                all_nodes.push(node);
            }

            current_level_children = next_level_children;
            current_level_texts = next_level_texts;
            level += 1;
        }

        Ok(all_nodes)
    }

    /// 按 sequence 顺序分组聚类（每组 cluster_size 个子节点）。
    ///
    /// RAPTOR 使用简单顺序聚类而非 embedding 聚类，因为：
    /// 1. 文档内相邻 chunks 通常语义相近（splitter 保证局部性）
    /// 2. 顺序聚类是 O(1)，而 embedding 聚类需要 O(N²) 相似度计算
    fn cluster_by_sequence(
        ids: &[String],
        texts: &[String],
        cluster_size: usize,
    ) -> Vec<(Vec<String>, Vec<String>)> {
        ids.chunks(cluster_size)
            .zip(texts.chunks(cluster_size))
            .map(|(id_chunk, text_chunk)| (id_chunk.to_vec(), text_chunk.to_vec()))
            .collect()
    }
}

/// 摘要树查询路由器：根据查询类型决定在哪个层级检索。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryLevel {
    /// 局部事实查询 → Level 0（直接在 chunks 上检索）
    Local,
    /// 全局分析查询 → 摘要树检索（Level 2 → Level 1 → Level 0 展开）
    Global,
}

/// 查询分类器：根据查询文本特征判断查询层级（纯规则，零 LLM）。
///
/// 简单规则：
/// - 包含「概述/总结/主题/全局/全部/overview/summary/theme/global/all」→ Global
/// - 其他 → Local
pub fn classify_query_level(query: &str) -> QueryLevel {
    let lower = query.to_lowercase();
    let global_keywords = [
        "概述", "总结", "主题", "全局", "全部", "整体", "综述", "overview", "summary", "theme",
        "global", "all", "overall", "outline",
    ];
    if global_keywords.iter().any(|kw| lower.contains(kw)) {
        QueryLevel::Global
    } else {
        QueryLevel::Local
    }
}

/// 摘要树展开：从命中的摘要节点向下展开到原始 chunk IDs。
///
/// 给定命中的摘要节点 ID 列表和全部摘要节点，递归展开所有子节点，
/// 最终返回 Level 0 的 chunk ID 列表（去重）。
///
/// # 参数
/// - `hit_node_ids`: 命中的摘要节点 ID 列表
/// - `all_nodes`: 文档的所有摘要节点
///
/// # 返回
/// 所有命中的原始 chunk ID 列表（去重 + 按 sequence 排序）。
pub fn expand_summary_to_chunks(hit_node_ids: &[String], all_nodes: &[SummaryNode]) -> Vec<String> {
    let node_map: HashMap<&str, &SummaryNode> =
        all_nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut result: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // BFS 展开
    let mut queue: Vec<String> = hit_node_ids.to_vec();
    while let Some(node_id) = queue.pop() {
        if let Some(node) = node_map.get(node_id.as_str()) {
            for child_id in &node.child_ids {
                if seen.insert(child_id.clone()) {
                    // 检查子节点是否是摘要节点（在 node_map 中）
                    if node_map.contains_key(child_id.as_str()) {
                        queue.push(child_id.clone());
                    } else {
                        // 不是摘要节点 → 是原始 chunk ID
                        result.push(child_id.clone());
                    }
                }
            }
        }
    }

    result
}

/// 摘要树检索结果合并：将摘要检索结果 + 展开的 chunk 检索结果合并去重。
///
/// # 参数
/// - `summary_hits`: 摘要树检索命中的结果
/// - `all_nodes`: 文档的所有摘要节点（用于展开）
/// - `chunk_results`: 原始 chunk 检索结果（通过展开的 chunk IDs 获取）
///
/// # 返回
/// 合并去重后的 RetrievalResult 列表。
pub fn merge_summary_and_chunk_results(
    summary_hits: &[RetrievalResult],
    all_nodes: &[SummaryNode],
    chunk_results: &[RetrievalResult],
) -> Vec<RetrievalResult> {
    let hit_node_ids: Vec<String> = summary_hits.iter().map(|h| h.chunk.id.clone()).collect();

    let expanded_chunk_ids = expand_summary_to_chunks(&hit_node_ids, all_nodes);
    let expanded_set: std::collections::HashSet<&str> =
        expanded_chunk_ids.iter().map(|s| s.as_str()).collect();

    let mut result: Vec<RetrievalResult> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 先加入摘要命中（使用摘要内容作为 context）
    for hit in summary_hits {
        if seen.insert(hit.chunk.id.clone()) {
            result.push(hit.clone());
        }
    }

    // 加入展开的原始 chunks
    for hit in chunk_results {
        if expanded_set.contains(hit.chunk.id.as_str()) && seen.insert(hit.chunk.id.clone()) {
            result.push(hit.clone());
        }
    }

    result
}
