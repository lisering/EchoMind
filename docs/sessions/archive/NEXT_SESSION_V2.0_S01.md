# EchoMind v2.0 深耕计划 — Session S01 提示词

> 复制下方 `---` 分隔线之间的全部内容粘贴到新会话即可。

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate（models → prompt → core → infra → tauri-app + compact + context + license-issuer）。面向律师/研究员等高隐私场景用户。250 SRS REQ 全部完成，v1.22.0 已发布。现在进入 **v2.0 深耕阶段** — 不新增功能，专注代码质量/测试/CI/文档。

## 测试基线（v1.22.0）

- Rust Free: 1294 passed / Rust Pro: 1573 passed / Clippy 零警告 / fmt 通过
- Vitest: 440 passed / tsc 0 errors / esbuild 1626 行
- E2E: 1354 passed, 0 failed, 10 skipped
- IPC: 242 命令 / 僵尸命令 0 / 幻影调用 0

## 本 Session 任务：Phase 1 S01 — `sqlite_storage.rs` 拆分（Schema + 核心）

**目标文件**: `crates/infra/src/sqlite_storage.rs`（当前 5,641 行 → 拆出 ~1,200 行到新模块）

**拆分方案**:

### 第 1 步：创建 `crates/infra/src/storage/` 目录

```
crates/infra/src/storage/
  ├── mod.rs          — pub use 重导出
  ├── schema.rs       — PRAGMAS + SCHEMA_TABLES + SCHEMA_INDEXES + KNOWN_TABLES + DOC_COLS + 常量
  ├── migration.rs    — init_schema / backfill_fts / migrate_schema / table_exists / has_column
  └── crypto.rs       — load_or_create_cipher / encrypt / decrypt / ensure_dir_0700 / ensure_file_0600
```

### 第 2 步：迁移代码

1. **schema.rs (~500 行)**: 所有 SQL DDL 常量（CREATE TABLE / CREATE INDEX / FTS 虚拟表 / 触发器）+ KNOWN_TABLES 白名单 + DOC_COLS 列常量 + IntegrityCheckResult 枚举 + NONCE_LEN / SECRET_KEY_FILE 等常量
2. **migration.rs (~500 行)**: init_schema() / backfill_fts_if_needed() / backfill_messages_fts_if_needed() / migrate_schema() / validate_table_name() / table_exists() / has_column()
3. **crypto.rs (~200 行)**: load_or_create_cipher() / encrypt() / decrypt() / ensure_dir_0700() / ensure_file_0600() / Aes256Gcm 相关
4. **sqlite_storage.rs**: 保留 SqliteStorage 结构体定义 + Pool 初始化 + new/new_encrypted/clone + `mod storage;` + `pub use storage::*;`

### 第 3 步：验证

- `cargo build` 编译通过
- `cargo test -p echomind-infra` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 通过
- `sqlite_storage.rs` 行数 ≤ 4,400（至少减少 1,200 行）
- 新模块每个 ≤ 500 行

## 关键约束

1. **`pub use` 重导出**: sqlite_storage.rs 顶部 `pub use storage::*;` 确保外部引用路径不变
2. **私有方法可见性**: 拆出的方法需加 `pub(crate)` 或在 `storage/mod.rs` 中 `pub use`
3. **字段访问**: 子模块方法需要访问 `self.pool`（r2d2 Pool）和 `self.vector_cache` 等，考虑传 `&Pool` 参数或将方法保留为 `impl SqliteStorage` 在主文件中（建议前者）
4. **逐步拆分**: 先拆 schema.rs（纯常量，最安全），验证通过 → 再拆 migration.rs → 最后 crypto.rs
5. **每步验证**: 每拆一个模块就 `cargo test -p echomind-infra` 确认无回归

## 铁律提醒

- 严禁 `unwrap()/expect()/panic!()/unreachable!()`
- 改源码后必须运行 `./sync-to-github.sh`
- 文档同步：`docs/api/code-contracts.md` 更新模块路径
- 前端无变更（纯 Rust 后端拆分）

## 验证命令速查

```bash
cargo test -p echomind-infra                    # 基础设施 crate 测试
cargo clippy --all-targets -- -D warnings       # Lint
cargo fmt --check                               # 格式检查
wc -l crates/infra/src/sqlite_storage.rs        # 确认行数减少
./sync-to-github.sh                             # GitHub 同步
```

## 必读文件

- `docs/sessions/CURRENT_STATE.md` — 项目状态快照
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` — 完整深耕计划（21 Sessions / 9 Phases）
- `AGENTS.md` — 架构概览 + 数据流图

## 后续 Sessions 预览

| Session | 任务 |
|---|---|
| S02 | sqlite_storage.rs 拆分 — 文档/会话/消息表 |
| S03 | sqlite_storage.rs 拆分 — 向量/实体/其他表 |
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
