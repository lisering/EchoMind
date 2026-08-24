# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.2.0 已发布。当前 **1560+ Free / 1831 Pro** 测试通过。fastembed ONNX 嵌入（5 种 EmbeddingModel：AllMiniLML6V2 384-dim 默认 / BgeSmallEnV1_5 384-dim 推荐 / BgeSmallZhV1_5 512-dim 中文 / E5SmallV2 384-dim / BgeM3 1024-dim 多语言），SQLite 向量存储 + HNSW 索引（Free，二进制加密持久化 + 懒加载 + 归一化点积），任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。

## 上 Session 完成内容（S90 全局搜索增强）

V2.4 原子计划 S90 全局搜索增强完成（REQ-IX-008 Implemented）。

### 实现内容

| 层 | 变更 | 关键文件 |
|---|---|---|
| 数据模型 | 新增 `GlobalSearchResults` + `DocumentSearchResult` + `EntitySearchResult` 结构体 | `crates/models/src/lib.rs` |
| Storage trait | 新增 `global_search` 方法（默认实现返回空结果） | `crates/core/src/lib.rs` |
| 后端实现 | `tokio::join!` 并行三路搜索：messages_fts FTS5 BM25 + documents LIKE + entities LIKE | `crates/infra/src/sqlite_storage.rs` |
| 文档搜索 | `search_documents` 函数（file_path + summary LIKE 匹配） | `crates/infra/src/storage/documents.rs` |
| 实体搜索 | `search_entities` 函数（entity_text LIKE 匹配 + JOIN chunks 获取 doc_id） | `crates/infra/src/storage/entities.rs` |
| IPC 命令 | `global_search` IPC 命令 + `global_search_inner` | `crates/tauri-app/src/commands/conversation.rs` |
| 命令注册 | 注册 `global_search` 命令 | `crates/tauri-app/src/lib.rs` |
| 前端增强 | `executeGlobalSearch` 单次 IPC 替代三次独立调用 + `renderSearchResults` 三组渲染 + `_handleResultClick` 实体跳转 | `ui/src/global-search.js` |
| 图谱跳转 | 新增 `openGraphViewerWithEntity` 函数 + `onOpenGraphViewer` 回调 | `ui/src/main.js` |
| i18n | 新增 `group_entities` 键（"Entities" / "实体"） | `ui/locales/{en,zh-CN}.json` |
| E2E Mock | `global_search` mock 返回三路搜索结果 | `e2e-tests/tests/bridge/tauri-stub.js` |
| E2E 测试 | TC-GSEARCH-001~006（面板打开 / 分组结果 / 消息跳转 / 文档跳转 / 实体跳转 / 中英文混合） | `e2e-tests/tests/v2.4-global-search.spec.ts` |
| 文档同步 | IPC 命令文档 + code-contracts 数据模型 + frontend-modules 模块描述 + SRS changelog v14.3.0 | `docs/api/` + `docs/SRS_v1.0.md` |

### 关键修复

1. **Rust 生命周期错误**：`search_documents` 中 `trimmed.to_lowercase()` 在闭包内引用导致 `'static` 约束 → 修复为在闭包外预先计算 `trimmed_lower` 再 move 进闭包
2. **Unused imports**：`sqlite_storage.rs` 和 `core/lib.rs` 中未使用的 `DocumentSearchResult` / `EntitySearchResult` 导入移除
3. **3 个 tsc 类型错误**：JSDoc `GlobalSearchResult` typedef 新增 `entityText` / `entityType` / `matchType` 可选属性 + `initGlobalSearch` JSDoc 新增 `onOpenGraphViewer` 回调参数
4. **E2E 导航错误**：`page.goto('/')` 无效 → 改用 `setupPage(page)` helper（注入 stub + 导航到 `file://` URL + 等待 `#app` 可见）

### 验证结果

| 验证项 | 结果 |
|---|---|
| Rust Free | 全通过, 0 failed |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| tsc | 0 errors |
| esbuild | 1641 行 |
| Vitest | 1767 passed (88 files) |
| E2E (global-search) | 6/6 passed |
| sync-to-github | 11 files committed, 385 ins, 157 del |

### 关键设计

- **统一 IPC**：`global_search` 单次调用替代前端三次独立调用（`search_conversations` + `get_documents` 过滤 + `get_graph_data` 过滤），减少 IPC 往返延迟
- **并行搜索**：后端使用 `tokio::join!` 并行执行三路搜索（messages_fts / documents / entities），总延迟 = max(三路) 而非 sum(三路)
- **分组展示**：前端按消息 / 文档 / 实体三组渲染，每组 top-5，空组不渲染
- **实体跳转**：点击实体结果 → `openGraphViewerWithEntity(entityText)` → 打开知识图谱查看器 → 延迟 800ms 在搜索框输入实体名称并触发搜索定位
- **向后兼容**：`Storage` trait 的 `global_search` 默认实现返回空结果，不影响其他适配器

## 测试基线

- Rust Free: 全通过, 0 failed, ~32 ignored
- Clippy Free: 零警告
- cargo fmt: 通过
- tsc: 0 errors
- esbuild: 1641 行
- Vitest: 1767 passed (88 files)
- 版本号: 2.2.0（v2.4 原子计划进行中，S86 ✅ S87 ✅ S88 ✅ S90 ✅ 已完成）

## V2.4 原子计划进度

| Session | 功能 | REQ | ROI | 预计天数 | 状态 |
|---|---|---|---|---|---|
| S86 | 对话书签/收藏 | REQ-RAG-053 | 11.0 | 0.5 | ✅ 已完成 |
| S87 | 自动更新检查 | REQ-HELP-004 | 10.6 | 0.5 | ✅ 已完成 |
| S88 | 对话模板导入/导出 | REQ-RAG-054 | 9.2 | 0.5 | ✅ 已完成 |
| S89 | macOS 代码签名与公证 | REQ-SEC-021 | 6.0 | 1 | ⏳ 待实施 |
| S90 | 全局搜索增强 | REQ-IX-008 | 5.4 | 1 | ✅ 已完成 |
| S91 | 批量操作增强 | REQ-ING-024 | 5.2 | 1 | ⏳ 待实施 |
| S92 | V2.4 发布准备 | — | — | 0.5 | ⏳ 待实施 |

详见 `docs/architecture/V2.4_ATOMIC_PLAN.md`。

## 下个 Session 任务

### 优先：S91 批量操作增强（REQ-ING-024，ROI 5.2，1 天）

S89 macOS 签名需要 Apple Developer 证书（用户提供），若无证书可跳过。**建议先做 S91（无外部依赖，纯代码实现）**。

#### 实现步骤

1. 写 SRS 规格定义（REQ-ING-024 Draft → Implemented，补充契约和测试编号）
2. 前端 `ui/src/sidebar.js` 文档列表新增复选框 + 全选/反选 + 批量操作工具栏
3. 后端 `crates/tauri-app/src/commands/document.rs` 新增 3 个 IPC 命令：
   - `batch_delete_documents(ids: Vec<String>) -> BatchResult` — 批量删除
   - `batch_move_documents(ids: Vec<String>, target_workspace_id: String) -> BatchResult` — 批量移动到工作空间
   - `batch_add_tags(ids: Vec<String>, tags: Vec<String>) -> BatchResult` — 批量添加标签
4. 后端 `crates/infra/src/sqlite_storage.rs` 实现 3 个批量操作方法
5. 验收标准：
   - AC-1：文档列表每行新增复选框，支持全选/反选（TC-ING-BATCH-001）
   - AC-2：选中 ≥ 1 个文档时显示批量操作工具栏（删除/移动/标签）（TC-ING-BATCH-002）
   - AC-3：批量删除前显示确认对话框含文档数量（TC-ING-BATCH-003）
   - AC-4：批量移动到其他工作空间（下拉选择目标）（TC-ING-BATCH-004）
   - AC-5：批量添加标签（输入标签名，多个用逗号分隔）（TC-ING-BATCH-005）
   - AC-6：操作完成后显示成功/失败统计（TC-ING-BATCH-006）
6. `tauri-stub.js` 新增 3 个 mock
7. E2E 测试 TC-ING-BATCH-001~006
8. i18n 键（en + zh-CN）

#### 关键文件

- `ui/src/sidebar.js` — 文档列表 UI（复选框 + 批量工具栏）
- `crates/tauri-app/src/commands/document.rs` — 3 个新 IPC 命令
- `crates/infra/src/sqlite_storage.rs` — 批量 DELETE/UPDATE
- `crates/core/src/lib.rs` — Storage trait 新增 3 个方法

#### 关键约束

- 批量操作前显示确认对话框（防误操作）
- 批量删除使用事务（全成功或全回滚）
- `BatchResult` 结构体：`{ success_count: usize, failed_count: usize, failed_ids: Vec<String> }`

### 备选：S89 macOS 代码签名与公证（REQ-SEC-021，ROI 6.0，1 天）

需要 Apple Developer 账号（用户提供证书 + API Key）。若无证书则跳过公证步骤，仅配置签名管线。

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
| `docs/SRS_v1.0.md` | 需求规格唯一依据（REQ-IX-008 已 Implemented，REQ-ING-024 Draft） |
| `docs/architecture/V2.4_ATOMIC_PLAN.md` | V2.4 原子计划（S86 ✅ S87 ✅ S88 ✅ S90 ✅ / S89 S91 S92 ⏳） |
| `ui/src/sidebar.js` | S91 起点：文档列表 UI + 批量选择模式 |
| `crates/tauri-app/src/commands/document.rs` | S91 起点：文档管理 IPC（delete_document 已有，新增 batch_*） |
| `crates/infra/src/sqlite_storage.rs` | S91 起点：SQLite 批量操作 |
| `crates/core/src/lib.rs` | Storage trait（S91 新增 batch_delete/batch_move/batch_add_tags） |
| `crates/tauri-app/src/lib.rs` | Tauri command setup + run()（注册新 IPC） |
| `crates/tauri-app/tauri.conf.json` | S89 起点：macOS 签名配置 |
| `e2e-tests/tests/bridge/tauri-stub.js` | E2E mock（S91 新增 batch_* mock） |
| `ui/locales/{en,zh-CN}.json` | i18n（S91 新增 batch 操作文案） |
