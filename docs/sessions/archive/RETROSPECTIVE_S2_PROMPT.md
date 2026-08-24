你是 EchoMind（灵犀）项目的 AI 开发助手。以下是下个 Session 的完整衔接提示词，请通读后立即开始执行。

---

# 复盘 S2：功能审计 + 借鉴消化度提升

## 一、项目背景

**EchoMind** — Rust + Tauri v2 桌面端本地 RAG 知识库。面向律师、研究者等高隐私需求用户。

- **技术栈**: Rust Edition 2024 / Tauri v2 / SQLite WAL + FTS5 / fastembed ONNX (all-MiniLM-L6-v2, 384-dim) / OpenAI 兼容 LLM / vanilla JS + esbuild 前端
- **架构**: 六边形（ports & adapters），8 crate: `models → prompt → compact → context → core → infra → tauri-app`，`license-issuer` 独立
- **版本**: v1.4.0 已发布。当前处于**功能迭代暂停 → 深度复盘阶段**
- **代码规模**: Rust ~98K 行（8 crates）/ 前端 ~23K 行（57 JS 模块）/ 2,410 个测试
- **SRS**: 248 个 REQ（100 Done + 96 Implemented + 52 Draft + 1 Deprecated），79% 已交付

## 二、复盘 S1 已完成（上一 Session）

### 2.1 审计结果

| 审计项 | 结果 | 状态 |
|---|---|---|
| unwrap/expect 审查 | 24 处，全部安全（编译时正则 + doc comments） | ✅ |
| 依赖方向检查 | 零违反（六边形架构严格） | ✅ |
| IPC 僵尸命令 | **55 个（28.4% 僵尸率）** | 🔴 |
| 借鉴消化度 | **40%（12/37 有 UI 入口）** | 🔴 |
| 架构重构候选 | sqlite_storage.rs 4925 行、features.rs 2138 行 | 🟡 |
| 测试统计 | 800 Rust + 1188 E2E + 422 Vitest = 2410 | ✅ |

### 2.2 55 个僵尸命令完整清单

以下是后端已注册但前端**从未调用**的 IPC 命令，按分类排列：

**安全功能（13 个）**:
`set_panic_wipe_password`, `clear_panic_wipe_password`, `is_panic_wipe_enabled`, `set_auto_lock_config`, `unlock_app`, `record_activity`, `check_password_strength`, `clear_audit_logs`, `set_clipboard_config`, `set_security_posture`, `get_security_posture`, `get_security_screen_stats`, `reset_security_screen_stats`

**Pro LLM 功能（10 个）**:
`save_kv_cache`, `load_kv_cache`, `clear_kv_cache`, `get_kv_cache_status`, `set_kv_cache_enabled`, `set_kernel_mode`, `get_kernel_mode`, `get_local_llm_device_kind`, `set_paged_attn`, `set_embedder_model`

**记忆/QM 功能（6 个）**:
`push_burst_turn`, `flush_memory_burst_buffer`, `get_burst_buffer_status`, `trigger_memory_consolidation`, `get_scratch_logs`, `pin_memory`

**Durable/B08（8 个）**:
`admit_input`, `promote_input`, `get_pending_inputs`, `add_session_todo`, `update_todo_status`, `get_session_todos`, `delete_session_todo`, `delete_session_todos`

**Trace/Budget（8 个）**:
`get_recent_traces`, `get_trace_detail`, `clear_traces`, `get_trace_count`, `get_budget_stats`, `set_budget_limit`, `get_token_budget_config`, `set_token_budget_config`

**性能优化（4 个）**:
`build_summary_tree`, `rebuild_bm25_index`, `rebuild_proposition_index`, `rebuild_contextual_embeddings`

**其他（6 个）**:
`get_conversations_paginated`, `get_messages_paginated`, `get_graph_layout`, `get_symbols_for_chunk`, `reclassify_document`, `web_search`

### 2.3 借鉴消化度矩阵

| 来源 | 借鉴项数 | 后端完成 | 前端接线 | 消化度 |
|---|---|---|---|---|
| DS4 (antirez) | 3 | 3/3 | 0/3 | 33% |
| OpenCode | 14 | 14/14 | 6/14 | 43% |
| OpenMontage | 7 | 7/7 | 3/7 | 43% |
| QuantaMind | 11 | 11/11 | 3/11 | 36% |
| Cherry Studio | 2 | 2/2 | 0/2 | 33% |
| **总计** | **37** | **37/37** | **12/37** | **40%** |

## 三、本 Session 任务

### 任务 1（P0）：批量接线僵尸命令

从 55 个僵尸命令中筛选**用户可见价值最高**的优先接线。按以下 3 批顺序执行：

#### 第一批：安全功能（13 个）

在 `ui/src/security.js` 中增强安全设置面板，接线以下命令：

- `unlock_app` + `record_activity` + `set_auto_lock_config` → 安全设置面板增加自动锁定配置区（超时时间输入 + 开关）
- `check_password_strength` → 加密/解锁对话框中集成密码强度指示器（弱/中/强进度条）
- `set_panic_wipe_password` / `is_panic_wipe_enabled` / `clear_panic_wipe_password` → 安全设置增加紧急销毁区域（设置密码 + 启用/禁用 + 清除）
- `clear_audit_logs` → 审计日志面板增加"清空日志"按钮
- `set_clipboard_config` → 安全设置增加剪贴板自动清除配置（超时时间 + 开关）
- `set_security_posture` / `get_security_posture` → 安全设置增加态势选择器（Dangerous/Auto/Strict 三选一）
- `get_security_screen_stats` / `reset_security_screen_stats` → 安全设置增加 Shadow 筛查统计区域（agree/disagree/unavailable 计数 + 重置按钮）

#### 第二批：Trace + Budget（8 个）

新建 `ui/src/trace-panel.js` 模块：

- `get_recent_traces` / `get_trace_detail` / `clear_traces` / `get_trace_count` → RAG 链路追踪面板（列表 + 详情弹窗 + 清空 + 计数）
- `get_budget_stats` / `set_budget_limit` → Token 预算面板（当前用量 + 上限设置 + 进度条）
- `get_token_budget_config` / `set_token_budget_config` → Token 预算配置（上下文窗口限制 + 预留 token 数）

在 `ui/index.html` 中新增工具栏按钮入口。在 `e2e-tests/tests/bridge/tauri-stub.js` 中确认已有 mock（这些命令在后端实现时已添加 mock）。

#### 第三批：记忆 + 性能（10 个）

- `pin_memory` / `trigger_memory_consolidation` / `get_scratch_logs` → 在 `ui/src/memory-panel.js` 中增加 pin 按钮 + 整合触发按钮 + scratch 日志查看
- `push_burst_turn` / `flush_memory_burst_buffer` / `get_burst_buffer_status` → 记忆面板增加 Burst Buffer 状态显示（待处理条目数 + 手动 flush 按钮）
- `build_summary_tree` / `rebuild_bm25_index` / `rebuild_proposition_index` / `rebuild_contextual_embeddings` → 在 `ui/src/perf-settings.js` 中增加 4 个索引重建按钮（带确认对话框 + 进度提示）

### 任务 2（P1）：52 个 Draft REQ 分类

逐一审查 `docs/SRS_v1.0.md` 中 52 个状态为 Draft 的 REQ，分类为：

- **Obsolete** → 状态改为 Deprecated（加注释说明废弃原因）
- **Needed** → 保留 Draft，加入 v1.5 路线图候选
- **Deferred** → 保留 Draft 但降级优先级 P2/P3

在 SRS 末尾追加 changelog 记录分类结果。

### 任务 3（P2）：功能交互路径验证

手动验证核心闭环（代码级审查即可，不需要运行 app）：
1. 导入：文件选择 → 解析 → 分块 → 嵌入 → 存储 → 事件通知 → 前端刷新
2. 检索：用户输入 → embed → 向量搜索 → 阈值过滤 → chunk expansion → prompt 构建
3. 对话：SSE 流式 → token 事件 → 前端渲染 → 完成持久化
4. 导出：对话选择 → Markdown 构建 → 保存对话框

找出断裂点并记录。

## 四、关键文件索引

### 后端

| 文件 | 用途 |
|---|---|
| `crates/tauri-app/src/lib.rs` | 194 IPC 命令注册（`generate_handler!`），第 101-358 行 |
| `crates/tauri-app/src/commands/mod.rs` | 共享类型/常量/辅助函数 + `pub use` 重导出 |
| `crates/tauri-app/src/commands/security.rs` | 安全 IPC 命令实现 |
| `crates/tauri-app/src/commands/features.rs` | Dream/日志/符号/工作流/代码执行/记忆/Web 搜索/自定义模型/模板/Wiki 链接 |
| `crates/tauri-app/src/commands/performance.rs` | 缓存/压缩/BM25/Proposition/检索记忆 |
| `crates/tauri-app/src/commands/local_llm.rs` | 本地 LLM/模型下载/KV cache |
| `crates/tauri-app/src/state.rs` | AppState 全部字段 |
| `crates/core/src/lib.rs` | 16 个 port trait 定义 |
| `crates/infra/src/sqlite_storage.rs` | SQLite WAL + FTS5 + SQLCipher（4925 行，最大单文件） |

### 前端

| 文件 | 用途 |
|---|---|
| `ui/src/ipc.js` | 所有 IPC 调用封装（invoke/listen/openDialog/saveDialog） |
| `ui/src/security.js` | 安全面板（加密/锁定/审计日志/PII），741 行 |
| `ui/src/settings.js` | 设置面板主入口，1557 行 |
| `ui/src/perf-settings.js` | 性能优化设置面板，404 行 |
| `ui/src/memory-panel.js` | 持久化记忆管理面板 |
| `ui/src/main.js` | 前端主入口，1325 行，初始化 + 事件绑定 |
| `ui/src/state.js` | EchoState 全局状态对象 |
| `ui/src/panel-stack.js` | 全局面板栈管理器（push/remove/closeTop） |
| `ui/src/zindex.js` | Z-index 常量表 + zClass() |
| `ui/src/input-toggles.js` | 快捷 toggle 开关（hybrid/agent/sub_agent/memory/web_search） |
| `ui/index.html` | 构建产物（`node scripts/build-ui.mjs` 生成，**不要手改**） |

### 测试

| 文件 | 用途 |
|---|---|
| `e2e-tests/tests/bridge/tauri-stub.js` | Mock `window.__TAURI__`，130+ mock handler |
| `e2e-tests/tests/*.spec.ts` | 1188 个 E2E 测试 |

### 文档

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据（248 REQ） |
| `docs/api/ipc-commands.md` | IPC 命令文档 |
| `docs/api/settings-reference.md` | 设置键文档 |
| `docs/sessions/RETROSPECTIVE_PLAN.md` | 3-Session 复盘框架 |
| `docs/sessions/RETROSPECTIVE_S1_REPORT.md` | S1 完整审计报告 |

## 五、前端开发规范

1. **编辑源码**: 只修改 `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json`，**不要手改 `ui/index.html`**
2. **构建**: `node scripts/build-ui.mjs`（esbuild 打包 IIFE + 内联 CSS 单文件）
3. **类型检查**: `npx tsc --noEmit`（根目录 tsconfig.json + types/*.d.ts）
4. **样式铁律**: 使用 Tailwind utility classes + design tokens（`bg-surface-2`, `text-text-primary`, `border-line`, `text-accent` 等映射自 `ui/styles/tokens.css` CSS 变量）。禁止硬编码 hex 颜色。自定义 CSS 仅用于 `@keyframes` / 伪元素 / parent-state selectors / 第三方覆盖
5. **面板管理**: 新面板用 `panel-stack.js` 的 `push()`/`remove()`，ESC 键统一关闭栈顶
6. **Z-index**: 使用 `zindex.js` 的 `zClass()` 函数，禁止硬编码 z-index
7. **i18n**: 所有用户可见文案在 `ui/locales/{en,zh-CN}.json` 中添加对应键
8. **IPC 封装**: 在 `ui/src/ipc.js` 中添加封装函数，前端模块通过 `import { invoke } from './ipc.js'` 调用

## 六、铁律提醒

| 铁律 | 内容 |
|---|---|
| 一 | 代理仅限开发调研，软件运行时禁止代理（`reqwest::Client::builder()` 必须加 `.no_proxy()`） |
| 二 | TDD 驱动，先写红灯测试再实现 |
| 五 | 零 unwrap/expect/panic/unreachable/todo/unimplemented（workspace deny lint 强制） |
| 七 | 文档实时同步（代码变更 → API 文档同步） |
| 十一 | `./sync-to-github.sh` 会话结束前执行 |
| 十二 | Session 上下文全自动管理（更新 CURRENT_STATE.md + LOG.md） |
| 十三 | Tailwind 样式规范（使用 design tokens，禁止硬编码 hex） |

## 七、验证标准

本 Session 结束时需满足：

1. 僵尸命令从 55 → <20（至少接线 35 个）
2. 借鉴消化度从 40% → 75%+
3. 52 个 Draft REQ 全部分类完成
4. `cargo clippy --all-targets -- -D warnings` 零警告
5. `npx tsc --noEmit` 零错误
6. `node scripts/build-ui.mjs` 成功
7. `docs/sessions/RETROSPECTIVE_S2_REPORT.md` 生成
8. `docs/sessions/CURRENT_STATE.md` + `LOG.md` 更新
9. `./sync-to-github.sh` 执行

## 八、常用命令速查

```
cargo build                              # 编译
cargo clippy --all-targets -- -D warnings # Lint
cargo fmt --check                        # 格式检查
npx tsc --noEmit                         # 前端类型检查
node scripts/build-ui.mjs                # 前端构建
cd e2e-tests && npm run test:bridge      # E2E 桥接测试
cd e2e-tests && npm run test:unit        # Vitest 单元测试
./sync-to-github.sh                      # GitHub 发布同步
```
