# EchoMind (灵犀) — Agent Guide

> **⚠️ 铁律规则**：请先阅读 `.catpaw/rules/system-constraints.md`（代理设置仅限开发调研、软件本身禁止代理、TDD 驱动+模糊测试、代码注释、真实数据测试、生产级代码质量+Miri UB 检测+forbid unsafe、最新版本依赖+供应链安全工具链、文档实时同步+文档治理、开发决策互联网搜索、Rust 生态优先、GitHub 发布同步、Tailwind 样式规范、SOLID 原则、Spec-driven Development 规格驱动开发、AI 生成代码审查、结构化日志与可观测性+axe-core、LLM 应用安全防护+致命三要素+Rule of Two、错误处理与弹性设计、AI Agent 行为规范、RAG 安全防护+向量嵌入弱点）。
>
> **📋 需求规格**：所有功能需求定义在 `docs/SRS_v1.0.md` 中，是开发、测试、验收的唯一依据。
>
> **📚 软件资产**：`docs/SOFTWARE_ASSET.md` 是项目完整软件资产清单（架构、代码、数据流、依赖、测试、数据库、IPC），后续迭代必须同步更新。
>
> **🔄 Session 管理（全自动）**：`docs/sessions/` 是跨会话上下文管理中心。**Agent 自动行为**：会话开始时自动读 `docs/sessions/CURRENT_STATE.md` 获取快照；会话结束时自动更新 `CURRENT_STATE.md` + 追加 `LOG.md` + 按 `HANDOFF_TEMPLATE.md` 生成衔接提示词。无需人工介入（铁律十二）。
>
> **🗺 版本路线图**：v1.1.0 已发布（✅ 5 功能 S44-S48）。v1.2 规划中（📋 4 功能 S49-S53：PDF 导出 / .pptx 导入 / .epub 导入 / 自定义嵌入模型上传 Pro），详见 `docs/architecture/V1.2_ATOMIC_PLAN.md`。
>
> **🔬 OpenCode 借鉴**：OpenCode v1.18.15 源码研究完成，14 项借鉴点（4 高 + 5 中 + 5 低），详见 `docs/architecture/OPENCODE_BORROW_ATOMIC_PLAN.md`。高优先级：System Context Registry / Context Epoch / 结构化压缩模板 / Prompt Caching 自动放置。

Rust + Tauri v2 desktop app: local RAG knowledge base. fastembed ONNX embedding (all-MiniLM-L6-v2, 384-dim), SQLite vector store, any OpenAI-compatible LLM. Features SQLCipher AES-256 encryption, PII detection/redaction, audit hash-chain logging, and auto-lock for high-privacy scenarios (lawyers, researchers, etc.).

## Architecture

Hexagonal (ports & adapters), seven crates:

| Crate | Role |
|---|---|
| `crates/models` | Domain contracts only (Document, Chunk, ChatMessage, Conversation, DocStatus, RetrievalResult, LlmConfig, SettingsPayload, event payloads) |
| `crates/prompt` | Prompt building: SegmentedPrompt, build_rag_prompt_segmented, build_agent_prompt, build_final_rag_prompt (Prompt Caching optimization) |
| `crates/compact` | Context compaction engine: CompactionEngine (LLM-based history summarization replacing truncation) |
| `crates/core` | Business logic: Loader, Splitter, Embedder, Storage, LLMProvider, Retriever traits + impls; chat engine, import service, license, stream parser |
| `crates/infra` | Adapters: SqliteStorage, LocalEmbedder (fastembed/ONNX), OpenAIProvider (SSE streaming), ModelManager + LocalLlmEngine (Pro, mistral.rs v0.9.0, GPU/PagedAttention/采样参数; Phase 3: 自研 GEMV 内核 + 权重重排 + 流式预取 + RAM 预算) |
| `crates/tauri-app` | Tauri shell, 150 IPC commands in `commands.rs` (16 pro-gated; registered via `generate_handler!` in `lib.rs:run()`), holds AppState (incl. SecurityManager, llm_mode, model_manager, cache, compression_ratio, progressive_enabled, speculative_enabled, retrieval_memory_enabled, memory_enabled, web_search_enabled, _log_guard) |
| `crates/license-issuer` | Pro 许可证签发 CLI（`genkey` / `issue`，输出 `base64(payload)-base64(signature)` License Key，与 `crates/core/src/license.rs::verify_license` 配套，REQ-LIC-005） |

Frontend: `ui/src/*.js` modular source (vanilla JS ES modules, 50 files) built into the single-file `ui/index.html` via `node scripts/build-ui.mjs` (prod: **esbuild** bundles JS IIFE + inlines CSS, no external deps; `--dev`: external refs for browser dev). TypeScript type checking via `npx tsc --noEmit` (tsconfig.json + `types/*.d.ts` global declarations). i18n via `ui/locales/{en,zh-CN}.json`. Tailwind/marked.js/DOMPurify/highlight.js vendored locally. **No CDN, no framework.** DeepSeek-style UI modules: `thinking-panel.js` (collapsible CoT panel), `input-toggles.js` (quick toggle switches for hybrid search / agent mode / sub_agent / memory), `download-manager.js` (download panel + pause/resume/cancel + crash recovery). Agentic AI UI modules: `workflow-panel.js` (DAG 工作流可视化), `memory-panel.js` (持久化记忆管理), `dream-panel.js` (AutoDream 知识库整理), `symbol-search.js` (代码符号搜索), `perf-settings.js` (性能优化设置). PDF export module: `export.js` (window.print() 零依赖 PDF 导出, REQ-EXP-005).

## Commands

| What | Command |
|---|---|
| Build all | `cargo build` |
| Build (Pro) | `cargo build --release --features pro` |
| Run (hot-reload) | `cargo tauri dev` |
| Test all | `cargo test` |
| Test (Pro) | `cargo test --features pro` |
| Focused test | `cargo test <name>` or `cargo test -p <crate>` |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Lint (Pro) | `cargo clippy --all-targets --features pro -- -D warnings` |
| UI type check | `npx tsc --noEmit`（根目录 tsconfig.json + types/*.d.ts 全局类型声明） |
| Frontend build | `node scripts/build-ui.mjs`（esbuild 打包 IIFE + 内联 CSS 单文件；`--dev` 外部引用） |
| UI unit tests (Vitest) | `cd e2e-tests && npm install && npm run test:unit`（403 tests，27 files） |
| E2E bridge (mock __TAURI__) | `cd e2e-tests && npm install && npm run test:bridge`（Playwright headless，全平台） |
| E2E real GUI (L3) | `cd e2e-tests && npm run test:e2e`（WebdriverIO + tauri-driver；**macOS 不支持**，需 `cargo build --workspace`） |
| E2E with real LLM | 先启动 `cargo tauri dev`，再 `ECHOMIND_E2E_REAL_LLM=1 ECHOMIND_E2E_URL=http://localhost:1420 npx playwright test tests/real-llm.spec.ts`（LLM 经 `ECHOMIND_LLM_API_KEY` / `ECHOMIND_LLM_BASE_URL` / `ECHOMIND_LLM_MODEL`） |
| Chaos tests | `python3 tests/fixtures/generate_corpus.py && cargo test --workspace -- --nocapture --include-ignored` |
| Supply chain | `cargo audit` + `cargo deny check`（根目录 `deny.toml`） |

## Quirks & gotchas

- **Dependency direction**（PROJECT_RULES 体系三）：`crates/tauri-app → crates/core → crates/models`，严禁反向依赖。
- **Workspace lint deny**：`Cargo.toml` workspace lints 强制 deny `unwrap_used` / `expect_used` / `panic` / `unreachable` / `todo` / `unimplemented` —— 生产代码必须返回 `Err`；测试代码通过 `#![allow(...)]` 豁免。CI 用 `cargo clippy --all-targets -- -D warnings` 强制。
- **Edition 2024** — requires recent Rust toolchain (≥1.85). Uses native `async fn` in trait, **no `async-trait` macro**. `#![allow(async_fn_in_trait)]` is a deliberate exemption (Send + Sync supertrait constrains all implementors).
- **Dev profile**: `[profile.dev.package."*"] opt-level = 3` — ML deps (fastembed/ort/tokenizers) unusably slow otherwise. First build slow; incremental fine.
- **Lazy embedder**: ONNX model NOT loaded at startup. `AppState.embedder()` uses `tokio::sync::OnceCell` — first call triggers model download (~30MB HuggingFace). `chat` command calls `embedder()` which may block on first use.
- **UI is a build artifact**: in prod mode `ui/index.html` is the generated esbuild IIFE bundle — edit `ui/src/*.js` / `ui/styles/*.css` (or `ui/locales/*.json`) and run `node scripts/build-ui.mjs`; do not hand-edit the inlined bundle.
- **TypeScript type checking**: `tsconfig.json` at root with `checkJs: true`; `types/tauri.d.ts` (Window.__TAURI__), `types/vendor.d.ts` (marked/DOMPurify/mermaid/hljs/katex/Chart), `types/dom-ext.d.ts` (Element/HTMLElement/Node/Event extensions). Run `npx tsc --noEmit` to check.
- **esbuild bundling**: `scripts/build-ui.mjs` uses esbuild to bundle `ui/src/main.js` → IIFE format. No more manual MODULE_ORDER or stripModuleSyntax. esbuild properly resolves ES module imports and tree-shakes unused exports.
- **SQLite storage**: `SqliteStorage` in `crates/infra` uses `rusqlite` with WAL mode. All DB operations via `tokio::task::spawn_blocking`. Vectors stored as BLOB (f32 bytes), cosine similarity computed in Rust after full-scan load.
- **Crash recovery**: On startup, `cleanup_zombies()` marks leftover `Processing` documents as `Failed`.
- **E2E mock**: bridge tests unconditionally inject `e2e-tests/tests/bridge/tauri-stub.js` via `page.addInitScript`（`window.__TAURI__` mock，无需环境变量开关）。`E2E_SPEED` 倍率缩放 mock 延迟（CI 用 0.2）。真实 GUI 模式（`npm run test:e2e`）经 `ECHOMIND_DATA_DIR` 隔离数据目录 + `ECHOMIND_MODEL_CACHE` 共享模型缓存；tauri-driver 走 WebDriver 协议（127.0.0.1:4444），**不支持 macOS**。
- **Tauri mock**: `e2e-tests/tests/bridge/tauri-stub.js` injects `window.__TAURI__` mock. Tests run in regular browser, not Tauri.
- **Abort/cancel**: Each conversation gets a `CancellationToken` (tokio_util). `abort_chat` command cancels the token, breaking the stream loop immediately. Generated content is preserved and persisted.
- **API Key masking**: `get_settings` returns `api_key_masked` (**** + last 4 chars), never plaintext. `mask_api_key()` in `commands.rs`.
- **Pro license**: `verify_license()` in `crates/core/src/license.rs`. License stored in settings table as `license.is_pro = "true"`. Free tier: 50 files max, PDF disabled.
- **Title auto-extraction**: First user message → `derive_title()` takes first 24 chars, appends `…` if longer.
- **Phase events**: `chat_phase` event pushes progress stages (preparing/retrieving/generating) before first token arrives, eliminating blank wait.
- **Security layer**: `SecurityManager` in `crates/core/src/security.rs` manages encryption state machine (Unencrypted → EncryptedUnlocked → Locked). `crates/core/src/encryption.rs` provides Argon2id key derivation + PBKDF2 fallback + `SecureString` (zeroize). `crates/core/src/privacy.rs` detects/redacts 8 PII types (email/phone/idcard/bankcard/ip/ssn/passport/intl_phone) + `AuditLogger` trait with hash-chain. `SqliteStorage` implements `AuditLogger`, persists audit logs with prev_hash chaining.
- **Auto-lock**: Idle timeout triggers `Locked` state. `record_activity()` resets timer. `system_wake_lock()` prevents sleep during active chat.
- **Clipboard auto-clear**: Sensitive data copied to clipboard is auto-cleared after configurable timeout.
- **Brute-force protection**: 5 failed unlock attempts → exponential backoff (30s → 60s → 120s → ...).

## Key files

- `crates/tauri-app/src/lib.rs` — Tauri command setup + `run()` (150 IPC commands, 16 pro-gated)
- `crates/tauri-app/src/commands/mod.rs` — Command 模块根（共享类型/常量/辅助函数 + `pub use` 重导出 + `#![allow(clippy::unreachable)]`）
- `crates/tauri-app/src/commands/` — 14 功能域子模块（S65 SRP 重构，7706→14 文件）：`chat.rs`（对话/RAG）、`import.rs`（文档导入/嵌入器）、`document.rs`（文档管理/摘要）、`conversation.rs`（会话/消息/分支）、`audit.rs`（审计 Pro）、`license.rs`（许可证）、`settings.rs`（LLM 配置/设置/主题）、`local_llm.rs`（本地 LLM/模型下载/KV cache Pro）、`security.rs`（安全/PII/审计日志）、`agent.rs`（Agent/Coordinator/SubAgent）、`performance.rs`（缓存/压缩/BM25/Proposition/检索记忆）、`graph.rs`（知识图谱）、`sync.rs`（文件同步）、`features.rs`（Dream/日志/符号/工作流/代码执行/记忆/Web 搜索/自定义模型/模板/Wiki 链接）
- `crates/tauri-app/src/state.rs` — AppState (storage, data_dir, llm_config, is_pro, lazy embedder, abort_tokens, llm_mode, local_llm (Pro), model_manager, hybrid_search toggle, reranker, stores/ (config/model/document/chat/security), dream_engine, page_renderer, ocr_engine, vision_provider)
- `crates/tauri-app/src/stores/` — Store 模式状态模块（chat_store / config_store / document_store / model_store / security_store）
- `crates/core/src/lib.rs` — Port traits (Loader, Splitter, Embedder, Storage, LLMProvider, Retriever, Reranker)
- `crates/prompt/src/lib.rs` — Prompt building: SegmentedPrompt, build_rag_prompt_segmented, build_agent_prompt, build_final_rag_prompt
- `crates/compact/src/lib.rs` — CompactionEngine (LLM-based history summarization)
- `crates/core/src/chat.rs` — ChatEngine (RAG query orchestration)
- `crates/core/src/coordinator.rs` — CoordinatorEngine (multi-agent coordination: Research→Synthesis→Implementation→Verification)
- `crates/core/src/agent.rs` — Agentic RAG 多步推理引擎（REQ-RAG-022）
- `crates/core/src/import.rs` — ImportService (file import + indexing + embedding pipeline; PDF 多模态提取注入 PageRenderer + OcrEngine + 可选 VLM)
- `crates/core/src/retriever.rs` — VectorRetriever
- `crates/core/src/hybrid_retriever.rs` — HybridRetriever: 向量 + BM25 关键词检索 → RRF 融合（REQ-RAG-010，`rag.hybrid_search` 设置开关）
- `crates/core/src/loader.rs` — Markdown/Text/PDF/Docx/Html/Pptx/Epub loaders
- `crates/core/src/splitter.rs` — Token-based text splitter
- `crates/core/src/semantic_splitter.rs` / `section_aware_splitter.rs` — 语义分块（REQ-VEC-002）/ 章节感知分块（REQ-VEC-006）
- `crates/core/src/stream_parse.rs` — SSE stream parser
- `crates/core/src/license.rs` — License verification
- `crates/core/src/encryption.rs` — Argon2id KDF, PBKDF2 fallback, SecureString (zeroize), password strength
- `crates/core/src/privacy.rs` — PII detector (8 types), PrivacyManager, AuditLogger trait (hash-chain)
- `crates/core/src/security.rs` — SecurityManager state machine, auto-lock, clipboard clear, brute-force protection
- `crates/core/src/auto_dream.rs` — 后台空闲整理引擎 AutoDream（DreamEngineState）
- `crates/infra/src/sqlite_storage.rs` — SQLite with WAL mode, r2d2 pool, vector BLOB storage, AuditLogger impl, SQLCipher encrypted mode
- `crates/infra/src/local_embedder.rs` — fastembed/ONNX embedding (all-MiniLM-L6-v2, 384-dim)
- `crates/infra/src/local_reranker.rs` — fastembed/ONNX Cross-Encoder reranking (bge-reranker-base, REQ-RAG-020)
- `crates/infra/src/openai_provider.rs` — OpenAI-compatible SSE streaming
- `crates/infra/src/hnsw_index.rs` — HNSW 近似最近邻索引（REQ-NFR-005）
- `crates/infra/src/hyde_rewriter.rs` — HyDE 查询改写（REQ-RAG-021）
- `crates/infra/src/pdfium_renderer.rs` / `ocrs_engine.rs` / `openai_vision.rs` — PDF 页面渲染（REQ-MM-001）/ 本地 OCR（REQ-MM-002）/ OpenAI 兼容 VLM 适配器（REQ-MM-003）
- `crates/infra/src/file_watcher.rs` — 文件监听自动同步（REQ-SYNC-003）
- `crates/infra/src/local_logger.rs` — 本地日志系统（REQ-OBS-001）：tracing 集成 + JSON Lines 格式 + 日期轮转 + 运行时级别控制 + 诊断信息导出
- `crates/infra/src/model_manager.rs` — Local GGUF model file management (download/list/delete, REQ-LLM-004)
- `crates/infra/src/local_llm.rs` — mistral.rs v0.9.0 local LLM inference engine (Pro feature, REQ-LLM-003; Phase 2: GPU/Metal/CUDA + PagedAttention + 采样参数 + 模型卸载 + 流取消 + 预热; Phase 3: 自研 GEMV 内核模式切换 + KernelMode)
- `crates/infra/src/gguf_reader.rs` — GGUF 文件解析器（mmap 零拷贝，V1/V2/V3 格式，Pro feature）
- `crates/infra/src/quant_blocks.rs` — 量化块结构 + 反量化（BlockQ4_0/BlockQ4K/BlockQ8_0/BlockQ8K，Pro feature）
- `crates/infra/src/gemv_kernel.rs` — 自研 GEMV 内核（4 种量化格式，标量 + AVX2 预留，Pro feature）
- `crates/infra/src/weight_repack.rs` — CPU cache 友好权重重排（Row-Major → Tile-Major，Pro feature）
- `crates/infra/src/layer_prefetch.rs` — Layer 级流式预取（madvise + 后台线程，Pro feature）
- `crates/infra/src/memory_budget.rs` — RAM 预算管理（LRU 驱逐 + libc::sysconf，Pro feature）
- `crates/infra/src/kv_cache.rs` — 本地 LLM KV cache 序列化/反序列化（Pro feature）
- `crates/models/src/lib.rs` — Data model definitions (Document, Chunk, ChatMessage, Conversation, LlmMode, ModelInfo, CacheLevel, CacheStats, CacheSettingsPayload, Proposition, SummaryNode, Entity, MemoryRecord, etc.)
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
- `crates/core/src/graph_analyzer_tests.rs` — 图分析引擎 10 个 TDD 测试（TC-GRAPH-ANALYSIS-001~010）
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
- `ui/src/code-executor.js` — 代码执行沙箱面板（语言选择 + 代码编辑区 + stdin + 结果展示 + Focus Trap，REQ-RAG-032）
- `ui/src/graph-viewer.js` — 知识图谱 D3.js v7 可视化（force-directed graph + 节点拖拽/缩放/高亮 + 实体类型图标 + 子图过滤 + 搜索定位 + SVG/PNG 导出 + 布局切换 + 路径分析 + 社区检测着色，REQ-RAG-027）
- `ui/index.html` — Frontend SPA（`node scripts/build-ui.mjs` 生成；源码见 `ui/src/`）
- `ui/src/download-manager.js` — 下载管理器面板（REQ-LLM-004 v2：暂停/恢复/取消 + 崩溃恢复 + model_download_progress 事件统一监听）
- `ui/src/workflow-panel.js` — DAG 工作流可视化面板（模板 CRUD + 运行 + 进度，REQ-RAG-030）
- `ui/src/memory-panel.js` — 持久化记忆管理面板（三列 tier 布局 Wing/Hall/Room + promote/delete/clear/pin，REQ-RAG-033）
- `ui/src/dream-panel.js` — AutoDream 知识库整理面板（触发/中止/历史建议，REQ-RAG-029）
- `ui/src/symbol-search.js` — 代码符号搜索面板（tree-sitter AST 符号搜索 + 重建索引，REQ-RAG-031）
- `ui/src/perf-settings.js` — 性能优化设置面板（缓存/压缩比/检索记忆/渐进式/Speculative/质量门控/图谱检索 toggle，REQ-PERF-001~012）
- `ui/src/export.js` — PDF 导出模块（window.print() 零依赖，复用 Markdown 渲染管线 + 隐藏 iframe + @media print CSS，REQ-EXP-005）
- `ui/styles/print.css` — 打印样式（@media print 规则：隐藏 UI 元素 + serif 字体 + 分页控制，REQ-EXP-005）

## Data flow (Document import)

```
import_files IPC → ImportService.import_one()
  → Loader.load() (markdown/text/docx/html/pptx/epub; PDF 多模态提取: pdfium 渲染 + ocrs OCR + 可选 VLM)
  → MD5 hash → duplicate check (find_document_by_hash)
  → Splitter.split() (token-based chunking)
  → Storage.add_document() + add_chunk() (SQLite)
  → Embedder.embed_batch() (fastembed/ONNX)
  → Storage.add_embedding() (vector BLOB)
  → doc-status-changed event → frontend list update
```

## Data flow (RAG query)

```
chat IPC → chat_inner()
  → empty KB check (count_documents == 0 → error)
  → load LLM config (api_key, base_url, model)
  → chat_phase "preparing" event → init embedder (lazy, first-use download)
  → create/update conversation
  → ChatEngine.chat()
    → VectorRetriever.retrieve() (或 HybridRetriever 向量+BM25+RRF，若 rag.hybrid_search 开启)
      → Embedder.embed(query) → Storage.vector_search(cosine sim, top-k=5)
    → threshold filter (score > 0.1)
    → (可选) LocalReranker 重排 (bge-reranker-base)
    → build RAG prompt → LLMProvider.chat_stream(SSE)
  → chat_sources event → frontend shows references
  → chat_phase "generating" event
  → chat_token events → frontend typing effect
  → abort_token check (CancellationToken)
  → chat_done event
  → persist_exchange() → Storage.add_message() (user + assistant)
  → auto-extract title on first exchange
```

## Data flow (Stream cancellation)

```
abort_chat IPC → AppState.abort_chat()
  → CancellationToken.cancel()
  → forward_stream() loop breaks on select! biased branch
  → emit "⏹ 生成已中断" + chat_done
  → persist_exchange() saves partial content
  → clear_abort() cleans up token
```

## Data flow (License & Quota)

```
import_files → ImportService.import_one()
  → check is_pro (from AppState, read from settings table)
  → if !is_pro: count_documents() >= 50 → reject with quota error
  → if !is_pro: file is .pdf → reject with paywall error
  → if is_pro: verify_license(key) → activate_pro() → persist + refresh runtime
```

## Data flow (Security & Encryption)

```
encrypt_database IPC → SecurityManager.encrypt_database(password)
  → Argon2id derive_key(password, salt) → 32-byte key
  → PRAGMA key + rekey (SQLCipher transparent encryption)
  → state: Unencrypted → EncryptedUnlocked
  → SecureString wipe password from memory

unlock_database IPC → SecurityManager.unlock(password)
  → Argon2id verify → PRAGMA key
  → brute-force check (5 attempts → backoff)
  → state: Locked → EncryptedUnlocked

Auto-lock (idle) → SecurityManager.check_auto_lock()
  → elapsed > auto_lock_timeout → state: EncryptedUnlocked → Locked
  → emit security-state-changed event → frontend lock overlay

PII detection → PrivacyManager.redact_text(text)
  → regex scan 8 PII types → replace with [REDACTED-TYPE]
  → AuditLogger.log(AuditEvent { action, target, timestamp })
  → hash-chain: entry_hash = SHA256(prev_hash || entry_data)
```

## Data flow (Knowledge graph)

```
导入管线集成:
import_files → ImportService.import_one()
  → Loader.load() → Splitter.split()
  → Storage.add_document() + add_chunk() + Embedder.embed_batch()
  → EntityExtractor.extract(text) → Storage.add_entities()           ← 实体索引
  → EntityExtractor.extract_relations(text, chunk_id)                 ← 关系抽取（8 种）
  → Storage.add_relations_batch()                                      ← entity_relations 表

检索管线集成:
chat IPC → chat_inner()
  → (if rag.graph_retriever_enabled)
  → GraphRetriever.expand(query, top_k=5)
    → EntityExtractor.extract(query) → query_entities
    → BFS: Storage.get_relations_for_entity(entity) (max 2-hop)
      → 1-hop: confidence × 1.0, 2-hop: confidence × 0.5
    → Storage.get_chunk_by_id(chunk_id) → RetrievalResult
  → rrf_fuse_four_way(vector, keyword, entity, graph, top_k)
    → Vector=1.0, BM25=0.8, Entity=0.6, Graph=0.5

可视化与分析:
openGraphViewer → get_graph_data() → D3 forceSimulation
  → 节点拖拽/缩放/高亮 + 8 种关系颜色编码 + 5 种实体类型 SVG 图标
  → 子图过滤（关系类型复选框）
  → 搜索定位（zoom transform 居中+缩放+高亮）
  → SVG/PNG 导出
  → 布局切换（force/hierarchical/radial）
  → 路径分析: get_shortest_path(from, to) → BFS → 高亮路径节点
  → 社区检测: get_communities() → Label Propagation → 10 色着色
```

## Data flow (Local LLM inference, Pro feature)

```
set_llm_mode IPC → AppState.set_llm_mode("local")
  → persist to settings table (llm.mode = "local")
  → Free user: reject with PRO_REQUIRED error
  → S14: tokio::spawn(engine.warm_up()) 后台预热模型

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
