//! Trace 系统 IPC 命令子模块（S70：Cherry Studio 借鉴）。
//!
//! 前端通过这些命令获取 RAG 链路追踪数据，在设置面板可视化展示。

use super::*;
use echomind_core::trace::TraceRecord;

/// 获取最近的 trace 记录列表。
///
/// # 参数
/// - `limit` — 返回最近 N 条记录（默认 20，最大 50）
#[tauri::command]
pub async fn get_recent_traces(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<TraceRecord>, String> {
    let limit = limit.unwrap_or(20).min(50);
    let store = state.trace_store.read().await;
    Ok(store.recent(limit).await)
}

/// 获取指定 ID 的 trace 记录详情。
#[tauri::command]
pub async fn get_trace_detail(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<TraceRecord>, String> {
    let store = state.trace_store.read().await;
    Ok(store.get(&id).await)
}

/// 清空所有 trace 记录。
#[tauri::command]
pub async fn clear_traces(state: State<'_, AppState>) -> Result<(), String> {
    let store = state.trace_store.read().await;
    store.clear().await;
    Ok(())
}

/// 获取 trace 记录数量。
#[tauri::command]
pub async fn get_trace_count(state: State<'_, AppState>) -> Result<usize, String> {
    let store = state.trace_store.read().await;
    Ok(store.count().await)
}
