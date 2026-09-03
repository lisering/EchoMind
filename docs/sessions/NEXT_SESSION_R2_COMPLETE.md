# Phase 1 R3 — 删除过度工程 LLM 功能

生成日期：2026-08-27 ｜ 生成人：AI Agent (Phase 1 R2 完成)

当前版本：v3.1.0 ｜ 分支：`refactor/great-simplification` ｜ 基线 tag：`v3.1.0-final`

核心文档：`docs/architecture/GREAT_SIMPLIFICATION_PLAN.md`（重构总计划，必读）
基线文档：`docs/refactoring/BASELINE.md`（基线指标快照）+ `docs/refactoring/DEPENDENCY_AUDIT.md`（依赖审计）

---

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate。当前 v3.1.0，已进入"大简化重构"阶段。

**重构模式**：所有 session 从"添加功能"转向"删除和简化"。每个 session 的产出不是新功能，而是更少的代码。

---

## 上个 Session 成果 — Phase 1 R2：删除过度工程 Agent 功能 ✅

### 删除的模块（10 个 Rust 模块 + IPC 命令 + 前端 UI）

| 模块 | 文件 | 削减行数 |
|---|---|---|
| AgentEngine (ReAct) | `crates/core/src/agent.rs` + `agent_run.rs` + tests | ~3,000 |
| CoordinatorEngine | `crates/core/src/coordinator.rs` + `coordinator_strategy.rs` + tests | ~2,500 |
| AutoDream | `crates/core/src/auto_dream.rs` + tests | ~1,500 |
| WorkflowEngine | `crates/core/src/workflow.rs` + tests | ~2,000 |
| MemoryStore (三层记忆) | `crates/core/src/memory_store.rs` + tests | ~2,500 |
| SessionStrip | `crates/core/src/session_strip.rs` + tests | ~500 |
| SubAgent | `crates/core/src/sub_agent.rs` + tests | ~500 |
| Hooks / TypedHook | `crates/core/src/hooks.rs` + `typed_hook.rs` + tests | ~500 |
| StepCache | `crates/core/src/step_cache.rs` + tests | ~300 |
| Durable Prompt Admission | IPC 命令 + Storage 方法 | ~500 |
| Session Todo | IPC 命令 + Storage 方法 | ~500 |
| 前端 Agent UI | input-toggles/main.js/dream-panel/memory-panel | ~1,000 |

**预计削减**: ~13,000 行

### R2 清理的级联引用

- `lib.rs`：移除 9 个测试 mod 声明（agent_tests/auto_dream_tests/coordinator_tests/hooks_tests/memory_store_tests/session_strip_tests/step_cache_tests/stream_parse_tests/sub_agent_tests/workflow_tests）
- `chat.rs`：删除 coordinator_enabled/agent_enabled 分支（仅保留标准 RAG 路径）+ Durable Prompt Admission IPC + Session Todo IPC
- `lib.rs` generate_handler!：移除 Session Todo 命令注册
- `commands/mod.rs`：移除 PendingInput/SessionTodo/TodoStatus 导入
- `state.rs`：移除 dream_engine/step_cache/memory_enabled 字段
- `settings.rs`：移除 set_agent_enabled/set_coordinator_mode/set_sub_agent_enabled 命令
- 前端：移除 agent toggle、dream-panel.js、memory-panel.js、state.js/memoryEnabled/subAgentEnabled/agentEnabled 字段
- SRS：REQ-RAG-022/025/029/030/033 标记为 Deprecated

### R2 验证结果

| 验证项 | 结果 |
|---|---|
| cargo check | ✅ 零错误零警告 |
| cargo clippy --all-targets | ✅ 零警告 |
| cargo fmt | ✅ 通过 |
| cargo test | ✅ **1,419 passed, 0 failed**, 33 ignored |
| npx tsc --noEmit | ✅ 0 errors |
| node scripts/build-ui.mjs | ✅ esbuild 1,584 行 |

### R2 后指标变化

| 维度 | R1 前 (基线) | R1 后 | R2 后 | 总削减量 | 总削减率 |
|---|---|---|---|---|---|
| Rust 源码总行数 | ~130,862 | ~118,274 | ~105,000 | -25,862 | -19.8% |
| Rust 测试数 (Free) | 1,714 | 1,568 | 1,419 | -295 | -17.2% |
| esbuild 构建行数 | 1,826 | 1,735 | 1,584 | -242 | -13.3% |

### 基线指标速查

| 维度 | 当前值 (R2 后) | 目标值 | 剩余削减 |
|---|---|---|---|
| Rust 源码总行数 | ~105,000 | ~50,000 | -47% |
| 最大单 crate 行数 | ~44,000 (core) | ≤ 10,000 | 拆分 |
| 前端 JS 总行数 | ~28,000 | ~12,000 | -57% |
| IPC 命令数 | ~200 | ~80 | -60% |
| 依赖总数 | ~1,700 | ~800 | -53% |
| Rust 测试数 | ~1,419 | ~1,500 | +5% |
| Vitest 测试数 | ~1,584 | ~600 | -62% |
| 文档文件数 | ~258 | ~20 | -92% |

---

## 下个 Session 任务 — Phase 1 R3：删除过度工程 LLM 功能

目标：删除所有自研 LLM 推理优化模块，保留基础 mistral.rs 调用能力，削减 ~12,000 行代码。

### 删除清单

| 模块 | 文件 | 估计行数 |
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

### 额外清理

- 从 `crates/infra/src/lib.rs` 移除 `pub mod` 声明
- 从 `crates/tauri-app/src/commands/local_llm.rs` 清理 IPC 命令
- 从 `AppState` 移除 `model_store` 中的 LLM 优化相关字段
- 从 `crates/tauri-app/src/commands/settings.rs` 移除相关设置命令
- 清理 `ui/src/settings-local-llm.js` 中相关 UI
- 清理 `e2e-tests/tests/bridge/tauri-stub.js` 相关 mock
- 清理相关 E2E 测试
- 从 SRS 标记相关 REQ 为 Deprecated

### 删除策略

1. 先删 Rust 模块：删除 `.rs` 文件 + 测试文件，从 `lib.rs` 移除 `pub mod` 声明
2. 再删 IPC：删除命令函数，从 `lib.rs` 的 `generate_handler!` 移除注册
3. 再删前端：从 `settings-local-llm.js` 移除相关 UI
4. 再删 mock + E2E：从 `tauri-stub.js` 移除相关 mock，删除相关 E2E 测试
5. 最后更新文档：SRS REQ 状态 + api 文档

### 每步验证

```bash
# 每删一个模块后
cargo check                    # 确认编译通过
# 删除 IPC 后
cargo check                    # 确认编译通过
# 删除前端后
node scripts/build-ui.mjs      # 确认 esbuild 打包成功
npx tsc --noEmit               # 确认类型检查通过
# 全部完成后
cargo test 2>&1 | grep "test result:" | awk '{s+=$4; f+=$6} END {print "Free passed:"s" failed:"f}'
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

### Phase 1 R3 完成标准

- [ ] 所有自研 LLM 推理优化模块文件已删除
- [ ] `generate_handler!` 中相关 IPC 命令注册已删除
- [ ] `cargo check` 编译通过
- [ ] `cargo test` 全量通过
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过
- [ ] `node scripts/build-ui.mjs` 构建成功
- [ ] `npx tsc --noEmit` 零错误
- [ ] SRS 相关 REQ 标记为 Deprecated
- [ ] `docs/api/` 文档已同步更新
- [ ] NEXT.md 已更新为 Phase 1 R4 衔接提示词
- [ ] `sync-to-github.sh --commit` 已执行

### R2 遗留注意事项（R3 需关注）

- `local_llm.rs` 仍包含 mistral.rs 适配层，R3 需要评估是否完全删除或保留基础调用
- `model_manager.rs` 仍管理 GGUF 模型文件，R3 需要评估是否删除
- `settings-local-llm.js` 仍包含 LLM 相关设置 UI，R3 需要评估清理范围

---

## 重构全景路线图

| Phase | 内容 | Sessions | 状态 |
|---|---|---|---|
| Phase 0 | 冻结与基线 | 1 | ✅ 完成 |
| Phase 1 R1 | 删除学术 RAG 优化 | 1 | ✅ 完成 |
| Phase 1 R2 | 删除过度工程 Agent 功能 | 1 | ✅ 完成 |
| Phase 1 R3 | 删除过度工程 LLM 功能 | 1 | ← 下个 Session |
| Phase 1 R4 | 删除过度工程工具与借鉴 | 1 | 待定 |
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
| `docs/sessions/NEXT_SESSION_R2_COMPLETE.md` | 本文件 (Phase 1 R3 衔接提示词) |
| `docs/sessions/LOG.md` | 会话时间线日志 |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `Cargo.toml` | Workspace 配置 |

---

## Session 结束时必须完成

- [ ] 更新 `docs/sessions/NEXT_SESSION_R2_COMPLETE.md` — 替换为 Phase 1 R4 的提示词
- [ ] 追加 `docs/sessions/LOG.md` — 记录本次 session 的时间线和成果
- [ ] 更新 `docs/sessions/CURRENT_STATE.md` — 更新状态快照（测试数/行数/版本号）
- [ ] 运行 `./sync-to-github.sh --commit` — 同步到 GitHub 发布仓库
- [ ] Git commit + push

---

下一步：Phase 1 R3 — 删除过度工程 LLM 功能，削减 ~12,000 行代码
