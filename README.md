<p align="center">
  <strong>Local-first AI Knowledge Base — Fast · Private · BYOK</strong>
</p>

<p align="center">
  Rust + Tauri v2 desktop app for private RAG (Retrieval-Augmented Generation) with your own LLM API key.
</p>

<p align="center">
  <a href="#-what-is-echomind">Features</a> ·
  <a href="#-quick-start">Quick Start</a> ·
  <a href="#-architecture">Architecture</a> ·
  <a href="#-pricing">Pricing</a> ·
  <a href="#-tech-stack">Tech Stack</a> ·
  <a href="README.zh-CN.md">📖 中文文档</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.85%2B-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/Tauri-v2-blue?logo=tauri" alt="Tauri v2">
  <img src="https://img.shields.io/badge/Edition-2024-orange" alt="Edition 2024">
  <img src="https://img.shields.io/badge/License-BUSL--1.1-blue" alt="BUSL 1.1 License">
  <img src="https://img.shields.io/badge/Tests-987%20passed-brightgreen" alt="Tests">
  <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Windows%20%7C%20Linux-lightgrey" alt="Cross-platform">
</p>

---

## 🌍 Language / 语言

**English** (current) · [简体中文](README.zh-CN.md)

---

## 🚀 What is EchoMind?

EchoMind is a desktop application that lets you **chat with your local documents** using any OpenAI-compatible LLM. Your files never leave your machine — parsing, chunking, embedding, and vector storage all happen locally. You bring your own API key (**BYOK**), so you stay in full control of costs and data.

> **Core value proposition: Rust speed · Privacy by design · One-time license**

### Why EchoMind?

| | EchoMind | AnythingLLM | Open WebUI | Jan |
|---|:---:|:---:|:---:|:---:|
| **Runtime** | Rust + Tauri (~15 MB) | Electron (~150 MB+) | Python + Docker | Tauri |
| **RAM usage** | Very low | High | Medium-high | Medium |
| **RAG knowledge base** | ✅ | ✅ | ✅ | ❌ |
| **BYOK (own API key)** | ✅ | ✅ | ✅ | Local models |
| **Local embedding (ONNX)** | ✅ | ✅ | ✅ | ❌ |
| **Local LLM (GGUF)** | ✅ Pro | ❌ | ❌ | ✅ |
| **Database encryption** | ✅ SQLCipher | ❌ | ❌ | ❌ |
| **Pricing model** | One-time license | Cloud subscription | Free | Free |
| **Zero server cost** | ✅ | ❌ | ❌ | ✅ |

---

## ✨ Features

### 📥 Document Intelligence
- **Multi-format support** — Markdown, text, PDF (Pro), DOCX, HTML, PPTX, EPUB, XLSX/CSV
- **100% local processing** — parsing, chunking, embedding, and vector storage all on-device
- **Semantic chunking** — paragraph → sentence → clause recursive splitting with code block preservation
- **Section-aware splitting** — Markdown heading hierarchy → section-boundary chunks
- **ONNX embedding** — all-MiniLM-L6-v2 (384-dim, ~30 MB) via fastembed; no external API
- **Custom embedding models** — Pro users can upload custom ONNX models
- **SQLite vector store** — WAL mode, FTS5 full-text index, zero configuration
- **HNSW index** — approximate nearest neighbor for sub-linear search (Pro)
- **File deduplication** — MD5 content hashing prevents duplicate imports
- **Crash recovery** — interrupted indexing tasks auto-recovered on restart

### 💬 RAG Chat
- **Hybrid retrieval** — vector search + BM25 keyword matching → RRF fusion
- **Cross-Encoder reranking** — bge-reranker-base for precision boost (Pro)
- **HyDE query rewriting** — LLM generates hypothetical answer → embed → search (Pro)
- **Knowledge graph** — entity extraction + relation mining → graph traversal retrieval
- **Agentic RAG** — ReAct multi-step reasoning with parallel tool execution
- **Progressive context injection** — start with top-2 chunks, expand if insufficient
- **Speculative RAG** — draft model generates, verify model confirms
- **Retrieval memory** — adaptive method selection based on query type
- **Semantic cache** — three-tier cache (exact / semantic / retrieval) for instant responses
- **Context compaction** — LLM-based history summarization replacing truncation
- **Progress phases** — preparing → retrieving → generating, no blank wait
- **Cancellable generation** — stop mid-response; partial content preserved
- **Multi-turn conversation** — full chat history with auto-extracted titles
- **Branch tree** — ChatGPT-style visual conversation branching

### 🧠 Local LLM Engine (Pro)
- **GGUF inference** — mistral.rs v0.9.0, pure Rust
- **GPU acceleration** — Metal (macOS) / CUDA (NVIDIA) / Accelerate (Apple BLAS)
- **PagedAttention** — efficient KV cache management for long conversations
- **Sampling parameters** — temperature, top-p, top-k, repetition penalty
- **KV cache persistence** — save/restore across sessions
- **Custom GEMV kernel** — self-developed quantization inference (Q4_0/Q4_K/Q8_0/Q8_K)
- **Weight repacking** — CPU cache-friendly Tile-Major layout
- **Layer prefetch** — `madvise(MADV_WILLNEED)` streaming prefetch
- **RAM budget** — LRU eviction + system memory awareness
- **Model download manager** — pause/resume/cancel + crash recovery

### 🔒 Privacy & Security
- **Data stays local** — documents and conversations never leave your machine
- **SQLCipher encryption** — AES-256 transparent database encryption
- **Argon2id key derivation** — memory-hard KDF (m=19456KB, t=2, p=1) + PBKDF2 fallback
- **PII detection & redaction** — 8 types (email, phone, ID card, bank card, IP, SSN, passport, intl phone)
- **Audit hash-chain** — SHA-256 linked audit logs with tamper detection
- **Auto-lock** — idle timeout → locked state, `record_activity()` resets timer
- **Brute-force protection** — 5 failed attempts → exponential backoff
- **Clipboard auto-clear** — sensitive data auto-cleared after timeout
- **API key masking** — `****` + last 4 chars, never plaintext
- **Security posture** — Dangerous / Auto / Strict tiers with shadow screening

### 🎨 Rich Rendering
- **Markdown** with code syntax highlighting (highlight.js)
- **Mermaid diagrams** — flowcharts, sequence diagrams, Gantt charts
- **KaTeX math** — inline and block LaTeX equations
- **Chart.js** — interactive data visualizations
- **Bidirectional wiki-links** — Obsidian-style `[[wiki-link]]` with backlinks
- **No CDN** — all frontend libraries locally vendored

### 🛠 Advanced Tools
- **AutoDream** — background idle tidying: duplicate detection, contradiction discovery
- **Persistent memory** — three-tier (Wing/Hall/Room) with LLM consolidation
- **Code symbol search** — tree-sitter AST extraction (Rust/TS/Python/Go)
- **Code execution sandbox** — Python/Node with timeout/memory/network limits
- **DAG workflow** — visual workflow builder with template management
- **Web search fusion** — DuckDuckGo Instant Answer + RRF local fusion
- **Knowledge graph visualization** — D3.js force-directed graph with community detection
- **PDF export** — `window.print()` zero-dependency export
- **Conversation export** — Markdown format with source citations
- **Folder sync** — file watcher + incremental sync (add/update/delete)

### 🖥 Cross-Platform
- macOS (Apple Silicon + Intel)
- Windows x64
- Linux x64
- Built with Tauri v2 — native performance, not Electron

### 💰 Freemium Model
- **Free tier** — 50 files, Markdown & text only
- **Pro license** — unlimited files, PDF support, local LLM, priority features
- **One-time payment** — no subscription, no recurring fees

### 🗺 Roadmap

#### v1.1 (Released)
- .docx import · .html import · Light theme · Voice input + TTS · Web search

#### v1.2 (Released)
- PDF export · .pptx import · .epub import · Custom embedding model upload

#### v1.3 (Released)
- Knowledge graph export · Document auto-summary · Slash command templates · Wiki-links · Branch tree

#### v1.4 (Released)
- Excel/CSV import · Conversation full-text search · Contextual Retrieval · Document tags · commands.rs modularization

#### v1.5+ (Candidates)
- Late Chunking · MCP protocol support · RAG evaluation metrics · Multi-window · Markdown editor

---

## 🏗 Architecture

Hexagonal (ports & adapters) architecture with 8 crates. Dependencies flow strictly inward:

```
crates/models → crates/prompt → crates/core → crates/infra → crates/tauri-app
 (contracts)    (prompts)       (ports+logic) (adapters)     (assembly)
                                   ↑
                            crates/compact
                            crates/context
```

| Crate | Role |
|---|---|
| `crates/models` | Domain contracts (Document, Chunk, ChatMessage, Conversation, etc.) |
| `crates/prompt` | Prompt building: SegmentedPrompt, RAG/Agent prompt construction, Cache policy |
| `crates/compact` | Context compaction engine: LLM-based history summarization |
| `crates/context` | System context registry: epoch management, durable baseline |
| `crates/core` | Port traits + business logic; chat engine, import service, license verification, security |
| `crates/infra` | Adapters: SqliteStorage, LocalEmbedder, OpenAIProvider, HNSW, LocalLlmEngine, OCR, VLM |
| `crates/tauri-app` | Tauri shell, 190+ IPC commands, AppState |
| `crates/license-issuer` | CLI tool for Ed25519 license key generation (not in public release) |

**Frontend**: Single-file SPA (`ui/index.html`) — 50 ES modules bundled via esbuild. Tailwind CSS (local JIT), vanilla JavaScript. **No CDN, no framework.**

### Data Flow

**Document Import** (100% local):
```
import_files → Loader.load() → MD5 dedup → Splitter.split()
  → Storage.add_document() + add_chunk() → Embedder.embed_batch()
  → Storage.add_embedding() → EntityExtractor → doc-status-changed event
```

**RAG Query** (BYOK):
```
chat → embed query (local ONNX) → hybrid search (vector + BM25 → RRF)
  → rerank (bge-reranker) → build RAG prompt → LLM chat_stream (SSE)
  → chat_token events → chat_done → persist exchange
```

---

## ⚡ Quick Start

### Prerequisites

- **Rust** ≥ 1.85 (Edition 2024) — [install](https://rustup.rs/)
- **Node.js** ≥ 18 (for E2E tests, optional)

### Build & Run

```bash
# Clone
git clone https://github.com/lisering/EchoMind.git
cd EchoMind

# Build all crates
cargo build

# Run in dev mode (hot-reload)
cargo tauri dev

# Build with Pro features
cargo build --features pro
```

> **Note**: First build takes 5–10 minutes due to ML dependencies (fastembed/ort/tokenizers) compiled at `opt-level = 3`. Incremental builds are fast.

### Test

```bash
# Rust unit + integration tests (987 tests)
cargo test

# Lint (zero warnings policy)
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Supply chain security
cargo audit
cargo deny check

# Frontend type check
npx tsc --noEmit

# Frontend build
node scripts/build-ui.mjs
```

### Usage Guide

1. **Launch** EchoMind
2. **Configure** — Settings → enter your LLM provider details (API key, base URL, model name)
3. **Import** — Drag files into the window (PDF/local LLM requires Pro)
4. **Wait** for indexing to complete (local ONNX embedding — watch the progress badge)
5. **Chat** — Type your question and get streaming answers with source citations

### Supported LLM Providers

Any OpenAI-compatible API endpoint works:

| Provider | Base URL | Notes |
|---|---|---|
| OpenAI | `https://api.openai.com/v1` | Default |
| Anthropic | `https://api.anthropic.com/v1` | Via OpenAI-compatible endpoint |
| DeepSeek | `https://api.deepseek.com/v1` | Popular in China |
| Ollama (local) | `http://localhost:11434/v1` | Empty API key |
| LM Studio | `http://localhost:1234/v1` | Local model runner |
| Local GGUF (Pro) | — | Built-in mistral.rs engine, no external service |
| Any OpenAI-compatible | Custom base URL | If it speaks OpenAI API, it works |

---

## 💰 Pricing

| Tier | Price | Limits |
|---|---|---|
| **Free** | $0 | 50 files, Markdown & text only |
| **Pro** | One-time license | Unlimited files, PDF, local LLM, all features |

Pro license is verified offline via Ed25519 signature — no internet connection required for activation.

---

## 🛠 Tech Stack

| Layer | Technology | Details |
|---|---|---|
| Language | Rust (Edition 2024) | Native `async fn` in trait, no `async-trait` macro |
| Desktop framework | Tauri v2 | Smaller, faster, more secure than Electron |
| Embedding | fastembed (ONNX Runtime) | all-MiniLM-L6-v2, 384-dim, ~30 MB |
| Local LLM | mistral.rs v0.9.0 | GGUF, Metal/CUDA, PagedAttention (Pro) |
| Vector store | SQLite (rusqlite + r2d2) | WAL mode, FTS5, SQLCipher AES-256 |
| LLM API | OpenAI-compatible | SSE streaming, 30s connection timeout |
| Frontend | Vanilla JS ES modules | esbuild IIFE bundle, no React/Vue/Svelte |
| Rendering | marked.js, DOMPurify, highlight.js | + Mermaid, KaTeX, Chart.js, D3.js |
| License | Ed25519 signature | Offline verification, zero network |

### Code Quality

- **Clippy** — zero warnings policy (`-D warnings`) with deny lints for `unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`
- **TDD** — test-first development, 987 tests, unit tests co-located with source
- **Supply chain** — `cargo audit` + `cargo deny check` on every CI run
- **Documentation** — all public types have `///` doc comments
- **No `unsafe`** — `forbid(unsafe_code)` in production crates

---

## 📄 License

**Business Source License 1.1 (BUSL-1.1)** — see [LICENSE](LICENSE).

EchoMind uses the Business Source License, which allows:
- ✅ **Personal non-commercial use** — learning, personal knowledge management, research
- ✅ **Source code review** — full transparency for privacy-conscious users
- ✅ **30-day evaluation** for organizations
- ❌ **Commercial use without a license** — requires purchasing a Pro license
- ❌ **Removing or bypassing** the Ed25519 license verification

On **January 1, 2030**, this license automatically converts to **Apache License 2.0**.

---

## 🙏 Acknowledgments

- [Tauri](https://tauri.app/) — for the amazing Rust desktop framework
- [fastembed](https://github.com/Anush008/fastembed-rs) — for making ONNX embedding effortless
- [SQLite](https://www.sqlite.org/) — for the world's most reliable embedded database
- [mistral.rs](https://github.com/EricLBuehler/mistral.rs) — for pure Rust LLM inference
- The Rust community — for building tools that make software fast and safe

---

<p align="center">
  <a href="README.zh-CN.md">📖 中文文档</a> ·
  <a href="LICENSE">BUSL-1.1</a>
</p>

<p align="center">
  Made with ❤️ and Rust
</p>
