# EchoMind v2.0 深耕 S17 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S16）

Draft REQ/AC 分类完成。扫描 SRS 254 个 REQ，提取 10 个 Draft 项（4 Draft REQ + 6 Draft AC）。按 ROI 分类：Implement 5 项 + Deprecate 4 项 + Merge 1 项。SRS 状态全部更新，分类报告输出到 `docs/architecture/DRAFT_REQ_CLASSIFICATION.md`。原计划 78→≤30，实际 10→5 项待实现。仅文档变更。

## 测试基线

- Vitest: 938 passed, 52 files, 0 failed
- esbuild: 1626 行 / tsc: 5 pre-existing errors
- E2E bridge: 1355 passed, 0 failed, 10 skipped (9.1m)
- Rust Free: 1155 passed / Rust Pro: 1573 passed

## 本 Session 任务：S17 — 文档完善

**目标**：用户文档 + API 文档同步率 100%。

### 执行步骤

1. 审查 `docs/api/` 下 5 个文档与实际代码一致性（ipc-commands / code-contracts / database-schema / settings-reference / frontend-modules）
2. 审查 `docs/SOFTWARE_ASSET.md` 与当前架构一致性
3. 修复文档不一致问题
4. 更新 `docs/sessions/CURRENT_STATE.md` + `LOG.md` + 本衔接文件

### 约束

- 不新增功能代码（仅文档同步）
- 每个文档变更需说明变更理由

### 后续

S18 v2.0 发布准备

### 关键文件

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` | 深耕原子计划（S1-S18） |
| `docs/architecture/DRAFT_REQ_CLASSIFICATION.md` | S16 Draft REQ 分类报告 |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
