# ADR-006: SSE 流式对话 + reqwest

> **状态**：✅ 已采纳
> **日期**：2026-07-25
> **决策者**：Chief Architect

## 背景

EchoMind 的 RAG 对话需要实时流式输出（类似 ChatGPT 的逐 token 打字效果）。要求：

- **SSE（Server-Sent Events）协议**：OpenAI 兼容 API 的标准流式协议
- **用户可中断**：用户可随时停止生成（CancellationToken）
- **推理内容分离**：DeepSeek R1 等模型的 reasoning_content 需独立通道
- **Token 用量追踪**：解析 SSE 末尾的 usage chunk
- **连接超时保护**：防止网络挂起导致 Tauri 命令永久阻塞
- **禁止代理**：软件运行时必须直连 LLM API（铁律一）

## 决策

采用 `reqwest` + 手写 SSE 解析器：

- **HTTP 客户端**：`reqwest::Client` + `.no_proxy()` 禁止代理
- **流式响应**：`resp.bytes_stream()` → 逐 chunk 异步流
- **SSE 解析器**：`SseParser` 手写缓冲切分（`data: ` 前缀 + `\n\n` 分隔）
- **事件类型**：`StreamItem::Token` / `Reasoning` / `Usage` / `Done`
- **推理通道**：独立 `mpsc::unbounded_channel` 分流 reasoning_content
- **用量追踪**：`Arc<Mutex<Option<TokenUsage>>>` 共享 cell
- **超时**：connect_timeout=30s + request_timeout=300s
- **429 退避重试**：P1-2 新增，指数退避 1s/2s/4s（仅在连接阶段重试）

## 考虑的替代方案

| 方案 | 优点 | 缺点 | 为什么没选 |
|---|---|---|---|
| reqwest-eventsource crate | 封装 SSE | 不支持 POST body 流式 | 功能不足 |
| hyper + 手写 HTTP | 极致控制 | 代码量大，需自己管理连接池 | 过度工程 |
| WebSocket | 双向通信 | OpenAI API 不支持 WS | 协议不兼容 |
| 轮询（非流式） | 实现简单 | 无实时打字效果，用户体验差 | UX 差 |

## 后果

- 正面：SSE 流式输出提供良好的打字效果 UX
- 正面：reasoning_content 独立通道支持 DeepSeek R1 思考过程展示
- 正面：CancellationToken 可随时中断，保留部分输出
- 正面：.no_proxy() 确保铁律一合规
- 正面：429 指数退避重试提升弹性（P1-2）
- 负面：SSE 解析器手写维护，需处理边界情况（跨 chunk 事件）
- 负面：300s 总体超时在极端网络下仍可能过长
