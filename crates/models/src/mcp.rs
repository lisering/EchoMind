//! MCP 协议数据模型。
//!
//! 对应 SRS：REQ-ARCH-016 MCP 协议适配器 Phase 1。

use serde::{Deserialize, Serialize};

/// MCP 传输类型（REQ-ARCH-016 AC-2/AC-3）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum McpTransportType {
    #[default]
    Stdio,
    Sse,
}

impl McpTransportType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Sse => "sse",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "stdio" => Some(Self::Stdio),
            "sse" => Some(Self::Sse),
            _ => None,
        }
    }
}

/// MCP 服务器配置（REQ-ARCH-016 AC-1）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub transport: McpTransportType,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub env: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub headers: Vec<(String, String)>,
    #[serde(default = "crate::default_true")]
    pub enabled: bool,
}

impl McpServerConfig {
    pub fn new_stdio(
        id: impl Into<String>,
        name: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport: McpTransportType::Stdio,
            command: Some(command.into()),
            args: Vec::new(),
            env: Vec::new(),
            url: None,
            headers: Vec::new(),
            enabled: true,
        }
    }

    pub fn new_sse(id: impl Into<String>, name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            transport: McpTransportType::Sse,
            command: None,
            args: Vec::new(),
            env: Vec::new(),
            url: Some(url.into()),
            headers: Vec::new(),
            enabled: true,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("服务器名称不能为空".to_string());
        }
        if self.id.trim().is_empty() {
            return Err("服务器 ID 不能为空".to_string());
        }
        match self.transport {
            McpTransportType::Stdio => {
                if self
                    .command
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
                {
                    return Err("stdio 传输需要指定 command（可执行文件路径）".to_string());
                }
            }
            McpTransportType::Sse => {
                if self.url.as_deref().map(str::trim).unwrap_or("").is_empty() {
                    return Err("SSE 传输需要指定 url（服务器地址）".to_string());
                }
            }
        }
        Ok(())
    }
}

/// MCP 工具定义（REQ-ARCH-016 AC-4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: String,
    pub server_id: String,
    pub server_name: String,
}

/// MCP 工具调用结果（REQ-ARCH-016 AC-5）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub tool_name: String,
    pub server_id: String,
    pub success: bool,
    pub content: String,
    #[serde(default)]
    pub is_error: bool,
}

/// MCP 服务器连接状态（REQ-ARCH-016 AC-6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum McpConnectionStatus {
    Connected,
    #[default]
    Disconnected,
    Error,
}

impl McpConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Error => "error",
        }
    }
}

/// MCP 服务器状态信息（REQ-ARCH-016 AC-6）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub config: McpServerConfig,
    pub status: McpConnectionStatus,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub tool_count: usize,
}
