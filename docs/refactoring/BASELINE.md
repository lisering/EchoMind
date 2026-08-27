# EchoMind 大简化重构 — 基线指标快照

> **生成日期**: 2026-08-27 ｜ **分支**: `refactor/great-simplification` ｜ **基线 tag**: `v3.1.0-final`
>
> 本文件记录重构前的全量基线指标，作为重构进展的对比基准。

---

## 1. Rust 代码指标

### 1.1 逐 Crate 行数

| Crate | 行数 | 文件数 | 备注 |
|---|---|---|---|
| `models` | 4,189 | 1 | 数据模型契约 |
| `prompt` | 1,667 | 5 | 提示词构建 |
| `compact` | 3,200 | 3 | 上下文压缩 |
| `context` | 1,776 | 5 | System Context Registry (OpenCode 借鉴) |
| `core` | **56,577** | **129** | 🔴 巨型上帝 crate，124+ 模块 |
| `infra` | **36,679** | **71** | 🔴 大量过度工程模块 |
| `tauri-app` | 26,307 | 56 | 🟡 IPC 命令过多 |
| `license-issuer` | 467 | 2 | 🟢 健康 |
| **合计** | **130,862** | **272** | |

### 1.2 模块行数 Top 10 (core)

| 模块 | 估计行数 | 问题 |
|---|---|---|
| `chat.rs` | ~3,000 | 核心但臃肿 |
| `import.rs` | ~2,500 | 核心但臃肿 |
| `agent.rs` + `agent_run.rs` | ~3,000 | 🔴 过度工程 (ReAct Agent) |
| `coordinator.rs` + `coordinator_strategy.rs` | ~2,500 | 🔴 过度工程 |
| `auto_dream.rs` | ~1,500 | 🔴 过度工程 |
| `memory_store.rs` | ~2,500 | 🔴 过度工程 |
| `speculative_rag.rs` | ~1,500 | 🔴 学术 RAG |
| `summary_tree.rs` | ~1,000 | 🔴 学术 RAG |
| `retrieval_memory.rs` | ~1,200 | 🔴 学术 RAG |
| `security.rs` | ~3,000 | 🟡 安全过度设计 |

---

## 2. 前端指标

| 指标 | 当前值 | 目标值 | 削减率 |
|---|---|---|---|
| JS 文件数 | 61 | ~25 | -59% |
| JS 总行数 | 30,923 | ~12,000 | -61% |
| CSS 文件数 | 6 | ~4 | -33% |
| esbuild 打包行数 | 1,826 | ~1,200 | -34% |
| 前端模块 (ui/src/) | 56 | ~20 | -64% |

### 2.1 前端模块行数 Top 10

| 文件 | 行数 | 问题 |
|---|---|---|
| `settings-advanced.js` | 1,562 | 🔴 设置爆炸根源 |
| `chat-render.js` | 1,485 | 🟡 大但可接受 |
| `graph-renderer.js` | 1,095 | 🔴 知识图谱可视化 |
| `sidebar.js` | 1,070 | 🟡 功能过多 |
| `security.js` | 1,061 | 🟡 可简化 |
| `chat.js` | 1,057 | 🟢 核心功能 |
| `main.js` | 991 | 🟡 入口偏大 |
| `export.js` | 965 | 🟡 可简化 |
| `context-menu.js` | 850 | 🟡 过度设计 |
| `markdown.js` | 785 | 🟢 核心功能 |

---

## 3. IPC 命令指标

| 命令域 | 命令数 | 目标 | 策略 |
|---|---|---|---|
| chat | 17 | 8 | 删除 agent/coordinator/dream |
| document | 19 | 8 | 删除标签/预览/摘要 |
| conversation | 25 | 8 | 删除 strip/分支树 |
| settings | 42 | 5 | 合并为统一 get/set |
| security | 22 | 8 | 删除 posture/shadow |
| local_llm | 23 | 5 | 删除自研内核相关 |
| performance | 23 | 0 | 全部删除 |
| graph | 9 | 0 | 全部删除 |
| import | 11 | 6 | 合并 wiki_link |
| mcp | 6 | 0 | 全部删除 |
| agent | 3 | 0 | 全部删除 |
| audit | 3 | 2 | 保留基本 |
| license | 3 | 3 | 不变 |
| sync | 4 | 2 | 简化 |
| rag_eval | 4 | 0 | 全部删除 |
| trace | 4 | 0 | 全部删除 |
| workspace | 9 | 4 | 简化 |
| features | 0 | 0 | 已为 0 |
| mod (helper) | 1 | 0 | 删除 |
| **合计** | **228** | **~80** | **-65%** |

---

## 4. 测试基线

| 测试类型 | 通过数 | 失败数 | 忽略数 | 备注 |
|---|---|---|---|---|
| Rust Free (`cargo test`) | **1,714** | 0 | 31 | 基线确认 |
| Rust Pro (`cargo test --features pro`) | **2,008** | 0 | 49 | 基线确认 |
| Clippy Free | 零警告 | - | - | `cargo clippy --all-targets -- -D warnings` ✅ |
| Clippy Pro | 零警告 | - | - | `cargo clippy --all-targets --features pro -- -D warnings` ✅ |
| cargo fmt | 通过 | - | - | `cargo fmt --check` ✅ |
| TypeScript | 0 errors | - | - | `npx tsc --noEmit` ✅ |
| esbuild 构建 | 1,826 行 | - | - | `node scripts/build-ui.mjs` ✅ |
| Vitest | **1,826** | 0 | - | 92 files, 18.09s ✅ |
| E2E Bridge | 1,355 | 0 | 10 | (历史基线，本次未重跑) |

---

## 5. 依赖指标

| 指标 | 当前值 | 目标值 | 削减率 |
|---|---|---|---|
| cargo tree 节点总数 | 1,765 | ~800 | -55% |
| 重复依赖包数 | 119 | ~50 | -58% |
| feature 依赖行数 | 5,295 | ~2,000 | -62% |

---

## 6. 文档指标

| 指标 | 当前值 | 目标值 | 削减率 |
|---|---|---|---|
| Markdown 文件数 | 258 | ~20 | -92% |
| E2E 测试文件数 | 213 | ~40 | -81% |
| E2E 测试行数 | 23,923 | ~12,000 | -50% |

---

## 7. 版本与 CI

| 指标 | 当前值 | 目标值 |
|---|---|---|
| 版本号 | 3.1.0 | 1.0.0-rc.1 (Phase 8) |
| CI 构建矩阵 | 4 OS × Free+Pro | 3 OS × Free-only |
| CI 全绿 | ✅ | ✅ 维持 |

---

## 8. 构建时间基线

| 步骤 | 时间 | 备注 |
|---|---|---|
| `cargo test` (Free) | 2m 51s (171s) | 1,714 tests |
| `cargo test` (Pro) | 3m 6s (186s) | 2,008 tests |
| `cargo clippy` (Free) | 1m 31s (91s) | 零警告 |
| `cargo clippy` (Pro) | 2m 16s (136s) | 零警告 |
| `npx tsc --noEmit` | <5s | 0 errors |
| `node scripts/build-ui.mjs` | <5s | 1,826 行 |
| Vitest | 18s | 1,826 tests, 92 files |

---

## 9. 基线总结

| 维度 | 当前值 | 目标值 | 削减率 |
|---|---|---|---|
| Rust 源码总行数 | 130,862 | ~50,000 | -62% |
| 最大单 crate 行数 | 56,577 (core) | ≤ 10,000 | 拆分 |
| 前端 JS 总行数 | 30,923 | ~12,000 | -61% |
| IPC 命令数 | 228 | ~80 | -65% |
| 依赖总数 | 1,765 | ~800 | -55% |
| Rust 测试数 (Free+Pro) | 3,722 (1,714+2,008) | ~1,500 | -60% |
| Vitest 测试数 | 1,826 | ~600 | -67% |
| 文档文件数 | 258 | ~20 | -92% |
| 版本号 | 3.1.0 | 1.0.0-rc.1 | 校准 |

---

> **基线确认人**: AI Agent (Phase 0, 2026-08-27)
>
> **Git tag**: `v3.1.0-final`
>
> **分支**: `refactor/great-simplification`
