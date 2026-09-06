//! 代码执行沙箱数据契约。
//!
//! 对应 SRS：REQ-RAG-032 代码片段执行（Pro feature）。

use serde::{Deserialize, Serialize};

/// 代码执行结果（REQ-RAG-032）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub timed_out: bool,
}

/// 代码执行器配置（REQ-RAG-032）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeExecutorConfig {
    pub timeout_secs: u64,
    pub memory_limit_mb: usize,
    pub allow_network: bool,
}

impl Default for CodeExecutorConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 10,
            memory_limit_mb: 64,
            allow_network: false,
        }
    }
}

impl CodeExecutorConfig {
    pub const MAX_TIMEOUT_SECS: u64 = 30;
    pub const MAX_MEMORY_MB: usize = 256;

    pub fn safe_new(timeout_secs: u64, memory_limit_mb: usize, _allow_network: bool) -> Self {
        Self {
            timeout_secs: timeout_secs.min(Self::MAX_TIMEOUT_SECS),
            memory_limit_mb: memory_limit_mb.min(Self::MAX_MEMORY_MB),
            allow_network: false,
        }
    }
}
