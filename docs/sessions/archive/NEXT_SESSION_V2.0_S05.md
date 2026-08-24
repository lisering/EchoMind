# EchoMind v2.0 深耕 Session S05 提示词

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S04）结果

integration.rs 拆分第 1 部分完成。新建 `tests/integration/` 子目录，包含 `common.rs`（67行，共享辅助函数）+ `chat_tests.rs`（910行，RAG 对话/流式/SSE/取消/abort/设置持久化）+ `import_tests.rs`（1,097行，文档导入/分块/嵌入/多格式/重建索引）。integration.rs 从 6,801→4,779 行（-2,022）。验证：144 passed / Clippy 零警告 / fmt 通过 / build 零警告。sync-to-github 已提交（4 files, 2286 insertions, 2234 deletions）。

## 测试基线

- `cargo test -p echomind-tauri-app --test integration`: 144 passed, 0 failed, 1 ignored
- Clippy: 零警告
- cargo fmt: 通过
- cargo build: 零警告

## 文件结构

```
tests/
  ├── integration.rs           — 主入口（4,779 行，#[path] 引用子模块 + 剩余测试）
  └── integration/
      ├── common.rs            — 共享辅助（67 行：test_config / make_valid_license / DEV_SEED / create_fake_onnx_file / create_fake_tokenizer_files）
      ├── chat_tests.rs         — Chat 测试（910 行：RAG 对话/流式/SSE/取消/abort/设置持久化）
      └── import_tests.rs       — Import 测试（1,097 行：文档导入/分块/嵌入/多格式/重建索引）
```

## 本 Session 任务：S05 — integration.rs 拆分（第 2 部分）

**目标**：crates/tauri-app/tests/integration.rs（当前 4,779 行 → 拆出 ~2,000 行到更多子模块）

### 拆分方案

在 `tests/integration/` 目录继续新建子模块：

```
tests/integration/
  ├── ...（已有 common.rs / chat_tests.rs / import_tests.rs）
  ├── conversation_tests.rs    — 会话/消息/编辑版本/分页（~800 行）
  └── security_tests.rs        — 安全/PII/审计/加密（~800 行）
```

### 执行步骤

1. grep 统计 integration.rs 剩余测试函数，按功能域分组
2. 迁移 conversation 相关测试到 `conversation_tests.rs`（conversation/message/edit/pagination/turn_active_versions）
3. `cargo test -p echomind-tauri-app --test integration` 验证
4. 迁移 security 相关测试到 `security_tests.rs`（security/PII/audit/encrypt/lock/brute_force/clipboard）
5. 再次验证 + clippy + fmt

### 关键约束

- 测试函数签名和逻辑不变，仅改变文件位置
- 共享辅助函数已在 `common.rs`，通过 `use super::common::*;` 访问
- 父模块通过 `#[path = "integration/xxx.rs"] mod xxx;` 引用子模块
- 测试文件顶部加 `#![allow(...)]` 豁免
- 子模块通过 `use super::*;` 继承父模块导入

### 验证

```bash
cargo build                                        # 编译
cargo test -p echomind-tauri-app --test integration # 集成测试
cargo clippy --all-targets -- -D warnings          # Lint
cargo fmt --check                                  # 格式
wc -l crates/tauri-app/tests/integration.rs         # 行数（目标 ≤ 2,500）
./sync-to-github.sh --commit                       # 同步
```

## 后续预览

| Session | 任务 |
|---|---|
| S06 | features.rs 拆分 |
| S07-S08 | CI/CD 自动化 |
| S09-S10 | IPC 命令精简 |
| S14 | Dead Code 清理 |
| S21 | v2.0 发布 |
