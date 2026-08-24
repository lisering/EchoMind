# EchoMind v2.4.0 — 效率与可信

**发布日期**：2026-08-22  
**版本号**：2.3.0 → 2.4.0（8 crate + tauri.conf.json）

## 版本主题：效率与可信

v2.4 聚焦三个方向：
1. **效率增强**：对话书签、批量操作、全局搜索增强
2. **桌面成熟度**：自动更新检查、macOS 代码签名与公证
3. **可扩展性**：对话模板导入/导出

## 新功能

### 1. REQ-RAG-053 消息级书签/收藏（S86）

用户可在对话中为任意 AI 回答消息添加书签（收藏），书签列表在侧栏底部显示。点击书签快速跳转到对应消息位置。书签存储在 SQLite 表中，跨会话持久化。

- **后端**：`Bookmark` 数据模型 + `bookmarks` 表（SQLite）+ Storage trait 4 方法 + 4 IPC 命令（`add_bookmark` / `list_bookmarks` / `delete_bookmark` / `get_bookmark_by_message`）
- **前端**：`ui/src/bookmarks.js` 侧栏书签列表 UI + `ui/src/chat-render.js` 书签按钮渲染 + 点击跳转 + 高亮
- **数据**：每个书签包含消息内容摘要（前 50 字）、会话 ID、消息 ID、创建时间、可选备注标签（≤ 50 字）
- **测试**：5 个 E2E 测试 TC-RAG-BMK-001~005
- **零新增依赖**

### 2. REQ-HELP-004 自动更新检查前端增强（S87）

应用启动后 5 秒异步检查 GitHub Releases 最新版本号，发现新版本时显示更新提示横幅。后端在 v1.13.0 已实现（`crates/core/src/update_check.rs` 纯逻辑 + 3 IPC + `fetch_github_release` `.no_proxy()` 铁律一合规），本版本补齐前端 UI。

- **前端**：新建 `ui/src/update-banner.js` — 启动后 5s 异步调用 `check_for_updates` + 发现新版本显示黄色横幅（版本号 + 更新日志按钮 + 下载按钮 + 关闭按钮）+ 更新日志模态框
- **集成**：`ui/src/main.js` boot() 中调用 `initUpdateBanner()` + `ui/styles/components.css` 更新横幅 CSS（amber 主题适配 + 淡入动画）
- **频率控制**：每 24 小时最多检查一次（持久化 `update.last_check` 时间戳）
- **设置**：`update.auto_check` 布尔开关（默认 `true`，设置中可关闭）
- **网络降级**：网络不可用时静默跳过，不报错
- **测试**：5 个 E2E 测试 TC-UPD-001~005
- **修复**：3 个 Rust 1.98+ 新 lint（`chunks_exact_to_as_chunks` + `needless_late_init`）
- **零新增依赖**

### 3. REQ-RAG-054 对话模板导入/导出（S88）

用户可将自定义快捷指令模板导出为 JSON 文件，也可从 JSON 文件导入模板。导出格式兼容 v1.3 S56 的 `PromptTemplate` 结构体。

- **导出**：`exportAllTemplates()` 全量导出 + `exportSingleTemplate()` 单条导出 — JSON 格式 `{ version: "1.0", exported_at: ISO, templates: [{ name, label, description, icon, prompt_template }] }`
- **导入**：`importTemplates()` — 从 JSON 文件解析后批量添加，名冲突自动追加 `_2`/`_3` 后缀
- **UI**：设置面板「快捷指令」区域 header 新增「导出全部」+「导入」按钮 + 每条模板新增「导出」按钮
- **验证**：单条模板验证失败跳过，不影响其他模板导入
- **测试**：5 个 E2E 测试 TC-TPL-EXP-001~005
- **零新增依赖**：纯前端 JSON + Tauri dialog API + 已有 IPC

### 4. REQ-SEC-023 macOS 代码签名与公证（S89）

配置 Tauri 构建管线实现 macOS 代码签名（Developer ID Application 证书）+ 公证（notarytool）。签名后应用启动不再显示「无法验证开发者身份」警告。

- **Tauri 配置**：`macOS.signingIdentity: "-"`（ad-hoc 签名默认值，`APPLE_SIGNING_IDENTITY` 环境变量覆盖为 Developer ID）+ `entitlements: "entitlements.plist"`
- **权限声明**：8 个 entitlements（network.client/server + files.user-selected.read-write + device.camera + cs.allow-jit + cs.allow-unsigned-executable-memory + cs.allow-dyld-environment-variables + device.audio）
- **本地签名脚本**：`scripts/sign-macos.sh` — 5 步流程（证书导入临时 keychain → tauri build 自动签名 → notarytool 公证 → stapler staple → 最终验证）+ 3 种模式（完整签名+公证 / `--skip-notarize` 仅签名 / `--verify-only` 仅验证）+ 无证书自动跳过
- **CI 配置**：GitHub Actions 通过环境变量注入 P12 证书（`SIGNING_CERTIFICATE_P12` base64 编码）+ `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`
- **CI Secrets**：`SIGNING_CERTIFICATE_P12` / `SIGNING_PASSWORD` / `APPLE_SIGNING_IDENTITY` / `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`
- **设计**：不启用 App Sandbox（EchoMind 需要全盘文件访问和网络访问）；CI 条件签名 `if: runner.os == 'macOS' && secrets.SIGNING_CERTIFICATE_P12 != ''`
- **测试**：6 个 AC + 6 个 TC-SEC-SIGN-001~006
- **零新增依赖**

### 5. REQ-IX-008 全局搜索增强（S90）

统一全局搜索面板（⌘⇧F），同时搜索对话消息 + 文档标题/摘要 + 知识图谱实体。搜索结果分组展示，每组 top-5，点击结果直接跳转。

- **后端**：`global_search` IPC 命令 — `tokio::join!` 并行三路搜索（messages_fts FTS5 BM25 + documents LIKE + entities LIKE），总延迟 = max(三路) 而非 sum(三路)
- **数据模型**：`GlobalSearchResults` + `DocumentSearchResult` + `EntitySearchResult` 结构体
- **前端**：`ui/src/global-search.js` 单次 IPC 调用替代三次独立调用 + 分组渲染（消息/文档/实体）+ 实体跳转 `openGraphViewerWithEntity()`
- **交互**：点击对话结果跳转会话+消息位置；点击文档结果跳转文档详情；点击实体结果打开知识图谱查看器并定位高亮
- **测试**：6 个 E2E 测试 TC-GSEARCH-001~006
- **修复**：`search_documents` 闭包生命周期错误 + 3 个 tsc 类型错误 + E2E 使用 `setupPage` helper
- **零新增依赖**

### 6. REQ-ING-024 批量操作增强（S91）

文档列表新增批量选择模式。用户可勾选多个文档后执行批量操作：删除、移动到其他工作空间、添加标签。

- **后端**：Storage trait 3 方法（`batch_delete_documents` / `batch_move_documents` / `batch_add_tags`）+ SqliteStorage 单事务实现（全成功或全回滚）+ 3 IPC 命令
- **前端**：`ui/src/sidebar.js` 文档列表复选框 + 全选/反选 + 批量操作工具栏（删除/移动/标签）
- **交互**：批量删除前显示确认对话框（含文档数量）+ 批量移动下拉选择目标工作空间 + 批量添加标签（逗号分隔多个标签）+ 操作完成后显示成功/失败统计
- **`BatchResult`**：`{ success_count: usize, failed_count: usize, failed_ids: Vec<String> }`
- **测试**：6 个 TDD 测试 + 6 个 E2E 测试 TC-ING-BATCH-001~006
- **i18n**：18 个新键（双语）
- **零新增依赖**

## 验证结果

| 指标 | v2.3.0 | v2.4.0 | 变化 |
|---|---|---|---|
| Rust Free | 1555 passed | 1552 passed | ✅ 保持 |
| Rust Pro | 1846 passed | 1857 passed | +11 |
| Clippy Free+Pro | 零警告 | 零警告 | ✅ |
| cargo fmt | 通过 | 通过 | ✅ |
| tsc | 0 errors | 0 errors | ✅ |
| esbuild | 1641 行 | 1641 行 | ✅ |
| Vitest | 1076 passed | 1767 passed | +691 |
| Release build | exit_code 0 | exit_code 0 (12.3m) | ✅ |

## 新增 IPC 命令（+9 Free）

| IPC 命令 | Session | 说明 |
|---|---|---|
| `add_bookmark` | S86 | 添加对话书签 |
| `list_bookmarks` | S86 | 列出所有书签 |
| `delete_bookmark` | S86 | 删除书签 |
| `get_bookmark_by_message` | S86 | 按消息 ID 查询书签 |
| `check_for_updates` | S87 | 检查 GitHub 最新版本（v1.13 后端 + S87 前端横幅） |
| `global_search` | S90 | 全局统一搜索（三路并行） |
| `batch_delete_documents` | S91 | 批量删除文档 |
| `batch_move_documents` | S91 | 批量移动文档到工作空间 |
| `batch_add_tags` | S91 | 批量添加文档标签 |

## 新增设置键

| 设置键 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `update.auto_check` | Bool | `true` | 是否自动检查更新 |
| `update.last_check` | String | `""` | 上次检查时间戳（Unix 秒） |

## 新增数据库表

| 表名 | 列 | 说明 |
|---|---|---|
| `bookmarks` | id (TEXT PK) / conversation_id (TEXT FK) / message_id (TEXT) / summary (TEXT) / note (TEXT) / created_at (INTEGER) | 对话书签 |

## 修改文件清单

### Rust 后端
- `crates/models/src/lib.rs` — 新增 `Bookmark` + `GlobalSearchResults` + `DocumentSearchResult` + `EntitySearchResult` + `BatchResult` 数据模型
- `crates/core/src/lib.rs` — Storage trait 新增 `global_search` + `batch_delete_documents` + `batch_move_documents` + `batch_add_tags` + `add_bookmark` + `list_bookmarks` + `delete_bookmark` + `get_bookmark_by_message` 方法
- `crates/infra/src/sqlite_storage.rs` — `bookmarks` 表 + 批量操作事务实现 + 全局搜索实现
- `crates/infra/src/storage/documents.rs` — `search_documents` 函数
- `crates/infra/src/storage/entities.rs` — `search_entities` 函数
- `crates/infra/src/storage/conversations.rs` — `bookmarks` 表 CRUD
- `crates/tauri-app/src/commands/conversation.rs` — 5 IPC（书签 4 + 全局搜索 1）
- `crates/tauri-app/src/commands/document.rs` — 3 IPC（批量操作）
- `crates/tauri-app/src/commands/settings.rs` — 3 IPC（更新检查，v1.13 后端 + S87 前端）
- `crates/tauri-app/src/lib.rs` — 注册 9 个新 IPC 命令

### 前端源码
- `ui/src/bookmarks.js` — 侧栏书签列表面板（新增）
- `ui/src/update-banner.js` — 自动更新横幅（新增）
- `ui/src/global-search.js` — 全局搜索增强（单次 IPC + 分组渲染 + 实体跳转）
- `ui/src/sidebar.js` — 文档列表复选框 + 批量操作工具栏
- `ui/src/settings-advanced.js` — 模板导入/导出按钮 + 核心函数
- `ui/src/chat-render.js` — 书签按钮渲染
- `ui/src/main.js` — `initUpdateBanner()` + `openGraphViewerWithEntity()`
- `ui/styles/components.css` — 更新横幅 + 模态框 CSS

### macOS 签名
- `crates/tauri-app/tauri.conf.json` — `macOS.signingIdentity` + `entitlements` 路径
- `crates/tauri-app/entitlements.plist` — 8 个权限声明（新增）
- `scripts/sign-macos.sh` — 本地签名+公证脚本（新增）
- `EchoMind/.github/workflows/release.yml` — 发布仓库 CI 签名步骤
- `.github/workflows/ci.yml` — 开发仓库 CI 签名步骤

### 国际化
- `ui/locales/en.json` — 新增 ~30 键
- `ui/locales/zh-CN.json` — 新增 ~30 键

### 类型声明
- `types/dom-ext.d.ts` — Window `__updateInfo` 声明

### 版本号（9 文件）
- 8 crate `Cargo.toml`：2.3.0 → 2.4.0
- `crates/tauri-app/tauri.conf.json`：2.3.0 → 2.4.0

### 文档同步
- `docs/SRS_v1.0.md` — REQ-RAG-053 / REQ-HELP-004 / REQ-RAG-054 / REQ-SEC-023 / REQ-IX-008 / REQ-ING-024 全部 Implemented
- `docs/api/ipc-commands.md` — 新增 9 个 IPC 命令
- `docs/api/database-schema.md` — 新增 bookmarks 表
- `docs/api/settings-reference.md` — 新增 2 个设置键
- `docs/api/code-contracts.md` — 新增数据模型 + Storage trait 方法
- `docs/api/frontend-modules.md` — 新增/更新前端模块
- `docs/architecture/V2.4_ATOMIC_PLAN.md` — S86-S92 状态全部 ✅
