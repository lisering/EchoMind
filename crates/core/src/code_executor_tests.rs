#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! TDD 测试：TC-EXEC-001~008 代码执行沙箱（REQ-RAG-032）。
//!
//! 测试策略：
//! - MockExecutor：返回预定义结果，验证 trait 行为
//! - NoExecutor：验证 Free 版本占位返回错误
//! - Agent 集成测试：验证 execute_code 工具调用和优雅降级

use std::sync::Arc;

use echomind_models::{CodeExecutorConfig, ExecutionResult};

use crate::CodeExecutor;
use crate::code_executor::{MockExecutor, NoExecutor, format_execution_result};

// ================== TC-EXEC-001: Mock 执行器正确返回结果 ==================

/// TC-EXEC-001：MockExecutor.execute() 返回预定义的 ExecutionResult。
#[tokio::test]
async fn tc_exec_001_mock_executor_returns_result() {
    let expected = ExecutionResult {
        stdout: "2\n".to_string(),
        stderr: String::new(),
        exit_code: 0,
        duration_ms: 42,
        timed_out: false,
    };
    let executor = MockExecutor::new(expected.clone(), vec!["python", "javascript"]);

    let result = executor.execute("print(1+1)", "python", None).await;

    assert!(result.is_ok());
    let actual = result.unwrap();
    assert_eq!(actual, expected);
}

// ================== TC-EXEC-002: 超时处理 ==================

/// TC-EXEC-002：MockExecutor 返回 timed_out=true 时，调用方正确处理超时。
#[tokio::test]
async fn tc_exec_002_timeout_handling() {
    let expected = ExecutionResult {
        stdout: String::new(),
        stderr: "Execution timed out".to_string(),
        exit_code: -1,
        duration_ms: 10000,
        timed_out: true,
    };
    let executor = MockExecutor::new(expected.clone(), vec!["python"]);

    let result = executor
        .execute("import time; time.sleep(100)", "python", None)
        .await;

    assert!(result.is_ok());
    let actual = result.unwrap();
    assert!(actual.timed_out);
    assert_eq!(actual.exit_code, -1);
}

// ================== TC-EXEC-003: NoExecutor 返回错误 ==================

/// TC-EXEC-003：NoExecutor.execute() 返回 Err("代码执行需要 Pro 版本")。
#[tokio::test]
async fn tc_exec_003_no_executor_returns_error() {
    let executor = NoExecutor;

    let result = executor.execute("print('hello')", "python", None).await;

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Pro"));
}

// ================== TC-EXEC-004: stderr 正确捕获 ==================

/// TC-EXEC-004：MockExecutor 返回 stderr 非空时，ExecutionResult.stderr 正确捕获。
#[tokio::test]
async fn tc_exec_004_stderr_captured() {
    let expected = ExecutionResult {
        stdout: String::new(),
        stderr: "error output".to_string(),
        exit_code: 1,
        duration_ms: 5,
        timed_out: false,
    };
    let executor = MockExecutor::new(expected, vec!["python"]);

    let result = executor
        .execute("import sys; sys.stderr.write('error')", "python", None)
        .await
        .unwrap();

    assert!(!result.stderr.is_empty());
    assert!(result.stderr.contains("error"));
}

// ================== TC-EXEC-005: Agent 集成——execute_code 工具 ==================

/// TC-EXEC-005：Agent 使用 execute_code 工具时，MockExecutor 返回正确结果。
///
/// 通过 `CodeExecutor` trait 直接验证：
/// 1. 创建 MockExecutor 返回 stdout="2"
/// 2. 调用 execute() 后格式化为 Observation 字符串
/// 3. 断言 Observation 包含 "2"
#[tokio::test]
async fn tc_exec_005_agent_execute_code_tool() {
    let result = ExecutionResult {
        stdout: "2\n".to_string(),
        stderr: String::new(),
        exit_code: 0,
        duration_ms: 10,
        timed_out: false,
    };
    let executor: Arc<dyn CodeExecutor> = Arc::new(MockExecutor::new(
        result.clone(),
        vec!["python", "javascript", "rust"],
    ));

    // 模拟 Agent 调用 execute_code 工具
    let exec_result = executor
        .execute("print(1+1)", "python", None)
        .await
        .unwrap();

    // 格式化为 Observation 字符串
    let observation = format_execution_result(&exec_result);

    // 断言 Observation 包含执行输出
    assert!(observation.contains("2"));
    assert!(observation.contains("exit_code=0"));
}

// ================== TC-EXEC-006: Agent 无 executor 时优雅降级 ==================

/// TC-EXEC-006：NoExecutor（Free 版本）调用 execute 返回 Pro 错误，不崩溃。
#[tokio::test]
async fn tc_exec_006_no_executor_graceful_degradation() {
    let executor: Arc<dyn CodeExecutor> = Arc::new(NoExecutor);

    // 模拟 Agent 调用 execute_code 工具
    let result = executor.execute("print('hello')", "python", None).await;

    // 断言返回错误（优雅降级，不 panic）
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Pro"));

    // 验证 supported_languages 返回空（Free 版本不支持任何语言）
    assert!(executor.supported_languages().is_empty());
}

// ================== TC-EXEC-007: CodeExecutorConfig 默认值 ==================

/// TC-EXEC-007：CodeExecutorConfig 默认值正确（timeout=10s, memory=64MB, network=false）。
#[test]
fn tc_exec_007_config_defaults() {
    let config = CodeExecutorConfig::default();

    assert_eq!(config.timeout_secs, 10);
    assert_eq!(config.memory_limit_mb, 64);
    assert!(!config.allow_network);
}

// ================== TC-EXEC-008: 安全限制上限 ==================

/// TC-EXEC-008：CodeExecutorConfig::safe_new() 应用安全限制上限。
#[test]
fn tc_exec_008_config_safe_limits() {
    // 超时超过上限 → 截断为 30s
    let config = CodeExecutorConfig::safe_new(999, 64, false);
    assert_eq!(config.timeout_secs, CodeExecutorConfig::MAX_TIMEOUT_SECS);

    // 内存超过上限 → 截断为 256MB
    let config = CodeExecutorConfig::safe_new(10, 99999, false);
    assert_eq!(config.memory_limit_mb, CodeExecutorConfig::MAX_MEMORY_MB);

    // allow_network 永远为 false（即使传入 true）
    let config = CodeExecutorConfig::safe_new(10, 64, true);
    assert!(!config.allow_network);

    // 正常值不受影响
    let config = CodeExecutorConfig::safe_new(5, 32, false);
    assert_eq!(config.timeout_secs, 5);
    assert_eq!(config.memory_limit_mb, 32);
    assert!(!config.allow_network);
}

// ================== TC-EXEC-009: format_execution_result 格式验证 ==================

/// TC-EXEC-009：format_execution_result 正确格式化执行结果。
#[test]
fn tc_exec_009_format_result() {
    // 正常执行
    let result = ExecutionResult {
        stdout: "42".to_string(),
        stderr: String::new(),
        exit_code: 0,
        duration_ms: 100,
        timed_out: false,
    };
    let formatted = format_execution_result(&result);
    assert!(formatted.contains("exit_code=0"));
    assert!(formatted.contains("100ms"));
    assert!(formatted.contains("42"));
    assert!(formatted.contains("(空)")); // 空 stderr 显示 "(空)"

    // 超时
    let result = ExecutionResult {
        stdout: String::new(),
        stderr: "timeout".to_string(),
        exit_code: -1,
        duration_ms: 10000,
        timed_out: true,
    };
    let formatted = format_execution_result(&result);
    assert!(formatted.contains("超时"));
    assert!(formatted.contains("10000ms"));
}

// ================== TC-EXEC-010: MockExecutor config 方法 ==================

/// TC-EXEC-010：MockExecutor.config() 返回正确配置。
#[tokio::test]
async fn tc_exec_010_mock_config() {
    let custom_config = CodeExecutorConfig {
        timeout_secs: 5,
        memory_limit_mb: 32,
        allow_network: false,
    };
    let executor = MockExecutor::with_config(
        ExecutionResult {
            stdout: "ok".to_string(),
            stderr: String::new(),
            exit_code: 0,
            duration_ms: 1,
            timed_out: false,
        },
        vec!["python"],
        custom_config.clone(),
    );

    let config = executor.config();
    assert_eq!(config.timeout_secs, 5);
    assert_eq!(config.memory_limit_mb, 32);
    assert!(!config.allow_network);
}

// ================== TC-EXEC-011: stdin 传入验证 ==================

/// TC-EXEC-011：execute() 接受 stdin 参数（Mock 验证签名兼容性）。
#[tokio::test]
async fn tc_exec_011_stdin_parameter() {
    let result = ExecutionResult {
        stdout: "Hello, World".to_string(),
        stderr: String::new(),
        exit_code: 0,
        duration_ms: 5,
        timed_out: false,
    };
    let executor = MockExecutor::new(result.clone(), vec!["python"]);

    // 传入 stdin
    let actual = executor
        .execute(
            "name = input(); print(f'Hello, {name}')",
            "python",
            Some("World"),
        )
        .await
        .unwrap();

    assert_eq!(actual, result);
}
