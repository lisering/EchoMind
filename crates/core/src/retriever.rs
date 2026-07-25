//! 向量检索器（REQ-RAG-003）：Embedder + Storage 编排与相关度阈值过滤。
//! 低于阈值的检索结果直接丢弃，防止低质量上下文污染 LLM 提示词。
//!
//! Phase 4 Chunk Expansion：检索命中后扩展前后各 1 个相邻 chunk，
//! 补全跨 chunk 上下文（如被分块器切断的段落、代码块后续行），
//! 显著减少「回答缺少关键上下文」的检索质量问题。

use echomind_models::RetrievalResult;

use crate::{Embedder, Retriever, Storage};

/// 默认相关度阈值：低于该分数的检索结果直接丢弃（REQ-RAG-003）
/// 0.35：all-MiniLM-L6-v2 对中文查询与 256-token 分块的余弦相似度通常在 0.3-0.5 区间，
/// 0.5 阈值会误杀大量有效结果导致空检索（用户反馈：上传文档后提问"没有任何反应"）。
pub const DEFAULT_SCORE_THRESHOLD: f32 = 0.35;

/// 向量检索适配器：query → embedding → vector_search → 阈值过滤。
pub struct VectorRetriever<E: Embedder, S: Storage> {
    embedder: E,
    storage: S,
    score_threshold: f32,
}

impl<E: Embedder, S: Storage> VectorRetriever<E, S> {
    pub fn new(embedder: E, storage: S) -> Self {
        Self {
            embedder,
            storage,
            score_threshold: DEFAULT_SCORE_THRESHOLD,
        }
    }

    /// Phase 4 Chunk Expansion：扩展命中 chunk 的相邻 chunk（前后各 1 个），合并去重。
    ///
    /// 检索命中后，对每命中 chunk 调 `list_chunks` 获取同文档全部 chunk，
    /// 找到命中 chunk 的位置，扩展前后各 1 个相邻 chunk，补全跨 chunk 上下文
    ///（如被分块器切断的段落、代码块后续行）。扩展的 chunk 使用命中 chunk 的
    /// score 作为近似（同文档同主题假设）。结果按 (doc_id, sequence) 排序，
    /// 使 prompt 上下文按文档顺序排列，提升 LLM 理解连贯性。
    async fn expand_neighbors(
        &self,
        hits: &[RetrievalResult],
    ) -> anyhow::Result<Vec<RetrievalResult>> {
        use std::collections::HashSet;

        let mut expanded: Vec<RetrievalResult> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        for hit in hits {
            // 添加命中 chunk 本身
            if seen.insert(hit.chunk.id.clone()) {
                expanded.push(hit.clone());
            }

            // 获取同文档全部 chunk（按 sequence 正序）
            let all_chunks = self.storage.list_chunks(&hit.chunk.doc_id).await?;
            if all_chunks.is_empty() {
                continue;
            }

            // 找到命中 chunk 的位置，扩展前后各 1 个
            if let Some(idx) = all_chunks.iter().position(|c| c.id == hit.chunk.id) {
                // 前一个 chunk
                if idx > 0 {
                    let prev = &all_chunks[idx - 1];
                    if seen.insert(prev.id.clone()) {
                        expanded.push(RetrievalResult {
                            chunk: prev.clone(),
                            score: hit.score,
                            doc_name: hit.doc_name.clone(),
                        });
                    }
                }
                // 后一个 chunk
                if idx + 1 < all_chunks.len() {
                    let next = &all_chunks[idx + 1];
                    if seen.insert(next.id.clone()) {
                        expanded.push(RetrievalResult {
                            chunk: next.clone(),
                            score: hit.score,
                            doc_name: hit.doc_name.clone(),
                        });
                    }
                }
            }
        }

        // 按 (doc_id, sequence) 排序，使 prompt 上下文按文档顺序排列
        expanded.sort_by(|a, b| {
            a.chunk
                .doc_id
                .cmp(&b.chunk.doc_id)
                .then(a.chunk.sequence.cmp(&b.chunk.sequence))
        });

        Ok(expanded)
    }
}

impl<E: Embedder, S: Storage> Retriever for VectorRetriever<E, S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
        let embedding = self.embedder.embed(query).await?;
        let mut hits = self.storage.vector_search(&embedding, top_k).await?;
        hits.retain(|h| h.score >= self.score_threshold);
        // Phase 4: Chunk Expansion — 扩展相邻 chunk 补全跨 chunk 上下文
        self.expand_neighbors(&hits).await
    }
}
