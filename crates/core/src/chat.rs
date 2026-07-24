//! 对话编排引擎（REQ-RAG-001/003）：检索 → 空上下文拦截 → 组装 Prompt → 调用 LLM。
//! 空上下文直接返回固定提示，不发起任何 LLM 网络请求（防止低质量上下文污染与无效计费）。

use echomind_models::{ChatMessage, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::{LLMProvider, Retriever};

/// 空上下文固定拒答文案（REQ-RAG-003-AC-1）
pub const NO_CONTEXT_MESSAGE: &str = "知识库中未找到相关内容。请尝试换个问题，或先导入相关文档。";

/// 对话编排输出。
pub enum ChatOutcome {
    /// 正常回答：引用来源 + token 流
    Answered {
        /// 实际注入上下文的引用来源（与 chat_sources 事件一致）
        sources: Vec<RetrievalResult>,
        /// token 增量流
        stream: BoxStream<'static, anyhow::Result<String>>,
    },
    /// 知识库未命中：固定提示流（未调用 LLM）
    NoContext {
        /// 固定拒答文案流
        stream: BoxStream<'static, anyhow::Result<String>>,
    },
}

/// RAG 对话编排引擎（六边形架构：仅依赖端口 Trait）。
pub struct ChatEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
}

impl<R: Retriever, L: LLMProvider> ChatEngine<R, L> {
    pub fn new(retriever: R, llm: L) -> Self {
        Self { retriever, llm }
    }

    /// 执行一轮对话：检索 → 空上下文拦截 → 组装提示词 → 流式调用 LLM。
    pub async fn chat(
        &self,
        history: &[ChatMessage],
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<ChatOutcome> {
        let sources = self.retriever.retrieve(query, top_k).await?;
        if sources.is_empty() {
            // 空上下文拦截：固定提示，不调用 LLM（REQ-RAG-003-AC-2）
            let stream = futures::stream::once(async move {
                Ok::<String, anyhow::Error>(NO_CONTEXT_MESSAGE.to_string())
            })
            .boxed();
            return Ok(ChatOutcome::NoContext { stream });
        }
        let system_prompt = build_rag_prompt(&sources);
        let stream = self.llm.chat_stream(&system_prompt, history, query).await?;
        Ok(ChatOutcome::Answered { sources, stream })
    }
}

/// 组装 RAG 系统提示词：编号上下文 + 引用标注指令（REQ-RAG-002 前置）。
fn build_rag_prompt(sources: &[RetrievalResult]) -> String {
    let mut prompt = String::from(
        "你是「灵犀」本地知识库助手。请仅根据下列知识库片段回答用户问题；\
         若片段不足以回答，请明确说明而非编造。回答中引用片段处以 [1][2] 形式标注来源编号。\n\n",
    );
    for (i, src) in sources.iter().enumerate() {
        prompt.push_str(&format!(
            "[{}] 来源《{}》：\n{}\n\n",
            i + 1,
            src.doc_name,
            src.chunk.content
        ));
    }
    prompt
}
