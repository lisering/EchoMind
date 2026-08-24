# NEXT_SESSION v1.20 → v1.21 衔接提示词

## 项目状态

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate，版本 v1.20.0，1279 Rust passed / 0 failed / 31 ignored。220 Free IPC / 230 Pro。esbuild 19165 行。SRS 250 REQ（100 Done + 147 Implemented + 1 Draft + 1 Deprecated，完成率 99.2%）。

## 本 Session（v1.20）完成

REQ-WS-002 知识库配额独立计算 + REQ-WS-004 文档跨知识库迁移。WS 系列全部收尾。

### REQ-WS-002 实现
- 后端：`ImportService` 新增 `workspace_id` 字段 + `with_workspace()` 构造器 + `check_free_tier_limits` 改用 `count_documents_in_workspace` 替代全局 `count_documents`
- 前端：`#workspaceQuota` 配额显示（N/50 格式，接近 90% 高亮 warning）+ `get_workspace_quota` IPC
- Mock：`tauri-stub.js` `import_files` 配额改为 per-workspace + `get_workspace_quota` mock

### REQ-WS-004 实现
- 后端：`Storage.migrate_document()` trait 方法 + `SqliteStorage` 实现 `UPDATE documents SET workspace_id` + `migrate_document` IPC（验证目标库 + 配额检查 + 清缓存）
- 前端：`context-menu.js` 右键「迁移至…」菜单项 + `_showMigrateDialog()` 选择弹窗 + `_doMigrate()` 执行
- Mock：`tauri-stub.js` `migrate_document` mock（含配额检查）

### E2E 测试
`e2e-tests/tests/v1.20-features.spec.ts` — 6 tests all passed
- TC-V20-WS002-001~003：配额独立计数 / 切换同步 / 一库满不影响其他
- TC-V20-WS004-001~003：迁移到目标库 / 不重新嵌入 / 配额不足拒绝

## SRS 剩余 1 Draft REQ

- **REQ-DS-004** Dream Session 整理（自动会话摘要 + 知识提取）
- **REQ-NFR-019** 冷启动优化（首屏 < 3s）

（注：REQ-WS-002 和 REQ-WS-004 已在本 Session 从 Draft 升级为 Implemented，SRS Draft 数从 4 减至 2）

## 验证基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1279 passed, 31 ignored |
| Clippy | 零警告 |
| tsc | 0 errors |
| esbuild | 19165 行 |
| E2E (v1.20) | 6 passed |
| 版本号 | 1.20.0 |
| IPC | 220 Free / 230 Pro |
| SRS Draft | 2 个（DS-004 + NFR-019） |

## 关键文件变更

| 文件 | 变更 |
|---|---|
| `crates/core/src/import.rs` | ImportService 新增 workspace_id 字段 + with_workspace() + check_free_tier_limits 改用 per-workspace |
| `crates/core/src/lib.rs` | Storage trait 新增 migrate_document() 方法 |
| `crates/infra/src/sqlite_storage.rs` | 实现 migrate_document() SQL UPDATE |
| `crates/tauri-app/src/commands/import.rs` | import_files_inner + replace_document_inner 获取 workspace_id |
| `crates/tauri-app/src/commands/workspace.rs` | 新增 get_workspace_quota + migrate_document IPC |
| `crates/tauri-app/src/lib.rs` | 注册 2 新 IPC 命令 |
| `ui/src/workspace.js` | 新增 updateQuotaDisplay() + refreshQuotaDisplay() |
| `ui/src/context-menu.js` | 新增 migrateDoc 菜单项 + _showMigrateDialog() + _doMigrate() |
| `ui/index.html` | 新增 #workspaceQuota span |
| `ui/locales/en.json` / `zh-CN.json` | 新增 6 个 i18n 键 |
| `e2e-tests/tests/bridge/tauri-stub.js` | import_files per-workspace + get_workspace_quota + migrate_document mock |
| `e2e-tests/tests/v1.20-features.spec.ts` | 6 E2E 测试 |
| `docs/SRS_v1.0.md` | REQ-WS-002/004 Draft → Implemented + changelog |
| `docs/sessions/CURRENT_STATE.md` | 状态更新 |
