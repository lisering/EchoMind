# Phase 1 R2 — 删除过度工程 Agent 功能

生成日期：2026-08-27 ｜ 生成人：AI Agent (Phase 1 R1 完成)

当前版本：v3.1.0 ｜ 分支：`refactor/great-simplification` ｜ 基线 tag：`v3.1.0-final`

核心文档：`docs/architecture/GREAT_SIMPLIFICATION_PLAN.md`（重构总计划，必读）
基线文档：`docs/refactoring/BASELINE.md`（基线指标快照）+ `docs/refactoring/DEPENDENCY_AUDIT.md`（依赖审计）

---

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate。当前 v3.1.0，已进入"大简化重构"阶段。

**重构模式**：所有 session 从"添加功能"转向"删除和简化"。每个 session 的产出不是新功能，而是更少的代码。

---

## 上个 Session 成果 — Phase 1 R1：删除学术 RAG 优化 ✅

### 删除的模块（10 个 Rust 模块 + 2 个 Infra 模块）

| 模块 | 文件 | 削减行数 |
|---|---|---|
| SpeculativeRagEngine | `crates/core/src/speculative_rag.rs` + tests | ~1,500 |
| SummaryTreeBuilder | `crates/core/src/summary_tree.rs` + tests | ~1,000 |
| PropositionSplitter | `crates/core/src/proposition_splitter.rs` + tests | ~800 |
| ProgressiveInjector | `crates/core/src/progressive_injector.rs` + tests | ~1,000 |
| RetrievalMemoryEngine | `crates/core/src/retrieval_memory.rs` + tests | ~1,200 |
| RuleBasedCompressor | `crates/core/src/prompt_compressor.rs` + tests | ~1,000 |
| ResponseCache (L0/L1/L3) | `crates/core/src/cache.rs` + tests | ~1,000 |
| DomainClassifier | `crates/core/src/domain.rs` + tests | ~1,000 |
| LateChunking | `crates/core/src/late_chunking.rs` + tests | ~500 |
| EmbedComparison | `crates/core/src/embed_comparison.rs` + tests | ~500 |
| SqliteCache | `crates/infra/src/sqlite_cache.rs` | ~1,000 |
| OnnxCompressor (Pro) | `crates/infra/src/onnx_compressor.rs` | ~500 |
| ColbertEmbedder (Pro) | `crates/infra/src/colbert_embedder.rs` + tests | ~2,000 |

**git diff 统计**：49 files changed, 91 insertions(+), **12,680 deletions(-)**

### 清理的级联引用

- `chat.rs`：删除 L3 缓存命中/写入分支 + Speculative RAG 分支 + PromptCompressor 集成 + 渐进式注入 + 质量门控 + 检索记忆记录 + StepCache if false 块 + 查询缓存写入空块 + `let false` 绑定（共 ~300 行死代码）
- `document.rs`：删除缓存失效残留（2 处）+ `EmbeddingDomainClassifier` → 简化为 "general" 默认
- `import.rs`：删除缓存失效残留（2 处）+ `embed_propositions` 函数及调用
- `workspace.rs`：删除缓存失效残留
- `settings.rs`：`state.speculative_enabled` → 从 settings 表读取
- `state.rs`：简化 `parse_settings` 测试函数（12→7 元组）
- `infra/entities.rs` + `infra/misc.rs`：删除未使用导入
- 测试文件：`chat_tests.rs` + `full_pipeline.rs` 跳过 compression_ratio 测试；`import_bench.rs` 删除 proposition 阶段

### R1 验证结果

| 验证项 | 结果 |
|---|---|
| `cargo check` | ✅ 零错误零警告 |
| `cargo clippy --all-targets` | ✅ 零警告 |
| `cargo fmt` | ✅ 通过 |
| `cargo test` | ✅ **1,568 passed, 0 failed, 33 ignored** |
| `npx tsc --noEmit` | ✅ 0 errors |
| `node scripts/build-ui.mjs` | ✅ esbuild 1,735 行 |
| `sync-to-github.sh --commit` | ✅ 已提交 |

### R1 后指标变化

| 维度 | R1 前 (基线) | R1 后 | 削减量 | 削减率 |
|---|---|---|---|---|
| Rust 源码总行数 | ~130,862 | **~118,274** | -12,588 | -9.6% |
| Rust 测试数 (Free) | 1,714 | **1,568** | -146 | -8.5% |
| esbuild 构建行数 | 1,826 | **1,735** | -91 | -5.0% |

---

## 基线指标速查

| 维度 | 当前值 (R1 后) | 目标值 | 剩余削减 |
|---|---|---|---|
| Rust 源码总行数 | ~118,274 | ~50,000 | -57% |
| 最大单 crate 行数 | ~44,000 (core) | ≤ 10,000 | 拆分 |
| 前端 JS 总行数 | ~30,000 | ~12,000 | -60% |
| IPC 命令数 | ~228 | ~80 | -65% |
| 依赖总数 | ~1,765 | ~800 | -55% |
| Rust 测试数 | ~1,568 | ~1,500 | -5% |
| Vitest 测试数 | ~1,826 | ~600 | -67% |
| 文档文件数 | ~258 | ~20 | -92% |

---

## 下个 Session 任务 — Phase 1 R2：删除过度工程 Agent 功能

**目标**：删除所有过度工程的 Agent/Coordinator/记忆/工作流模块及其测试，削减 ~13,000 行代码。

### 删除清单

| 模块 | 文件 | 估计行数 |
|---|---|---|
| AgentEngine (ReAct) | `crates/core/src/agent.rs` + `agent_run.rs` + tests | ~3,000 |
| CoordinatorEngine | `crates/core/src/coordinator.rs` + `coordinator_strategy.rs` + tests | ~2,500 |
| AutoDream | `crates/core/src/auto_dream.rs` + tests | ~1,500 |
| WorkflowEngine | `crates/core/src/workflow.rs` + tests | ~2,000 |
| MemoryStore (三层记忆) | `crates/core/src/memory_store.rs` + tests | ~2,500 |
| SessionStrip | `crates/core/src/session_strip.rs` + tests | ~500 |
| SubAgent | `crates/core/src/sub_agent.rs` + tests | ~500 |
| Hooks / TypedHook | `crates/core/src/hooks.rs` + `typed_hook.rs` + tests | ~500 |

**预计削减**: ~13,000 行

### 额外清理

- 从 `crates/core/src/lib.rs` 移除 `pub mod` 声明
- 从 `crates/tauri-app/src/commands/chat.rs` 移除 `coordinator_enabled` / `agent_enabled` 分支逻辑（chat_inner 中的 `if coordinator_enabled` / `else if agent_enabled` 整块删除，仅保留标准 RAG 路径）
- 从 `crates/tauri-app/src/commands/agent.rs` 删除 IPC 命令
- 从 `crates/tauri-app/src/lib.rs` 的 `generate_handler!` 移除命令注册
- 从 `crates/tauri-app/src/commands/mod.rs` 的 `pub use` 移除重导出
- 从 AppState 移除 `sub_agent_enabled` / `dream_engine` 等字段
- 从 `crates/tauri-app/src/commands/settings.rs` 移除 `set_agent_enabled` / `set_coordinator_mode` / `set_sub_agent_enabled` 等 set 命令
- 清理 `ui/src/input-toggles.js`（移除 agent toggle）
- 清理 `ui/src/main.js` 相关导入和初始化
- 清理 `ui/locales/{en,zh-CN}.json` 相关 i18n 键
- 清理 `e2e-tests/tests/bridge/tauri-stub.js` 相关 mock
- 清理相关 E2E 测试
- 从 SRS 标记 REQ-RAG-022/025/029/030/033 为 Deprecated

### 删除策略

1. **先删 Rust 模块**：删除 `.rs` 文件 + 测试文件，从 `lib.rs` 移除 `pub mod` 声明
2. **再删 chat.rs 中的分支**：移除 `if coordinator_enabled` / `else if agent_enabled` 整块，仅保留标准 RAG 路径
3. **再删 IPC**：从 `commands/agent.rs` 删除命令函数，从 `lib.rs` 的 `generate_handler!` 移除注册
4. **再删前端**：从 `input-toggles.js` 移除 agent toggle，从 `main.js` 移除相关导入
5. **再删 mock + E2E**：从 `tauri-stub.js` 移除相关 mock，删除相关 E2E 测试
6. **最后更新文档**：SRS REQ 状态 + api 文档

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

### Phase 1 R2 完成标准

- [ ] 所有 Agent/Coordinator/AutoDream/Workflow/Memory 模块文件已删除
- [ ] `chat.rs` 中 `coordinator_enabled` / `agent_enabled` 分支已删除，仅保留标准 RAG 路径
- [ ] `cargo check` 编译通过
- [ ] `cargo test` 全量通过
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过
- [ ] `node scripts/build-ui.mjs` 构建成功
- [ ] `npx tsc --noEmit` 零错误
- [ ] SRS REQ-RAG-022/025/029/030/033 标记为 Deprecated
- [ ] `docs/api/` 文档已同步更新
- [ ] NEXT.md 已更新为 Phase 1 R3 衔接提示词
- [ ] `sync-to-github.sh --commit` 已执行

### R1 遗留注意事项（R2 需关注）

1. **`step_cache` 仍在 AppState 中**：R1 保留了 `step_cache` 字段和 `StepCache` 类型（在 `crates/core/src/step_cache.rs`），因为 CoordinatorEngine 和 AgentEngine 使用它。R2 删除这两个引擎后，`step_cache` 也应一并删除。
2. **`chat.rs` 的 `coordinator_enabled` / `agent_enabled` 分支**：这两个分支各约 60 行，包含完整的 coordinator/agent 执行流程。删除后 `chat_inner` 将大幅简化为仅标准 RAG 路径。
3. **`settings.rs` 中 set 命令**：`set_agent_enabled_inner` / `set_coordinator_mode_inner` / `set_sub_agent_enabled_inner` 仍在 `settings.rs` 中注册。R2 需要删除这些函数和 `lib.rs` 的注册。
4. **前端 `input-toggles.js`**：当前有 2 个 toggle（hybrid + agent）。删除 agent 后仅剩 hybrid 1 个 toggle。
5. **`tauri-stub.js`**：有 agent/coordinator/sub_agent/dream/workflow 相关的 mock 需要清理。
6. **`EmbeddingDomainClassifier` 已在 R1 简化**：`classify_and_update_domain` 现在只设为 "general"，但 `reclassify_document` IPC 命令仍然存在（返回 "general"）。R2 可考虑删除此命令。
7. **`bench_tests.rs` 已在 R1 删除**：`crates/core/src/benchmark_tests.rs` 已删除。
8. **`perf_optim_tests.rs` 已在 R1 删除**。
9. **R1 保留了 L1 embedding cache**：`put_embedding_cache` / `get_embedding_cache` / `put_embedding_cache_batch` 仍在 Storage trait 和 SqliteStorage 中。这是嵌入向量去重缓存（不同于已删除的语义/查询/响应缓存），R2 可保留。

---

## 重构全景路线图

| Phase | 内容 | Sessions | 状态 |
|---|---|---|---|
| Phase 0 | 冻结与基线 | 1 | ✅ 完成 |
| Phase 1 R1 | 删除学术 RAG 优化 | 1 | ✅ 完成 |
| **Phase 1 R2** | **删除过度工程 Agent 功能** | **1** | **← 下个 Session** |
| Phase 1 R3 | 删除过度工程 LLM 功能 | 1 | 待定 |
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

- **代理仅限开发调研，软件本身禁止代理** — `reqwest::Client::builder()` 必须加 `.no_proxy()`
- **TDD 驱动** — 删除功能时同步删除测试，修改功能时先改测试
- **生产级代码质量** — 零 `unwrap()/expect()/panic!()/unreachable!()/todo!()/unimplemented!()`
- **代码变更同步更新文档** — `docs/api/` + `docs/SOFTWARE_ASSET.md`
- **修改源代码后运行 `./sync-to-github.sh`**
- **Session 结束更新 NEXT.md + LOG.md + CURRENT_STATE.md**
- **前端构建**：`node scripts/build-ui.mjs`（esbuild 打包 IIFE，不要手改 `ui/index.html`）
- **TypeScript**：`npx tsc --noEmit` 类型检查
- **依赖方向**：`tauri-app → core → models`，严禁反向
- **Edition 2024**：原生 `async fn` in trait，无 `#[async_trait]` 宏
- **先写规格再写代码** — 删除功能时同步更新 SRS REQ 状态为 `Deprecated`
- **Git 历史保留** — 删除的代码通过 Git 历史可恢复，无需注释保留
- **不修改数据库 schema** — 删除功能只删除相关查询代码，不删除数据库表

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
| `docs/sessions/NEXT_SESSION_R1_COMPLETE.md` | 本文件 (Phase 1 R2 衔接提示词) |
| `docs/sessions/LOG.md` | 会话时间线日志 |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `Cargo.toml` | Workspace 配置 |

---

## Session 结束时必须完成

- [ ] 更新 `docs/sessions/NEXT_SESSION_R1_COMPLETE.md` — 替换为 Phase 1 R3 的提示词
- [ ] 追加 `docs/sessions/LOG.md` — 记录本次 session 的时间线和成果
- [ ] 更新 `docs/sessions/CURRENT_STATE.md` — 更新状态快照（测试数/行数/版本号）
- [ ] 运行 `./sync-to-github.sh --commit` — 同步到 GitHub 发布仓库
- [ ] Git commit + push

**下一步**：Phase 1 R2 — 删除过度工程 Agent 功能，削减 ~13,000 行代码
