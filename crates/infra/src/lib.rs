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
/// 本地 LLM 模型文件管理器（REQ-LLM-004）：下载、删除、列表本地 GGUF 模型。
/// MCP 传输层实现（REQ-ARCH-016 AC-2/AC-3）：`StdioTransport` + `SseTransport`。
pub mod mcp_transport;
pub mod model_manager;
pub mod openai_provider;
/// 性能基准测试框架（借鉴 Zed `util_macros::perf`）：统一计时、吞吐量计算、断言。
pub mod perf;
/// Prompt Prefix 磁盘缓存 + LRU 驱逐（DS-01：借鉴 ds4 `ds4_kvstore.c`）。
///
/// 缓存 tokenize 后的 prompt prefix，加速会话恢复。SHA1 文本前缀匹配、
/// LRU 驱逐评分（半衰期 6 小时）、边界裁剪 + 对齐（防 BPE 重分词）、
/// 4 种保存时机（cold/continued/evict/shutdown）、原子写入、预算管理。
pub mod prompt_cache;
/// 健壮下载系统（REQ-LLM-004 v2）：断点续传 + 多源容错 + 并发分块 + 崩溃恢复 + SHA256 校验。
///
/// 参考：Ollama download.go + HuggingFace hf_transfer + HuggingFace Hub file_download.py。
/// 统一 ONNX 嵌入模型和 GGUF 大模型的下载管线。
pub mod robust_downloader;
pub mod sqlite_storage;

// ---- Pro 模块（仅在 --features pro 时编译）----
/// 自研 GEMV 内核（Phase 3）：单批次推理优化的量化矩阵-向量乘法。
#[cfg(feature = "pro")]
pub mod gemv_kernel;
/// GGUF 文件解析器（Phase 3：自研量化内核 + 内存层次流式加载）。
///
/// 独立于 candle-core 的 GGUF 格式解析器，支持 mmap 零拷贝读取。
#[cfg(feature = "pro")]
pub mod gguf_reader;
/// HNSW 近似最近邻索引（REQ-NFR-005 + REQ-PERF-013）。
///
/// 基于 `hnsw_rs` crate（Malkov & Yashunin 2016/2018 论文纯 Rust 实现），
/// 将向量检索复杂度从 O(n) 暴力扫描降至 O(log n)。
///
/// **已从 Pro 下沉到 Free**（REQ-PERF-013）：大知识库（>500 chunks）自动使用 HNSW，
/// 小知识库保持全量扫描（阈值切换，无需用户配置）。
pub mod hnsw_index;
/// KV cache 序列化/反序列化（Phase 4 Session 24）。
///
/// 设计磁盘格式用于保存/恢复 transformer 模型的 Key-Value 缓存快照，
/// 消除多轮对话中重复前缀计算的开销。
#[cfg(feature = "pro")]
pub mod kv_cache;
/// Layer 级流式预取（Phase 3 Session 21）。
///
/// 在推理进行时后台预取下一层权重到 RAM，利用 `madvise(MADV_WILLNEED)`
/// 告知 OS 后台异步加载页面到 page cache。
#[cfg(feature = "pro")]
pub mod layer_prefetch;
/// 本地 LLM 推理引擎（REQ-LLM-003）：基于 mistral.rs 的纯 Rust 本地推理。
#[cfg(feature = "pro")]
pub mod local_llm;
/// RAM 预算管理（Phase 3 Session 22）。
///
/// 在模型 > 可用 RAM 时自动管理内存：分配/释放预算、LRU 驱逐旧层、预取新层。
/// 使用 `libc::sysconf` 获取系统内存（无需 sysinfo 依赖）。
#[cfg(feature = "pro")]
pub mod memory_budget;
#[cfg(feature = "pro")]
pub mod ocrs_engine;
#[cfg(feature = "pro")]
pub mod openai_vision;
#[cfg(feature = "pro")]
pub mod pdfium_renderer;
/// 量化块结构与反量化内核（Phase 3）。
///
/// 定义 GGUF 量化格式的块结构体（BlockQ4_0/BlockQ4K/BlockQ5K/BlockQ6K/BlockQ8_0/BlockQ8K）
/// 并提供反量化（dequantize）方法，从 mmap 数据零拷贝提取量化权重块。
#[cfg(feature = "pro")]
pub mod quant_blocks;
/// 代码符号引擎（REQ-RAG-031 代码感知 RAG, Pro feature）：tree-sitter AST 级符号抽取。
#[cfg(feature = "pro")]
pub mod symbol_engine;
/// WASM 沙箱执行器（REQ-RAG-032 代码片段执行, Pro feature）：安全执行代码片段并返回结果。
///
/// 借鉴 CodeForge 多语言执行器：Agent 在回答代码相关问题时可执行代码片段验证结果。
/// 第一阶段 MVP 使用本地进程执行（python3/node），后续替换为 WASM 沙箱。
#[cfg(feature = "pro")]
pub mod wasm_executor;
/// CPU cache 友好的权重重排（Phase 3 Session 19）。
///
/// 将 GGUF 量化权重从行优先布局重排为 Tile-Major 布局，减少 GEMV 推理时的
/// L1/L2 cache miss。核心策略：将所有行的同一列 Block 连续存储，使每个输入
/// Block 从 L2 只读取一次，在 L1 中被 N 行复用。
#[cfg(feature = "pro")]
pub mod weight_repack;

/// bge-m3 多语言嵌入模型 TDD 测试（REQ-VEC-015）。
#[cfg(test)]
mod bge_m3_tests;
#[cfg(test)]
mod embedder_tests;
#[cfg(all(test, feature = "pro"))]
mod gemv_kernel_tests;
#[cfg(all(test, feature = "pro"))]
mod gguf_reader_tests;
/// HNSW 向量索引下沉 Free TDD 测试（REQ-PERF-013）。
#[cfg(test)]
mod hnsw_free_tests;
#[cfg(test)]
mod hyde_rewriter_tests;
#[cfg(all(test, feature = "pro"))]
mod kv_cache_tests;
#[cfg(all(test, feature = "pro"))]
mod layer_prefetch_tests;
#[cfg(all(test, feature = "pro"))]
mod local_llm_tests;
#[cfg(test)]
mod local_logger_tests;
#[cfg(test)]
mod local_reranker_tests;
#[cfg(all(test, feature = "pro"))]
mod memory_budget_tests;
#[cfg(test)]
mod model_manager_tests;
/// 真实文件端到端性能测试（使用 lisp-rs/README_zh.md + 真实 ONNX embedder）。
#[cfg(test)]
mod perf_real_e2e_tests;
/// 全面性能测试套件（REQ-NFR-002 + RAG 管线全链路基准）。
#[cfg(test)]
mod perf_suite_tests;
/// Prompt Prefix 磁盘缓存 TDD 测试（DS-01：借鉴 ds4 `ds4_kvstore.c`）。
#[cfg(test)]
mod prompt_cache_tests;
#[cfg(all(test, feature = "pro"))]
mod quant_blocks_tests;
#[cfg(test)]
mod storage_tests;
/// 嵌入模型下载境内加速 TDD 测试（REQ-VEC-017, v2.3）。
#[cfg(test)]
mod vec_accel_tests;
/// 向量缓存正确性修复 + 归一化点积 TDD 测试（REQ-PERF-016/017, v2.2）。
#[cfg(test)]
mod vec_cache_tests;
#[cfg(all(test, feature = "pro"))]
mod weight_repack_tests;

/// GEMV 内核 + 权重重排端到端集成测试（Phase 3 CI 自动化）。
#[cfg(all(test, feature = "pro"))]
mod gemv_integration_tests;
/// LocalLlmEngine 状态机 Mock 测试（Phase 3 CI 自动化）。
#[cfg(all(test, feature = "pro"))]
mod local_llm_mock_tests;
/// 代码符号引擎 TDD 测试（REQ-RAG-031, Pro feature）。
#[cfg(all(test, feature = "pro"))]
mod symbol_engine_tests;
/// 合成 GGUF 测试 fixture 生成器 + V1/V2/V3 解析器集成测试（Phase 3 CI 自动化）。
#[cfg(all(test, feature = "pro"))]
mod synthetic_gguf_tests;
/// WASM 沙箱执行器 TDD 测试（REQ-RAG-032, Pro feature）。
#[cfg(all(test, feature = "pro"))]
mod wasm_executor_tests;
