//! document 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 列出全部文档（知识库面板）。
#[tauri::command]
pub async fn get_documents(state: State<'_, AppState>) -> Result<Vec<Document>, String> {
    state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除文档：级联清理 chunks / embeddings，并删除数据目录中的文件副本（REQ-ING-005）。
///
/// **注意**：删除文档不影响 `conversations` 和 `messages` 表数据（REQ-ING-005-AC-3）。
#[tauri::command]
pub async fn delete_document(id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_document_inner(&id, state.inner()).await
}

/// 删除文档逻辑（命令与集成测试复用）。
///
/// 级联清理 chunks / embeddings，并删除数据目录中的文件副本。
/// **不触碰** conversations 和 messages 表（REQ-ING-005-AC-3）。
pub async fn delete_document_inner(id: &str, state: &AppState) -> Result<(), String> {
    // 先取副本路径再删记录（删除后无法回查）
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let file_path = docs
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.file_path.clone());

    state
        .storage
        .delete_document(id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 副本可能已不存在，记录日志但不视为失败
    if let Some(path) = file_path.as_ref()
        && let Err(err) = tokio::fs::remove_file(path).await
    {
        warn!("文档副本清理失败（可忽略）: {path}: {err}");
    }

    // **性能/一致性**：删除文档 → 按文档名精确失效相关缓存条目
    // （新鲜度感知：保留引用其他文档的有效缓存命中，替代全库清空）
    if let Some(fp) = &file_path {
        let doc_name = Path::new(fp)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Err(e) = state.cache.invalidate_by_doc(&doc_name).await {
            warn!("删除文档后失效缓存失败: {e:#}");
        }
    }
    // P2-1 StepCache：删除文档 → 清空步骤级缓存（检索结果可能引用被删文档）
    state.step_cache.clear();
    Ok(())
}

/// 对文档执行领域分类并持久化结果（REQ-VEC-013）。
///
/// 取文档前 5 个 chunk 的嵌入均值与 16 领域质心做余弦相似度比较，
/// 将分类结果写入 `documents.domain` 列。
/// 分类失败时设为 `"general"`（AC-5 优雅降级）。
pub(crate) async fn classify_and_update_domain(
    storage: &SqliteStorage,
    embedder: &echomind_infra::local_embedder::LocalEmbedder,
    doc_id: &str,
) -> anyhow::Result<()> {
    let chunks = storage.list_chunks(doc_id).await?;
    if chunks.is_empty() {
        storage.update_document_domain(doc_id, "general").await?;
        return Ok(());
    }
    let chunk_texts: Vec<String> = chunks.iter().take(5).map(|c| c.content.clone()).collect();
    let classifier = EmbeddingDomainClassifier::new(embedder.clone()).await?;
    let domain = classifier.classify(&chunk_texts).await?;
    storage.update_document_domain(doc_id, &domain).await?;
    Ok(())
}

/// 重新分类指定文档的领域（REQ-VEC-013 AC-7）。
#[tauri::command]
pub async fn reclassify_document(
    doc_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    reclassify_document_inner(&doc_id, state.inner()).await
}

/// `reclassify_document` 的逻辑实现（命令与集成测试复用）。
///
/// 获取文档 chunks → 初始化分类器 → 分类 → 持久化 → 返回领域标识。
pub async fn reclassify_document_inner(doc_id: &str, state: &AppState) -> Result<String, String> {
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?;
    classify_and_update_domain(&state.storage, &embedder, doc_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    // 重新读取分类结果
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let doc = docs
        .into_iter()
        .find(|d| d.id == doc_id)
        .ok_or_else(|| format!("文档不存在: {doc_id}"))?;
    Ok(doc.domain.unwrap_or_else(|| "general".to_string()))
}

/// 异步生成文档摘要的辅助函数（导入管线后台调用）。
///
/// 使用 `ImportService::generate_summary()` 生成摘要并持久化。
/// 失败时返回 `Err`，调用方应静默降级（不影响导入流程）。
pub(crate) async fn generate_doc_summary_async(
    storage: &SqliteStorage,
    doc_id: &str,
    chunks: &[echomind_models::Chunk],
    provider: &OpenAIProvider,
) -> anyhow::Result<()> {
    let service = ImportService::new(storage.clone(), std::path::PathBuf::new());
    service.generate_summary(doc_id, chunks, provider).await
}

/// 获取文档摘要（REQ-ING-019）。
///
/// 返回 `documents.summary` 列的值。若摘要尚未生成（导入时 LLM 未配置或生成失败），
/// 返回 `Ok(None)`。
#[tauri::command]
pub async fn get_document_summary(
    doc_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    get_document_summary_inner(&doc_id, state.inner()).await
}

/// `get_document_summary` 的逻辑实现（命令与集成测试复用）。
pub async fn get_document_summary_inner(
    doc_id: &str,
    state: &AppState,
) -> Result<Option<String>, String> {
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let doc = docs
        .into_iter()
        .find(|d| d.id == doc_id)
        .ok_or_else(|| format!("文档不存在: {doc_id}"))?;
    Ok(doc.summary)
}

/// 重新生成文档摘要（REQ-ING-019）。
///
/// 获取文档 chunks → 初始化 LLM → 调用 `ImportService::generate_summary()` → 返回新摘要。
/// 适用于用户切换 LLM 后重新生成摘要的场景。
#[tauri::command]
pub async fn regenerate_summary(
    doc_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    regenerate_summary_inner(&doc_id, state.inner()).await
}

/// `regenerate_summary` 的逻辑实现（命令与集成测试复用）。
///
/// 获取文档 chunks → 初始化 LLM Provider → 生成摘要 → 持久化 → 返回摘要文本。
pub async fn regenerate_summary_inner(doc_id: &str, state: &AppState) -> Result<String, String> {
    // 获取文档 chunks
    let chunks = state
        .storage
        .list_chunks(doc_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if chunks.is_empty() {
        return Err("文档无分块内容，无法生成摘要".to_string());
    }

    // 初始化 LLM Provider
    let llm_config = state.llm_config().read().await.clone();
    let provider = match llm_config {
        Some(cfg) => OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model)
            .map_err(|e| format!("LLM 初始化失败: {e:#}"))?,
        None => return Err("未配置 LLM：请完成初始配置向导".to_string()),
    };

    // 生成摘要
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    service
        .generate_summary(doc_id, &chunks, &provider)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 返回新生成的摘要
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let doc = docs
        .into_iter()
        .find(|d| d.id == doc_id)
        .ok_or_else(|| format!("文档不存在: {doc_id}"))?;
    Ok(doc.summary.unwrap_or_default())
}

// ------------------------------------------------------------------
// 文档标签系统（REQ-ING-022 用户自定义标签管理）
// ------------------------------------------------------------------

/// 添加文档标签（REQ-ING-022）。
///
/// 将 `tag` 追加到指定文档的标签列表。标签已存在时幂等跳过。
#[tauri::command]
pub async fn add_document_tag(
    doc_id: String,
    tag: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    add_document_tag_inner(&doc_id, &tag, state.inner()).await
}

/// `add_document_tag` 的逻辑实现（命令与集成测试复用）。
pub async fn add_document_tag_inner(
    doc_id: &str,
    tag: &str,
    state: &AppState,
) -> Result<(), String> {
    state
        .storage
        .add_document_tag(doc_id, tag)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 移除文档标签（REQ-ING-022）。
///
/// 从指定文档的标签列表中移除 `tag`。标签不存在时幂等返回。
#[tauri::command]
pub async fn remove_document_tag(
    doc_id: String,
    tag: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    remove_document_tag_inner(&doc_id, &tag, state.inner()).await
}

/// `remove_document_tag` 的逻辑实现（命令与集成测试复用）。
pub async fn remove_document_tag_inner(
    doc_id: &str,
    tag: &str,
    state: &AppState,
) -> Result<(), String> {
    state
        .storage
        .remove_document_tag(doc_id, tag)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 列出所有文档标签（REQ-ING-022）。
///
/// 返回去重后的标签列表，每个标签附带使用该标签的文档数。
/// 按文档数降序排列。
#[tauri::command]
pub async fn list_all_tags(state: State<'_, AppState>) -> Result<Vec<(String, usize)>, String> {
    list_all_tags_inner(state.inner()).await
}

/// `list_all_tags` 的逻辑实现（命令与集成测试复用）。
pub async fn list_all_tags_inner(state: &AppState) -> Result<Vec<(String, usize)>, String> {
    state
        .storage
        .list_all_tags()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 按标签筛选文档（REQ-ING-022）。
///
/// 返回 `tags` 列中包含指定标签的所有文档。
#[tauri::command]
pub async fn filter_documents_by_tag(
    tag: String,
    state: State<'_, AppState>,
) -> Result<Vec<Document>, String> {
    filter_documents_by_tag_inner(&tag, state.inner()).await
}

/// `filter_documents_by_tag` 的逻辑实现（命令与集成测试复用）。
pub async fn filter_documents_by_tag_inner(
    tag: &str,
    state: &AppState,
) -> Result<Vec<Document>, String> {
    state
        .storage
        .filter_documents_by_tag(tag)
        .await
        .map_err(|e| format!("{e:#}"))
}
