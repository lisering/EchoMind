# EchoMind (灵犀) — Agent Guide

> **⚠️ 铁律规则**：先读 `.catpaw/rules/system-constraints.md`（代理仅限开发调研、软件禁止代理、TDD+模糊测试、生产级代码质量、最新依赖+供应链安全、文档实时同步、Rust 优先、SOLID、Spec-driven Development 等，绝对强制）。
>
> **📋 唯一需求真相源**：`docs/SRS_v1.0.md`（格式 `REQ-<域>-<序号>`，每条 AC 由 `TC-*`/`E2E-*` 测试验证）。改动功能必须同步更新 REQ 状态。
>
> **📚 软件资产清单**：`docs/SOFTWARE_ASSET.md`（架构/代码/数据流/依赖/测试/DB/IPC 全量索引）。迭代后必须同步更新——本文件只保留入口指引，不重复该清单。
>
> **🔄 Session 管理（自动）**：会话开始先读 `docs/sessions/CURRENT_STATE.md`（权威状态快照：测试数/IPC 数/版本号，本文件中的数字可能漂移，以它为准）；会话结束更新 `CURRENT_STATE.md` + 追加 `LOG.md` + 生成下个 Session 提示词。
>
> **📜 项目宪法**：`PROJECT_RULES.md`（技术栈锁定、角色分工、三条规则体系），冲突时以其为准。

## What this is

Rust + Tauri v2 desktop app: local RAG knowledge base. fastembed ONNX embeddings (5 models, 384~1024-dim, runtime switchable), SQLite vector store + HNSW index, any OpenAI-compatible LLM (BYOK). SQLCipher AES-256 encryption, PII redaction, hash-chain audit log, auto-lock. Current version: **v3.0.0**.

## Architecture

Hexagonal (ports & adapters), **eight crates**:

| Crate | Role |
|---|---|
| `crates/models` | Domain contracts only (Document, Chunk, ChatMessage, event payloads…) |
| `crates/prompt` | Prompt building (SegmentedPrompt, RAG prompts, Prompt Caching layout, PII sanitize) |
| `crates/compact` | LLM-based context compaction engine |
| `crates/context` | System context management |
| `crates/core` | Business logic & port traits (Loader/Splitter/Embedder/Storage/LLMProvider/Retriever/Reranker/WebSearchProvider/SymbolExtractor/CodeExecutor) + impls: chat engine, import service, security, license |
| `crates/infra` | Adapters: SqliteStorage, LocalEmbedder (fastembed/ONNX), OpenAIProvider (SSE), pdfium/OCR/VLM, HNSW, MCP transport, symbol engine, WASM executor |
| `crates/tauri-app` | Tauri shell: ~179 unique commands in `src/commands/*` (~181 Debug, ~171 Release — 10 dev-tool commands are `#[cfg(debug_assertions)]`-gated), holds AppState |
| `crates/license-issuer` | Pro 许可证签发 CLI (`genkey`/`issue`) |

Frontend: vanilla JS ES modules in `ui/src/*.js` (~56 files), built by esbuild into the single-file `ui/index.html`. No framework, no CDN. i18n via `ui/locales/{en,zh-CN,ja}.json`.

## Commands

| What | Command |
|---|---|
| Build all | `cargo build` |
| Build (Pro) | `cargo build --release --features pro` |
| Run (hot-reload) | `cargo tauri dev` |
| Format check | `cargo fmt --check` (**CI-enforced**) |
| Lint | `cargo clippy --all-targets -- -D warnings` |
| Lint (Pro) | `cargo clippy --all-targets --features pro -- -D warnings` |
| Test all | `cargo test` |
| Test (Pro) | `cargo test --features pro` |
| Focused test | `cargo test <name>` or `cargo test -p echomind-core` |
| UI type check | `npx tsc --noEmit` (or `npm run typecheck`) |
| Frontend build | `node scripts/build-ui.mjs` (prod: IIFE bundle + inline CSS; `--dev`: external refs) |
| UI unit tests (Vitest) | `cd e2e-tests && npm install && npm run test:unit` (tests live in `e2e-tests/tests/unit/`, ~92 files) |
| E2E bridge (mock __TAURI__) | `cd e2e-tests && npm run test:bridge` (Playwright headless, works on macOS) |
| E2E real GUI (L3) | `cd e2e-tests && npm run test:e2e` (WebdriverIO + tauri-driver; **macOS unsupported**, needs `cargo build --workspace`) |
| E2E with real LLM | start `cargo tauri dev` first, then `cd e2e-tests && ECHOMIND_E2E_REAL_LLM=1 npx playwright test tests/real-llm.spec.ts` (LLM via `ECHOMIND_LLM_API_KEY`/`ECHOMIND_LLM_BASE_URL`/`ECHOMIND_LLM_MODEL`) |
| Chaos tests | `python3 tests/fixtures/generate_corpus.py && cargo test --workspace -- --nocapture --include-ignored` |
| Supply chain | `cargo audit` + `cargo deny check` (root `deny.toml`) |
| GitHub 发布同步 | `./sync-to-github.sh [--commit|--push]`（开发目录 → `EchoMind/` 发布仓库 → GitHub） |

CI (`.github/workflows/ci.yml`): `lint` + `frontend-checks` + `supply-chain` run in parallel → `test` → `build-matrix` (4 OS × Free+Pro) → e2e-bridge (sharded Playwright, ubuntu+macos) / e2e-gui (ubuntu+windows only) → `release-build` (installers, push-to-main only).

## Quirks & gotchas

- **Dependency direction**: `tauri-app → core → models` (+ prompt/compact/context below core). Never reverse. Enforced by review; see PROJECT_RULES 体系三.
- **Workspace lint deny**: `unwrap_used`/`expect_used`/`panic`/`unreachable`/`todo`/`unimplemented` are **denied workspace-wide** — production code must return `Err`; tests exempt via `#![allow(...)]`.
- **Edition 2024** (Rust ≥1.85): native `async fn` in trait, **no `async-trait` macro**; `#![allow(async_fn_in_trait)]` is deliberate.
- **Dev profile**: `[profile.dev.package."*"] opt-level = 3` — ML deps (fastembed/ort/tokenizers) unusably slow otherwise. First build is slow; incremental fine.
- **Lazy embedder**: ONNX model NOT loaded at startup. First embed call triggers model download (~30MB from HuggingFace); `chat` may block on first use.
- **UI is a build artifact**: never hand-edit `ui/index.html` (generated bundle). Edit `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json`, then run `node scripts/build-ui.mjs`. Bundle size is checked (`scripts/check-bundle-size.sh`).
- **TypeScript checking without TS source**: root `tsconfig.json` has `checkJs: true`; globals declared in `types/{tauri,vendor,dom-ext}.d.ts`.
- **SQLite storage**: `rusqlite` + WAL + r2d2 pool; all DB ops via `tokio::task::spawn_blocking`. Vectors as BLOB (f32 bytes); search via HNSW (>500 vectors) or Arc-snapshot full scan + normalized dot product.
- **Crash recovery**: startup `cleanup_zombies()` marks leftover `Processing` documents as `Failed`.
- **Stream cancellation**: each conversation gets a tokio_util `CancellationToken`; `abort_chat` breaks the stream loop, partial content is preserved and persisted.
- **API keys never returned**: `get_settings` returns `api_key_masked` only (**** + last 4).
- **Free tier gates**: 50-file cap (`FREE_TIER_MAX_FILES` in `crates/core/src/import.rs`) + PDF import paywall; Pro features behind `--features pro` cfg + Ed25519 license (`crates/core/src/license.rs`).
- **Pre-commit hook** (`.githooks/`): blocks merge-conflict markers and potential secrets (sk-…/password=/api_key= patterns; false positives need `--no-verify`). CI also runs `cargo fmt --check`.
- **`EchoMind/` nested dir is a gitlink**, not part of the build — it's the GitHub release mirror maintained by `sync-to-github.sh`. Don't edit or build inside it.
- **Title auto-extraction**: first user message → first 24 chars + `…`.
- **chat_phase events** ("preparing"/"retrieving"/"generating") are emitted before the first token to avoid blank waits.

## Command module map (`crates/tauri-app/src/commands/`)

SRP-split into domain submodules: `chat` `import` `document` `conversation` `settings` `security` `audit` `license` `performance` `graph` `sync` `mcp` `rag_eval` `trace` `workspace` `features` (+ `features/` dir). Shared helpers/types live in `mod.rs`; everything is registered in one `generate_handler!` in `lib.rs::run()`. AppState in `src/state.rs`; per-domain state split into `src/stores/{chat,config,document,model,security}_store.rs`.

## Core data flows

```
Import:  import_files → ImportService.import_one()
         → Loader.load (md/txt/pdf/docx/html/pptx/epub/xlsx/csv)
         → MD5 dedup → Splitter.split (token/semantic/section-aware; optional late-chunking/contextual prefixes)
         → Storage.add_document/add_chunk → Embedder.embed_batch → Storage.add_embedding (BLOB)
         → EntityExtractor.extract(+relations) → Storage entities/entity_relations   [知识图谱索引]
         → doc-status-changed event → frontend

RAG:     chat → chat_inner()
         → empty-KB check → load LLM config → chat_phase "preparing"
         → ChatEngine.chat(): retrieve (vector, or hybrid vector+BM25→RRF; optional rerank/HyDE/graph-expansion/MMR)
         → threshold filter → segmented RAG prompt (Prompt Caching) → LLMProvider.chat_stream(SSE)
         → chat_sources/chat_phase/chat_token events → abort_token checked per chunk → chat_done
         → persist_exchange() + auto-title on first exchange
```

Other flows (security state machine, knowledge graph, local LLM download/inference, MCP) are documented in `docs/SOFTWARE_ASSET.md` and `docs/architecture/`.

## Key files (only the non-obvious entrypoints)

- `crates/tauri-app/src/lib.rs` — `run()` + single `generate_handler!` registration point
- `crates/core/src/lib.rs` — all hexagonal port traits
- `crates/core/src/import.rs` / `chat.rs` — the two central pipelines
- `crates/infra/src/sqlite_storage.rs` — storage adapter + AuditLogger impl + SQLCipher mode
- `crates/core/src/security.rs` — SecurityManager state machine (Unencrypted → EncryptedUnlocked ⇄ Locked, auto-lock, brute-force backoff)
- `ui/src/main.js` + `ui/src/ipc.js` + `ui/src/state.js` — frontend wiring
- `scripts/build-ui.mjs` — the only sanctioned way to produce `ui/index.html`
