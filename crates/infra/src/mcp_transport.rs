//! MCP 传输层实现（REQ-ARCH-016 AC-2/AC-3）。
//!
//! 实现 `crates/core::mcp::transport::McpTransport` trait 的两种具体传输：
//!
//! - `StdioTransport` — 启动子进程，通过 stdin/stdout 进行 JSON-RPC 2.0 通信
//! - `SseTransport` — 通过 HTTP POST 请求发送 JSON-RPC，接收 SSE 响应
//!
//! ## 安全
//!
//! - stdio 传输：子进程继承当前环境变量 + 用户配置的环境变量
//! - SSE 传输：HTTP 客户端强制 `.no_proxy()` 直连（铁律一：软件本身禁止代理）
//! - 请求超时：默认 30 秒

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use anyhow::{Context, Result, bail};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::debug;

use echomind_core::mcp::transport::{JsonRpcRequest, JsonRpcResponse, McpTransport};

/// 请求超时时间（秒）。
const REQUEST_TIMEOUT_SECS: u64 = 30;

// ============================================================================
// StdioTransport — 子进程 stdin/stdout JSON-RPC 通信（AC-2）
// ============================================================================

/// stdio 传输层：启动子进程，通过 stdin/stdout 进行 JSON-RPC 2.0 通信。
///
/// 每条 JSON-RPC 消息为一行 JSON（以 `\n` 分隔）。
///
/// # 生命周期
///
/// 1. `new()` — 启动子进程
/// 2. `send_request()` — 写入 stdin + 从 stdout 读取响应
/// 3. `disconnect()` — 关闭子进程（kill）
pub struct StdioTransport {
    /// 子进程句柄（Mutex 保护，避免并发写入 stdin）
    child: Mutex<Option<Child>>,
    /// 子进程 stdin 写入端
    stdin: Mutex<Option<tokio::process::ChildStdin>>,
    /// 子进程 stdout 读取端
    stdout: Mutex<Option<BufReader<tokio::process::ChildStdout>>>,
    /// 是否已连接
    connected: std::sync::atomic::AtomicBool,
}

impl StdioTransport {
    /// 启动子进程并创建 stdio 传输。
    ///
    /// # 参数
    /// - `command` — 可执行文件路径（如 `npx`、`node`、`python3`）
    /// - `args` — 命令行参数
    /// - `env` — 环境变量
    pub async fn new(command: &str, args: &[String], env: &[(String, String)]) -> Result<Self> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // 设置环境变量
        for (k, v) in env {
            cmd.env(k, v);
        }
        // 安全：子进程不继承父进程的代理设置
        cmd.env_remove("http_proxy");
        cmd.env_remove("https_proxy");
        cmd.env_remove("all_proxy");

        let mut child = cmd
            .spawn()
            .with_context(|| format!("启动 MCP 子进程失败: {command}"))?;

        let stdin = child.stdin.take().context("无法获取子进程 stdin")?;
        let stdout = child.stdout.take().context("无法获取子进程 stdout")?;

        debug!(command, "MCP stdio 子进程已启动");

        Ok(Self {
            child: Mutex::new(Some(child)),
            stdin: Mutex::new(Some(stdin)),
            stdout: Mutex::new(Some(BufReader::new(stdout))),
            connected: std::sync::atomic::AtomicBool::new(true),
        })
    }
}

impl McpTransport for StdioTransport {
    fn send_request<'a>(
        &'a self,
        request: &'a JsonRpcRequest,
    ) -> Pin<Box<dyn Future<Output = Result<JsonRpcResponse>> + Send + 'a>> {
        Box::pin(async move {
            let json = request.to_json().context("序列化 JSON-RPC 请求失败")?;
            let json_line = format!("{json}\n");

            // 写入 stdin
            {
                let mut stdin_guard = self.stdin.lock().await;
                let stdin = stdin_guard
                    .as_mut()
                    .context("stdio 传输已断开（stdin 不可用）")?;
                stdin
                    .write_all(json_line.as_bytes())
                    .await
                    .context("写入子进程 stdin 失败")?;
                stdin.flush().await.context("flush stdin 失败")?;
            }

            // 从 stdout 读取一行响应
            let response = {
                let mut stdout_guard = self.stdout.lock().await;
                let stdout = stdout_guard
                    .as_mut()
                    .context("stdio 传输已断开（stdout 不可用）")?;
                let mut line = String::new();
                // 读取一行（含超时保护）
                let read_result = tokio::time::timeout(
                    std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS),
                    stdout.read_line(&mut line),
                )
                .await;
                match read_result {
                    Ok(Ok(0)) => bail!("子进程 stdout 已关闭"),
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => bail!("读取子进程 stdout 失败: {e}"),
                    Err(_) => bail!("MCP 请求超时（{REQUEST_TIMEOUT_SECS}s）"),
                }
                line
            };

            let response =
                JsonRpcResponse::from_json(&response).context("反序列化 JSON-RPC 响应失败")?;
            Ok(response)
        })
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn disconnect(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.connected
                .store(false, std::sync::atomic::Ordering::SeqCst);
            // 关闭 stdin
            if let Some(mut stdin) = self.stdin.lock().await.take() {
                let _ = stdin.shutdown().await;
            }
            // 关闭 stdout
            self.stdout.lock().await.take();
            // kill 子进程
            if let Some(mut child) = self.child.lock().await.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
            debug!("MCP stdio 传输已断开");
        })
    }

    fn transport_name(&self) -> &'static str {
        "stdio"
    }
}

// ============================================================================
// SseTransport — HTTP + SSE JSON-RPC 通信（AC-3）
// ============================================================================

/// SSE 传输层：通过 HTTP POST 发送 JSON-RPC 请求，接收 SSE 响应。
///
/// 每次请求通过 HTTP POST 发送到服务器 URL，响应可以是：
/// - 直接 JSON 响应（非 SSE 模式）
/// - SSE 事件流（`Content-Type: text/event-stream`）
///
/// # 安全
///
/// HTTP 客户端强制 `.no_proxy()` 直连（铁律一：软件本身禁止代理）。
pub struct SseTransport {
    /// 服务器 URL
    url: String,
    /// HTTP 客户端（直连，无代理）
    client: reqwest::Client,
    /// 认证头
    headers: HashMap<String, String>,
    /// 是否已连接
    connected: std::sync::atomic::AtomicBool,
}

impl SseTransport {
    /// 创建 SSE 传输。
    ///
    /// # 参数
    /// - `url` — MCP 服务器 URL（如 `https://example.com/mcp`）
    /// - `headers` — 认证头（如 `Authorization: Bearer xxx`）
    pub fn new(url: &str, headers: &[(String, String)]) -> Result<Self> {
        let client = reqwest::Client::builder()
            .no_proxy() // 铁律一：软件本身禁止代理
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("创建 HTTP 客户端失败")?;

        let headers_map: HashMap<String, String> = headers.iter().cloned().collect();

        Ok(Self {
            url: url.to_string(),
            client,
            headers: headers_map,
            connected: std::sync::atomic::AtomicBool::new(true),
        })
    }
}

impl McpTransport for SseTransport {
    fn send_request<'a>(
        &'a self,
        request: &'a JsonRpcRequest,
    ) -> Pin<Box<dyn Future<Output = Result<JsonRpcResponse>> + Send + 'a>> {
        Box::pin(async move {
            let json = request.to_json().context("序列化 JSON-RPC 请求失败")?;

            let mut req_builder = self
                .client
                .post(&self.url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .body(json);

            // 添加认证头
            for (k, v) in &self.headers {
                req_builder = req_builder.header(k, v);
            }

            let response = req_builder.send().await.context("MCP HTTP 请求失败")?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                bail!("MCP 服务器返回 HTTP {status}: {body}");
            }

            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            // 解析响应
            if content_type.contains("text/event-stream") {
                // SSE 模式：读取事件流，提取 data 行
                let text = response.text().await.context("读取 SSE 响应失败")?;
                let json_str = parse_sse_response(&text)?;
                let resp = JsonRpcResponse::from_json(&json_str)
                    .context("反序列化 SSE JSON-RPC 响应失败")?;
                Ok(resp)
            } else {
                // 普通 JSON 响应
                let text = response.text().await.context("读取 HTTP 响应失败")?;
                let resp =
                    JsonRpcResponse::from_json(&text).context("反序列化 JSON-RPC 响应失败")?;
                Ok(resp)
            }
        })
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn disconnect(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            self.connected
                .store(false, std::sync::atomic::Ordering::SeqCst);
            debug!("MCP SSE 传输已断开");
        })
    }

    fn transport_name(&self) -> &'static str {
        "sse"
    }
}

/// 从 SSE 事件流文本中提取 JSON-RPC 响应。
///
/// SSE 格式：每行以 `data: ` 前缀开头，多个 data 行拼接为完整 JSON。
fn parse_sse_response(text: &str) -> Result<String> {
    let mut data_lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            continue;
        }
        if let Some(data) = trimmed.strip_prefix("data:") {
            data_lines.push(data.trim());
        }
    }
    if data_lines.is_empty() {
        bail!("SSE 响应中无 data 行");
    }
    Ok(data_lines.join("\n"))
}

// ============================================================================
// TDD 测试
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    /// TC-MCP-002c：StdioTransport 创建失败（无效命令）
    #[tokio::test]
    async fn test_stdio_transport_invalid_command() {
        let result = StdioTransport::new("this-command-does-not-exist-12345", &[], &[]).await;
        assert!(result.is_err());
    }

    /// TC-MCP-002d：StdioTransport 正常创建（echo 命令）
    #[cfg(unix)]
    #[tokio::test]
    async fn test_stdio_transport_create_echo() {
        let transport = StdioTransport::new("echo", &["hello".to_string()], &[]).await;
        assert!(transport.is_ok());
        let t = transport.unwrap();
        assert!(t.is_connected());
        t.disconnect().await;
        assert!(!t.is_connected());
    }

    /// TC-MCP-003b：SseTransport 创建
    #[test]
    fn test_sse_transport_create() {
        let transport = SseTransport::new("https://example.com/mcp", &[]);
        assert!(transport.is_ok());
        let t = transport.unwrap();
        assert_eq!(t.transport_name(), "sse");
        assert!(t.is_connected());
    }

    /// TC-MCP-003c：SseTransport 带认证头创建
    #[test]
    fn test_sse_transport_with_headers() {
        let headers = vec![("Authorization".to_string(), "Bearer token123".to_string())];
        let transport = SseTransport::new("https://example.com/mcp", &headers);
        assert!(transport.is_ok());
        let t = transport.unwrap();
        assert_eq!(t.headers.len(), 1);
        assert_eq!(
            t.headers.get("Authorization"),
            Some(&"Bearer token123".to_string())
        );
    }

    /// TC-MCP-003d：parse_sse_response 解析 SSE 事件流
    #[test]
    fn test_parse_sse_response_single_data() {
        let sse = "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
        let json = parse_sse_response(sse).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
    }

    /// TC-MCP-003e：parse_sse_response 多行 data 拼接
    #[test]
    fn test_parse_sse_response_multi_data() {
        let sse = "data: {\"jsonrpc\":\"2.0\",\ndata: \"id\":1,\ndata: \"result\":{}}\n\n";
        let json = parse_sse_response(sse).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
    }

    /// TC-MCP-003f：parse_sse_response 无 data 行
    #[test]
    fn test_parse_sse_response_no_data() {
        let sse = ": comment\n\nevent: ping\n\n";
        let result = parse_sse_response(sse);
        assert!(result.is_err());
    }

    /// TC-MCP-003g：parse_sse_response 跳过注释和空行
    #[test]
    fn test_parse_sse_response_skip_comments() {
        let sse = ": this is a comment\n\ndata: {\"jsonrpc\":\"2.0\",\"id\":1}\n\n";
        let json = parse_sse_response(sse).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
    }

    /// TC-MCP-002e：StdioTransport transport_name
    #[cfg(unix)]
    #[tokio::test]
    async fn test_stdio_transport_name() {
        let t = StdioTransport::new("echo", &[], &[]).await.unwrap();
        assert_eq!(t.transport_name(), "stdio");
        t.disconnect().await;
    }
}
