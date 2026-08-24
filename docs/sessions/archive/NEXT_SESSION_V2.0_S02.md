# EchoMind v2.0 深耕计划 — Session S02 提示词

> 复制下方 `---` 分隔线之间的全部内容粘贴到新会话即可。

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate（models → prompt → core → infra → tauri-app + compact + context + license-issuer）。面向律师/研究员等高隐私场景用户。v1.22.0 已发布，现在进入 **v2.0 深耕阶段** — 不新增功能，专注代码质量/测试/CI/文档。

## 上一个 Session（S01）完成情况

S01 已完成 `sqlite_storage.rs` 第一阶段拆分：
- 新建 `crates/infra/src/storage/` 子模块：`schema.rs`（433行，SQL DDL 常量）、`migration.rs`（390行，init_schema/migrate_schema 等）、`crypto.rs`（103行，AES-256-GCM 加解密）、`mod.rs`（27行，重导出）
- `sqlite_storage.rs` 从 5,641 行减至 4,805 行（-836 行）
- `IntegrityCheckResult` 枚举保留在 `sqlite_storage.rs`（pub API，外部 crate 引用）
- `storage` 模块通过 `#[path = "storage/mod.rs"] pub(crate) mod storage;` 声明
- 验证全通过：build / 185 tests / clippy 零警告 / fmt 通过
- sync-to-github 已提交（5 files, 991 insertions）

## 测试基线（S01 完成后）

- Rust Free: 1294 passed / Clippy 零警告 / fmt 通过
- `cargo test -p echomind-infra`: 185 passed, 0 failed, 19 ignored
- `sqlite_storage.rs`: 4,805 行（目标 S02 后 ≤ 4,000 行）

## 本 Session 任务：Phase 1 S02 — `sqlite_storage.rs` 拆分（文档/会话/消息表，第 2 部分）

**目标文件**: `crates/infra/src/sqlite_storage.rs`（当前 4,805 行 → 拆出 ~1,600 行到 3 个新模块）

### 拆分方案

在已有的 `crates/infra/src/storage/` 目录中新增 3 个文件：

```
crates/infra/src/storage/
  ├── mod.rs              — 已有，需更新重导出
  ├── schema.rs           — 已有（S01）
  ├── migration.rs        — 已有（S01）
  ├── crypto.rs           — 已有（S01）
  ├── documents.rs        — 新建 ~800 行
  ├── conversations.rs   — 新建 ~800 行
  └── messages.rs         — 新建 ~500 行
```

### 第 1 步：创建 `storage/documents.rs`（~800 行）

迁移以下 `SqliteStorage` 的 `impl Storage` 方法到自由函数（接收 `&Pool` 参数）：

- `add_document()`, `update_doc_status()`, `find_document_by_hash()`
- `count_documents()`, `count_chunks()`, `count_embeddings()`
- `cleanup_zombies()`, `list_documents()`, `list_documents_paginated()`
- `get_document_preview()`, `get_document_chunks()`, `reclassify_document()`
- `add_document_tag()`, `remove_document_tag()`, `list_all_tags()`, `filter_documents_by_tag()`
- `add_import_log()`, `get_import_logs()`, `clear_import_logs()`, `get_kb_stats()`
- `migrate_document()`, `list_documents_in_workspace()`, `count_documents_in_workspace()`

### 第 2 步：创建 `storage/conversations.rs`（~800 行）

- `create_conversation()`, `list_conversations()`, `list_conversations_paginated()`
- `count_conversations()`, `delete_conversation()`, `get_conversation()`
- `update_conversation_title()`, `reorder_conversations()`
- `create_workspace()`, `list_workspaces()`, `rename_workspace()`, `delete_workspace()`
- `get_workspace_stats()`, `get_workspace_quota()`
- `add_bookmark()`, `remove_bookmark()`, `list_bookmarks()`, `is_bookmarked()`

### 第 3 步：创建 `storage/messages.rs`（~500 行）

- `add_message()`, `list_messages()`, `list_messages_paginated()`, `delete_message()`
- `edit_message()`, `set_entry_security_tainted()`, `get_entry_security_tainted()`
- `search_messages()` (FTS5)
- `turn_active_versions` 相关方法

### 第 4 步：更新 `storage/mod.rs`

添加 `pub(crate) mod documents; pub(crate) mod conversations; pub(crate) mod messages;` + 重导出。

### 第 5 步：修改 `sqlite_storage.rs`

- 将迁移的方法从 `impl Storage for SqliteStorage` 中删除
- 改为调用自由函数：如 `documents::add_document(&self.pool, ...).await`
- 保持 `Storage` trait 方法签名不变（外部调用方无感知）

### 第 6 步：验证

- `cargo build` 编译通过（全 workspace）
- `cargo test -p echomind-infra` 全部通过（185 tests）
- `cargo clippy --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 通过
- `sqlite_storage.rs` 行数 ≤ 4,000
- 新模块每个 ≤ 800 行
- `./sync-to-github.sh --commit` 同步

## 关键约束

1. **`Storage` trait 方法签名不变**：`impl Storage for SqliteStorage` 中的方法签名必须保持不变，内部委托给自由函数
2. **自由函数接收 `&Pool`**：拆出的函数签名为 `pub(crate) fn add_document(pool: &Pool, ...) -> Result<...>`，不持有 `&self`
3. **`pub(crate)` 可见性**：所有新函数使用 `pub(crate)`，通过 `mod.rs` 重导出
4. **逐步拆分**：先拆 documents.rs → 验证 → 再拆 conversations.rs → 验证 → 最后 messages.rs
5. **每步验证**：每拆一个模块就 `cargo test -p echomind-infra` 确认无回归

## 铁律提醒

- 严禁 `unwrap()/expect()/panic!()/unreachable!()`
- 改源码后必须运行 `./sync-to-github.sh`
- 文档同步：`docs/api/code-contracts.md` 更新模块路径
- 前端无变更（纯 Rust 后端拆分）

## 验证命令速查

```bash
cargo build                                       # 全 workspace 编译
cargo test -p echomind-infra                      # 基础设施 crate 测试
cargo clippy --all-targets -- -D warnings         # Lint
cargo fmt --check                                 # 格式检查
wc -l crates/infra/src/sqlite_storage.rs          # 确认行数减少
./sync-to-github.sh --commit                      # GitHub 同步
```

## 必读文件

- `docs/sessions/CURRENT_STATE.md` — 项目状态快照
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` — 完整深耕计划（21 Sessions / 9 Phases）
- `AGENTS.md` — 架构概览 + 数据流图

## 后续 Sessions 预览

| Session | 任务 |
|---|---|
| S03 | sqlite_storage.rs 拆分 — 向量/实体/其他表（第 3 部分） |
| S04 | integration.rs 拆分 — 第 1 部分 |
| S05 | integration.rs 拆分 — 第 2 部分 |
| S06 | features.rs 拆分 |
| S07-S08 | CI/CD 自动化 |
| S09-S10 | IPC 命令精简 |
| S11-S13 | 集成测试增强 |
| S14 | Dead Code 清理 |
| S15-S17 | 前端测试增强 |
| S18 | SRS Draft REQ 清理 |
| S19-S20 | 文档完善 |
| S21 | v2.0 发布 |

---
