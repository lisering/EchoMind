# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.2.0 已发布。当前 **1566 Free / 1831 Pro** 测试通过。fastembed ONNX 嵌入（5 种 EmbeddingModel：AllMiniLML6V2 384-dim 默认 / BgeSmallEnV1_5 384-dim 推荐 / BgeSmallZhV1_5 512-dim 中文 / E5SmallV2 384-dim / BgeM3 1024-dim 多语言），SQLite 向量存储 + HNSW 索引（Free，二进制加密持久化 + 懒加载 + 归一化点积），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容（S91 批量操作增强）

V2.4 原子计划 S91 批量操作增强完成（REQ-ING-024 Implemented）。

### 实现内容

| 层 | 变更 | 关键文件 |
|---|---|---|
| 数据模型 | 新增 `BatchResult` 结构体（success_count/failed_count/failed_ids + `all_success()` + `from_results()` 方法） | `crates/models/src/lib.rs` |
| Storage trait | 新增 3 个默认空操作方法（`batch_delete_documents` / `batch_move_documents` / `batch_add_tags`） | `crates/core/src/lib.rs` |
| SqliteStorage 实现 | 3 个方法均使用单事务（`unchecked_transaction` → prepare → execute 循环 → commit），批量删除级联 chunks_fts + documents + 向量缓存失效，批量移动 UPDATE workspace_id，批量标签 SELECT tags → JSON 解析 → 去重追加 → UPDATE | `crates/infra/src/sqlite_storage.rs` |
| IPC 命令 | 3 个 IPC 命令 + `*_inner` 函数（`batch_delete_documents` 含文件副本清理 + 缓存失效 + step_cache 清空 / `batch_move_documents` 含工作空间验证 + 配额检查 / `batch_add_tags` 含空标签过滤） | `crates/tauri-app/src/commands/document.rs` |
| 类型导入 | `commands/mod.rs` 导入 `BatchResult` | `crates/tauri-app/src/commands/mod.rs` |
| 命令注册 | 注册 3 个新命令 | `crates/tauri-app/src/lib.rs` |
| 前端增强 | `toggleSelectAll` 全选/反选 + `showBatchDeleteConfirm` 改用 `batch_delete_documents` IPC + `showBatchMoveDialog` 工作空间下拉选择 + `showBatchTagDialog` 逗号分隔标签输入 | `ui/src/sidebar.js` |
| HTML 增强 | `kbBatchBar` 新增全选复选框 + 移动/标签按钮 | `ui/index.html` |
| 确认对话框 | `customContent` 参数支持（下拉选择/输入框嵌入确认对话框） | `ui/src/confirm-dialog.js` |
| i18n | 新增 18 个批量操作键（en + zh-CN） | `ui/locales/{en,zh-CN}.json` |
| E2E Mock | 3 个 batch mock（delete/move/add_tags） | `e2e-tests/tests/bridge/tauri-stub.js` |
| TDD 测试 | TC-ING-BATCH-001~006（批量删除/移动/标签/空列表/幂等/缓存失效） | `crates/infra/src/storage_tests.rs` |
| E2E 测试 | TC-ING-BATCH-001~006（复选框/工具栏/删除确认/移动/标签/成功统计） | `e2e-tests/tests/v2.4-batch-ops.spec.ts` |
| 文档同步 | IPC 命令文档 + code-contracts 数据模型 + SRS changelog v14.4.0 | `docs/api/` + `docs/SRS_v1.0.md` |

### 关键修复

1. **编译错误**：`if let Ok(doc_name)` → `if let Some(doc_name)`（`file_name()` 返回 `Option` 非 `Result`）
2. **Clippy `collapsible_if`**：嵌套 `if let` → let-chain（`if let Some(...) && let Err(...) { ... }`）
3. **Clippy `cloned_ref_to_slice_refs`**：`&[doc1.id.clone()]` → `std::slice::from_ref(&doc1.id)`
4. **tsc 类型错误**：`showConfirmDialog` JSDoc 新增 `customContent` 参数声明

### 验证结果

| 验证项 | 结果 |
|---|---|
| Rust Free | 1566 passed, 0 failed |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| tsc | 0 errors |
| esbuild | 1641 行 |
| Vitest | 1767 passed (88 files) |

### 关键设计

- **单事务批量操作**：`SqliteStorage` 使用 `unchecked_transaction` → `prepare` → `execute` 循环 → `commit`，保证全成功或全回滚
- **BatchResult 结构体**：`{ success_count, failed_count, failed_ids }` 统一返回格式，前端可针对性提示失败文档
- **全选/反选**：`kbSelectAll` 复选框联动所有文档复选框
- **customContent 扩展**：`showConfirmDialog` 新增 `customContent` 参数支持嵌入自定义 DOM（下拉选择/输入框），复用统一确认对话框的防误触和键盘交互
- **向后兼容**：Storage trait 的 3 个批量方法默认实现逐条调用单条方法，不影响其他适配器

## 测试基线

- Rust Free: 1566 passed, 0 failed, ~32 ignored
- Clippy Free: 零警告
- cargo fmt: 通过
- tsc: 0 errors
- esbuild: 1641 行
- Vitest: 1767 passed (88 files)
- 版本号: 2.2.0（v2.4 原子计划进行中，S86 ✅ S87 ✅ S88 ✅ S90 ✅ S91 ✅ 已完成）

## V2.4 原子计划进度

| Session | 功能 | REQ | ROI | 预计天数 | 状态 |
|---|---|---|---|---|---|
| S86 | 对话书签/收藏 | REQ-RAG-053 | 11.0 | 0.5 | ✅ 已完成 |
| S87 | 自动更新检查 | REQ-HELP-004 | 10.6 | 0.5 | ✅ 已完成 |
| S88 | 对话模板导入/导出 | REQ-RAG-054 | 9.2 | 0.5 | ✅ 已完成 |
| S89 | macOS 代码签名与公证 | REQ-SEC-021 | 6.0 | 1 | ⏳ 待实施 |
| S90 | 全局搜索增强 | REQ-IX-008 | 5.4 | 1 | ✅ 已完成 |
| S91 | 批量操作增强 | REQ-ING-024 | 5.2 | 1 | ✅ 已完成 |
| S92 | V2.4 发布准备 | — | — | 0.5 | ⏳ 待实施 |

详见 `docs/architecture/V2.4_ATOMIC_PLAN.md`。

## 下个 Session 任务

### 优先：S89 macOS 代码签名与公证（REQ-SEC-021，ROI 6.0，1 天）

S89 macOS 签名需要 Apple Developer 证书（用户提供），若无证书可跳过。仅配置签名管线。

#### 实现步骤

1. 配置 `crates/tauri-app/tauri.conf.json` 的 `macOS.signingIdentity` + `entitlements`
2. 新建 `scripts/sign-macos.sh` 本地签名脚本
3. 配置 `.github/workflows/release.yml` CI 签名步骤（通过环境变量注入证书）
4. 验收标准：AC-1~AC-5（签名验证 + Gatekeeper + 公证）

### 后续：S92 V2.4 发布准备（0.5 天）

S89 + S91 完成后执行：

1. 版本号升级 2.2.0 → 2.4.0（8 crate Cargo.toml + tauri.conf.json）
2. CHANGELOG_v2.4.md 编写
3. 全量回归测试（Rust Free + Pro + Clippy + fmt + tsc + esbuild + Vitest + E2E）
4. Release build
5. GitHub tag

## 关键约束（铁律）

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现；生产代码严禁 `unwrap()/expect()/panic!()`
3. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
4. 修改源代码后运行 `./sync-to-github.sh --commit`
5. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成本文件
6. 先写规格再写代码（Spec-driven Development）
7. 前端样式铁律十三：Tailwind utility classes + design tokens，禁止硬编码 hex 颜色
8. `async fn in trait`（Edition 2024），无 `async-trait` 宏

## 验证命令

```bash
# Rust 测试
cargo test 2>&1 | grep -oE "[0-9]+ passed" | awk '{s+=$1} END {print "Free passed:"s}'
cargo test --features pro 2>&1 | grep -E "test result:" | awk '{print $4}' | awk -F, '{s+=$1} END {print "Pro passed:"s}'

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit

# 同步
./sync-to-github.sh --commit
```

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据（REQ-ING-024 已 Implemented，REQ-SEC-021 Draft） |
| `docs/architecture/V2.4_ATOMIC_PLAN.md` | V2.4 原子计划（S86 ✅ S87 ✅ S88 ✅ S90 ✅ S91 ✅ / S89 S92 ⏳） |
| `crates/tauri-app/tauri.conf.json` | S89 起点：macOS 签名配置 |
| `crates/tauri-app/src/commands/document.rs` | 文档管理 IPC（batch_* 已实现） |
| `crates/infra/src/sqlite_storage.rs` | SQLite 批量操作（已实现） |
| `crates/core/src/lib.rs` | Storage trait（batch_* 已实现） |
| `crates/tauri-app/src/lib.rs` | Tauri command setup + run()（已注册 batch_*） |
| `e2e-tests/tests/bridge/tauri-stub.js` | E2E mock（batch_* 已实现） |
| `ui/locales/{en,zh-CN}.json` | i18n（batch 操作文案已实现） |
