# EchoMind 下个 Session 提示词

你是 CatPaw，在 CatPaw IDE 中工作。用中文回答（代码除外）。工作目录：`/Users/john/freesoft/EchoMind`。

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。**v2.4.0 已发布**，**v3.0 规划已完成**。当前 **1552 Free / 1857 Pro** 测试通过。fastembed ONNX 嵌入（5 种 EmbeddingModel），SQLite 向量存储 + HNSW 索引，任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。前端 72 个 ES module（vanilla JS，esbuild 打包 IIFE）。

## 上 Session 完成内容（S94 Toggle 5→2 精简）

### Toggle 精简 ✅ 完成

| 变更 | 详情 |
|---|---|
| `ui/src/input-toggles.js` | TOGGLE_CONFIG 从 5 项精简为 2 项（hybrid + agent），移除 sub_agent/memory/web_search。警告信息更新为 "S94 精简后仅支持 hybrid/agent" |
| `ui/locales/en.json` | 移除 `toggle_sub_agent`/`toggle_memory`/`toggle_web_search`，`toggle_agent` → "Deep Thinking" |
| `ui/locales/zh-CN.json` | 同上，`toggle_agent` → "深度思考" |
| `ui/locales/ja.json` | 同上，`toggle_agent` → "深い思考" |
| `e2e-tests/tests/unit/input-toggles.test.js` | 新增 7 个 Vitest 单元测试（TC-DS-010~012e），使用 jsdom 环境 |
| `e2e-tests/tests/ui-comprehensive.spec.ts` | TC-INPUT-006 更新期望 toggle 数为 2 |
| `docs/sessions/CURRENT_STATE.md` | 状态更新 |

### S94 已完成 vs 待完成

S94 有 6 个验收标准（AC-1~AC-6），本次 Session 完成了其中 1 个：

| AC | 描述 | 状态 | 说明 |
|---|---|---|---|
| AC-5 | 输入区 toggle 5→2 | ✅ 完成 | TOGGLE_CONFIG 精简 + i18n 清理 + 7 Vitest 测试 |
| AC-1 | 设置面板 35→8 分区 | ❌ 待完成 | `settings.js` SETTINGS_TABS 部分更新，但 `ui/index.html` 的 8 分区 HTML 结构 + `settings-general.js`/`settings-advanced.js` 分区合并未完成 |
| AC-2 | 智能模式单一开关 | ❌ 待完成 | `perf-settings.js` 已有智能模式开关（S6 P1-5），但后端 IPC `set_smart_mode`/`get_smart_mode` 未实现，设置键 `ui.smart_mode` 未注册 |
| AC-3 | 20 个术语通俗化 | 🟡 部分完成 | i18n 中 perf.* 系列已通俗化（"逐步补充"/"快速预览"/"质量检查"等），但 settings.* 中的部分术语（Speculative RAG / PagedAttention / GEMV / KV Cache 等）可能仍为英文原文 |
| AC-4 | 开发者菜单隔离 | ❌ 待完成 | 4 个开发者工具（RAG Eval / Trace / Durable / Session Strip）移入隐藏菜单未实现。注意 durable-panel.js / session-strip.js 已在 S12 删除，需检查残留引用 |
| AC-6 | 智能模式 ON/OFF 联动 | ❌ 待完成 | 智能模式开启时隐藏高级调优分区、关闭时展开全部设置未实现 |

### 测试基线（不变）

- Rust Free: **1552** passed, 0 failed, 23 ignored
- Rust Pro: **1857** passed, 0 failed, 49 ignored
- Clippy Free+Pro: 零警告
- cargo fmt: 通过
- tsc: 0 errors
- esbuild: **1590** 行
- Vitest: **1767** passed (88 files)
- 版本号: **2.4.0**

---

## 下个 Session 任务

### 继续 S94：设置面板重构剩余 5 个 AC

详细计划见 `docs/architecture/V3.0_ATOMIC_PLAN.md` §3.1。

#### 优先级 1：AC-2 智能模式 IPC + AC-6 联动（后端 + 前端）

1. **后端 IPC**：在 `crates/tauri-app/src/commands/settings.rs` 新增 2 个 IPC 命令：
   - `set_smart_mode(enabled: bool)` — 设置 `ui.smart_mode` 键，同时联动 6 项参数
   - `get_smart_mode() -> bool` — 读取 `ui.smart_mode` 键

2. **联动逻辑**（智能模式 ON 时自动设置）：
   | 参数 | ON | OFF |
   |---|---|---|
   | hybrid_search | true | 恢复用户上次选择 |
   | rerank_enabled | true | false |
   | hyde_enabled | true | false |
   | graph_retriever_enabled | true | false |
   | quality_gate_enabled | true | false |
   | compression_ratio | 2x | 1x |

3. **前端联动**：智能模式 ON 时隐藏「高级调优」分区（折叠），OFF 时展开全部设置

#### 优先级 2：AC-1 设置面板 35→8 分区 HTML 重构

1. **`ui/index.html`**：将当前 ~16 个 `data-settings-section` 容器精简为 8 个：
   - ① 模型配置 ② 外观语言 ③ 知识库 ④ 安全隐私 ⑤ 智能检索（含智能模式开关） ⑥ 高级调优（默认折叠） ⑦ 应用设置 ⑧ 快捷指令

2. **`ui/src/settings-general.js`** + **`ui/src/settings-advanced.js`**：合并/重组织分区内容到新的 8 分区结构

3. **`ui/src/settings.js`**：确认 `SETTINGS_TABS` 8 分区 Tab 导航正确

#### 优先级 3：AC-4 开发者菜单隔离

- 将 RAG Eval / Trace / Budget / Durable / Session Strip 设置移入隐藏的「开发者菜单」
- 注意：`durable-panel.js` / `session-strip.js` / `workflow-panel.js` / `code-executor.js` 已在 S12 删除，检查残留引用
- 前端 `devInvoke()` 已实现（S10），开发者菜单需对应门控

#### 优先级 4：AC-3 术语通俗化验证

- 检查 `ui/locales/{en,zh-CN}.json` 中 20 个术语是否全部通俗化
- 如发现仍为英文原文的术语（如 PagedAttention / GEMV 内核 / KV Cache 等），更新为通俗化文案

#### TDD 测试

- TC-UI-SETTINGS-001：设置面板 8 个分区 Tab（E2E）
- TC-UI-SETTINGS-002：智能模式开关默认开启（E2E）
- TC-UI-SETTINGS-003：术语通俗化文案 + tooltip（E2E）
- TC-UI-SETTINGS-004：开发者菜单隐藏（E2E）
- TC-UI-SETTINGS-006：智能模式 ON/OFF 联动（E2E）
- ✅ TC-UI-SETTINGS-005：输入区仅 2 个 toggle（已完成 — Vitest 7 passed + E2E TC-INPUT-006 更新）

---

### S95-S98 后续任务概览

| Session | 功能 | 关键点 | 状态 |
|---|---|---|---|
| S95 | 前端模块精简 72→≤60 | 12 个合并候选 + Release 构建移除开发者工具前端模块 | 📋 待启动 |
| S96 | 嵌入模型对比评估面板 | 复用 `RagEvaluator` + `RagEvalDataset`，新建 `ui/src/embed-eval.js` + `run_embed_comparison` IPC | 📋 依赖 S94 |
| S97 | MCP 协议 Phase 1 | `crates/core/src/mcp/` 新建模块，stdio + SSE 双 transport，6 IPC 命令 | 📋 待启动 |
| S98 | Pro ROI 评估 + v3.0 发布 | 14 个 Pro 功能 ROI 评估 + 版本号升级 2.4.0→3.0.0 | 📋 最后执行 |

### 执行顺序

```
S94 设置面板重构（进行中）───┐
                              ├─ S96 嵌入模型评估 ──┐
S95 模块精简 ──────────────────┘                      ├─ S98 Pro ROI 评估 + v3.0 发布
S97 MCP 协议 ───────────────────────────────────────┘
```

S94/S95/S97 可并行（无相互依赖）。S96 依赖 S94（设置面板重构后新增评估入口）。S98 必须最后执行。

### 详细计划

详见 `docs/architecture/V3.0_ATOMIC_PLAN.md`。

---

## 关键约束（铁律）

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现；生产代码严禁 `unwrap()/expect()/panic!()`
3. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
4. 修改源代码后运行 `./sync-to-github.sh --commit`
5. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成本文件
6. 先写规格再写代码（Spec-driven Development）
7. 前端样式铁律十三：Tailwind utility classes + design tokens，禁止硬编码 hex 颜色
8. `async fn in trait`（Edition 2024），无 `async-trait` 宏
9. 前端构建：`node scripts/build-ui.mjs`（esbuild 打包 IIFE，不要手改 `ui/index.html`）
10. Workspace lint deny：`unwrap_used` / `expect_used` / `panic` / `unreachable` / `todo` / `unimplemented` 全部 deny

## 验证命令

```bash
# Rust 测试
cargo test 2>&1 | grep -oE "[0-9]+ passed" | awk '{s+=$1} END {print "Free passed:"s}'
cargo test --features pro 2>&1 | grep -E "test result:" | awk '{print $4}' | awk -F, '{s+=$1} END {print "Pro passed:"s}'

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
| `docs/SRS_v1.0.md` | 需求规格唯一依据（v16.0.0，含 §4.28 v3.0 规划需求） |
| `docs/architecture/V3.0_ATOMIC_PLAN.md` | v3.0 原子计划（S94-S98） |
| `docs/sessions/archive/audit_s3_ui_evaluation.md` | S3 UI 审计报告（设置面板 35 分区详细清单 + 术语列表 + toggle 清单） |
| `docs/sessions/archive/audit_s1_frontend_modules.md` | S1 前端模块审计报告（72 模块等级分布 + 合并候选） |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `CHANGELOG_v2.4.md` | v2.4.0 发布说明 |

## 注意事项

1. **durable-panel.js / session-strip.js 已在 S12 删除**：设置面板重构时检查是否有残留的 `renderDurableTodoSettings()` / `renderSessionStripSettings()` 调用，如有则直接移除
2. **workflow-panel.js / code-executor.js 也已删除**（S12 Dead Code 清理）：同样检查残留引用
3. **perf-settings.js 智能模式开关**：S6 P1-5 已实现 perf-settings.js 智能模式开关（隐藏全部设置），S94 需要将其整合到新的「智能检索」分区中，并添加后端 IPC `set_smart_mode`/`get_smart_mode`
4. **S10 开发者工具门控**：21 个后端命令已有 `#[cfg(debug_assertions)]` 门控，前端 `devInvoke()` 也已实现。S94 需确保开发者菜单在前端也正确门控
5. **esbuild 打包**：修改 `ui/src/*.js` 后必须运行 `node scripts/build-ui.mjs` 重新打包
6. **i18n 双语**：所有新增文案需同时更新 `ui/locales/en.json` 和 `ui/locales/zh-CN.json`（+ `ja.json` 三语）
7. **input-toggles.test.js 测试环境**：该测试使用 vitest.config.ts 提供的 jsdom 环境（`environment: 'jsdom'`），mock i18n/ipc/state 三个依赖模块，不需要自定义 DOM mock
