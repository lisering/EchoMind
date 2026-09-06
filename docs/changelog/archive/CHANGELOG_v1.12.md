# EchoMind v1.12.0 变更日志

> **发布日期**：2026-08-11 | **版本**：v1.11.0 → v1.12.0

---

## 新增功能

### REQ-EXP-004 文档原文导出
- **功能**：用户可从知识库导出文档的原始文件副本。文档列表项右键菜单提供「导出原文」选项，通过系统保存对话框选择保存位置。导出的文件与导入时完全一致（从数据目录复制原始副本）
- **后端**：新增 `export_document_original` IPC 命令（接收 `doc_id` + `dest_path`，使用 `tokio::fs::copy` 复制文件）
- **前端**：`ui/src/context-menu.js` 右键菜单新增「导出原文」+ `ui/src/ipc.js` 新增 `docExportApi.exportOriginal`
- **测试**：3 个 Rust 集成测试 TC-EXP-ORIG-001~003

### REQ-VEC-009 索引重建前端 UI + 取消支持
- **功能**：用户可对已索引文档发起「重建索引」操作。点击后弹出确认对话框，确认后文档状态变为 Processing，旧 chunks 与向量被清理，重新执行索引
- **后端**：新增 `rebuild_index` IPC 命令（封装 `retry_index_inner` 逻辑，清理旧 chunks → 重新索引 + 嵌入）
- **前端**：`ui/src/context-menu.js` Indexed 文档右键菜单新增「重建索引」+ 确认对话框
- **测试**：2 个 Rust 集成测试 TC-VEC-REBUILD-001~002

### REQ-I18N-003 数字与文件大小本地化
- **功能**：文件大小显示使用人类可读格式（B/KB/MB/GB/TB），采用 1024 进制，保留 1 位小数。大数字使用千分位分隔符。百分比显示保留整数。统一三处重复的 `formatBytes` 实现
- **前端**：`ui/src/utils.js` 增强 `formatBytes`（支持 GB/TB）+ 新增 `formatNumber`（千分位）+ `formatPercent`
- **去重**：`ui/src/kb-stats.js` 和 `ui/src/settings-local-llm.js` 移除重复 `formatBytes`，改为 import from utils.js
- **测试**：5 个 Vitest 单元测试 TC-I18N-FMT-001~005

### REQ-NAV-005 回到顶部与滚动定位
- **功能**：对话区滚动超过一屏后，右下角显示「回到顶部」浮动按钮。点击后平滑滚动到对话区顶部。流式生成中自动滚动到底部，用户手动向上滚动后暂停自动滚动，显示「新消息」提示按钮
- **前端**：`ui/src/scroll-lock.js` 新增 `createBackToTopButton()` + 滚动检测 + 平滑滚动 + 「新消息」提示按钮
- **样式**：`ui/styles/components.css` 新增按钮样式
- **测试**：3 个 E2E 测试 TC-V12-SCROLL-001~003

### SRS 状态修正
- **REQ-ING-012 文档内容更新（重新导入）**：Draft → Implemented（`ImportOutcome::NameConflict` + `replace_and_import()` + `replace_document` IPC + 前端确认对话框均已实现）
- **REQ-VEC-011 分块参数可视化配置**：Draft → Implemented（`ChunkParams` struct + `get/set_chunk_params` IPC + 前端设置面板均已实现）

---

## 测试

| 类型 | 数量 | 说明 |
|---|---|---|
| Rust 单元+集成 | 1255+ passed, 0 failed | 新增 5 集成测试 |
| E2E 桥接 | 1060+ passed | 新增 v1.12-features.spec.ts |
| Clippy | 零警告 | `cargo clippy --all-targets -- -D warnings` |
| cargo fmt | 通过 | `cargo fmt --check` |
| TypeScript | 0 errors | `npx tsc --noEmit` |
| Vitest | 419+ passed | 新增 5 formatBytes/formatNumber/formatPercent 测试 |

---

## 修改文件

| 文件 | 改动 |
|---|---|
| `crates/tauri-app/src/commands/document.rs` | 新增 `export_document_original` IPC |
| `crates/tauri-app/src/commands/import.rs` | 新增 `rebuild_index` IPC |
| `crates/tauri-app/src/lib.rs` | 注册 2 个新命令 |
| `crates/tauri-app/tests/integration.rs` | 5 个新集成测试 |
| `ui/src/ipc.js` | 新增 `docExportApi.exportOriginal` + `docApi.rebuild` |
| `ui/src/context-menu.js` | 右键菜单新增「导出原文」+「重建索引」 |
| `ui/src/utils.js` | 增强 `formatBytes` + 新增 `formatNumber` / `formatPercent` |
| `ui/src/kb-stats.js` | 移除重复 `formatBytes`，改为 import |
| `ui/src/settings-local-llm.js` | 移除重复 `formatModelSize`，改为 import |
| `ui/src/scroll-lock.js` | 新增 `createBackToTopButton()` + 滚动管理 |
| `ui/src/chat.js` | 集成 scroll-manager |
| `ui/styles/components.css` | 「回到顶部」按钮样式 |
| `ui/locales/{en,zh-CN}.json` | i18n 键 |
| `e2e-tests/tests/bridge/tauri-stub.js` | 2 个新 mock |
| `e2e-tests/tests/v1.12-features.spec.ts` | 新增 E2E 测试 |
| `e2e-tests/tests/unit/utils.test.js` | 5 个新 Vitest 测试 |
| `crates/tauri-app/tauri.conf.json` | 版本号 1.11.0 → 1.12.0 |

---

## 零新增依赖

全部 4 个功能使用已有 Rust crate + 前端模块，无新增外部依赖。

---

## IPC 命令变更

| 命令 | 类型 | 说明 |
|---|---|---|
| `export_document_original` | 新增 Free | 导出文档原始文件副本 |
| `rebuild_index` | 新增 Free | 重建文档索引（封装 retry_index） |

IPC 命令数：204 Free / 214 Pro → **206 Free / 214 Pro**
