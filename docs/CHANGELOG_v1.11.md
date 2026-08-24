# EchoMind v1.11.0 变更日志

> **发布日期**：2026-08-11 | **版本**：v1.10.0 → v1.11.0

---

## 新增功能

### REQ-ING-010 文档内容预览
- **功能**：点击文档列表中的文档名弹出预览面板，显示文档元数据（文件名、格式、状态、导入时间、分块数、指纹）+ 前 500 字内容预览 + 分块列表（每个 chunk 前 200 字，可展开折叠）
- **后端**：新增 `get_document_preview` IPC 命令（Storage trait 默认实现，复用 `list_chunks`）
- **前端**：新增 `ui/src/doc-preview.js` 模块（预览面板 + Focus Trap + ESC 关闭）
- **交互**：文档列表项文件名可点击 + hover 显示 👁 预览按钮
- **数据模型**：新增 `DocumentPreview` + `ChunkPreview` 结构体

### REQ-IX-007 加载状态与骨架屏
- **功能**：文档列表和会话列表加载时显示骨架占位（200ms 延迟显示，避免闪烁），加载完成后自动移除
- **前端**：新增 `ui/src/skeleton.js` 模块（`showSkeleton(container, type, count)` + `hideSkeleton(container)`）
- **集成**：`loadDocuments()` 和 `loadConversations()` 中集成骨架屏调用
- **样式**：`animate-pulse` 占位块（文件名 + 状态徽标）

### REQ-RAG-013 单条消息删除
- **功能**：用户可删除对话中的单条消息。删除 user 消息时对应的 assistant 回答一并删除（保持问答对完整性）。删除操作需确认对话框
- **后端**：新增 `delete_message` IPC 命令（复用已有 `delete_messages_by_ids` + `list_messages`）
- **前端**：消息操作栏新增删除按钮（垃圾桶图标），user 和 assistant 消息均可删除
- **确认**：复用 `showConfirmDialog()`，user 消息提示「将同时删除对应的 AI 回答」

---

## 测试

| 类型 | 数量 | 说明 |
|---|---|---|
| Rust 单元+集成 | 1255 passed, 0 failed, 31 ignored | 无回归 |
| E2E 桥接 | 1051 + 9 = 1060 passed | 新增 9 TC-V11 测试 |
| Clippy | 零警告 | `cargo clippy --all-targets -- -D warnings` |
| cargo fmt | 通过 | `cargo fmt --check` |
| TypeScript | 0 errors | `npx tsc --noEmit` |
| esbuild | 17196 行 | `node scripts/build-ui.mjs` |

---

## 新增文件

| 文件 | 用途 |
|---|---|
| `ui/src/doc-preview.js` | 文档内容预览面板 |
| `ui/src/skeleton.js` | 骨架屏加载占位 |
| `docs/architecture/V1.11_ATOMIC_PLAN.md` | v1.11 原子规划 |
| `docs/CHANGELOG_v1.11.md` | 本变更日志 |
| `e2e-tests/tests/v1.11-features.spec.ts` | 9 个 E2E 桥接测试 |

---

## 修改文件

| 文件 | 改动 |
|---|---|
| `crates/models/src/lib.rs` | 新增 `DocumentPreview` + `ChunkPreview` 结构体 |
| `crates/core/src/lib.rs` | Storage trait 新增 `get_document_preview` 默认方法 |
| `crates/tauri-app/src/commands/document.rs` | 新增 `get_document_preview` IPC |
| `crates/tauri-app/src/commands/conversation.rs` | 新增 `delete_message` IPC |
| `crates/tauri-app/src/commands/mod.rs` | 导入 `DocumentPreview` |
| `crates/tauri-app/src/lib.rs` | 注册 2 个新命令 |
| `ui/src/ipc.js` | 新增 `docPreviewApi` + `convApi.deleteMessage` |
| `ui/src/main.js` | 文档预览按钮 + 骨架屏集成 |
| `ui/src/chat-render.js` | 消息删除按钮 |
| `ui/locales/{en,zh-CN}.json` | i18n 键 |
| `e2e-tests/tests/bridge/tauri-stub.js` | 2 个新 mock |
| `crates/tauri-app/tauri.conf.json` | 版本号 1.10.0 → 1.11.0 |

---

## 零新增依赖

全部 3 个功能使用已有 Rust crate + 前端库，无新增外部依赖。

---

## IPC 命令变更

| 命令 | 类型 | 说明 |
|---|---|---|
| `get_document_preview` | 新增 Free | 返回文档预览数据 |
| `delete_message` | 新增 Free | 删除单条消息（user 连带 assistant） |

IPC 命令数：202 Free / 214 Pro → **204 Free / 214 Pro**
