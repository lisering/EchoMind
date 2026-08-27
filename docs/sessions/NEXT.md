# Phase 1 R1 — 删除学术 RAG 优化

生成日期：2026-08-27 ｜ 生成人：AI Agent (Phase 0 完成)

当前版本：v3.1.0 ｜ 分支：`refactor/great-simplification` ｜ 基线 tag：`v3.1.0-final`

核心文档：`docs/architecture/GREAT_SIMPLIFICATION_PLAN.md`（重构总计划，必读）
基线文档：`docs/refactoring/BASELINE.md`（基线指标快照）+ `docs/refactoring/DEPENDENCY_AUDIT.md`（依赖审计）

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate。当前 v3.1.0，已进入"大简化重构"阶段。

**重构模式**：所有 session 从"添加功能"转向"删除和简化"。每个 session 的产出不是新功能，而是更少的代码。

## 本次会话成果 — Phase 0 冻结与基线

- ✅ git tag `v3.1.0-final` 已创建
- ✅ 分支 `refactor/great-simplification` 已创建并切换
- ✅ `docs/refactoring/BASELINE.md` 已生成（全量基线指标）
- ✅ `docs/refactoring/DEPENDENCY_AUDIT.md` 已生成（依赖审计报告）
- ✅ 全量测试基线确认：
  - Rust Free: 1,714 passed, 0 failed
  - Rust Pro: 2,008 passed, 0 failed
  - Clippy Free+Pro: 零警告
  - cargo fmt: 通过
  - tsc: 0 errors
  - esbuild: 1,826 行
  - Vitest: 1,826 passed, 92 files

## 基线指标速查

| 维度 | 当前值 | 目标值 | 削减率 |
|---|---|---|---|
| Rust 源码总行数 | 130,862 | ~50,000 | -62% |
| 最大单 crate 行数 | 56,577 (core) | ≤ 10,000 | 拆分 |
| 前端 JS 总行数 | 30,923 | ~12,000 | -61% |
| IPC 命令数 | 228 | ~80 | -65% |
| 依赖总数 | 1,765 | ~800 | -55% |
| Rust 测试数 | 3,722 | ~1,500 | -60% |
| Vitest 测试数 | 1,826 | ~600 | -67% |
| 文档文件数 | 258 | ~20 | -92% |

## 下个 Session 任务 — Phase 1 R1：删除学术 RAG 优化

**目标**：删除所有学术 RAG 优化模块及其测试，削减 ~10,000 行代码。

### 删除清单

| 模块 | 文件 | 估计行数 |
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

**预计削减**: ~10,000 行

### 额外清理

- 删除 `crates/infra/src/sqlite_cache.rs` + tests
- 删除 `crates/infra/src/onnx_compressor.rs` (Pro)
- 删除 `crates/infra/src/colbert_embedder.rs` + tests (Pro)
- 清理 `crates/tauri-app/src/commands/performance.rs` 中相关 IPC 命令 (~15 个)
- 清理 `ui/src/perf-settings.js` 前端模块
- 清理相关设置键 (~20 个)
- 清理相关 E2E 测试
- 从 `crates/core/src/lib.rs` 中移除 `pub mod` 声明
- 从 `crates/infra/src/lib.rs` 中移除 `pub mod` 声明
- 从 `crates/tauri-app/src/lib.rs` 的 `generate_handler!` 中移除相关命令注册
- 从 `crates/tauri-app/src/commands/mod.rs` 的 `pub use` 中移除相关重导出
- 从 AppState (`crates/tauri-app/src/state.rs`) 中移除相关字段
- 从 `chat_inner` (`crates/tauri-app/src/commands/chat.rs`) 中移除相关分支逻辑
- 清理 `ui/src/ipc.js` 中相关 IPC 封装方法
- 清理 `ui/src/main.js` 中相关导入和初始化
- 清理 `ui/locales/{en,zh-CN}.json` 中相关 i18n 键
- 清理 `e2e-tests/tests/bridge/tauri-stub.js` 中相关 mock
- 从 SRS (`docs/SRS_v1.0.md`) 中标记 REQ-PERF-001~012 为 Deprecated
- 同步更新 `docs/api/` 下相关文档

### 删除策略

1. **先删 Rust 模块**：删除 `.rs` 文件 + 测试文件，从 `lib.rs` 移除 `pub mod` 声明
2. **再删 IPC**：从 `commands/performance.rs` 删除命令函数，从 `lib.rs` 的 `generate_handler!` 移除注册
3. **再删前端**：删除 `perf-settings.js`，从 `main.js` 移除导入，从 `settings.js` 移除面板入口
4. **再删 IPC 封装**：从 `ipc.js` 移除相关方法
5. **再删 mock**：从 `tauri-stub.js` 移除相关 mock
6. **再删 E2E**：删除相关 E2E 测试文件
7. **最后更新文档**：SRS REQ 状态 + api 文档

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

### Phase 1 R1 完成标准

- [ ] 所有学术 RAG 模块文件已删除
- [ ] `cargo check` 编译通过
- [ ] `cargo test` 全量通过（测试数减少 ~500-800）
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] `cargo fmt --check` 通过
- [ ] `node scripts/build-ui.mjs` 构建成功
- [ ] `npx tsc --noEmit` 零错误
- [ ] SRS REQ-PERF-001~012 标记为 Deprecated
- [ ] `docs/api/` 文档已同步更新
- [ ] NEXT.md 已更新为 Phase 1 R2 衔接提示词

## 重构全景路线图

| Phase | 内容 | Sessions | 状态 |
|---|---|---|---|
| Phase 0 | 冻结与基线 | 1 | ✅ 完成 |
| **Phase 1 R1** | **删除学术 RAG 优化** | **1** | **← 下个 Session** |
| Phase 1 R2 | 删除过度工程 Agent 功能 | 1 | 待定 |
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

## 关键约束 (铁律，重构期间继续遵守)

- **代理仅限开发调研，软件本身禁止代理** — `reqwest::Client::builder()` 必须加 `.no_proxy()`
- **TDD 驱动** — 删除功能时同步删除测试，修改功能时先改测试
- **生产级代码质量** — 零 `unwrap()/expect()/panic!()/unreachable!()/todo!()/unimplemented!()`
- **代码变更同步更新文档** — `docs/api/` + `docs/SOFTWARE_ASSET.md`
- **修改源代码后运行 `./sync-to-github.sh`**
- **Session 结束更新 NEXT.md + LOG.md**
- **前端构建**：`node scripts/build-ui.mjs`（esbuild 打包 IIFE，不要手改 `ui/index.html`）
- **TypeScript**：`npx tsc --noEmit` 类型检查
- **依赖方向**：`tauri-app → core → models`，严禁反向
- **Edition 2024**：原生 `async fn` in trait，无 `async-trait` 宏
- **先写规格再写代码** — 删除功能时同步更新 SRS REQ 状态为 `Deprecated`
- **Git 历史保留** — 删除的代码通过 Git 历史可恢复，无需注释保留
- **不修改数据库 schema** — 删除功能只删除相关查询代码，不删除数据库表

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

## 关键文件索引

| 文件 | 说明 |
|---|---|
| `docs/architecture/GREAT_SIMPLIFICATION_PLAN.md` | 重构总计划 (必读) |
| `docs/refactoring/BASELINE.md` | 基线指标快照 |
| `docs/refactoring/DEPENDENCY_AUDIT.md` | 依赖审计报告 |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `docs/sessions/NEXT.md` | 本文件 (Phase 1 R1 衔接提示词) |
| `docs/sessions/LOG.md` | 会话时间线日志 |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `Cargo.toml` | Workspace 配置 |

## Session 结束时必须完成

- ✅ 更新 `docs/sessions/NEXT.md` — 替换为下个 session (Phase 1 R2) 的提示词
- ✅ 追加 `docs/sessions/LOG.md` — 记录本次 session 的时间线和成果
- ✅ 更新 `docs/sessions/CURRENT_STATE.md` — 更新状态快照
- ✅ 运行 `./sync-to-github.sh --commit` — 同步到 GitHub 发布仓库
- ✅ Git commit + push

**下一步**：Phase 1 R1 — 删除学术 RAG 优化模块，削减 ~10,000 行代码
