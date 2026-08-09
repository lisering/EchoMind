#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! ChatEngine 单元测试：`chat_with_sources`（预检索入口，性能优化用）行为验证。
//!
//! 覆盖：
//! - 空预检索结果 → `ChatOutcome::NoContext`（不调用 LLM）
//! - 非空预检索结果 → `ChatOutcome::Answered`（走 LLM 流式）

use crate::chat::{ChatEngine, ChatOutcome, NO_CONTEXT_MESSAGE};
use crate::{LLMProvider, Retriever};
use echomind_models::{ChatMessage, RetrievalResult};
use futures::stream::BoxStream;
use std::sync::Arc;

// ---- Mock 端口 ----

struct MockRetriever;

impl Retriever for MockRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> anyhow::Result<Vec<RetrievalResult>> {
        // chat_with_sources 不调用 retrieve；此 mock 仅满足 trait 约束
        Ok(vec![])
    }
}

struct MockLlm {
    /// 是否被调用过（验证 NoContext 路径不触碰 LLM）
    called: Arc<std::sync::atomic::AtomicBool>,
}

impl MockLlm {
    fn new() -> (Self, Arc<std::sync::atomic::AtomicBool>) {
        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        (
            Self {
                called: called.clone(),
            },
            called,
        )
    }
}

impl LLMProvider for MockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        self.called.store(true, std::sync::atomic::Ordering::SeqCst);
        let stream =
            futures::stream::once(async { Ok::<String, anyhow::Error>("mock 回答".into()) });
        Ok(Box::pin(stream))
    }

    async fn chat_stream_segmented(
        &self,
        _static_prefix: &str,
        _dynamic_context: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        self.called.store(true, std::sync::atomic::Ordering::SeqCst);
        let stream =
            futures::stream::once(async { Ok::<String, anyhow::Error>("mock 回答".into()) });
        Ok(Box::pin(stream))
    }
}

fn sample_chunk(doc: &str, seq: usize) -> RetrievalResult {
    RetrievalResult {
        chunk: echomind_models::Chunk {
            id: format!("chunk-{seq}"),
            doc_id: format!("doc-{seq}"),
            content: format!("{doc} 第 {seq} 段内容"),
            token_count: 10,
            sequence: seq,
        },
        score: 0.9,
        doc_name: format!("{doc}.md"),
    }
}

// ---- 测试 ----

#[tokio::test]
async fn tc_chat_prefetch_001_empty_sources_returns_no_context() {
    let (llm, called) = MockLlm::new();
    let engine = ChatEngine::new(MockRetriever, llm);
    let history: Vec<ChatMessage> = vec![];

    let outcome = engine
        .chat_with_sources(&history, "问题", vec![])
        .await
        .expect("空检索结果不应报错");

    assert!(
        matches!(outcome, ChatOutcome::NoContext { .. }),
        "空预检索结果应返回 NoContext"
    );
    assert!(
        !called.load(std::sync::atomic::Ordering::SeqCst),
        "NoContext 路径不应调用 LLM"
    );
}

#[tokio::test]
async fn tc_chat_prefetch_002_nonempty_sources_answers() {
    let (llm, called) = MockLlm::new();
    let engine = ChatEngine::new(MockRetriever, llm);
    let history: Vec<ChatMessage> = vec![];
    let sources = vec![sample_chunk("文档A", 1), sample_chunk("文档B", 2)];

    let outcome = engine
        .chat_with_sources(&history, "问题", sources)
        .await
        .expect("非空检索结果不应报错");

    match outcome {
        ChatOutcome::Answered {
            sources,
            mut stream,
            ..
        } => {
            assert_eq!(sources.len(), 2, "应透传全部预检索结果");
            let mut collected = String::new();
            use futures::StreamExt;
            while let Some(Ok(tok)) = stream.next().await {
                collected.push_str(&tok);
            }
            assert_eq!(collected, "mock 回答");
        }
        _other => panic!("期望 Answered，得到未知变体（ChatOutcome 未实现 Debug）"),
    }
    assert!(
        called.load(std::sync::atomic::Ordering::SeqCst),
        "Answered 路径应调用 LLM"
    );
}

#[tokio::test]
async fn tc_chat_prefetch_003_no_context_message() {
    // 验证空检索时返回的流内容为 NO_CONTEXT_MESSAGE
    let (llm, _) = MockLlm::new();
    let engine = ChatEngine::new(MockRetriever, llm);
    let history: Vec<ChatMessage> = vec![];

    let outcome = engine
        .chat_with_sources(&history, "问题", vec![])
        .await
        .unwrap();
    let ChatOutcome::NoContext { mut stream } = outcome else {
        panic!("应返回 NoContext");
    };
    use futures::StreamExt;
    let mut text = String::new();
    while let Some(Ok(tok)) = stream.next().await {
        text.push_str(&tok);
    }
    assert_eq!(text, NO_CONTEXT_MESSAGE);
}
