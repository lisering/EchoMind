# EchoMind v2.1 P2 行动衔接提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate。v2.0.0 已发布，三轮复盘 + P1 修复全部完成。

## 上一 Session 完成内容

R3 复盘的 5 个 P1 项已**全部完成**：

| P1 项 | 内容 | 状态 |
|---|---|---|
| P1-1 | SQLite 磁盘满 — `disk_space.rs` + 写入前检查 + IPC 命令 | ✅ |
| P1-2 | 429 限流 — `classify_llm_error()` 识别 429 + `send_stream_request_with_retry()` 指数退避(1s/2s/4s) + toast.js 前缀映射 | ✅ |
| P1-3 | ADR-004~008 归档 — 5 个独立 ADR 文件创建 | ✅ |
| P1-4 | mistral.rs bus factor=1 — ADR-014 监控文档（三层缓解策略） | ✅ |
| P1-5 | fastembed 默认模型 — 新增 `BgeSmallEnV1_5` 预设 + UI 选择器 + 默认切换 | ✅ |

## 测试基线（v2.0.0 + P1 修复后）

- Rust infra: 212 passed (含 7 新 429 退避测试)
- Rust Free: 1322 passed / Rust Pro: 1618 passed
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- Vitest: 938 passed / E2E: 1355 passed
- ADR 数: 14（新增 ADR-014 + ADR-004~008 独立归档）

## 本 Session 任务：v2.1 P2 行动（前 3 项）

R3 复盘识别了 9 个 P2 项，建议按以下顺序实施前 3 项（预计 2 Sessions）：

### 1. P2-1: 文件监听器自动重启（0.5 Session）

**问题**：`file_watcher.rs` 在监听器崩溃后无自动恢复机制。
**修复**：`tracing` 替代 `eprintln!` + supervisor 自动重启（3 次退避）。
**关键文件**：`crates/infra/src/file_watcher.rs`

### 2. P2-5: LLM 首 token 超时（0.5 Session）

**问题**：`REQUEST_TIMEOUT_SECS=300`（5 分钟）过长，用户无反馈。
**修复**：增加 60s 首 token 超时检测 + 保持 300s 整体超时兜底。
**关键文件**：`crates/infra/src/openai_provider.rs`（`send_stream_request` 方法）

### 3. P2-6: ONNX 嵌入失败降级（0.5 Session）

**问题**：ONNX 嵌入引擎加载失败时无降级路径。
**修复**：嵌入失败 → 降级到纯关键词搜索（BM25）+ 前端提示。
**关键文件**：`crates/tauri-app/src/commands/import.rs`（嵌入调用路径）

## 约束

1. **铁律五**: 生产代码严禁 `unwrap()/expect()/panic!()`
2. **铁律七**: 代码变更同步更新 `docs/api/` + `SOFTWARE_ASSET.md`
3. **铁律十一**: 修改源代码后运行 `./sync-to-github.sh`
4. **TDD**: 先写测试再实现
5. **依赖方向**: `tauri-app → core → models`，严禁反向

## 关键文件索引

| 文件 | 用途 |
|---|---|
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（必读） |
| `docs/sessions/archive/retrospective_v2_round2_R3.md` | R3 复盘报告（P2 清单全文） |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `.catpaw/rules/system-constraints.md` | 铁律规则（21 条） |
| `docs/api/ipc-commands.md` | IPC 命令签名 |

## 验证命令

```bash
cargo test -p echomind-infra          # infra 测试
cargo clippy --all-targets -- -D warnings  # 全量 clippy
cargo fmt --check                     # 格式检查
npx tsc --noEmit                      # 前端类型检查
node scripts/build-ui.mjs             # 前端构建
./sync-to-github.sh --commit          # GitHub 同步
```
