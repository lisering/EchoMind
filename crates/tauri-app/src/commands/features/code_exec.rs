//! 代码执行沙箱（REQ-RAG-032，Pro 门控）。
use super::super::*;

/// 执行代码片段并返回结果（Pro feature，REQ-RAG-032）。
///
/// 在安全沙箱中执行代码片段（Python / JavaScript），返回 stdout / stderr / exit_code。
/// 安全限制：超时 10s、内存 64MB、无网络访问。
///
/// Free 版本调用此命令返回 Pro 错误。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn execute_code_snippet(
    _state: State<'_, AppState>,
    code: String,
    language: String,
    stdin: Option<String>,
) -> Result<ExecutionResult, String> {
    execute_code_snippet_inner(&code, &language, stdin.as_deref())
        .await
        .map_err(|e| format!("{e:#}"))
}

/// Free 版本：返回 Pro 错误。
#[cfg(not(feature = "pro"))]
#[tauri::command]
pub async fn execute_code_snippet(
    _state: State<'_, AppState>,
    _code: String,
    _language: String,
    _stdin: Option<String>,
) -> Result<ExecutionResult, String> {
    Err(prefix_error(ERR_PRO_REQUIRED, "代码执行需要 Pro 版本"))
}

/// 代码执行内部实现（Pro 版本）。
#[cfg(feature = "pro")]
async fn execute_code_snippet_inner(
    code: &str,
    language: &str,
    stdin: Option<&str>,
) -> anyhow::Result<ExecutionResult> {
    use echomind_core::CodeExecutor;
    use echomind_infra::wasm_executor::WasmExecutor;

    let executor = WasmExecutor::with_defaults();
    executor.execute(code, language, stdin).await
}
