//! MCP registry (REQ-ARCH-016 AC-4/AC-5/AC-6).
use super::client::McpClient;
use super::transport::McpTransport;
use echomind_models::{
    McpConnectionStatus, McpServerConfig, McpServerStatus, McpTool, McpToolCallResult,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

pub struct McpRegistry {
    clients: tokio::sync::RwLock<HashMap<String, Arc<McpClient>>>,
    tools_cache: tokio::sync::RwLock<HashMap<String, Vec<McpTool>>>,
    errors: tokio::sync::RwLock<HashMap<String, String>>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            clients: tokio::sync::RwLock::new(HashMap::new()),
            tools_cache: tokio::sync::RwLock::new(HashMap::new()),
            errors: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    pub async fn connect(
        &self,
        config: McpServerConfig,
        transport: Arc<dyn McpTransport>,
    ) -> anyhow::Result<()> {
        let server_id = config.id.clone();
        let server_name = config.name.clone();
        debug!(server = %server_name, "connecting MCP server");
        let client = Arc::new(McpClient::new(config, transport));
        if let Err(e) = client.initialize().await {
            let msg = format!("{e:#}");
            warn!(server = %server_name, error = %msg, "MCP connect failed");
            self.errors.write().await.insert(server_id.clone(), msg);
            return Err(e);
        }
        let tools = match client.list_tools().await {
            Ok(t) => t,
            Err(e) => {
                let msg = format!("{e:#}");
                warn!(server = %server_name, error = %msg, "list tools failed");
                self.errors.write().await.insert(server_id.clone(), msg);
                return Err(e);
            }
        };
        debug!(server = %server_name, tool_count = tools.len(), "MCP server connected");
        self.errors.write().await.remove(&server_id);
        self.clients.write().await.insert(server_id.clone(), client);
        self.tools_cache.write().await.insert(server_id, tools);
        Ok(())
    }

    pub async fn disconnect(&self, server_id: &str) {
        if let Some(client) = self.clients.write().await.remove(server_id) {
            client.disconnect().await;
            debug!(server_id, "MCP server disconnected");
        }
        self.tools_cache.write().await.remove(server_id);
        self.errors.write().await.remove(server_id);
    }

    pub async fn disconnect_all(&self) {
        let mut clients = self.clients.write().await;
        for (id, client) in clients.drain() {
            client.disconnect().await;
            debug!(server_id = %id, "MCP server disconnected");
        }
        self.tools_cache.write().await.clear();
        self.errors.write().await.clear();
    }

    pub async fn get_all_tools(&self) -> Vec<McpTool> {
        let cache = self.tools_cache.read().await;
        let mut all = Vec::new();
        for tools in cache.values() {
            all.extend(tools.iter().cloned());
        }
        all
    }

    pub async fn find_tool(&self, server_id: &str, tool_name: &str) -> Option<McpTool> {
        let cache = self.tools_cache.read().await;
        cache
            .get(server_id)?
            .iter()
            .find(|t| t.name == tool_name)
            .cloned()
    }

    pub async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> anyhow::Result<McpToolCallResult> {
        let client = { self.clients.read().await.get(server_id).cloned() }
            .ok_or_else(|| anyhow::anyhow!("MCP server not connected: {server_id}"))?;
        let result = client.call_tool(tool_name, args).await?;
        if result.is_error {
            warn!(server_id, tool = tool_name, "MCP tool call error");
        }
        Ok(result)
    }

    pub async fn get_server_statuses(&self, configs: &[McpServerConfig]) -> Vec<McpServerStatus> {
        let clients = self.clients.read().await;
        let errors = self.errors.read().await;
        let tools = self.tools_cache.read().await;
        configs
            .iter()
            .map(|c| {
                let (status, err, tool_count) = if let Some(err_msg) = errors.get(&c.id) {
                    (McpConnectionStatus::Error, Some(err_msg.clone()), 0)
                } else if clients.contains_key(&c.id) {
                    let count = tools.get(&c.id).map(|t| t.len()).unwrap_or(0);
                    (McpConnectionStatus::Connected, None, count)
                } else {
                    (McpConnectionStatus::Disconnected, None, 0)
                };
                McpServerStatus {
                    config: c.clone(),
                    status,
                    error_message: err,
                    tool_count,
                }
            })
            .collect()
    }

    pub async fn is_connected(&self, server_id: &str) -> bool {
        self.clients.read().await.contains_key(server_id)
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}
