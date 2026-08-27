# EchoMind 大简化重构 — 依赖审计报告

> **生成日期**: 2026-08-27 ｜ **分支**: `refactor/great-simplification`
>
> 本报告记录重构前的依赖状态，识别可删除的依赖。

---

## 1. 依赖总览

| 指标 | 数值 |
|---|---|
| cargo tree 节点总数 | 1,765 |
| 重复依赖包数 | 119 |
| feature 依赖行数 | 5,295 |

---

## 2. 直接依赖（Workspace 级）

### 2.1 echomind-models
- 无外部直接依赖（仅 dev-dependencies: serde_json, chrono, uuid）

### 2.2 echomind-prompt
- `regex` v1.12.4
- `serde` v1.0.229
- `serde_json` v1.0.151

### 2.3 echomind-compact
- `anyhow` v1.0.104
- `futures` v0.3.33
- `tiktoken-rs` v0.12.0
- `tokio` v1.53.1
- `tracing` v0.1.44

### 2.4 echomind-context
- `anyhow` v1.0.104
- `serde` v1.0.229
- `serde_json` v1.0.151

### 2.5 echomind-core
- **隐藏依赖通过 pub use**：实际直接依赖通过各子模块 Cargo.toml 管理
- 核心直接依赖包括：anyhow, serde, serde_json, tokio, regex, chrono, uuid, md-5, lopdf, docx-rs, html2text, pptx-to-md, rbook, calamine, csv, argon2, ed25519-dalek, sha2, tiktoken-rs, futures, tracing

### 2.6 echomind-infra
- `aes-gcm` v0.10.3 — HNSW 加密持久化
- `backon` v1.6.0 — 重试逻辑
- `base64` v0.22.1
- `fastembed` v5.17.4 — ONNX 嵌入
- `hnsw_rs` v0.3.4 — HNSW 近似最近邻
- `notify` v8.2.0 — 文件监听
- `notify-debouncer-full` v0.7.0 — 防抖
- `r2d2` v0.8.10 — 连接池
- `r2d2_sqlite` v0.25.0 — SQLite 连接池
- `rand` v0.8.7
- `reqwest` v0.12.28 — HTTP 客户端
- `rusqlite` v0.32.1 — SQLite
- `sha1` v0.10.7 — Prompt Prefix 缓存 (🔴 可删除)
- `sha2` v0.10.9
- `tokio-util` v0.7.19
- `tracing-appender` v0.2.5 — 日志
- `tracing-subscriber` v0.3.23 — 日志
- `uuid` v1.24.0

### 2.7 echomind-tauri-app
- `tauri` v2.11.5
- `tauri-plugin-autostart` v2.5.1
- `tauri-plugin-dialog` v2.7.2
- `tauri-plugin-opener` v2.5.4
- `md-5` v0.10.6 — 文件哈希
- `ed25519-dalek` v2.2.0 — 许可证验证
- `lopdf` v0.42.0 — PDF 文本提取

### 2.8 license-issuer
- `clap` v4.6.6
- `base64` v0.22.1
- `ed25519-dalek` v2.2.0
- `rand` v0.8.7

---

## 3. 可删除依赖 (Phase 1-5 削减目标)

### 3.1 随学术 RAG 功能删除 (R1)

| 依赖 | 用途 | 替代方案 |
|---|---|---|
| `sha1` | Prompt Prefix 缓存 (KPC 格式) | 直接删除 |
| `sha2` (部分) | ResponseCache hash | 保留（安全功能也用） |

### 3.2 随 Agent 功能删除 (R2)

| 依赖 | 用途 | 替代方案 |
|---|---|---|
| (无额外依赖) | AgentEngine/Coordinator/AutoDream 均使用已有依赖 | - |

### 3.3 随 LLM 过度工程功能删除 (R3)

| 依赖 | 用途 | 替代方案 |
|---|---|---|
| `half` | f16 量化 | 删除 |
| `memmap2` | GGUF mmap 零拷贝 | 删除 |
| `libc` | sysconf/madvise 层预取 | 删除 |

### 3.4 随工具/借鉴功能删除 (R4)

| 依赖 | 用途 | 替代方案 |
|---|---|---|
| tree-sitter 系列 | SymbolEngine 代码符号 | 删除 |
| (colbert 相关) | ColBERT 多向量嵌入 | 删除 |
| (MCP 传输相关) | MCP 协议适配器 | 删除 |

### 3.5 前端依赖删除 (R5)

| 依赖 | 用途 | 替代方案 |
|---|---|---|
| `d3` (vendor) | 知识图谱可视化 | 删除 |
| `chart.js` (vendor) | 数据可视化 | 删除 |
| `mermaid` (vendor) | 图表渲染 | 评估后决定 |

---

## 4. 重复依赖分析

### 4.1 主要重复

| 包 | 版本冲突 | 来源 |
|---|---|---|
| `aes` | v0.8.4 vs v0.9.2 | aes-gcm (infra) vs zip (calamine/docx-rs/pptx-to-md/rbook) |
| `base64` | v0.13.1 vs v0.21.7 vs v0.22.1 | spm_precompiled (tokenizers) vs swift-rs (tauri) vs 直接使用 |
| (更多 119 个重复) | | |

### 4.2 重复依赖削减策略

1. 删除 tree-sitter 系列 → 消除其传递依赖
2. 删除 mistral.rs GPU 后端 → 消除 metal/cuda 相关传递依赖
3. 统一 base64 版本 → 升级 spm_precompiled 或替换
4. 统一 aes 版本 → 升级 aes-gcm 或降级 zip

---

## 5. 依赖削减预期

| 阶段 | 删除依赖数 (估) | 累计节点数 |
|---|---|---|
| Phase 0 (基线) | - | 1,765 |
| Phase 1 R1 (学术 RAG) | ~50 | ~1,715 |
| Phase 1 R2 (Agent) | ~20 | ~1,695 |
| Phase 1 R3 (LLM 过度工程) | ~200 | ~1,495 |
| Phase 1 R4 (工具/借鉴) | ~300 | ~1,195 |
| Phase 1 R5 (前端) | ~50 | ~1,145 |
| Phase 5 (依赖清理) | ~345 | ~800 |
| **目标** | | **~800** |

---

## 6. 供应链安全

| 工具 | 状态 | 配置 |
|---|---|---|
| `cargo audit` | ✅ 已配置 | RUSTSEC 数据库 |
| `cargo deny check` | ✅ 已配置 | `deny.toml` |
| `cargo deny check advisories` | ✅ | 安全公告检查 |
| `cargo deny check licenses` | ✅ | 许可证兼容检查 |
| `cargo deny check bans` | ✅ | 禁止依赖检查 |
| `cargo deny check sources` | ✅ | 来源检查 |

---

> **审计人**: AI Agent (Phase 0, 2026-08-27)
