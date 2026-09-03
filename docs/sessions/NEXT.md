# Phase 1 R5 — 删除前端过度工程模块

> 生成日期：2026-08-28 ｜ Phase 1 R4 已完成

当前版本：v3.1.0 ｜ 分支：`refactor/great-simplification` ｜ 基线 tag：`v3.1.0-final`

核心文档：`docs/architecture/GREAT_SIMPLIFICATION_PLAN.md`（重构总计划，必读）
基线文档：`docs/refactoring/BASELINE.md`（基线指标快照）

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate。当前 v3.1.0，已进入"大简化重构"阶段。

**重构模式**：所有 session 从"添加功能"转向"删除和简化"。每个 session 的产出不是新功能，而是更少的代码。

## 本次会话成果 — Phase 1 R4：删除过度工程工具与借鉴

- ✅ 删除 14 个过度工程模块（audit_pair, concurrency, event_stream, llm_protocol, mmr_diversifier, permission, pipeline_checkpoint, retrieval_quality_gate, scored_selector, session_coordinator, skill + coordination_strategy, speculative_executor, tool_orchestrator 之前已删）
- ✅ 重写 compact/src/lib.rs：移除 CompactionTrigger, DualThresholdConfig, VerbatimTailConfig, TokenEstimator, heal_dangling_calls, schedule_background_compaction, run_background_compaction, try_acquire_background_compaction, release_background_compaction 等过度工程压缩逻辑
- ✅ 简化 chat.rs：移除双阈值触发、后台异步压缩、SessionCoordinator 包装、persist_queue 序列化
- ✅ 清理 AppState：移除 compaction_pending, session_coordinator, persist_queue 字段
- ✅ 重写 compact_tests.rs：仅保留 10 个有效 TC-COMPACT-001~010 测试
- ✅ Skill 系统全量清理：删除 skill.rs IPC 命令 + tauri-stub.js mock + lib.rs discover_skills 注册
- ✅ 全量测试通过：compact 29 tests, core 595 tests, tauri-app 143 tests
- ✅ Clippy 零警告，fmt 通过，TypeScript 类型检查通过，esbuild 构建成功

## 基线指标速查

| 维度 | 当前值 (R4 后) | 目标值 | 剩余削减 |
|---|---|---|---|
| Rust 源码总行数 | ~85,000 | ~50,000 | -41% |
| 最大单 crate 行数 | ~32,000 (core) | ≤ 10,000 | 拆分 |
| 前端 JS 总行数 | ~28,000 | ~12,000 | -57% |
| IPC 命令数 | ~180 | ~80 | -56% |
| 依赖总数 | ~1,600 | ~800 | -50% |
| Rust 测试数 | ~767 | ~1,500 | +96% |

## 下个 Session 任务 — Phase 1 R5：删除前端过度工程模块

**目标**：删除前端未使用的 UI 组件和功能模块，削减 ~5,000 行 JavaScript/TypeScript 代码。

### 清理范围

| 模块 | 路径 | 估计行数 |
|---|---|---|
| Skill Panel 组件 | `ui/src/skill/` | ~800 |
| Event Stream Viewer | `ui/src/event-stream/` | ~600 |
| Session Coordinator UI | `ui/src/session/` | ~500 |
| Pipeline Checkpoint UI | `ui/src/pipeline/` | ~500 |
| Quality Gate Dashboard | `ui/src/quality/` | ~600 |
| Scored Selector UI | `ui/src/scoring/` | ~500 |
| Audit Panel | `ui/src/audit/` | ~500 |

**预计削减**: ~4,000-5,000 行

## 重构全景路线图

| Phase | 内容 | Sessions | 状态 |
|---|---|---|---|
| Phase 0 | 冻结与基线 | 1 | ✅ 完成 |
| Phase 1 R1 | 删除学术 RAG 优化 | 1 | ✅ 完成 |
| Phase 1 R2 | 删除过度工程 Agent 功能 | 1 | ✅ 完成 |
| Phase 1 R3 | 删除过度工程 LLM 功能 | 1 | ✅ 完成 |
| **Phase 1 R4** | **删除过度工程工具与借鉴** | **1** | **✅ 完成** |
| Phase 1 R5 | 删除前端过度工程模块 | 1 | ← 下个 Session |
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
| `docs/sessions/NEXT.md` | 本文件 (Phase 1 R5 衔接提示词) |
| `docs/sessions/LOG.md` | 会话时间线日志 |
| `docs/SRS_v1.0.md` | 需求规格唯一依据 |
| `docs/api/ipc-commands.md` | IPC 命令签名 |
| `docs/api/code-contracts.md` | Trait 签名 + 数据模型 |
| `.catpaw/rules/system-constraints.md` | 铁律规则 |
| `Cargo.toml` | Workspace 配置 |

## Session 结束时必须完成

- ✅ 更新 `docs/sessions/NEXT.md` — 替换为下个 session (Phase 1 R5) 的提示词
- ✅ 追加 `docs/sessions/LOG.md` — 记录本次 session 的时间线和成果
- ✅ 更新 `docs/sessions/CURRENT_STATE.md` — 更新状态快照
- ✅ 运行 `./sync-to-github.sh --commit` — 同步到 GitHub 发布仓库
- ✅ Git commit + push

**下一步**：Phase 1 R5 — 删除前端过度工程模块，削减 ~5,000 行前端代码
