//! 网页搜索集成（REQ-RAG-036，DuckDuckGo Instant Answer API）。
use super::super::*;

/// 切换网页搜索开关（REQ-RAG-036）。
///
/// 持久化到 settings 表 `rag.web_search_enabled` 键，下次 chat 命令调用时即时生效。
/// 启用后，当本地检索 top-1 score < 阈值（0.3）时，自动调用 DuckDuckGo API 搜索互联网，
/// 将搜索结果通过 RRF 融合到本地检索结果中，在 prompt 中标注来源（🌐 Web）。
/// 搜索失败时优雅降级为仅使用本地结果。默认关闭（opt-in）。
#[tauri::command]
pub async fn set_web_search_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_web_search_enabled_inner(enabled, state.inner()).await
}

/// 网页搜索开关写入逻辑（命令与集成测试复用）。
pub async fn set_web_search_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.web_search_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 执行网页搜索（REQ-RAG-036）。
///
/// 直接调用 DuckDuckGo Instant Answer API 搜索互联网，返回搜索结果列表。
/// 用于前端搜索面板或独立搜索功能（非 chat 管线内的自动触发）。
///
/// # 参数
/// - `query`: 搜索查询文本
///
/// # 返回
/// 搜索结果列表（`SearchResult`），按相关性降序排列。
/// 搜索失败返回空列表（不报错）。
#[tauri::command]
pub async fn web_search(
    query: String,
    _state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    web_search_inner(&query).await
}

/// 网页搜索逻辑（命令与集成测试复用）。
pub async fn web_search_inner(query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let provider = DuckDuckGoProvider::new().map_err(|e| format!("{e:#}"))?;
    let provider_arc: Arc<dyn echomind_core::WebSearchProvider> = Arc::new(provider);
    provider_arc
        .search(query)
        .await
        .map_err(|e| format!("{e:#}"))
}
