//! performance 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
//
// Phase 1 R1 大简化重构：删除了所有学术 RAG 优化命令（缓存/压缩/Proposition/
// SummaryTree/ColBERT/检索记忆/Speculative/渐进式注入/嵌入对比/LateChunking）。
// 仅保留嵌入重建、BM25 索引重建、Contextual Retrieval 开关。
use super::*;

// ============================================================================
// Contextual Retrieval（REQ-RAG-041：上下文增强嵌入）
// ============================================================================

/// 切换 Contextual Retrieval 开关（REQ-RAG-041）。
///
/// 写入 settings 表 `rag.contextual_retrieval`，控制嵌入时是否拼接文档名上下文前缀。
/// 开启时嵌入文本 = `build_contextual_text(doc_name, chunk_content)`（含文档名前缀）；
/// 关闭时嵌入文本 = 纯 chunk content（不含前缀）。
///
/// 默认 true（嵌入管线已使用上下文文本）。变更后需重启或调用
/// `rebuild_contextual_embeddings` 重建已有嵌入向量。
#[tauri::command]
pub async fn set_contextual_retrieval(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_contextual_retrieval_inner(enabled, state.inner()).await
}

/// Contextual Retrieval 开关写入逻辑（命令与集成测试复用）。
pub async fn set_contextual_retrieval_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.contextual_retrieval", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 重建 BM25 全文索引（REQ-PERF-005 Contextual BM25）。
///
/// 清空并重建 FTS5 索引，使用 `build_contextual_text()` 拼接文档名前缀，
/// 提升精确匹配查询命中率（Anthropic Contextual Retrieval：失败率 ↓49%）。
///
/// 旧数据库升级到 Contextual BM25 时由用户手动触发。
#[tauri::command]
pub async fn rebuild_bm25_index(state: State<'_, AppState>) -> Result<(), String> {
    rebuild_bm25_index_inner(state.inner()).await
}

/// BM25 索引重建逻辑（命令与集成测试复用）。
pub async fn rebuild_bm25_index_inner(state: &AppState) -> Result<(), String> {
    state
        .storage
        .rebuild_bm25_index()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 重建上下文增强嵌入（REQ-RAG-041）。
///
/// 遍历所有已索引文档，对每个文档的 chunks 重新计算嵌入向量。
/// 嵌入文本根据当前 `contextual_retrieval` 设置决定是否拼接文档名前缀。
/// 适用于用户切换 Contextual Retrieval 开关后重建已有向量。
#[tauri::command]
pub async fn rebuild_contextual_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    rebuild_contextual_embeddings_inner(&app, state.inner()).await
}

/// 上下文嵌入重建逻辑（命令与集成测试复用）。
pub async fn rebuild_contextual_embeddings_inner<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let total = docs.len();
    let mut completed = 0usize;

    for doc in &docs {
        // 跳过非 Indexed 状态的文档（Failed/Pending/Processing）
        if !matches!(doc.status, DocStatus::Indexed) {
            completed += 1;
            continue;
        }
        let name = display_name(&doc.file_path);
        emit_status(app, "indexing", format!("正在重建上下文嵌入：{name}"));

        match super::import::embed_document_chunks(app, state, &doc.id, &name).await {
            Ok(count) => {
                completed += 1;
                emit_status(
                    app,
                    "done",
                    format!("上下文嵌入重建完成：{name}（{count} 向量）[{completed}/{total}]"),
                );
            }
            Err(err) => {
                warn!("上下文嵌入重建失败（doc_id={}）: {err}", doc.id);
            }
        }
    }

    state.step_cache.clear();

    Ok(())
}

/// 重建全库嵌入向量（REQ-VEC-016）。
///
/// 遍历所有 Indexed 状态文档，使用当前嵌入模型重新计算全部 chunks 的嵌入向量。
/// 适用于用户切换嵌入模型后（如从 bge-small-en-v1.5 切换到 bge-m3）的维度迁移。
/// 重建过程中通过 `doc-status-changed` 事件推送进度，重建完成后清空查询缓存。
#[tauri::command]
pub async fn rebuild_all_embeddings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    rebuild_all_embeddings_inner(&app, state.inner()).await
}

/// 全库嵌入重建逻辑（命令与集成测试复用）。
///
/// 与 `rebuild_contextual_embeddings_inner` 的区别：
/// - `rebuild_contextual_embeddings`：仅重建上下文前缀（Contextual Retrieval 开关变更后使用）
/// - `rebuild_all_embeddings`：全库重建（嵌入模型切换后维度迁移使用）
///
/// 两者底层都调用 `embed_document_chunks`，但 `rebuild_all_embeddings`
/// 会强制重置 embedder 实例（确保使用新模型）。
pub async fn rebuild_all_embeddings_inner<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    // 强制重置 embedder 实例（确保使用切换后的新模型）
    state.model_store.reset_embedder().await;

    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let total = docs.len();
    let mut completed = 0usize;
    let mut failed = 0usize;

    for doc in &docs {
        // 跳过非 Indexed 状态的文档（Failed/Pending/Processing）
        if !matches!(doc.status, DocStatus::Indexed) {
            completed += 1;
            continue;
        }
        let name = display_name(&doc.file_path);
        emit_status(app, "indexing", format!("正在重建嵌入：{name}"));

        match super::import::embed_document_chunks(app, state, &doc.id, &name).await {
            Ok(count) => {
                completed += 1;
                emit_status(
                    app,
                    "done",
                    format!("嵌入重建完成：{name}（{count} 向量）[{completed}/{total}]"),
                );
            }
            Err(err) => {
                failed += 1;
                warn!("嵌入重建失败（doc_id={}）: {err}", doc.id);
                emit_status(
                    app,
                    "error",
                    format!("嵌入重建失败：{name}（已处理 {completed}/{total}）"),
                );
            }
        }
    }

    state.step_cache.clear();

    if failed > 0 {
        emit_status(
            app,
            "done",
            format!("全库嵌入重建完成（{completed}/{total} 成功，{failed} 失败）"),
        );
    } else {
        emit_status(app, "done", format!("全库嵌入重建完成（{total} 个文档）"));
    }

    Ok(())
}
