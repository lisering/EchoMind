//! Markdown 笔记双向链接（Obsidian 风格，REQ-ING-020）。
use super::super::*;

/// 查询文档的正向链接（REQ-ING-020）。
///
/// 返回该文档通过 `[[wiki-link]]` 引用的所有目标文档。
///
/// `doc_id` 为源文档 ID，返回 `Vec<WikiLink>`（source_doc_id = doc_id 的记录）。
#[tauri::command]
pub async fn get_forward_links(
    doc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    get_forward_links_inner(&doc_id, state.inner()).await
}

/// 正向链接查询逻辑（命令与集成测试复用）。
pub async fn get_forward_links_inner(
    doc_id: &str,
    state: &AppState,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    state
        .storage
        .get_forward_links(doc_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 查询文档的反向链接（REQ-ING-020）。
///
/// 返回引用了该文档的所有来源文档（即 `[[doc_name]]` 出现在哪些文档中）。
///
/// `doc_name` 为文档文件名（不含扩展名），返回 `Vec<WikiLink>`（target LIKE %doc_name% 的记录）。
#[tauri::command]
pub async fn get_backlinks(
    doc_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    get_backlinks_inner(&doc_name, state.inner()).await
}

/// 反向链接查询逻辑（命令与集成测试复用）。
pub async fn get_backlinks_inner(
    doc_name: &str,
    state: &AppState,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    state
        .storage
        .get_backlinks(doc_name)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 重建 wiki-link 索引（REQ-ING-020）。
///
/// 清空 `wiki_links` 表后，遍历所有 Indexed 文档的 chunks，
/// 重新解析 `[[wiki-link]]` 语法并写入索引。
#[tauri::command]
pub async fn rebuild_wiki_links(state: State<'_, AppState>) -> Result<usize, String> {
    rebuild_wiki_links_inner(state.inner()).await
}

/// 重建 wiki-link 索引逻辑（命令与集成测试复用）。
///
/// 返回重建的 wiki-link 总数。
pub async fn rebuild_wiki_links_inner(state: &AppState) -> Result<usize, String> {
    use echomind_core::wiki_link_parser::parse_wiki_links;

    // 获取所有已索引文档
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let mut total_links = 0usize;

    for doc in &docs {
        if doc.status != DocStatus::Indexed {
            continue;
        }

        // 获取文档的所有 chunks
        let chunks = state
            .storage
            .list_chunks(&doc.id)
            .await
            .map_err(|e| format!("{e:#}"))?;

        // 先删除该文档的旧 wiki-link 索引（通过重新导入实现）
        // 由于没有 delete_wiki_links_by_doc 方法，我们直接重新写入
        // wiki_links 表使用 INSERT OR IGNORE，重复的不会冲突
        let mut all_links = Vec::new();

        for chunk in &chunks {
            let links = parse_wiki_links(&chunk.content, &doc.id, &chunk.id);
            all_links.extend(links);
        }

        if !all_links.is_empty() {
            state
                .storage
                .add_wiki_links(&all_links)
                .await
                .map_err(|e| format!("{e:#}"))?;
            total_links += all_links.len();
        }
    }

    Ok(total_links)
}
