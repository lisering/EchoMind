//! MCP 服务器配置管理（REQ-ARCH-016 AC-1）。
//!
//! 提供服务器配置列表的序列化/反序列化、CRUD 操作和验证。
//! 配置以 JSON 格式存储在 settings 表 `mcp.servers` 键中。

use echomind_models::McpServerConfig;

/// settings 表中存储 MCP 服务器配置的键名。
pub const MCP_SERVERS_SETTINGS_KEY: &str = "mcp.servers";

/// 序列化服务器配置列表为 JSON 字符串。
pub fn serialize_servers(servers: &[McpServerConfig]) -> Result<String, serde_json::Error> {
    serde_json::to_string(servers)
}

/// 反序列化 JSON 字符串为服务器配置列表。
///
/// 空字符串返回空列表（向后兼容：首次使用时 settings 表无此键）。
pub fn deserialize_servers(json: &str) -> Result<Vec<McpServerConfig>, serde_json::Error> {
    if json.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(json)
}

/// 添加服务器到配置列表。
///
/// 返回更新后的列表。如果 ID 已存在则返回错误。
pub fn add_server(
    servers: &mut Vec<McpServerConfig>,
    config: McpServerConfig,
) -> Result<(), String> {
    config.validate()?;
    if servers.iter().any(|s| s.id == config.id) {
        return Err(format!("服务器 ID 已存在: {}", config.id));
    }
    servers.push(config);
    Ok(())
}

/// 从配置列表中移除指定 ID 的服务器。
///
/// 返回是否成功移除。
pub fn remove_server(servers: &mut Vec<McpServerConfig>, id: &str) -> bool {
    let before = servers.len();
    servers.retain(|s| s.id != id);
    servers.len() < before
}

/// 启用/禁用指定 ID 的服务器。
///
/// 返回是否成功找到并修改。
pub fn toggle_server(servers: &mut [McpServerConfig], id: &str, enabled: bool) -> bool {
    if let Some(server) = servers.iter_mut().find(|s| s.id == id) {
        server.enabled = enabled;
        true
    } else {
        false
    }
}

/// 查找指定 ID 的服务器配置。
pub fn find_server<'a>(servers: &'a [McpServerConfig], id: &str) -> Option<&'a McpServerConfig> {
    servers.iter().find(|s| s.id == id)
}

/// 获取所有已启用的服务器配置。
pub fn enabled_servers(servers: &[McpServerConfig]) -> Vec<&McpServerConfig> {
    servers.iter().filter(|s| s.enabled).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use echomind_models::{McpServerConfig, McpTransportType};

    /// TC-MCP-001a：添加/删除/启用/禁用 MCP 服务器配置
    #[test]
    fn test_server_config_crud() {
        let mut servers = Vec::new();

        // 添加 stdio 服务器
        let s1 = McpServerConfig::new_stdio("srv-1", "文件系统", "npx");
        assert!(add_server(&mut servers, s1).is_ok());
        assert_eq!(servers.len(), 1);

        // 重复 ID 拒绝
        let s1_dup = McpServerConfig::new_stdio("srv-1", "重复", "node");
        assert!(add_server(&mut servers, s1_dup).is_err());

        // 添加 SSE 服务器
        let s2 = McpServerConfig::new_sse("srv-2", "远程工具", "https://example.com/mcp");
        assert!(add_server(&mut servers, s2).is_ok());
        assert_eq!(servers.len(), 2);

        // 禁用
        assert!(toggle_server(&mut servers, "srv-1", false));
        assert!(!find_server(&servers, "srv-1").unwrap().enabled);

        // 启用
        assert!(toggle_server(&mut servers, "srv-1", true));
        assert!(find_server(&servers, "srv-1").unwrap().enabled);

        // 启用列表
        let enabled = enabled_servers(&servers);
        assert_eq!(enabled.len(), 2);

        // 禁用后
        toggle_server(&mut servers, "srv-2", false);
        let enabled = enabled_servers(&servers);
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].id, "srv-1");

        // 删除
        assert!(remove_server(&mut servers, "srv-1"));
        assert_eq!(servers.len(), 1);
        assert!(!remove_server(&mut servers, "nonexistent"));
    }

    /// TC-MCP-001b：配置验证
    #[test]
    fn test_server_config_validate() {
        // 有效 stdio 配置
        let valid = McpServerConfig::new_stdio("id-1", "测试", "npx");
        assert!(valid.validate().is_ok());

        // 有效 SSE 配置
        let valid_sse = McpServerConfig::new_sse("id-2", "远程", "https://example.com/mcp");
        assert!(valid_sse.validate().is_ok());

        // 空名称
        let mut invalid = McpServerConfig::new_stdio("id-3", "", "npx");
        assert!(invalid.validate().is_err());

        // stdio 缺少 command
        invalid.name = "测试".to_string();
        invalid.command = None;
        assert!(invalid.validate().is_err());

        // SSE 缺少 url
        invalid.transport = McpTransportType::Sse;
        invalid.command = None;
        invalid.url = None;
        assert!(invalid.validate().is_err());

        // SSE 有 url
        invalid.url = Some("https://example.com".to_string());
        assert!(invalid.validate().is_ok());
    }

    /// TC-MCP-001c：序列化/反序列化往返
    #[test]
    fn test_server_config_serde_roundtrip() {
        let servers = vec![
            McpServerConfig::new_stdio("srv-1", "文件系统", "npx"),
            McpServerConfig::new_sse("srv-2", "远程", "https://example.com/mcp"),
        ];

        let json = serialize_servers(&servers).unwrap();
        let deserialized = deserialize_servers(&json).unwrap();

        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].id, "srv-1");
        assert_eq!(deserialized[0].transport, McpTransportType::Stdio);
        assert_eq!(deserialized[1].id, "srv-2");
        assert_eq!(deserialized[1].transport, McpTransportType::Sse);
    }

    /// TC-MCP-001d：空 JSON 反序列化返回空列表
    #[test]
    fn test_deserialize_empty() {
        let servers = deserialize_servers("").unwrap();
        assert!(servers.is_empty());

        let servers = deserialize_servers("  ").unwrap();
        assert!(servers.is_empty());
    }
}
