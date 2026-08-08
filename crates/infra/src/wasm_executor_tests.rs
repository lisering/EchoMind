#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! TDD 集成测试：TC-EXEC-INT-001~004 WASM 沙箱执行器（REQ-RAG-032, Pro feature）。
//!
//! 测试策略：
//! - 需要真实 python3/node 安装的测试标记为 `#[ignore]`
//! - CI 环境可能没有解释器，手动运行：`cargo test --features pro -- wasm_executor_tests --ignored --nocapture`

use echomind_core::CodeExecutor;
use echomind_models::CodeExecutorConfig;

use crate::wasm_executor::WasmExecutor;

/// TC-EXEC-INT-001：Python 代码执行（需 python3 安装）。
///
/// 输入: `print(1+1)` → stdout == "2\n", exit_code == 0
#[tokio::test]
#[ignore = "需要 python3 已安装"]
async fn tc_exec_int_001_python_execution() {
    let executor = WasmExecutor::with_defaults();
    let result = executor
        .execute("print(1+1)", "python", None)
        .await
        .expect("执行失败");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("2"));
    assert!(!result.timed_out);
}

/// TC-EXEC-INT-002：超时 kill。
///
/// 输入: `import time; time.sleep(100)` → timed_out == true
#[tokio::test]
#[ignore = "需要 python3 已安装"]
async fn tc_exec_int_002_timeout_kill() {
    let config = CodeExecutorConfig {
        timeout_secs: 2,
        memory_limit_mb: 64,
        allow_network: false,
    };
    let executor = WasmExecutor::new(config);
    let result = executor
        .execute("import time; time.sleep(100)", "python", None)
        .await
        .expect("执行失败");

    assert!(result.timed_out);
    assert_eq!(result.exit_code, -1);
}

/// TC-EXEC-INT-003：stderr 捕获。
///
/// 输入: `import sys; sys.stderr.write("error")` → stderr 包含 "error"
#[tokio::test]
#[ignore = "需要 python3 已安装"]
async fn tc_exec_int_003_stderr_capture() {
    let executor = WasmExecutor::with_defaults();
    let result = executor
        .execute("import sys; sys.stderr.write(\"error\\n\")", "python", None)
        .await
        .expect("执行失败");

    assert!(result.stderr.contains("error"));
}

/// TC-EXEC-INT-004：stdin 传入。
///
/// 输入: `name = input(); print(f"Hello, {name}")`  stdin = "World"
/// 断言: stdout 包含 "Hello, World"
#[tokio::test]
#[ignore = "需要 python3 已安装"]
async fn tc_exec_int_004_stdin_input() {
    let executor = WasmExecutor::with_defaults();
    let result = executor
        .execute(
            "name = input(); print(f\"Hello, {name}\")",
            "python",
            Some("World"),
        )
        .await
        .expect("执行失败");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("Hello, World"));
}

/// TC-EXEC-INT-005：不支持的语言返回错误。
#[tokio::test]
async fn tc_exec_int_005_unsupported_language() {
    let executor = WasmExecutor::with_defaults();
    let result = executor.execute("echo hello", "bash", None).await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("不支持的语言"));
}

/// TC-EXEC-INT-006：supported_languages 返回正确列表。
#[test]
fn tc_exec_int_006_supported_languages() {
    let executor = WasmExecutor::with_defaults();
    let langs = executor.supported_languages();

    assert!(langs.contains(&"python"));
    assert!(langs.contains(&"javascript"));
}

/// TC-EXEC-INT-007：config 返回正确配置。
#[test]
fn tc_exec_int_007_config() {
    let config = CodeExecutorConfig {
        timeout_secs: 5,
        memory_limit_mb: 32,
        allow_network: false,
    };
    let executor = WasmExecutor::new(config.clone());
    let actual = executor.config();

    assert_eq!(actual.timeout_secs, 5);
    assert_eq!(actual.memory_limit_mb, 32);
    assert!(!actual.allow_network);
}

/// TC-EXEC-INT-008：JavaScript 代码执行（需 node 安装）。
///
/// 输入: `console.log(1+1)` → stdout 包含 "2"
#[tokio::test]
#[ignore = "需要 node 已安装"]
async fn tc_exec_int_008_javascript_execution() {
    let executor = WasmExecutor::with_defaults();
    let result = executor
        .execute("console.log(1+1)", "javascript", None)
        .await
        .expect("执行失败");

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("2"));
}
