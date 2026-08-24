# EchoMind v2.0 深耕 S11 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。深耕计划见 `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md`。

## 上个 Session（S10）

借鉴命令清理完成。21 个开发者工具命令标记 `#[cfg(debug_assertions)]`，Release 构建中不注册不编译。IPC 从 ~233(Debug) 减至 ~212(Release)。前端 `ipc.js` 新增 `devInvoke` 静默降级 + `ragEvalApi` 封装。`rag-eval.js` 4 处直接 invoke 迁移到 `ragEvalApi`。

## 测试基线

- Rust Free: 1304 passed / Clippy Free+Pro 零警告 / fmt 通过
- tsc 5 pre-existing errors / esbuild 1626 行 / Vitest 440 passed
- E2E: 1355 passed 0 failed 10 skipped (8.9m)
- Release build: exit_code 0 (4m30s) 零 warning

## 本 Session 任务：S11 — 集成测试增强（Phase 4 第 1 步）

**目标**：集成测试覆盖率从 2.5% 提升至 15%，新增全链路/安全链路/本地 LLM 集成测试。

### 步骤

1. 分析现有 `crates/tauri-app/tests/integration.rs` 测试覆盖盲区
2. 新增全链路集成测试：导入→分块→嵌入→检索→对话→流式响应
3. 新增安全链路集成测试：加密→解锁→PII 检测→审计链验证
4. 新增本地 LLM 集成测试（Pro feature 门控）
5. 确保 `integration.rs` 不超过 2000 行（深耕计划 Phase 1 约束）

### 约束

- 严禁 `unwrap()/expect()/panic!()`
- 改源码后必须运行 `./sync-to-github.sh`
- E2E + Rust 测试全量通过（零回归）
- 集成测试使用 `*_inner` 函数复用命令逻辑（不需 Tauri runtime）

### 必读文件

- `docs/sessions/CURRENT_STATE.md`
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` §6 Phase 4
- `crates/tauri-app/tests/integration.rs`
- `crates/tauri-app/src/commands/mod.rs`（`*_inner` 函数索引）

### 后续

S12-S13 集成测试增强继续 → S14 Dead Code 清理 → S21 v2.0 发布
