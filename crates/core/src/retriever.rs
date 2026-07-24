//! 向量检索器（REQ-RAG-003）：Embedder + Storage 编排与相关度阈值过滤。
//! 低于阈值的检索结果直接丢弃，防止低质量上下文污染 LLM 提示词。

use echomind_models::RetrievalResult;

use crate::{Embedder, Retriever, Storage};

/// 默认相关度阈值：低于该分数的检索结果直接丢弃（REQ-RAG-003）
pub const DEFAULT_SCORE_THRESHOLD: f32 = 0.5;

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
}

impl<E: Embedder, S: Storage> Retriever for VectorRetriever<E, S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
        let embedding = self.embedder.embed(query).await?;
        let mut hits = self.storage.vector_search(&embedding, top_k).await?;
        hits.retain(|h| h.score >= self.score_threshold);
        Ok(hits)
    }
}
