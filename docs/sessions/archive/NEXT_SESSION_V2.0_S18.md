# EchoMind v2.0 深耕 S18 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S17）

文档完善完成。审查 `docs/api/` 下 5 个文档与 `docs/SOFTWARE_ASSET.md` 与实际代码一致性，修复大量不一致：ipc-commands 移除 12 个旧命令+新增 ~35 个缺失命令；code-contracts/database-schema/settings-reference/frontend-modules/SOFTWARE_ASSET 全部同步至 v2.0 状态。仅文档变更。

## 测试基线

- Vitest: 938 passed, 52 files, 0 failed
- esbuild: 1626 行 / tsc: 5 pre-existing errors
- E2E bridge: 1355 passed, 0 failed, 10 skipped (9.1m)
- Rust Free: 1155 passed / Rust Pro: 1573 passed

## 本 Session 任务：S18 — v2.0 发布准备

**目标**：v2.0 正式发布。

### 执行步骤

1. 全量回归测试（Rust Free/Pro + Clippy + fmt + Vitest + E2E + tsc + esbuild）
2. 版本号升级（`tauri.conf.json` + `Cargo.toml`）
3. Release build 验证（`cargo build --release --features pro`）
4. CHANGELOG_v2.0.md 编写
5. `./sync-to-github.sh --commit` GitHub 同步
6. git tag v2.0.0 创建
7. 更新 `docs/sessions/CURRENT_STATE.md` + `LOG.md` + 本衔接文件

### 约束

- 不新增功能代码（仅版本号 + CHANGELOG）
- 所有测试必须通过（0 failed）
- Release build 必须 exit_code 0

### 后续

v2.0 发布后进入新功能开发阶段（SRS Draft REQ 中 5 项 Implement 候选）

### 关键文件

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` | 深耕原子计划（S1-S18） |
| `docs/architecture/DRAFT_REQ_CLASSIFICATION.md` | S16 Draft REQ 分类报告 |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
