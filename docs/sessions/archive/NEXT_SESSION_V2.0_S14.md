# EchoMind v2.0 深耕 S14 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S13）

前端测试增强完成。Vitest 从 440→716（+276 新测试，37 files）。新增 10 个测试文件覆盖核心模块（main/chat/settings/sidebar/import/markdown/ipc/state/export/security）。修复 1 个失败测试（state-enhanced subscribe 验证替代引用相等性）。零回归：esbuild 1626 行、E2E 1353 passed。

## 测试基线

- Vitest: **716 passed**, 37 files, 0 failed
- esbuild: 1626 行 / tsc: 2 pre-existing errors
- E2E bridge: 1353 passed, 2 pre-existing failed, 10 skipped (9.1m)
- Rust Free: 1155 passed / Rust Pro: 1573 passed

## 本 Session 任务：S14 — 高级模块测试增强（Phase 6 第二步）

**目标**：Vitest 716→≥836，新增 ~120 个测试覆盖 10 个高级模块。

### 要测试的 10 个模块

| 模块 | 新增 | 覆盖重点 |
|---|---|---|
| `graph-renderer.js` | 15 | D3 渲染 / 缩放 / 高亮 / 导出 |
| `graph-viewer.js` | 10 | 面板 / 布局切换 / 路径分析 |
| `context-menu.js` | 12 | 右键菜单 / 导出 / 重分类 |
| `message-edit.js` | 12 | 编辑 / 分支 / IME 防护 |
| `turn-tree.js` | 10 | 树视图 / 版本切换 |
| `wizard.js` | 10 | 首启向导 / 步骤验证 |
| `command-palette.js` | 10 | 命令面板 / 搜索 / 执行 |
| `perf-settings.js` | 10 | 智能模式 / 高级设置 |
| `keyboard-help.js` | 8 | 搜索过滤 / 分组 |
| `download-manager.js` | 8 | 下载 / 暂停 / 恢复 |

### 执行步骤

1. 读 `e2e-tests/tests/unit/` 下已有测试了解 Mock 结构（参考 `main.test.js`、`chat.test.js` 的 vi.mock 模式）
2. 为每个模块在 `e2e-tests/tests/unit/` 下创建 `*.test.js`，用 `vi.mock` 隔离 Tauri IPC / i18n / toast 等依赖
3. 每完成 3-4 个模块运行 `cd e2e-tests && npm run test:unit` 确认通过
4. 全部完成后：`node scripts/build-ui.mjs` + `npx tsc --noEmit`
5. E2E bridge 确认无回归：`cd e2e-tests && npm run test:bridge`
6. `./sync-to-github.sh`（测试文件不同步到发布仓库，预期无变更）
7. 更新 `docs/sessions/CURRENT_STATE.md` + `LOG.md` + 本衔接文件

### 约束

- 严禁 `unwrap()/expect()/panic!()`（仅限 Rust）
- AAA 模式 + 边界值测试 + Mock/Stub 隔离外部依赖
- Vitest + E2E 全量通过（零回归）
- `vi.hoisted()` 解决 mock 提升顺序问题（参考 `security-enhanced.test.js`）

### 后续

S15 补充模块测试（workspace/kb-stats/offline-detector 等 8 模块 → Vitest ≥806）→ S16 Draft REQ 分类 → S17 文档完善 → S18 v2.0 发布

### 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` | 深耕原子计划（S1-S18 全景） |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `e2e-tests/tests/unit/` | Vitest 测试目录（37 files, 716 tests） |
| `ui/src/` | 前端模块源码（74 个 JS 文件） |
| `scripts/build-ui.mjs` | esbuild 打包 |
