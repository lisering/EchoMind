# Cherry Studio 借鉴方向原子级实施计划

> **状态**: 📋 Planning | **创建日期**: 2026-08-07 | **父文档**: `docs/architecture/CHERRY_STUDIO_ANALYSIS.md`
>
> **版本基线**: v1.3.0（803 Free / 1094 Pro tests, 955 E2E, 152 IPC 命令）
>
> **设计原则**: 借鉴 Cherry Studio 的工程成熟度，结合 EchoMind 的 Rust + Tauri 架构优势，不做简单移植而是适配为 Rust 原生实现。

---

## Session 总览

| Session | 功能 | Cherry Studio 对应模块 | 预计天数 | 优先级 |
|---|---|---|---|---|
| S67 | API Gateway（OpenAI 兼容 HTTP 服务端） | `features/apiGateway/` | 3 | P0 |
| S68 | 多 Provider 支持（Anthropic + Gemini） | `ai/provider/` | 2 | P0 |
| S69 | 上下文压缩增强（token-budget 驱动 in-loop compaction） | `ai/streamManager/context/compaction.ts` | 2 | P1 |
| S70 | 轻量 Trace 系统（RAG 链路 span 级追踪） | `ai/observability/` | 2 | P1 |
| S71 | 文件处理器注册表模式 | `features/fileProcessing/processors/` | 1 | P1 |
| S72 | MCP 客户端（HTTP/SSE 桥接） | `ai/mcp/` | 3 | P0 |

总计：6 个 Session，预计 13 天。

---

## S67: API Gateway（OpenAI 兼容 HTTP 服务端）

### 目标
在 EchoMind 内嵌轻量 HTTP 服务器，将 RAG 能力暴露为 OpenAI 兼容 API，使其他应用（Obsidian 插件、VSCode 扩展、curl）可以调用 EchoMind 的知识库检索 + LLM 对话。

### 架构设计

```
crates/tauri-app/src/api_gateway/
├── mod.rs              # 模块根 + Server 生命周期管理
├── server.rs           # Axum 服务器启动/停止 + 路由注册
├── auth.rs             # Bearer token 中间件
├── types.rs            # OpenAI 兼容请求/响应类型
├── routes/
│   ├── mod.rs          # 路由聚合
│   ├── chat.rs         # POST /v1/chat/completions（SSE 流式 + RAG）
│   ├── models.rs       # GET /v1/models
│   └── kb.rs           # POST /v1/kb/search + GET /v1/kb/documents
└── tests.rs            # 单元测试
```

### 新增依赖
- `axum = "0.7"` — HTTP 服务器框架
- `tower-http = { version = "0.6", features = ["cors"] }` — CORS 中间件

### 新增 IPC 命令（3 个）
- `start_api_gateway(port: u16, token: String) -> ()`
- `stop_api_gateway() -> ()`
- `get_api_gateway_status() -> ApiGatewayStatus`

### 新增设置键
- `api_gateway.enabled`（bool，默认 false）
- `api_gateway.port`（u16，默认 8080）
- `api_gateway.token`（String，Bearer 认证令牌）

### API 端点

| 端点 | 方法 | 说明 |
|---|---|---|
| `/v1/chat/completions` | POST | OpenAI 兼容流式对话，可选 `knowledge_base: true` 启用 RAG |
| `/v1/models` | GET | 返回可用模型列表（EchoMind 配置的 LLM 模型 + 本地模型） |
| `/v1/kb/search` | POST | 知识库向量+关键词混合搜索 |
| `/v1/kb/documents` | GET | 知识库文档列表 |
| `/health` | GET | 健康检查 |

### 验收标准
- [ ] axum HTTP 服务器可在指定端口启动/停止
- [ ] Bearer token 认证生效（无 token / 错误 token → 401）
- [ ] CORS 头正确返回
- [ ] `/v1/chat/completions` 支持 stream=true（SSE）和 stream=false（JSON）
- [ ] `/v1/chat/completions` 可选 RAG 检索（请求体 `knowledge_base: true`）
- [ ] `/v1/models` 返回 OpenAI 兼容模型列表
- [ ] `/v1/kb/search` 返回检索结果
- [ ] 单元测试 ≥ 10 个
- [ ] Clippy 零警告 + fmt 通过
- [ ] 文档同步（SRS / IPC / code-contracts / settings-reference / frontend-modules）

---

## S68: 多 Provider 支持（Anthropic + Gemini）

### 目标
扩展 LLMProvider 实现，新增 Anthropic Claude 和 Google Gemini 原生 API 支持。

### 架构设计
- `crates/infra/src/anthropic_provider.rs` — Claude Messages API SSE 流式
- `crates/infra/src/gemini_provider.rs` — Gemini generateContent API SSE 流式
- 两者均实现 `LLMProvider` trait

### 验收标准
- [ ] AnthropicProvider 支持 Claude 3.5/4 系列，SSE 流式
- [ ] GeminiProvider 支持 Gemini 2.0/1.5 系列，SSE 流式
- [ ] 前端 LLM 配置新增 provider 类型选择（openai / anthropic / gemini / local）
- [ ] 单元测试 ≥ 8 个
- [ ] 文档同步

---

## S69: 上下文压缩增强（token-budget 驱动）

### 目标
借鉴 Cherry Studio 的 token-budget 驱动 in-loop compaction，增强 EchoMind 的 CompactionEngine。

### 架构设计
- 在 `crates/compact/src/lib.rs` 新增 `TokenBudgetCompactor`
- 基于 tiktoken-rs token 估算，从尾部累积 token，超出预算时折叠历史
- 在 Agent 循环中动态压缩上下文

### 验收标准
- [ ] token 估算精度 ≥ 90%（vs 实际 API token 计数）
- [ ] 超预算时自动折叠最旧消息，注入 summary
- [ ] Agent 循环中 in-loop compaction 生效
- [ ] 单元测试 ≥ 8 个

---

## S70: 轻量 Trace 系统

### 目标
为 RAG 检索链路添加 span 级追踪，在设置面板可视化展示每步耗时。

### 架构设计
- `crates/core/src/trace.rs` — TraceSpan + TraceCollector
- 记录：嵌入耗时 → 向量搜索 → BM25 → RRF → 重排 → LLM 首 token
- 前端 trace 面板可视化

### 验收标准
- [ ] 每次 RAG 查询生成完整 TraceSpan 链
- [ ] 前端设置面板可查看最近 20 次 trace
- [ ] 单元测试 ≥ 6 个

---

## S71: 文件处理器注册表模式

### 目标
将 Loader trait 改为注册表模式，支持运行时注册新文件类型处理器。

### 架构设计
- `crates/core/src/loader_registry.rs` — LoaderRegistry
- 现有 Loader 实现注册到 registry
- 支持动态注册自定义 Loader

### 验收标准
- [ ] LoaderRegistry 支持按扩展名查找 Loader
- [ ] 现有所有 Loader 迁移到注册表
- [ ] 单元测试 ≥ 6 个

---

## S72: MCP 客户端（HTTP/SSE 桥接）

### 目标
支持连接外部 MCP 服务器（HTTP/SSE 传输），将 MCP 工具暴露给 Agent 使用。

### 架构设计
- `crates/core/src/mcp_client.rs` — MCP 客户端 trait
- `crates/infra/src/mcp_http_client.rs` — HTTP/SSE 传输实现
- Agent 可调用 MCP 工具作为 Action

### 验收标准
- [ ] 支持连接 HTTP/SSE MCP 服务器
- [ ] 工具发现 + 工具调用
- [ ] Agent 集成（MCP 工具作为可选 Action）
- [ ] 单元测试 ≥ 8 个
