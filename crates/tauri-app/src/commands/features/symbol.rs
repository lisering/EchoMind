//! 代码符号搜索（tree-sitter AST，REQ-RAG-031，Pro 门控）。
use super::super::*;

/// 搜索代码符号（精确 + 模糊匹配，REQ-RAG-031）。
///
/// 精确匹配优先：在 `code_symbols` 表中按 `name` 精确查找；
/// 若结果不足，追加模糊匹配（`name LIKE '%query%'`）。
#[tauri::command]
pub async fn search_symbols(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::CodeSymbol>, String> {
    search_symbols_inner(query, limit, state.inner()).await
}

/// 符号搜索逻辑（命令与集成测试复用）。
pub async fn search_symbols_inner(
    query: String,
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<echomind_models::CodeSymbol>, String> {
    let max = limit.unwrap_or(20);
    let mut results = state
        .storage
        .search_by_symbol(&query, None)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 精确匹配不足时追加模糊匹配
    if results.len() < max {
        let fuzzy = state
            .storage
            .search_symbols_fuzzy(&query, max)
            .await
            .map_err(|e| format!("{e:#}"))?;
        // 去重：跳过已精确匹配的符号（owned strings 避免借用冲突）
        let existing: std::collections::HashSet<String> =
            results.iter().map(|s| s.name.clone()).collect();
        for sym in fuzzy {
            if !existing.contains(&sym.name) {
                results.push(sym);
            }
            if results.len() >= max {
                break;
            }
        }
    }

    results.truncate(max);
    Ok(results)
}

/// 获取指定 chunk 的所有代码符号（REQ-RAG-031）。
#[tauri::command]
pub async fn get_symbols_for_chunk(
    chunk_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::CodeSymbol>, String> {
    state
        .storage
        .get_symbols_for_chunk(&chunk_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 重建代码符号索引（REQ-RAG-031）。
///
/// 遍历所有已索引的代码文件（.rs/.ts/.tsx/.py/.go），
/// 重新通过 tree-sitter AST 抽取符号并写入 `code_symbols` 表。
/// Pro 专用功能。
#[tauri::command]
#[cfg(feature = "pro")]
pub async fn rebuild_symbol_index(state: State<'_, AppState>) -> Result<usize, String> {
    rebuild_symbol_index_inner(state.inner()).await
}

/// 符号索引重建逻辑（命令与集成测试复用，Pro feature）。
#[cfg(feature = "pro")]
pub async fn rebuild_symbol_index_inner(state: &AppState) -> Result<usize, String> {
    use echomind_core::SymbolExtractor;

    let engine = SymbolEngine;
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let code_exts = ["rs", "ts", "tsx", "py", "go"];
    let mut total_symbols = 0usize;

    for doc in &docs {
        // 仅处理代码文件
        let ext = Path::new(&doc.file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        if !code_exts.contains(&ext.as_str()) {
            continue;
        }

        // 检测语言
        let language = match engine.detect_language(&doc.file_path) {
            Some(lang) => lang,
            None => continue,
        };

        // 获取该文档的所有 chunk
        let chunks = state
            .storage
            .list_chunks(&doc.id)
            .await
            .map_err(|e| format!("{e:#}"))?;

        // 为每个 chunk 抽取符号
        let mut all_symbols = Vec::new();
        for chunk in &chunks {
            let symbols = engine.extract_symbols(&chunk.content, &language, &chunk.id);
            all_symbols.extend(symbols);
        }

        if !all_symbols.is_empty() {
            total_symbols += all_symbols.len();
            state
                .storage
                .add_symbols(&all_symbols)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
    }

    Ok(total_symbols)
}
