# EchoMind Q04 衔接提示词

> **复制下面 ```` ```text ```` 代码块中的全部内容作为下个 session 的第一条消息即可。**

```text
你正在继续 EchoMind（灵犀）项目的开发。这是 QM 借鉴原子计划的第 4 个 Session。

## 项目背景

EchoMind 是 Rust + Tauri v2 桌面 RAG 知识库应用。六边形架构 8 crate workspace（models / prompt / compact / core / infra / tauri-app / context / license-issuer）。前端 vanilla JS ES modules（50 文件），esbuild 打包。详见 AGENTS.md / CLAUDE.md。

## 已完成 Session 汇总

### Q01 Scratch-Promote 两层记忆 + Consolidation 引擎（已完成）
- `crates/models/src/lib.rs`：新增 `MemoryConsolidationAction` 枚举（Update/Delete/Add）+ `ScratchLogEntry` 结构体 + `ScratchConsolidationResult`
- `crates/core/src/lib.rs`：Storage trait 新增 4 个 scratch 方法（默认空操作）
- `crates/infra/src/sqlite_storage.rs`：新增 `scratch_logs` 表 + KNOWN_TABLES 16→17 + 4 方法实现
- `crates/core/src/memory_store.rs`：新增 `write_scratch()` + `consolidate_scratch<L>()` + `apply_consolidation_actions()` + `parse_consolidation_output()` + `MEMORY_CONSOLIDATION_PROMPT` 常量
- `crates/core/src/memory_store_tests.rs`：7 TDD 测试 TC-SCRATCH-001~007
- `crates/core/src/auto_dream.rs`：DreamEngine Phase 5 `consolidate_scratch_layer()`
- `crates/tauri-app/src/commands/features.rs`：2 IPC 命令（trigger_memory_consolidation / get_scratch_logs）

### Q02 Burst Buffer 延迟捕获 + Provenance 标记（已完成）
- `crates/models/src/lib.rs`：新增 `ProvenanceTag` 结构体（conversation_id/message_seq/captured_at/source_label）+ `new()` / `with_timestamp()` / `said_in_suffix()` 方法 + 4 单元测试 TC-MODEL-PROV-001~004
- `crates/core/src/memory_store.rs`：新增 `BurstTurn` 结构体 + `BurstBuffer` 结构体（pending/quiet_ms/max_turns/last_activity）+ `new()` / `with_config()` / `push()` / `should_flush()` / `pending_count()` / `is_empty()` / `drain()` / `flush()` 方法 + `flush_burst_turns()` 独立函数（聚合多轮 → LLM 提取 → scratch 写入 + provenance 后缀）
- `crates/core/src/memory_store_tests.rs`：10 TDD 测试 TC-BURST-001~010
- `crates/tauri-app/src/state.rs`：AppState 新增 `memory_burst_buffer: Arc<Mutex<BurstBuffer>>` 字段
- `crates/tauri-app/src/commands/features.rs`：3 IPC 命令（push_burst_turn / flush_memory_burst_buffer / get_burst_buffer_status）
- `crates/tauri-app/src/lib.rs`：注册 3 命令（164→167）
- `e2e-tests/tests/bridge/tauri-stub.js`：3 mock + burstBuffer state

### Q03 双阈值压缩 + 后台异步压缩（已完成）
- `crates/models/src/lib.rs`：新增 `CompactionKind` 枚举（Background/Synchronous）+ `as_str()` 方法 + `CompactionInfo` 新增 `compaction_kind: Option<CompactionKind>` 字段（`#[serde(default)]` 向后兼容）
- `crates/compact/src/lib.rs`：新增 `CompactionTrigger` 枚举（None/Background/Synchronous，`#[derive(Default)]`）+ `DualThresholdConfig` 结构体（soft_threshold/hard_threshold/max_entries/max_tokens，默认 0.7/0.9/50/8192）+ `from_settings()` 方法 + `estimate_history_tokens()` 粗估函数 + `check_compaction_needed()` 方法（双阈值判断：entry_ratio vs token_ratio 取 max）+ `try_acquire_background_compaction()` / `release_background_compaction()` 去重函数 + `run_background_compaction()` 错误吞咽执行函数 + `serde` 依赖新增
- `crates/compact/src/compact_tests.rs`：新增 12 TDD 测试 TC-COMPACT-DUAL-001~012（注册 `#[cfg(test)] mod compact_tests;`）
- `crates/tauri-app/src/state.rs`：AppState 新增 `compaction_pending: Arc<Mutex<HashSet<String>>>` 字段
- `crates/tauri-app/src/commands/chat.rs`：`chat_inner` 集成双阈值逻辑（Soft → spawn 后台 / Hard → 同步 / None → 跳过）

当前测试基线：compact 41 passed + 1 ignored / core 540 passed + 12 doc tests / models 40 / clippy Free 零警告 / fmt 通过 / 167 IPC 命令 / 8 crate workspace。

## 本 Session 目标：Q04 Token 级预算 + Entry Token 缓存 + Dangling Call 修复

借鉴 QM `harness/context-compaction.ts` 的 Token 级预算管理和 Entry Token 缓存，提升压缩精度。同时引入 Dangling Call 修复（Agent 模式下 tool_call 被中断时自动补全 tool_result）。

### QM 对比
- QM `harness/context-compaction.ts`：`estimateEntryTokens()` 带 LRU 缓存（50k 条），`recentEntryCountWithinBudget()` 从末尾向前保留尽可能多的消息
- QM `harness/tape-fold.ts`：`healDanglingCalls()` — 未配对的 tool_call 自动添加 `[interrupted]` 占位 result
- EchoMind 现状：`CompactionEngine` 使用 `bpe()` 逐条计算 token，无缓存；无 dangling call 修复

### 当前代码现状（关键 API）

**CompactionEngine**（`crates/compact/src/lib.rs`）：
- `check_compaction_needed()` — Q03 新增的双阈值检查（使用 `estimate_history_tokens()` 粗估）
- `compact()` / `compact_with_config()` — 同步压缩，使用 `bpe()` 逐条计算 token
- `try_acquire_background_compaction()` / `release_background_compaction()` / `run_background_compaction()` — Q03 新增的后台压缩去重 + 执行

**token_budget 模块**（`crates/compact/src/token_budget.rs`）：
- `estimate_tokens(text)` / `estimate_messages_tokens(messages)` — 使用 `bpe()` 计算 token（无缓存）
- `find_compaction_boundary(messages, recent_budget)` — 从末尾向前保留消息
- `IncrementalCompactor` — 增量压缩管理器

**CompactionInfo**（`crates/models/src/lib.rs`）：
- `compacted_count` / `total_tokens` / `compacted_tokens` / `token_limit` / `compaction_kind: Option<CompactionKind>`

### 改动文件
1. `crates/compact/src/lib.rs` — 新增 `TokenEstimator` 带 LRU 缓存 + `heal_dangling_calls()` 函数
2. `crates/compact/src/compact_tests.rs` — TDD 测试

### 核心实现设计
```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Token 估算器（借鉴 QM estimateEntryTokens 的 LRU 缓存）
pub struct TokenEstimator {
    cache: Arc<RwLock<HashMap<u64, usize>>>,
    max_cache_size: usize,
}

impl TokenEstimator {
    pub fn new(max_cache: usize) -> Self { ... }
    pub async fn estimate(&self, content: &str) -> usize { ... }
}

/// 修复悬空的 tool_call（借鉴 QM healDanglingCalls）
pub fn heal_dangling_calls(history: &mut Vec<ChatMessage>) { ... }
```

### TDD 测试（tc_token_cache_001~003 + tc_heal_dangling_001~003）
- `tc_token_cache_001`：相同内容第二次估算命中缓存
- `tc_token_cache_002`：缓存满时 LRU 驱逐最旧条目
- `tc_token_cache_003`：`recent_entry_count_within_budget` 正确从末尾保留
- `tc_heal_dangling_001`：单个未配对 tool_call 被修复
- `tc_heal_dangling_002`：多个未配对 tool_call 全部修复
- `tc_heal_dangling_003`：已配对的 tool_call 不受影响

### 验收标准
- cargo test 全部通过（Free + Pro）
- cargo clippy --all-targets -- -D warnings 零警告
- cargo fmt --check 通过
- TokenEstimator 带 LRU 缓存，避免重复 BPE 编码
- heal_dangling_calls 修复未配对的 tool_call

### 关键约束
1. **铁律**：TDD 先写红灯测试再实现；零 unwrap/expect/panic（workspace deny lint，测试模块可加 `#![allow(clippy::unwrap_used)]`）
2. **compact crate** 在 `crates/compact/src/lib.rs`，已有 `check_compaction_needed()` + `DualThresholdConfig` + 后台压缩函数
3. **token_budget 模块** 在 `crates/compact/src/token_budget.rs`，有 `estimate_tokens()` / `estimate_messages_tokens()` / `find_compaction_boundary()`
4. **ChatMessage** 在 `crates/models/src/lib.rs`，有 `role` / `content` / `sources` / `reasoning` / `turn_group` / `version` 字段
5. **Edition 2024** native async fn in trait，无 async-trait macro
6. **文档同步**：修改后运行 `./sync-to-github.sh --commit` 同步到发布仓库
7. **SRS changelog**：在 `docs/SRS_v1.0.md` 末尾添加 v7.5.0 changelog 条目
8. **NEXT_SESSION_PROMPT.md**：完成后更新 `docs/sessions/NEXT_SESSION_PROMPT.md` 为 Q05 衔接提示词

### 原子计划文档
完整计划详见 `docs/architecture/QM_BORROW_ATOMIC_PLAN.md`，Q04 部分约在 271-310 行。后续 Q05（安全态势分层 + Security-Tainted 条目）约在 322-388 行。

请开始实施 Q04。
```
