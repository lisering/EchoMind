# EchoMind v1.7 Session 提示词（S4-S8）

> 复制本文件全部内容（--- 之间）作为新会话的第一条消息。

---

## 项目

EchoMind — Rust + Tauri v2 本地 RAG 知识库（律师/研究者高隐私场景）。六边形架构 8 crate，~100K 行 Rust + ~27K 行前端。版本 v1.6.0，1228 passed / 0 failed / 28 ignored。

上一 Session 已完成 3 项借鉴功能集成补全（heal_dangling_calls / VerbatimTail / schedule_background_compaction），并创建 `docs/architecture/V1.7_ATOMIC_PLAN.md`。本 Session 执行 v1.7 规划中的 4 项功能。

## 本 Session 任务：v1.7 功能实现（4 项）

按 `docs/architecture/V1.7_ATOMIC_PLAN.md` 第 3 节执行，TDD 先写红灯测试。

### S4（P0）：Q06/S71 Shadow Screen 集成（ROI 7.3）

`security.rs` 中 `run_shadow_screen()` / `llm_classify()` 已定义但从未在生产代码调用。`SecurityPosture` 支持 Strict 模式但无 Shadow 筛查。

**做法**：在 `chat_inner` 中，当 `security_posture == Strict` 时对用户 query 执行 `llm_classify()` 获取 Shadow 裁决，与权威决策对比并记录统计。LLM 不可用时降级为 Unscreened（不影响对话流）。超时 5s 降级。

TDD：TC-SHADOW-INTEGRATE-001~003（Strict 触发 / LLM 降级 / 安全隔离）。

### S5（P1）：B06 SessionCoordinator 集成（ROI 5.9）

`state.rs` 中 `SessionCoordinator` 已初始化但 `.run()` 从未被 `chat_inner` 调用。

**做法**：用 `session_coordinator.run(conv_id, || async { ... })` 包装 `chat_inner` 中的对话执行，实现会话级串行化 + interrupt/wake。

TDD：TC-COORD-INTEGRATE-001~003（串行执行 / interrupt 清理 / wake 合并）。

### S6（P1）：向量缓存 LRU 驱逐（ROI 5.7）

`SqliteStorage::vector_cache` 全量加载内存，无 LRU。大规模知识库内存高。

**做法**：改为 `Arc<RwLock<LruCache<String, Vec<f32>>>>`，添加 `max_vectors` 配置项（默认 5000），超限驱逐最久未访问。

TDD：TC-LRU-001~003（驱逐 / 访问顺序更新 / 写失效）。

### S7（P1）：启动并行化（ROI 5.3）

`AppState::new()` 串行初始化（storage → embedder → settings），冷启动 ~3s。

**做法**：`tokio::join!` 并行化独立初始化路径，添加启动耗时 tracing span。

TDD：TC-BOOT-001~002（并行结果一致 / 单项失败不影响其他）。

### S8：文档同步 + 发布验证

同步 `docs/api/` + `docs/SRS_v1.0.md` + CHANGELOG，验证全部命令通过。

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
| Rust Free | 1228 passed, 0 failed, 28 ignored |
| Clippy | 零警告 |
| cargo fmt | 通过 |
| TypeScript | 0 errors |
| 版本号 | 1.6.0 |

## 关键文件

| 文件 | 用途 |
|---|---|
| `docs/architecture/V1.7_ATOMIC_PLAN.md` | v1.7 规划详情（ROI 矩阵 + 实现规格） |
| `crates/tauri-app/src/commands/chat.rs` | chat_inner 主流程（集成接入点） |
| `crates/tauri-app/src/security.rs` | 安全管理（Shadow Screen / llm_classify） |
| `crates/tauri-app/src/state.rs` | AppState（SessionCoordinator 初始化点） |
| `crates/infra/src/storage.rs` | SqliteStorage（vector_cache 所在） |
| `.catpaw/rules/system-constraints.md` | 铁律 21 条 |
