//! MCP 域 IPC 命令子模块（REQ-ARCH-016 MCP 协议适配器）。
//!
//! 6 个 IPC 命令：
//! - `add_mcp_server` — 添加 MCP 服务器配置
//! - `remove_mcp_server` — 删除 MCP 服务器
//! - `list_mcp_servers` — 列出所有 MCP 服务器配置 + 状态
//! - `toggle_mcp_server` — 启用/禁用 MCP 服务器
//! - `get_mcp_tools` — 获取已连接 MCP 服务器的工具列表
//! - `call_mcp_tool` — 调用 MCP 工具（需用户确认）

use super::*;
use echomind_core::mcp::config as mcp_config;
use echomind_core::mcp::transport::McpTransport;
use echomind_models::{
    McpServerConfig, McpServerStatus, McpTool, McpToolCallResult, McpTransportType,
};
use std::sync::Arc;

/// settings 表中存储 MCP 服务器配置的键名。
const MCP_SERVERS_KEY: &str = "mcp.servers";

/// 从 settings 表读取 MCP 服务器配置列表。
async fn load_servers(state: &AppState) -> Result<Vec<McpServerConfig>, String> {
    let json = state
        .storage
        .get_setting(MCP_SERVERS_KEY)
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    mcp_config::deserialize_servers(&json).map_err(|e| format!("解析 MCP 配置失败: {e}"))
}

/// 保存 MCP 服务器配置列表到 settings 表。
async fn save_servers(state: &AppState, servers: &[McpServerConfig]) -> Result<(), String> {
    let json = mcp_config::serialize_servers(servers).map_err(|e| format!("序列化失败: {e}"))?;
    state
        .storage
        .set_setting(MCP_SERVERS_KEY, &json)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 添加 MCP 服务器配置（REQ-ARCH-016 AC-1）。
#[tauri::command]
pub async fn add_mcp_server(
    config: McpServerConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    add_mcp_server_inner(config, state.inner()).await
}

pub async fn add_mcp_server_inner(config: McpServerConfig, state: &AppState) -> Result<(), String> {
    config.validate().map_err(|e| e.to_string())?;
    let config_clone = config.clone();
    let mut servers = load_servers(state).await?;
    mcp_config::add_server(&mut servers, config).map_err(|e| e.to_string())?;
    save_servers(state, &servers).await?;
    // 如果配置启用，尝试连接
    if config_clone.enabled {
        try_connect_server(state, &config_clone).await;
    }
    Ok(())
}

/// 删除 MCP 服务器配置（REQ-ARCH-016 AC-1）。
#[tauri::command]
pub async fn remove_mcp_server(id: String, state: State<'_, AppState>) -> Result<(), String> {
    remove_mcp_server_inner(id, state.inner()).await
}

pub async fn remove_mcp_server_inner(id: String, state: &AppState) -> Result<(), String> {
    let mut servers = load_servers(state).await?;
    if !mcp_config::remove_server(&mut servers, &id) {
        return Err(format!("服务器不存在: {id}"));
    }
    // 断开运行时连接
    state.mcp_registry.disconnect(&id).await;
    save_servers(state, &servers).await
}

/// 列出所有 MCP 服务器配置 + 连接状态（REQ-ARCH-016 AC-6）。
#[tauri::command]
pub async fn list_mcp_servers(state: State<'_, AppState>) -> Result<Vec<McpServerStatus>, String> {
    list_mcp_servers_inner(state.inner()).await
}

pub async fn list_mcp_servers_inner(state: &AppState) -> Result<Vec<McpServerStatus>, String> {
    let servers = load_servers(state).await?;
    Ok(state.mcp_registry.get_server_statuses(&servers).await)
}

/// 启用/禁用 MCP 服务器（REQ-ARCH-016 AC-1）。
#[tauri::command]
pub async fn toggle_mcp_server(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    toggle_mcp_server_inner(id, enabled, state.inner()).await
}

pub async fn toggle_mcp_server_inner(
    id: String,
    enabled: bool,
    state: &AppState,
) -> Result<(), String> {
    let mut servers = load_servers(state).await?;
    if !mcp_config::toggle_server(&mut servers, &id, enabled) {
        return Err(format!("服务器不存在: {id}"));
    }
    if enabled {
        // 启用：尝试连接
        if let Some(config) = mcp_config::find_server(&servers, &id).cloned() {
            try_connect_server(state, &config).await;
        }
    } else {
        // 禁用：断开连接
        state.mcp_registry.disconnect(&id).await;
    }
    save_servers(state, &servers).await
}

/// 获取已连接 MCP 服务器的工具列表（REQ-ARCH-016 AC-4）。
#[tauri::command]
pub async fn get_mcp_tools(state: State<'_, AppState>) -> Result<Vec<McpTool>, String> {
    get_mcp_tools_inner(state.inner()).await
}

pub async fn get_mcp_tools_inner(state: &AppState) -> Result<Vec<McpTool>, String> {
    Ok(state.mcp_registry.get_all_tools().await)
}

/// 调用 MCP 工具（REQ-ARCH-016 AC-5）。
///
/// **安全说明**：前端必须在调用前弹出确认对话框并获得用户许可。
/// 后端不二次确认，信任前端已获得用户同意。
#[tauri::command]
pub async fn call_mcp_tool(
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<McpToolCallResult, String> {
    call_mcp_tool_inner(server_id, tool_name, arguments, state.inner()).await
}

pub async fn call_mcp_tool_inner(
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    state: &AppState,
) -> Result<McpToolCallResult, String> {
    state
        .mcp_registry
        .call_tool(&server_id, &tool_name, &arguments)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 尝试连接 MCP 服务器（REQ-ARCH-016 AC-2/AC-3）。
///
/// 根据传输类型创建对应的 transport 实现，然后通过 registry 建立连接。
/// 连接失败不会返回错误（静默降级），前端通过 `list_mcp_servers` 查看状态。
async fn try_connect_server(state: &AppState, config: &McpServerConfig) {
    let transport: Arc<dyn McpTransport> = match config.transport {
        McpTransportType::Stdio => {
            let command = match &config.command {
                Some(c) if !c.trim().is_empty() => c.clone(),
                _ => {
                    warn!(server_id = %config.id, "stdio 传输缺少 command");
                    return;
                }
            };
            match echomind_infra::mcp_transport::StdioTransport::new(
                &command,
                &config.args,
                &config.env,
            )
            .await
            {
                Ok(t) => Arc::new(t),
                Err(e) => {
                    warn!(server_id = %config.id, error = %format!("{e:#}"), "stdio 传输创建失败");
                    return;
                }
            }
        }
        McpTransportType::Sse => {
            let url = match &config.url {
                Some(u) if !u.trim().is_empty() => u.clone(),
                _ => {
                    warn!(server_id = %config.id, "SSE 传输缺少 url");
                    return;
                }
            };
            match echomind_infra::mcp_transport::SseTransport::new(&url, &config.headers) {
                Ok(t) => Arc::new(t),
                Err(e) => {
                    warn!(server_id = %config.id, error = %format!("{e:#}"), "SSE 传输创建失败");
                    return;
                }
            }
        }
    };

    if let Err(e) = state.mcp_registry.connect(config.clone(), transport).await {
        warn!(server_id = %config.id, error = %format!("{e:#}"), "MCP 服务器连接失败");
    }
}
