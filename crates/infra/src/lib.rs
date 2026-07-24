//! EchoMind 适配器层（六边形架构 infra）：SQLite 持久化、本地向量化引擎。
//! 本 crate 实现 `crates/core` 定义的端口 Trait，依赖方向：infra → core，严禁反转。

pub mod local_embedder;
pub mod openai_provider;
pub mod sqlite_storage;

#[cfg(test)]
mod embedder_tests;
#[cfg(test)]
mod storage_tests;
