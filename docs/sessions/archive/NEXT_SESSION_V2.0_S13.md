# EchoMind v2.0 深耕 S13 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S12）

Dead Code 清理完成。删除 4 个 F 级模块文件（`workflow-panel.js` / `code-executor.js` / `durable-panel.js` / `session-strip.js`）+ 清理注释引用。Dead Code 文件归零。前端模块数 78→74。

## 测试基线

- esbuild: 1626 行 / Vitest: 440 passed / E2E: 1355 passed, 0 failed, 10 skipped (8.9m) / Rust: 1155 passed (Free)

## 本 Session 任务：S13 — 前端测试增强（Phase 6 第一步）

**目标**：Vitest 440→≥590，新增 ~150 个测试覆盖 10 个核心模块。

### 要测试的 10 个模块

| 模块 | 新增 | 覆盖重点 |
|---|---|---|
| `main.js` | 20 | boot 初始化 / 事件绑定 / 导入流程 |
| `chat.js` | 20 | 发送 / 中断 / 斜杠命令 / 临时 toggle |
| `settings.js` | 15 | open/close / Tab 导航 / 智能模式 |
| `sidebar.js` | 15 | 会话列表 / 搜索 / 拖拽 |
| `import.js` | 10 | 文件选择 / 拖拽 / 格式验证 |
| `markdown.js` | 15 | Mermaid / KaTeX / 代码高亮 / 表格 |
| `ipc.js` | 10 | IPC 封装 / 错误处理 |
| `state.js` | 10 | 状态同步 / 草稿 / 主题 |
| `export.js` | 10 | PDF 导出 / HTML 导出 |
| `security.js` | 15 | 加密 / 解锁 / PII / 审计 |

### 执行步骤

1. 先读 `e2e-tests/tests/unit/` 下已有测试（如 `action.test.js`、`guards.test.js`）了解测试结构
2. 为每个模块在 `e2e-tests/tests/unit/` 下创建 `*.test.js`，用 Mock/Stub 隔离 Tauri IPC
3. 每完成 3-4 个模块运行一次 `cd e2e-tests && npm run test:unit` 确认通过
4. 全部完成后运行：`node scripts/build-ui.mjs` + `npx tsc --noEmit`
5. 运行 E2E bridge 确认无回归：`cd e2e-tests && npm run test:bridge`
6. 运行 `./sync-to-github.sh --commit`
7. 更新 Session 文档（`docs/sessions/CURRENT_STATE.md` + `LOG.md` + 本衔接文件）

### 约束

- 严禁 `unwrap()/expect()/panic!()`
- 改源码后必须运行 `./sync-to-github.sh`
- Vitest + E2E 全量通过（零回归）
- AAA 模式 + 边界值测试 + Mock/Stub 隔离外部依赖

### 后续

S14 继续前端测试（模块 11-20）→ S15 收尾 → S16 Draft REQ 分类 → S17 文档完善 → S18 v2.0 发布
