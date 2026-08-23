//! MCP 传输层抽象（REQ-ARCH-016 AC-2/AC-3）。
//!
//! 定义 JSON-RPC 2.0 消息的传输接口。两种实现：
//! - `StdioTransport`（infra 层）— 启动子进程，通过 stdin/stdout 通信
//! - `SseTransport`（infra 层）— HTTP POST 请求 + SSE 响应
//!
//! 本模块仅定义 trait，具体实现在 `crates/infra` 中（六边形架构依赖方向）。

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 请求消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC 版本（固定 `"2.0"`）
    pub jsonrpc: String,
    /// 请求 ID（整数或字符串）
    pub id: serde_json::Value,
    /// 方法名
    pub method: String,
    /// 参数
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 响应消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC 版本
    pub jsonrpc: String,
    /// 对应的请求 ID
    pub id: serde_json::Value,
    /// 结果（成功时）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub result: Option<serde_json::Value>,
    /// 错误（失败时）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 错误对象。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// 错误码
    pub code: i32,
    /// 错误消息
    pub message: String,
    /// 附加数据
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// 创建新的 JSON-RPC 请求。
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(serde_json::Number::from(id)),
            method: method.into(),
            params,
        }
    }

    /// 序列化为 JSON 字符串（stdio 传输每行一条消息）。
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl JsonRpcResponse {
    /// 从 JSON 字符串反序列化。
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 是否为错误响应。
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// 获取结果值（如果是成功响应）。
    pub fn result_value(&self) -> Option<&serde_json::Value> {
        self.result.as_ref()
    }
}

/// MCP 传输层 trait（抽象端口）。
///
/// 定义 JSON-RPC 消息的发送和接收接口。
/// 具体实现在 `crates/infra` 中：
/// - `StdioTransport` — 子进程 stdin/stdout
/// - `SseTransport` — HTTP + SSE
///
/// # 设计
///
/// 使用 `async fn` in trait（Edition 2024），无需 `async-trait` 宏。
/// 所有方法返回 `Pin<Box<dyn Future>>` 以支持 trait 对象安全。
pub trait McpTransport: Send + Sync {
    /// 发送 JSON-RPC 请求并等待响应。
    ///
    /// 超时由实现层控制（默认 30s）。
    fn send_request<'a>(
        &'a self,
        request: &'a JsonRpcRequest,
    ) -> Pin<Box<dyn Future<Output = Result<JsonRpcResponse, anyhow::Error>> + Send + 'a>>;

    /// 检查传输层是否已连接。
    fn is_connected(&self) -> bool;

    /// 关闭传输层连接。
    fn disconnect(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// 获取传输类型名称（用于日志）。
    fn transport_name(&self) -> &'static str;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// TC-MCP-002a：JSON-RPC 请求序列化
    #[test]
    fn test_jsonrpc_request_serialize() {
        let req = JsonRpcRequest::new(
            1,
            "initialize",
            Some(serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {}
            })),
        );

        let json = req.to_json().unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"initialize\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("protocolVersion"));
    }

    /// TC-MCP-002b：JSON-RPC 响应反序列化（成功）
    #[test]
    fn test_jsonrpc_response_success() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {}
                }
            }
        }"#;

        let resp = JsonRpcResponse::from_json(json).unwrap();
        assert!(!resp.is_error());
        assert!(resp.result_value().is_some());
        let result = resp.result_value().unwrap();
        assert!(result.get("protocolVersion").is_some());
    }

    /// TC-MCP-002c：JSON-RPC 响应反序列化（错误）
    #[test]
    fn test_jsonrpc_response_error() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 2,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        }"#;

        let resp = JsonRpcResponse::from_json(json).unwrap();
        assert!(resp.is_error());
        assert!(resp.error.is_some());
        let error = resp.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }

    /// TC-MCP-003a：JSON-RPC 无参数请求
    #[test]
    fn test_jsonrpc_request_no_params() {
        let req = JsonRpcRequest::new(5, "tools/list", None);
        let json = req.to_json().unwrap();
        assert!(!json.contains("params"));
    }
}
