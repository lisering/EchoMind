//! PortableTool Trait — 工具注册解耦（B-05 借鉴 Rig `PortableTool`）。
//!
//! ## 核心设计
//!
//! `PortableTool` 是 context-free 的工具抽象，将工具定义与 AgentEngine 解耦：
//!
//! - `ToolDefinition` — 工具元数据（name + description + parameters JSON Schema）
//! - `PortableTool` trait — 工具执行接口（`call(args) -> output`）
//! - `ToolRegistry` — 工具注册表（运行时注册/查找）
//!
//! ## 与现有 `AgentEngine::execute_tool_single` 的对比
//!
//! | 维度 | 现有 | PortableTool |
//! |---|---|---|
//! | 工具注册 | 硬编码 match 分支 | 运行时注册 `Box<dyn PortableTool>` |
//! | 参数声明 | 无 | JSON Schema |
//! | 新增工具 | 修改 agent.rs 源码 | 实现 trait + 注册 |
//! | 可测试性 | 需要 mock AgentEngine | 独立单元测试 |

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 工具元数据定义（name + description + parameters JSON Schema）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 工具名称（如 "search_kb"）
    pub name: String,
    /// 工具描述（LLM 可见的说明文本）
    pub description: String,
    /// 参数 JSON Schema（描述输入格式）
    pub parameters: Value,
}

impl ToolDefinition {
    /// 创建新工具定义。
    pub fn new(name: &str, description: &str, parameters: Value) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            parameters,
        }
    }

    /// 创建无参数工具定义。
    pub fn no_params(name: &str, description: &str) -> Self {
        Self::new(
            name,
            description,
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }
}

/// 工具执行输出。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// 输出文本（Observation 文本）
    pub text: String,
    /// 是否为错误输出
    pub is_error: bool,
}

impl ToolOutput {
    /// 创建正常输出。
    pub fn ok(text: String) -> Self {
        Self {
            text,
            is_error: false,
        }
    }

    /// 创建错误输出。
    pub fn error(text: String) -> Self {
        Self {
            text,
            is_error: true,
        }
    }
}

impl From<String> for ToolOutput {
    fn from(text: String) -> Self {
        Self::ok(text)
    }
}

/// PortableTool trait — context-free 工具执行接口。
///
/// 工具实现此 trait 后注册到 `ToolRegistry`，AgentEngine 通过注册表查找并执行工具，
/// 无需在源码中硬编码 match 分支。
///
/// # 对象安全
///
/// 使用手动 `Pin<Box<Future>>` 返回类型（与 `AgentHook` / `Reranker` 一致），
/// 保证对象安全，允许 `Box<dyn PortableTool>` 存储。
pub trait PortableTool: Send + Sync {
    /// 工具元数据。
    fn definition(&self) -> ToolDefinition;

    /// 执行工具。
    ///
    /// # 参数
    /// - `args`: 工具输入文本（LLM 生成的 Action Input）
    ///
    /// # 返回
    /// 工具执行结果（观察文本 + 是否为错误）。
    fn call<'a>(
        &'a self,
        args: &'a str,
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolOutput>> + Send + 'a>>;

    /// 是否启用（默认 true）。
    fn enabled(&self) -> bool {
        true
    }
}

/// 工具注册表：管理多个工具，按名称查找。
///
/// # 线程安全
///
/// 内部使用 `RwLock<HashMap<String, Box<dyn PortableTool>>>`，
/// 允许多线程并发读、单线程写。
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Box<dyn PortableTool>>>,
}

impl ToolRegistry {
    /// 创建空注册表。
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// 注册工具。
    pub fn register(&self, tool: Box<dyn PortableTool>) {
        let name = tool.definition().name;
        self.tools.write().unwrap().insert(name, tool);
    }

    /// 按名称查找工具。
    pub fn get(&self, name: &str) -> Option<ToolHandle> {
        let guard = self.tools.read().unwrap();
        guard.get(name).map(|t| ToolHandle {
            name: name.to_string(),
        })
    }

    /// 执行工具。
    pub async fn call(&self, name: &str, args: &str) -> anyhow::Result<ToolOutput> {
        let guard = self.tools.read().unwrap();
        let tool = guard.get(name).ok_or_else(|| {
            anyhow::anyhow!("未知工具: {name}")
        })?;
        if !tool.enabled() {
            return Ok(ToolOutput::error(format!("工具 {name} 已禁用")));
        }
        drop(guard);
        // 重新获取读锁调用（Pin<Box<Future>> 需要 &'a 引用）
        let guard = self.tools.read().unwrap();
        let tool = guard.get(name).ok_or_else(|| {
            anyhow::anyhow!("工具 {name} 在执行时被注销")
        })?;
        tool.call(args).await
    }

    /// 列出全部工具定义。
    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .read()
            .unwrap()
            .values()
            .map(|t| t.definition())
            .collect()
    }

    /// 已注册工具数量。
    pub fn count(&self) -> usize {
        self.tools.read().unwrap().len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.tools.read().unwrap().is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<String> = self.tools.read().unwrap().keys().cloned().collect();
        f.debug_struct("ToolRegistry")
            .field("tool_count", &names.len())
            .field("tool_names", &names)
            .finish()
    }
}

/// 工具句柄（轻量引用，用于查找结果）。
pub struct ToolHandle {
    pub name: String,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// 测试用工具：回显输入。
    struct EchoTool;

    impl PortableTool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::new(
                "echo",
                "回显输入文本",
                serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": { "type": "string", "description": "要回显的文本" }
                    },
                    "required": ["text"]
                }),
            )
        }

        fn call<'a>(
            &'a self,
            args: &'a str,
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolOutput>> + Send + 'a>>
        {
            Box::pin(async move { Ok(ToolOutput::ok(format!("Echo: {args}"))) })
        }
    }

    /// 测试用工具：返回固定文本。
    struct StaticTool {
        name: String,
        text: String,
    }

    impl PortableTool for StaticTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::no_params(&self.name, "Returns static text")
        }

        fn call<'a>(
            &'a self,
            _args: &'a str,
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolOutput>> + Send + 'a>>
        {
            let text = self.text.clone();
            Box::pin(async move { Ok(ToolOutput::ok(text)) })
        }
    }

    /// 测试用工具：返回错误。
    struct ErrorTool;

    impl PortableTool for ErrorTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition::no_params("fail", "Always fails")
        }

        fn call<'a>(
            &'a self,
            _args: &'a str,
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<ToolOutput>> + Send + 'a>>
        {
            Box::pin(async { Ok(ToolOutput::error("Something went wrong".into())) })
        }
    }

    // ─── ToolDefinition ───

    #[test]
    fn tc_tool_001_definition_new() {
        let def = ToolDefinition::new("test", "A test tool", serde_json::json!({"type": "object"}));
        assert_eq!(def.name, "test");
        assert_eq!(def.description, "A test tool");
    }

    #[test]
    fn tc_tool_002_definition_no_params() {
        let def = ToolDefinition::no_params("noop", "No params");
        assert_eq!(def.name, "noop");
        assert_eq!(def.parameters["type"], "object");
    }

    // ─── ToolOutput ───

    #[test]
    fn tc_tool_003_output_ok() {
        let out = ToolOutput::ok("result".into());
        assert!(!out.is_error);
        assert_eq!(out.text, "result");
    }

    #[test]
    fn tc_tool_004_output_error() {
        let out = ToolOutput::error("failed".into());
        assert!(out.is_error);
        assert_eq!(out.text, "failed");
    }

    #[test]
    fn tc_tool_005_output_from_string() {
        let out: ToolOutput = "hello".to_string().into();
        assert!(!out.is_error);
        assert_eq!(out.text, "hello");
    }

    // ─── ToolRegistry ───

    #[test]
    fn tc_tool_006_registry_empty() {
        let registry = ToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn tc_tool_007_registry_register_and_count() {
        let registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        registry.register(Box::new(StaticTool {
            name: "static".into(),
            text: "hello".into(),
        }));
        assert_eq!(registry.count(), 2);
        assert!(!registry.is_empty());
    }

    #[tokio::test]
    async fn tc_tool_008_registry_call_echo() {
        let registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let result = registry.call("echo", "test input").await.unwrap();
        assert!(!result.is_error);
        assert_eq!(result.text, "Echo: test input");
    }

    #[tokio::test]
    async fn tc_tool_009_registry_call_unknown() {
        let registry = ToolRegistry::new();
        let result = registry.call("nonexistent", "").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn tc_tool_010_registry_call_error_tool() {
        let registry = ToolRegistry::new();
        registry.register(Box::new(ErrorTool));
        let result = registry.call("fail", "").await.unwrap();
        assert!(result.is_error);
        assert_eq!(result.text, "Something went wrong");
    }

    #[test]
    fn tc_tool_011_registry_list_definitions() {
        let registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        registry.register(Box::new(StaticTool {
            name: "static".into(),
            text: "hello".into(),
        }));
        let defs = registry.list_definitions();
        assert_eq!(defs.len(), 2);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"echo"));
        assert!(names.contains(&"static"));
    }

    #[test]
    fn tc_tool_012_registry_debug_format() {
        let registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("echo"));
    }

    #[tokio::test]
    async fn tc_tool_013_portable_tool_default_enabled() {
        let tool = EchoTool;
        assert!(tool.enabled());
    }
}
