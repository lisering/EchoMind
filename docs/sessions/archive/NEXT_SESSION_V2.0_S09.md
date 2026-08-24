# EchoMind v2.0 深耕 S09 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。深耕计划见 `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md`。

## 上个 Session（S08）

CI 增强完成。在 S07 的 6 job 基础上增至 9 job：(1) 新增 `frontend-checks` job（tsc+esbuild+Vitest，独立并行）；(2) 新增 `release-build` job（`cargo tauri build --features pro` 产出 .dmg/.msi/.deb/.AppImage，仅 push to main 触发）；(3) CI 优化（test 双门控 lint+frontend-checks，e2e-bridge 依赖 test+frontend-checks，`workflow_dispatch` 手动触发）。PR 路径 ~18min ≤ 20min 目标 ✅。

## 测试基线

- Rust Free: 1137 passed / Pro: 1573 passed
- Clippy 零警告 / fmt 通过
- tsc 0 errors / esbuild 1622 行 / Vitest 440 passed
- E2E: 1354 passed 0 failed

## 本 Session 任务：S09 — IPC 命令精简（set/get 对合并，243 → ≤ 200）

**目标**：合并 ~30 对 set/get 命令为统一 `update_setting(key, value)` 命令，IPC 从 243 减到 ≤ 200。

### 步骤

1. 在 `crates/tauri-app/src/commands/settings.rs` 新增 `update_setting(key: String, value: String)` IPC 命令（内部按 key 分发到现有逻辑）
2. 前端 `ui/src/ipc.js` 新增 `settingsApi.update(key, value)` 封装
3. 前端逐步将 ~20 个 `set_xxx()` 调用迁移到 `settingsApi.update("xxx", value)`
4. 旧命令标记 `#[deprecated]` 保留（向后兼容），但从 `generate_handler!` 移除已迁移的
5. 保留不合并的常用命令：`set_locale`/`get_locale`、`set_theme`/`get_theme`、`set_chunk_params`/`get_chunk_params`（需类型验证）

### 可合并的命令（~20 个 set 迁移到 update_setting）

`set_hybrid_search` / `set_rerank_enabled` / `set_hyde_enabled` / `set_agent_enabled` / `set_coordinator_mode` / `set_sub_agent_enabled` / `set_embedding_model` / `set_compression_ratio` / `set_context_token_limit` / `set_vlm_enabled` / `set_progressive_injection` / `set_speculative_enabled` / `set_quality_gate_enabled` / `set_graph_retriever_enabled` / `set_memory_enabled` / `set_web_search_enabled` / `set_contextual_retrieval` / `set_close_to_tray` / `set_sidebar_collapsed` / `set_autostart`

### 约束

- 严禁 `unwrap()/expect()/panic!()`
- 改源码后必须运行 `./sync-to-github.sh`
- E2E + Rust 测试全量通过（零回归）
- 前端 `npx tsc --noEmit` 0 errors + `node scripts/build-ui.mjs` 成功

### 必读文件

- `docs/sessions/CURRENT_STATE.md`
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` §5 Phase 3 S09
- `crates/tauri-app/src/commands/settings.rs`
- `crates/tauri-app/src/lib.rs`（`generate_handler!`）
- `ui/src/ipc.js`

### 后续

S10 借鉴命令清理（开发者工具 `#[cfg(debug_assertions)]` 门控，21 命令）→ S11-S13 集成测试增强 → S14 Dead Code 清理 → S21 v2.0 发布
