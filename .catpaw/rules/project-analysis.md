# EchoMind（灵犀）项目分析 v1.4.0

> 本文件由 AI Agent 维护，用于跨会话保留项目分析结果，避免重复分析。
> **最后更新**: 2026-08-09 | **版本**: v1.4.0

## 项目概览

**EchoMind（灵犀）** 是一个基于 Rust + Tauri v2 的**本地优先 RAG（检索增强生成）知识库桌面应用**。所有数据（文档、向量、对话）完全本地化，仅 LLM 推理通过用户自配的 OpenAI 兼容 API 进行。面向有隐私需求的个人用户（律师、研究人员等）。

| 属性 | 值 |
|---|---|
| 语言 | Rust (Edition 2024, ≥1.85) + Vanilla JS (ES modules) |
| 框架 | Tauri v2 + 六边形架构（端口与适配器） |
| 嵌入模型 | all-MiniLM-L6-v2（ONNX，384 维，本地 CPU 推理，fastembed）+ 自定义 ONNX 模型上传（Pro） |
| 向量存储 | SQLite（WAL 模式，BLOB 存原始 f32 字节，r2d2 连接池，SQLCipher AES-256 加密） |
| LLM | 任意 OpenAI Chat Completions 兼容 API（流式 SSE）+ 本地 mistral.rs v0.9.0（Pro） |
| 前端 | 单文件 SPA（`ui/index.html`，126K 行），50 个 ES module 源文件，esbuild 打包 IIFE，Tailwind CSS JIT，无框架无 CDN |
| 许可证 | BUSL-1.1（商业源码许可证，2030-01-01 转 Apache 2.0） |
| 版本 | v1.4.0 |

## 八层 Crate 架构

依赖方向铁律：`tauri-app → core → models`，严禁反向。

| Crate | 职责 | 模块数 | 关键 Trait/结构 |
|---|---|---|---|
| `crates/models` | 纯数据契约（零业务逻辑、零外部服务依赖） | 1 | Document, Chunk, ChatMessage, Conversation, LlmMode, ModelInfo, CacheStats, Proposition, MemoryRecord, Entity, WikiLink, PendingInput 等 |
| `crates/prompt` | 提示词构建（Prompt Caching 优化） | 2 | SegmentedPrompt, build_rag_prompt_segmented, CachePolicy, CachedSegmentedPrompt |
| `crates/compact` | 上下文压缩引擎 | 1 | CompactionEngine, TokenEstimator, heal_dangling_calls, schedule_background_compaction |
| `crates/context` | 系统上下文注册表（OpenCode 借鉴 B01-B02） | 2 | SystemContext, ContextEpochManager, ContextSnapshot, 5 个内置 Source |
| `crates/core` | 端口 Trait + 业务逻辑（禁止依赖具体实现） | 50+ | Loader, Splitter, Embedder, Storage, LLMProvider, Retriever, Reranker, QueryRewriter, PageRenderer, OcrEngine, VisionLanguageModel, DomainClassifier, CodeExecutor, PromptCompressor, ResponseCache, SecurityScreener, MemoryRetriever 等 16 个 Trait |
| `crates/infra` | 适配器（实现 core Trait） | 28 | SqliteStorage, LocalEmbedder, OpenAIProvider, LocalLlmEngine, HnswIndex, OcrsEngine, PdfiumRenderer, OpenAiVision, SymbolEngine, WasmExecutor, LocalReranker, HydeRewriter, DuckDuckGoProvider, FileWatcher, LocalLogger, ModelManager, KvCache 等 |
| `crates/tauri-app` | Tauri IPC 外壳 | 16 | 162 个 IPC 命令（14 个功能域子模块 + stores/ 5 个状态模块） |
| `crates/license-issuer` | Ed25519 License 签发 CLI | 1 | genkey / issue 命令 |

## 核心 Trait（16 个端口）

```rust
// 原生 async fn in trait（Edition 2024），不使用 async-trait 宏
pub trait Loader: Send + Sync { async fn load(&self, file_path: &str) -> Result<String>; }
pub trait Splitter: Send + Sync { async fn split(&self, text: &str) -> Result<Vec<String>>; }
pub trait Embedder: Send + Sync { async fn embed(&self, text: &str) -> Result<Vec<f32>>; async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>; }
pub trait MultiVectorEmbedder: Send + Sync { /* ColBERT 多向量 */ }  // Pro
pub trait Storage: Send + Sync { /* 60+ 方法：CRUD + vector_search + keyword_search + 会话 + settings + wiki + graph + memory + scratch + pending_inputs */ }
pub trait LLMProvider: Send + Sync { async fn chat_stream(...) -> BoxStream; async fn chat_stream_segmented(...) -> BoxStream; /* 默认方法: generate_title, one_shot, judge, context_token_budget */ }
pub trait Retriever: Send + Sync { async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<RetrievalResult>>; async fn retrieve_with_embedding(&self, query_vec: &[f32], top_k: usize) -> Result<Vec<RetrievalResult>>; }
pub trait Reranker: Send + Sync { /* bge-reranker-base Cross-Encoder */ }
pub trait QueryRewriter: Send + Sync { /* HyDE 查询改写 */ }
pub trait PageRenderer: Send + Sync { /* PDF 页面 → PNG，Pro */ }
pub trait OcrEngine: Send + Sync { /* 图片 → 文本，Pro */ }
pub trait VisionLanguageModel: Send + Sync { /* 图片 + prompt → 结构化描述，Pro */ }
pub trait DomainClassifier: Send + Sync { /* 16 领域嵌入质心分类 */ }
pub trait CodeExecutor: Send + Sync { /* 代码沙箱执行，Pro */ }
pub trait PromptCompressor: Send + Sync { /* 上下文压缩 */ }
pub trait ResponseCache: Send + Sync { /* 三级语义缓存 L0/L1/L3 */ }
pub trait SecurityScreener: Send + Sync { /* Shadow 安全筛查 */ }
pub trait MemoryRetriever: Send + Sync { /* 持久化记忆检索 */ }
```

## Tauri IPC 命令（162 个，14 个功能域子模块）

| 模块 | 文件 | 命令数 | 功能 |
|---|---|---|---|
| chat | `chat.rs` | ~20 | 对话/RAG/持久化/abort/标题生成/记忆管道 |
| import | `import.rs` | ~10 | 文档导入/嵌入/索引/重建/wiki 链接 |
| document | `document.rs` | ~12 | 文档管理/摘要/标签/分类 |
| conversation | `conversation.rs` | ~15 | 会话/消息/搜索/分支树 |
| audit | `audit.rs` | ~5 | 文档一致性审计（Pro） |
| license | `license.rs` | ~5 | 许可证激活/验证 |
| settings | `settings.rs` | ~10 | LLM 配置/主题/设置 |
| local_llm | `local_llm.rs` | ~15 | 本地 LLM/模型下载/采样参数/KV cache（Pro） |
| security | `security.rs` | ~15 | 加密/解锁/PII/审计/安全态势/Shadow 筛查 |
| agent | `agent.rs` | ~5 | Agent/SubAgent/Coordinator |
| performance | `performance.rs` | ~15 | 缓存/压缩/BM25/Proposition/检索记忆/Contextual Retrieval |
| graph | `graph.rs` | ~10 | 知识图谱/路径分析/社区检测/导出 |
| sync | `sync.rs` | ~5 | 文件同步/文件夹监听 |
| features | `features.rs` | ~20 | Dream/日志/符号/工作流/代码执行/记忆/Web 搜索/自定义模型/模板 |

## Tauri Events

| 事件 | 用途 |
|---|---|
| `doc-status-changed` | 文档导入/索引状态（含 sub_phase: text_extracting/image_rendering/ocr/vlm_enhancing） |
| `chat_phase` | 对话进度阶段（preparing/retrieving/generating） |
| `chat_token` | 流式 token 推送 |
| `chat_sources` | 检索引用来源 |
| `chat_error` / `chat_done` | 错误/完成 |
| `agent_step` | Agent ReAct 步骤 |
| `coordinator_phase` | 多代理协调阶段 |
| `model_download_progress` | 本地 LLM 模型下载进度 |
| `model_load_progress` | 本地 LLM 加载状态（loading/ready/error） |
| `embedding_progress` | 大文档嵌入进度 |
| `sync_progress` | 文件同步进度 |
| `security-state-changed` | 安全状态变更（加密/解锁/锁定） |

## 数据库 Schema（SqliteStorage，WAL + r2d2 池 + FTS5 + SQLCipher）

```
documents(id, file_path, file_hash, status, created_at, domain, tags, summary)
chunks(id, doc_id, content, token_count, sequence, section_path)
embeddings(chunk_id, embedding BLOB)
conversations(id, workspace_id, title, created_at)
messages(id, conversation_id, role, content, sources, turn_group, version, turn_active_versions, security_tainted, created_at)
messages_fts(FTS5 虚拟表 + 3 触发器)  -- 对话全文搜索
settings(key, value)
entities(id, name, entity_type, doc_id, chunk_id)
entity_relations(id, source_entity, target_entity, relation_type, chunk_id, confidence)
wiki_links(id, source_doc_id, target, chunk_id, created_at)
propositions(id, chunk_id, content, embedding BLOB, sequence)
summary_nodes(id, doc_id, level, content, embedding BLOB, parent_id)
retrieval_memory(query_type, retrieval_method, hit_rate, avg_score, total_queries)
memory_records(id, tier, content, source, created_at, pinned)
scratch_logs(id, date, content, created_at)
pending_inputs(id, conversation_id, content, delivery, created_at, promoted_seq)
audit_logs(id, action, target, timestamp, entry_hash, prev_hash)
budget_records(id, session_id, tokens_in, tokens_out, model, timestamp)
```

## 测试矩阵

| 层级 | 数量 | 运行命令 |
|---|---|---|
| Rust 单元+集成 (Free) | 822+ passed, 24 ignored | `cargo test` |
| Rust 单元+集成 (Pro) | 1094 passed, 44 ignored | `cargo test --features pro` |
| 前端单元 (Vitest) | 407 passed, 27 files | `cd e2e-tests && npm run test:unit` |
| E2E 桥接 (Playwright) | 955 passed, 9 skipped | `cd e2e-tests && npm run test:bridge` |
| Clippy (Free+Pro) | 零警告 | `cargo clippy --all-targets -- -D warnings` |
| TypeScript | 0 errors | `npx tsc --noEmit` |
| Rust 测试文件 | 68 个 `*_tests.rs` | core 44 + infra 22 + compact 1 + context 1 |

## 关键设计模式

- **`*_inner` 模式**: 每个 IPC 命令有对应的 `*_inner` 函数，接收 `&AppState` 而非 `State<'_, AppState>`，供集成测试复用
- **懒加载 Embedder**: `AppState.embedder()` 使用 `tokio::sync::OnceCell`，首次调用触发 ONNX 模型下载
- **CancellationToken**: 每个会话一个中断令牌，`abort_chat` 触发后流立即中断，已生成内容保留
- **崩溃恢复**: 启动时 `cleanup_zombies()` 将 Processing 状态文档标记为 Failed
- **API Key 脱敏**: `get_settings` 仅返回 `****` + 末四位
- **Pro 门控**: `#[cfg(feature = "pro")]` 编译时条件编译，Free 版 50 文件上限 + PDF 禁用
- **KeyedQueue**: `persist_exchange` 按会话 ID 序列化，防止并发写入竞争
- **LlmRouter**: 路由决策（Free={Remote}, Pro={Remote,Local}），不持有 provider 实例
- **SessionCoordinator**: 并发会话管理（Fn 非 FnOnce 允许 wake 重执行，Box::pin 解决 async 递归）
- **ShadowScreenCollector**: 线程安全统计收集器（Arc<Mutex>），记录 agree/disagree/unavailable
- **前端构建**: `node scripts/build-ui.mjs` 用 esbuild 打包 IIFE，不要手改 `ui/index.html`

## 数据流

### 文档导入
```
import_files IPC → ImportService.import_one()
  → Loader.load() (md/txt/pdf/docx/html/pptx/epub/xlsx/csv; PDF 多模态: pdfium+ocrs+VLM)
  → MD5 hash → 去重 → Splitter.split() → Storage.add_document() + add_chunk()
  → Embedder.embed_batch() → Storage.add_embedding()
  → EntityExtractor.extract() → Storage.add_entities() + add_relations_batch()
  → index_wiki_links() → index_propositions()
  → doc-status-changed 事件 → 前端列表更新
```

### RAG 查询
```
chat IPC → chat_inner()
  → 空库拦截 → LlmRouter.resolve() → 加载 LLM 配置 → chat_phase "preparing"
  → 初始化 embedder → 创建/更新会话
  → ChatEngine.chat()
    → HybridRetriever.retrieve() (向量+BM25 RRF, 若开启)
    → (可选) GraphRetriever.expand() → rrf_fuse_four_way()
    → 阈值过滤(0.1) → chunk 扩展(±1)
    → (可选) RuleBasedCompressor 压缩
    → build RAG prompt → LLMProvider.chat_stream(SSE)
  → chat_sources → chat_phase "generating" → chat_token* → abort_token 检查 → chat_done
  → persist_exchange() (KeyedQueue 序列化) → 落库
```

### 安全 & 加密
```
encrypt_database → SecurityManager.encrypt_database(password)
  → Argon2id derive_key → SqliteStorage new_encrypted(PRAGMA key)
  → Unencrypted → EncryptedUnlocked → SecureString wipe
unlock_database → Argon2id verify → PRAGMA key → 暴力破解检查(5次→指数退避)
Auto-lock → idle timeout → Locked → security-state-changed 事件
PII detection → PrivacyManager.redact_text() → 8 种 PII → AuditLogger hash-chain
```

### 本地 LLM 推理（Pro）
```
set_llm_mode("local") → 持久化 settings → tokio::spawn(warm_up())
chat IPC (Local mode) → AppState.local_llm() → LocalLlmEngine::new(model_path)
  → model_load_progress { loading } → mistral.rs GGUF load (spawn_blocking)
  → model_load_progress { ready } → chat_stream() → 同 forward_stream() 循环
download_model → ModelManager.download_model() → .tmp → atomic rename → model_download_progress
```

## 已完成功能矩阵（v1.0~v1.4）

| 版本 | 功能 |
|---|---|
| v1.0 | RAG 检索 + 文档导入(md/txt/pdf/docx/html) + 流式对话 + 安全加密 + 本地 LLM Phase 1-3 + Agent RAG + 极致性能 S1-S10 + 知识图谱 |
| v1.1 | .docx 导入 / .html 导入 / 浅色主题 / 语音输入+TTS / 网页搜索 |
| v1.2 | PDF 导出 / .pptx 导入 / .epub 导入 / 自定义 ONNX 嵌入模型（Pro） |
| v1.3 | 知识图谱导出 / 文档摘要 / 快捷指令模板 / Wiki 双向链接 / 对话分支树 |
| v1.4 | Excel/CSV 导入 / 对话全文搜索(FTS5) / Contextual Retrieval / 文档标签 / commands.rs 模块化 |
| QM Q01-Q11 | Scratch-Promote 记忆 / 记忆摘要注入 / Token 预算 / Dangling Call 修复 / 安全态势分层 / Shadow 筛查 / Sweeper / 幂等性 / KeyedQueue / LLMProvider 扩展 / LlmRouter |
| OpenCode B01-B14 | System Context Registry / Context Epoch / 结构化压缩模板 / Prompt Caching / Prompt Admission / Session Coordinator / Tool Bounding / Ask SDK / Skill 发现 / 多协议 LLM / Permission 控制 |
