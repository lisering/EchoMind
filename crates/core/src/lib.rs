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
pub mod license;
pub mod loader;
pub mod retriever;
/// 语义分块器：段落→句子→子句递归分割，保留代码块完整性。
pub mod semantic_splitter;
/// 章节感知分块器：Markdown 标题层级 → section 切分 → 章节路径前缀（REQ-VEC-006）。
pub mod section_aware_splitter;
pub mod splitter;
pub mod stream_parse;

use echomind_models::{ChatMessage, Chunk, Conversation, DocStatus, Document, RetrievalResult};
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

    /// 写入设置项（REQ-UI-008；敏感值由实现方负责加密存储）。
    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()>;

    /// 读取设置项（REQ-UI-008）。
    async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>>;

    /// 创建会话（REQ-RAG-006；重复 ID 忽略，保证幂等）。
    async fn create_conversation(&self, conversation: &Conversation) -> anyhow::Result<()>;

    /// 列出工作区会话（按创建时间倒序）。
    async fn list_conversations(&self, workspace_id: &str) -> anyhow::Result<Vec<Conversation>>;

    /// 删除会话（外键级联清理其消息）。
    async fn delete_conversation(&self, id: &str) -> anyhow::Result<()>;

    /// 更新会话标题（首轮问答后自动提取）。
    async fn update_conversation_title(&self, id: &str, title: &str) -> anyhow::Result<()>;

    /// 写入一条消息。
    async fn add_message(&self, conversation_id: &str, message: &ChatMessage)
    -> anyhow::Result<()>;

    /// 列出会话消息（按写入顺序正序）。
    async fn list_messages(&self, conversation_id: &str) -> anyhow::Result<Vec<ChatMessage>>;

    /// 列出全部文档（按创建时间倒序，REQ-UI-006）。
    async fn list_documents(&self) -> anyhow::Result<Vec<Document>>;

    /// 删除文档（外键级联清理 chunks 与 embeddings，REQ-ING-005）。
    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()>;

    /// 列出文档全部分块（按 sequence 正序，嵌入写入链路用，REQ-VEC-003）。
    async fn list_chunks(&self, doc_id: &str) -> anyhow::Result<Vec<Chunk>>;
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

/// PDF 页面渲染端口（REQ-MM-001）：将 PDF 页面渲染为位图，供 OCR 或 VLM 处理。
///
/// 渲染引擎优先选择 Rust 可用的 PDF 渲染库（pdfium 绑定），经架构评审确定（ADR-010）。
/// 渲染为 CPU 密集任务，实现方必须经 `spawn_blocking` 执行。
pub trait PageRenderer: Send + Sync {
    /// 将指定页面渲染为位图（PNG 字节），供 OCR 或 VLM 处理。
    ///
    /// # 参数
    /// - `pdf_path`: PDF 文件路径
    /// - `page_num`: 页码（从 0 开始）
    /// - `dpi`: 渲染分辨率（建议 150-200，平衡清晰度与内存）
    ///
    /// # 错误
    /// - 文件不存在 / 非 PDF 格式 / 页码越界 / 渲染引擎初始化失败
    async fn render_page(
        &self,
        pdf_path: &str,
        page_num: usize,
        dpi: u32,
    ) -> anyhow::Result<Vec<u8>>;
}

/// OCR 文字识别端口（REQ-MM-002）：对位图执行 OCR，提取文字内容。
///
/// OCR 引擎使用 ocrs（纯 Rust PaddleOCR PP-OCRv4 + rten 推理引擎，ADR-010 架构评审确定），
/// 完全本地运行，零网络请求（符合「隐私不出域」承诺）。
/// OCR 为 CPU 密集任务，实现方必须经 `spawn_blocking` 执行。
pub trait OcrEngine: Send + Sync {
    /// 对位图执行 OCR，返回识别出的文本。
    ///
    /// # 参数
    /// - `image_bytes`: 位图数据（PNG/JPEG 字节）
    ///
    /// # 返回
    /// 识别出的文本内容。如果 OCR 不可用或图片中无文字，返回空字符串。
    async fn recognize(&self, image_bytes: &[u8]) -> anyhow::Result<String>;
}

/// VLM 图片理解端口（REQ-MM-003）：将位图发送到视觉语言模型，获取结构化文本描述。
///
/// 与 OCR（提取印刷文字）互补：VLM 能理解表格→Markdown 表格、甘特图→Mermaid gantt、
/// 流程图→Mermaid flowchart 等结构化内容。OCR 对这类结构化内容识别效果有限。
///
/// # 隐私边界
///
/// 图片仅发送到用户自行配置的 LLM 端点（BYOK），符合「隐私不出域」——数据出口由用户控制。
/// 网络请求必须且仅发往用户配置的 `base_url`（ADR-010 安全官要求 AC-4）。
/// 用户可在设置中关闭 VLM 增强（默认关闭），此时管线仅使用 OCR。
///
/// # 适配器
///
/// - `OpenAIVisionProvider`（`crates/infra/src/openai_vision.rs`）— 复用 reqwest，
///   调用 OpenAI 兼容 Vision API（Chat Completions + image_url base64）
pub trait VisionLanguageModel: Send + Sync {
    /// 将图片发送到 VLM，获取结构化文本描述。
    ///
    /// # 参数
    /// - `image_bytes`: 位图数据（PNG/JPEG 字节，base64 编码后嵌入请求体）
    /// - `prompt`: 系统提示，引导 VLM 将图片中的结构化内容转换为目标格式
    ///   （表格→Markdown 表格、甘特图→Mermaid gantt、流程图→Mermaid flowchart）
    ///
    /// # 返回
    /// VLM 生成的结构化文本描述。如果 VLM 不可用或调用失败，返回空字符串（优雅降级）。
    async fn describe_image(&self, image_bytes: &[u8], prompt: &str) -> anyhow::Result<String>;
}

/// VLM 禁用占位实现（REQ-MM-003 AC-3）：用户未开启 VLM 增强时使用。
///
/// `describe_image` 始终返回空字符串，使管线天然跳过 VLM 阶段，仅使用 OCR。
/// 设计为零大小类型 + `Send + Sync`，可安全跨线程共享。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoVlm;

impl VisionLanguageModel for NoVlm {
    /// 始终返回空字符串，表示 VLM 未启用。
    async fn describe_image(&self, _image_bytes: &[u8], _prompt: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
}

#[cfg(test)]
mod chat_tests;
#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod license_tests;
#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod retriever_tests;
#[cfg(test)]
mod splitter_tests;
#[cfg(test)]
mod stream_parse_tests;
