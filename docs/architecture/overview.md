# EchoMind 架构概览

> **状态**：活跃 | **最后审阅**：2026-07-25
>
> 本文面向想理解系统设计的开发者。AI Agent 操作手册见根目录 `AGENTS.md`。

---

## 1. 架构风格：六边形架构（Ports & Adapters）

EchoMind 采用六边形架构（又称 Hexagonal Architecture），将业务逻辑与外部依赖隔离。

```
                    ┌──────────────────────────────────────┐
                    │           crates/models              │
                    │  （领域契约：Document, Chunk,          │
                    │   ChatMessage, Conversation,          │
                    │   DocStatus, RetrievalResult...）      │
                    └──────────────────┬───────────────────┘
                                       │
                    ┌──────────────────▼───────────────────┐
                    │           crates/core                 │
                    │  ┌──────────────────────────────────┐ │
                    │  │  Traits (Ports)                   │ │
                    │  │  Loader │ Splitter │ Embedder    │ │
                    │  │  Storage │ LLMProvider │ Retriever│ │
                    │  └──────────────────────────────────┘ │
                    │  ┌──────────────────────────────────┐ │
                    │  │  Business Logic                    │ │
                    │  │  • ChatEngine (RAG orchestration) │ │
                    │  │  • ImportService (file pipeline)  │ │
                    │  │  • VectorRetriever                │ │
                    │  │  • License verification            │ │
                    │  │  • SSE stream parser               │ │
                    │  └──────────────────────────────────┘ │
                    └──────────────────┬───────────────────┘
                                       │
                    ┌──────────────────▼───────────────────┐
                    │           crates/infra                │
                    │  ┌──────────────────────────────────┐ │
                    │  │  Adapters                          │ │
                    │  │  • SqliteStorage (WAL, BLOB vec)  │ │
                    │  │  • LocalEmbedder (fastembed/ONNX) │ │
                    │  │  • OpenAIProvider (SSE streaming) │ │
                    │  └──────────────────────────────────┘ │
                    └──────────────────┬───────────────────┘
                                       │
                    ┌──────────────────▼───────────────────┐
                    │        crates/tauri-app               │
                    │  Tauri Shell + 14 IPC Commands        │
                    │  holds AppState (Arc<RwLock<...>>)   │
                    └──────────────────┬───────────────────┘
                                       │
                    ┌──────────────────▼───────────────────┐
                    │           ui/index.html               │
                    │  Single-file SPA (Tailwind local,     │
                    │  vanilla JS, marked.js, DOMPurify,     │
                    │  highlight.js — NO CDN)                │
                    └──────────────────────────────────────┘
```

### 四个 Crate 职责

| Crate | 角色 | 依赖 |
|---|---|---|
| `echomind-models` | 领域契约（数据结构定义） | 无业务依赖 |
| `echomind-core` | 业务逻辑（trait + impl） | echomind-models |
| `echomind-infra` | 适配器（SQLite, ONNX, HTTP） | echomind-core, echomind-models |
| `tauri-app` | Tauri 壳（IPC 命令 + AppState） | echomind-infra, echomind-core, echomind-models |

### 关键设计决策

详见 [ADR 索引](adr/README.md)。

---

## 2. 核心数据流

### 2.1 文档导入流程

```
import_files IPC → ImportService.import_one()
  → Loader.load() (markdown/text/pdf parsing, pulldown-cmark)
  → MD5 hash → duplicate check (find_document_by_hash)
  → Splitter.split() (token-based chunking, chunk_size=500, overlap=50)
  → Storage.add_document() + add_chunk() (SQLite)
  → Embedder.embed_batch() (fastembed/ONNX, 384-dim)
  → Storage.add_embedding() (vector BLOB)
  → doc-status-changed event → frontend list update
```

### 2.2 RAG 查询流程

```
chat IPC → chat_inner()
  → empty KB check (count_documents == 0 → error)
  → load LLM config (api_key, base_url, model)
  → chat_phase "preparing" event → init embedder (lazy, first-use download)
  → create/update conversation
  → ChatEngine.chat()
    → VectorRetriever.retrieve()
      → Embedder.embed(query) → Storage.vector_search(cosine sim, top-k=5)
    → threshold filter (score > 0.1)
    → build RAG prompt → LLMProvider.chat_stream(SSE)
  → chat_sources event → frontend shows references
  → chat_phase "generating" event
  → chat_token events → frontend typing effect
  → abort_token check (CancellationToken)
  → chat_done event
  → persist_exchange() → Storage.add_message() (user + assistant)
  → auto-extract title on first exchange
```

### 2.3 流中断流程

```
abort_chat IPC → AppState.abort_chat()
  → CancellationToken.cancel()
  → forward_stream() loop breaks on select! biased branch
  → emit "⏹ 生成已中断" + chat_done
  → persist_exchange() saves partial content
  → clear_abort() cleans up token
```

### 2.4 License & 配额流程

```
import_files → ImportService.import_one()
  → check is_pro (from AppState, read from settings table)
  → if !is_pro: count_documents() >= 50 → reject with quota error
  → if !is_pro: file is .pdf → reject with paywall error

activate_pro → verify_license(key)
  → persist to settings table ("license.is_pro" = "true")
  → refresh runtime is_pro = true
```

---

## 3. 关键组件

### 3.1 存储层

| 实现 | 用途 | 特点 |
|---|---|---|
| `SqliteStorage` (`crates/infra`) | 生产环境 | WAL 模式、向量 BLOB、`spawn_blocking`、崩溃恢复 |
| (测试用 fake) | 单元测试 | 待实现 InMemoryStorage |

### 3.2 异步 Trait 设计

`crates/core/src/lib.rs` 中的 trait（`Loader`, `Splitter`, `Embedder`, `Storage`, `LLMProvider`, `Retriever`）使用原生 `async fn` in trait（Edition 2024），不使用 `#[async_trait]` 宏。`#![allow(async_fn_in_trait)]` 是刻意豁免——所有 trait 有 `Send + Sync` supertrait 约束。详见 [ADR-003](adr/003-async-trait-desugaring.md)。

### 3.3 `*_inner` 测试复用模式

每个 IPC 命令有对应的 `*_inner` 函数，接收 `&AppState` 而非 `State<'_, AppState>`，集成测试可复用：

```rust
#[tauri::command]
pub async fn import_files(app, paths, state) -> Result<Vec<String>, String> {
    import_files_inner(&app, &paths, state.inner()).await
}

pub async fn import_files_inner<R: Runtime>(
    app: &AppHandle<R>, paths: &[String], state: &AppState,
) -> Result<Vec<String>, String> { ... }
```

### 3.4 懒加载 Embedder

`AppState.embedder` 使用 `tokio::sync::OnceCell`，首次调用触发模型下载（~30MB HuggingFace）。`chat` 命令调用 `embedder()` 时可能阻塞。

### 3.5 CancellationToken

每个会话一个 `CancellationToken`（tokio_util），`abort_chat` 触发后 `forward_stream()` 中的 `select! { biased; _ = abort_token.cancelled() => break, ... }` 立即中断。已生成内容保留并落库。

---

## 4. 前端架构

单文件 SPA 位于 `ui/index.html`，使用：
- Tailwind CSS（**本地构建，禁止 CDN**）
- 原生 JavaScript（无框架）
- marked.js（Markdown 渲染，本地 vendor）
- DOMPurify（XSS 防护，本地 vendor）
- highlight.js（代码高亮，本地 vendor）
- 通过 Tauri IPC (`window.__TAURI__.core.invoke()`) 与后端通信

所有前端库本地打包于 `ui/vendor/` 目录，**严禁引入 CDN 或框架**。

---

## 5. 构建配置要点

| 配置 | 值 | 原因 |
|---|---|---|
| Edition | 2024 | 需要 Rust ≥1.85 |
| Dev profile `opt-level` (deps) | 3 | ML 依赖（fastembed/ort/tokenizers）在 opt-level=0 下不可用 |
| Workspace lints | deny (unwrap/expect/panic/unreachable/todo/unimplemented) | 铁律五强制 |

---

## 6. E2E 测试模式

`ECHOMIND_DATA_DIR` 环境变量重定向数据目录（隔离测试数据）。
`ECHOMIND_E2E_TEST=1` 启用 Mock 模式：
- `e2e-tests/tests/bridge/tauri-stub.js` 通过 Playwright 注入 `window.__TAURI__` mock
- 测试在普通 Chromium 浏览器中运行（非 Tauri）
- 零网络、零 API 费用
