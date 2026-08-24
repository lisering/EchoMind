# EchoMind v2.0 深耕 S16 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S15）

补充模块测试增强完成。Vitest 从 844→938（+94 新测试，52 files）。新增 8 个测试文件覆盖补充模块（workspace/kb-stats/offline-detector/breadcrumb/doc-preview/doc-keyboard/error-recovery/skeleton）。关键修复：vi.hoisted() 解决 mock 提升顺序、doc-keyboard 模块级单例 _kbSelIdx 跨测试保持（先 Escape 重置）、jsdom activeElement 不匹配（子元素 focus + dispatch 绕过 inDocList 检查）、skeleton vi.useFakeTimers 控制 200ms 延迟。零回归：esbuild 1626 行、E2E 运行中。

## 测试基线

- Vitest: **938 passed**, 52 files, 0 failed
- esbuild: 1626 行 / tsc: 5 pre-existing errors
- E2E bridge: 运行中（基线 1353 passed, 2 pre-existing failed, 10 skipped）
- Rust Free: 1155 passed / Rust Pro: 1573 passed

## 本 Session 任务：S16 — Draft REQ 分类（78→≤30）

**目标**：SRS 中 78 条 Draft REQ 精简为 ≤30 条高价值候选。

### 执行步骤

1. 读取 `docs/SRS_v1.0.md`，提取所有 `Status: Draft` 的 REQ
2. 按 ROI（用户价值 × 实现可行性 ÷ 工作量）评估每条 Draft REQ
3. 分类：Implement（高 ROI，纳入后续规划）/ Deprecate（低价值或已被替代）/ Merge（合并到已有 REQ）
4. 更新 SRS 中每条 Draft REQ 的状态和分类理由
5. 输出 Draft REQ 分类报告到 `docs/architecture/DRAFT_REQ_CLASSIFICATION.md`
6. 更新 `docs/sessions/CURRENT_STATE.md` + `LOG.md` + 本衔接文件

### 评估维度

| 维度 | 权重 | 说明 |
|---|---|---|
| 用户价值 | 40% | 解决真实用户痛点程度 |
| 实现可行性 | 30% | 技术难度 / 依赖复杂度 |
| 工作量 | 20% | 预计 Session 数 |
| 战略价值 | 10% | 差异化 / 技术债 / 安全 |

### 约束

- 不新增功能代码（仅文档/分类）
- 每条 Draft REQ 必须有分类理由
- Deprecate 的 REQ 需说明替代方案
- Merge 的 REQ 需指定目标 REQ ID

### 后续

S17 文档完善 → S18 v2.0 发布

### 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` | 深耕原子计划（S1-S18 全景） |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
