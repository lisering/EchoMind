# EchoMind v1.8 Session 提示词

> 复制本文件 --- 之间的全部内容作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，~100K 行 Rust + ~27K 行前端。版本 v1.7.0，1242 passed / 0 failed / 31 ignored。

上一 Session（v1.7）已完成 4 项功能：Shadow Screen 生产集成（`execute_shadow_screen`）/ SessionCoordinator 集成（`chat()` 串行化）/ 向量缓存 LRU 驱逐（`LruVectorCache`）/ 启动并行化（`tokio::join!` + settings 批量读取）。全部 13 TDD 测试通过，clippy 零警告，GitHub 已同步。

## 本 Session 任务：v1.8 规划 + 剩余未集成项评估

v1.7 的 `V1.7_ATOMIC_PLAN.md` 第 5 节列出了 4 项不纳入的远期项目（F5-F8）。本 Session 需评估哪些可在 v1.8 实施，并创建 `docs/architecture/V1.8_ATOMIC_PLAN.md`。

### 任务 1：评估 F5 OM3 Scored Selector（ROI 4.2）

`crates/core/src/` 中 Scored Selector 已实现但当前单 provider 场景未使用。评估多 provider 路由场景下是否需要接入。

### 任务 2：评估 F6 B01/B02 SystemContext+Epoch（ROI 4.1）

需 chat 启动流程重构。评估 v1.8 是否具备实施条件，或推迟到 v2.0。

### 任务 3：评估 F7 DS-01 PromptCache（ROI 4.0）

`crates/infra/src/local_llm.rs` 中 mistral.rs API 是否已暴露 prompt caching 接口。如可用则规划集成。

### 任务 4：评估 F8 B04 CachePolicy（ROI 3.7）

需 LLM 调用链重构。评估是否可与 F7 合并实施。

### 任务 5（如评估后有可实施项）：TDD 实现

按新 `V1.8_ATOMIC_PLAN.md` 执行，TDD 先写红灯测试。

### 任务 6：文档同步 + 发布验证

同步 `docs/` + CHANGELOG + `sync-to-github.sh --commit`。

## 开发规范

- TDD（铁律二）：先写红灯测试再实现
- 零 unwrap/expect/panic（workspace deny lint）
- forbid unsafe
- 文档同步（铁律七）：代码变更同步 docs
- GitHub 同步（铁律十一）：会话结束前 `./sync-to-github.sh`

## 验证命令

```bash
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test
npx tsc --noEmit
./sync-to-github.sh --commit
```

## 当前测试基线

| 指标 | 数值 |
|---|---|
| Rust Free | 1242 passed, 0 failed, 31 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| 版本号 | 1.7.0 |

## v1.7 已完成功能摘要

| 功能 | 关键文件 | TDD 测试 |
|---|---|---|
| Shadow Screen 集成 | `crates/core/src/security.rs`（`execute_shadow_screen`）+ `chat.rs` | TC-SHADOW-INTEGRATE-001~003 |
| SessionCoordinator 集成 | `crates/tauri-app/src/commands/chat.rs`（`chat()` 用 `coord.run()` 包装） | TC-COORD-INTEGRATE-001~003 |
| 向量缓存 LRU | `crates/infra/src/sqlite_storage.rs`（`LruVectorCache` struct） | TC-LRU-001~003 + 2 额外 |
| 启动并行化 | `crates/tauri-app/src/state.rs`（`tokio::join!` + `get_settings_batch`） | TC-BOOT-001~002 |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V1.7_ATOMIC_PLAN.md` | v1.7 规划（第 5 节列出 v1.8 候选） |
| `docs/CHANGELOG_v1.7.md` | v1.7 变更日志 |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/SRS_v1.0.md` | 需求规格 |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
