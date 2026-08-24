//! MCP（Model Context Protocol）协议适配器（REQ-ARCH-016）。
//!
//! 实现 MCP 客户端支持，允许 EchoMind 连接外部 MCP 服务器扩展工具能力。
//!
//! ## 架构
//!
//! - `config` — 服务器配置管理（CRUD + 序列化）
//! - `transport` — JSON-RPC 传输层抽象（stdio + SSE）
//! - `client` — MCP JSON-RPC 2.0 客户端（initialize + list_tools + call_tool）
//! - `registry` — 多服务器管理 + 工具聚合注册表
//!
//! ## 安全
//!
//! - MCP 工具调用前必须经用户确认（AC-5）
//! - 工具描述被视为不可信内容
//! - 服务器配置加密存储在 settings 表

pub mod client;
pub mod config;
pub mod registry;
pub mod transport;
