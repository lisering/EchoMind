# EchoMind v1.9.0 变更日志

> **发布日期**：2026-08-11 | **版本**：v1.9.0 | **上一版本**：v1.8.0

---

## 新增功能

### S1: 检索 & LLM 参数可配置（REQ-RAG-014 + REQ-RAG-015）

用户可在设置面板配置检索参数和 LLM 生成参数，参数持久化到 settings 表，对新查询生效。

**检索参数（REQ-RAG-014）**：
- `rag.top_k`（1-20，默认 8）— 检索 top-k 值
- `rag.score_threshold`（0.0-1.0，默认 0.0）— 最低相似度阈值
- `rag.chunk_expansion_enabled`（默认 true）— Chunk Expansion 开关
- `rag.chunk_expansion_window`（0-3，默认 1）— 扩展窗口大小

**LLM 生成参数（REQ-RAG-015）**：
- `llm.temperature`（0.0-2.0，默认 0.7）— 采样温度
- `llm.max_tokens`（256-8192，默认 4096）— 最大生成 token 数
- `llm.top_p`（0.0-1.0，默认 1.0）— nucleus sampling 概率阈值

**新增 IPC 命令**（4 个 Free）：
- `get_rag_params() -> RagParams`
- `set_rag_params(params: RagParams) -> ()`
- `get_generation_params() -> GenerationParams`
- `set_generation_params(params: GenerationParams) -> ()`

**新增数据模型**：
- `RagParams` 结构体（`crates/models/src/lib.rs`）+ `clamped()` 范围验证
- `GenerationParams` 结构体（`crates/models/src/lib.rs`）+ `clamped()` 范围验证

**关键文件变更**：
- `crates/models/src/lib.rs` — 新增 `RagParams` + `GenerationParams` 结构体 + 7 TDD 测试
- `crates/infra/src/openai_provider.rs` — 新增 `generation_params` 字段 + `with_generation_params()` builder + 请求体条件包含参数
- `crates/tauri-app/src/commands/settings.rs` — 新增 4 个 IPC 命令 + `_inner` 逻辑函数
- `crates/tauri-app/src/commands/mod.rs` — 导入 `RagParams` + `GenerationParams`
- `crates/tauri-app/src/commands/chat.rs` — `chat_inner` 读取 `rag.top_k`/`rag.score_threshold`/`rag.chunk_expansion_enabled` 替代硬编码 `DEFAULT_TOP_K`
- `crates/tauri-app/src/lib.rs` — 注册 4 个新 IPC 命令

**TDD 测试**（7 个）：
- TC-RAG-PARAMS-001~003：RagParams 默认值/clamp/serde 往返
- TC-LLM-PARAMS-001~003：GenerationParams 默认值/clamp/serde 往返
- TC-STRIP-RESULT-001：StripResult serde 往返（含新字段）

### S2: 文档列表排序（REQ-ING-008）

文档列表支持按导入时间（默认倒序）和文件名（A-Z / Z-A）排序，排序状态在当前会话内保持。

**新增 IPC 参数**：
- `get_documents(sort_by: Option<String>, sort_order: Option<String>)` — 扩展现有命令

**排序维度**：
| sort_by | sort_order | 说明 |
|---|---|---|
| `imported_at`（默认） | `desc`（默认） | 最新在前 |
| `imported_at` | `asc` | 最早在前 |
| `file_name` | `asc` | A-Z 字母序 |
| `file_name` | `desc` | Z-A 逆序 |

**关键文件变更**：
- `crates/tauri-app/src/commands/document.rs` — `get_documents` 新增排序参数 + `sort_documents()` 可测试函数 + 3 TDD 测试

**TDD 测试**（3 个）：
- TC-ING-SORT-001：默认排序 created_at desc
- TC-ING-SORT-002：file_name asc（A-Z 字母序）
- TC-ING-SORT-003：created_at asc（最早在前）

### S3: DS-03 Session Strip — StripResult 扩展

Session Strip 功能已在 v1.5 实现（`crates/core/src/session_strip.rs`），本次扩展 `StripResult` 结构体新增 `summary` 和 `kept_count` 字段，支持返回生成的摘要文本和保留的消息数量。

**StripResult 新增字段**：
- `summary: String` — 生成的摘要文本（空串表示未生成）
- `kept_count: usize` — 保留的消息数量

**关键文件变更**：
- `crates/models/src/lib.rs` — `StripResult` 新增 `summary` + `kept_count` 字段（`#[serde(default)]` 向后兼容）+ `PartialEq` derive
- `crates/core/src/session_strip.rs` — `strip_range` 返回结果包含 `summary` + `kept_count`
- `crates/core/src/session_strip_tests.rs` — 测试更新适配新字段

---

## 测试基线

| 指标 | v1.8.0 | v1.9.0 | 变化 |
|---|---|---|---|
| Rust Free | 1245 passed | **1255 passed** | +10 |
| Rust Failed | 0 | 0 | — |
| Rust Ignored | 31 | 31 | — |
| Clippy | 零警告 | 零警告 | — |
| cargo fmt | 通过 | 通过 | — |
| TypeScript | 0 errors | 0 errors | — |
| 版本号 | 1.8.0 | 1.9.0 | — |
| IPC 命令数 | 198 Free | **202 Free** | +4 |

---

## 零新增依赖

v1.9 未引入任何新的 Rust crate 依赖或前端 npm 包。

---

## 铁律检查

| 铁律 | 状态 |
|---|---|
| 铁律一 — 禁止代理 | ✅ 无网络请求变更 |
| 铁律二 — TDD | ✅ 10 个新 TDD 测试 |
| 铁律五 — 零 unwrap/expect/panic | ✅ 使用 Result 类型 |
| 铁律六 — 最新版本依赖 | ✅ 零新增依赖 |
| 铁律七 — 文档同步 | ✅ 本 CHANGELOG + V1.9_ATOMIC_PLAN.md |
| 铁律十一 — GitHub 同步 | ✅ sync-to-github.sh --commit |
