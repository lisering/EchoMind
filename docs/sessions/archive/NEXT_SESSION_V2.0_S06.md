# EchoMind v2.0 深耕 Session S06 提示词

---

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v1.22.0 已发布，v2.0 深耕阶段（不新增功能，专注代码质量/测试/CI/文档）。

## 上个 Session（S05）结果

integration.rs 拆分第 2 部分完成。新建 `conversation_tests.rs`（593行：会话导出/Token用量/成本追踪/协调模式/编辑版本持久化）+ `security_tests.rs`（412行：审计门控/安全IPC/加密/锁定/暴力破解/剪贴板/PII/密码强度）。integration.rs 从 4,779→3,808 行（-971）。验证：144 passed / Clippy 零警告 / fmt 通过 / build 零警告。sync-to-github 已提交（3 files, 1013 insertions, 979 deletions）。

## 测试基线

- `cargo test -p echomind-tauri-app --test integration`: 144 passed, 0 failed, 1 ignored
- Clippy: 零警告 / cargo fmt: 通过 / cargo build: 零警告
- integration.rs: 3,808 行（目标后续继续缩减）

## 文件结构

```
crates/tauri-app/tests/
  ├── integration.rs              — 主入口（3,808 行，#[path] 引用 5 子模块 + 剩余测试）
  └── integration/
      ├── common.rs               — 共享辅助（67 行）
      ├── chat_tests.rs           — Chat 测试（910 行）
      ├── import_tests.rs         — Import 测试（1,097 行）
      ├── conversation_tests.rs   — 会话/Token/成本/协调/编辑版本（593 行）
      └── security_tests.rs       — 安全/PII/审计/加密/锁定（412 行）
```

## 本 Session 任务：S06 — features.rs 拆分

**目标**：`crates/tauri-app/src/commands/features.rs`（当前 ~1,559 行 → 按功能域拆分到多个子文件）

### 拆分方案

将 features.rs 按功能域拆分为独立文件，commands/ 目录下新增 features/ 子目录：

```
crates/tauri-app/src/commands/
  ├── features.rs                 — 重构为薄入口（mod 声明 + pub use 重导出）
  └── features/
      ├── mod.rs                  — 模块声明 + pub use 重导出
      ├── dream.rs                — AutoDream 相关命令
      ├── memory.rs               — 持久化记忆相关命令
      ├── web_search.rs           — 网页搜索相关命令
      ├── trace.rs                — Trace/日志相关命令
      ├── code_exec.rs            — 代码执行沙箱相关命令
      ├── custom_model.rs         — 自定义嵌入模型上传（Pro 门控）
      ├── template.rs             — 快捷指令模板管理
      └── wiki.rs                  — Wiki 链接相关命令
```

### 执行步骤

1. `wc -l crates/tauri-app/src/commands/features.rs` 确认行数
2. `grep -n "^pub async fn\|^async fn\|^pub fn\|^fn " features.rs` — 统计所有函数及行号，按功能域分组
3. 创建 `commands/features/` 目录 + `mod.rs`
4. 逐个迁移功能域函数到对应子文件（每个子文件顶部加 `use super::*;`）
5. `features.rs` 重构为薄入口：`#[path]` 引用子模块 + `pub use` 重导出
6. 确保 `commands/mod.rs` 中的 `pub use features::*;` 不变（保持 lib.rs `generate_handler!` 不变）
7. 每拆一个子模块 → `cargo build -p echomind-tauri-app` 验证编译 → 再拆下一个

### 关键约束

- 函数签名和逻辑**不变**，仅改变文件位置
- `use super::*;` 继承父模块（commands/mod.rs）的共享导入
- `pub use` 重导出保持 `lib.rs` `generate_handler!` 不变（与 S65 commands 拆分模式一致）
- Pro 门控函数保持 `#[cfg(feature = "pro")]` 标注
- 逐步拆分：拆一个模块 → 编译验证 → 再拆下一个

### 验证

```bash
cargo build -p echomind-tauri-app                    # 编译
cargo test -p echomind-tauri-app --test integration  # 集成测试（144 passed 基线）
cargo clippy --all-targets -- -D warnings             # Lint
cargo fmt --check                                     # 格式
wc -l crates/tauri-app/src/commands/features.rs      # 行数（目标 ≤ 100，仅 mod + pub use）
./sync-to-github.sh --commit                          # 同步
```

### 铁律

- 严禁 `unwrap()/expect()/panic!()/unreachable!()`（workspace deny lint 强制）
- 改源码后必须运行 `./sync-to-github.sh`
- 测试代码顶部加 `#![allow(clippy::unwrap_used, ...)]` 豁免

### 必读文件

- `docs/sessions/CURRENT_STATE.md` — 项目状态快照
- `docs/architecture/V2.0_DEEP_CULTIVATION_ATOMIC_PLAN.md` — 深耕计划
- `AGENTS.md` — 架构概览（commands/ 14 子模块结构）

### 后续预览

| Session | 任务 |
|---|---|
| S07-S08 | CI/CD 自动化 |
| S09-S10 | IPC 命令精简 |
| S14 | Dead Code 清理 |
| S21 | v2.0 发布 |

---
