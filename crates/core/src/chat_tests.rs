#![allow(clippy::unwrap_used)]
//! QA 红灯测试：TC-RAG-002 空上下文拦截（REQ-RAG-003-AC-1/AC-2）。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::{ChatMessage, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::chat::{ChatEngine, ChatOutcome, NO_CONTEXT_MESSAGE};
use crate::{LLMProvider, Retriever};

/// 空检索 Retriever（知识库未命中场景）。
struct EmptyRetriever;

impl Retriever for EmptyRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// 间谍 LLM：记录 chat_stream 调用次数（被调用即证明拦截失败）。
#[derive(Clone, Default)]
struct SpyLlm {
    calls: Arc<AtomicUsize>,
}

impl LLMProvider for SpyLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(futures::stream::empty().boxed())
    }
}

/// TC-RAG-002：检索为空时直接返回固定提示，绝不发起 LLM 网络请求（REQ-RAG-003）。
#[tokio::test]
async fn tc_rag_002_empty_context_skips_llm() {
    let llm = SpyLlm::default();
    let engine = ChatEngine::new(EmptyRetriever, llm.clone());

    let outcome = engine.chat(&[], "知识库没有的问题", 5).await.unwrap();

    let ChatOutcome::NoContext { stream } = outcome else {
        panic!("空检索结果必须走 NoContext 分支");
    };
    let tokens: Vec<String> = stream.map(|r| r.unwrap()).collect().await;
    assert_eq!(tokens.join(""), NO_CONTEXT_MESSAGE, "必须返回固定拒答提示");
    assert_eq!(
        llm.calls.load(Ordering::SeqCst),
        0,
        "空上下文时严禁发起 LLM 请求"
    );
}

/// TC-RAG-002 补充：有命中时走 Answered 分支且调用 LLM 恰好一次。
#[tokio::test]
async fn tc_rag_002b_with_context_calls_llm_once() {
    use echomind_models::Chunk;

    struct OneHitRetriever;

    impl Retriever for OneHitRetriever {
        async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
            Ok(vec![RetrievalResult {
                chunk: Chunk::new("doc-1".to_string(), "命中片段".to_string(), 3, 0),
                score: 0.9,
                doc_name: "note.md".to_string(),
            }])
        }
    }

    let llm = SpyLlm::default();
    let engine = ChatEngine::new(OneHitRetriever, llm.clone());

    let outcome = engine.chat(&[], "有答案的问题", 5).await.unwrap();

    let ChatOutcome::Answered { sources, .. } = outcome else {
        panic!("有检索结果必须走 Answered 分支");
    };
    assert_eq!(sources.len(), 1);
    assert_eq!(
        llm.calls.load(Ordering::SeqCst),
        1,
        "有上下文时必须调用 LLM 一次"
    );
}
