# 代码契约 — Traits 与数据模型

> **状态**：活跃 | **最后审阅**：2026-08-10（v8.0 全量同步 QM/OpenCode 借鉴：LLMProvider 4 个辅助方法 + SecurityPosture/SecurityScreener/LlmRouter/KeyedQueue/SessionCoordinator/Skill/Permission/LlmProtocol 新模块 + AppState 新字段）
>
> 本文档是 AI Agent 开发的**第一技术参考**（铁律七文档优先级 #2）。
> 所有签名必须与源码逐字一致。

---

## 0. Crate 依赖方向

```
crates/models  →  crates/prompt  →  crates/core  →  crates/infra  →  crates/tauri-app
  (contracts)     (prompts)       (ports+logic)  (adapters)     (assembly)
                                     ↑
                              crates/compact
                              (compaction)
```

- `crates/prompt`：从 `crates/core` 提取的提示词构建逻辑（SegmentedPrompt / build_rag_prompt / build_agent_prompt），仅依赖 `echomind-models`。
- `crates/compact`：从 `crates/core` 提取的上下文压缩引擎（CompactionEngine），依赖 `echomind-models` + `echomind-core`（LLMProvider trait + bpe() tokenizer）。
- `crates/core` 不依赖 `crates/compact`（无循环依赖）；`chat_with_compaction` 编排逻辑移至命令层。

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
    // 首次编辑升级：把原始 user 行 + 紧随其后的 assistant 行原地标记为 turn_group 的 version=1
    // （默认实现 no-op；SqliteStorage 事务实现，REQ-QA 首次编辑分页）
    async fn promote_original_turn(
        &self,
        conversation_id: &str,
        original_message_id: &str,
        turn_group: &str,
    ) -> anyhow::Result<()>;
    // 设置/读取轮次活跃版本号（分支切换状态持久化；默认 no-op，SqliteStorage UPSERT/SELECT）
    async fn set_turn_active_version(
        &self,
        conversation_id: &str,
        turn_group: &str,
        active_version: i32,
    ) -> anyhow::Result<()>;
    async fn get_turn_active_versions(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<Vec<TurnActiveVersion>>;
    async fn list_documents(&self) -> anyhow::Result<Vec<Document>>;
    async fn delete_document(&self, doc_id: &str) -> anyhow::Result<()>;
    async fn list_chunks(&self, doc_id: &str) -> anyhow::Result<Vec<Chunk>>;
    async fn delete_chunks_by_doc(&self, doc_id: &str) -> anyhow::Result<()>;
    async fn keyword_search(
        &self,
        query: &str,
        top_k: usize,
    ) -> anyhow::Result<Vec<RetrievalResult>>;
    // 内部经 build_fts5_or_query 分词后逐词 OR 查询（非整句短语匹配）。
    // 英文按空格分词保留 _/-；纯 CJK 无空格时拆为 3 字符滑窗。
    // 每 token 双引号转义防 FTS5 操作符注入（REQ-SEC-012）。
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

    // Q10 辅助方法（默认实现返回 None/Ok(None)，向后兼容）
    async fn generate_title(&self, _query: &str, _answer: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    async fn one_shot(&self, _system: &str, _user: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    async fn judge(&self, _system: &str, _user: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    fn context_token_budget(&self) -> Option<usize> {
        None
    }
}
```

- `generate_title` — LLM 辅助标题生成（Q10 借鉴 QM 辅助模型方法；`OpenAIProvider` 覆盖实现，复用 `complete()` 非流式调用，30 字截断）
- `one_shot` — 非流式单次调用（Q10；用于 Burst Buffer 记忆提取、安全筛查判断等辅助 LLM 调用）
- `judge` — 裁决调用（Q10；用于 Shadow 安全筛查模式 S71）
- `context_token_budget` — 返回模型上下文窗口大小（Q10；用于 Token 预算计算）

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
    pub original_path: Option<String>, // 源文件路径（REQ-SYNC-002）
    pub domain: Option<String>,        // 领域分类标签（REQ-VEC-013）
    pub summary: Option<String>,       // 文档摘要（REQ-ING-019）
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
    pub id: Option<String>,                    // 消息行 id（DB 主键 UUID v4；get_messages 读路径填充，新消息构造为 None）
    pub role: String,                          // user / assistant / system
    pub content: String,
    pub sources: Option<Vec<RetrievalResult>>, // 助手消息的引用来源
    pub reasoning: Option<String>,             // 推理思考过程（reasoning_content）
    pub turn_group: Option<String>,            // 轮次分组 ID（编辑版本管理）
    pub version: Option<i32>,                  // 轮次内版本号（从 1 递增）
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
    pub hybrid_search: bool,     // 混合检索开关（REQ-RAG-010；默认 true）
    pub rerank_enabled: bool,    // Cross-Encoder 重排序开关（REQ-RAG-020；默认 false）
    pub hyde_enabled: bool,      // HyDE 查询改写开关（REQ-RAG-021；默认 false）
    pub embedding_model: String, // 嵌入模型标识（REQ-VEC-012；默认 "all-MiniLM-L6-v2"）
}
```

### 2.9.1 SyncResult / WatchedFolderInfo / SyncProgressPayload

```rust
pub struct SyncResult {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

pub struct WatchedFolderInfo {
    pub path: String,
    pub name: String,
    pub sync_status: String,  // idle / stopped
    pub last_synced_at: Option<i64>,
}

pub struct SyncProgressPayload {
    pub phase: String,      // scanning / importing / deleting / complete / error
    pub folder_path: String,
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub skipped: usize,
    pub message: String,
}
```

### 2.10 ConversationTreeNode / ConversationTree（REQ-RAG-039 对话分支树）

```rust
pub struct ConversationTreeNode {
    pub node_id: String,                        // 格式：{turn_group}#{version}
    pub conversation_id: String,
    pub parent_message_id: Option<String>,      // 父节点 ID（根节点为 None）
    pub child_message_ids: Vec<String>,         // 子节点 ID 列表
    pub active_child: Option<String>,           // 当前活跃子节点
    pub created_at: i64,
    pub turn_group: String,
    pub version: i32,
    pub preview: String,                        // 用户消息预览（前 100 字符）
}

pub struct ConversationTree {
    pub conversation_id: String,
    pub nodes: Vec<ConversationTreeNode>,
    pub root_ids: Vec<String>,                  // 根节点 ID 列表
    pub active_path: Vec<String>,               // 活跃路径（根到当前节点的序列）
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

// 本地 LLM 相关（REQ-LLM-003/004）
pub enum LlmMode { Remote, Local }  // 默认 Remote

pub struct ModelInfo {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
    pub architecture: String,   // 从文件名推断（如 qwen2.5）
    pub param_size: String,     // 如 "7B"
    pub quantization: String,   // 如 "Q4_K_M"
}

pub struct ModelDownloadProgressPayload {
    pub filename: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,             // bytes/sec
}

pub struct ModelLoadStatusPayload {
    pub model: String,
    pub status: String,         // loading / ready / error
    pub error: Option<String>,
}

// 采样参数（Phase 2，REQ-LLM-006）
pub struct LlmSamplingParams {
    pub temperature: Option<f64>,       // 0.0~2.0
    pub top_p: Option<f64>,            // 0.0~1.0
    pub top_k: Option<usize>,          // 1~100
    pub max_tokens: Option<usize>,     // >0
    pub frequency_penalty: Option<f32>, // -2.0~2.0
    pub presence_penalty: Option<f32>,  // -2.0~2.0
}

// SettingsPayload 新增字段（Phase 2）
// llm_mode: String           — 推理模式 (remote/local)
// local_model: String        — 选中的模型文件名
// llm_paged_attn: bool       — PagedAttention 开关
// llm_block_size: usize      — KV cache 块大小 (8/16/32)
// llm_gpu_memory_ctx: usize  — GPU 上下文 token 数
// llm_sampling: Option<LlmSamplingParams> — 采样参数
```

---

## 3. AppState（`crates/tauri-app/src/state.rs`）

```rust
pub struct AppState {
    pub storage: SqliteStorage,
    pub data_dir: PathBuf,
    pub llm_config: tokio::sync::RwLock<Option<LlmConfig>>,
    pub is_pro: tokio::sync::RwLock<bool>,
    embedder: tokio::sync::RwLock<Option<LocalEmbedder>>,  // 懒加载 + 可切换（REQ-VEC-012）
    page_renderer: tokio::sync::OnceCell<PdfiumRenderer>,  // 懒加载（REQ-MM-001）
    ocr_engine: tokio::sync::OnceCell<OcrsEngine>,  // 懒加载（REQ-MM-002）
    reranker: tokio::sync::OnceCell<LocalReranker>,  // 懒加载（REQ-RAG-020）
    // vision_provider 不再缓存（REQ-MM-003 前端接线）：每次根据 settings 重新创建，支持运行时开关
    abort_tokens: tokio::sync::Mutex<HashMap<String, CancellationToken>>,
    audit_cancels: tokio::sync::Mutex<HashMap<String, Arc<AtomicBool>>>,  // REQ-AUDIT-005
    file_watchers: tokio::sync::Mutex<HashMap<String, FileWatcherHandle>>,  // REQ-SYNC-003
    // ── QM/OpenCode 借鉴新增字段 ──
    llm_router: LlmRouter,  // Q11 LLM 路由器（模式路由 + fallback + available_modes）
    persist_queue: KeyedQueue<String>,  // Q03 持久化队列（同 conversation_id 串行化写入）
    security_posture: Arc<AtomicU8>,  // Q05 安全态势（0=Dangerous, 1=Auto, 2=Strict）
    shadow_screen_collector: Arc<ShadowScreenCollector>,  // Q06 Shadow 安全筛查统计收集器
    memory_burst_buffer: Arc<Mutex<BurstBuffer>>,  // Q02 延迟批量记忆提取缓冲
    compaction_pending: Arc<AtomicBool>,  // Q04 后台压缩待处理标志
    session_coordinator: Arc<SessionCoordinator>,  // B06 会话运行协调器
    hybrid_search: bool,  // 混合检索开关（settings 表读取）
    llm_mode: LlmMode,  // 推理模式（Remote/Local）
    compression_ratio: f32,  // Prompt 压缩比
    progressive_enabled: bool,  // 渐进式注入开关
    speculative_enabled: bool,  // Speculative RAG 开关
    retrieval_memory_enabled: bool,  // 检索记忆开关
    memory_enabled: bool,  // 持久化记忆开关
    web_search_enabled: bool,  // 网页搜索开关
    _log_guard: Option<WorkerGuard>,  // REQ-OBS-001 日志守卫
}
```

关键方法：
- `AppState::new(data_dir) -> anyhow::Result<Self>` — 初始化（DB + 崩溃恢复 + 配置恢复）
- `embedder() -> anyhow::Result<LocalEmbedder>` — 懒加载，根据 `vec.embedding_model` 选择预设模型（REQ-VEC-012）
- `set_embedding_model(model_str) -> anyhow::Result<()>` — 切换嵌入模型，持久化 + 销毁实例（REQ-VEC-012）
- `embedder_initialized() -> bool` — 检查是否已初始化（async）
- `page_renderer() -> anyhow::Result<&PdfiumRenderer>` — 懒加载 PDF 渲染引擎（REQ-MM-001）
- `ocr_engine() -> anyhow::Result<&OcrsEngine>` — 懒加载 OCR 引擎（REQ-MM-002）
- `vision_provider() -> anyhow::Result<Option<OpenAIVisionProvider>>` — 获取 VLM 引擎（REQ-MM-003；每次根据 `mm.vlm_enabled` 重新创建，支持运行时开关）
- `abort_token_for(conversation_id) -> CancellationToken` — 获取/创建中断令牌
- `abort_chat(conversation_id)` — 触发中断
- `clear_abort(conversation_id)` — 清理令牌
- `audit_cancel_for(doc_id) -> Arc<AtomicBool>` — 获取/创建审计取消标志（REQ-AUDIT-005）
- `abort_audit(doc_id)` — 触发审计取消
- `clear_audit_cancel(doc_id)` — 清理审计取消标志
- `register_file_watcher(path, handle)` — 注册文件监听器句柄（REQ-SYNC-003）
- `unregister_file_watcher(path)` — 注销文件监听器（REQ-SYNC-003）
- `is_watcher_active(path) -> bool` — 检查监听器是否活跃（REQ-SYNC-001 AC-3）

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
- **并行推理会话池（GB 级文档加速）**：内部维护 N 个独立 ONNX 会话（`Vec<Arc<Mutex<TextEmbedding>>>`），
  `embed_batch` 将输入按会话数分片，在 N 个 `spawn_blocking` 任务中并行推理
  - `default_pool_size()` → `available_parallelism().min(8)`（封顶 8，每个实例 ~30MB）
  - `new_with_pool_size(cache_dir, model, progress, pool_size)` — 自定义池大小
  - `shard_texts(texts, pool_size)` — 纯逻辑分片函数（均匀分配，差 ≤ 1）
  - `pool_size()` / `dim()` — 运行时诊断
  - 分片后结果按原始顺序合并，调用方无感知

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

### 4.7 ModelManager（`crates/infra/src/model_manager.rs`）

本地 GGUF 模型文件管理器（REQ-LLM-004）：下载、删除、列表、路径管理。

```rust
pub struct ModelManager { models_dir: PathBuf }

impl ModelManager {
    pub fn new(data_dir: &Path) -> Result<Self>;
    pub fn models_dir(&self) -> &Path;
    pub fn list_models(&self) -> Result<Vec<ModelInfo>>;
    pub fn delete_model(&self, filename: &str) -> Result<()>;  // 含路径穿越防护
    pub fn model_path(&self, filename: &str) -> Result<PathBuf>;
    pub async fn download_model(&self, url: &str, filename: &str, progress: DownloadProgressFn) -> Result<()>;
}

pub type DownloadProgressFn = Box<dyn Fn(u64, u64, u64) + Send + Sync>;

pub const RECOMMENDED_MODELS: &[RecommendedModel];  // 4 个推荐模型
```

- **安全**：`sanitize_filename()` 拒绝含 `/`、`\`、`..` 的文件名
- **下载**：先写入 `.tmp` 文件，完成后原子重命名
- **推荐模型**：Qwen2.5-3B/7B、Llama-3.2-3B、Phi-3.5-mini（Q4_K_M 量化）

### 4.8 LocalLlmEngine（`crates/infra/src/local_llm.rs`，Pro 门控）

基于 mistral.rs v0.9.0 的本地 LLM 推理适配器（REQ-LLM-003），实现 `LLMProvider` trait。

Phase 2 新增能力：GPU 加速（Metal/CUDA）、PagedAttention KV cache、采样参数、模型卸载/切换、流取消、预热。

```rust
pub struct LocalLlmEngine {
    inner: Arc<LocalLlmEngineInner>,  // Arc 包装支持 Clone
}

impl LocalLlmEngine {
    pub fn new(model_path: PathBuf, quantization: Quantization) -> Result<Self>;
    pub fn is_loaded(&self) -> bool;
    pub async fn unload(&self);  // S12: RwLock<Option> 置为 None，释放模型内存
    pub fn device_kind(&self) -> &str;  // S9: 返回 "cpu"/"metal"/"cuda"/"unknown"
    pub fn with_paged_attn(self, block_size: usize, gpu_memory_ctx: usize) -> Self;  // S10: 配置 PagedAttention
    pub async fn sampling_params(&self) -> LlmSamplingParams;  // S11: 读取采样参数
    pub async fn set_sampling_params(&self, params: LlmSamplingParams);  // S11: 运行时修改采样参数
    pub async fn abort(&self);  // S13: 中断推理流（CancellationToken）
    pub async fn warm_up(&self) -> Result<()>;  // S14: 后台预热模型
    pub(crate) async fn get_or_load_model(&self) -> Result<Arc<Model>>;  // double-check locking
}

impl LLMProvider for LocalLlmEngine {
    async fn chat_stream(&self, system_prompt: &str, history: &[ChatMessage], query: &str)
        -> Result<BoxStream<'static, Result<String>>>;
}

// 量化格式枚举（元信息）
pub enum Quantization { Q4K_M, Q5K_M, Q8_0, Auto }
```

- **懒加载**：模型在首次 `chat_stream` 调用时加载（`spawn_blocking`）
- **Pro 门控**：`#[cfg(feature = "pro")]`，Free 版本编译不含 mistral.rs 依赖
- **流式输出**：与 `OpenAIProvider` 完全一致的 `BoxStream<Result<String>>` 接口
- **GPU 加速**：`metal`/`cuda` feature 启用时通过 `best_device(false)` 自动选择设备
- **PagedAttention**：仅 GPU 模式生效，通过 `with_paged_attn()` 配置
- **采样参数**：运行时动态修改，即时生效，无需重新加载模型
- **模型卸载**：`RwLock<Option<Arc<Model>>>` 替代 `OnceCell`，支持运行时卸载和切换
- **流取消**：内部 `CancellationToken`，`abort()` 中断推理，已生成内容保留
- **预热**：`warm_up()` 后台加载模型，模式切换/模型选择后自动触发

### 4.9 Phase 3 自研量化内核（`crates/infra/src/`，Pro 门控）

Phase 3 自研 GEMV（量化矩阵-向量乘法）内核，支持 Q4_0/Q4_K/Q8_0/Q8_K 四种 GGML 量化格式。

```rust
// GGUF 文件解析器（crates/infra/src/gguf_reader.rs）
pub struct GgufFile { mmap: Mmap, metadata: HashMap<String, GgufValue>, tensors: Vec<TensorInfo>, ... }
impl GgufFile {
    pub fn open(path: &Path) -> Result<Self>;  // mmap 映射 + 解析文件头
    pub fn tensor_info(&self, name: &str) -> Option<&TensorInfo>;
    pub fn tensor_data(&self, name: &str) -> Option<&[u8]>;  // 零拷贝
    pub fn mmap(&self) -> &Mmap;  // S23: 暴露 mmap 供 LayerPrefetcher 使用
    pub fn tensor_names(&self) -> impl Iterator<Item = &str>;
}

pub enum GgmlDType { F32, F16, BF16, Q4_0, Q4_1, Q5_0, Q5_1, Q8_0, Q8_1, Q2K, Q3K, Q4K, Q5K, Q6K, Q8K, Unknown(u32) }

// 量化块结构（crates/infra/src/quant_blocks.rs）
pub enum QuantBlock<'a> { Q4_0(&'a [BlockQ4_0]), Q4K(&'a [BlockQ4K]), Q8_0(&'a [BlockQ8_0]), Q8K(&'a [BlockQ8K]), ... }
pub fn blocks_from_bytes(data: &[u8], dtype: GgmlDType) -> Result<QuantBlock<'_>>;  // 零拷贝解析

// GEMV 内核（crates/infra/src/gemv_kernel.rs）
pub fn gemv_dispatch(dtype: GgmlDType, weights: &[u8], input: &[f32], output: &mut [f32], n: usize, k: usize) -> Result<()>;

// 权重重排（crates/infra/src/weight_repack.rs）
pub fn repack_for_gemv(data: &[u8], dtype: GgmlDType, n: usize, k: usize) -> Result<RepackedWeights>;
pub fn gemv_repacked_dispatch(repacked: &RepackedWeights, input: &[f32], output: &mut [f32]) -> Result<()>;

// Layer 级流式预取（crates/infra/src/layer_prefetch.rs）
pub struct LayerPrefetcher { mmap_ptr: SendPtr, layer_offsets: Vec<(String, u64, u64)>, ... }
impl LayerPrefetcher {
    pub fn new(mmap: &Mmap, layer_offsets: Vec<(String, u64, u64)>) -> Self;
    pub fn prefetch_next(&self, layer_idx: usize);  // madvise(MADV_WILLNEED)，非阻塞
    pub fn prefetch_range(&self, start: usize, count: usize);  // 后台线程批量预取
    pub fn stop(&self);  // 停止预取线程
}
pub fn layer_offsets_from_gguf(gguf: &GgufFile) -> Vec<(String, u64, u64)>;

// RAM 预算管理（crates/infra/src/memory_budget.rs）
pub struct MemoryBudget { max_bytes: usize, current_usage: usize, lru_queue: VecDeque<LruEntry>, ... }
impl MemoryBudget {
    pub fn new(max_bytes: usize) -> Self;
    pub fn from_system(percentage: f64) -> Self;  // 基于系统 RAM 百分比
    pub fn allocate(&mut self, layer_id: &str, bytes: usize) -> Result<()>;  // 自动 LRU 驱逐
    pub fn release(&mut self, layer_id: &str);
    pub fn touch(&mut self, layer_id: &str);  // 标记最近使用
    pub fn evict_lru(&mut self, need_bytes: usize) -> Result<()>;  // 空间充足时不驱逐
    pub fn contains_layer(&self, layer_id: &str) -> bool;
    pub fn stats(&self) -> MemoryStats;
}
pub fn get_system_memory() -> usize;  // libc::sysconf，无需 sysinfo

// 内核模式枚举（crates/infra/src/local_llm.rs）
pub enum KernelMode { MistralRs, CustomGemv }
impl KernelMode {
    pub fn from_str(s: &str) -> Result<Self>;  // "mistral" / "custom"
    pub fn as_str(&self) -> &'static str;
}
```

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

**GB 级文档加速（路径 5）**：分块大小可根据文件大小自动调整：
- `DEFAULT_CHUNK_TOKENS = 256`（小文件，精确检索）
- `LARGE_FILE_CHUNK_TOKENS = 1024`（>1MB 文件，减少 chunk 总量 4x）
- `LARGE_FILE_THRESHOLD = 1_000_000`（1MB 阈值）
- `auto_chunk_tokens(file_size)` — 自动选择窗口
- `with_chunk_tokens(storage, data_dir, chunk_tokens)` — 手动指定窗口
- `index_with_section` / `index_with_text` 内部自动检测文件大小并切换窗口

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

### 5.5b PptxLoader（`crates/core/src/loader.rs`）

.pptx 演示文稿加载器（REQ-ING-017），实现 `Loader` 端口。基于 `pptx-to-md` v1.0.0 纯 Rust 解析。

```rust
pub struct PptxLoader;

impl PptxLoader {
    fn extract(file_path: &str) -> anyhow::Result<String>;
}

impl Loader for PptxLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}
```

**加载流程**：
1. `spawn_blocking` 执行 `PptxContainer::open(path, config)` 打开 .pptx ZIP 包
2. `container.convert_to_md()` 将幻灯片转为 Markdown（标题→`#`、要点→`-`、表格→MD 表格、备注→blockquote）
3. 后处理：压缩连续 3+ 换行为双换行（与 DocxLoader / MarkdownLoader 一致）
4. 空文本 bail!

**配置策略**（`ParserConfig::builder()`）：
- `extract_images(false)` — 不提取图片二进制数据（避免 base64 膨胀）
- `include_speaker_notes(true)` — 备注含语义信息，纳入提取
- `include_slide_number_as_comment(false)` — 清理 HTML 注释噪声
- `include_presentation_metadata(false)` — 清理元数据头部

**接入点**：`ImportService.index_inner` 对 `.pptx` 文件调 `PptxLoader.load()` → `index_with_text`。

### 5.5c EpubLoader（`crates/core/src/loader.rs`）

.epub 电子书加载器（REQ-ING-018），实现 `Loader` 端口。基于 `rbook` v0.7.10 纯 Rust 解析（EPUB 2/3）。

```rust
pub struct EpubLoader;

impl EpubLoader {
    fn extract(file_path: &str) -> anyhow::Result<String>;
}

impl Loader for EpubLoader {
    async fn load(&self, file_path: &str) -> anyhow::Result<String>;
}
```

**加载流程**：
1. `spawn_blocking` 执行 `Epub::open(path)` 打开 .epub ZIP 包
2. `epub.spine().iter()` 遍历 spine 条目（阅读顺序）
3. 每个 `spine_entry.manifest_entry().read_str()` 读取 XHTML 内容
4. `html2text::from_read(xhtml.as_bytes(), 80)` 将 XHTML 转为纯文本（与 HtmlLoader 一致）
5. 章节间以 `---` 分隔
6. 后处理：压缩连续 3+ 换行为双换行（与 PptxLoader / HtmlLoader 一致）
7. 空文本 bail!

**配置策略**：
- `rbook` features: `threadsafe`（Send+Sync）+ `prelude`（trait 便捷导入）
- `html2text` 宽度 80 列（与 HtmlLoader 一致）
- HTML 标签（`<b>`/`<i>`/`<a>`/`<img>` 等）被 html2text 自动剥离，纯文本保留

**接入点**：`ImportService.index_inner` 和 `index_multimodal_inner` 对 `.epub` 文件调 `EpubLoader.load()` → `index_with_text`。

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

### 5.7 HybridRetriever（`crates/core/src/hybrid_retriever.rs`）

实现 `Retriever` 端口，向量检索 + 关键词检索 → RRF 融合 → Chunk Expansion（REQ-RAG-010）。
弥补纯向量检索在精确匹配（代码片段、API 名称、专有名词）上的不足。

```rust
pub struct HybridRetriever<E: Embedder, S: Storage> {
    embedder: E,
    storage: S,
    score_threshold: f32,      // DEFAULT_SCORE_THRESHOLD = 0.35
    hybrid_enabled: bool,      // 运行时开关（false 时等价于 VectorRetriever）
}

impl<E: Embedder, S: Storage> HybridRetriever<E, S> {
    pub fn new(embedder: E, storage: S) -> Self;
    pub fn with_threshold(embedder: E, storage: S, score_threshold: f32) -> Self;
    pub fn set_hybrid_enabled(&mut self, enabled: bool);
}

impl<E: Embedder, S: Storage> Retriever for HybridRetriever<E, S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;
}
```

**检索流程**：
1. `embedder.embed(query)` → 查询向量
2. `storage.vector_search(embedding, retrieval_k)` → 向量候选（阈值过滤）
3. （`hybrid_enabled` 为 false 时跳至步骤 6）
4. `storage.keyword_search(query, retrieval_k)` → 关键词候选（FTS5 BM25）
5. `rrf_fuse(vector_hits, keyword_hits, top_k*2)` → RRF 融合排序
6. `expand_neighbors(&storage, &fused)` → Chunk Expansion（共享 §5.6 函数）

**RRF 融合算法**（`rrf_fuse` 函数）：

```rust
pub fn rrf_fuse(
    vector_results: Vec<RetrievalResult>,
    keyword_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult>;
```

- **公式**：`score = Σ weight_i / (rrf_k + rank_i + 1)`，其中 `rrf_k = 60`（Cormack et al. 2009 标准参数）
- **权重**：向量通道 1.0，关键词通道 0.8（向量检索通常更准确）
- **去重**：按 `chunk.id` 去重，重叠 chunk 累积两通道分数获得更高排名
- **过度检索**：`retrieval_k = top_k * 3`，给 RRF 融合更大的候选池

**设计原因**：纯向量检索在精确匹配场景召回不足。LangChain `EnsembleRetriever` 验证了向量+BM25 RRF 融合的有效性。FTS5 `trigram` 分词器支持中英文子串匹配。

### 5.8 SectionAwareSplitter（`crates/core/src/section_aware_splitter.rs`）

实现 `Splitter` 端口，在 SemanticSplitter 前置结构感知层（REQ-VEC-006）。

```rust
pub struct SectionAwareSplitter {
    inner: SemanticSplitter,  // 组合：section 内部委托 SemanticSplitter 切分
}

impl SectionAwareSplitter {
    pub fn default_config() -> Result<Self>;  // 256 tokens / 32 overlap
    pub fn count_tokens(&self, text: &str) -> Result<usize>;
    fn extract_sections(text: &str) -> Vec<Section>;  // pulldown-cmark 标题层级解析
}

impl Splitter for SectionAwareSplitter {
    async fn split(&self, text: &str) -> Result<Vec<String>>;
}
```

**分块策略**（章节感知）：
1. `pulldown-cmark` 解析 Markdown 标题层级（`#`/`##`/`###`）
2. 按 section 切分（标题 + 其下正文），构建层级路径（如 `引言 > 1.1 背景`）
3. 每 section 委托 `SemanticSplitter.split()` 切分
4. 每 chunk 前缀追加 `[章节路径] ` 标记，使 embedding 与 LLM 感知结构位置
5. 无标题的纯文本退化为 `SemanticSplitter` 行为（不加前缀）

**接入点**：`ImportService.index_inner` 对 `.md` 文件直接读取原始 markdown（不经过 MarkdownLoader 剥离 `#`），调 `index_with_section`。

### 5.9 AuditEngine（`crates/core/src/audit.rs`）

文档一致性审计引擎（REQ-AUDIT-001~005），与 `ChatEngine` 平级。走全量扫描路径（`list_chunks`）而非向量检索路径（`vector_search`）。

```rust
pub struct AuditEngine<E: Embedder, S: Storage, L: LLMProvider> {
    embedder: E,
    storage: S,
    llm: L,
}

impl<E: Embedder, S: Storage, L: LLMProvider> AuditEngine<E, S, L> {
    pub fn new(embedder: E, storage: S, llm: L) -> Self;
    pub async fn audit(
        &self,
        doc_id: &str,
        doc_name: &str,
        cancel: AuditCancelFlag,
    ) -> anyhow::Result<AuditOutcome>;
}
```

**三阶段流水线**（Decompose-Then-Verify 范式）：
1. **Decompose**——批量 LLM 调用提取全部 chunk 的原子声明（`Claim`），JSON 解析失败时优雅降级为空列表
2. **Verify**——`embed_batch` 嵌入声明文本 → cosine 相似度预筛（阈值 0.65，SIFiD 式）→ 高风险对 LLM 精判
3. **Report**——聚合为 `AuditReport`（含矛盾清单 + 耗时）

**取消机制**：`AuditCancelFlag = Arc<AtomicBool>`，每阶段间检查，触发后返回 `Cancelled` 含部分报告。

**关键数据模型**：

```rust
pub enum ClaimType { NumericParameter, Definition, Causal, Conclusion, Other }

pub struct Claim {
    pub claim_id: String,
    pub chunk_id: String,
    pub sequence: usize,
    pub text: String,
    pub claim_type: ClaimType,
    pub entities: Vec<String>,
}

pub enum Verdict { Contradiction, Consistent, Unverifiable }

pub enum Severity { High, Medium, Low }

pub struct ContradictionPair {
    pub pair_id: String,
    pub claim_a: Claim,
    pub claim_b: Claim,
    pub verdict: Verdict,
    pub explanation: String,
    pub severity: Severity,
}

pub struct AuditReport {
    pub doc_name: String,
    pub total_chunks: usize,
    pub total_claims: usize,
    pub contradictions: Vec<ContradictionPair>,  // 含所有通过预筛的声明对（verdict 字段区分）
    pub elapsed_ms: u64,
}

pub enum AuditOutcome {
    Completed { report: AuditReport },
    NoChunks,
    Cancelled { partial_report: AuditReport },
}

pub type AuditCancelFlag = Arc<AtomicBool>;
```

**设计说明**：`contradictions` 字段包含所有通过 embedding 预筛的声明对（含 `Consistent` 和 `Unverifiable`），`verdict` 字段标识具体判定结果。IPC 层 `report_to_markdown()` 仅渲染 `verdict == Contradiction` 的对为矛盾清单。

### 5.10 VLM 分级图表理解提示词（`crates/core/src/vlm_prompt.rs`）

集中定义 VLM 图片理解的 4 级精度提取策略提示词（REQ-MM-005），消除 `loader.rs` 与 `import.rs` 中重复的 `VLM_IMAGE_PROMPT` 常量（铁律三）。

```rust
/// VLM 分级图表理解系统提示（REQ-MM-005）。
pub const VLM_TIERED_PROMPT: &str = "...";
```

**4 级精度策略**：

| 等级 | 内容类型 | 提取格式 |
|---|---|---|
| Level 1 | 表格 | 完整 Markdown 表格语法 |
| Level 2 | 流程图 / 甘特图 / 架构图 | Mermaid 语法（flowchart / gantt / graph） |
| Level 3 | 数据图表（散点图 / 柱状图 / 折线图 / 饼图 / 热力图等） | CSV 数据点（坐标轴标签 + 数据系列 + ≥3 数值点）+ 趋势描述 + 误差标注 |
| Level 4 | 示意图 / 照片 / 插图 | 简洁文字描述 |

**附加策略**：
- 数学公式 → LaTeX（行内 `$...$`，行间 `$$...$$`），避免 OCR 乱码
- 纯文字图片 → 直接提取文字内容

**误差标注**（AC-3）：Level 3 数据图表提取结果末尾标注「以下数据由 AI 视觉提取，可能存在误差，请核对原文」，定位为「AI 辅助提取，人类核对原文」。

**调用点**：
- `MultimodalPdfLoader::load()`（`loader.rs`）— `vlm.describe_image(&image_bytes, VLM_TIERED_PROMPT)`
- `ImportService::index_document_multimodal_with_vlm()`（`import.rs`）— `vlm.describe_image(&image_bytes, VLM_TIERED_PROMPT)`

**设计原因**：原 `VLM_IMAGE_PROMPT` 在 `loader.rs` 和 `import.rs` 中重复定义且为单一提示词（表格→MD / 甘特图→Mermaid / 其他→简洁描述），无法提取数据图表中的具体数值。REQ-MM-005 升级为 4 级分级策略并集中定义，使数据图表 CSV 数据进入分块→嵌入→存储链路后可被 RAG 检索（AC-5）。

---

## 6. 后续迭代扩展（v1.4.3~v1.4.7）

### 6.1 License 签发 CLI 工具（`crates/license-issuer`，REQ-LIC-005）

```rust
pub fn generate_keypair() -> (Vec<u8>, Vec<u8>);  // (pubkey_32B, privkey_seed_32B)
pub fn issue_license(payload: &str, privkey_seed: &[u8]) -> Result<String>;  // base64(payload)-base64(sig)
pub fn save_keypair_to_dir(pubkey: &[u8], privkey: &[u8], dir: &Path) -> Result<(PathBuf, PathBuf)>;
pub fn load_privkey(path: &Path) -> Result<Vec<u8>>;
pub fn load_pubkey_from_b64(b64: &str) -> Result<VerifyingKey>;
```

### 6.2 HNSW 向量索引（`crates/infra/src/hnsw_index.rs`，REQ-NFR-005）

```rust
pub struct HnswIndex { /* Hnsw<'static, f32, DistCosine> + id_map + vectors */ }

impl HnswIndex {
    pub fn build(vectors: &[(String, Vec<f32>)]) -> Result<Self>;  // 批量构建
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<(String, f32)>;  // 查询 top-k
    pub fn save(&self, path: &Path) -> Result<()>;  // JSON 持久化
    pub fn load(path: &Path) -> Result<Self>;  // 重建索引
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

参数：`max_nb_connection=32` / `max_layer=16` / `ef_construction=400` / `ef_search=64`（Malkov & Yashunin 论文推荐值）

### 6.3 SqliteStorage HNSW 支持方法（F1-2）

```rust
impl SqliteStorage {
    pub async fn load_all_embeddings(&self) -> Result<Vec<(String, Vec<f32>)>>;
    pub async fn get_chunks_by_ids(&self, ids: &[String]) -> Result<Vec<RetrievalResult>>;
}
```

### 6.4 EmbeddingModel 多模型预设 + 自定义模型（E3-1 + REQ-VEC-014）

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbeddingModel {
    AllMiniLML6V2,   // 384 维（默认）
    BgeSmallZhV1_5,  // 512 维（中文优化）
    E5SmallV2,       // 384 维（多语言）
    Custom(String),   // 自定义 ONNX 模型（REQ-VEC-014，Pro 门控；dim 动态检测）
}

impl LocalEmbedder {
    pub async fn new(cache_dir: PathBuf) -> Result<Self>;  // 向后兼容，使用默认模型
    pub async fn new_with_model(cache_dir: PathBuf, model: EmbeddingModel) -> Result<Self>;
    /// 从本地文件加载用户自定义 ONNX 嵌入模型（REQ-VEC-014，Pro 门控）。
    /// 使用 fastembed UserDefinedEmbeddingModel API，首次推理检测维度。
    pub async fn new_with_custom_model(model_dir: PathBuf) -> Result<Self>;
    /// 查找目录中的 ONNX 文件（model.onnx / model_quantized.onnx）。
    pub fn find_onnx_file(model_dir: &Path) -> Option<&'static str>;
    /// 验证 ONNX 文件有效性（文件存在 + 大小 ≥ 1KB）。
    pub fn validate_onnx_file(path: &Path) -> Result<()>;
    /// 验证 tokenizer 文件存在性，返回缺失文件列表。
    pub fn validate_tokenizer_files(model_dir: &Path) -> Vec<String>;
    /// 列出所有已上传的自定义模型。
    pub fn list_custom_models(custom_models_dir: &Path) -> Vec<CustomModelInfo>;
    /// 删除自定义模型目录，返回删除的文件数。
    pub fn delete_custom_model(custom_models_dir: &Path, name: &str) -> u64;
    /// 复制用户选择的模型文件到 custom_models 目录。
    pub fn copy_custom_model_files(
        custom_models_dir: &Path,
        name: &str,
        onnx_path: &Path,
        tokenizer_files: &[String],
    ) -> Result<PathBuf>;
    /// 是否为自定义模型。
    pub fn is_custom(&self) -> bool;
}

/// 自定义嵌入模型信息（REQ-VEC-014，由 list_custom_models IPC 命令返回）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomModelInfo {
    pub name: String,        // 模型名称
    pub dim: usize,          // 嵌入维度（0 = 未检测）
    pub size_bytes: u64,     // 模型文件总大小（字节）
    pub is_valid: bool,      // 文件完整性（ONNX 存在 + tokenizer 齐全）
}
```

### 6.5 论文审稿扩展（A1-1）

```rust
#[derive(Default, Serialize, Deserialize)]
pub enum AuditMode { #[default] ConsistencyCheck, PaperReview }

pub enum ReviewDimension { Methodology, Statistics, Logic, Reproducibility }

pub struct ReviewFinding { dimension, issue, chunk_sequence, recommendation, severity }
pub struct PaperReviewReport { doc_name, total_chunks, findings, overall_assessment, elapsed_ms }

pub fn build_paper_review_prompt(chunks: &[Chunk]) -> String;
```

### 6.6 模型下载管理（REQ-VEC-008）

```rust
// infra 层
pub type DownloadProgressFn = Arc<dyn Fn(DownloadEvent) + Send + Sync>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadEvent {
    Downloading { file_name: String, current: u64, total: u64 },
    Loading,
    Done,
    Error { message: String },
}

pub struct ModelCacheInfo { total_size_bytes: u64, models: Vec<ModelEntry> }
pub struct ModelEntry { name: String, size_bytes: u64 }

impl LocalEmbedder {
    pub async fn new_with_progress(cache_dir, model, progress: Option<DownloadProgressFn>) -> Result<Self>;
    pub fn get_cache_info(cache_dir: &Path) -> ModelCacheInfo;  // 静态方法
    pub fn clear_cache(cache_dir: &Path, model_name: Option<&str>) -> u64;  // 静态方法
}

// tauri-app 层
impl AppState {
    pub fn embedder_initialized(&self) -> bool;
    pub async fn init_embedder_with_progress(&self, progress: DownloadProgressFn) -> Result<()>;
    pub async fn set_embedding_model(&self, model_str: &str) -> Result<()>;  // REQ-VEC-012
}

// IPC 命令（28 个命令）
#[tauri::command] pub async fn init_embedder(app: AppHandle, state) -> Result<(), String>;
#[tauri::command] pub async fn get_model_cache_info(state) -> Result<ModelCacheInfo, String>;
#[tauri::command] pub async fn clear_model_cache(model_name: Option<String>, state) -> Result<u64, String>;
#[tauri::command] pub async fn set_embedding_model(model: String, state) -> Result<(), String>;  // REQ-VEC-012

// 事件
// model_download_progress payload: DownloadEvent（serde JSON）
```

---

## 7. 前端原子契约与错误前缀系统（SRS v2.1.0 同步）

> 本节记录 SRS v2.1.0 原子化重构引入的前端契约规格与错误前缀系统规划。
> 详细规格见 SRS §4.4（UI）/ §4.14（DS）/ §4.15（IX）/ §4.21（ERR）。

### 7.1 前端关键函数契约（已实现）

以下函数已在 `ui/index.html` 中实现，SRS v2.1.0 已将其原子化为规格文档：

| 函数 | 签名 | SRS 引用 | 状态 |
|------|------|----------|------|
| `setInputState(state, hint)` | `(state: 'idle'\|'streaming'\|'error', hint: string) => void` | REQ-UI-003 | 已实现 |
| `send()` | `() => void`（8 步流程：读取→清空→appendUserBlock→history.push→appendBlock→setInputState→invoke→catch） | REQ-UI-003 | 已实现 |
| `sanitizeError(err)` | `(err: string\|Error) => string`（API Key `sk-****` + Unix 路径 `/Users/****/` + Windows 路径 `\Users\****\`） | REQ-UI-005 / REQ-ERR-001 | 已实现 |
| `toastError(err)` | `(err: string\|Error) => void`（= `toast(sanitizeError(err), 'error')`） | REQ-UI-005 / REQ-ERR-001 | 已实现 |
| `toastSuccess(message)` | `(message: string) => void`（= `toast(message, 'success')`） | REQ-UI-005 | 已实现 |
| `showPaywall(reason)` | `(reason: string) => void`（`LIMIT_REACHED` / `PRO_REQUIRED` 前缀触发） | REQ-LIC-003 / REQ-ERR-001 | 已实现 |
| `importPaths(paths)` | `(paths: string[]) => void`（catch 块检查 `startsWith('LIMIT_REACHED')` / `startsWith('PRO_REQUIRED')`） | REQ-ING-001 / REQ-ERR-001 | 已实现 |
| `scheduleRender()` | `() => void`（rAF 节流 + `renderScheduled` 标志位防抖） | REQ-UI-002 / REQ-DS-006 | 已实现 |

### 7.2 错误前缀系统（已实现，REQ-ERR-001 Done）

SRS v1.0 §4.21 定义了 10 类结构化错误前缀，用于前端统一处理。全部前缀已在 `crates/core/src/errors.rs` 中实现。

#### 7.2.1 错误前缀常量定义

```rust
// crates/core/src/errors.rs
pub const ERR_LIMIT_REACHED: &str = "LIMIT_REACHED";
pub const ERR_PRO_REQUIRED: &str = "PRO_REQUIRED";
pub const ERR_NETWORK: &str = "NETWORK";
pub const ERR_AUTH: &str = "AUTH";
pub const ERR_PARSE: &str = "PARSE";
pub const ERR_EMBED: &str = "EMBED";
pub const ERR_LLM: &str = "LLM";
pub const ERR_STORAGE: &str = "STORAGE";
pub const ERR_VALIDATION: &str = "VALIDATION";
pub const ERR_UNKNOWN: &str = "UNKNOWN";

pub const MAX_QUERY_LENGTH: usize = 8000;
pub const MAX_API_KEY_LENGTH: usize = 256;
```

#### 7.2.2 分类辅助函数

```rust
pub fn prefix_error(prefix: &str, detail: &str) -> String  // 格式化 "{PREFIX}: {detail}"
pub fn classify_llm_error(err: &str) -> String  // 自动分类 NETWORK/AUTH/LLM
pub fn has_error_prefix(err: &str) -> bool  // 判断是否已携带前缀
```

#### 7.2.3 前缀实现位置与前端处理

| 前缀 | 错误类型 | 后端实现位置 | 前端处理 |
|------|----------|---------------------|----------|
| `LIMIT_REACHED:` | 配额超限 | `import.rs` / `commands.rs` | `showPaywall(reason)` |
| `PRO_REQUIRED:` | Pro 专属限制 | `import.rs` / `commands.rs` | `showPaywall(reason)` |
| `NETWORK:` | 网络错误 | `classify_llm_error()` / `chat_inner()` | `toastError(err)` + `setInputState('error')` |
| `AUTH:` | 鉴权失败 | `classify_llm_error()` / `chat_inner()` | `toastError(err)` + `setInputState('error')` |
| `PARSE:` | 文件解析失败 | `import_files_inner()` emit_status | `toastError(p.message)` via `doc-status-changed` |
| `EMBED:` | 嵌入失败 | `import_files_inner()` emit_status | `toastError(p.message)` via `doc-status-changed` |
| `LLM:` | LLM 调用失败 | `classify_llm_error()` / `chat_inner()` | `toastError(err)` + `finalizeStream(false)` |
| `STORAGE:` | 存储错误 | `import_files_inner()` catch | `toastError(err)` |
| `VALIDATION:` | 输入校验 | `chat_inner()` / `update_llm_config_inner()` | `toast(detail, 'warning')` |
| `UNKNOWN:` | 未知错误 | `chat` 命令兑底包装 | `toastError(err)` |

### 7.3 DS/IX/ERR 域前端契约规划（Draft）

以下函数/行为在 SRS v2.1.0 中已规格化但尚未实现，待开发后补全契约：

| 函数 | 签名（规划） | SRS 引用 | 状态 |
|------|------|----------|------|
| `showConfirmDialog(title, body, confirmText, onConfirm)` | `(title: string, body: string, confirmText: string, onConfirm: () => void) => void` | REQ-IX-005 | Draft |
| `showSkeleton(container, count)` | `(container: HTMLElement, count?: number) => void` | REQ-IX-007 | Draft |
| `hideSkeleton()` | `() => void` | REQ-IX-007 | Draft |
| `setOfflineMode(offline)` | `(offline: boolean) => void` | REQ-ERR-003 | Draft |
| `showContextMenu(event, items)` | `(event: MouseEvent, items: MenuItem[]) => void` | REQ-IX-001 | Draft |
| `reorderConversations(draggedId, targetId)` | `(draggedId: string, targetId: string) => void` | REQ-IX-002 | Draft |

### 7.4 IPC 命令规划（Draft）

| 命令 | 签名（规划） | SRS 引用 | 状态 |
|------|------|----------|------|
| `reorder_conversations` | `{ items: Vec<{ id: String, sort_order: i64 }> } -> Result<(), String>` | REQ-IX-002 | Draft |

### 7.5 数据模型扩展规划（Draft）

| 模型 | 字段扩展 | SRS 引用 | 状态 |
|------|----------|----------|------|
| `Conversation` | `sort_order: i32`（默认 0，用户拖拽后设置） | REQ-IX-002 | Draft |
| `ChatPhasePayload` | `phase` 新增 `retrying` 值（REQ-ERR-002 网络重试阶段） | REQ-ERR-002 | Draft |

---

## 8. 极致性能优化契约（REQ-PERF-001~012）

### 8.1 ResponseCache trait（REQ-PERF-001）

```rust
pub trait ResponseCache: Send + Sync {
    async fn get_exact(&self, query_hash: &str) -> anyhow::Result<Option<CacheEntry>>;
    async fn get_semantic(&self, query_embedding: &[f32], threshold: f32) -> anyhow::Result<Option<CacheEntry>>;
    async fn get_retrieval(&self, query_hash: &str) -> anyhow::Result<Option<Vec<RetrievalResult>>>;
    async fn put_exact(&self, query_hash: &str, entry: CacheEntry) -> anyhow::Result<()>;
    async fn put_semantic(&self, query_embedding: &[f32], entry: CacheEntry) -> anyhow::Result<()>;
    async fn put_retrieval(&self, query_hash: &str, results: &[RetrievalResult]) -> anyhow::Result<()>;
    async fn clear_all(&self) -> anyhow::Result<()>;
    async fn get_stats(&self) -> anyhow::Result<CacheStats>;
}
```

- **实现**：`SqliteCache`（`crates/infra/src/sqlite_cache.rs`，3 张表：exact_cache / semantic_cache / retrieval_cache）
- **辅助函数**：`normalize_query()` / `query_hash()` / `cosine_similarity()` / `is_expired()` / `estimate_rag_token_cost()`

### 8.2 PromptCompressor trait（REQ-PERF-002）

```rust
pub trait PromptCompressor: Send + Sync {
    fn compress<'a>(&'a self, text: &'a str, ratio: f32, query: &'a str)
        -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'a>>;
    fn compress_batch<'a>(&'a self, chunks: &'a [String], ratio: f32, query: &'a str)
        -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + 'a>>;
}
```

- **实现**：`RuleBasedCompressor`（Free，`crates/core/src/prompt_compressor.rs`）+ `OnnxCompressor`（Pro，`crates/infra/src/onnx_compressor.rs`）+ `NoCompressor`（占位）

### 8.3 ProgressiveInjector（REQ-PERF-010）

```rust
pub struct ProgressiveInjector { /* ... */ }
impl ProgressiveInjector {
    pub fn new(config: ProgressiveConfig, total_sources: usize) -> Self;
    pub fn with_defaults(total_sources: usize) -> Self;
    pub fn initial_indices(&self) -> Vec<usize>;
    pub fn needs_more_info(&self, text: &str) -> bool;
    pub fn can_append(&self) -> bool;
    pub fn next_batch(&mut self) -> Vec<usize>;
    pub fn injected_count(&self) -> usize;
    pub fn append_rounds(&self) -> usize;
}
pub fn detect_insufficient_info(text: &str) -> bool;
```

### 8.4 SpeculativeRagEngine（REQ-PERF-011）

```rust
pub struct SpeculativeRagEngine<D: LLMProvider, L: LLMProvider> { /* ... */ }
impl<D: LLMProvider, L: LLMProvider> SpeculativeRagEngine<D, L> {
    pub fn new(draft_llm: D, verify_llm: L) -> Self;
    pub fn with_config(mut self, config: SpeculativeRagConfig) -> Self;
    pub async fn speculate(&self, query: &str, sources: &[RetrievalResult]) -> anyhow::Result<SpeculativeOutcome>;
}
pub struct SpeculativeRagConfig { pub draft_top_k: usize, pub min_draft_length: usize, pub accept_threshold: f32 }
pub enum SpeculativeOutcome { DraftAccepted(BoxStream<...>), DraftCorrected(BoxStream<...>), FallbackDirect(BoxStream<...>) }
pub fn text_similarity(a: &str, b: &str) -> f32;
```

### 8.5 RetrievalMemoryEngine（REQ-PERF-012）

```rust
pub struct RetrievalMemoryEngine<S: RetrievalMemoryStore> { /* ... */ }
impl<S: RetrievalMemoryStore> RetrievalMemoryEngine<S> {
    pub fn new(store: S) -> Self;
    pub async fn record_retrieval(&self, query: &str, method: RetrievalMethod, results: &[RetrievalResult]) -> anyhow::Result<()>;
    pub async fn select_method(&self, query: &str) -> anyhow::Result<RetrievalMethod>;
    pub async fn reset_all(&self) -> anyhow::Result<()>;
}
pub trait RetrievalMemoryStore: Send + Sync {
    async fn get_memory(&self, query_type: QueryType, method: RetrievalMethod) -> anyhow::Result<Option<MemoryRecord>>;
    async fn upsert_memory(&self, record: MemoryRecord) -> anyhow::Result<()>;
    async fn list_memories(&self, query_type: QueryType) -> anyhow::Result<Vec<MemoryRecord>>;
    async fn list_all_memories(&self) -> anyhow::Result<Vec<MemoryRecord>>;
    async fn clear_all_memories(&self) -> anyhow::Result<()>;
}
pub enum QueryType { Factual, Analytical, Code, Conversational }
pub enum RetrievalMethod { VectorOnly, Hybrid, HybridRerank, Proposition, Colbert }
pub struct MemoryRecord { pub query_type: QueryType, pub method: RetrievalMethod, pub hit_count: u32, pub miss_count: u32, pub avg_score: f32 }
pub fn classify_query_type(query: &str) -> QueryType;

// REQ-PERF-012 扩展：用户反馈自学习（v3.4.0）
pub enum FeedbackType { RetryWithDifferentMethod, EditAndResend, Accepted, ThumbsUp, ThumbsDown }
impl FeedbackType {
    pub fn delta(&self) -> f32;  // -0.10 / -0.15 / +0.05 / +0.20 / -0.25
    pub fn is_positive(&self) -> bool;
}
pub struct FeedbackSignal { pub query: String, pub method: RetrievalMethod, pub feedback: FeedbackType, pub timestamp: i64 }
impl FeedbackSignal { pub fn new(query: String, method: RetrievalMethod, feedback: FeedbackType) -> Self; }
// RetrievalMemoryEngine 新增方法：
impl<S: RetrievalMemoryStore> RetrievalMemoryEngine<S> {
    pub async fn record_feedback(&self, signal: FeedbackSignal) -> anyhow::Result<()>;
}
```

### 8.6 PropositionSplitter（REQ-PERF-007）

```rust
pub struct PropositionSplitter;
impl PropositionSplitter {
    pub fn split(&self, chunk: &Chunk) -> Vec<Proposition>;
}
pub struct Proposition { pub id: String, pub chunk_id: String, pub content: String, pub sequence: usize }
```

### 8.7 SummaryTreeBuilder（REQ-PERF-009）

```rust
pub struct SummaryTreeBuilder { /* ... */ }
impl SummaryTreeBuilder {
    pub fn new() -> Self;
    pub async fn build(&self, chunks: &[Chunk], summarize_fn: &dyn Fn(&str) -> Pin<Box<dyn Future<Output = String> + Send>>) -> anyhow::Result<Vec<SummaryNode>>;
}
pub struct SummaryNode { pub id: String, pub doc_id: String, pub level: u32, pub content: String, pub child_ids: Vec<String> }
pub fn classify_query_level(query: &str) -> u32;
```

### 8.8 EntityExtractor（REQ-PERF-006 + REQ-RAG-027）

```rust
pub struct EntityExtractor;
impl EntityExtractor {
    pub fn extract(text: &str) -> Vec<Entity>;
    pub fn extract_with_chunk_id(text: &str, chunk_id: &str) -> Vec<(String, String, String)>;
    /// 实体关系抽取（REQ-RAG-026 知识图谱）：纯规则模式匹配，零 LLM
    pub fn extract_relations(text: &str, chunk_id: &str) -> Vec<EntityRelation>;
}
```

#### 8.8.1 EntityRelation / GraphTriple（REQ-RAG-027）

```rust
/// 实体间关系（知识图谱边）
pub struct EntityRelation {
    pub id: String,           // UUID v4
    pub subject: String,      // 主体实体文本
    pub relation_type: String, // defined_as / part_of / depends_on / uses / implements / extends / references / related_to
    pub object: String,       // 客体实体文本
    pub chunk_id: String,     // 来源 chunk ID
    pub confidence: f32,      // 规则抽取 0.5-1.0
}
impl EntityRelation {
    pub fn new(subject: String, relation_type: String, object: String, chunk_id: String, confidence: f32) -> Self;
    pub fn to_triple(&self) -> GraphTriple;
}

/// 图谱三元组（用于检索结果展示）
pub struct GraphTriple {
    pub subject: String,
    pub relation: String,
    pub object: String,
}
```

#### 8.8.2 Storage trait 实体关系方法（REQ-RAG-027）

```rust
pub trait Storage: Send + Sync {
    // ... 既有方法 ...

    /// 写入单条实体关系
    async fn add_relation(&self, relation: &EntityRelation) -> anyhow::Result<()>;
    /// 批量写入实体关系
    async fn add_relations_batch(&self, relations: &[EntityRelation]) -> anyhow::Result<()>;
    /// 查询指定实体参与的所有关系（图遍历，subject 或 object 匹配）
    async fn get_relations_for_entity(&self, entity_text: &str) -> anyhow::Result<Vec<EntityRelation>>;
    /// 查询指定 chunk 的所有关系
    async fn get_relations_for_chunk(&self, chunk_id: &str) -> anyhow::Result<Vec<EntityRelation>>;
    /// 按主体 + 关系类型查询
    async fn search_by_relation(&self, subject: &str, relation_type: &str) -> anyhow::Result<Vec<EntityRelation>>;
    /// 按 chunk ID 查找单个 chunk（REQ-RAG-027 图遍历检索用）
    async fn get_chunk_by_id(&self, chunk_id: &str) -> anyhow::Result<Option<Chunk>>;
    /// 分页查询全部实体关系（REQ-RAG-027 前端图谱可视化用）
    async fn list_all_relations(&self, limit: usize, offset: usize) -> anyhow::Result<Vec<EntityRelation>>;
    /// 统计实体关系总数（REQ-RAG-027 前端图谱可视化用）
    async fn count_relations(&self) -> anyhow::Result<usize>;
    /// 批量查询实体类型（REQ-RAG-027 前端图谱可视化增强）
    async fn get_entity_types(&self, entities: &[String]) -> anyhow::Result<HashMap<String, String>>;
    /// 获取全量实体邻接表（REQ-RAG-027 Session 5 高级分析用）
    async fn get_entity_graph(&self) -> anyhow::Result<HashMap<String, Vec<String>>>;
}
```

### 8.8.3 GraphAnalyzer 图分析引擎（REQ-RAG-027 Session 5）

```rust
/// 图分析引擎：最短路径 + 社区检测 + 度中心性。
///
/// 接收邻接表 HashMap<String, Vec<String>> 作为输入，不依赖具体 Storage 实现。
/// 纯 Rust 实现，零外部依赖。
/// 空图降级：所有分析返回空结果不报错。
pub struct GraphAnalyzer;

impl GraphAnalyzer {
    /// BFS 最短路径分析（AC-32~AC-33）
    pub fn shortest_path(adjacency: &HashMap<String, Vec<String>>, from: &str, to: &str) -> Vec<String>;
    /// 所有路径分析（DFS，限制最大深度 5 防止指数爆炸）
    pub fn all_paths(adjacency: &HashMap<String, Vec<String>>, from: &str, to: &str, max_depth: usize) -> Vec<Vec<String>>;
    /// Label Propagation 社区检测（AC-38~AC-39，迭代直到收敛或最大 10 轮）
    pub fn detect_communities(adjacency: &HashMap<String, Vec<String>>) -> HashMap<String, usize>;
    /// 度中心性分析（AC-37，degree / (n-1)）
    pub fn degree_centrality(adjacency: &HashMap<String, Vec<String>>) -> HashMap<String, f32>;
}
```

### 8.9 Wiki 双向链接（REQ-ING-020）

#### 8.9.1 WikiLink 结构体

```rust
/// Wiki 双向链接（REQ-ING-020 Markdown 笔记双向链接，Obsidian 风格）。
///
/// 表示 Markdown 文档中 `[[link-target]]` 语法引用的链接关系。
/// 导入时自动解析并建立索引，支持正向链接和反向链接查询。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiLink {
    pub id: String,           // UUID v4
    pub source_doc_id: String, // 源文档 ID（包含 [[link]] 的文档）
    pub target: String,        // 目标 wiki-link 文本
    pub chunk_id: String,      // 来源 chunk ID
    pub created_at: i64,        // Unix 秒级时间戳
}

impl WikiLink {
    pub fn new(source_doc_id: String, target: String, chunk_id: String) -> Self;
}
```

#### 8.9.2 parse_wiki_links 解析函数

```rust
/// 解析文本中的所有 wiki-link，返回 WikiLink 列表（纯函数，零 LLM）。
///
/// 支持语法：
/// - `[[target]]` — 基本链接
/// - `[[target|alias]]` — 别名链接
/// - `[[target#heading]]` — 标题引用
/// - `[[target#heading|alias]]` — 标题+别名
///
/// 跳过代码块（``` / ~~~）和行内代码中的 wiki-link。
/// 同一 chunk 内重复 target 去重。
pub fn parse_wiki_links(text: &str, source_doc_id: &str, chunk_id: &str) -> Vec<WikiLink>;
```

#### 8.9.3 Storage trait Wiki-links 方法

```rust
/// 批量写入 wiki-link 索引（REQ-ING-020）。
async fn add_wiki_links(&self, links: &[WikiLink]) -> anyhow::Result<()>;

/// 查询文档的正向链接（REQ-ING-020）。
/// WHERE source_doc_id = ? → 该文档引用了哪些文档
async fn get_forward_links(&self, doc_id: &str) -> anyhow::Result<Vec<WikiLink>>;

/// 查询文档的反向链接（REQ-ING-020）。
/// WHERE target LIKE '%doc_name%' → 哪些文档引用了该文档
async fn get_backlinks(&self, doc_name: &str) -> anyhow::Result<Vec<WikiLink>>;
```

### 8.8.4 GraphPath / GraphCommunity 数据模型（REQ-RAG-027 Session 5）

```rust
/// 知识图谱路径分析结果
pub struct GraphPath {
    pub path: Vec<String>,   // 路径上的实体节点列表（含起点和终点）
    pub hops: usize,          // 路径跳数（path.len() - 1）
}

/// 知识图谱社区检测结果
pub struct GraphCommunity {
    pub communities: HashMap<String, usize>, // 实体 → 社区 ID 映射
    pub community_count: usize,               // 社区总数
}
```

### 8.8.3 GraphRetriever 图遍历检索器（REQ-RAG-027）

```rust
/// 知识图谱图遍历检索器：沿实体关系图边扩展到关联 chunk。
///
/// 图扩展作为 RRF 融合的第四路检索通道（Vector+BM25+Entity+Graph）。
/// BFS 遍历最大 2 跳，置信度×距离衰减加权排序。
pub struct GraphRetriever<S: Storage> {
    storage: S,
    max_depth: usize,
    relation_filter: Option<Vec<String>>,
}

impl<S: Storage> GraphRetriever<S> {
    /// 创建图遍历检索器，默认最大深度 2 跳
    pub fn new(storage: S) -> Self;
    /// 自定义最大深度
    pub fn with_max_depth(storage: S, max_depth: usize) -> Self;
    /// 仅沿指定关系类型扩展
    pub fn with_relation_filter(storage: S, relation_types: Vec<String>) -> Self;
    /// 设置关系类型过滤器（运行时切换）
    pub fn set_relation_filter(&mut self, relation_types: Option<Vec<String>>);
    /// 从查询文本抽取实体，沿图边扩展到关联 chunk
    pub async fn expand(&self, query: &str, top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;
    /// 使用预抽取的实体进行图扩展
    pub async fn expand_with_entities(&self, entities: &[String], top_k: usize) -> anyhow::Result<Vec<RetrievalResult>>;
}

/// 四路 RRF 融合（Vector + BM25 + Entity + Graph）
pub fn rrf_fuse_four_way(
    vector_results: Vec<RetrievalResult>,
    keyword_results: Vec<RetrievalResult>,
    entity_results: Vec<RetrievalResult>,
    graph_results: Vec<RetrievalResult>,
    top_k: usize,
) -> Vec<RetrievalResult>;
```

**常量**：
- `GRAPH_WEIGHT = 0.5`（图扩展通道权重，低于 Entity 0.6）
- `DEFAULT_MAX_DEPTH = 2`（默认图遍历最大深度）
- `DISTANCE_DECAY = 0.5`（距离衰减因子：1 跳 ×1.0，2 跳 ×0.5）

### 8.9 MultiVectorEmbedder trait（REQ-PERF-008, Pro）

```rust
pub trait MultiVectorEmbedder: Send + Sync {
    async fn embed_multi(&self, text: &str) -> anyhow::Result<Vec<Vec<f32>>>;
}
pub fn maxsim(query_vectors: &[Vec<f32>], doc_vectors: &[Vec<f32>]) -> f32;
pub fn multivec_to_bytes(vectors: &[Vec<f32>]) -> Vec<u8>;
pub fn multivec_from_bytes(bytes: &[u8]) -> Vec<Vec<f32>>;
```

### 8.10 QualityGate 质量门控（REQ-RAG-028）

```rust
/// 质量门控配置。
pub struct GateConfig {
    pub threshold: f32,           // 通过阈值（默认 0.6）
    pub weight_coverage: f32,     // 检索覆盖率权重（默认 0.4）
    pub weight_diversity: f32,    // 来源多样性权重（默认 0.3）
    pub weight_score: f32,        // 分数方差权重（默认 0.3）
    pub degradation: DegradationStrategy,
}

/// 降级策略。
pub enum DegradationStrategy {
    ExpandTopK { factor: f32, max_retry: usize },
    WarnUser,
    PassThrough,
}

/// 质量评分结果。
pub struct GateScore {
    pub coverage: f32,        // 检索覆盖率（top-1 分数归一化）
    pub diversity: f32,       // 来源多样性（不同文档占比）
    pub score_variance: f32,  // 分数方差（标准差）
    pub weighted: f32,        // 加权总分
    pub passed: bool,        // 是否通过门控
}

/// 默认通过阈值。
pub const GATE_PASS_THRESHOLD: f32 = 0.6;

/// 评估检索结果质量。
pub fn evaluate(results: &[RetrievalResult], config: &GateConfig) -> GateScore;
```

**ChatEngine 集成**：
- `ChatEngine::with_quality_gate(config: GateConfig) -> Self` — builder 方法启用质量门控
- `ChatEngine::gate_score() -> Option<&GateScore>` — 获取最近一次评分结果
- `ChatOutcome::Answered { gate_score: Option<GateScore> }` — 评分结果通过返回值传递

**评分公式**：
- `coverage = results[0].score / 0.3`，clamp 到 `[0, 2.0]`
- `diversity = unique_doc_count / results.len()`
- `score_variance = stddev(scores)`
- `weighted = coverage * 0.4 + diversity * 0.3 + score_variance * 0.3`
- `passed = weighted >= threshold`

### 8.11 AgentHook 生命周期 Hooks（REQ-RAG-029）

**HookPhase 枚举**（`crates/core/src/hooks.rs`）：

```rust
pub enum HookPhase {
    BeforeRetrieval,    // 检索前：可修改查询文本
    AfterRetrieval,     // 检索后：可修改/过滤检索结果
    BeforeGeneration,   // LLM 生成前：可修改 prompt 上下文
    AfterGeneration,    // LLM 生成后：记录用（流式输出不可修改）
}
```

- `HookPhase::as_str() -> &'static str` — 返回阶段字符串标识（`"before_retrieval"` 等）

**HookContext 结构体**（`crates/core/src/hooks.rs`）：

```rust
pub struct HookContext {
    pub conversation_id: String,
    pub query: String,                    // BeforeRetrieval 阶段可被修改
    pub current_phase: HookPhase,
    pub retrieval_results: Vec<RetrievalResult>,  // AfterRetrieval / BeforeGeneration 阶段可被修改
    pub output: Option<String>,           // AfterGeneration 阶段有值（流式场景为 None）
    pub metadata: serde_json::Value,      // 自由元数据（hook 间传递任意数据）
}
```

- `HookContext::new(conversation_id: String, query: String, phase: HookPhase) -> Self`

**AgentHook trait**（`crates/core/src/hooks.rs`）：

```rust
pub trait AgentHook: Send + Sync {
    fn name(&self) -> &str;
    fn phase(&self) -> HookPhase;
    fn execute<'a>(
        &'a self,
        ctx: &'a mut HookContext,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
    fn enabled(&self) -> bool { true }  // 默认启用
}
```

- 使用 `Pin<Box<Future>>` 返回类型保证对象安全（dyn-compatible），与 `Reranker`/`QueryRewriter`/`PromptCompressor` 一致
- `execute` 返回 `Err` 时中断后续 hook 执行（错误冒泡到引擎）

**HookRegistry 结构体**（`crates/core/src/hooks.rs`）：

```rust
pub struct HookRegistry {
    hooks: Vec<Box<dyn AgentHook>>,
}
```

- `HookRegistry::new() -> Self` — 创建空注册表
- `HookRegistry::register(&mut self, hook: Box<dyn AgentHook>)` — 注册 hook（按注册顺序执行）
- `HookRegistry::run_phase(&self, phase: HookPhase, ctx: &mut HookContext) -> Result<()>` — 执行指定阶段的已启用 hook（async）
- `HookRegistry::count() -> usize` — 已注册 hook 数量
- `HookRegistry::is_empty() -> bool` — 是否为空
- 实现 `Default` + `Debug`

**AgentEngine 集成**（`crates/core/src/agent.rs`）：
- `AgentEngine.hooks: Option<Arc<HookRegistry>>` — `None` = 无 hook，行为与之前完全一致
- `AgentEngine::with_hooks(self, registry: HookRegistry) -> Self` — builder 方法注册 hooks

**4 处 Hook 调用点（AgentEngine::run）**：
1. **BeforeRetrieval** — `run()` 方法开头，构建 prompt 前。修改 `ctx.query` → 后续使用 `hook_query`
2. **AfterRetrieval** — `execute_tool_single` / `execute_tools_parallel` 返回后。修改 `ctx.retrieval_results` → 过滤/增强检索结果
3. **BeforeGeneration** — `build_final_rag_prompt` 前。修改 `ctx.retrieval_results` → 调整 prompt 上下文
4. **AfterGeneration** — answer_stream 创建后，return 前。记录用（`output` 为 `None`）

**CoordinatorEngine 集成**（`crates/core/src/coordinator.rs`）：
- `CoordinatorEngine.hooks: Option<Arc<HookRegistry>>` — `None` = 无 hook
- `CoordinatorEngine::with_hooks(self, registry: HookRegistry) -> Self` — builder 方法注册 hooks

**4 处 Hook 调用点（CoordinatorEngine::run）**：
1. **BeforeRetrieval** — Phase 1 Research 前。修改 `ctx.query` → 后续使用 `hook_query`
2. **AfterRetrieval** — Phase 1 去重后。修改 `ctx.retrieval_results` → 过滤/增强检索结果
3. **BeforeGeneration** — Phase 3 Implementation 前。修改 `ctx.retrieval_results` → 调整 prompt 上下文
4. **AfterGeneration** — answer_stream 创建后。记录用

**AppState 集成**（`crates/tauri-app/src/state.rs`）：
- `AppState.hook_registry: Option<Arc<HookRegistry>>` — 默认 `None`（后续可从设置加载自定义 hook）

### 8.12 DAG 工作流引擎（REQ-RAG-030）

**WorkflowEngine 结构体**（`crates/core/src/workflow.rs`）：

```rust
pub struct WorkflowEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
}
```

- `WorkflowEngine::new(retriever: R, llm: L) -> Self` — 创建引擎
- `WorkflowEngine::run(&self, workflow: &Workflow, input: &str) -> Result<WorkflowResult>` — 执行工作流（拓扑排序 → 分层并行执行 → 条件分支 → 聚合 → 容错）
- `WorkflowEngine::parallel_layers(&self, nodes: &[WorkflowNode], edges: &[WorkflowEdge]) -> Result<Vec<Vec<String>>>` — Kahn's algorithm 拓扑排序，返回并行执行层
- `WorkflowEngine::evaluate_condition(expression: &str, input: &str) -> bool` — 评估条件表达式（`contains:` / `length>N` / `length<N` / `nonempty` / `true`）

**数据模型**（`crates/models/src/lib.rs`）：

- `Workflow` — `{ id, name, description, nodes: Vec<WorkflowNode>, edges: Vec<WorkflowEdge>, created_at }`
- `WorkflowNode` — `{ id, label, node_type: NodeType }`
- `NodeType` — `#[serde(tag = "kind")]` 标签枚举：
  - `Retrieval { top_k, retrieval_mode }` — 知识库检索
  - `Generation { system_prompt, model }` — LLM 生成（`{input}` 占位符替换）
  - `Condition { expression }` — 条件分支
  - `Aggregate { strategy }` — 多输入聚合（`concat` / `summarize` / `best_of`）
  - `Output { format }` — 工作流终点
- `WorkflowEdge` — `{ source, target, mapping: Option<String> }`（Condition 下游边 mapping 用作条件标签 `"true"` / `"false"`）
- `NodeStatus` — `Pending` / `Running` / `Completed { output }` / `Failed { error }` / `Skipped`
- `WorkflowResult` — `{ node_results: HashMap<String, NodeStatus>, final_output: String, duration_ms: u64 }`

### 8.13 子代理舰队（REQ-RAG-025 扩展，`crates/core/src/sub_agent.rs`）

借鉴 Bamboo-agent 子代理 + mailbox 通信：主代理将复杂查询分解为子任务，为每个子任务创建独立 SubAgent，通过 mailbox 消息传递协调。

**AgentMessage 结构体**：

```rust
pub struct AgentMessage {
    pub from: String,          // 发送者 ID
    pub to: String,            // 接收者 ID（"coordinator" = 主代理）
    pub kind: MessageKind,     // 消息类型
    pub payload: String,       // 消息内容（JSON 序列化）
    pub timestamp: i64,        // Unix 秒
}
```

- `AgentMessage::new(from, to, kind, payload) -> Self`

**MessageKind 枚举**：`Task` / `Result` / `Query` / `Response` / `Terminate`

**SubAgentResult 结构体**：

```rust
pub struct SubAgentResult {
    pub agent_name: String,
    pub status: SubAgentStatus,
    pub output: String,            // 成功=答案文本，失败=错误信息
    pub sources: Vec<RetrievalResult>,
    pub duration_ms: u64,
}
```

**SubAgentStatus 枚举**：`Completed` / `Failed` / `Terminated` / `TimedOut`

**SubAgent 结构体**：

```rust
pub struct SubAgent {
    pub id: String,
    pub name: String,
    pub task: String,
    mailbox_tx: mpsc::UnboundedSender<AgentMessage>,
}
```

- `SubAgent::new(id, name, task) -> (Self, UnboundedReceiver<AgentMessage>)` — 创建子代理 + mailbox 接收端
- `SubAgent::run<R: Retriever, L: LLMProvider>(self, retriever: &R, llm: &L, timeout_secs: Option<u64>)` — 执行子任务（检索 + LLM 生成简要回答 → mailbox 发送 SubAgentResult）
  - 共享 `&R` / `&L` 不可变引用（`Retriever: Send + Sync` / `LLMProvider: Send + Sync` 保证并发安全），无需 Clone 或 Arc
  - `timeout_secs` 为 `Some(secs)` 时使用 `tokio::time::timeout` 包装，超时返回 `TimedOut` 状态
  - 检索/LLM 调用失败返回 `Failed` 状态（不影响其他子代理）

**SubAgentFleet 结构体**：

```rust
pub struct SubAgentFleet {
    agent_count: usize,
    max_agents: usize,             // 默认 4
    receivers: Vec<UnboundedReceiver<AgentMessage>>,
    agent_names: HashMap<String, String>,
}
```

- `SubAgentFleet::new() -> Self` — 创建空舰队（max_agents=4）
- `SubAgentFleet::with_max_agents(max_agents: usize) -> Self`
- `SubAgentFleet::dispatch(tasks: Vec<(String, String)>) -> Result<Vec<SubAgent>>` — 派发子代理任务（超过 max_agents 返回 Err）
- `SubAgentFleet::join_all() -> Vec<SubAgentResult>` — 等待所有子代理完成并收集结果
- `SubAgentFleet::count() -> usize` / `is_empty() -> bool`

**CoordinatorEngine 集成**（`crates/core/src/coordinator.rs`）：

- `CoordinatorEngine.sub_agent_timeout: Option<u64>` — `None` = 禁用（标准并行检索），`Some(secs)` = 启用子代理舰队
- `CoordinatorEngine::with_sub_agent_timeout(self, secs: u64) -> Self` — builder 方法
- `CoordinatorEngine::research_with_sub_agents(&self, sub_queries) -> Result<Vec<(String, Vec<RetrievalResult>)>>` — 子代理模式：dispatch → join_all 并发执行 → 收集结果
- `research_parallel()` 检查 `sub_agent_timeout.is_some()`，是则走 `research_with_sub_agents()`，否则走原有 `join_all + retrieve()` 路径

**IPC 命令**：

- `set_sub_agent_enabled(enabled: bool) -> ()` — 写入 `rag.sub_agent_enabled` 设置键
- `set_sub_agent_enabled_inner(enabled, state) -> Result<(), String>` — 测试复用

**设置键**：`rag.sub_agent_enabled`（bool，默认 false，仅在 `rag.coordinator_enabled` 为 true 时生效）

---

## 10. 代码符号引擎（REQ-RAG-031）

**SymbolExtractor trait**（`crates/core/src/lib.rs`）：

```rust
pub trait SymbolExtractor: Send + Sync {
    /// 检测文件语言（根据扩展名）。
    fn detect_language(&self, file_path: &str) -> Option<String>;

    /// 从代码文本中抽取符号。
    fn extract_symbols(&self, content: &str, language: &str, chunk_id: &str) -> Vec<CodeSymbol>;

    /// 代码感知分块：按函数/类边界分块（而非 token 窗口）。
    fn split_by_symbols(&self, content: &str, language: &str, max_tokens: usize) -> Vec<(String, usize, usize)>;
}
```

- `detect_language(&self, file_path: &str) -> Option<String>` — 返回 `Some("rust")` / `Some("typescript")` / `Some("python")` / `Some("go")`，非代码文件返回 `None`
- `extract_symbols(&self, content: &str, language: &str, chunk_id: &str) -> Vec<CodeSymbol>` — 递归遍历 AST 节点，返回所有符号定义
- `split_by_symbols(&self, content: &str, language: &str, max_tokens: usize) -> Vec<(String, usize, usize)>` — 返回 `Vec<(chunk_text, start_line, end_line)>`，每个顶级函数/类/结构体为一个独立 chunk，超大函数内部按行分割

**SymbolEngine 实现**（`crates/infra/src/symbol_engine.rs`，Pro feature）：

```rust
pub struct SymbolEngine;

impl SymbolExtractor for SymbolEngine {
    fn detect_language(&self, file_path: &str) -> Option<String> { /* ... */ }
    fn extract_symbols(&self, content: &str, language: &str, chunk_id: &str) -> Vec<CodeSymbol> { /* ... */ }
    fn split_by_symbols(&self, content: &str, language: &str, max_tokens: usize) -> Vec<(String, usize, usize)> { /* ... */ }
}
```

- 零大小类型（ZST），无内部状态
- 支持语言：Rust（tree-sitter-rust）、TypeScript/TSX（tree-sitter-typescript）、Python（tree-sitter-python）、Go（tree-sitter-go）
- 解析失败优雅降级（返回空 Vec）

**数据模型**（`crates/models/src/lib.rs`）：

`CodeSymbol` — `{ id, chunk_id, name, kind, language, start_line, end_line, signature: Option<String> }`

`SymbolKind` 枚举（序列化/反序列化）：

```rust
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Enum,
    Constant,
    Module,
}
```

- `Function` — 独立函数（Rust `fn`，TS/Python/Go 的顶层函数）
- `Method` — impl / class 内部方法
- `Class` — Python/TS class 定义
- `Struct` — Rust/Go struct 定义
- `Interface` — Rust trait / TS interface / Go interface
- `Enum` — Rust/TS enum 定义
- `Constant` — Rust const / Go const
- `Module` — Rust module

**数据库表**（`code_symbols`，SQLite，`docs/api/database-schema.md` §1.9）：

- 3 索引：`idx_symbols_name` (name)、`idx_symbols_chunk` (chunk_id)、`idx_symbols_kind` (kind)
- 级联删除：删除 chunk 自动清理其所有符号（`ON DELETE CASCADE`）

**IPC 命令**（`crates/tauri-app/src/commands.rs`）：

- `search_symbols(query: String, limit?: number) -> CodeSymbol[]` — 精确 + 模糊匹配，去重后截断到 limit
- `search_symbols_inner(query, limit, state) -> Result<Vec<CodeSymbol>, String>` — 测试复用
- `get_symbols_for_chunk(chunk_id: String) -> CodeSymbol[]` — 获取指定 chunk 的所有符号
- `rebuild_symbol_index() -> number` (Pro) — 重建所有代码文件的符号索引
- `rebuild_symbol_index_inner(state) -> Result<usize, String>` (Pro) — 测试复用

## 11. 代码执行沙箱（REQ-RAG-032）

**CodeExecutor trait**（`crates/core/src/lib.rs`）：

```rust
pub trait CodeExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        code: &'a str,
        language: &'a str,
        stdin: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<ExecutionResult>> + Send + 'a>>;

    fn supported_languages(&self) -> Vec<&str>;

    fn config(&self) -> CodeExecutorConfig;
}
```

- `execute(&self, code, language, stdin) -> Pin<Box<Future>>` — 执行代码片段，返回 `ExecutionResult`。`Pin<Box<Future>>` 返回类型保证对象安全（dyn-compatible），与 `Reranker` / `QueryRewriter` / `AgentHook` 一致
- `supported_languages(&self) -> Vec<&str>` — 返回支持的语言列表（如 `["python", "javascript"]`）
- `config(&self) -> CodeExecutorConfig` — 返回执行器配置（超时 / 内存限制 / 网络访问）

**安全约束**：
- 超时：默认 10s（`CodeExecutorConfig::timeout_secs`，硬编码上限 30s）
- 内存：默认 64MB（`CodeExecutorConfig::memory_limit_mb`，硬编码上限 256MB）
- 网络：永远禁用（`CodeExecutorConfig::allow_network` 恒为 `false`）

**实现清单**：

| 实现 | 位置 | 用途 |
|---|---|---|
| `MockExecutor` | `crates/core/src/code_executor.rs` | 测试用，返回预定义结果 |
| `NoExecutor` | `crates/core/src/code_executor.rs` | Free 版本占位，始终返回错误 |
| `WasmExecutor` | `crates/infra/src/wasm_executor.rs` (Pro) | 本地进程执行（python3/node）+ 安全限制 |

**WasmExecutor**（`crates/infra/src/wasm_executor.rs`，Pro feature）：

```rust
pub struct WasmExecutor {
    config: CodeExecutorConfig,
}

impl WasmExecutor {
    pub fn new(config: CodeExecutorConfig) -> Self;
    pub fn with_defaults() -> Self;  // 10s / 64MB / no network
}
```

- 第一阶段 MVP：使用 `tokio::process::Command` 调用本地解释器（`python3 -c` / `node -e`）
- 安全措施：`env_clear()` 清空环境变量 + `tempfile::tempdir()` 临时目录隔离 + `libc::setrlimit(RLIMIT_AS)` 内存限制（Unix） + `tokio::time::timeout` 超时 kill
- 后续替换为 WASM 沙箱（wasmtime）

**辅助函数**：

- `format_execution_result(result: &ExecutionResult) -> String` — 格式化执行结果为 Agent Observation 字符串

**Agent 集成**：

- `AgentEngine::with_executor(executor: Arc<dyn CodeExecutor>) -> Self` — builder 方法，注册代码执行器
- `execute_code` Agent 工具：LLM 在 ReAct 循环中调用，JSON 输入 `{"language":"python","code":"...","stdin":"..."}`
- Free 版本（executor=None）：`execute_code` 返回 "代码执行需要 Pro 版本"（优雅降级）

**数据模型**（`crates/models/src/lib.rs`）：

```rust
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub struct CodeExecutorConfig {
    pub timeout_secs: u64,       // 默认 10s
    pub memory_limit_mb: usize,  // 默认 64MB
    pub allow_network: bool,     // 默认 false（恒为 false）
}
```

**IPC 命令**（`crates/tauri-app/src/commands.rs`）：

- `execute_code_snippet(code: String, language: String, stdin: Option<String>) -> ExecutionResult` (Pro) — 执行代码片段
- `execute_code_snippet_inner(code, language, stdin) -> anyhow::Result<ExecutionResult>` (Pro) — 测试复用

## 12. 持久化记忆系统（REQ-RAG-033）

**MemoryRetriever trait**（`crates/core/src/memory_store.rs`，对象安全）：

```rust
pub trait MemoryRetriever: Send + Sync {
    fn retrieve_relevant_memories<'a>(
        &'a self,
        query: &'a str,
        top_k: usize,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<MemoryEntry>>> + Send + 'a>>;
}
```

- `retrieve_relevant_memories(&self, query, top_k) -> Pin<Box<Future>>` — 检索与查询相关的记忆条目，按 importance DESC 排序。对象安全设计（与 `Reranker`/`QueryRewriter` 一致），使 `ChatEngine` 能以 `Option<Arc<dyn MemoryRetriever>>` 形式注入。

**MemoryStore<S: Storage>**（`crates/core/src/memory_store.rs`）：

```rust
pub struct MemoryStore<S: Storage> {
    pub(crate) storage: S,
    max_wing: usize,  // 默认 50
    max_hall: usize,  // 默认 100
    max_room: usize,  // 默认 500
}
```

**方法清单**：

| 方法 | 签名 | 说明 |
|---|---|---|
| `new(storage)` | `MemoryStore<S>` | 创建记忆存储引擎（默认上限） |
| `with_limits(wing, hall, room)` | `Self` | 自定义各层上限 |
| `extract_from_conversation<L: LLMProvider>(messages, llm)` | `Vec<MemoryEntry>` | LLM 辅助从对话提取关键事实 → Wing 层 |
| `promote(memory_id)` | `()` | 提升层级 Wing→Hall→Room，importance += 0.1 |
| `forget(tier)` | `usize` | 遗忘低分记忆（forget_score < 0.3） |
| `retrieve_relevant(query, top_k)` | `Vec<MemoryEntry>` | 检索相关记忆 + 更新 access_count |
| `consolidate()` | `ConsolidationResult` | 自动整合：Wing≥0.6→Hall, Hall≥0.7→Room, <0.3→遗忘 |
| `pin_memory(conversation_id, content)` | `MemoryEntry` | 用户置顶 → Room 层 importance 1.0 |

**数据模型**（`crates/models/src/lib.rs`）：

```rust
pub struct MemoryEntry {
    pub id: String,              // UUID v4
    pub tier: MemoryTier,        // Wing / Hall / Room
    pub content: String,         // 记忆内容
    pub source: MemorySource,    // UserStatement / AssistantAnswer / AutoExtracted / UserPinned
    pub conversation_id: Option<String>,
    pub created_at: i64,         // Unix 秒
    pub last_accessed: i64,
    pub access_count: u32,
    pub importance: f32,         // 0.0-1.0
}

pub struct ConsolidationResult {
    pub promoted: usize,
    pub forgotten: usize,
    pub remaining_wing: usize,
    pub remaining_hall: usize,
    pub remaining_room: usize,
}
```

**ChatEngine 集成**：

- `ChatEngine::with_memory(memory: Arc<dyn MemoryRetriever>) -> Self` — builder 方法，启用记忆注入
- `chat_with_sources()` 在构建 system prompt 前检索相关记忆，以 `[相关记忆]` 块注入到静态前缀之前
- 向后兼容：未调用 `with_memory()` 时行为完全一致

**DreamEngine 集成**：

- `DreamEngine::consolidate_memories()` — Phase 4 记忆整合（AutoDream 后台空闲时执行）
- 整合结果生成 `DreamSuggestion`（类型 Organization，严重等级 Low）

**Storage trait 扩展**（6 个方法）：

- `add_memory_entry(entry: &MemoryEntry) -> ()` — INSERT OR REPLACE
- `get_memory_entries(tier: Option<&MemoryTier>) -> Vec<MemoryEntry>` — SELECT（按 importance DESC）
- `update_memory_entry(entry: &MemoryEntry) -> ()` — UPDATE tier/access_count/importance
- `delete_memory_entry(id: &str) -> ()` — DELETE
- `clear_memory_entries(tier: Option<&MemoryTier>) -> usize` — 批量 DELETE
- `search_memory_entries(query: &str, limit: usize) -> Vec<MemoryEntry>` — LIKE 模糊搜索

**IPC 命令**（`crates/tauri-app/src/commands.rs`）：

- `set_memory_enabled(enabled: bool) -> ()` — 启用/禁用记忆系统
- `get_memories(tier: Option<String>) -> Vec<MemoryEntry>` — 查询记忆列表
- `pin_memory(conversation_id: String, content: String) -> MemoryEntry` — 用户置顶
- `promote_memory(memory_id: String) -> ()` — 手动提升层级
- `delete_memory(memory_id: String) -> ()` — 删除记忆
- `clear_memories(tier: Option<String>) -> usize` — 清空记忆

---

### 4.10 网页搜索集成（REQ-RAG-036）

**WebSearchProvider trait**（`crates/core/src/lib.rs`）：

```rust
pub trait WebSearchProvider: Send + Sync {
    fn search<'a>(
        &'a self,
        query: &'a str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<SearchResult>>> + Send + 'a>>;
}
```

- `search(&self, query) -> Pin<Box<Future>>` — 执行网页搜索，返回 `SearchResult` 列表。对象安全设计（与 `Reranker`/`QueryRewriter` 一致），使 `ChatEngine` 能以 `Option<Arc<dyn WebSearchProvider>>` 形式注入。

**SearchResult struct**（`crates/models/src/lib.rs`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,  // 如 "duckduckgo"
}
```

**web_search 模块**（`crates/core/src/web_search.rs`）：

| 函数 | 签名 | 说明 |
|------|------|------|
| `should_search` | `(local_results: &[RetrievalResult], threshold: f32) -> bool` | 阈值触发判断：本地结果为空或 top-1 score < threshold 时返回 true |
| `convert_to_retrieval_results` | `(results: &[SearchResult]) -> Vec<RetrievalResult>` | 将 SearchResult 转换为 RetrievalResult（doc_name 前缀 🌐 标记 Web 来源） |
| `rrf_fuse_with_web` | `(local: Vec<RetrievalResult>, web: Vec<RetrievalResult>, top_k: usize) -> Vec<RetrievalResult>` | RRF 融合本地（权重 1.0）+ Web（权重 0.4）结果 |
| `search_and_fuse` | `(provider: &Arc<dyn WebSearchProvider>, query, local_results, top_k) -> Vec<RetrievalResult>` | 主入口：搜索→转换→融合，失败时优雅降级返回原始本地结果 |
| `DEFAULT_SEARCH_THRESHOLD` | `f32 = 0.3` | 默认触发阈值 |

**DuckDuckGoProvider**（`crates/infra/src/duckduckgo_provider.rs`）：

```rust
pub struct DuckDuckGoProvider {
    client: reqwest::Client,
}
```

- 实现 `WebSearchProvider` trait
- API 端点：`https://api.duckduckgo.com/?q=QUERY&format=json&no_html=1&skip_disambig=1`
- 连接超时 5s，请求超时 10s
- 解析 `AbstractText`（直接答案）+ `RelatedTopics`（相关主题列表，含嵌套 Topics）
- HTTP 非 2xx → 返回空 Vec（优雅降级）
- 无需 API Key（DuckDuckGo Instant Answer API 免费）

**ChatEngine 集成**（`crates/core/src/chat.rs`）：

- `ChatEngine::with_web_search(provider: Arc<dyn WebSearchProvider>) -> Self` — builder 方法，注入网页搜索 provider
- `chat()` 方法在检索后判断 `should_search()`，若 true 则调用 `search_and_fuse()` 融合结果