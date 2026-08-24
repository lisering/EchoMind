# EchoMind v2.1 P2 行动 R3 衔接提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库，六边形架构 8 crate，v2.0.0 已发布。

## 上一 Session 完成内容

P2 行动 R2 已完成 2 项：

| 项 | 内容 | 关键改动 | 状态 |
|---|---|---|---|
| P2-2 | LLM 无降级策略 | `llm_router.rs`: `remote_failure_count` + `record_remote_failure()` 3 次自动切换 Local + `chat.rs`: `forward_stream_tracked` 包装器 + `chat.js`: `llm_fallback` 事件 toast | ✅ |
| P2-9 | Session 文件整理 | 92 个旧文件归档到 `archive/`，保留 4 活跃 + 5 最近 NEXT_SESSION | ✅ |

## 测试基线（P2 R2 完成后）

- Rust Free: **1373** passed, 0 failed, 31 ignored (+7 fallback 测试)
- Rust Pro: **1669** passed, 0 failed, 48 ignored
- Clippy Free+Pro: 零警告 / cargo fmt: 通过
- tsc: 0 errors / esbuild: 1626 行
- sync-to-github: 6 files committed (269 insertions, 26 deletions)

## P2 行动总览（已完成 5/9 项）

| 项 | 状态 | 说明 |
|---|---|---|
| P2-1 文件监听器自动重启 | ✅ | CancellationToken + 3 次退避重启 |
| P2-2 LLM 无降级策略 | ✅ | 远程失败 3 次 → 自动切换 Local |
| P2-3 向量搜索 O(n) 优化 | 📋 待定 | HNSW 接入默认路径（2 Sessions） |
| P2-4 知识图谱渲染 | 📋 暂缓 | 低频场景 |
| P2-5 LLM 首 token 超时 | ✅ | 60s 首 token + 300s 整体兜底 |
| P2-6 ONNX 嵌入失败降级 | ✅ | 降级到 BM25 关键词搜索 |
| P2-7 重复依赖版本 | 📋 持续优化 | 非阻塞 |
| P2-8 Vanilla JS 架构 | 📋 长期规划 | 非短期 |
| P2-9 Session 文件整理 | ✅ | 92 个文件归档 |

## 下 Session 建议

### 可选 1: P2-3 — 向量搜索 O(n) 优化（2 Sessions）

**问题**: 向量检索全量扫描 O(n)，大知识库（>10k chunks）慢。
**修复**: HNSW 索引接入默认检索路径（Pro 已有 `hnsw_index.rs`）。
**关键文件**: `crates/infra/src/hnsw_index.rs`, `crates/infra/src/sqlite_storage.rs`

### 可选 2: 新功能开发

P2 行动已完成 5/9 项（全部推荐项），剩余 4 项均为暂缓或持续优化。
可以转向新功能开发（S19+ Draft REQ Implement 候选）。

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
| `crates/core/src/llm_router.rs` | LLM 路由器（P2-2 改动在此） |
| `crates/tauri-app/src/commands/chat.rs` | chat 主逻辑（P2-2/P2-5/P2-6 改动在此） |
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
