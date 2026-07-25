# 代码契约 — Traits 与数据模型

> **状态**：活跃 | **最后审阅**：2026-07-25（v4 补充 MultimodalPdfLoader + ImportService 多模态索引 + DocStatusPayload.sub_phase）
>
> 本文档是 AI Agent 开发的**第一技术参考**（铁律七文档优先级 #2）。
> 所有签名必须与源码逐字一致。

---

## 1. 核心 Trait（`crates/core/src/lib.rs`）

### 1.1 Loader

```rust
pub trait Loader: Send + Sync {
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}
```

### 1.2 Splitter

```rust
pub trait Splitter: Send + Sync {
    async fn split(&self, text: &str) -> anyhow::Result<Vec<String>>;
}
```

### 1.3 Embedder

```rust
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>>;
}
```

### 1.4 Storage

```rust
pub trait Storage: Send + Sync {
    async fn add_document(&self, doc: &Document) -> anyhow::Result<()>;
    async fn update_doc_status(&self, doc_id: &str, status: DocStatus) -> anyhow::Result<()>;
    async fn add_chunk(&self, chunk: &Chunk) -> anyhow::Result<()>;
    async fn add_embedding(&self, chunk_id: &str, embedding: &[f32]) -> anyhow::Result<()>;
    async fn vector_search(&self, query_embedding: &[f32], top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;
    async fn find_document_by_hash(&self, hash: &str) -> anyhow::Result<Option<Document>>;
    async fn count_documents(&self) -> anyhow::Result<usize>;
    async fn cleanup_zombies(&self) -> anyhow::Result<usize>;
    async fn set_setting(&self, key: &str, value: &str) -> anyhow::Result<()>;
    async fn get_setting(&self, key: &str) -> anyhow::Result<Option<String>>;
    async fn create_conversation(&self, conversation: &Conversation) -> anyhow::Result<()>;
    async fn list_conversations(&self, workspace_id: &str) -> anyhow::Result<Vec<Conversation>>;
    async fn delete_conversation(&self, id: &str) -> anyhow::Result<()>;
    async fn update_conversation_title(&self, id: &str, title: &str) -> anyhow::Result<()>;
    async fn add_message(&self, conversation_id: &str, message: &ChatMessage) -> anyhow::Result<()>;
    async fn list_messages(&self, conversation_id: &str) -> anyhow::Result<Vec<ChatMessage>>;
    async fn list_documents(&self) -> anyhow::Result<Vec<Document>>;
    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()>;
    async fn list_chunks(&self, doc_id: &str) -> anyhow::Result<Vec<Chunk>>;
}
```

### 1.5 LLMProvider

```rust
pub trait LLMProvider: Send + Sync {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>>;
}
```

### 1.6 Retriever

```rust
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;
}
```

### 1.7 PageRenderer

```rust
/// PDF 页面渲染端口（REQ-MM-001）：将 PDF 页面渲染为位图，供 OCR 或 VLM 处理。
pub trait PageRenderer: Send + Sync {
    async fn render_page(
        &self,
        pdf_path: &str,
        page_num: usize,
        dpi: u32,
    ) -> anyhow::Result<Vec<u8>>;
}
```

- `page_num` 从 0 开始计数
- `dpi` 建议 150-200（平衡清晰度与内存）
- 返回 PNG 字节流，供下游 `OcrEngine` 或 `VisionLLM` 消费
- 渲染为 CPU 密集任务，实现方必须经 `spawn_blocking` 执行（铁律三）

### 1.8 OcrEngine

```rust
/// OCR 文字识别端口（REQ-MM-002）：对位图执行 OCR，提取文字内容。
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image_bytes: &[u8]) -> anyhow::Result<String>;
}
```

- 输入为位图字节（PNG/JPEG），输出为识别文本
- OCR 不可用或图片无文字时返回空字符串（优雅降级，不返回 Err）
- OCR 为 CPU 密集任务，实现方必须经 `spawn_blocking` 执行
- 纯本地运行，零网络请求（符合「隐私不出域」承诺，ADR-010 安全官要求）

### 1.9 VisionLanguageModel

```rust
/// VLM 图片理解端口（REQ-MM-003）：将位图发送到视觉语言模型，获取结构化文本描述。
pub trait VisionLanguageModel: Send + Sync {
    async fn describe_image(&self, image_bytes: &[u8], prompt: &str) -> anyhow::Result<String>;
}
```

- 图片以 base64 编码嵌入请求体，发送到用户配置的 LLM 端点（BYOK）
- VLM 不可用或调用失败时返回空字符串（优雅降级，不阻断索引管线）
- 网络请求必须且仅发往用户配置的 `base_url`（ADR-010 安全官要求 AC-4）
- 用户可在设置中关闭 VLM 增强（默认关闭），此时使用 `NoVlm` 占位实现

**`NoVlm` 占位实现**（VLM 禁用时使用）:
```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct NoVlm;

impl VisionLanguageModel for NoVlm {
    async fn describe_image(&self, _image_bytes: &[u8], _prompt: &str) -> anyhow::Result<String> {
        Ok(String::new()) // 始终返回空字符串，管线天然跳过 VLM 阶段
    }
}
```

> **Splitter 实现说明**：`Splitter` 端口有两个实现：
> - `TextSplitter`（`crates/core/src/splitter.rs`）— 固定 token 窗口分块（默认 512/64 overlap）
> - `SemanticSplitter`（`crates/core/src/semantic_splitter.rs`）— 语义分块（段落→句子→子句递归降级，详见 §5）

---

## 2. 数据模型（`crates/models/src/lib.rs`）

### 2.1 DocStatus

```rust
pub enum DocStatus {
    Pending,
    Processing,
    Indexed,
    Failed(String),
}
```

### 2.2 Document

```rust
pub struct Document {
    pub id: String,           // UUID v4
    pub file_path: String,    // 入库后本地路径
    pub file_hash: String,    // MD5 内容指纹
    pub status: DocStatus,
    pub created_at: i64,       // Unix 秒级时间戳
}
```

### 2.3 Chunk

```rust
pub struct Chunk {
    pub id: String,           // UUID v4
    pub doc_id: String,
    pub content: String,
    pub token_count: usize,
    pub sequence: usize,
}
```

### 2.4 RetrievalResult

```rust
pub struct RetrievalResult {
    pub chunk: Chunk,
    pub score: f32,
    pub doc_name: String,
}
```

### 2.5 ChatMessage

```rust
pub struct ChatMessage {
    pub role: String,                          // user / assistant / system
    pub content: String,
    pub sources: Option<Vec<RetrievalResult>>, // 助手消息的引用来源
}
```

### 2.6 Conversation

```rust
pub struct Conversation {
    pub id: String,           // UUID v4
    pub workspace_id: String, // v1.0 单工作区 "default"
    pub title: String,
    pub created_at: i64,
}
```

### 2.7 LlmConfig

```rust
pub struct LlmConfig {
    pub api_key: String,   // 允许空串（兼容本地 Ollama）
    pub base_url: String,  // OpenAI 兼容端点
    pub model: String,     // 模型名
}
```

### 2.8 SettingsPayload

```rust
pub struct SettingsPayload {
    pub has_llm_config: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_masked: String,  // **** + 末四位
    pub vlm_enabled: bool,       // VLM 图片理解增强开关（REQ-MM-003）
}
```

### 2.9 事件负载

```rust
pub struct DocStatusPayload {
    pub status: String,           // indexing / done / error
    pub message: String,          // 可读进度文案
    pub sub_phase: Option<String>, // text_extracting / image_rendering / ocr / vlm_enhancing（REQ-MM-004）
}

pub struct ChatPhasePayload {
    pub phase: String,     // preparing / retrieving / generating
    pub message: String,
}
```

---

## 3. AppState（`crates/tauri-app/src/state.rs`）

```rust
pub struct AppState {
    pub storage: SqliteStorage,
    pub data_dir: PathBuf,
    pub llm_config: tokio::sync::RwLock<Option<LlmConfig>>,
    pub is_pro: tokio::sync::RwLock<bool>,
    embedder: tokio::sync::OnceCell<LocalEmbedder>,  // 懒加载
    page_renderer: tokio::sync::OnceCell<PdfiumRenderer>,  // 懒加载（REQ-MM-001）
    ocr_engine: tokio::sync::OnceCell<OcrsEngine>,  // 懒加载（REQ-MM-002）
    // vision_provider 不再缓存（REQ-MM-003 前端接线）：每次根据 settings 重新创建，支持运行时开关
    abort_tokens: tokio::sync::Mutex<HashMap<String, CancellationToken>>,
}
```

关键方法：
- `AppState::new(data_dir) -> anyhow::Result<Self>` — 初始化（DB + 崩溃恢复 + 配置恢复）
- `embedder() -> anyhow::Result<&LocalEmbedder>` — 懒加载（首次触发模型下载）
- `page_renderer() -> anyhow::Result<&PdfiumRenderer>` — 懒加载 PDF 渲染引擎（REQ-MM-001）
- `ocr_engine() -> anyhow::Result<&OcrsEngine>` — 懒加载 OCR 引擎（REQ-MM-002）
- `vision_provider() -> anyhow::Result<Option<OpenAIVisionProvider>>` — 获取 VLM 引擎（REQ-MM-003；每次根据 `mm.vlm_enabled` 重新创建，支持运行时开关）
- `abort_token_for(conversation_id) -> CancellationToken` — 获取/创建中断令牌
- `abort_chat(conversation_id)` — 触发中断
- `clear_abort(conversation_id)` — 清理令牌

---

## 4. 适配器实现（`crates/infra`）

| 端口 | 适配器 | 文件 |
|---|---|---|
| Storage | `SqliteStorage` | `crates/infra/src/sqlite_storage.rs` |
| Embedder | `LocalEmbedder` | `crates/infra/src/local_embedder.rs` |
| LLMProvider | `OpenAIProvider` | `crates/infra/src/openai_provider.rs` |
| PageRenderer | `PdfiumRenderer` | `crates/infra/src/pdfium_renderer.rs` |
| OcrEngine | `OcrsEngine` | `crates/infra/src/ocrs_engine.rs` |
| VisionLanguageModel | `OpenAIVisionProvider` | `crates/infra/src/openai_vision.rs` |

### 4.1 SqliteStorage

- `rusqlite` + WAL 模式
- 所有操作经 `tokio::task::spawn_blocking`
- 向量存储为 BLOB（原始 f32 字节）
- 余弦相似度全量加载后 Rust 计算
- 崩溃恢复：`cleanup_zombies()` 将 Processing → Failed

### 4.2 LocalEmbedder

- `fastembed` + ONNX Runtime
- 模型：all-MiniLM-L6-v2（384 维，~30MB）
- 懒加载（`OnceCell`）
- 批量嵌入支持

### 4.3 OpenAIProvider

- OpenAI Chat Completions 兼容 API
- SSE 流式输出
- 支持本地 Ollama（空 API Key）
- `test_connection()` 方法验证配置

### 4.4 PdfiumRenderer

`crates/infra/src/pdfium_renderer.rs` — PDF 页面渲染适配器（实现 `PageRenderer` 端口）。

- 基于 `pdfium-render` v0.9.3（Google Chrome Pdfium C++ 绑定，ADR-010 确认）
- `OnceCell<Arc<Pdfium>>` 惰性初始化（首次调用 `Pdfium::bind_to_system_library()` 显式绑定系统路径动态库，找不到库返回 `Err` 而非 panic，铁律五零 panic）
- `PdfRenderConfig::new().scale_page_by_factor(dpi / 72.0)` 按 DPI 缩放渲染
- `bitmap.as_image()` → `image::DynamicImage` → PNG 编码
- 渲染经 `spawn_blocking` 执行（CPU 密集）
- **隐私**：✅ 完全本地渲染，零网络请求
- **部署**：pdfium 二进制文件随 Tauri 应用打包分发（resource 机制）

### 4.5 OcrsEngine

`crates/infra/src/ocrs_engine.rs` — 本地 OCR 适配器（实现 `OcrEngine` 端口）。

- 基于 `ocrs` v0.12.2 + `rten` v0.24.0（纯 Rust PaddleOCR PP-OCRv4，ADR-010 确认）
- `Arc<RwLock<Option<ocrs::OcrEngine>>>` 惰性加载
- 模型从 GitHub Releases 下载（`zh-cn.det-model.rten` ~4.7MB + `zh-cn.rec-model.rten` ~11.8MB）
- `init()` 幂等初始化：下载模型 → `rten::Model::load_file` → `ocrs::OcrEngine::new`
- `recognize()` 流程：`spawn_blocking` + `blocking_read` → 解码图片 → RGB8 → 检测 → 分行 → 识别 → 后处理
- 后处理：去除行首尾空白 + 合并连续空行（最多保留一个）
- 优雅降级：未初始化或 OCR 失败时返回空字符串，不崩溃
- **隐私**：✅ 完全本地运行，图片数据不离开设备，零网络请求

### 4.6 OpenAIVisionProvider

`crates/infra/src/openai_vision.rs` — VLM 图片理解适配器（实现 `VisionLanguageModel` 端口）。

- 复用 BYOK LLM 配置（api_key / base_url / model），无需额外设置
- 调用 OpenAI 兼容 Vision API（Chat Completions + `image_url` base64 data URL）
- 非流式请求（VLM 需完整理解图片后一次性返回）
- 系统提示引导：表格→Markdown 表格、甘特图→Mermaid gantt、流程图→Mermaid flowchart
- `max_tokens = 2000` 防止返回过长描述拖慢索引管线
- 连接超时 30s，总体超时 120s（图片理解比文本对话耗时更长）
- 优雅降级：HTTP 错误 / 超时 / 响应格式异常时返回空字符串，不崩溃
- **隐私**：图片仅发送到用户配置的 `base_url`（BYOK 控制，AC-4 网络审计）
- **部署**：无需额外依赖，复用 `reqwest` + `base64`（已在 Cargo.toml 中）

---

## 5. core 层具体实现

> 以下实现位于 `crates/core`（非 infra 适配器），是端口 Trait 的领域内实现。

### 5.1 SemanticSplitter（`crates/core/src/semantic_splitter.rs`）

实现 `Splitter` 端口，替代纯 token 窗口分块，保留语义完整性。
借鉴 LangChain `MarkdownHeaderTextSplitter` 与 LlamaIndex `SentenceSplitter`，纯 Rust 实现。

```rust
pub struct SemanticSplitter {
    target_tokens: usize,
    overlap_tokens: usize,
    max_tokens: usize,  // target_tokens * MAX_CHUNK_MULTIPLIER(3)
}

impl SemanticSplitter {
    pub fn new(target_tokens: usize, overlap_tokens: usize) -> anyhow::Result<Self>;
    pub fn default_config() -> anyhow::Result<Self>;  // 256 tokens / 32 overlap
    pub fn count_tokens(&self, text: &str) -> anyhow::Result<usize>;
}

impl Splitter for SemanticSplitter {
    async fn split(&self, text: &str) -> anyhow::Result<Vec<String>>;
}
```

**分块策略**（递归降级）：
1. 按段落分割（`\n\n`，代码块 ```...``` 保持完整）
2. 段落过长 → 按句子分割（中文 `。！？`；英文 `. ! ?`）
3. 句子过长 → 按子句分割（`，；,;`）
4. 子句仍超窗口 → 回退 token 窗口切分
5. 合并相邻小块至目标 token 数
6. 重叠窗口（默认 32 tokens）

### 5.2 TextSplitter（`crates/core/src/splitter.rs`）

实现 `Splitter` 端口，固定 token 窗口分块（默认 512 tokens / 64 overlap）。使用 `tiktoken-rs` cl100k_base BPE 编码。

### 5.3 ChatEngine（`crates/core/src/chat.rs`）

RAG 查询编排器，组装 Retriever + LLMProvider，构建 RAG Prompt（含图表引导指令 REQ-VIZ-004）。

### 5.4 ImportService（`crates/core/src/import.rs`）

文档导入管线编排：Loader → MD5 去重 → Splitter → Storage → Embedder → Storage。含配额/PDF 门控。

**多模态扩展（REQ-MM-004）**：新增泛型方法 `index_document_multimodal<R, O>`，
注入 `PageRenderer` + `OcrEngine`，对 PDF 执行文本提取 + 图片检测 + 渲染 + OCR + 分块入库。
降级策略：多模态引擎初始化失败时回退到 `index_document`（纯文本索引）。

**VLM 增强扩展（REQ-MM-003）**：新增泛型方法 `index_document_multimodal_with_vlm<R, O, V>`，
注入 `PageRenderer` + `OcrEngine` + `VisionLanguageModel`，在 OCR 基础上额外调用 VLM，
将表格→Markdown、甘特图→Mermaid 等结构化内容追加到文本中。
`index_document_multimodal` 委托给 `index_document_multimodal_with_vlm` 并传入 `NoVlm` 占位。
降级策略：VLM 不可用时返回空字符串，管线天然跳过 VLM 阶段，仅使用 OCR。

### 5.5 MultimodalPdfLoader（`crates/core/src/loader.rs`）

多模态 PDF 加载器，实现 `Loader` 端口。泛型注入 `PageRenderer` + `OcrEngine` + `VisionLanguageModel`。

```rust
pub struct MultimodalPdfLoader<R: PageRenderer, O: OcrEngine, V: VisionLanguageModel = NoVlm> {
    page_renderer: R,
    ocr_engine: O,
    vlm: V,
}

impl<R: PageRenderer, O: OcrEngine> MultimodalPdfLoader<R, O, NoVlm> {
    pub fn new(page_renderer: R, ocr_engine: O) -> Self;
}

impl<R: PageRenderer, O: OcrEngine, V: VisionLanguageModel> MultimodalPdfLoader<R, O, V> {
    pub fn with_vlm(page_renderer: R, ocr_engine: O, vlm: V) -> Self;
    pub fn extract_text_and_detect_images(file_path: &str) -> anyhow::Result<(String, Vec<usize>)>;
}

impl<R: PageRenderer, O: OcrEngine, V: VisionLanguageModel> Loader for MultimodalPdfLoader<R, O, V> {
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}
```

**加载流程**：
1. `spawn_blocking` 执行 lopdf 文本提取 + `get_page_images()` 图片检测
2. 对含图片的页面 async 调用 `PageRenderer.render_page()` 渲染为位图
3. 对位图调用 `OcrEngine.recognize()` 提取文字
4. 对位图调用 `VisionLanguageModel.describe_image()` 获取结构化描述（REQ-MM-003）
5. 合并文本层 + OCR 文本 + VLM 描述

**设计原因**：`async fn` in trait 不支持 `dyn Trait`（Edition 2024），故使用泛型注入。`V` 默认 `NoVlm`，无 VLM 时天然跳过 VLM 阶段。

### 5.6 VectorRetriever（`crates/core/src/retriever.rs`）

实现 `Retriever` 端口，query → embedding → `vector_search` → 阈值过滤 → **Chunk Expansion**。

```rust
pub struct VectorRetriever<E: Embedder, S: Storage> {
    embedder: E,
    storage: S,
    score_threshold: f32,  // DEFAULT_SCORE_THRESHOLD = 0.35
}
```

**Phase 4 Chunk Expansion**（`expand_neighbors` 方法）：

检索命中后，对每命中 chunk 调 `Storage.list_chunks(doc_id)` 获取同文档全部 chunk，找到命中 chunk 的位置，扩展前后各 1 个相邻 chunk，补全跨 chunk 上下文（如被分块器切断的段落、代码块后续行）。

- 扩展的 chunk 使用命中 chunk 的 score 作为近似（同文档同主题假设）
- 去重（按 chunk_id，防止相邻命中 chunk 扩展出重复 chunk）
- 结果按 (doc_id, sequence) 排序，使 prompt 上下文按文档顺序排列
- `list_chunks` 返回空或找不到命中 chunk 时不扩展（优雅降级）

**检索流程**：
1. `embedder.embed(query)` → 查询向量
2. `storage.vector_search(embedding, top_k)` → top-k 候选
3. `retain(|h| h.score >= 0.35)` → 阈值过滤
4. `expand_neighbors(hits)` → 扩展相邻 chunk（Phase 4）
5. 返回 `Vec<RetrievalResult>`（可能 > top_k 个，含扩展 chunk）

**Phase 3 ContextualEmbedding**（`build_contextual_text` 函数）：

嵌入时为 chunk 内容拼接文档名上下文前缀（`文档《{doc_name}》：\n{chunk_content}`），
使向量包含文档上下文信息，提升检索质量（Anthropic Contextual Retrieval 调研：
检索失败率 ↓49%）。EchoMind 采用零 LLM 成本规则方案（不调用 LLM 生成上下文摘要）。

- 调用点：`commands.rs::embed_document_chunks`（嵌入写入链路）
- 不改变 `chunk.content`（存储仍为原始内容），仅嵌入文本拼接前缀
- 检索时 `build_rag_prompt` 仍用原始 `chunk.content`（不含前缀），保持引用块可读
