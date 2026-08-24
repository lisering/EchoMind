# 复盘 S3 提示词 — 性能基准 + 安全审计 + 文档治理 + 测试策略

## 一、项目背景

EchoMind — Rust + Tauri v2 桌面端本地 RAG 知识库。面向律师、研究者等高隐私需求用户。

技术栈: Rust Edition 2024 / Tauri v2 / SQLite WAL + FTS5 / fastembed ONNX (all-MiniLM-L6-v2, 384-dim) / OpenAI 兼容 LLM / vanilla JS + esbuild 前端
架构: 六边形（ports & adapters），8 crate: models → prompt → compact → context → core → infra → tauri-app，license-issuer 独立
版本: v1.4.0 已发布。当前处于功能迭代暂停 → 深度复盘阶段（3-Session 复盘计划的最后一个 Session）
代码规模: Rust ~98K 行（8 crates）/ 前端 ~25K 行（58 JS 模块）/ 2,410 个测试
SRS: 248 个 REQ（100 Done + 96 Implemented + 40 Draft + 12 Deprecated），S2 已将 12 个 Obsolete Draft 标记为 Deprecated

## 二、复盘 S1 + S2 已完成回顾

### 2.1 S1 审计结果

| 审计项 | 结果 | 状态 |
|---|---|---|
| unwrap/expect 审查 | 24 处，全部安全（编译时正则 + doc comments） | ✅ |
| 依赖方向检查 | 零违反（六边形架构严格） | ✅ |
| IPC 僵尸命令 | 55 个（28.4% 僵尸率） | 🔴 |
| 借鉴消化度 | 40%（12/37 有 UI 入口） | 🔴 |
| 架构重构候选 | sqlite_storage.rs 4925 行、features.rs 2138 行 | 🟡 |
| 测试统计 | 800 Rust + 1188 E2E + 422 Vitest = 2410 | ✅ |

### 2.2 S2 完成结果

| 指标 | S1 基线 | S2 完成后 | 状态 |
|---|---|---|---|
| 僵尸命令数 | 55 | 16 | ✅ <20 |
| 借鉴消化度 | 40% | 78% | ✅ >75% |
| Draft REQ 分类 | 0/52 | 52/52 | ✅ 全部完成 |
| 核心路径验证 | 未执行 | 5/5 闭环无断裂 | ✅ |
| tsc 错误 | - | 0 | ✅ |
| esbuild 构建 | 13688 行 | 15018 行 | ✅ |

**S2 产出**: 39 个僵尸命令接线（13 安全 + 8 Trace/Budget + 6 记忆 + 8 Durable/Todo + 4 已有 Perf），52 个 Draft REQ 全部分类（12 Obsolete→Deprecated + 25 Needed→v1.5 候选 + 15 Deferred→P3），5 条核心交互路径（导入/检索/对话/导出/安全）全部验证无断裂点。

### 2.3 剩余 16 个未接线僵尸命令

| 分类 | 命令 | 原因 |
|---|---|---|
| Pro LLM (10) | `save_kv_cache`, `load_kv_cache`, `clear_kv_cache`, `get_kv_cache_status`, `set_kv_cache_enabled`, `set_kernel_mode`, `get_kernel_mode`, `get_local_llm_device_kind`, `set_paged_attn`, `set_embedder_model` | Pro 门控功能，需在 `settings-local-llm.js` 中增强 |
| 其他 (6) | `get_conversations_paginated`, `get_messages_paginated`, `get_graph_layout`, `get_symbols_for_chunk`, `reclassify_document`, `web_search` | 已有非分页版本在用 / 特定面板辅助 API / IPC 封装已添加但无 UI 按钮入口 |

### 2.4 S2 文件变更清单

**新建文件**: `ui/src/trace-panel.js`（~350 行 Trace+Budget 面板）、`ui/src/durable-panel.js`（~280 行 Durable+Todo 面板）
**修改文件**: `ui/src/ipc.js`（新增 5 个 API 对象 36 方法）、`ui/src/security.js`（+280 行 7 个新 UI 区域）、`ui/src/memory-panel.js`（+120 行 pin/整合/Scratch/Burst）、`ui/src/settings.js`（渲染 trace+durable 面板）、`ui/src/state.js`（新增 clipboard 字段）、`ui/locales/en.json` + `zh-CN.json`（trace/durable/todo 键值）、`e2e-tests/tests/bridge/tauri-stub.js`（6 mock + state 字段）

## 三、本 Session 任务（复盘 S3）

根据 `docs/sessions/RETROSPECTIVE_PLAN.md` 第三个 Session 框架，本 Session 聚焦 **性能 + 安全 + 文档 + 测试策略**。

### 任务 1（P0）：性能基准量化测量

对以下 4 个维度进行代码级性能分析（不需运行 app，通过审查代码逻辑评估）：

#### 1.1 冷启动性能
- 审查 `crates/tauri-app/src/lib.rs` 的 `main()` / `run()` 启动流程
- 审查 `state.rs` 的 `AppState::new()` 初始化路径
- 识别阻塞性操作（DB 连接、模型加载、索引恢复）
- 评估 `init_embedder_with_progress` 的延迟初始化效果
- 产出: 冷启动关键路径分析 + 优化建议

#### 1.2 检索延迟
- 审查 `crates/core/src/retrieval.rs` 的检索管线
- 审查 `crates/infra/src/sqlite_storage.rs` 的 `vector_search()` 实现
- 分析向量缓存机制（`vector_cache` RwLock + 首次全量加载）
- 评估 BM25 混合搜索 + RRF 融合 + Reranker 的累积延迟
- 产出: 检索延迟拆解表 + 瓶颈识别

#### 1.3 导入速度
- 审查 `crates/core/src/import.rs` 的 `import_one()` 管线
- 分析文件解析 → 分块 → 嵌入 → 存储的各阶段耗时占比
- 评估嵌入并行池（`embed_batch` parallel pool）的并发度
- 产出: 导入速度瓶颈分析 + 优化建议

#### 1.4 内存占用
- 审查 `AppState` 全部字段的内存占用估算
- 审查 `vector_cache` 全量加载的内存消耗
- 评估大型知识库（10K+ chunks）的内存压力
- 产出: 内存占用估算表 + 优化建议

### 任务 2（P1）：安全审计验证

对以下 5 个安全功能进行代码级审查：

#### 2.1 SQLCipher 加密
- 审查 `crates/infra/src/sqlite_storage.rs` 中的 `PRAGMA key` 设置
- 验证 Argon2id 密钥派生参数（`crates/core/src/encryption.rs`）
- 确认 `SecureString` 密码擦除机制
- 检查加密状态机转换（Unencrypted → EncryptedUnlocked → Locked）

#### 2.2 PII 检测与脱敏
- 审查 `crates/core/src/privacy.rs` 的 8 类 PII 正则表达式
- 验证 `redact_text()` 脱敏逻辑
- 检查 PII 检测开关的前后端一致性

#### 2.3 审计日志哈希链
- 审查 `crates/core/src/privacy.rs` 的 `AuditLogger` 实现
- 验证 `SHA256(prev_hash || entry_data)` 链式哈希
- 确认 `clear_audit_logs` 的安全边界

#### 2.4 紧急销毁
- 审查 `crates/core/src/security.rs` 的 `set_panic_wipe_password` / `is_panic_wipe_enabled`
- 验证密码输入后立即永久删除所有数据的完整路径
- 确认不可撤销性

#### 2.5 暴力破解防护
- 审查 `SecurityManager` 的 5 次尝试限制 + 指数退避
- 验证 `remaining_attempts` / `remaining_lock_seconds` 前端同步

### 任务 3（P1）：文档同步率审计

逐条对比 API 文档与代码实现：

#### 3.1 IPC 命令文档同步
- 对比 `docs/api/ipc-commands.md` 与 `crates/tauri-app/src/lib.rs` 的 `generate_handler!` 注册列表
- 标记缺失文档的命令 + 文档中已废弃但未删除的命令
- S2 新增的 39 个接线命令需补充文档

#### 3.2 设置键文档同步
- 对比 `docs/api/settings-reference.md` 与 `crates/models/src/lib.rs` 的 `SettingsPayload`
- 标记缺失的设置键 + 类型不匹配

#### 3.3 数据库 Schema 文档同步
- 对比 `docs/api/database-schema.md` 与 `crates/infra/src/sqlite_storage.rs` 的 `KNOWN_TABLES`
- 验证 S2 后是否有新增表或列

#### 3.4 前端模块文档同步
- 对比 `docs/api/frontend-modules.md` 与 `ui/src/` 目录实际文件
- S2 新增 `trace-panel.js` 和 `durable-panel.js` 需补充

### 任务 4（P2）：测试覆盖空白分析

#### 4.1 关键路径覆盖
- 审查 5 条核心交互路径（S2 已验证）的测试覆盖情况
- 导入路径：是否有测试覆盖各文件格式 + 重复检测 + 崩溃恢复？
- 检索路径：是否有测试覆盖向量搜索 + BM25 + RRF + Reranker？
- 对话路径：是否有测试覆盖 SSE 流式 + 中断 + 持久化？
- 产出: 覆盖空白清单 + 补测优先级

#### 4.2 S2 新功能测试评估
- `trace-panel.js` — 是否需要 E2E 测试？
- `durable-panel.js` — 是否需要 E2E 测试？
- `security.js` 新增的 7 个区域 — 是否需要 Vitest 单元测试？

#### 4.3 测试质量抽样
- 抽查 20 个现有测试的断言质量
- 识别"凑数测试"（断言过于宽松或仅验证不 panic）

### 任务 5（P2）：剩余 16 个僵尸命令评估

评估剩余 16 个僵尸命令的处理方案：

- **Pro LLM (10 个)**：是否值得在 `settings-local-llm.js` 中增加 KV Cache 管理 UI + Kernel 模式选择 + 设备类型显示？评估 ROI
- **其他 (6 个)**：`reclassify_document` IPC 封装已添加，是否需要在文档右键菜单中增加"重分类"按钮？`web_search` 命令是否需要从聊天输入区调用？

产出: 每个命令的处置决策（接线 / 保留为 API-only / 标记为 Deprecated）

## 四、关键文件索引

### 后端

| 文件 | 用途 |
|---|---|
| `crates/tauri-app/src/lib.rs` | 194 IPC 命令注册（generate_handler!），第 101-358 行 |
| `crates/tauri-app/src/commands/security.rs` | 安全 IPC 命令实现（含 S2 接线的 13 个命令） |
| `crates/tauri-app/src/commands/trace.rs` | Trace 系统命令（S2 已接线） |
| `crates/tauri-app/src/commands/settings.rs` | Token 预算配置 + Budget 统计（S2 已接线） |
| `crates/tauri-app/src/commands/features.rs` | 记忆/Burst Buffer/Scratch 日志命令（S2 已接线） |
| `crates/tauri-app/src/state.rs` | AppState 全部字段（含 trace_store/shadow_screen_collector） |
| `crates/core/src/lib.rs` | 16 个 port trait 定义 |
| `crates/core/src/encryption.rs` | Argon2id 密钥派生 + PasswordStrengthChecker |
| `crates/core/src/security.rs` | SecurityManager + SecurityPosture + AutoLockConfig |
| `crates/core/src/privacy.rs` | PiiRedactor + AuditLogger + 8 类 PII 正则 |
| `crates/core/src/retrieval.rs` | 检索管线（向量搜索 + BM25 + RRF + Reranker） |
| `crates/core/src/import.rs` | 导入管线（Loader → Splitter → Embedder → Storage） |
| `crates/infra/src/sqlite_storage.rs` | SQLite WAL + FTS5 + SQLCipher（4925 行，最大单文件） |

### 前端

| 文件 | 用途 |
|---|---|
| `ui/src/ipc.js` | 所有 IPC 调用封装（S2 新增 traceApi/budgetApi/memoryExtApi/durableApi/todoApi/docExtApi） |
| `ui/src/security.js` | 安全面板（S2 增强 +280 行：7 个新区域 + 紧急销毁对话框） |
| `ui/src/trace-panel.js` | S2 新建：RAG 链路追踪 + Token 预算面板 |
| `ui/src/durable-panel.js` | S2 新建：Durable 输入队列 + Session Todo 面板 |
| `ui/src/memory-panel.js` | 记忆面板（S2 增强 +120 行：pin/整合/Scratch/Burst Buffer） |
| `ui/src/perf-settings.js` | 性能优化设置面板（含 4 个索引重建按钮，S1 前已接线） |
| `ui/src/settings.js` | 设置面板主入口（S2 新增渲染 trace + durable 面板） |
| `ui/src/state.js` | EchoState 全局状态对象（S2 新增 clipboardClearEnabled/Timeout） |
| `ui/src/main.js` | 前端主入口，1325 行，初始化 + 事件绑定 |
| `ui/index.html` | 构建产物（`node scripts/build-ui.mjs` 生成，不要手改） |

### 测试

| 文件 | 用途 |
|---|---|
| `e2e-tests/tests/bridge/tauri-stub.js` | Mock window.__TAURI__（S2 新增 6 mock + state 字段） |
| `e2e-tests/tests/*.spec.ts` | 1188 个 E2E 测试 |

### 文档

| 文件 | 用途 |
|---|---|
| `docs/SRS_v1.0.md` | 需求规格唯一依据（248 REQ，S2 已分类 52 Draft） |
| `docs/api/ipc-commands.md` | IPC 命令文档（S3 需同步） |
| `docs/api/settings-reference.md` | 设置键文档（S3 需同步） |
| `docs/api/database-schema.md` | 数据库 Schema 文档（S3 需同步） |
| `docs/api/frontend-modules.md` | 前端模块文档（S3 需同步） |
| `docs/sessions/RETROSPECTIVE_PLAN.md` | 3-Session 复盘框架 |
| `docs/sessions/RETROSPECTIVE_S1_REPORT.md` | S1 完整审计报告 |
| `docs/sessions/RETROSPECTIVE_S2_REPORT.md` | S2 完整报告（S2 产出） |
| `docs/sessions/CURRENT_STATE.md` | 项目状态快照（S2 已更新） |
| `docs/sessions/LOG.md` | 会话时间线日志（S2 已追加） |

## 五、前端开发规范

- **编辑源码**: 只修改 `ui/src/*.js` / `ui/styles/*.css` / `ui/locales/*.json`，不要手改 `ui/index.html`
- **构建**: `node scripts/build-ui.mjs`（esbuild 打包 IIFE + 内联 CSS 单文件）
- **类型检查**: `npx tsc --noEmit`（根目录 `tsconfig.json` + `types/*.d.ts`）
- **样式铁律**: 使用 Tailwind utility classes + design tokens（`bg-surface-2`, `text-text-primary`, `border-line`, `text-accent` 等映射自 `ui/styles/tokens.css` CSS 变量）。禁止硬编码 hex 颜色。自定义 CSS 仅用于 `@keyframes` / 伪元素 / parent-state selectors / 第三方覆盖
- **面板管理**: 新面板用 `panel-stack.js` 的 `push()`/`remove()`，ESC 键统一关闭栈顶
- **Z-index**: 使用 `zindex.js` 的 `zClass()` 函数，禁止硬编码 z-index
- **i18n**: 所有用户可见文案在 `ui/locales/{en,zh-CN}.json` 中添加对应键
- **IPC 封装**: 在 `ui/src/ipc.js` 中添加封装函数，前端模块通过 `import { invoke } from './ipc.js'` 调用

## 六、铁律提醒

| 铁律 | 内容 |
|---|---|
| 一 | 代理仅限开发调研，软件运行时禁止代理（`reqwest::Client::builder()` 必须加 `.no_proxy()`） |
| 二 | TDD 驱动，先写红灯测试再实现 |
| 五 | 零 `unwrap`/`expect`/`panic`/`unreachable`/`todo`/`unimplemented`（workspace deny lint 强制） |
| 七 | 文档实时同步（代码变更 → API 文档同步） |
| 十一 | `./sync-to-github.sh` 会话结束前执行 |
| 十二 | Session 上下文全自动管理（更新 `CURRENT_STATE.md` + `LOG.md`） |
| 十三 | Tailwind 样式规范（使用 design tokens，禁止硬编码 hex） |

## 七、验证标准

本 Session 结束时需满足：

1. 4 个性能维度（冷启动/检索延迟/导入速度/内存）全部有量化分析 + 优化建议
2. 5 个安全功能（SQLCipher/PII/审计链/紧急销毁/暴力破解）全部有代码级审查报告
3. 4 类 API 文档（IPC 命令/设置键/数据库 Schema/前端模块）同步率 >95%
4. 测试覆盖空白清单完成 + 补测优先级排序
5. 16 个剩余僵尸命令全部有处置决策
6. `cargo clippy --all-targets -- -D warnings` 零警告
7. `npx tsc --noEmit` 零错误
8. `node scripts/build-ui.mjs` 成功
9. `docs/sessions/RETROSPECTIVE_S3_REPORT.md` 生成
10. `docs/sessions/CURRENT_STATE.md` + `LOG.md` 更新
11. `./sync-to-github.sh` 执行

## 八、常用命令速查

```bash
cargo build                              # 编译
cargo clippy --all-targets -- -D warnings # Lint
cargo fmt --check                        # 格式检查
npx tsc --noEmit                         # 前端类型检查
node scripts/build-ui.mjs                # 前端构建
cd e2e-tests && npm run test:bridge      # E2E 桥接测试
cd e2e-tests && npm run test:unit        # Vitest 单元测试
./sync-to-github.sh                      # GitHub 发布同步
```

## 九、3-Session 复盘框架全景

| Session | 主题 | 状态 |
|---|---|---|
| S1 | 代码审计 + 架构健康度 | ✅ 已完成（2026-08-10） |
| S2 | 功能审计 + 借鉴消化度提升 | ✅ 已完成（2026-08-10） |
| S3 | 性能 + 安全 + 文档 + 测试策略 | 📋 本 Session |

| # | 维度 | 归属 Session | 状态 |
|---|---|---|---|
| 1 | 业务定位 | S2 | ✅ |
| 2 | 功能完整度 | S2 | ✅ |
| 3 | 代码质量 | S1 | ✅ |
| 4 | 性能 | S3 | 📋 本 Session |
| 5 | UI/交互 | S2 | ✅ |
| 6 | 借鉴消化度 | S2 | ✅ |
| 7 | 架构健康度 | S1 | ✅ |
| 8 | 测试策略 | S1+S3 | 📋 本 Session |
| 9 | 文档治理 | S3 | 📋 本 Session |
| 10 | 安全性 | S3 | 📋 本 Session |
| 11 | 可维护性 | S1+S2 | ✅ |
