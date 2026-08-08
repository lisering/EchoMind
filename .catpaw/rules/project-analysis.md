# EchoMind（灵犀）项目分析

> 本文件由 CatPaw 自动生成，用于跨会话保留项目分析结果，避免重复分析。

## 项目概览

**EchoMind（灵犀）** 是一个基于 Rust + Tauri v2 的**本地 RAG（检索增强生成）知识库桌面应用**。所有数据（文档、向量、对话）完全本地化，仅 LLM 推理通过用户自配的 OpenAI 兼容 API 进行。

| 属性 | 值 |
|---|---|
| 语言 | Rust (Edition 2024) + Vanilla JS |
| 框架 | Tauri v2 + 六边形架构 |
| 嵌入模型 | all-MiniLM-L6-v2（ONNX，384 维，本地 CPU 推理） |
| 向量存储 | SQLite（WAL 模式，BLOB 存原始 f32 字节） |
| LLM | 任意 OpenAI Chat Completions 兼容 API（流式 SSE） |
| 前端 | 单文件 SPA（Tailwind 本地构建 + marked.js + DOMPurify + highlight.js，禁止 CDN） |

## 四层 Crate 架构

| Crate | 职责 | 关键文件 |
|---|---|---|
| `crates/models` | 纯数据模型（Document, Chunk, ChatMessage, Conversation, DocStatus, RetrievalResult, LlmConfig, SettingsPayload, event payloads） | `src/lib.rs` (~177行) |
| `crates/core` | 核心业务逻辑（Trait + 实现） | 7 个模块: chat, import, license, loader, retriever, splitter, stream_parse |
| `crates/infra` | 适配器（SQLite, fastembed, OpenAI） | 3 个模块: sqlite_storage, local_embedder, openai_provider |
| `crates/tauri-app` | Tauri IPC 外壳，14 个命令 | `src/lib.rs` (~50行), `src/commands.rs` (~635行), `src/state.rs` (~112行) |

## 核心 Trait（原生 async fn in trait，不使用 async-trait 宏）

```rust
pub trait Loader: Send + Sync { async fn load(&self, file_path: &str) -> Result<String>; }
pub trait Splitter: Send + Sync { async fn split(&self, text: &str) -> Result<Vec<String>>; }
pub trait Embedder: Send + Sync { async fn embed(&self, text: &str) -> Result<Vec<f32>>; async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>; }
pub trait Storage: Send + Sync { /* 20+ 方法：CRUD + vector_search + 会话管理 + settings */ }
pub trait LLMProvider: Send + Sync { async fn chat_stream(&self, system_prompt: &str, history: &[ChatMessage], query: &str) -> Result<BoxStream<'static, Result<String>>>; }
pub trait Retriever: Send + Sync { async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<RetrievalResult>>; }
```

## Tauri IPC 命令（14个）

| 分类 | 命令 |
|---|---|
| 导入 | `import_files` |
| 文档管理 | `get_documents`, `delete_document` |
| 对话 | `chat`, `abort_chat` |
| 会话 | `create_conversation`, `get_conversations`, `get_messages`, `delete_conversation` |
| 配置 | `test_llm_connection`, `update_llm_config`, `get_settings` |
| 授权 | `activate_pro`, `get_pro_status` |

## Tauri Events

| 事件 | 负载 | 用途 |
|---|---|---|
| `doc-status-changed` | `DocStatusPayload { status, message }` | 文档导入/索引状态更新 |
| `chat_phase` | `ChatPhasePayload { phase, message }` | 对话进度阶段（preparing/retrieving/generating） |
| `chat_token` | `String` | 流式 token 推送 |
| `chat_sources` | `Vec<RetrievalResult>` | 检索引用来源 |
| `chat_error` | `String` | 错误消息 |
| `chat_done` | `()` | 对话结束 |

## 数据库 Schema

```sql
documents(id, file_path UNIQUE, file_hash, status, created_at)
chunks(id, doc_id, content, token_count, sequence)
embeddings(chunk_id, embedding BLOB)
conversations(id, workspace_id, title, created_at)
messages(id, conversation_id, role, content, sources)
settings(key UNIQUE, value)
```

## 关键设计

- **`*_inner` 模式**: 每个 IPC 命令有对应的 `*_inner` 函数，接收 `&AppState` 而非 `State<'_, AppState>`，供集成测试复用
- **懒加载 Embedder**: `AppState.embedder` 使用 `tokio::sync::OnceCell`，首次调用触发模型下载
- **CancellationToken**: 每个会话一个中断令牌，`abort_chat` 触发后流立即中断，已生成内容保留
- **崩溃恢复**: 启动时 `cleanup_zombies()` 将 Processing 状态文档标记为 Failed
- **API Key 脱敏**: `get_settings` 仅返回 `****` + 末四位
- **E2E 测试模式**: `ECHOMIND_DATA_DIR` 重定向数据目录，`tauri-stub.js` 注入 Mock
- **Pro 授权**: License 离线校验，存储于 settings 表 `license.is_pro`
- **免费版限制**: 50 文件上限、PDF 禁用

## 数据流

### 文档导入
```
import_files IPC → ImportService.import_one()
  → Loader.load() → MD5 hash → 去重检查
  → Splitter.split() → Storage.add_document() + add_chunk()
  → Embedder.embed_batch() → Storage.add_embedding()
  → doc-status-changed 事件 → 前端列表更新
```

### RAG 查询
```
chat IPC → chat_inner()
  → 空库拦截 → 加载 LLM 配置 → chat_phase "preparing"
  → 初始化 embedder → 创建/更新会话
  → ChatEngine.chat()
    → VectorRetriever.retrieve() → embed(query) → vector_search(top-k=5)
    → 阈值过滤(0.1) → build RAG prompt → LLM chat_stream(SSE)
  → chat_sources 事件 → chat_phase "generating"
  → chat_token* 事件 → 前端打字机
  → abort_token 检查 → chat_done 事件
  → persist_exchange() → 落库(user + assistant 消息)
```

## 性能优化

- `[profile.dev.package."*"] opt-level=3`: ML 依赖优化编译
- 懒加载 embedder: 启动不阻塞
- `tokio::task::spawn_blocking`: 所有 DB 操作不阻塞 async executor
- 批量嵌入: `embed_batch` 支持批量推理

## 测试

- 单元测试: 每模块 `#[cfg(test)] mod *_tests`（与实现同文件）
- 集成测试: `crates/tauri-app/tests/integration.rs`
- E2E: `e2e-tests/tests/`（Playwright + tauri-stub.js mock）
- 混沌测试: `tests/fixtures/`（generate_corpus.py 自动生成）
- CI: `.github/workflows/e2e.yml`
