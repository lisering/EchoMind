# EchoMind v3.0.0 — 架构与生态

**发布日期**：2026-08-24  
**版本号**：2.4.0 → 3.0.0（8 crate + tauri.conf.json）

## 版本主题：架构与生态

v3.0 是 EchoMind 的**架构升级版本**，聚焦三个方向：
1. **架构级突破**：MCP 协议支持（P0-6 插件系统 Phase 1），打开生态扩展能力
2. **用户体验根本改善**：设置面板重构（35→8 分区 + 智能模式 + 术语通俗化）
3. **技术债清理**：前端模块精简（72→56）+ 开发者工具隔离 + Pro ROI 评估

v2.x 系列已将 RAG 核心能力（检索/嵌入/对话/安全/性能/多模态/本地 LLM）打磨到生产级。v3.0 转向**生态扩展**和**用户体验根本性改善**，为 v4.0 多端协同/云同步奠定架构基础。

## 新功能

### 1. REQ-UI-014 设置面板信息架构重构（S94）

将设置面板从 35 个分区精简为 8 个：① 模型配置 ② 外观语言 ③ 知识库 ④ 安全隐私 ⑤ 智能检索 ⑥ 高级调优 ⑦ 应用设置 ⑧ 快捷指令。实现「智能模式」单一开关（默认开启混合搜索 + Rerank + HyDE + 知识图谱 + 质量门控 + 压缩 2x），将 20 个术语化设置项通俗化，4 个开发者工具移入隐藏的「开发者菜单」。输入区 toggle 从 5 个精简为 2 个（保留混合搜索 + Agent 模式）。

- **前端**：`ui/src/settings.js` + `ui/src/settings-general.js` + `ui/src/settings-advanced.js` + `ui/src/input-toggles.js` + `ui/src/perf-settings.js`
- **设计**：智能模式一键控制 6 项 RAG 优化参数；关闭时展开全部设置
- **测试**：10 个测试（6 E2E + 4 Vitest）
- **零新增依赖**：纯前端重构

### 2. REQ-ARCH-015 前端模块高内聚合并（S95）

将 72 个前端 JS 模块精简至 56 个。合并策略：功能内聚的相邻模块合并，减少模块碎片化。同时将开发者工具模块（trace-panel / rag-eval）在 Release 构建中完全移除（esbuild exclude 插件）。

- **合并候选**：scroll-lock → chat / queue-send → chat / regen-carousel → chat-render / doc-keyboard → sidebar / offline-detector → utils / breadcrumb → sidebar / kb-stats → sidebar / doc-preview → sidebar / icons → utils / ime-guard → input-keymap / error-recovery → error-detail / update-banner → main / caret → chat-render / command-palette → main / followup → chat-render / guards → chat / paywall → chat / privacy-indicator → chat / retrieval-feedback → chat / session-strip → chat / workflow-panel → features / code-executor → features / durable-panel → features / zindex → panel-stack / input-keymap → chat / input-history → chat
- **测试**：4 个测试（模块合并 + 构建验证 + 测试回归 + Release 隔离）
- **零新增依赖**

### 3. REQ-VEC-018 嵌入模型对比评估面板（S96）

用户可选择 2-3 个嵌入模型，对同一文档集进行嵌入并对比检索质量（Hit Rate / MRR / NDCG）。评估结果以表格 + 柱状图展示。复用已有的 RAG 评估数据集和评估指标。

- **后端**：`crates/core/src/embed_comparison.rs` + `run_embed_comparison` IPC（`#[cfg(debug_assertions)]` 门控）
- **前端**：`ui/src/embed-eval.js` 前端面板（模型多选 + 数据集选择 + 表格 + SVG 柱状图 + 进度）
- **数据模型**：`EmbedComparisonRequest` / `EmbedMetricScores` / `EmbedComparisonResult`
- **测试**：7 TDD 测试 TC-VEC-EVAL-001~007 + 15 Vitest 测试 TC-VEC-EVAL-UI-001~015
- **零新增依赖**：复用已有 RAG 评估框架

### 4. REQ-ARCH-016 MCP 协议适配器 Phase 1（S97）

实现 Model Context Protocol (MCP) 客户端支持，允许 EchoMind 连接外部 MCP 服务器（如文件系统、数据库、Web 搜索等工具服务器）。Phase 1 聚焦：① MCP 客户端 JSON-RPC 通信（stdio + SSE 两种 transport）② 工具注册表（MCP 工具自动注册为 Agent 可用工具）③ 安全沙箱（MCP 工具调用需用户确认）。

- **核心模块**：`crates/core/src/mcp/` — `client.rs` JSON-RPC 2.0 客户端 + `transport.rs` 传输层 trait + `registry.rs` 工具注册表 + `config.rs` 服务器配置
- **传输层**：`crates/infra/src/mcp_transport.rs` — `StdioTransport`（子进程 stdin/stdout + 30s 超时）+ `SseTransport`（HTTP POST + SSE 解析 + `.no_proxy()` 铁律一合规）
- **前端**：`ui/src/mcp-settings.js` MCP 服务器配置面板
- **IPC 命令**：6 个（`add_mcp_server` / `remove_mcp_server` / `list_mcp_servers` / `toggle_mcp_server` / `get_mcp_tools` / `call_mcp_tool`）
- **自动连接**：`add_mcp_server` 和 `toggle_mcp_server`（启用时）自动调用 `try_connect_server()` 建立连接
- **安全**：MCP 工具调用前弹出确认对话框，用户可允许/拒绝
- **测试**：23 MCP 测试（core 14 + infra 9）+ 15 Vitest 测试 TC-MCP-UI-001~015
- **零新增 crate 依赖**：MCP JSON-RPC 协议用已有 tokio + serde_json 实现

### 5. P0-3 Pro 功能 ROI 评估（S98）

对 14 个 Pro 功能逐一评估用户需求度 × 技术维护成本 × 差异化价值。

- **产出**：`docs/architecture/PRO_ROI_EVALUATION.md`
- **决策**：10 个保留 Pro，4 个考虑后续下沉（GEMV/ColBERT/ONNX压缩器/WasmExecutor），0 个 Deprecated
- **核心发现**：本地 LLM 推理 + PDF 多模态是 Pro 双核心（ROI 4.1 并列第一）
- **P0-3 状态**：✅ 已评估

## 验证结果

| 指标 | v2.4.0 | v3.0.0 | 变化 |
|---|---|---|---|
| Rust Free | 1552 passed | 1700 passed | +148 |
| Rust Pro | 1857 passed | 1991 passed | +134 |
| Clippy Free+Pro | 零警告 | 零警告 | ✅ |
| cargo fmt | 通过 | 通过 | ✅ |
| tsc | 0 errors | 0 errors | ✅ |
| esbuild | 1641 行 | 1816 行 (prod) / 1471 行 (release) | +175 |
| Vitest | 1767 passed | 1821 passed | +54 |
| Release build | exit_code 0 (12.3m) | exit_code 0 (23.3m) | ✅ |

## 新增 IPC 命令（+9 Free）

| IPC 命令 | Session | 说明 |
|---|---|---|
| `set_smart_mode` | S94 | 智能模式开关 |
| `get_smart_mode` | S94 | 获取智能模式状态 |
| `run_embed_comparison` | S96 | 嵌入模型对比评估（`#[cfg(debug_assertions)]` 门控） |
| `add_mcp_server` | S97 | 添加 MCP 服务器配置 |
| `remove_mcp_server` | S97 | 删除 MCP 服务器 |
| `list_mcp_servers` | S97 | 列出所有 MCP 服务器 |
| `toggle_mcp_server` | S97 | 启用/禁用 MCP 服务器 |
| `get_mcp_tools` | S97 | 获取已连接 MCP 服务器的工具列表 |
| `call_mcp_tool` | S97 | 调用 MCP 工具（需用户确认） |

## 新增设置键

| 设置键 | 类型 | 默认值 | 说明 |
|---|---|---|---|
| `ui.smart_mode` | Bool | `true` | 智能模式开关（一键控制 6 项 RAG 优化） |
| `mcp.servers` | String (JSON) | `"[]"` | MCP 服务器配置列表 |

## 新增数据库表

**无新增表**。MCP 服务器配置存储在 settings 表（JSON 格式）。

## 新增前端模块

| 模块文件 | Session | 说明 |
|---|---|---|
| `ui/src/embed-eval.js` | S96 | 嵌入模型对比评估面板 |
| `ui/src/mcp-settings.js` | S97 | MCP 服务器配置面板 |

## P0 待办状态更新

| P0 编号 | 来源 | 问题 | v3.0 状态 |
|---|---|---|---|
| P0-1 (R1) | 嵌入模型升级 | bge-m3 已完成 | ✅ 已修复 |
| P0-2 (R1) | Prompt 注入防护 | 三层防御已实现 | ✅ 已修复 |
| P0-3 (R2) | Pro 功能 ROI | S98 评估完成 | ✅ 已评估 |
| P0-4 (R2) | 持续收入模型 | 商业层面 | 📋 待产品决策 |
| P0-5 (R3) | 数据库回滚 | 迁移前备份已实现 | ✅ 已修复 |
| P0-6 (R3) | 插件系统 | S97 MCP Phase 1 | ✅ Phase 1 完成 |

**v3.0 修复 3/6 P0**（P0-3 评估 + P0-6 实施 + P0-1 已修复）。剩余 P0-4 为商业层面。

## 版本亮点

1. **MCP 协议支持**：EchoMind 成为 MCP 生态客户端，可连接任意 MCP 服务器扩展工具能力
2. **设置面板根本性改善**：35→8 分区 + 智能模式一键优化 + 术语通俗化，从「开发者工具」升级为「消费者产品」
3. **前端架构精简**：72→56 模块，减少模块碎片化，提升加载性能
4. **嵌入模型对比评估**：用户可数据驱动选择最佳嵌入模型
5. **Pro ROI 评估**：14 个 Pro 功能全面评估，明确核心差异化价值

## 修改文件清单

### Rust 后端
- `crates/core/src/mcp/` — 新建 MCP 模块（4 子模块：client / transport / config / registry）
- `crates/core/src/embed_comparison.rs` — 嵌入对比评估引擎
- `crates/infra/src/mcp_transport.rs` — StdioTransport + SseTransport
- `crates/tauri-app/src/commands/mcp.rs` — 6 MCP IPC 命令
- `crates/tauri-app/src/commands/performance.rs` — `run_embed_comparison` IPC
- `crates/tauri-app/src/state.rs` — `mcp_registry` 字段
- `crates/tauri-app/src/lib.rs` — 注册 9 个新 IPC 命令

### 前端源码
- `ui/src/mcp-settings.js` — MCP 服务器配置面板（新增）
- `ui/src/embed-eval.js` — 嵌入模型对比评估面板（新增）
- `ui/src/settings.js` — 设置面板 8 分区重构 + 智能模式
- `ui/src/settings-general.js` / `ui/src/settings-advanced.js` — 分区重组织
- `ui/src/input-toggles.js` — 5→2 toggle 精简
- `ui/src/perf-settings.js` — 智能模式开关
- `ui/src/main.js` — MCP + 嵌入评估入口
- `ui/index.html` — 8 分区 Tab + 智能模式开关 + MCP 按钮 + 嵌入评估按钮

### 国际化
- `ui/locales/en.json` — 新增 ~80 键
- `ui/locales/zh-CN.json` — 新增 ~80 键
- `ui/locales/ja.json` — 新增 ~80 键

### 版本号（9 文件）
- 8 crate `Cargo.toml`：2.4.0 → 3.0.0
- `crates/tauri-app/tauri.conf.json`：2.4.0 → 3.0.0

### 文档同步
- `docs/SRS_v1.0.md` — REQ-UI-014 / REQ-ARCH-015 / REQ-VEC-018 / REQ-ARCH-016 全部 Implemented
- `docs/api/ipc-commands.md` — 新增 9 个 IPC 命令
- `docs/api/settings-reference.md` — 新增 2 个设置键
- `docs/api/code-contracts.md` — 新增 MCP trait/struct 签名
- `docs/api/frontend-modules.md` — 新增前端模块
- `docs/architecture/PRO_ROI_EVALUATION.md` — Pro ROI 评估报告
- `docs/architecture/V3.0_ATOMIC_PLAN.md` — S94-S98 状态全部 ✅
