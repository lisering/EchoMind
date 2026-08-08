//! 知识图谱图遍历检索器（REQ-RAG-027）：沿实体关系图边扩展到关联 chunk。
//!
//! ## 架构
//!
//! ```text
//! query → EntityExtractor::extract() → query_entities
//!       → Storage::get_relations_for_entity() (1-hop)
//!       → Storage::get_relations_for_entity() (2-hop, BFS)
//!       → relation filtering (可选)
//!       → confidence × distance_decay 加权排序
//!       → RetrievalResult 列表
//! ```
//!
//! ## 多阶段检索管线
//!
//! 图扩展作为 RRF 融合的**第四路检索通道**，与 Vector / BM25 / Entity 三路融合：
//!
//! ```text
//! Vector (1.0) + BM25 (0.8) + Entity (0.6) + Graph (0.5) → RRF Fuse
//! ```
//!
//! ## 距离衰减
//!
//! - 1 跳（直接关系）：score = confidence × 1.0
//! - 2 跳（间接关系）：score = confidence × 0.5
//!
//! ## 关键约束
//!
//! - 图遍历最大深度 2 跳（防止无限扩散）
//! - 已访问实体去重（防止环路）
//! - `rag.graph_retriever_enabled` 默认 false，用户主动开启

use std::collections::{HashMap, HashSet};

use echomind_models::RetrievalResult;

use crate::{Storage, entity_extractor::EntityExtractor};

/// 图扩展通道权重（低于实体 0.6，因为图扩展是间接信号）。
/// 在四路 RRF 融合中：Vector=1.0, BM25=0.8, Entity=0.6, Graph=0.5。
const GRAPH_WEIGHT: f32 = 0.5;

/// 默认图遍历最大深度（2 跳：entity → related entity → chunk）。
const DEFAULT_MAX_DEPTH: usize = 2;

/// 距离衰减因子：每跳分数乘以此因子。
/// 1 跳 confidence × 1.0，2 跳 confidence × 0.5。
const DISTANCE_DECAY: f32 = 0.5;

/// 四路 Reciprocal Rank Fusion (RRF) 融合算法（REQ-RAG-027 知识图谱检索）。
///
/// 将向量检索、关键词检索、实体匹配和图扩展的结果按排名融合。
/// 四路同时命中的结果获得最高 RRF 分数，排名最高。
///
/// # 参数
/// - `vector_results`: 向量检索结果（按相似度降序）
/// - `keyword_results`: 关键词检索结果（按 BM25 降序）
/// - `entity_results`: 实体匹配结果（按命中实体数降序）
/// - `graph_results`: 图扩展结果（按置信度加权降序）
/// - `top_k`: 返回的结果数量上限
///
/// # 算法
///
/// 对于每个结果，计算 RRF 分数：
/// ```text
/// score = Σ weight_i / (rrf_k + rank_i + 1)
/// ```
/// 其中 `rrf_k = 60`，`weight_i` 是各通道权重
/// （向量=1.0，关键词=0.8，实体=0.6，图=0.5）。
pub fn rrf_fuse_four_way(
    vector_results: Vec<RetrievalResult>,
    keyword_results: Vec<RetrievalResult>,
    entity_results: Vec<RetrievalResult>,
    graph_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult> {
    if top_k == 0 {
        return Vec::new();
    }

    const RRF_K: f32 = 60.0;
    const KEYWORD_WEIGHT: f32 = 0.8;
    const ENTITY_WEIGHT: f32 = 0.6;

    // 使用 chunk ID 作为唯一标识，HashMap 存储累积 RRF 分数与对应结果
    let mut scores: HashMap<String, (f32, RetrievalResult)> = HashMap::new();

    // 向量检索结果（权重 1.0）
    for (rank, result) in vector_results.iter().enumerate() {
        let score = 1.0 / (RRF_K + rank as f32 + 1.0);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert_with(|| (score, result.clone()));
    }

    // 关键词检索结果（权重 0.8）
    for (rank, result) in keyword_results.iter().enumerate() {
        let score = KEYWORD_WEIGHT / (RRF_K + rank as f32 + 1.0);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert_with(|| (score, result.clone()));
    }

    // 实体匹配结果（权重 0.6）
    for (rank, result) in entity_results.iter().enumerate() {
        let score = ENTITY_WEIGHT / (RRF_K + rank as f32 + 1.0);
        scores
            .entry(result.chunk.id.clone())
            .and_modify(|(s, _)| *s += score)
            .or_insert_with(|| (score, result.clone()));
    }

    // 图扩展结果（权重 0.5，REQ-RAG-027）
    for (rank, result) in graph_results.iter().enumerate() {
        let score = GRAPH_WEIGHT / (RRF_K + rank as f32 + 1.0);
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

/// 知识图谱图遍历检索器（REQ-RAG-027）。
///
/// 沿实体关系图边扩展到关联 chunk，作为 RRF 融合的第四路检索通道。
///
/// ## 检索流程
///
/// 1. 从查询中抽取实体（`EntityExtractor::extract`）
/// 2. 对每个实体查询其参与的关系（`Storage::get_relations_for_entity`）
/// 3. BFS 遍历：对关系中的对端实体继续查询（最大 `max_depth` 跳）
/// 4. 收集关系指向的 chunk_id，通过 `get_chunk_by_id` 查询 chunk 内容
/// 5. 按置信度 × 距离衰减因子加权排序
///
/// ## 关系类型过滤
///
/// 可选设置 `relation_filter`，仅沿指定关系类型扩展。
/// 例如仅沿 `depends_on` 扩展，过滤掉 `related_to` 等弱关系。
///
/// ## 空图降级
///
/// 如果查询中无实体、或实体无关系、或关系指向的 chunk 不存在，
/// 返回空 Vec（不报错），使管线优雅降级为三路 RRF。
pub struct GraphRetriever<S: Storage> {
    storage: S,
    max_depth: usize,
    relation_filter: Option<Vec<String>>,
}

impl<S: Storage> GraphRetriever<S> {
    /// 创建图遍历检索器，默认最大深度 2 跳，无关系类型过滤。
    pub fn new(storage: S) -> Self {
        Self {
            storage,
            max_depth: DEFAULT_MAX_DEPTH,
            relation_filter: None,
        }
    }

    /// 创建图遍历检索器，自定义最大深度。
    ///
    /// # 参数
    /// - `max_depth`: 图遍历最大跳数（建议 ≤ 3，防止无限扩散）
    pub fn with_max_depth(storage: S, max_depth: usize) -> Self {
        Self {
            storage,
            max_depth: max_depth.max(1),
            relation_filter: None,
        }
    }

    /// 创建图遍历检索器，仅沿指定关系类型扩展。
    ///
    /// # 参数
    /// - `relation_types`: 允许扩展的关系类型列表（如 `["depends_on", "uses"]`）
    pub fn with_relation_filter(storage: S, relation_types: Vec<String>) -> Self {
        Self {
            storage,
            max_depth: DEFAULT_MAX_DEPTH,
            relation_filter: Some(relation_types),
        }
    }

    /// 设置关系类型过滤器（运行时切换）。
    pub fn set_relation_filter(&mut self, relation_types: Option<Vec<String>>) {
        self.relation_filter = relation_types;
    }

    /// 从查询文本中抽取实体，沿图边扩展到关联 chunk。
    ///
    /// # 参数
    /// - `query`: 用户查询文本
    /// - `top_k`: 返回结果数量上限
    ///
    /// # 返回
    /// 图扩展检索结果列表（按置信度加权降序）。
    /// 空查询或无关系时返回空 Vec（不报错）。
    pub async fn expand(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }

        // 1. 从查询中抽取实体
        let query_entities: Vec<String> = EntityExtractor::extract(query)
            .into_iter()
            .map(|e| e.text)
            .collect();

        if query_entities.is_empty() {
            return Ok(vec![]);
        }

        self.expand_with_entities(&query_entities, top_k).await
    }

    /// 使用预抽取的实体进行图扩展（避免重复实体抽取）。
    ///
    /// # 参数
    /// - `entities`: 从查询中抽取的实体文本列表
    /// - `top_k`: 返回结果数量上限
    ///
    /// # 返回
    /// 图扩展检索结果列表（按置信度加权降序）。
    pub async fn expand_with_entities(
        &self,
        entities: &[String],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 || entities.is_empty() {
            return Ok(vec![]);
        }

        // 2. BFS 遍历实体关系图
        // 记录 (chunk_id → 最高加权分数)
        let mut chunk_scores: HashMap<String, f32> = HashMap::new();
        // 已访问实体（防止环路）
        let mut visited: HashSet<String> = HashSet::new();

        // BFS 队列：(entity_text, current_depth)
        let mut queue: Vec<(String, usize)> = entities
            .iter()
            .filter(|e| !e.is_empty())
            .map(|e| (e.clone(), 0))
            .collect();

        while let Some((entity, depth)) = queue.pop() {
            // 超过最大深度，停止扩展
            if depth >= self.max_depth {
                continue;
            }

            // 已访问实体跳过（防止环路）
            if !visited.insert(entity.clone()) {
                continue;
            }

            // 查询该实体参与的所有关系
            let relations = match self.storage.get_relations_for_entity(&entity).await {
                Ok(rels) => rels,
                Err(e) => {
                    eprintln!("[GRAPH] 查询实体关系失败 ({entity}): {e:#}");
                    continue;
                }
            };

            for rel in relations {
                // 关系类型过滤
                if let Some(filter) = &self.relation_filter
                    && !filter.contains(&rel.relation_type)
                {
                    continue;
                }

                // 计算距离衰减后的加权分数
                // depth=0 → 1 跳（直接关系），衰减因子 1.0
                // depth=1 → 2 跳（间接关系），衰减因子 0.5
                let decay = DISTANCE_DECAY.powi(depth as i32);
                let weighted_score = rel.confidence * decay;

                // 收集关系指向的 chunk
                // chunk_id 是关系来源 chunk，包含该实体和关系
                chunk_scores
                    .entry(rel.chunk_id.clone())
                    .and_modify(|s| {
                        // 保留最高分
                        if weighted_score > *s {
                            *s = weighted_score;
                        }
                    })
                    .or_insert(weighted_score);

                // 将对端实体加入 BFS 队列
                // subject 是当前实体时，object 是对端；反之亦然
                let next_entity = if rel.subject == entity {
                    rel.object.clone()
                } else {
                    rel.subject.clone()
                };

                // 只有对端实体未被访问时才加入队列
                if !visited.contains(&next_entity) && !next_entity.is_empty() {
                    queue.push((next_entity, depth + 1));
                }
            }
        }

        if chunk_scores.is_empty() {
            return Ok(vec![]);
        }

        // 3. 按 score 降序排列，取 top_k 个 chunk_id
        let mut scored_chunks: Vec<(String, f32)> = chunk_scores.into_iter().collect();
        scored_chunks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let top_chunk_ids: Vec<(String, f32)> = scored_chunks.into_iter().take(top_k).collect();

        // 4. 通过 get_chunk_by_id 查询 chunk 内容并构建 RetrievalResult
        let mut results: Vec<RetrievalResult> = Vec::new();
        for (chunk_id, score) in top_chunk_ids {
            match self.storage.get_chunk_by_id(&chunk_id).await {
                Ok(Some(chunk)) => {
                    results.push(RetrievalResult {
                        chunk,
                        score,
                        doc_name: String::new(), // doc_name 由调用方在 RRF 融合后填充
                    });
                }
                Ok(None) => {
                    // chunk 不存在（可能已被删除），跳过
                    eprintln!("[GRAPH] chunk 不存在: {chunk_id}");
                }
                Err(e) => {
                    eprintln!("[GRAPH] 查询 chunk 失败 ({chunk_id}): {e:#}");
                }
            }
        }

        Ok(results)
    }
}
