# EchoMind v1.19 Session 提示词

> 复制下方 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.18.0，1279 Rust passed / 0 failed / 31 ignored。211 Free IPC / 223 Pro。esbuild 18738 行。SRS 250 REQ（100 Done + 143 Implemented + 6 Draft + 1 Deprecated，完成率 97.2%）。

上一 Session（v1.18）：REQ-KB-002 命令面板（⌘K focus-trap 增强）+ REQ-NAV-002 全局搜索（⌘⇧F 新建 global-search.js，三类并行搜索 + 分组高亮 + 点击跳转）。11 E2E 全通过。

## 本 Session 任务：多知识库（Workspace）核心功能

SRS 剩余 6 Draft REQ，本 Session 实施 WS 系列核心 2 个（高 ROI，解锁 WS-002/004 依赖）。

### 功能 1: REQ-WS-001 多知识库创建与切换

SRS 描述：用户可创建多个知识库（工作空间），每个有独立名称、文档集、会话列表。侧栏顶部显示知识库选择器，切换后文档/会话列表同步切换。通过 `workspace_id` 字段隔离，持久化到 settings 表。

实现要点：
- **后端**：`workspaces` 表（id/name/created_at）+ `create_workspace` / `list_workspaces` / `switch_workspace` / `get_current_workspace` IPC 命令
- **DB 迁移**：documents 和 conversations 表已有 `workspace_id` 列（当前默认 'default'），新增 workspaces 元数据表
- **前端**：侧栏顶部知识库选择器（下拉）+ 新建知识库按钮 + 切换时刷新文档/会话列表 + 持久化当前选择

AC：选择器列出全部库 / 新建输入框 / 切换同步刷新 / 库间数据隔离 / 重启恢复选择

### 功能 2: REQ-WS-003 知识库重命名与删除

SRS 描述：重命名（双击或右键）+ 删除（级联清理文档/chunks/向量/会话/消息，确认对话框显示数据量，至少保留一个库）。

实现要点：
- **后端**：`rename_workspace` / `delete_workspace` IPC（delete 级联清理 + 返回将删除的数据量预览）
- **前端**：双击重命名编辑模式 + 删除确认对话框 + 最后一个库禁用删除

AC：双击重命名 / 即时更新 / 确认显示数据量 / 级联清理 / 最后一个库禁删

### 执行步骤

1. TDD 实现 REQ-WS-001（后端 IPC + DB 迁移 → 前端选择器 UI → E2E）
2. TDD 实现 REQ-WS-003（后端 rename/delete IPC → 前端确认对话框 → E2E）
3. 验证全链路
4. 更新 SRS 状态 + CURRENT_STATE.md
5. `./sync-to-github.sh --commit`

### E2E 测试

创建 `e2e-tests/tests/v1.19-features.spec.ts`，覆盖：
- WS-001：新建知识库 / 切换 / 数据隔离 / 持久化（~4 测试）
- WS-003：重命名 / 删除确认 / 级联清理 / 最后一个库禁删（~4 测试）

## 开发规范

- TDD：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- Tailwind design tokens，禁止硬编码 hex
- 前端构建：改 `ui/src/*.js` 后运行 `node scripts/build-ui.mjs`
- E2E 模式：`injectStub(page)` → `page.addInitScript(mock state)` → `injectLocales(page)` → `page.goto(uiUrl)` → 操作 → 断言。参考 `e2e-tests/tests/helpers.mjs`

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npx playwright test tests/v1.19-features.spec.ts --reporter=line
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1279 passed, 31 ignored |
| Clippy | 零警告 |
| tsc | 0 errors |
| esbuild | 18738 行 |
| E2E (v1.18) | 11 passed |
| 版本号 | 1.18.0 |
| IPC | 211 Free / 223 Pro |
| SRS Draft | 6 个（WS-001~004 + DS-004 + NFR-019） |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格（REQ-WS-001 第 3180 行，REQ-WS-003 第 3201 行） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `crates/infra/src/sqlite_storage.rs` | SQLite 存储（workspaces 表 + 迁移） |
| `crates/tauri-app/src/commands/` | IPC 命令目录 |
| `ui/src/sidebar.js` | 侧栏（知识库选择器挂载点） |
| `ui/src/main.js` | 前端入口（WORKSPACE 常量 + loadConversations） |
| `e2e-tests/tests/helpers.mjs` | E2E 工具函数 |
| `e2e-tests/tests/bridge/tauri-stub.js` | Mock 桥接（新增 workspace mock） |

---
