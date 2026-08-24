//! MCP JSON-RPC 2.0 客户端（REQ-ARCH-016 AC-2/AC-3）。
//!
//! 通过 `McpTransport` trait 与 MCP 服务器通信。
//! 支持三种核心操作：
//! - `initialize` — 握手 + 能力协商
//! - `list_tools` — 获取服务器暴露的工具列表
//! - `call_tool` — 调用指定工具

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use tracing::debug;

use super::transport::{JsonRpcRequest, McpTransport};
use echomind_models::{McpConnectionStatus, McpServerConfig, McpTool, McpToolCallResult};

/// MCP 协议版本。
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// MCP 客户端（REQ-ARCH-016）。
///
/// 通过 `McpTransport` 与 MCP 服务器通信。每个 `McpClient` 管理一个服务器连接。
///
/// # 生命周期
///
/// 1. `new()` — 创建客户端（未连接）
/// 2. `initialize()` — 发送 initialize 请求，握手
/// 3. `list_tools()` — 获取工具列表
/// 4. `call_tool()` — 调用工具
/// 5. `disconnect()` — 关闭连接
pub struct McpClient {
    /// 服务器配置
    config: McpServerConfig,
    /// 传输层实现
    transport: Arc<dyn McpTransport>,
    /// 请求 ID 自增计数器
    next_id: AtomicU64,
    /// 是否已初始化（完成握手）
    initialized: std::sync::Mutex<bool>,
}

impl McpClient {
    /// 创建新的 MCP 客户端。
    pub fn new(config: McpServerConfig, transport: Arc<dyn McpTransport>) -> Self {
        Self {
            config,
            transport,
            next_id: AtomicU64::new(1),
            initialized: std::sync::Mutex::new(false),
        }
    }

    /// 获取下一个请求 ID。
    fn next_request_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// 发送 initialize 请求（MCP 握手）。
    ///
    /// 协议版本协商 + 能力交换。
    pub async fn initialize(&self) -> Result<()> {
        let id = self.next_request_id();
        let params = serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": { "roots": { "listChanged": true } },
            "clientInfo": { "name": "EchoMind", "version": env!("CARGO_PKG_VERSION") }
        });

        let request = JsonRpcRequest::new(id, "initialize", Some(params));
        let response = self
            .transport
            .send_request(&request)
            .await
            .context("MCP initialize 请求失败")?;

        if response.is_error() {
            let error = response.error.context("initialize 返回错误但无错误对象")?;
            bail!("MCP initialize 失败: [{}] {}", error.code, error.message);
        }

        debug!(server = %self.config.name, "MCP initialize 成功");
        *self.initialized.lock().unwrap_or_else(|e| e.into_inner()) = true;
        Ok(())
    }

    /// 获取服务器暴露的工具列表（REQ-ARCH-016 AC-4）。
    ///
    /// 返回工具定义列表，包含名称、描述和输入参数 Schema。
    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        if !self.is_initialized() {
            bail!("MCP 客户端未初始化，请先调用 initialize()");
        }

        let id = self.next_request_id();
        let request = JsonRpcRequest::new(id, "tools/list", None);
        let response = self
            .transport
            .send_request(&request)
            .await
            .context("MCP tools/list 请求失败")?;

        if response.is_error() {
            let error = response.error.context("tools/list 返回错误但无错误对象")?;
            bail!("MCP tools/list 失败: [{}] {}", error.code, error.message);
        }

        let result = response.result.context("tools/list 成功但无 result 字段")?;
        let tools_value = result
            .get("tools")
            .context("tools/list 结果中无 tools 字段")?;
        let raw_tools: Vec<RawMcpTool> =
            serde_json::from_value(tools_value.clone()).context("解析工具列表失败")?;

        let tools: Vec<McpTool> = raw_tools
            .into_iter()
            .map(|t| McpTool {
                name: t.name,
                description: t.description.unwrap_or_default(),
                input_schema: t
                    .input_schema
                    .map(|s| serde_json::to_string(&s).unwrap_or_default())
                    .unwrap_or_default(),
                server_id: self.config.id.clone(),
                server_name: self.config.name.clone(),
            })
            .collect();

        debug!(
            server = %self.config.name,
            tool_count = tools.len(),
            "MCP 工具列表获取成功"
        );
        Ok(tools)
    }

    /// 调用 MCP 工具（REQ-ARCH-016 AC-5）。
    ///
    /// **安全说明**：调用方必须确保已获得用户确认。
    ///
    /// # 参数
    /// - `tool_name` — 工具名称
    /// - `arguments` — 工具参数（JSON 对象）
    ///
    /// # 返回
    /// 工具执行结果
    pub async fn call_tool(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<McpToolCallResult> {
        if !self.is_initialized() {
            bail!("MCP 客户端未初始化，请先调用 initialize()");
        }

        let id = self.next_request_id();
        let params = serde_json::json!({ "name": tool_name, "arguments": arguments });
        let request = JsonRpcRequest::new(id, "tools/call", Some(params));
        let response = self
            .transport
            .send_request(&request)
            .await
            .context("MCP tools/call 请求失败")?;

        if response.is_error() {
            let error = response.error.context("tools/call 返回错误但无错误对象")?;
            return Ok(McpToolCallResult {
                tool_name: tool_name.to_string(),
                server_id: self.config.id.clone(),
                success: false,
                content: format!("[{}] {}", error.code, error.message),
                is_error: true,
            });
        }

        let result = response.result.context("tools/call 成功但无 result 字段")?;
        let is_error = result
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let content = if let Some(arr) = result.get("content").and_then(|v| v.as_array()) {
            arr.iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                        item.get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string())
        };

        debug!(
            server = %self.config.name,
            tool = %tool_name,
            is_error,
            "MCP 工具调用完成"
        );
        Ok(McpToolCallResult {
            tool_name: tool_name.to_string(),
            server_id: self.config.id.clone(),
            success: !is_error,
            content,
            is_error,
        })
    }

    /// 关闭连接。
    pub async fn disconnect(&self) {
        self.transport.disconnect().await;
        *self.initialized.lock().unwrap_or_else(|e| e.into_inner()) = false;
    }

    /// 是否已初始化（完成握手）。
    pub fn is_initialized(&self) -> bool {
        *self.initialized.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 获取连接状态。
    pub fn connection_status(&self) -> McpConnectionStatus {
        if !self.transport.is_connected() {
            return McpConnectionStatus::Disconnected;
        }
        if self.is_initialized() {
            McpConnectionStatus::Connected
        } else {
            McpConnectionStatus::Disconnected
        }
    }

    /// 获取服务器配置。
    pub fn config(&self) -> &McpServerConfig {
        &self.config
    }
}

/// MCP 服务器返回的原始工具定义（内部解析用）。
#[derive(Debug, serde::Deserialize)]
struct RawMcpTool {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    input_schema: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::mcp::transport::{JsonRpcResponse, McpTransport};
    use std::future::Future;
    use std::pin::Pin;

    struct MockTransport {
        connected: std::sync::atomic::AtomicBool,
        responses: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }
    impl MockTransport {
        fn new() -> Self {
            Self {
                connected: std::sync::atomic::AtomicBool::new(true),
                responses: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
        fn set_response(&self, method: &str, json: &str) {
            self.responses
                .lock()
                .unwrap()
                .insert(method.to_string(), json.to_string());
        }
    }
    impl McpTransport for MockTransport {
        fn send_request<'a>(
            &'a self,
            req: &'a JsonRpcRequest,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<JsonRpcResponse>> + Send + 'a>> {
            let m = req.method.clone();
            let r = self.responses.lock().unwrap();
            let j = r.get(&m).cloned().unwrap_or_else(|| {
                r#"{"jsonrpc":"2.0","id":0,"error":{"code":-32601,"message":"not found"}}"#
                    .to_string()
            });
            Box::pin(async move { JsonRpcResponse::from_json(&j).map_err(anyhow::Error::from) })
        }
        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }
        fn disconnect(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.connected.store(false, Ordering::SeqCst);
            Box::pin(async {})
        }
        fn transport_name(&self) -> &'static str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_mcp_client_initialize() {
        let t = Arc::new(MockTransport::new());
        t.set_response("initialize", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}}}}"#);
        let c = McpClient::new(McpServerConfig::new_stdio("s1", "test", "echo"), t);
        assert!(!c.is_initialized());
        c.initialize().await.unwrap();
        assert!(c.is_initialized());
        assert_eq!(c.connection_status(), McpConnectionStatus::Connected);
    }

    #[tokio::test]
    async fn test_mcp_client_list_tools() {
        let t = Arc::new(MockTransport::new());
        t.set_response("initialize", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}}}}"#);
        t.set_response("tools/list", r#"{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"read_file","description":"read","inputSchema":{"type":"object"}}]}}"#);
        let c = McpClient::new(McpServerConfig::new_stdio("s1", "fs", "npx"), t);
        c.initialize().await.unwrap();
        let tools = c.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "read_file");
        assert_eq!(tools[0].server_id, "s1");
    }

    #[tokio::test]
    async fn test_mcp_client_call_tool() {
        let t = Arc::new(MockTransport::new());
        t.set_response("initialize", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}}}}"#);
        t.set_response("tools/call", r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"Hello!"}],"isError":false}}"#);
        let c = McpClient::new(McpServerConfig::new_stdio("s1", "test", "npx"), t);
        c.initialize().await.unwrap();
        let r = c
            .call_tool("read_file", &serde_json::json!({"path":"/tmp"}))
            .await
            .unwrap();
        assert!(r.success);
        assert_eq!(r.content, "Hello!");
        assert_eq!(r.tool_name, "read_file");
    }

    #[tokio::test]
    async fn test_mcp_client_call_tool_error() {
        let t = Arc::new(MockTransport::new());
        t.set_response("initialize", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}}}}"#);
        t.set_response("tools/call", r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"not found"}],"isError":true}}"#);
        let c = McpClient::new(McpServerConfig::new_stdio("s1", "test", "npx"), t);
        c.initialize().await.unwrap();
        let r = c
            .call_tool("read_file", &serde_json::json!({}))
            .await
            .unwrap();
        assert!(!r.success);
        assert!(r.is_error);
        assert_eq!(r.content, "not found");
    }

    #[tokio::test]
    async fn test_mcp_connection_status() {
        let t = Arc::new(MockTransport::new());
        t.set_response("initialize", r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-06-18","capabilities":{"tools":{}}}}"#);
        let c = McpClient::new(McpServerConfig::new_stdio("s1", "test", "npx"), t);
        assert_eq!(c.connection_status(), McpConnectionStatus::Disconnected);
        c.initialize().await.unwrap();
        assert_eq!(c.connection_status(), McpConnectionStatus::Connected);
        c.disconnect().await;
        assert_eq!(c.connection_status(), McpConnectionStatus::Disconnected);
    }

    #[tokio::test]
    async fn test_call_without_init_fails() {
        let t = Arc::new(MockTransport::new());
        let c = McpClient::new(McpServerConfig::new_stdio("s1", "test", "npx"), t);
        assert!(c.call_tool("test", &serde_json::json!({})).await.is_err());
    }
}
