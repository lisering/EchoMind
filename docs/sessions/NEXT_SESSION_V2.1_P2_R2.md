# EchoMind v2.1 P2 行动 R2 衔接提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate，v2.0.0 已发布。

## 上一 Session 完成内容

P2 行动前 3 项已**全部完成**：

| 项 | 内容 | 关键改动 | 状态 |
|---|---|---|---|
| P2-1 | 文件监听器自动重启 | `file_watcher.rs`: `CancellationToken` + 3 次退避重启 + `tracing` 日志 | ✅ |
| P2-5 | LLM 首 token 超时 | `chat.rs`: `FIRST_TOKEN_TIMEOUT_SECS=60` + `forward_stream` 两阶段 `select!` | ✅ |
| P2-6 | ONNX 嵌入失败降级 | `chat.rs`: `embedding_degraded` 标志 → 关键词搜索（BM25）+ 降级提示 | ✅ |

## 测试基线（P2 前 3 项完成后）

- Rust Free: **1366** passed, 0 failed, 31 ignored
- Rust Pro: **1662** passed, 0 failed, 48 ignored
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- sync-to-github: 3 files committed (452 insertions, 97 deletions)

## 本 Session 任务：P2 行动 R2（后 6 项，选做 2-3 项）

R3 复盘 P2 清单剩余 6 项，按优先级建议选做 2-3 项：

### 推荐 1: P2-2 — LLM 无降级策略（1 Session）

**问题**: 远程 LLM 连续失败后无自动切换本地 LLM。
**修复**: `chat_inner` 中 LLM 连续失败 3 次 → 自动切换到 Local 模式（如已配置 GGUF 模型）+ 前端 toast 提示降级。
**关键文件**: `crates/tauri-app/src/commands/chat.rs`（`chat_inner` LLM 调用路径）, `crates/core/src/llm_router.rs`（`LlmRouter` 已有 fallback 机制可复用）

### 推荐 2: P2-9 — Session 文件整理（1 Session）

**问题**: `docs/sessions/` 下 98+ 个文件，开发者上手困难。
**修复**: 整理为精简时间线 + 归档旧 `NEXT_SESSION_*.md` 到 `archive/`，保留最近 5 个。
**关键文件**: `docs/sessions/` 目录

### 可选: P2-3 — 向量搜索 O(n) 优化（2 Sessions，较大）

**问题**: 向量检索全量扫描 O(n)，大知识库（>10k chunks）慢。
**修复**: HNSW 索引接入默认检索路径（Pro 已有 `hnsw_index.rs`）。
**关键文件**: `crates/infra/src/hnsw_index.rs`, `crates/infra/src/sqlite_storage.rs`

### 暂缓项

| 项 | 原因 |
|---|---|
| P2-4 知识图谱渲染 | 低频场景，暂不紧急 |
| P2-7 重复依赖版本 | 持续优化，非阻塞 |
| P2-8 Vanilla JS 架构 | 长期规划，非短期 |

## 约束

1. **铁律五**: 生产代码严禁 `unwrap()/expect()/panic!()`
2. **铁律七**: 代码变更同步更新 `docs/api/` + `SOFTWARE_ASSET.md`
3. **铁律十一**: 修改源代码后运行 `./sync-to-github.sh --commit`
4. **TDD**: 先写测试再实现
5. **依赖方向**: `tauri-app → core → models`，严禁反向

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/sessions/archive/retrospective_v2_round2_R3.md` | R3 复盘报告（P2 清单全文） |
| `crates/tauri-app/src/commands/chat.rs` | chat 主逻辑（P2-5/P2-6 改动在此） |
| `crates/core/src/llm_router.rs` | LLM 路由器（P2-2 复用） |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条） |

## 验证命令

```bash
cargo test 2>&1 | grep -E "^test result:" | awk '{s+=$4;f+=$6;i+=$8} END{print "passed:"s" failed:"f" ignored:"i}'
cargo clippy --all-targets -- -D warnings
cargo fmt --check
npx tsc --noEmit
node scripts/build-ui.mjs
./sync-to-github.sh --commit
```
