# EchoMind v2.0 深耕 S10 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。深耕计划见 `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md`。

## 上个 Session（S09）

IPC 命令精简完成。22 个 `set_xxx` 命令合并为统一 `update_setting(key, value)` + 新增 `get_setting(key)`。`generate_handler!` 移除 22 个旧命令 + `get_compression_ratio`，新增 2 个统一命令。前端 7 个模块 22 处 invoke 调用迁移。tauri-stub.js 新增 update_setting/get_setting mock（22 键映射）。IPC 净减 20（243→~233）。

## 测试基线

- Rust Free: 1137 passed / Pro: 1573 passed
- Clippy 零警告 / fmt 通过
- tsc 5 pre-existing errors / esbuild 1626 行 / Vitest 440 passed
- E2E: 1355 passed 0 failed 10 skipped (8.7m)

## 本 Session 任务：S10 — 借鉴命令清理（开发者工具 `#[cfg(debug_assertions)]` 门控，21 命令）

**目标**：将 21 个开发者工具命令标记 `#[cfg(debug_assertions)]`，Release 构建中不注册，IPC 从 ~233 减至 ~212。

### 步骤

1. 在 `crates/tauri-app/src/lib.rs` 的 `generate_handler!` 中，对 21 个命令添加 `#[cfg(debug_assertions)]` 门控
2. 在 `crates/tauri-app/src/commands/` 源码中给对应 `#[tauri::command]` 函数添加 `#[cfg(debug_assertions)]`
3. 前端 `ui/src/ipc.js` 对开发者工具 API 添加 `__DEV_MODE__` 门控（Release 时静默跳过）
4. tauri-stub.js 保持不变（E2E 在 debug 模式运行）
5. 运行 `cargo build --release` 验证 Release 构建中 21 命令不注册

### 21 个开发者工具命令

- **Trace**(4): `get_recent_traces`, `get_trace_detail`, `clear_traces`, `get_trace_count`
- **Budget**(2): `get_budget_stats`, `set_budget_limit`
- **RAG Eval**(4): `evaluate_rag_response`, `evaluate_rag_batch`, `get_rag_eval_settings`, `set_rag_eval_settings`
- **Durable**(3): `admit_input`, `promote_input`, `get_pending_inputs`
- **Session Todo**(5): `add_session_todo`, `update_todo_status`, `get_session_todos`, `delete_session_todo`, `delete_session_todos`
- **Strip**(3): `strip_messages`, `strip_keeping_recent`, `preview_strip`

### 约束

- 严禁 `unwrap()/expect()/panic!()`
- 改源码后必须运行 `./sync-to-github.sh`
- E2E + Rust 测试全量通过（零回归）
- 前端 `npx tsc --noEmit` + `node scripts/build-ui.mjs` 成功

### 必读文件

- `docs/sessions/CURRENT_STATE.md`
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` §5.2 S10
- `crates/tauri-app/src/lib.rs`（`generate_handler!`）
- `crates/tauri-app/src/commands/features/trace.rs`
- `crates/tauri-app/src/commands/features/memory.rs`（durable/todo 在此）
- `crates/tauri-app/src/commands/conversation.rs`（strip 在此）
- `ui/src/ipc.js`

### 后续

S11-S13 集成测试增强 → S14 Dead Code 清理 → S21 v2.0 发布
