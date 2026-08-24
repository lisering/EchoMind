# EchoMind 复盘 S1 报告：代码审计 + 架构健康度

> **日期**: 2026-08-10 | **版本**: v1.4.0 后 | **审计人**: AI Agent

## 一、代码质量审计

### 1.1 unwrap/expect 违反审查

| 文件 | 数量 | 类型 | 严重度 | 说明 |
|---|---|---|---|---|
| `privacy.rs` | 10 | `Regex::new().unwrap()` | ✅ 安全 | LazyLock 静态正则，编译时已知有效，`#![allow]` + 注释说明 |
| `entity_extractor.rs` | 1 | `Regex::new().unwrap()` | ✅ 安全 | 宏内编译时已知正则，`#![allow]` |
| `local_llm.rs` | 4 | doc comment | ✅ 无害 | `/// #` doctest 示例 |
| `layer_prefetch.rs` | 3 | doc comment | ✅ 无害 | `/// #` doctest 示例 |
| `local_logger.rs` | 2 | doc comment | ✅ 无害 | `///` doctest 示例 |
| `kv_cache.rs` | 2 | doc comment | ✅ 无害 | `///` doctest 示例 |
| `skill.rs` | 1 | doc comment | ✅ 无害 | `///` doctest 示例 |
| `lib.rs` | 1 | doc comment | ✅ 无害 | `///` doctest 示例 |
| **合计** | **24** | | **0 危险** | 全部可接受 |

**结论：铁律五（生产级代码质量）遵守良好，无危险 unwrap/expect。**

### 1.2 依赖方向检查

| 检查项 | 结果 |
|---|---|
| `core → infra/tauri` 反向依赖 | ✅ 零违反 |
| `models → core/infra` 反向依赖 | ✅ 零违反 |
| `prompt → infra` 反向依赖 | ✅ 零违反 |
| `compact → infra` 反向依赖 | ✅ 零违反 |
| `core` 依赖 `rusqlite/reqwest/fastembed` | ✅ 零违反 |

**结论：六边形架构依赖方向严格，零违规。**

## 二、IPC 僵尸命令排查

### 2.1 概况

- **后端注册**: 194 个 IPC 命令
- **前端调用**: 139 个
- **僵尸命令**: **55 个**（28.4% 僵尸率）

### 2.2 僵尸命令分类

| 分类 | 数量 | 命令 | 根因 |
|---|---|---|---|
| **安全功能未接线** | 13 | `set_panic_wipe_password`, `clear_panic_wipe_password`, `is_panic_wipe_enabled`, `set_auto_lock_config`, `unlock_app`, `record_activity`, `check_password_strength`, `clear_audit_logs`, `set_clipboard_config`, `set_security_posture`, `get_security_posture`, `get_security_screen_stats`, `reset_security_screen_stats` | 后端实现完整但 UI 未集成 |
| **Pro LLM 功能未接线** | 10 | `save_kv_cache`, `load_kv_cache`, `clear_kv_cache`, `get_kv_cache_status`, `set_kv_cache_enabled`, `set_kernel_mode`, `get_kernel_mode`, `get_local_llm_device_kind`, `set_paged_attn`, `set_embedder_model` | 设置面板部分接线，高级功能缺失 |
| **记忆/QM 功能未接线** | 6 | `push_burst_turn`, `flush_memory_burst_buffer`, `get_burst_buffer_status`, `trigger_memory_consolidation`, `get_scratch_logs`, `pin_memory` | Q01-Q02 借鉴后端完成但前端未集成 |
| **Durable/B08 未接线** | 8 | `admit_input`, `promote_input`, `get_pending_inputs`, `add_session_todo`, `update_todo_status`, `get_session_todos`, `delete_session_todo`, `delete_session_todos` | B05/B08 借鉴仅后端实现 |
| **Trace/Budget 未接线** | 8 | `get_recent_traces`, `get_trace_detail`, `clear_traces`, `get_trace_count`, `get_budget_stats`, `set_budget_limit`, `get_token_budget_config`, `set_token_budget_config` | S69/S70/Q08 借鉴仅后端实现 |
| **性能优化未接线** | 4 | `build_summary_tree`, `rebuild_bm25_index`, `rebuild_proposition_index`, `rebuild_contextual_embeddings` | 索引重建命令未暴露 UI |
| **其他未接线** | 6 | `get_conversations_paginated`, `get_messages_paginated`, `get_graph_layout`, `get_symbols_for_chunk`, `reclassify_document`, `web_search` | 分页 API 未使用 / 功能入口缺失 |

### 2.3 僵尸率根因分析

**核心问题**：借鉴驱动开发导致"后端先行，UI 滞后"。

5 个借鉴来源共计 36 个借鉴项，其中大部分精力投入后端 trait + 实现 + 测试，但前端 UI 集成被推迟到"后续 Session"而从未完成。这是**借鉴消化度不足**的直接体现。

## 三、架构健康度

### 3.1 各 crate 规模

| Crate | 行数 | 文件数 | 评价 |
|---|---|---|---|
| `core` | 47,523 | 105 | ⚠️ 偏大，但模块化良好 |
| `infra` | 29,731 | 53 | ⚠️ 偏大，sqlite_storage.rs 是瓶颈 |
| `tauri-app` | 12,061 | 27 | ✅ 合理 |
| `models` | 3,304 | 1 | ✅ 合理（纯数据定义） |
| `compact` | 3,004 | 3 | ✅ 合理 |
| `context` | 1,776 | 5 | ✅ 合理 |
| `prompt` | 543 | 3 | ✅ 合理 |
| `license-issuer` | 467 | 2 | ✅ 合理 |

### 3.2 重构候选

| 文件 | 行数 | 问题 | 建议 |
|---|---|---|---|
| `sqlite_storage.rs` | 4,925 | 单文件过大，Storage trait + SQLCipher + AuditLogger + FTS5 混合 | 拆分为 `sqlite_storage/mod.rs` + `audit_logger.rs` + `fts_search.rs` + `vector_search.rs` |
| `core/lib.rs` | 2,162 | 16 个 trait + 30+ 模块注册集中 | 拆分为 `traits/mod.rs` + `traits/storage.rs` + `traits/llm.rs` 等 |
| `features.rs` | 2,138 | 15+ 功能域混合 | 按功能域拆分（memory / dream / symbol / workflow / code_exec 等） |
| `chat.rs` (commands) | 1,776 | 对话 + RAG + Agent + Coordinator 混合 | 已拆分但仍偏大，可进一步分离 agent 路径 |

### 3.3 前端模块复杂度

| 模块 | 行数 | 评价 |
|---|---|---|
| `graph-viewer.js` | 1,879 | ⚠️ D3 可视化逻辑复杂，可拆分渲染/交互/导出 |
| `settings.js` | 1,557 | ⚠️ 设置面板过大，已部分拆分 |
| `main.js` | 1,325 | ✅ 主入口，包含初始化和事件绑定 |
| 其余 51 模块 | < 900 | ✅ 合理 |

## 四、测试统计

| 测试类型 | 数量 |
|---|---|
| Rust `#[test]` | 800 |
| E2E `test()` | 1,188 |
| Vitest `it()` | 422 |
| **总计** | **2,410** |

测试/代码比 ≈ 1:2.3（业界合理水平）

## 五、SRS 需求覆盖

| 状态 | 数量 | 占比 |
|---|---|---|
| Done | 100 | 40.3% |
| Implemented | 96 | 38.7% |
| Draft | 52 | 21.0% |
| Deprecated | 1 | 0.4% |
| **总计** | **249** | **100%** |
| **已交付** | **196** | **79.0%** |

## 六、借鉴消化度矩阵

| 来源 | 借鉴项数 | 后端完成 | 前端接线 | 消化度 |
|---|---|---|---|---|
| DS4 (antirez) | 3 | 3/3 | 0/3 | 33% |
| OpenCode | 14 | 14/14 | 6/14 | 43% |
| OpenMontage | 7 | 7/7 | 3/7 | 43% |
| QuantaMind | 11 | 11/11 | 3/11 | 36% |
| Cherry Studio | 2 | 2/2 | 0/2 | 33% |
| **总计** | **37** | **37/37** | **12/37** | **40%** |

**核心发现**：后端借鉴消化度 100%，但前端 UI 接线仅 40%。大量借鉴功能"写了后端但用户看不到"。

## 七、复盘 S1 总结

### 7.1 优势

1. **架构纪律优秀**：零依赖方向违反，六边形架构严格执行
2. **代码安全质量高**：零危险 unwrap/expect，workspace lint deny 强制执行
3. **测试覆盖充分**：2,410 个测试，测试/代码比 1:2.3
4. **需求规格化**：248 个 REQ，79% 已交付，SRS 作为唯一依据

### 7.2 劣势（按严重度排序）

| # | 问题 | 严重度 | 影响范围 |
|---|---|---|---|
| 1 | **55 个 IPC 僵尸命令**（28% 僵尸率） | 🔴 高 | 37 个借鉴功能中 25 个无 UI 入口 |
| 2 | **借鉴消化度仅 40%** | 🔴 高 | 用户无法使用 60% 的借鉴功能 |
| 3 | **52 个 Draft REQ 未分类** | 🟡 中 | 需判断 Obsolete / Needed / Deferred |
| 4 | **sqlite_storage.rs 4925 行** | 🟡 中 | 可维护性风险 |
| 5 | **core lib.rs 2162 行** | 🟢 低 | trait 集中定义，可接受 |
| 6 | **features.rs 2138 行** | 🟢 低 | 已拆分但仍偏大 |

### 7.3 推荐优先级

| 优先级 | 行动 | 预期收益 |
|---|---|---|
| P0 | **批量接线 55 个僵尸命令** | 28% → <5% 僵尸率，借鉴消化度 40% → 90% |
| P1 | **52 个 Draft REQ 分类** | 清理需求 backlog，明确下一步 |
| P2 | **sqlite_storage.rs 拆分** | 可维护性提升 |
| P3 | **core lib.rs trait 分拆** | 编译加速 + 可读性 |
