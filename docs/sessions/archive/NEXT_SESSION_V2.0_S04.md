# v2.0 S04 提示词 — integration.rs 拆分第 1 部分

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S03）结果

sqlite_storage.rs 第三阶段拆分完成。新建 `storage/{vectors.rs(497行), entities.rs(1050行), misc.rs(778行)}`。sqlite_storage.rs 从 3,883→2,053 行（-1,830）。总计 5,641→2,053（-63.6%）。验证：infra 185 passed / Clippy 零警告 / fmt 通过 / build 零警告。sync-to-github 已提交。

## 测试基线

- `cargo test -p echomind-infra`: 185 passed, 0 failed, 19 ignored
- Clippy 零警告 / fmt 通过 / build 零警告
- sqlite_storage.rs: 2,053 行 ✅ 拆分完成
- storage/ 目录 10 个文件（mod/schema/migration/crypto/documents/conversations/messages/vectors/entities/misc）

## 本 Session 任务：S04 — integration.rs 拆分（第 1 部分）

目标：`crates/tauri-app/tests/integration.rs`（当前 ~6,801 行 → 拆出 ~3,000 行）

### 拆分方案

在 `crates/tauri-app/tests/` 新建 `integration/` 子目录：

```
tests/
  ├── integration.rs           — 主入口（mod 引用子模块 + 共享辅助函数）
  └── integration/
      ├── mod.rs                — 模块声明
      ├── common.rs             — 共享辅助（创建临时存储/初始化测试环境）
      ├── chat_tests.rs         — ~800 行（RAG 对话/流式/SSE/取消）
      └── import_tests.rs       — ~800 行（文档导入/分块/嵌入/多格式）
```

S04 只拆 2 个子模块（chat_tests + import_tests），剩余的 conversation_tests / security_tests 留给 S05。

### 执行步骤

1. `grep -n "#\[test\]\|#\[tokio::test\]" crates/tauri-app/tests/integration.rs` — 统计所有测试函数及行号，按功能域分组
2. 创建 `integration/` 目录 + `mod.rs`
3. 提取共享辅助函数到 `common.rs`（如创建临时存储、初始化测试环境等 `pub(crate)` 函数）
4. 迁移 chat 相关测试到 `chat_tests.rs`（chat/stream/SSE/cancel/abort/phase events）
5. `cargo test -p echomind-tauri-app --test integration` 验证
6. 迁移 import 相关测试到 `import_tests.rs`（import/split/embed/loader/format）
7. 再次验证

### 关键约束

- 测试函数签名和逻辑**不变**，仅改变文件位置
- `integration.rs` 用 `mod integration;` 引用子模块，或 `include!` 宏
- 共享辅助函数提取为 `pub(crate)` 或移到 `common.rs`
- 测试文件顶部加 `#![allow(clippy::unwrap_used, clippy::expect_used)]` 豁免
- 逐步拆分：拆一个模块 → 验证 → 再拆下一个
- `vector_search` 等依赖 `self` 状态的方法不在此次拆分范围（已保留在 sqlite_storage.rs）

### 验证

```bash
cargo build                                        # 编译
cargo test -p echomind-tauri-app --test integration # 集成测试
cargo clippy --all-targets -- -D warnings          # Lint
cargo fmt --check                                  # 格式
wc -l crates/tauri-app/tests/integration.rs         # 行数
./sync-to-github.sh --commit                       # 同步
```

### 铁律

- 严禁 `unwrap()/expect()/panic!()/unreachable!()`（测试代码豁免）
- 改源码后必须运行 `./sync-to-github.sh`
- 前端无变更（纯 Rust 测试拆分）

### 必读文件

- `docs/sessions/CURRENT_STATE.md` — 项目状态快照
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` — 深耕计划
- `AGENTS.md` — 架构概览

### 后续预览

| Session | 任务 |
|---|---|
| S05 | integration.rs 拆分第 2 部分（conversation_tests + security_tests）|
| S06 | features.rs 拆分 |
| S07-S08 | CI/CD 自动化 |
| S09-S10 | IPC 命令精简 |
| S14 | Dead Code 清理 |
| S21 | v2.0 发布 |

---
