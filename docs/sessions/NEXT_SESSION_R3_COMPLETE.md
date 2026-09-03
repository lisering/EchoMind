# Phase 1 R4 — 删除过度工程工具与借鉴

生成日期：2026-08-27 ｜ 生成人：AI Agent (Phase 1 R3 完成)

当前版本：v3.1.0 ｜ 分支：`refactor/great-simplification` ｜ 基线 tag：`v3.1.0-final`

核心文档：`docs/architecture/GREAT_SIMPLIFICATION_PLAN.md`（重构总计划，必读）
基线文档：`docs/refactoring/BASELINE.md`（基线指标快照）+ `docs/refactoring/DEPENDENCY_AUDIT.md`（依赖审计）

---

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate。当前 v3.1.0，已进入"大简化重构"阶段。

**重构模式**：所有 session 从"添加功能"转向"删除和简化"。每个 session 的产出不是新功能，而是更少的代码。

---

## 上个 Session 成果 — Phase 1 R3：删除过度工程 LLM 功能 ✅

### 删除的模块（10 个 Rust 模块 + IPC + 前端引用）

| 模块 | 文件 | 削减行数 |
|---|---|---|
| GEMV 内核 | `crates/infra/src/gemv_kernel.rs` + tests | ~1,500 |
| 量化块结构 | `crates/infra/src/quant_blocks.rs` + tests | ~500 |
| GGUF 解析器 | `crates/infra/src/gguf_reader.rs` + tests | ~1,000 |
| 权重重排 | `crates/infra/src/weight_repack.rs` + tests | ~500 |
| 层预取 | `crates/infra/src/layer_prefetch.rs` + tests | ~500 |
| RAM 预算 | `crates/infra/src/memory_budget.rs` + tests | ~500 |
| KV Cache | `crates/infra/src/kv_cache.rs` + tests | ~1,000 |
| Prompt Prefix 缓存 | `crates/infra/src/prompt_cache.rs` + tests | ~1,000 |
| Robust Downloader | `crates/infra/src/robust_downloader.rs` | ~1,500 |
| Mistral.rs 适配层 | `crates/infra/src/local_llm.rs` + tests | ~3,000 |
| ModelManager | `crates/infra/src/model_manager.rs` + tests | ~1,000 |

**预计削减**: ~12,000 行

### R3 清理内容

1. **删除 LlmMode::Local 变体**：
   - `crates/models/src/lib.rs` — LlmMode enum 仅保留 Remote
   - `crates/core/src/llm_router.rs` — 移除 Local 路由逻辑和自动切换
   - `crates/core/src/llm_router_tests.rs` — 移除 7 个 Local/回退相关测试
   - `crates/tauri-app/src/stores/config_store.rs` — 仅处理 Remote 模式
   - `crates/tauri-app/src/commands/features/trace.rs` — 移除 Local 匹配

2. **删除本地 LLM 推理模块**（10 个文件 + 测试）

3. **清理 integration.rs**：
   - 删除 `local_llm_chain.rs` 模块声明
   - 移除所有本地 LLM 相关测试（KV cache/kernel mode/paged_attn/sampling/model_manager）
   - 清理 `get_llm_mode`/`set_llm_mode` 相关测试引用

4. **更新 SRS 文档**：
   - REQ-LLM-003 标记为 Deprecated
   - REQ-LLM-004/005/006/007/008/009 更新状态描述

### R3 验证结果

| 验证项 | 结果 |
|---|---|
| cargo check | ✅ 零错误零警告 |
| cargo test | ✅ **1,326 passed, 0 failed** |
| cargo clippy --all-targets | ✅ 零警告 |
| cargo fmt | ✅ 通过 |

### 累计指标变化

| 维度 | 基线 (Phase 0) | R1 后 | R2 后 | R3 后 | 总削减量 | 总削减率 |
|---|---|---|---|---|---|---|
| Rust 源码总行数 | ~130,862 | ~118,274 | ~105,000 | ~93,000 | -37,862 | -28.9% |
| Rust 测试数 (Free) | 1,714 | 1,568 | 1,419 | 1,326 | -388 | -22.6% |
| esbuild 构建行数 | 1,826 | 1,735 | 1,584 | 1,584 | -242 | -13.3% |

---

## 下个 Session 任务 — Phase 1 R4：删除过度工程工具与借鉴

目标：删除过度工程的工具模块和零用户使用的借鉴功能，预计削减 ~5,000 行代码。

### 删除清单

| 模块 | 文件 | 估计行数 |
|---|---|---|
| KeyedQueue | `crates/core/src/concurrency.rs` + tests | ~300 |
| Sweeper (后台压缩) | `crates/compact/src/lib.rs` 部分功能 | ~200 |
| EventStream | `crates/core/src/event_stream.rs` + tests | ~500 |
| PipelineCheckpoint | `crates/core/src/pipeline_checkpoint.rs` + tests | ~500 |
| MMR Diversifier | `crates/core/src/mmr_diversifier.rs` + tests | ~500 |
| Quality Gate | `crates/core/src/retrieval_quality_gate.rs` + tests | ~500 |
| Scored Selector | `crates/core/src/scored_selector.rs` + tests | ~500 |
| CoordinatorStrategy | `crates/core/src/coordinator_strategy.rs` + tests | ~500 |
| AuditPair | `crates/core/src/audit_pair.rs` + tests | ~500 |
| SessionCoordinator | `crates/core/src/session_coordinator.rs` + tests | ~500 |
| LlmProtocol | `crates/core/src/llm_protocol.rs` + tests | ~300 |
| Permission | `crates/core/src/permission.rs` + tests | ~300 |
| Skill | `crates/core/src/skill.rs` + tests | ~300 |

**预计削减**: ~5,000 行

### 额外清理

- 从 `crates/core/src/lib.rs` 移除 `pub mod` 声明
- 从 `crates/core/src/chat.rs` 移除相关集成调用
- 清理 `crates/tauri-app/src/commands/` 中相关 IPC
- 清理 `e2e-tests` 中相关测试
- 清理 `crates/tauri-app/src/state.rs` 中相关字段
- 更新 SRS 相关 REQ 为 Deprecated

### 删除策略

1. 先删 Rust 模块：删除 `.rs` 文件 + 测试文件
2. 再删 IPC：从相关命令文件中移除
3. 再删 mock/E2E：从 `tauri-stub.js` 移除相关 mock
4. 最后更新文档：SRS REQ 状态 + api 文档

### Phase 1 R4 完成标准

- [ ] 所有过度工程工具与借鉴模块文件已删除
- [ ] `generate_handler!` 中相关 IPC 命令注册已删除
- [ ] `cargo check` 编译通过
- [ ] `cargo test` 全量通过
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过
- [ ] `node scripts/build-ui.mjs` 构建成功
- [ ] `npx tsc --noEmit` 零错误
- [ ] SRS 相关 REQ 标记为 Deprecated
- [ ] `docs/api/` 文档已同步更新
- [ ] NEXT.md 已更新为 Phase 1 R5 衔接提示词
- [ ] `sync-to-github.sh --commit` 已执行

---

## 重构全景路线图

| Phase | 内容 | Sessions | 状态 |
|---|---|---|---|
| Phase 0 | 冻结与基线 | 1 | ✅ 完成 |
| Phase 1 R1 | 删除学术 RAG 优化 | 1 | ✅ 完成 |
| Phase 1 R2 | 删除过度工程 Agent 功能 | 1 | ✅ 完成 |
| Phase 1 R3 | 删除过度工程 LLM 功能 | 1 | ✅ 完成 |
| **Phase 1 R4** | **删除过度工程工具与借鉴** | **1** | **← 下个 Session** |
| Phase 1 R5 | 删除前端过度工程模块 | 1 | 待定 |
| Phase 2 | Core crate 拆分 | 3 | 待定 |
| Phase 3 | IPC 命令精简 | 2 | 待定 |
| Phase 4 | 前端简化 | 3 | 待定 |
| Phase 5 | 依赖削减 | 2 | 待定 |
| Phase 6 | 测试重整 | 2 | 待定 |
| Phase 7 | 文档重置 | 1 | 待定 |
| Phase 8 | 版本校准与发布 | 2 | 待定 |

---

## 关键约束 (铁律，重构期间继续遵守)

- 代理仅限开发调研，软件本身禁止代理
- TDD 驱动 — 删除功能时同步删除测试
- 生产级代码质量 — 零 `unwrap()/expect()/panic!()/unreachable!()/todo!()/unimplemented!()`
- 代码变更同步更新文档
- 修改源代码后运行 `./sync-to-github.sh`
- Session 结束更新 NEXT.md + LOG.md + CURRENT_STATE.md
- 前端构建：`node scripts/build-ui.mjs`
- TypeScript：`npx tsc --noEmit` 类型检查
- 依赖方向：`tauri-app → core → models`，严禁反向
- Edition 2024：原生 `async fn` in trait
- 先写规格再写代码
- Git 历史保留 — 删除的代码通过 Git 历史可恢复
- 不修改数据库 schema

---

## 验证命令

```bash
# Rust 测试
cargo test 2>&1 | grep "test result:" | awk '{s+=$4; f+=$6} END {print "Free passed:"s" failed:"f}'
cargo test --features pro 2>&1 | grep "test result:" | awk '{s+=$4; f+=$6} END {print "Pro passed:"s" failed:"f}'

# Lint
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features pro -- -D warnings
cargo fmt --check

# 前端
npx tsc --noEmit
node scripts/build-ui.mjs
cd e2e-tests && npm run test:unit

# 同步
./sync-to-github.sh --commit
```

---

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/architecture/GREAT_SIMPLIFICATION_PLAN.md` | 重构总计划 (必读) |
| `docs/refactoring/BASELINE.md` | 基线指标快照 |
| `docs/refactoring/DEPENDENCY_AUDIT.md` | 依赖审计报告 |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `docs/sessions/NEXT_SESSION_R3_COMPLETE.md` | 本文件 (Phase 1 R4 衔接提示词) |
| `docs/sessions/LOG.md` | 会话时间线日志 |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `Cargo.toml` | Workspace 配置 |

---

## Session 结束时必须完成

- [ ] 更新 `docs/sessions/NEXT.md` — 替换为 Phase 1 R5 的提示词
- [ ] 追加 `docs/sessions/LOG.md` — 记录本次 session 的时间线和成果
- [ ] 更新 `docs/sessions/CURRENT_STATE.md` — 更新状态快照（测试数/行数/版本号）
- [ ] 运行 `./sync-to-github.sh --commit` — 同步到 GitHub 发布仓库
- [ ] Git commit + push

---

下一步：Phase 1 R4 — 删除过度工程工具与借鉴，削减 ~5,000 行代码
