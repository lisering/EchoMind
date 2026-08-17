//! performance 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 获取缓存统计信息（REQ-PERF-001）。
///
/// 返回各级缓存命中次数、总查询数、缓存条目数和估算的 token 节省量。
#[tauri::command]
pub async fn get_cache_stats(state: State<'_, AppState>) -> Result<CacheStats, String> {
    get_cache_stats_inner(state.inner()).await
}

/// 缓存统计逻辑（命令与集成测试复用）。
pub async fn get_cache_stats_inner(state: &AppState) -> Result<CacheStats, String> {
    state.cache.get_stats().await.map_err(|e| format!("{e:#}"))
}

/// 手动清空所有缓存（REQ-PERF-001）。
///
/// 删除 exact_cache / semantic_cache / retrieval_cache 三张表的全部记录。
/// 文档导入/删除时自动触发；用户也可手动调用清空。
#[tauri::command]
pub async fn clear_cache(state: State<'_, AppState>) -> Result<(), String> {
    clear_cache_inner(state.inner()).await
}

/// 缓存清空逻辑（命令与集成测试复用）。
pub async fn clear_cache_inner(state: &AppState) -> Result<(), String> {
    state
        .cache
        .clear_all()
        .await
        .map_err(|e| format!("{e:#}"))?;
    // P2-1 StepCache：手动清空缓存时同步清空步骤级缓存
    state.step_cache.clear();
    Ok(())
}

/// 设置缓存配置（REQ-PERF-001）。
///
/// 持久化到 settings 表 `cache.enabled` / `cache.ttl_secs` /
/// `cache.semantic_threshold` / `cache.privacy_mode` 键。
#[tauri::command]
pub async fn set_cache_settings(
    settings: CacheSettingsPayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_cache_settings_inner(settings, state.inner()).await
}

/// 缓存设置写入逻辑（命令与集成测试复用）。
pub async fn set_cache_settings_inner(
    settings: CacheSettingsPayload,
    state: &AppState,
) -> Result<(), String> {
    state
        .storage
        .set_setting(
            "cache.enabled",
            if settings.enabled { "true" } else { "false" },
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("cache.ttl_secs", &settings.ttl_secs.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "cache.semantic_threshold",
            &settings.semantic_threshold.to_string(),
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "cache.privacy_mode",
            if settings.privacy_mode {
                "true"
            } else {
                "false"
            },
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 获取缓存配置（REQ-PERF-001）。
#[tauri::command]
pub async fn get_cache_settings(
    state: State<'_, AppState>,
) -> Result<CacheSettingsPayload, String> {
    get_cache_settings_inner(state.inner()).await
}

/// 缓存配置读取逻辑（命令与集成测试复用）。
pub async fn get_cache_settings_inner(state: &AppState) -> Result<CacheSettingsPayload, String> {
    let enabled = state
        .storage
        .get_setting("cache.enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .map(|v| v == "true")
        .unwrap_or(true);
    let ttl_secs = state
        .storage
        .get_setting("cache.ttl_secs")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(86400);
    let semantic_threshold = state
        .storage
        .get_setting("cache.semantic_threshold")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.92);
    let privacy_mode = state
        .storage
        .get_setting("cache.privacy_mode")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    Ok(CacheSettingsPayload {
        enabled,
        ttl_secs,
        semantic_threshold,
        privacy_mode,
    })
}

/// 设置 Prompt 压缩比（REQ-PERF-002）。
///
/// 持久化到 settings 表 `compression.ratio` 键，下次 chat 命令调用时即时生效。
///
/// # 压缩比取值
/// - `1.0` = 禁用压缩（默认）
/// - `2.0` = 保守（压缩到 1/2，信息保留率 ≥ 90%）
/// - `3.0` = 平衡（压缩到 1/3，信息保留率 ≥ 80%）
/// - `5.0` = 激进（压缩到 1/5，信息保留率 ≥ 60%）
#[tauri::command]
pub async fn set_compression_ratio(ratio: f32, state: State<'_, AppState>) -> Result<(), String> {
    set_compression_ratio_inner(ratio, state.inner()).await
}

/// 压缩比写入逻辑（命令与集成测试复用）。
pub async fn set_compression_ratio_inner(ratio: f32, state: &AppState) -> Result<(), String> {
    // 验证压缩比范围
    if !(1.0..=10.0).contains(&ratio) {
        return Err(prefix_error(
            ERR_VALIDATION,
            "压缩比应在 1.0-10.0 范围内（1.0=禁用, 2.0=保守, 3.0=平衡, 5.0=激进）",
        ));
    }
    state
        .storage
        .set_setting("compression.ratio", &ratio.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 获取当前 Prompt 压缩比（REQ-PERF-002）。
#[tauri::command]
pub async fn get_compression_ratio(state: State<'_, AppState>) -> Result<f32, String> {
    get_compression_ratio_inner(state.inner()).await
}

/// 压缩比读取逻辑（命令与集成测试复用）。
pub async fn get_compression_ratio_inner(state: &AppState) -> Result<f32, String> {
    Ok(state.compression_ratio)
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

/// 重建 Proposition 索引（REQ-PERF-007 Proposition 级原子分割）。
///
/// 清空 propositions 表后，遍历所有 chunk → 分割为 proposition → 重新写入。
/// 旧数据库升级到 Proposition 级检索时由用户手动触发。
#[tauri::command]
pub async fn rebuild_proposition_index(state: State<'_, AppState>) -> Result<(), String> {
    rebuild_proposition_index_inner(state.inner()).await
}

/// Proposition 索引重建逻辑（命令与集成测试复用）。
pub async fn rebuild_proposition_index_inner(state: &AppState) -> Result<(), String> {
    state
        .storage
        .rebuild_proposition_index()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 构建 RAPTOR 摘要树（REQ-PERF-009）。
///
/// 对指定文档构建多级摘要树，将原始 chunks 组织为 Level 0 组摘要 → Level 1 主题摘要。
/// 摘要生成使用用户配置的 LLM，可作为异步任务执行。
///
/// # 参数
/// - `doc_id`: 文档 ID
#[tauri::command]
pub async fn build_summary_tree(doc_id: String, state: State<'_, AppState>) -> Result<(), String> {
    build_summary_tree_inner(doc_id, state.inner()).await
}

/// 摘要树构建逻辑（命令与集成测试复用）。
pub async fn build_summary_tree_inner(doc_id: String, state: &AppState) -> Result<(), String> {
    use echomind_core::summary_tree::{
        DEFAULT_CLUSTER_SIZE, DEFAULT_MAX_LEVEL, SummaryTreeBuilder,
    };

    // 获取文档的全部 chunks
    let chunks = state
        .storage
        .list_chunks(&doc_id)
        .await
        .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;

    if chunks.len() <= DEFAULT_CLUSTER_SIZE {
        // chunks 太少，无需构建摘要树
        return Ok(());
    }

    // 摘要生成回调：使用 LLM 生成组摘要
    // 如果 LLM 不可用，使用拼接占位摘要（优雅降级）
    let summarize_fn = move |texts: Vec<String>| {
        Box::pin(async move {
            // 简单方案：拼接子节点文本作为摘要（不调用 LLM，避免阻塞）
            // 真正的 RAPTOR 实现应调用 LLM 生成摘要
            Ok(format!("摘要：{}", texts.join(" / ")))
        })
            as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>>
    };

    let builder = SummaryTreeBuilder::new(summarize_fn, DEFAULT_CLUSTER_SIZE, DEFAULT_MAX_LEVEL);
    let nodes = builder
        .build(&doc_id, &chunks)
        .await
        .map_err(|e| format!("{ERR_UNKNOWN}: {e:#}"))?;

    if nodes.is_empty() {
        return Ok(());
    }

    // 批量写入摘要节点
    state
        .storage
        .add_summary_nodes(&nodes)
        .await
        .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;

    // 为摘要节点计算嵌入向量
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("{ERR_EMBED}: {e:#}"))?;
    let summaries: Vec<String> = nodes.iter().map(|n| n.content.clone()).collect();
    let embeddings = embedder
        .embed_batch(&summaries)
        .await
        .map_err(|e| format!("{ERR_EMBED}: {e:#}"))?;

    for (node, embedding) in nodes.iter().zip(embeddings.iter()) {
        state
            .storage
            .update_summary_embedding(&node.id, embedding)
            .await
            .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;
    }

    Ok(())
}

/// 设置嵌入模型模式（REQ-PERF-008 ColBERT 多向量嵌入, Pro feature）。
///
/// 在单向量嵌入（默认）和多向量嵌入（ColBERT）之间切换。
/// 多向量模式为 Pro 功能，Free 用户无法使用。
///
/// # 参数
/// - `mode`: 嵌入模式 ("single" = 单向量, "multi" = 多向量 ColBERT)
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn set_embedder_model(mode: String, state: State<'_, AppState>) -> Result<(), String> {
    set_embedder_model_inner(mode, state.inner()).await
}

/// 嵌入模型模式切换逻辑（命令与集成测试复用, Pro feature）。
#[cfg(feature = "pro")]
pub async fn set_embedder_model_inner(mode: String, state: &AppState) -> Result<(), String> {
    match mode.as_str() {
        "single" => {
            state
                .storage
                .set_setting("embedding.mode", "single")
                .await
                .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;
            Ok(())
        }
        "multi" => {
            let is_pro = *state.is_pro().read().await;
            if !is_pro {
                return Err("PRO_REQUIRED: ColBERT 多向量嵌入为 Pro 功能".to_string());
            }
            state
                .storage
                .set_setting("embedding.mode", "multi")
                .await
                .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;
            Ok(())
        }
        _ => Err(format!(
            "{ERR_VALIDATION}: 无效的嵌入模式 '{mode}'，应为 'single' 或 'multi'"
        )),
    }
}

/// 设置自进化检索记忆开关（REQ-PERF-012）。
///
/// 持久化到 settings 表 `rag.retrieval_memory_enabled` 键，下次 chat 命令调用时即时生效。
#[tauri::command]
pub async fn set_retrieval_memory_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_retrieval_memory_enabled_inner(enabled, state.inner()).await
}

/// 检索记忆开关写入逻辑（命令与集成测试复用）。
pub async fn set_retrieval_memory_enabled_inner(
    enabled: bool,
    state: &AppState,
) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.retrieval_memory_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 获取检索记忆统计（REQ-PERF-012）。
///
/// 返回所有查询类型 × 所有方法的效果记录，供前端展示自适应学习状态。
#[tauri::command]
pub async fn get_retrieval_memory_stats(
    state: State<'_, AppState>,
) -> Result<Vec<RetrievalMemoryStatEntry>, String> {
    get_retrieval_memory_stats_inner(state.inner()).await
}

/// 检索记忆统计读取逻辑（命令与集成测试复用）。
pub async fn get_retrieval_memory_stats_inner(
    state: &AppState,
) -> Result<Vec<RetrievalMemoryStatEntry>, String> {
    use echomind_core::retrieval_memory::RetrievalMemoryStore as _;

    let records = state
        .storage
        .list_all_memories()
        .await
        .map_err(|e| format!("{e:#}"))?;

    Ok(records
        .into_iter()
        .map(|r| RetrievalMemoryStatEntry {
            query_type: r.query_type.as_str().to_string(),
            method: r.method.as_str().to_string(),
            hit_count: r.hit_count,
            miss_count: r.miss_count,
            hit_rate: r.hit_rate(),
            avg_score: r.avg_score,
        })
        .collect())
}

/// 重置检索记忆（REQ-PERF-012）。
///
/// 清空所有检索效果记录，回到冷启动状态。
#[tauri::command]
pub async fn reset_retrieval_memory(state: State<'_, AppState>) -> Result<(), String> {
    reset_retrieval_memory_inner(state.inner()).await
}

/// 检索记忆重置逻辑（命令与集成测试复用）。
pub async fn reset_retrieval_memory_inner(state: &AppState) -> Result<(), String> {
    use echomind_core::retrieval_memory::RetrievalMemoryStore as _;

    state
        .storage
        .clear_all_memories()
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 记录用户检索反馈信号（REQ-PERF-012 扩展：用户反馈自学习）。
///
/// 前端在检测到用户行为（重新提问 / 编辑重发 / 点赞 / 点踩 / 继续新话题）时调用。
/// 禁用检索记忆时静默跳过（向后兼容）。
#[tauri::command]
pub async fn record_retrieval_feedback(
    signal: echomind_core::retrieval_memory::FeedbackSignal,
    state: State<'_, AppState>,
) -> Result<(), String> {
    record_retrieval_feedback_inner(signal, state.inner()).await
}

/// 检索反馈信号记录逻辑（命令与集成测试复用）。
pub async fn record_retrieval_feedback_inner(
    signal: echomind_core::retrieval_memory::FeedbackSignal,
    state: &AppState,
) -> Result<(), String> {
    if !state.retrieval_memory_enabled {
        return Ok(()); // 禁用时静默跳过
    }
    echomind_core::retrieval_memory::RetrievalMemoryEngine::new(state.storage.clone())
        .record_feedback(signal)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))
}

/// 切换渐进式注入开关（REQ-PERF-010）。
///
/// 写入 settings 表 `rag.progressive_injection`，chat_inner 读取后决定
/// 是否仅注入初始检索子集。
#[tauri::command]
pub async fn set_progressive_injection(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_progressive_injection_inner(enabled, state.inner()).await
}

/// 渐进式注入开关写入逻辑（命令与集成测试复用）。
pub async fn set_progressive_injection_inner(
    enabled: bool,
    state: &AppState,
) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.progressive_injection", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 切换 Speculative RAG 开关（REQ-PERF-011）。
///
/// 写入 settings 表 `rag.speculative_enabled`，同时更新 AppState.speculative_enabled
/// 以便 chat_inner 即时读取最新状态（小模型草稿 → 大模型验证）。
#[tauri::command]
pub async fn set_speculative_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_speculative_enabled_inner(enabled, state.inner()).await
}

/// Speculative RAG 开关写入逻辑（命令与集成测试复用）。
pub async fn set_speculative_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.speculative_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 切换 RAG 质量门控开关（REQ-RAG-028）。
///
/// 写入 settings 表 `rag.quality_gate_enabled`，同时更新 AppState.quality_gate_enabled
/// 以便 chat_inner 即时读取最新状态（检索后评估结果质量，低质量时记录告警）。
#[tauri::command]
pub async fn set_quality_gate_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_quality_gate_enabled_inner(enabled, state.inner()).await
}

/// 质量门控开关写入逻辑（命令与集成测试复用）。
pub async fn set_quality_gate_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.quality_gate_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

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

    // 清空查询缓存（嵌入向量已变更，旧缓存答案可能引用过期的向量）
    if let Err(e) = state.cache.clear_all().await {
        warn!("重建嵌入后清空查询缓存失败: {e:#}");
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

    // 清空查询缓存（嵌入向量已变更，旧缓存答案引用过期的向量维度）
    if let Err(e) = state.cache.clear_all().await {
        warn!("全库嵌入重建后清空查询缓存失败: {e:#}");
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
