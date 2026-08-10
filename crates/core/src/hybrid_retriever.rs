//! 混合检索器（REQ-RAG-010）：向量检索 + 关键词检索 → RRF 融合。
//!
//! ## 架构
//!
//! ```text
//! query → Embedder.embed() → Storage.vector_search() (Top-N)
//!       → Storage.keyword_search() (Top-N, FTS5 BM25)
//!       → rrf_fuse() 融合排序
//!       → Chunk Expansion
//!       → Top-K 最终结果
//! ```
//!
//! ## RRF 算法
//!
//! Reciprocal Rank Fusion：将两个检索结果列表按排名融合，
//! 重叠的结果（在两个列表中都出现）获得更高分数。
//!
//! ```text
//! score = Σ 1 / (rrf_k + rank_i + 1)
//! ```
//!
//! 其中 `rrf_k = 60`（标准参数），`rank_i` 是该结果在第 i 个列表中的排名。

use std::sync::Arc;

use echomind_models::RetrievalResult;

use crate::mmr_diversifier::{MmrConfig, mmr_diversify};
use crate::retrieval_quality_gate::{QualityGateConfig, QualityVerdict, score_retrieval_quality};
use crate::{Embedder, QueryRewriter, Reranker, Retriever, Storage};

/// RRF 常量 k（标准参数 60，来自 Cormack et al. 2009 论文）。
const RRF_K: f32 = 60.0;

/// 关键词通道权重（略低于向量通道，因为向量检索通常更准确）。
const KEYWORD_WEIGHT: f32 = 0.8;

/// 实体匹配通道权重（REQ-PERF-006 实体链接增强）。
/// 低于向量 (1.0) 和关键词 (0.8)，因为实体匹配是精确匹配的补充信号。
const ENTITY_WEIGHT: f32 = 0.6;

/// Reciprocal Rank Fusion (RRF) 融合算法。
///
/// 将向量检索和关键词检索的结果按排名融合，重叠的结果获得更高分数。
///
/// # 参数
/// - `vector_results`: 向量检索结果（按相似度降序）
/// - `keyword_results`: 关键词检索结果（按 BM25 降序）
/// - `top_k`: 返回的结果数量上限
///
/// # 算法
///
/// 对于每个结果，计算 RRF 分数：
/// ```text
/// score = Σ weight_i / (rrf_k + rank_i + 1)
/// ```
/// 其中 `rrf_k = 60`，`rank_i` 是该结果在第 i 个列表中的排名，
/// `weight_i` 是该通道的权重（向量=1.0，关键词=0.8）。
pub fn rrf_fuse(
    vector_results: Vec<RetrievalResult>,
    keyword_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult> {
    rrf_fuse_three_way(vector_results, keyword_results, vec![], top_k)
}

/// 三路 Reciprocal Rank Fusion (RRF) 融合算法（REQ-PERF-006 实体链接增强）。
///
/// 将向量检索、关键词检索和实体匹配的结果按排名融合。
/// 三路同时命中的结果获得最高 RRF 分数，排名最高。
///
/// # 参数
/// - `vector_results`: 向量检索结果（按相似度降序）
/// - `keyword_results`: 关键词检索结果（按 BM25 降序）
/// - `entity_results`: 实体匹配结果（按命中实体数降序）
/// - `top_k`: 返回的结果数量上限
///
/// # 算法
///
/// 对于每个结果，计算 RRF 分数：
/// ```text
/// score = Σ weight_i / (rrf_k + rank_i + 1)
/// ```
/// 其中 `rrf_k = 60`，`weight_i` 是各通道权重
/// （向量=1.0，关键词=0.8，实体=0.6）。
pub fn rrf_fuse_three_way(
    vector_results: Vec<RetrievalResult>,
    keyword_results: Vec<RetrievalResult>,
    entity_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult> {
    if top_k == 0 {
        return Vec::new();
    }

    // 使用 chunk ID 作为唯一标识，HashMap 存储累积 RRF 分数与对应结果
    let mut scores: std::collections::HashMap<String, (f32, RetrievalResult)> =
        std::collections::HashMap::new();

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

    // 实体匹配结果（权重 0.6，REQ-PERF-006）
    for (rank, result) in entity_results.iter().enumerate() {
        let score = ENTITY_WEIGHT / (RRF_K + rank as f32 + 1.0);
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

/// 混合检索过检索倍数：向量 + 关键词各检索 top_k * OVER_RETRIEVE_FACTOR 条，
/// 给 RRF 融合更大的候选池以提升排序质量。
const OVER_RETRIEVE_FACTOR: usize = 3;

/// 重排序过检索倍数：启用 reranker 时，从 RRF 融合结果中取 top_k * RERANK_OVER_RETRIEVE_FACTOR
/// 条交给 Cross-Encoder 精排，给 reranker 更大的候选池。
/// 倍数 5：在 top_k=5 默认值下取 25 条候选，Cross-Encoder 推理 25 对 query-document 在
/// CPU 上约 200ms（bge-reranker-base 量化模型），延迟可接受。
const RERANK_OVER_RETRIEVE_FACTOR: usize = 5;

/// 混合检索适配器：向量检索 + 关键词检索 → RRF 融合 → [可选]重排序 → Chunk Expansion。
///
/// 弥补纯向量检索在精确匹配（代码片段、API 名称、专有名词）上的不足。
/// 向量检索擅长语义匹配，关键词检索擅长精确匹配，RRF 融合两者优势。
///
/// 可选注入 `Reranker`（REQ-RAG-020）：在 RRF 融合后、Chunk Expansion 前，
/// 对 top-N 候选进行 Cross-Encoder 精排，取 top-k 后再扩展相邻 chunk。
/// 重排序显著提升 top-k 精度（Anthropic 调研：检索失败率 ↓49%）。
///
/// 实现 `Retriever` 端口，可无缝替换 `VectorRetriever`。
pub struct HybridRetriever<E: Embedder, S: Storage> {
    embedder: E,
    storage: S,
    score_threshold: f32,
    /// 混合检索开关：false 时仅走向量检索（等价于 VectorRetriever），true 时启用关键词+RRF融合。
    hybrid_enabled: bool,
    /// 可选 Cross-Encoder 重排序器（REQ-RAG-020）：注入后 RRF 融合结果先经 reranker 精排再取 top-k。
    /// `None` 时跳过重排序，行为与未注入时完全一致。
    reranker: Option<Arc<dyn Reranker>>,
    /// 可选查询改写器（REQ-RAG-021 HyDE）：注入后向量检索使用改写后的文本嵌入。
    /// 关键词检索仍使用原始查询（精确匹配）。`None` 时跳过改写，行为与未注入时完全一致。
    rewriter: Option<Arc<dyn QueryRewriter>>,
    /// MMR 多样性重排配置（借鉴 OpenMontage corpus.py）。
    /// `None` 时禁用 MMR，行为与之前完全一致。启用后在 RRF/重排序后、Chunk Expansion 前执行。
    mmr_config: Option<MmrConfig>,
    /// 检索质量门控配置（借鉴 OpenMontage slideshow_risk.py）。
    /// 每次检索后评估质量并输出 tracing 日志，不影响检索结果（仅可观测性）。
    quality_gate_config: QualityGateConfig,
}

impl<E: Embedder, S: Storage> HybridRetriever<E, S> {
    /// 创建混合检索器，默认启用混合检索，不启用重排序和查询改写。
    pub fn new(embedder: E, storage: S) -> Self {
        Self {
            embedder,
            storage,
            score_threshold: crate::retriever::DEFAULT_SCORE_THRESHOLD,
            hybrid_enabled: true,
            reranker: None,
            rewriter: None,
            mmr_config: None,
            quality_gate_config: QualityGateConfig::default(),
        }
    }

    /// 创建混合检索器，自定义阈值。
    pub fn with_threshold(embedder: E, storage: S, score_threshold: f32) -> Self {
        Self {
            embedder,
            storage,
            score_threshold,
            hybrid_enabled: true,
            reranker: None,
            rewriter: None,
            mmr_config: None,
            quality_gate_config: QualityGateConfig::default(),
        }
    }

    /// 设置混合检索开关（运行时切换）。
    /// false 时仅走向量检索，跳过关键词检索与 RRF 融合。
    pub fn set_hybrid_enabled(&mut self, enabled: bool) {
        self.hybrid_enabled = enabled;
    }

    /// 注入 Cross-Encoder 重排序器（REQ-RAG-020）。
    /// 注入后，`retrieve()` 会在 RRF 融合后对候选进行 Cross-Encoder 精排。
    /// 传入 `None` 可在运行时关闭重排序。
    pub fn set_reranker(&mut self, reranker: Option<Arc<dyn Reranker>>) {
        self.reranker = reranker;
    }

    /// 注入查询改写器（REQ-RAG-021 HyDE）。
    /// 注入后，向量检索使用改写后的文本嵌入，关键词检索仍使用原始查询。
    /// 传入 `None` 可在运行时关闭查询改写。
    pub fn set_rewriter(&mut self, rewriter: Option<Arc<dyn QueryRewriter>>) {
        self.rewriter = rewriter;
    }

    /// 设置 MMR 多样性重排配置（借鉴 OpenMontage corpus.py）。
    /// 传入 `Some(config)` 启用 MMR，`None` 关闭。
    /// 启用后，检索结果在 RRF/重排序后、Chunk Expansion 前执行 MMR 多样性重排。
    pub fn set_mmr(&mut self, config: Option<MmrConfig>) {
        self.mmr_config = config;
    }

    /// Builder 方法：启用 MMR 多样性重排。
    #[must_use]
    pub fn with_mmr(mut self, config: MmrConfig) -> Self {
        self.mmr_config = Some(config);
        self
    }

    /// 设置检索质量门控配置（借鉴 OpenMontage slideshow_risk.py）。
    /// 每次检索后评估质量并输出 tracing 日志。
    pub fn set_quality_gate_config(&mut self, config: QualityGateConfig) {
        self.quality_gate_config = config;
    }

    /// Builder 方法：自定义质量门控配置。
    #[must_use]
    pub fn with_quality_gate(mut self, config: QualityGateConfig) -> Self {
        self.quality_gate_config = config;
        self
    }

    /// 对检索结果应用 MMR 多样性重排（如已启用）。
    /// 在 RRF 融合/重排序后、Chunk Expansion 前调用。
    fn apply_mmr(&self, results: Vec<RetrievalResult>, top_k: usize) -> Vec<RetrievalResult> {
        if let Some(ref config) = self.mmr_config {
            mmr_diversify(results, config, top_k)
        } else {
            results
        }
    }

    /// 评估检索质量并输出 tracing 日志（可观测性，不影响结果）。
    fn log_quality(&self, results: &[RetrievalResult], query: &str) {
        let report = score_retrieval_quality(results, &self.quality_gate_config);
        match report.verdict {
            QualityVerdict::Strong => {
                tracing::debug!(
                    target: "echomind::retriever::quality",
                    verdict = report.verdict.as_str(),
                    score = report.overall_score,
                    query = %query,
                    "检索质量评估: Strong"
                );
            }
            QualityVerdict::Acceptable => {
                tracing::info!(
                    target: "echomind::retriever::quality",
                    verdict = report.verdict.as_str(),
                    score = report.overall_score,
                    query = %query,
                    violations = ?report.violations,
                    "检索质量评估: Acceptable"
                );
            }
            QualityVerdict::Revise | QualityVerdict::Fail => {
                tracing::warn!(
                    target: "echomind::retriever::quality",
                    verdict = report.verdict.as_str(),
                    score = report.overall_score,
                    query = %query,
                    violations = ?report.violations,
                    suggestions = ?report.suggestions,
                    "检索质量评估: 需要改进"
                );
            }
        }
    }

    /// 检索候选结果（不含 Chunk Expansion），供重排序管线使用。
    ///
    /// 执行向量检索 + 关键词检索 + RRF 融合，返回融合后的候选列表。
    /// 当 `hybrid_enabled=false` 时仅返回向量检索结果。
    ///
    /// # 查询改写（REQ-RAG-021 HyDE）
    ///
    /// 当 `rewriter` 存在时，先调用 `rewriter.rewrite(query)` 生成假设性答案文档，
    /// 用该文档的嵌入进行向量检索。关键词检索仍使用原始查询（精确匹配优势）。
    /// 改写失败时优雅降级为原始查询（由 `QueryRewriter` 实现保证）。
    ///
    /// # 参数
    /// - `query`: 用户查询
    /// - `top_k`: 期望返回的结果数量上限（内部会过检索以提升融合质量）
    ///
    /// # 返回
    /// RRF 融合后的候选结果（已过阈值过滤），不含 Chunk Expansion 扩展的相邻 chunk。
    async fn retrieve_candidates(
        &self,
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }

        // 查询改写（REQ-RAG-021 HyDE）：改写后的文本仅用于向量检索
        let embedding_query = if let Some(rewriter) = &self.rewriter {
            let rewritten = rewriter.rewrite(query).await.unwrap_or_else(|e| {
                eprintln!("HyDE 改写失败，使用原始查询: {e:#}");
                query.to_string()
            });
            // 空改写结果降级为原始查询（如 LLM 返回空字符串）
            if rewritten.trim().is_empty() {
                eprintln!("HyDE 改写返回空内容，使用原始查询");
                query.to_string()
            } else {
                rewritten
            }
        } else {
            query.to_string()
        };

        // 1. 向量检索（语义匹配）——使用改写后的文本嵌入（如已启用 HyDE）
        let embedding = self.embedder.embed(&embedding_query).await?;
        self.retrieve_candidates_with_embedding(query, &embedding, top_k)
            .await
    }

    /// 使用预计算嵌入检索候选结果（性能优化：避免冗余 ONNX 推理）。
    ///
    /// 与 `retrieve_candidates` 的区别：跳过 `embedder.embed(query)` 调用，
    /// 直接使用调用方提供的查询嵌入进行向量检索。
    /// 关键词检索和实体检索仍使用原始查询文本（不依赖嵌入）。
    ///
    /// **注意**：此方法不处理 HyDE 查询改写。HyDE 改写由 `retrieve_candidates`
    /// 或 `retrieve()` 处理，改写后的嵌入传入此方法。直接调用此方法时，
    /// 提供的嵌入将原样用于向量检索。
    async fn retrieve_candidates_with_embedding(
        &self,
        query: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }

        // 直接使用预计算嵌入，不重新嵌入（包括 HyDE 场景——改写已在调用方处理）
        let embedding = query_embedding.to_vec();

        // **性能优化**：混合检索启用时直接用过检索倍数搜索，避免双重全表扫描。
        if !self.hybrid_enabled {
            let mut vector_hits = self.storage.vector_search(&embedding, top_k).await?;
            vector_hits.retain(|h| h.score >= self.score_threshold);
            return Ok(vector_hits);
        }

        let retrieval_k = top_k.saturating_mul(OVER_RETRIEVE_FACTOR);
        let mut vector_hits = self.storage.vector_search(&embedding, retrieval_k).await?;
        // 阈值过滤：低分向量结果直接丢弃
        vector_hits.retain(|h| h.score >= self.score_threshold);

        // 2. 关键词检索（精确匹配，FTS5 BM25 + Contextual BM25 REQ-PERF-005）
        let keyword_hits = self.storage.keyword_search(query, retrieval_k).await?;

        // 3. 实体检索（精确匹配，REQ-PERF-006 实体链接增强）
        let query_entities: Vec<String> = crate::entity_extractor::EntityExtractor::extract(query)
            .into_iter()
            .map(|e| e.text)
            .collect();
        let entity_hits = if query_entities.is_empty() {
            vec![]
        } else {
            self.storage
                .entity_search(&query_entities, retrieval_k)
                .await
                .unwrap_or_else(|e| {
                    eprintln!("实体检索失败，降级为空: {e:#}");
                    vec![]
                })
        };

        // 4. 三路 RRF 融合（Vector + BM25 + Entity，REQ-PERF-006）
        let fused = rrf_fuse_three_way(vector_hits, keyword_hits, entity_hits, retrieval_k);

        Ok(fused)
    }
}

impl<E: Embedder, S: Storage> Retriever for HybridRetriever<E, S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }

        // 混合检索关闭且无 reranker 时，走原有快速路径（向量检索 + 扩展）
        if !self.hybrid_enabled && self.reranker.is_none() {
            // 查询改写（REQ-RAG-021 HyDE）：快速路径也支持改写
            let embedding_query = if let Some(rewriter) = &self.rewriter {
                let rewritten = rewriter.rewrite(query).await.unwrap_or_else(|e| {
                    eprintln!("HyDE 改写失败，使用原始查询: {e:#}");
                    query.to_string()
                });
                if rewritten.trim().is_empty() {
                    eprintln!("HyDE 改写返回空内容，使用原始查询");
                    query.to_string()
                } else {
                    rewritten
                }
            } else {
                query.to_string()
            };
            let embedding = self.embedder.embed(&embedding_query).await?;
            let mut hits = self.storage.vector_search(&embedding, top_k).await?;
            hits.retain(|h| h.score >= self.score_threshold);
            // MMR 多样性重排（如已启用）
            let mmr_hits = self.apply_mmr(hits, top_k);
            // 质量门控评估（仅日志，不影响结果）
            self.log_quality(&mmr_hits, query);
            return crate::retriever::expand_neighbors(&self.storage, &mmr_hits).await;
        }

        match &self.reranker {
            Some(reranker) => {
                let candidate_k = top_k.saturating_mul(RERANK_OVER_RETRIEVE_FACTOR);
                let candidates = self.retrieve_candidates(query, candidate_k).await?;
                if candidates.is_empty() {
                    return Ok(vec![]);
                }
                let reranked = reranker.rerank(query, &candidates).await?;
                let top_k_results: Vec<RetrievalResult> =
                    reranked.into_iter().take(top_k).collect();
                // MMR 多样性重排（如已启用）
                let mmr_results = self.apply_mmr(top_k_results, top_k);
                // 质量门控评估（仅日志）
                self.log_quality(&mmr_results, query);
                crate::retriever::expand_neighbors(&self.storage, &mmr_results).await
            }
            None => {
                let fused = self.retrieve_candidates(query, top_k).await?;
                let fused_top: Vec<RetrievalResult> =
                    fused.into_iter().take(top_k.saturating_mul(2)).collect();
                // MMR 多样性重排（如已启用）
                let mmr_results = self.apply_mmr(fused_top, top_k);
                // 质量门控评估（仅日志）
                self.log_quality(&mmr_results, query);
                crate::retriever::expand_neighbors(&self.storage, &mmr_results).await
            }
        }
    }

    /// 性能优化：使用预计算嵌入跳过冗余 ONNX 推理（省 ~50-100ms）。
    ///
    /// 当 HyDE 未启用时，直接复用预计算嵌入进行向量检索；
    /// 当 HyDE 启用时，回退到 `retrieve()`（因为改写后的查询需要重新嵌入）。
    async fn retrieve_with_embedding(
        &self,
        query: &str,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        if top_k == 0 {
            return Ok(vec![]);
        }

        // HyDE 启用时回退到 retrieve（需要重新嵌入改写后的查询）
        if self.rewriter.is_some() {
            return self.retrieve(query, top_k).await;
        }

        // 混合检索关闭且无 reranker 时，走快速路径
        if !self.hybrid_enabled && self.reranker.is_none() {
            let mut hits = self.storage.vector_search(query_embedding, top_k).await?;
            hits.retain(|h| h.score >= self.score_threshold);
            // MMR 多样性重排（如已启用）
            let mmr_hits = self.apply_mmr(hits, top_k);
            // 质量门控评估（仅日志）
            self.log_quality(&mmr_hits, query);
            return crate::retriever::expand_neighbors(&self.storage, &mmr_hits).await;
        }

        match &self.reranker {
            Some(reranker) => {
                let candidate_k = top_k.saturating_mul(RERANK_OVER_RETRIEVE_FACTOR);
                let candidates = self
                    .retrieve_candidates_with_embedding(query, query_embedding, candidate_k)
                    .await?;
                if candidates.is_empty() {
                    return Ok(vec![]);
                }
                let reranked = reranker.rerank(query, &candidates).await?;
                let top_k_results: Vec<RetrievalResult> =
                    reranked.into_iter().take(top_k).collect();
                // MMR 多样性重排（如已启用）
                let mmr_results = self.apply_mmr(top_k_results, top_k);
                // 质量门控评估（仅日志）
                self.log_quality(&mmr_results, query);
                crate::retriever::expand_neighbors(&self.storage, &mmr_results).await
            }
            None => {
                let fused = self
                    .retrieve_candidates_with_embedding(query, query_embedding, top_k)
                    .await?;
                let fused_top: Vec<RetrievalResult> =
                    fused.into_iter().take(top_k.saturating_mul(2)).collect();
                // MMR 多样性重排（如已启用）
                let mmr_results = self.apply_mmr(fused_top, top_k);
                // 质量门控评估（仅日志）
                self.log_quality(&mmr_results, query);
                crate::retriever::expand_neighbors(&self.storage, &mmr_results).await
            }
        }
    }
}
