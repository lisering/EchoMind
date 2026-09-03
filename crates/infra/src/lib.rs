//! EchoMind 适配器层（六边形架构 infra）：SQLite 持久化、本地向量化引擎。
//! 本 crate 实现 `crates/core` 定义的端口 Trait，依赖方向：infra → core，严禁反转。

/// 磁盘空间检查模块（P1-1）：跨平台磁盘可用空间检查 + 临时文件清理。
///
/// R3 复盘发现磁盘满是最大弹性缺口（评分 2/10）。本模块提供预防性检查
/// 和自动清理，避免 SQLite 写入时遇到 SQLITE_FULL 错误。
pub mod disk_space;
/// DuckDuckGo Instant Answer API 适配器（REQ-RAG-036）：免费网页搜索，无需 API Key。
pub mod duckduckgo_provider;
/// 文件监听器（REQ-SYNC-003）：基于 notify + notify-debouncer-full 的跨平台实时文件监听。
pub mod file_watcher;
/// HyDE 查询改写适配器（REQ-RAG-021）：使用 OpenAI 兼容 LLM 生成假设性答案文档。
pub mod hyde_rewriter;
pub mod local_embedder;
/// 本地日志系统（REQ-OBS-001）：基于 tracing 的结构化日志 + JSON Lines 格式 + 日期轮转 + 运行时级别控制。
pub mod local_logger;
pub mod local_reranker;
/// MCP 传输层实现（REQ-ARCH-016 AC-2/AC-3）：`StdioTransport` + `SseTransport`。
pub mod mcp_transport;
pub mod openai_provider;
/// 性能基准测试框架（借鉴 Zed `util_macros::perf`）：统一计时、吞吐量计算、断言。
pub mod perf;
pub mod sqlite_storage;

// ---- Pro 模块（仅在 --features pro 时编译）----
/// HNSW 近似最近邻索引（REQ-NFR-005 + REQ-PERF-013）。
///
/// 基于 `hnsw_rs` crate（Malkov & Yashunin 2016/2018 论文纯 Rust 实现），
/// 将向量检索复杂度从 O(n) 暴力扫描降至 O(log n)。
///
/// **已从 Pro 下沉到 Free**（REQ-PERF-013）：大知识库（>500 chunks）自动使用 HNSW，
/// 小知识库保持全量扫描（阈值切换，无需用户配置）。
pub mod hnsw_index;
#[cfg(feature = "pro")]
pub mod ocrs_engine;
#[cfg(feature = "pro")]
pub mod openai_vision;
#[cfg(feature = "pro")]
pub mod pdfium_renderer;
/// 代码符号引擎（REQ-RAG-031 代码感知 RAG, Pro feature）：tree-sitter AST 级符号抽取。
#[cfg(feature = "pro")]
pub mod symbol_engine;
/// WASM 沙箱执行器（REQ-RAG-032 代码片段执行, Pro feature）：安全执行代码片段并返回结果。
///
/// 借鉴 CodeForge 多语言执行器：Agent 在回答代码相关问题时可执行代码片段验证结果。
/// 第一阶段 MVP 使用本地进程执行（python3/node），后续替换为 WASM 沙箱。
#[cfg(feature = "pro")]
pub mod wasm_executor;

/// bge-m3 多语言嵌入模型 TDD 测试（REQ-VEC-015）。
#[cfg(test)]
mod bge_m3_tests;
#[cfg(test)]
mod embedder_tests;
/// HNSW 向量索引下沉 Free TDD 测试（REQ-PERF-013）。
#[cfg(test)]
mod hnsw_free_tests;
#[cfg(test)]
mod hyde_rewriter_tests;
#[cfg(test)]
mod local_logger_tests;
#[cfg(test)]
mod local_reranker_tests;
/// 真实文件端到端性能测试（使用 lisp-rs/README_zh.md + 真实 ONNX embedder）。
#[cfg(test)]
mod perf_real_e2e_tests;
/// 全面性能测试套件（REQ-NFR-002 + RAG 管线全链路基准）。
#[cfg(test)]
mod perf_suite_tests;
#[cfg(test)]
mod storage_tests;
/// 嵌入模型下载境内加速 TDD 测试（REQ-VEC-017, v2.3）。
#[cfg(test)]
mod vec_accel_tests;
/// 向量缓存正确性修复 + 归一化点积 TDD 测试（REQ-PERF-016/017, v2.2）。
#[cfg(test)]
mod vec_cache_tests;
/// 代码符号引擎 TDD 测试（REQ-RAG-031, Pro feature）。
#[cfg(all(test, feature = "pro"))]
mod symbol_engine_tests;
/// WASM 沙箱执行器 TDD 测试（REQ-RAG-032, Pro feature）。
#[cfg(all(test, feature = "pro"))]
mod wasm_executor_tests;
