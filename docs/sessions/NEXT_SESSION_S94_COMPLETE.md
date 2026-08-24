# 下个 Session 衔接提示词 — S94 设置面板重构完成

> **生成时间**: 2026-08-22
> **项目**: EchoMind — Rust + Tauri v2 本地 RAG 知识库
> **工作目录**: `/Users/john/freesoft/EchoMind`

## 项目概要

EchoMind — Rust + Tauri v2 本地 RAG 知识库。六边形架构 8 crate：models → prompt → core → infra → tauri-app（+ compact / context / license-issuer）。v2.4.0 已发布，v3.0 规划已完成。当前 1552 Free / 1857 Pro 测试通过。fastembed ONNX 嵌入（5 种 EmbeddingModel），SQLite 向量存储 + HNSW 索引，任意 OpenAI 兼容 LLM。Pro 功能：本地 LLM 推理（mistral.rs）、GPU 加速、ColBERT、tree-sitter、WasmExecutor。前端 72 个 ES module（vanilla JS，esbuild 打包 IIFE）。

## 上 Session 完成内容（S94 设置面板重构）

**S94 全部 6 个 AC 已完成 ✅**

| AC | 描述 | 状态 | 说明 |
|---|---|---|---|
| AC-1 | 设置面板 35→8 分区 | ✅ | `ui/index.html` 已有 8 个 `data-settings-section` 容器 + `settings.js` SETTINGS_TABS 8 项 |
| AC-2 | 智能模式单一开关 | ✅ | 后端 `set_smart_mode`/`get_smart_mode` IPC（settings.rs）+ 前端 `smartModeApi`（ipc.js）+ 6 项参数联动 |
| AC-3 | 20 个术语通俗化 | ✅ | 三语更新：rerank（Cross-Encoder→结果重排序）、progressive（→逐步补充/Smart Context Injection）、graph_retriever（→知识图谱检索）、contextual（→上下文增强嵌入） |
| AC-4 | 开发者菜单隔离 | ✅ | RAG Eval/Trace/Budget 已 `_devMode` 门控（⌘Shift+D 切换）+ 零残留引用（durable-panel/session-strip/workflow-panel/code-executor 已在 S12 删除） |
| AC-5 | 输入区 toggle 5→2 | ✅ | `input-toggles.js` TOGGLE_CONFIG 5→2（hybrid + agent）+ i18n 清理 |
| AC-6 | 智能模式 ON/OFF 联动 | ✅ | `perf-settings.js` 智能模式 ON 时 `.hidden` 隐藏全部高级设置，OFF 时展开 |

### 变更详情

- `ui/locales/en.json` — 4 个术语通俗化：`rerank_toggle` → "Result Re-ranking" / `progressive_toggle` → "Smart Context Injection" / `graph_retriever_toggle` → "Knowledge Graph Search" / `contextual_retrieval_toggle` → "Context-Enhanced Embeddings"
- `ui/locales/zh-CN.json` — `rerank_toggle` → "结果重排序"
- `ui/locales/ja.json` — `rerank_toggle` → "結果再ランキング" / `progressive_toggle` → "スマートコンテキスト注入" / `graph_retriever_toggle` → "ナレッジグラフ検索" / `contextual_retrieval_toggle` → "コンテキスト強化埋め込み"
- `e2e-tests/tests/s94-settings-refactor.spec.ts` — 新增 5 个 E2E 测试（TC-UI-SETTINGS-001/002/003/004/006）
- `e2e-tests/tests/unit/s94-settings-refactor.test.js` — 新增 4 个 Vitest 单元测试
- `docs/sessions/CURRENT_STATE.md` — 状态更新

## 测试基线

- Rust Free: 1552 passed, 0 failed, 23 ignored
- Rust Pro: 1857 passed, 0 failed, 49 ignored
- Clippy Free+Pro: 零警告
- cargo fmt: 通过
- tsc: 0 errors
- esbuild: 1590 行
- Vitest: 1771 passed (89 files) +4
- 版本号: 2.4.0
- sync-to-github: 3 files committed

## 下个 Session 任务

### S95: 前端模块精简 72→≤60（可并行启动）

**目标**：将前端 72 个 ES module 合并到 ≤60 个，同时移除 Release 构建中的开发者工具模块。

**12 个合并候选**（详见 `docs/sessions/archive/audit_s1_frontend_modules.md`）：
1. `empty-state.js` + `skeleton.js` → `placeholders.js`
2. `caret.js` + `scroll-lock.js` → `dom-utils.js`（caret.js 已删除）
3. `confirm-dialog.js` + `context-menu.js` → `overlay-menus.js`
4. `breadcrumb.js` + `doc-keyboard.js` → `doc-nav.js`
5. `offline-detector.js` + `error-recovery.js` → `network-utils.js`
6. `input-keymap.js` + `input-history.js` → `input-utils.js`
7. `workspace.js` + `kb-stats.js` → `workspace-utils.js`
8. `global-search.js` + `command-palette.js` → `search-ui.js`
9. `graph-renderer.js` + `graph-viewer.js` → `graph-viewer.js`（v1.21 已拆分，可合并回来）
10. `date-utils.js` + `utils.js` → `utils.js`（date-utils 已合并）
11. `about-panel.js` + `keyboard-help.js` → `help-panels.js`
12. `paywall.js` + `wizard.js` → `onboarding.js`

**开发者工具模块移除**（Release 构建）：
- `rag-eval.js` / `trace-panel.js` 已有 `#[cfg(debug_assertions)]` 后端门控
- 前端需在 `build-ui.mjs` 中配置 Release 模式排除这些模块的导入

### S96: 嵌入模型对比评估面板（依赖 S94）

**目标**：复用 `RagEvaluator` + `RagEvalDataset`，新建 `ui/src/embed-eval.js` + `run_embed_comparison` IPC。

### S97: MCP 协议 Phase 1（可并行启动）

**目标**：`crates/core/src/mcp/` 新建模块，stdio + SSE 双 transport，6 IPC 命令。

### S98: Pro ROI 评估 + v3.0 发布（最后执行）

**目标**：14 个 Pro 功能 ROI 评估 + 版本号升级 2.4.0→3.0.0。

## 执行顺序

```
S94 设置面板重构（✅ 完成）───┐
                              ├─ S96 嵌入模型评估 ──┐
S95 模块精简 ──────────────────┘                      ├─ S98 Pro ROI 评估 + v3.0 发布
S97 MCP 协议 ───────────────────────────────────────┘
```

S95/S97 可并行（无相互依赖）。S96 依赖 S94（已完成）。S98 必须最后执行。

## 关键约束（铁律）

1. 代理仅限开发调研，软件本身禁止代理（`reqwest` 必须加 `.no_proxy()`）
2. TDD 驱动，先写红灯测试再实现；生产代码严禁 `unwrap()`/`expect()`/`panic!()`
3. 代码变更同步更新 `docs/api/` + `docs/SOFTWARE_ASSET.md`
4. 修改源代码后运行 `./sync-to-github.sh --commit`
5. Session 结束更新 `CURRENT_STATE.md` + `LOG.md` + 生成本文件
6. 先写规格再写代码（Spec-driven Development）
7. 前端样式铁律十三：Tailwind utility classes + design tokens，禁止硬编码 hex 颜色
8. `async fn` in trait（Edition 2024），无 `async-trait` 宏
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
| `docs/sessions/archive/audit_s1_frontend_modules.md` | S1 前端模块审计报告（72 模块等级分布 + 合并候选） |
| `docs/sessions/CURRENT_STATE.md` | 当前项目状态快照 |
| `CHANGELOG_v2.4.md` | v2.4.0 发布说明 |

## 注意事项

- S94 后端 IPC `set_smart_mode`/`get_smart_mode` 已在 `crates/tauri-app/src/commands/settings.rs` 中实现（SMART_MODE_SETTINGS 10 项参数联动）
- 前端 `smartModeApi` 已在 `ui/src/ipc.js` 中注册
- `perf-settings.js` 智能模式开关 ON 时通过 `.hidden` CSS class 隐藏全部高级设置子项
- `settings.js` 开发者模式通过 ⌘Shift+D 切换，RAG Eval / Trace 仅在 `_devMode=true` 时渲染
- `input-toggles.js` TOGGLE_CONFIG 仅 2 项（hybrid + agent），移除的 3 个 toggle（sub_agent/memory/web_search）功能移入设置面板
- esbuild 打包：修改 `ui/src/*.js` 后必须运行 `node scripts/build-ui.mjs` 重新打包
- i18n 三语：所有新增文案需同时更新 `ui/locales/en.json` 和 `zh-CN.json`（+ `ja.json` 三语）
