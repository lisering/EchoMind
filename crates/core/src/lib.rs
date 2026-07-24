//! EchoMind 六边形架构核心层（对应 REQ-ARCH-000）。
//! 铁律（PROJECT_RULES.md 体系三）：本 crate 只定义端口 Trait 与领域逻辑，
//! 严禁依赖 Tauri、reqwest、rusqlite 等任何具体实现；适配器一律落于 crates/infra（后续 Phase）。

// 体系三锁定技术栈：使用原生 `async fn` in trait（Edition 2024），禁用 async-trait 宏。
// `async_fn_in_trait` lint 提醒「pub trait 无法约束返回 Future 的 auto traits」；
// 本 crate 通过 `Send + Sync` supertrait 约束全部实现者，且端口仅供 workspace 内部消费，
// 属经架构评审的刻意豁免，非疏漏。
#![allow(async_fn_in_trait)]

pub mod chat;
pub mod import;
pub mod loader;
pub mod retriever;
pub mod splitter;
pub mod stream_parse;

use echomind_models::{ChatMessage, Chunk, DocStatus, Document, RetrievalResult};
use futures::stream::BoxStream;

/// 文档加载端口：从文件路径提取纯文本（对应 REQ-ING-001/002）。
pub trait Loader: Send + Sync {
    /// 读取并解析指定文件，返回纯文本内容；不支持的格式或 I/O 错误返回 Err。
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}

/// 文本分块端口（对应 REQ-VEC-001）。
pub trait Splitter: Send + Sync {
    /// 将整篇文本切分为有序分块；空文本返回空 Vec 而非 Err。
    async fn split(&self, text: &str) -> anyhow::Result<Vec<String>>;
}

/// Embedding 端口（对应 REQ-VEC-002）：本地模型与 BYOK 远程端点均实现本端口。
pub trait Embedder: Send + Sync {
    /// 计算单条文本的向量。
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;

    /// 批量计算向量；默认实现逐条调用 `embed`，适配器可覆盖为真正的批量请求。
    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}

/// 持久化端口：文档、分块、向量与检索（对应 REQ-VEC-003/004、REQ-ING-005）。
pub trait Storage: Send + Sync {
    /// 写入新文档记录。
    async fn add_document(&self, doc: &Document) -> anyhow::Result<()>;

    /// 推进文档索引状态机；每次迁移由组装层负责发射 Tauri Event。
    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> anyhow::Result<()>;

    /// 写入分块记录。
    async fn add_chunk(&self, chunk: &Chunk) -> anyhow::Result<()>;

    /// 为指定分块写入向量。
    async fn add_embedding(&self, chunk_id: &str, embedding: &[f32]) -> anyhow::Result<()>;

    /// 以查询向量执行 top-k 相似度检索。
    async fn vector_search(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>>;

    /// 按内容指纹查找文档（REQ-ING-004 去重）。
    async fn find_document_by_hash(&self, hash: &str) -> anyhow::Result<Option<Document>>;

    /// 统计已入库文档总数（REQ-LIC-001 配额判定）。
    async fn count_documents(&self) -> anyhow::Result<usize>;

    /// 崩溃恢复（REQ-DB-001）：将上次会话遗留的 Processing 僵尸文档置为 Failed，返回清理条数。
    async fn cleanup_zombies(&self) -> anyhow::Result<usize>;
}

/// 大模型端口（对应 REQ-RAG-001/007）：token 级流式输出。
pub trait LLMProvider: Send + Sync {
    /// 发起流式对话；返回的流在首个 token 到达前即应建立，逐 token 产出增量文本。
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>>;
}

/// 检索端口（对应 REQ-RAG-003）：面向自然语言查询的 top-k 召回。
pub trait Retriever: Send + Sync {
    /// 对查询做向量化并检索最相关的 top_k 个分块。
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;
}

#[cfg(test)]
mod chat_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod retriever_tests;
#[cfg(test)]
mod splitter_tests;
#[cfg(test)]
mod stream_parse_tests;
