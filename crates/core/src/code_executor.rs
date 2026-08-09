//! 代码执行沙箱核心实现（REQ-RAG-032，Pro feature）。
//!
//! 本模块定义 `MockExecutor`（测试用）和 `NoExecutor`（Free 版本占位）。
//! 真正的 WASM 沙箱执行器 `WasmExecutor` 在 `crates/infra/src/wasm_executor.rs` 中实现（Pro 门控）。
//!
//! ## 安全设计
//!
//! - `CodeExecutorConfig` 硬编码安全上限（超时 30s / 内存 256MB / 网络永远禁用）
//! - `NoExecutor` 始终返回错误，确保 Free 版本无法执行代码
//! - `MockExecutor` 仅返回预定义结果，不实际执行任何代码

use echomind_models::{CodeExecutorConfig, ExecutionResult};

use crate::CodeExecutor;

/// Mock 执行器（测试用）。
///
/// 返回预定义的 `ExecutionResult`，不实际执行任何代码。
/// 用于 Agent 集成测试中验证 `execute_code` 工具的行为。
pub struct MockExecutor {
    /// 预定义的执行结果
    result: ExecutionResult,
    /// 支持的语言列表
    languages: Vec<&'static str>,
    /// 执行器配置
    config: CodeExecutorConfig,
}

impl MockExecutor {
    /// 创建 Mock 执行器：指定返回结果和支持语言。
    pub fn new(result: ExecutionResult, languages: Vec<&'static str>) -> Self {
        Self {
            result,
            languages,
            config: CodeExecutorConfig::default(),
        }
    }

    /// 创建 Mock 执行器并指定配置。
    pub fn with_config(
        result: ExecutionResult,
        languages: Vec<&'static str>,
        config: CodeExecutorConfig,
    ) -> Self {
        Self {
            result,
            languages,
            config,
        }
    }
}

impl CodeExecutor for MockExecutor {
    fn execute<'a>(
        &'a self,
        _code: &'a str,
        _language: &'a str,
        _stdin: Option<&'a str>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<ExecutionResult>> + Send + 'a>,
    > {
        Box::pin(async move {
            let result = self.result.clone();
            Ok(result)
        })
    }

    fn supported_languages(&self) -> Vec<&str> {
        self.languages.clone()
    }

    fn config(&self) -> CodeExecutorConfig {
        self.config.clone()
    }
}

/// 不支持代码执行的占位实现（Free 版本用）。
///
/// `execute()` 始终返回错误，确保 Free 版本无法执行代码。
/// 设计为零大小类型 + `Send + Sync`，可安全跨线程共享。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoExecutor;

impl CodeExecutor for NoExecutor {
    fn execute<'a>(
        &'a self,
        _code: &'a str,
        _language: &'a str,
        _stdin: Option<&'a str>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<ExecutionResult>> + Send + 'a>,
    > {
        Box::pin(async move { anyhow::bail!("代码执行需要 Pro 版本") })
    }

    fn supported_languages(&self) -> Vec<&str> {
        Vec::new()
    }

    fn config(&self) -> CodeExecutorConfig {
        CodeExecutorConfig::default()
    }
}

/// 将 `ExecutionResult` 格式化为 Agent Observation 字符串。
///
/// 格式：
/// ```text
/// 执行结果 (exit_code=0, 123ms):
/// stdout:
/// 2
///
/// stderr:
/// (空)
/// ```
pub fn format_execution_result(result: &ExecutionResult) -> String {
    let stderr_display = if result.stderr.is_empty() {
        "(空)"
    } else {
        &result.stderr
    };

    if result.timed_out {
        format!(
            "执行结果 (超时, {}ms):\nstdout:\n{}\nstderr:\n{}",
            result.duration_ms, result.stdout, stderr_display
        )
    } else {
        format!(
            "执行结果 (exit_code={}, {}ms):\nstdout:\n{}\nstderr:\n{}",
            result.exit_code, result.duration_ms, result.stdout, stderr_display
        )
    }
}
