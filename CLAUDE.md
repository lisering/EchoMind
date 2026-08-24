# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> **⚠️ 铁律规则**：请先阅读 `.catpaw/rules/system-constraints.md`（代理设置仅限开发调研、软件本身禁止代理、TDD 驱动+模糊测试、代码注释、真实数据测试、生产级代码质量+Miri UB 检测+forbid unsafe、最新版本依赖+供应链安全工具链、文档实时同步+文档治理、开发决策互联网搜索、Rust 生态优先、GitHub 发布同步、Tailwind 样式规范、SOLID 原则、Spec-driven Development 规格驱动开发、AI 生成代码审查、结构化日志与可观测性+axe-core、LLM 应用安全防护+致命三要素+Rule of Two、错误处理与弹性设计、AI Agent 行为规范、RAG 安全防护+向量嵌入弱点）——本项目为绝对强制规则，每次新会话必须遵守。
>
> **📋 需求规格**：所有功能需求定义在 `docs/SRS_v1.0.md` 中（格式 `REQ-<域>-<序号>`，每条 AC 由 `TC-*` / `E2E-*` 测试验证），是开发、测试、验收的唯一依据。改动功能时必须同步更新对应 REQ 状态。
>
> **📚 软件资产**：`docs/SOFTWARE_ASSET.md` 是项目完整软件资产清单（架构、代码、数据流、依赖、测试、数据库、IPC），后续迭代必须同步更新。
>
> **🔄 Session 管理（全自动）**：`docs/sessions/` 是跨会话上下文管理中心。**Agent 自动行为**：会话开始时自动读 `docs/sessions/CURRENT_STATE.md` 获取快照；会话结束时自动更新 `CURRENT_STATE.md` + 追加 `LOG.md` + **无条件生成下个 Session 提示词**（`NEXT_SESSION_*.md`）。无需人工介入（铁律十二）。
>
> **🗺 版本路线图**：v1.1.0 已发布（✅ 5 功能 S44-S48）。v1.2 规划中（📋 4 功能 S49-S53：PDF 导出 / .pptx 导入 / .epub 导入 / 自定义嵌入模型上传 Pro），详见 `docs/architecture/V1.2_ATOMIC_PLAN.md`。
>
> **🔬 OpenCode 借鉴**：OpenCode v1.18.15 源码研究完成，14 项借鉴点（4 高 + 5 中 + 5 低），详见 `docs/architecture/OPENCODE_BORROW_ATOMIC_PLAN.md`。高优先级：System Context Registry / Context Epoch / 结构化压缩模板 / Prompt Caching 自动放置。
>
> See also `AGENTS.md` for architecture overview, key files, and data flow diagrams. `PROJECT_RULES.md` 定义了三条体系（纪律体系、工程体系、架构体系）的完整规则。

## Commands

| What | Command |
|---|---|
| Build all | `cargo build` |
| Build with Pro features | `cargo build --features pro` |
| Run (hot-reload) | `cargo tauri dev` |
| Test all | `cargo test` |
| Test all (Pro) | `cargo test --features pro` |
| Test single crate | `cargo test -p echomind-core` |
| Test single test | `cargo test <test_name>` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Lint (Pro) | `cargo clippy --all-targets --features pro -- -D warnings` |
| UI type check | `npx tsc --noEmit`（tsconfig.json + types/*.d.ts） |
| Frontend build | `node scripts/build-ui.mjs`（esbuild 打包 IIFE + 内联 CSS 单文件；`--dev` 外部引用） |
| UI unit tests (Vitest) | `cd e2e-tests && npm install && npm run test:unit`（403 tests，27 files） |
| E2E bridge (mock __TAURI__, Playwright) | `cd e2e-tests && npm install && npm run test:bridge` |
| E2E real GUI (WebdriverIO + tauri-driver) | `cd e2e-tests && npm run test:e2e`（**macOS 不支持**） |
| Chaos tests | `python3 tests/fixtures/generate_corpus.py && cargo test --workspace -- --nocapture --include-ignored` |
| Supply chain | `cargo audit` + `cargo deny check`（根目录 `deny.toml`） |
| Sync to GitHub publish repo | `./sync-to-github.sh` |

Crates: `echomind-models`, `echomind-prompt`, `echomind-compact`, `echomind-core`, `echomind-infra`, `echomind-tauri-app`, `license-issuer` (workspace members in `Cargo.toml`).

## Architecture

### Multi-layer hexagonal architecture

```
crates/models  →  crates/prompt  →  crates/core  →  crates/infra  →  crates/tauri-app
  (contracts)     (prompts)       (ports+logic)  (adapters)     (assembly)
                                     ↑
                              crates/compact
                              (compaction)
```

Dependency direction is strictly inward. `crates/core` must NEVER depend on Tauri, reqwest, rusqlite, or any concrete implementation. `crates/prompt` and `crates/compact` are extracted from core to improve compilation speed and module boundaries. `crates/compact` depends on `crates/core` for the `LLMProvider` trait and `bpe()` tokenizer.

### Native async trait (no `#[async_trait]` macro)

Traits in `crates/core/src/lib.rs` use native `async fn` in trait (Edition 2024). `#![allow(async_fn_in_trait)]` is a deliberate exemption — all traits have `Send + Sync` supertrait constraints, and ports are only consumed within the workspace.

### Pro feature gating (`#[cfg(feature = "pro")]`)

Both `echomind-core` and `echomind-infra` have a `pro` Cargo feature that gates optional functionality behind a compile-time flag:
- **Core (pro)**: `audit` (document consistency audit engine), `vlm_prompt` (VLM tiered chart understanding prompts)
- **Infra (pro)**: `ocrs_engine` (local OCR), `openai_vision` (VLM API adapter), `pdfium_renderer` (PDF page rendering), `local_llm` (mistral.rs v0.9.0 local LLM inference, REQ-LLM-003; Phase 2: GPU/Metal/CUDA + PagedAttention + 采样参数 + 模型卸载 + 流取消 + 预热; Phase 3: 自研 GEMV 内核 + 权重重排 + 流式预取 + RAM 预算 + KernelMode), `colbert_embedder` (multi-vector embedding), `onnx_compressor`, `symbol_engine` (tree-sitter AST 代码符号抽取, REQ-RAG-031), `wasm_executor` (代码执行沙箱, REQ-RAG-032)
- **Infra (Free, v2.2 下沉)**: `hnsw_index` (HNSW 近似最近邻索引, REQ-NFR-005 + REQ-PERF-013, 原 Pro 已下沉 Free; 二进制加密持久化 + 懒加载 + 归一化点积, REQ-PERF-015)
- **GPU feature flags**: `metal` (Apple Metal), `cuda` (NVIDIA CUDA), `accelerate` (Apple BLAS), `flash-attn` (Flash Attention 2, needs cuda), `cudnn` (cuDNN, needs cuda). These are Cargo features on `echomind-infra` and forwarded by `echomind-tauri-app`.
- Pro deps include: `pdfium-render`, `ocrs` + `rten`, `image`, `mistral.rs` v0.9.0 (local LLM, with optional `metal`/`cuda`/`accelerate` GPU features), `memmap2` (mmap GGUF), `half` (f16), `libc` (sysconf/madvise)
- Free deps (non-optional): `hnsw_rs` (HNSW, v2.2 从 Pro 下沉 Free), `rusqlite`, `r2d2`, `fastembed`, `tokio`, `rayon`, etc.

Tests in pro-gated modules use `#[cfg(all(test, feature = "pro"))]` and require `cargo test --features pro` to run.

### Core traits (crates/core/src/lib.rs)

Sixteen port traits define the entire system contract:

| Trait | Purpose |
|---|---|
| `Loader` | Parse file → plain text (markdown/text/PDF/docx/html/pptx) |
| `Splitter` | Text → ordered chunks |
| `Embedder` | Text → f32 vector (single + batch) |
| `MultiVectorEmbedder` | Text → multiple vectors (ColBERT, pro) |
| `Storage` | CRUD for documents, chunks, embeddings, conversations, messages, settings; vector search, keyword search, pagination variants, zombie cleanup |
| `LLMProvider` | SSE streaming chat → `BoxStream<'static, Result<String>>` |
| `PromptCompressor` | Context compression (RuleBasedCompressor) |
| `ResponseCache` | Semantic response caching |
| `Retriever` | Query → top-k `RetrievalResult` |
| `Reranker` | Cross-encoder rerank of retrieval results (bge-reranker-base) |
| `QueryRewriter` | Query rewriting (HyDE) |
| `PageRenderer` | PDF page → PNG bytes (pro feature, pdfium-based) |
| `OcrEngine` | Image bytes → recognized text (pro feature, ocrs/rten-based) |
| `VisionLanguageModel` | Image bytes + prompt → structured description (pro feature, BYOK LLM endpoint) |
| `DomainClassifier` | Document → one of 16 domains via embedding centroid classification (REQ-VEC-013) |
| `CodeExecutor` | Execute code snippet → `ExecutionResult` (pro feature, sandbox with timeout/memory/network limits, REQ-RAG-032) |

`NoVlm` is a zero-sized `VisionLanguageModel` impl that always returns empty string — used when VLM is disabled.

### Storage: SqliteStorage

`crates/infra/src/sqlite_storage.rs` — production storage:
- `rusqlite` with WAL mode + busy_timeout, r2d2 connection pool
- All DB operations via `tokio::task::spawn_blocking`
- Vectors stored as BLOB (raw f32 bytes)
- Vector search: HNSW index (auto-enabled > 500 vectors, O(log n)) or full-scan with Arc snapshot cache (zero-copy, REQ-PERF-016)
- Normalized dot product (REQ-PERF-017): vectors pre-normalized on load, 3× FLOP reduction vs cosine_similarity
- Vector snapshot cache: `Arc<Vec<(chunk_id, Vec<f32>)>>` shared clone, invalidated on write (REQ-PERF-016, replaced LruVectorCache 5000 truncation bug)
- FTS5 full-text index for `keyword_search()` (BM25 ranking)
- Tables: documents, chunks, embeddings, conversations, messages, settings
- Cascading deletes (ON DELETE CASCADE)
- Crash recovery: `cleanup_zombies()` on startup marks Processing → Failed
- Pagination: `list_documents_paginated`, `list_conversations_paginated`, `list_messages_paginated` with SQL LIMIT/OFFSET
- Implements `AuditLogger` (hash-chain audit logs) and SQLCipher encrypted mode (`new_encrypted`)

### Embedder: LocalEmbedder

`crates/infra/src/local_embedder.rs` — fastembed/ONNX Runtime:
- Models: 5 EmbeddingModel variants — `AllMiniLML6V2` (384-dim, default), `BgeSmallEnV1_5` (384-dim, recommended), `BgeSmallZhV1_5` (512-dim, Chinese), `E5SmallV2` (384-dim), `BgeM3` (1024-dim, multilingual, REQ-VEC-015)
- Runtime model switching via `EmbeddingModel` enum + `ModelManager` store
- Lazy init via `tokio::sync::OnceCell` in AppState
- Model download from HuggingFace on first use
- Batch embedding support (`embed_batch`)
- Parallel inference pool (`rayon` thread pool) for large batch throughput

### HNSW Index (Free, v2.2 下沉)

`crates/infra/src/hnsw_index.rs` — approximate nearest neighbor search (Free, 原 Pro 已下沉):
- Pure Rust `hnsw_rs` crate (Malkov & Yashunin 2016/2018 algorithm)
- Reduces vector search from O(n) brute-force to O(log n), auto-enabled > 500 vectors (`HNSW_AUTO_THRESHOLD`)
- Cosine distance metric, 32 max connections, 16 max layers
- Binary encrypted persistence (REQ-PERF-015): AES-256-GCM encrypted binary format + lazy load from disk + crash consistency
- `storage::crypto` module for encrypt/decrypt HNSW index files

### LLM: OpenAIProvider / LocalLlmEngine

`crates/infra/src/openai_provider.rs` — SSE streaming:
- OpenAI Chat Completions compatible API
- Stream parsed by `crates/core/src/stream_parse.rs`
- Connection timeout 30s, overall timeout 5min
- Supports local Ollama (empty API key)

`crates/infra/src/local_llm.rs` — local LLM inference (Pro feature, REQ-LLM-003):
- Based on mistral.rs v0.9.0 (pure Rust)
- Implements the same `LLMProvider` trait as `OpenAIProvider`
- Phase 2: GPU acceleration via `best_device()` (Metal/CUDA feature flags), PagedAttention KV cache, sampling parameters (temperature/top_p/top_k/max_tokens/penalty), model unload/switch (RwLock<Option<Arc<Model>>>), stream cancellation (CancellationToken), model warm-up
- Phase 3: 自研 GEMV 内核模式（KernelMode::CustomGemv）+ GGUF 解析（mmap 零拷贝 `gguf_reader.rs`）+ 权重重排（`weight_repack.rs` Tile-Major）+ 流式预取（`layer_prefetch.rs` madvise）+ RAM 预算管理（`memory_budget.rs` LRU）+ KV cache 持久化（`kv_cache.rs`）
- Lazy model loading via `AppState.local_llm()` (first call triggers GGUF load in `spawn_blocking`)
- Emits `model_load_progress` event during load (loading/ready/error)
- `chat_inner` routes to `LocalLlmEngine` when `llm_mode == Local`
- `warm_up()` called after `set_llm_mode` / `set_local_model` to pre-load model in background
- Free version: `LlmMode::Local` returns error (Pro gate)

`crates/infra/src/model_manager.rs` — GGUF model file management (REQ-LLM-004):
- Download from HuggingFace with `.tmp` → atomic rename pattern
- List / delete / path management with path traversal protection
- 4 recommended models: Qwen2.5-3B/7B, Llama-3.2-3B, Phi-3.5-mini
- Emits `model_download_progress` event during download

### Splitter ecosystem

Three splitter implementations, each serving different use cases:

| Splitter | File | Strategy |
|---|---|---|
| `TextSplitter` | `crates/core/src/splitter.rs` | Token-window (tiktoken-rs cl100k_base), configurable size + overlap |
| `SemanticSplitter` | `crates/core/src/semantic_splitter.rs` | Recursive degradation: paragraph → sentence → clause → token window. Recognizes Chinese punctuation, preserves code blocks. Default 256 tokens / 32 overlap |
| `SectionAwareSplitter` | `crates/core/src/section_aware_splitter.rs` | Markdown heading hierarchy → section boundaries. Each chunk carries its section path prefix (e.g., "## Installation > ### Prerequisites") |

### Retrieval pipeline

| Component | File | Strategy |
|---|---|---|
| `VectorRetriever` | `crates/core/src/retriever.rs` | Pure cosine similarity vector search |
| `HybridRetriever` | `crates/core/src/hybrid_retriever.rs` | Vector + keyword (BM25) → RRF fusion (k=60). Runtime toggle via `set_hybrid_enabled(bool)`. Keyword channel weight 0.8, vector channel 1.0 |
| `LocalReranker` | `crates/infra/src/local_reranker.rs` | bge-reranker-base Cross-Encoder rerank (REQ-RAG-020) |
| `HydeRewriter` | `crates/infra/src/hyde_rewriter.rs` | HyDE query rewriting via non-streaming LLM call; graceful degradation to original query on failure/timeout (REQ-RAG-021) |

### Engines (crates/core)

| Engine | File | Purpose |
|---|---|---|
| `ChatEngine` | `chat.rs` | RAG query orchestration (retrieve → expand → prompt → stream) |
| `CoordinatorEngine` | `coordinator.rs` | Multi-agent coordination: Research→Synthesis→Implementation→Verification (REQ-RAG-025) |
| `AgentEngine` | `agent.rs` | ReAct agentic RAG (REQ-RAG-022): Thought/Action/Observation loop, parallel tool execution (search_kb / list_documents), max 5 iterations, graceful degradation to standard RAG |
| `AutoDream` | `auto_dream.rs` | Background idle tidying: duplicate detection (exact + embedding similarity), cross-document contradiction discovery, organization suggestions. Reuses `Embedder` + `Storage` + `LLMProvider` ports, zero new deps |
| `ImportService` | `import.rs` | File import + indexing + embedding pipeline, standard + multimodal PDF paths |
| `SyncEngine` | `sync.rs` | Folder watch incremental sync (REQ-SYNC-002): add/update/delete vs knowledge base by MD5 + source path. Idempotent. Wraps `ImportService` |
| `ExportService` | `export.rs` | Conversation export to Markdown (REQ-EXP-001), includes sources list per assistant message |
| `DomainClassifier` | `domain.rs` | Embedding centroid classification into 16 domains (REQ-VEC-013), zero LLM cost, fully offline |
| `AuditEngine` | `audit.rs` | Full-scan document consistency audit (pro feature) |

### Frontend

Source of truth is **modular ES modules in `ui/src/`** (50 files, vanilla JS, no framework). `ui/index.html` is a **build artifact**: `node scripts/build-ui.mjs` uses **esbuild** to bundle `ui/src/main.js` → IIFE format and inlines CSS into a single file (prod, works under `file://`) or emits external refs (`--dev`, for browser development). **Never hand-edit the inlined bundle — edit `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json` and rebuild.**

- TypeScript type checking: `tsconfig.json` at root with `checkJs: true`; `types/tauri.d.ts` (Window.__TAURI__), `types/vendor.d.ts` (marked/DOMPurify/mermaid/hljs/katex/Chart), `types/dom-ext.d.ts` (Element/HTMLElement/Node/Event extensions). Run `npx tsc --noEmit` to check (42 residual errors are code-level type mismatches for gradual migration).

- Tailwind CSS (local JIT build, **no CDN**), marked.js, DOMPurify, highlight.js — all locally vendored in `ui/vendor/`
- i18n via `ui/locales/{en,zh-CN}.json` (REQ-I18N-001)
- **Styling iron rule (铁律十三)**: use Tailwind utility classes + design tokens (`bg-surface-2`, `text-text-primary`, `border-line`, `text-accent` mapped from CSS variables in `ui/styles/tokens.css`). Custom CSS only for `@keyframes`, pseudo-elements, parent-state selectors, browser-specific props, third-party overrides, and the tokens themselves. No hardcoded hex colors, no `display:flex`-style rules Tailwind can express.
- DeepSeek-style modules: `thinking-panel.js` (collapsible CoT panel), `input-toggles.js` (hybrid search / agent mode toggles)
- Communicates with backend via Tauri IPC (`window.__TAURI__.core.invoke()`)
- UI unit tests run under Vitest (`e2e-tests`, `npm run test:unit`)

### License issuer

`crates/license-issuer/` — standalone CLI for generating signed Pro licenses (Ed25519 keypair; `genkey` / `issue`). Not part of the main app binary; used by the project maintainer to issue licenses to Pro users. Output format `base64(payload)-base64(signature)` verified by `verify_license()` in `crates/core/src/license.rs` (REQ-LIC-005).

## Data flows

### Document import (standard)

```
import_files IPC → ImportService.import_one()
  → Loader.load() (markdown/text/docx/html/pptx/epub parsing; PDF multimodal: pdfium+ocrs+VLM)
  → MD5 hash → duplicate check (find_document_by_hash)
  → Splitter.split() (token-based or semantic chunking)
  → Storage.add_document() + add_chunk() (SQLite)
  → Embedder.embed_batch() (fastembed/ONNX, parallel inference pool)
  → Storage.add_embedding() (vector BLOB)
  → doc-status-changed event → frontend list update
  → embedding_progress event per micro-batch (large docs)
```

### Document import (multimodal PDF, pro feature)

```
import_files IPC → ImportService.import_one() (multimodal path)
  → Loader.load() → MultimodalPdfLoader
    → Phase 1: Text layer extraction (lopdf, embedded text)
    → Phase 2: PageRenderer.render_page() → PNG bytes (pdfium, per-page)
    → Phase 3: OcrEngine.recognize() → OCR text (ocrs, local PaddleOCR PP-OCRv4)
    → Phase 4 (optional): VisionLanguageModel.describe_image() → VLM structured text
      (tables→Markdown, flowcharts→Mermaid, charts→CSV+trend; VLM_TIERED_PROMPT)
    → Merge text + OCR + VLM layers into unified content
  → Splitter.split() → Storage → Embedder (same as standard path)
  → doc-status-changed event with sub_phase progress (text_extracting/image_rendering/ocr/vlm_enhancing)
```

### RAG query

```
chat IPC → chat_inner()
  → empty KB check (count_documents == 0 → error)
  → load LLM config (api_key, base_url, model)
  → chat_phase "preparing" event → init embedder (lazy, first-use download)
  → create/update conversation
  → ChatEngine.chat()
    → HybridRetriever.retrieve() (if hybrid_search enabled)
      → Embedder.embed(query) → Storage.vector_search(cosine sim, top-k=5)
      → Storage.keyword_search(BM25, top-k=5)
      → rrf_fuse() merge
    → threshold filter (score > 0.1)
    → chunk expansion: expand to adjacent chunks (±1) for context (REQ-RAG Phase 4)
    → build RAG prompt → LLMProvider.chat_stream(SSE)
  → chat_sources event → frontend shows references
  → chat_phase "generating" event
  → chat_token events → frontend typing effect
  → abort_token check (CancellationToken)
  → chat_done event
  → persist_exchange() → Storage.add_message() (user + assistant)
  → auto-extract title on first exchange
```

### Agentic RAG (REQ-RAG-022)

```
AgentEngine.chat() — ReAct loop:
  → LLM generates Thought + Action(s) (or Final Answer)
  → parallel tool execution (search_kb / list_documents) via join_all
  → Observation(s) feed next round; max 5 iterations
  → parse failure → degrade to standard RAG single retrieval
  → each step emits agent_step event
```

### Stream cancellation

```
abort_chat IPC → AppState.abort_chat()
  → CancellationToken.cancel()
  → forward_stream() loop breaks on select! biased branch
  → emit "⏹ 生成已中断" + chat_done
  → persist_exchange() saves partial content
  → clear_abort() cleans up token
```

### License & quota

```
import_files → ImportService.import_one()
  → check is_pro (from AppState, read from settings table)
  → if !is_pro: count_documents() >= 50 → reject with quota error
  → if !is_pro: file is .pdf → reject with paywall error
  → if is_pro: verify_license(key) → activate_pro() → persist + refresh runtime
```

### Security & encryption

```
encrypt_database IPC → SecurityManager.encrypt_database(password)
  → Argon2id derive_key(password, random_salt) → 32-byte key
  → SqliteStorage new_encrypted(key) → PRAGMA key (SQLCipher AES-256)
  → state: Unencrypted → EncryptedUnlocked
  → SecureString wipe password from memory (zeroize)

unlock_database IPC → SecurityManager.unlock(password)
  → Argon2id verify → PRAGMA key
  → brute-force check (5 attempts → exponential backoff)
  → state: Locked → EncryptedUnlocked

Auto-lock (idle) → SecurityManager.check_auto_lock()
  → elapsed > auto_lock_timeout → state → Locked
  → emit security-state-changed event → frontend lock overlay

PII detection → PrivacyManager.redact_text(text)
  → regex scan 8 PII types → replace with [REDACTED-TYPE]
  → AuditLogger.log(AuditEvent) → SQLite audit_logs table
  → hash-chain: entry_hash = SHA256(prev_hash || entry_data)

Audit verification → verify_audit_chain()
  → replay hash-chain → detect tampering (hash mismatch)
```

### Local LLM inference (pro feature)

```
set_llm_mode IPC → AppState.set_llm_mode("local")
  → persist to settings table (llm.mode = "local")
  → Free user: reject with PRO_REQUIRED error

chat IPC (LlmMode::Local) → chat_inner()
  → load selected model filename from settings (llm.local_model)
  → AppState.local_llm() → LocalLlmEngine::new(model_path)
    → emit model_load_progress { status: "loading" }
    → mistral.rs load GGUF (spawn_blocking, first call only)
    → emit model_load_progress { status: "ready" | "error" }
  → LocalLlmEngine.chat_stream() → BoxStream<Result<String>>
    → same forward_stream() loop as OpenAIProvider
    → chat_token events → frontend typing
    → chat_done event

Model download flow:
  download_model IPC → ModelManager.download_model(url, filename)
    → write to .tmp file → atomic rename
    → emit model_download_progress { Downloading { current, total } }
    → emit model_download_progress { Done }
  → frontend updates progress bar
```

### Folder sync (REQ-SYNC-002/003)

```
watch_folder IPC → FileWatcher (notify + notify-debouncer-full, 500ms debounce)
  → sync_folder IPC → SyncEngine.sync()
    → scan folder vs knowledge base (md/txt/pdf/docx/html/pptx, skip hidden files)
    → new file → ImportService.import_one()
    → changed (MD5 differs) → delete old doc + re-import
    → deleted on disk → cascade delete
  → sync_progress events (started/syncing/completed)
```

### Knowledge graph (REQ-RAG-026/027)

```
导入管线集成:
import_files → ImportService.import_one()
  → Loader.load() → Splitter.split() → Storage.add_document() + add_chunk()
  → Embedder.embed_batch() → Storage.add_embedding()
  → EntityExtractor.extract(text) → Storage.add_entities()           ← 实体索引
  → EntityExtractor.extract_relations(text, chunk_id)                 ← 关系抽取（8 种）
  → Storage.add_relations_batch()                                      ← entity_relations 表

检索管线集成:
chat IPC → chat_inner()
  → (if rag.graph_retriever_enabled)
  → GraphRetriever.expand(query, top_k=5)
    → EntityExtractor.extract(query) → query_entities
    → BFS: Storage.get_relations_for_entity(entity) (max 2-hop, distance decay)
    → Storage.get_chunk_by_id(chunk_id) → RetrievalResult
  → rrf_fuse_four_way(vector, keyword, entity, graph, top_k)
    → Vector=1.0, BM25=0.8, Entity=0.6, Graph=0.5

可视化与分析:
openGraphViewer → get_graph_data() → D3 forceSimulation
  → 节点交互 + 8 种关系颜色编码 + 5 种实体类型 SVG 图标
  → 子图过滤 + 搜索定位 + SVG/PNG 导出
  → 布局切换（force/hierarchical/radial）
  → 路径分析: get_shortest_path(from, to) → BFS → 高亮
  → 社区检测: get_communities() → Label Propagation → 10 色着色
```

## Key files

- `crates/tauri-app/src/lib.rs` — Tauri command setup + `run()` (150 IPC commands registered via `generate_handler!`, 16 pro-gated)
- `crates/tauri-app/src/commands/mod.rs` — Command 模块根（共享类型 LlmProvider / 常量 / 辅助函数 emit_* / `pub use` 重导出 + `#![allow(clippy::unreachable)]`）
- `crates/tauri-app/src/commands/` — 14 功能域子模块（S65 SRP 重构，7706 行 → 14 文件 ≤ 1600 行 each）：`chat.rs` / `import.rs` / `document.rs` / `conversation.rs` / `audit.rs` / `license.rs` / `settings.rs` / `local_llm.rs` / `security.rs` / `agent.rs` / `performance.rs` / `graph.rs` / `sync.rs` / `features.rs`。各子模块 `use super::*` 访问共享导入，`pub use module::*` 重导出保持 `lib.rs` `generate_handler!` 不变
- `crates/tauri-app/src/state.rs` — AppState (storage, data_dir, llm_config, is_pro, lazy embedder/ocr/vlm, abort_tokens, hybrid_search toggle, llm_mode, local_llm (Pro), model_manager, reranker, dream_engine, _log_guard (REQ-OBS-001))
- `crates/tauri-app/src/stores/` — Store 模式状态模块（chat_store / config_store / document_store / model_store / security_store）
- `crates/core/src/lib.rs` — All port traits (Loader, Splitter, Embedder, Storage, LLMProvider, Retriever, Reranker, PageRenderer, OcrEngine, VisionLanguageModel, MultiVectorEmbedder, DomainClassifier, PromptCompressor, ResponseCache, QueryRewriter)
- `crates/prompt/src/lib.rs` — Prompt building: SegmentedPrompt, build_rag_prompt_segmented, build_agent_prompt, build_final_rag_prompt
- `crates/compact/src/lib.rs` — CompactionEngine (LLM-based history summarization)
- `crates/core/src/chat.rs` — ChatEngine (RAG query orchestration)
- `crates/core/src/coordinator.rs` — CoordinatorEngine (multi-agent coordination, REQ-RAG-025)
- `crates/core/src/agent.rs` — AgentEngine (ReAct agentic RAG, REQ-RAG-022)
- `crates/core/src/auto_dream.rs` — AutoDream (background idle tidying engine)
- `crates/core/src/sync.rs` — SyncEngine (folder incremental sync, REQ-SYNC-002)
- `crates/core/src/export.rs` — ExportService (conversation → Markdown, REQ-EXP-001)
- `crates/core/src/domain.rs` — DomainClassifier (16-domain embedding centroid classification, REQ-VEC-013)
- `crates/core/src/import.rs` — ImportService (file import + indexing + embedding pipeline, standard + multimodal PDF paths)
- `crates/core/src/retriever.rs` — VectorRetriever
- `crates/core/src/hybrid_retriever.rs` — HybridRetriever (vector + keyword RRF fusion)
- `crates/core/src/loader.rs` — Markdown/Text/PDF/Docx/Html/Pptx/Epub loaders + MultimodalPdfLoader (pro)
- `crates/core/src/splitter.rs` — Token-based TextSplitter (tiktoken-rs)
- `crates/core/src/semantic_splitter.rs` — SemanticSplitter (paragraph→sentence→clause recursive)
- `crates/core/src/section_aware_splitter.rs` — SectionAwareSplitter (markdown heading hierarchy)
- `crates/core/src/stream_parse.rs` — SSE stream parser
- `crates/core/src/license.rs` — License verification (Ed25519)
- `crates/core/src/encryption.rs` — Argon2id KDF, PBKDF2 fallback, SecureString (zeroize), password strength assessment
- `crates/core/src/privacy.rs` — PII detector (8 types), PrivacyManager, AuditLogger trait (hash-chain)
- `crates/core/src/security.rs` — SecurityManager state machine, auto-lock, clipboard clear, brute-force protection
- `crates/core/src/audit.rs` — AuditEngine (full-scan document consistency audit, pro feature)
- `crates/core/src/vlm_prompt.rs` — VLM tiered prompt (4-level chart understanding, pro feature)
- `crates/core/src/errors.rs` — 统一错误类型
- `crates/infra/src/sqlite_storage.rs` — SQLite with WAL mode, r2d2 pool, FTS5, vector BLOB storage, AuditLogger impl (hash-chain), SQLCipher encrypted mode (`new_encrypted`)
- `crates/infra/src/local_embedder.rs` — fastembed/ONNX embedding (5 models: AllMiniLML6V2/BgeSmallEnV1_5/BgeSmallZhV1_5/E5SmallV2/BgeM3, 384~1024-dim, parallel inference, runtime switchable)
- `crates/infra/src/local_reranker.rs` — fastembed/ONNX Cross-Encoder reranking (bge-reranker-base, REQ-RAG-020)
- `crates/infra/src/hyde_rewriter.rs` — HyDE query rewriting adapter (REQ-RAG-021)
- `crates/infra/src/openai_provider.rs` — OpenAI-compatible SSE streaming
- `crates/infra/src/local_llm.rs` — mistral.rs v0.9.0 local LLM inference engine (Pro feature, REQ-LLM-003)
- `crates/infra/src/gguf_reader.rs` — GGUF 文件解析器（mmap 零拷贝，V1/V2/V3，Pro feature，REQ-LLM-007）
- `crates/infra/src/quant_blocks.rs` — 量化块结构 + 反量化（BlockQ4_0/BlockQ4K/BlockQ8_0/BlockQ8K，Pro feature，REQ-LLM-007）
- `crates/infra/src/gemv_kernel.rs` — 自研 GEMV 内核（4 种量化格式，标量 + AVX2 预留，Pro feature，REQ-LLM-007）
- `crates/infra/src/weight_repack.rs` — CPU cache 友好权重重排（Row-Major → Tile-Major，Pro feature，REQ-LLM-007）
- `crates/infra/src/layer_prefetch.rs` — Layer 级流式预取（madvise + 后台线程，Pro feature，REQ-LLM-008）
- `crates/infra/src/memory_budget.rs` — RAM 预算管理（LRU 驱逐 + libc::sysconf，Pro feature，REQ-LLM-008）
- `crates/infra/src/kv_cache.rs` — KV cache 序列化/反序列化磁盘格式（Pro feature）
- `crates/infra/src/model_manager.rs` — Local GGUF model file management (download/list/delete, REQ-LLM-004)
- `crates/infra/src/file_watcher.rs` — 文件监听器（notify + debouncer，REQ-SYNC-003）
- `crates/infra/src/local_logger.rs` — 本地日志系统（REQ-OBS-001）：tracing 集成 + JSON Lines + 日期轮转 + 运行时级别控制 + 诊断信息导出（REQ-OBS-003）
- `crates/infra/src/hnsw_index.rs` — HNSW approximate nearest neighbor index (Free, v2.2 下沉, 二进制加密持久化 + 懒加载, REQ-PERF-015)
- `crates/infra/src/ocrs_engine.rs` — Local OCR engine (ocrs + rten, PaddleOCR PP-OCRv4, pro feature)
- `crates/infra/src/pdfium_renderer.rs` — PDF page renderer (pdfium, pro feature)
- `crates/infra/src/openai_vision.rs` — VLM API adapter (OpenAI-compatible Vision, pro feature)
- `crates/models/src/lib.rs` — Data model definitions (Document, Chunk, ChatMessage, Conversation, LlmMode, ModelInfo, CacheLevel, CacheStats, CacheSettingsPayload, Proposition, SummaryNode, Entity, MemoryRecord, SyncProgressPayload, etc.)
- `crates/core/src/cache.rs` — 缓存逻辑：normalize_query / query_hash / cosine_similarity / is_expired (REQ-PERF-001)
- `crates/core/src/prompt_compressor.rs` — RuleBasedCompressor: 停用词去除 + Markdown 装饰去除 + 句子级信息密度评分 (REQ-PERF-002)
- `crates/core/src/progressive_injector.rs` — ProgressiveInjector 状态机 + detect_insufficient_info() (REQ-PERF-010)
- `crates/core/src/speculative_rag.rs` — SpeculativeRagEngine<D, L> 泛型引擎 + text_similarity() (REQ-PERF-011)
- `crates/core/src/retrieval_memory.rs` — RetrievalMemoryEngine + QueryType + RetrievalMethod + classify_query_type() (REQ-PERF-012)
- `crates/core/src/proposition_splitter.rs` — PropositionSplitter: 句子分割 + 代词消解 + 上下文补全 (REQ-PERF-007)
- `crates/core/src/summary_tree.rs` — SummaryTreeBuilder: RAPTOR 多级摘要树 (REQ-PERF-009)
- `crates/core/src/entity_extractor.rs` — EntityExtractor: NER 纯规则抽取 + extract_relations() 实体关系抽取（8 种关系类型，零 LLM，REQ-RAG-026）
- `crates/core/src/graph_retriever.rs` — GraphRetriever<S>: 知识图谱图遍历检索器 + rrf_fuse_four_way() 四路 RRF 融合（REQ-RAG-027）
- `crates/core/src/graph_analyzer.rs` — GraphAnalyzer: 图分析引擎（shortest_path BFS + all_paths DFS + detect_communities Label Propagation + degree_centrality，REQ-RAG-027 AC-32~40）
- `crates/core/src/benchmark_tests.rs` — 性能基准测试 TC-BENCH-001~005 (Session 10)
- `crates/infra/src/sqlite_cache.rs` — SqliteCache: 三级缓存 SQLite 实现 (REQ-PERF-001)
- `crates/infra/src/onnx_compressor.rs` — OnnxCompressor: ONNX 嵌入余弦相似度压缩 (Pro, REQ-PERF-002)
- `crates/infra/src/colbert_embedder.rs` — ColbertEmbedder: ColBERT 多向量嵌入 (Pro, REQ-PERF-008)
- `crates/infra/src/symbol_engine.rs` — SymbolEngine: tree-sitter AST 代码符号抽取（REQ-RAG-031，Pro 门控，支持 Rust/TS/Python/Go）
- `crates/core/src/code_executor.rs` — 代码执行沙箱核心实现（MockExecutor + NoExecutor + format_execution_result，REQ-RAG-032）
- `crates/core/src/memory_store.rs` — 持久化记忆系统引擎（MemoryStore + MemoryRetriever trait，IfAI 三层记忆 Wing/Hall/Room + Bamboo-agent 上下文压缩，REQ-RAG-033）
- `crates/core/src/web_search.rs` — 网页搜索融合引擎（阈值触发判断 + RRF 融合本地与 Web 结果，REQ-RAG-036）
- `crates/infra/src/duckduckgo_provider.rs` — DuckDuckGo Instant Answer API 适配器（REQ-RAG-036）
- `crates/infra/src/wasm_executor.rs` — WasmExecutor: 本地进程安全执行代码片段（Pro 门控，python3/node + 超时/内存限制/临时目录隔离，REQ-RAG-032）
- `crates/license-issuer/` — Standalone CLI for generating signed Pro licenses
- `ui/src/` — 前端模块化源码（50 个 ES module；`thinking-panel.js`、`input-toggles.js`、`chat-render.js`、`settings-local-llm.js`、`download-manager.js`、`code-executor.js`、`workflow-panel.js`、`memory-panel.js`、`dream-panel.js`、`symbol-search.js`、`perf-settings.js`、`export.js` 等）
- `ui/src/download-manager.js` — 下载管理器面板（REQ-LLM-004 v2：暂停/恢复/取消 + 崩溃恢复 + model_download_progress 事件统一监听）
- `ui/src/graph-viewer.js` — 知识图谱 D3.js v7 可视化（force-directed graph + 节点交互 + 实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 布局切换 + 路径分析 + 社区检测着色，REQ-RAG-027）
- `ui/src/workflow-panel.js` — DAG 工作流可视化面板（模板 CRUD + 运行 + 进度，REQ-RAG-030）
- `ui/src/memory-panel.js` — 持久化记忆管理面板（三列 tier 布局 Wing/Hall/Room + promote/delete/clear/pin，REQ-RAG-033）
- `ui/src/dream-panel.js` — AutoDream 知识库整理面板（触发/中止/历史建议，REQ-RAG-029）
- `ui/src/symbol-search.js` — 代码符号搜索面板（tree-sitter AST 符号搜索 + 重建索引，REQ-RAG-031）
- `ui/src/perf-settings.js` — 性能优化设置面板（缓存/压缩比/检索记忆/渐进式/Speculative/质量门控/图谱检索 toggle，REQ-PERF-001~012）
- `ui/src/export.js` — PDF 导出模块（window.print() 零依赖，复用 Markdown 渲染管线 + 隐藏 iframe + @media print CSS，REQ-EXP-005）
- `ui/styles/print.css` — 打印样式（@media print 规则：隐藏 UI 元素 + serif 字体 + 分页控制，REQ-EXP-005）
- `ui/src/zindex.js` — 统一 Z-index 常量表 + zClass() 辅助函数（12 层级，禁止硬编码 z-index）
- `ui/src/panel-stack.js` — 全局面板栈管理器（push/remove/closeTop/peek/isOpen/hasOpen/clear，ESC 键统一关闭栈顶面板）
- `ui/src/input-keymap.js` — 统一输入区 keydown 处理器（斜杠命令导航 > Enter 发送 > 历史导航 > Escape 重置）
- `ui/src/input-history.js` — 输入历史导航 + 草稿持久化 + Token 估算（会话级历史 + 切换会话保存/恢复草稿）
- `ui/index.html` — Frontend SPA 构建产物（`node scripts/build-ui.mjs` esbuild 打包生成；**不要手改**）
- `ui/styles/tokens.css` — CSS 变量三层架构（Static → Alias → Component）+ DeepSeek 风格设计令牌
- `ui/styles/components.css` — DeepSeek 风格组件样式
- `ui/locales/{en,zh-CN}.json` — i18n 语言包（REQ-I18N-001）
- `scripts/build-ui.mjs` — 前端构建脚本（esbuild 打包 IIFE + 内联 CSS 单文件；`--dev` 外部引用）
- `tsconfig.json` — TypeScript checkJs 配置（`npx tsc --noEmit` 类型检查）
- `types/dom-ext.d.ts` / `types/tauri.d.ts` / `types/vendor.d.ts` — 全局类型声明（DOM 扩展 / Tauri IPC / 第三方库）
- `docs/SRS_v1.0.md` — 需求规格唯一依据（3736 行，v2.3.0）
- `docs/api/` — 接口契约文档（ipc-commands / code-contracts / database-schema / settings-reference / frontend-modules）
- `.catpaw/rules/system-constraints.md` — 铁律规则（强制）

## Key quirks

- **Edition 2024** in workspace `Cargo.toml` — requires Rust ≥1.85
- **Dev profile**: `[profile.dev.package."*"] opt-level = 3` — ML deps unusably slow at opt-level=0
- **Lazy embedder**: ONNX model NOT loaded at startup. `chat` command may block on first call (model download)
- **`*_inner` pattern**: Each IPC command has a corresponding `*_inner` function that takes `&AppState` instead of `State<'_, AppState>`. This allows integration tests to reuse the same logic with a mock runtime.
- **`ECHOMIND_DATA_DIR`**: Environment variable to redirect data directory for E2E test isolation
- **API Key masking**: `get_settings` returns `api_key_masked` (**** + last 4), never plaintext
- **Phase events**: `chat_phase` event (preparing/retrieving/generating) pushes progress before first token, eliminating blank wait
- **Embedding progress events**: `embedding_progress` event per micro-batch during large-document vectorization
- **`DocStatusPayload.sub_phase`**: Multimodal PDF import emits sub-phase progress (text_extracting/image_rendering/ocr/vlm_enhancing)
- **Hybrid search toggle**: `SettingsPayload.hybrid_search` (default true) controlled from settings UI
- **VLM toggle**: `SettingsPayload.vlm_enabled` (default false) — when enabled, image bytes are sent to user's BYOK LLM endpoint
- **UI is a build artifact**: edit `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json`, then `node scripts/build-ui.mjs` (esbuild bundles IIFE) — never hand-edit the inlined bundle
- **TypeScript type checking**: `tsconfig.json` at root with `checkJs: true`; `types/*.d.ts` global declarations (DOM extensions, Tauri IPC, vendor libs). Run `npx tsc --noEmit` to check (42 residual errors for gradual migration)
- **esbuild bundling**: `scripts/build-ui.mjs` uses esbuild to bundle `ui/src/main.js` → IIFE format. No more manual MODULE_ORDER — esbuild resolves imports and tree-shakes
- **CI**: `.github/workflows/e2e.yml` runs Playwright E2E tests
- **Workspace lints**: `Cargo.toml` has `[workspace.lints.clippy]` with `unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented` all set to `"deny"`. New crates must inherit via `[lints] workspace = true`.
- **commands.rs exemption**: `#![allow(clippy::unreachable)]` because `#[tauri::command]` macro generates unreachable!() internally. Hand-written code in this file still must not use it — enforced by code review.
- **FTS5 keyword search**: SQLite FTS5 virtual table enables BM25-ranked full-text search for hybrid retrieval
- **Chunk expansion**: After retrieval, adjacent chunks (±1) are included in the RAG context to provide surrounding context
- **Security layer**: `SecurityManager` (`crates/core/src/security.rs`) manages encryption state machine (Unencrypted → EncryptedUnlocked → Locked). `crates/core/src/encryption.rs` provides Argon2id KDF (m=19456KB, t=2, p=1) + PBKDF2 fallback + `SecureString` (zeroize on drop). `crates/core/src/privacy.rs` detects/redacts 8 PII types + `AuditLogger` trait (hash-chain: `entry_hash = SHA256(prev_hash || entry_data)`). `SqliteStorage` implements `AuditLogger`.
- **SQLCipher encryption**: `SqliteStorage::new_encrypted()` opens DB with `PRAGMA key` (SQLCipher AES-256-CBC transparent encryption). Key derived from user password via Argon2id.
- **Auto-lock**: `SecurityManager` runs a background timer; idle timeout → `Locked` state → frontend lock overlay. `record_activity()` resets timer.
- **Brute-force protection**: 5 failed unlock attempts → exponential backoff (30s base, doubles each failure).
- **Clipboard auto-clear**: Sensitive clipboard content auto-cleared after configurable timeout (default 30s).
- **Security IPC commands**: 13 commands — `get_security_status`, `encrypt_database`, `unlock_database`, `lock_app`, `set_auto_lock_timeout`, `detect_pii`, `redact_pii`, `get_audit_logs`, `verify_audit_chain`, `set_pii_detection_enabled`, `clear_clipboard`, `set_clipboard_clear_timeout`, `check_security_status`
- **E2E mock**: bridge tests inject `e2e-tests/tests/bridge/tauri-stub.js` via `page.addInitScript`（mock `window.__TAURI__`，无需环境变量开关）。`E2E_SPEED` 倍率缩放 mock 延迟（CI 用 0.2）。真实 GUI 模式（`npm run test:e2e`）经 `ECHOMIND_DATA_DIR` 隔离数据目录，**不支持 macOS**。

## 文档实时同步（铁律七）

代码变更必须同步更新文档，文档与代码脱节是最大威胁：

| 代码变更类型 | 必须同步更新的文档 |
|---|---|
| 新增/修改 IPC 命令 | `docs/api/ipc-commands.md`（同一 PR 内） |
| 新增/修改数据库表 | `docs/api/database-schema.md`（同一 PR 内） |
| 新增/修改 trait 方法、数据模型、AppState 字段 | `docs/api/code-contracts.md`（同一 PR 内） |
| 新增前端 JS 文件 | `docs/api/frontend-modules.md`（同一 PR 内） |
| 新增 settings 键 / Tauri 事件 | `docs/api/settings-reference.md`（同一 PR 内） |
| 功能实现/补齐 | `docs/SRS_v1.0.md` 对应 REQ 状态（同一 PR 内） |
| 新增架构决策 | `docs/architecture/adr/` 新建 ADR（同一 PR 内） |

AI Agent 开发前必读文档优先级：`PROJECT_RULES.md` → `.catpaw/rules/system-constraints.md` → `docs/api/code-contracts.md` → `docs/api/ipc-commands.md` → `docs/api/database-schema.md` → `AGENTS.md`。

## GitHub 发布同步（铁律十一）

双仓库工作流：开发在根目录，`EchoMind/EchoMind/` 是只含打包编译必需文件的 GitHub 发布仓库。**修改任何 `.rs` / `.html` / `.css` / `.js` / `.json` / `.toml` 源文件或新增/删除源文件后，必须立即运行 `./sync-to-github.sh` 同步**，并在每次会话结束前执行。不同步的文件：`docs/`、`tests/fixtures/`、`e2e-tests/`、`.catpaw/`、`.githooks/`、`PROJECT_RULES.md`、`ui/src/` 等开发专属资产。

## 其他强制纪律（摘要）

- **TDD（铁律二）**：先写红灯测试（对应 AC），再实现变绿；禁空断言；测试与生产代码同等质量。
- **网络代理（铁律一）**：代理仅限开发调研（搜索资料/下载依赖/抓取文档）。软件本身运行时绝对禁止代理——所有 `reqwest::Client::builder()` 必须加 `.no_proxy()` 确保直连。开发时访问 HuggingFace/GitHub/npm 等失败时，前置 `export https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7890`，用后 `unset`。
- **生产级代码（铁律五）**：零 unwrap/expect/panic/unreachable/todo/unimplemented（workspace deny lint 强制）；溢出必须 `checked_*` / `saturating_*`；错误信息不含敏感信息。
- **最新版本依赖（铁律六）**：开发前 `cargo update --dry-run` 与 `npm outdated`；新增依赖必须确认最新版本；API 变化查官方迁移指南。
- **开发决策互联网搜索（铁律八）**：技术选型、架构设计前先搜索最新资料并引用来源。
- **Rust 生态优先（铁律九）**：功能实现优先纯 Rust crate；参考 `/Users/john/rs-pro`（成熟 Rust 实现移植首选）。
- **跨 Session 衔接（铁律十二）**：Session 上下文管理是 AI Agent 的**全自动行为**。会话开始时自动读 `docs/sessions/CURRENT_STATE.md`，会话结束时自动更新 `CURRENT_STATE.md` + 追加 `LOG.md` + **无条件生成下个 Session 提示词**（`NEXT_SESSION_*.md`，含项目概要/上 Session 摘要/测试基线/下 Session 任务/约束/后续路径/关键文件索引）。无需人工介入。
- **Spec-driven Development（铁律十五）**：先写规格再写代码。任何功能实现前，必须先在 `docs/SRS_v1.0.md` 或原子规划文档中定义 REQ + AC + TC，测试从 AC 派生。实现偏离规格时必须先回写规格。规格是开发的唯一依据。
