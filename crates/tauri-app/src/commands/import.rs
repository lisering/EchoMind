//! import 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 获取文件大小列表（REQ-ING-013）：前端在导入前调用此命令检查文件大小，
/// 对超限文件分别处理（警告 / 拒绝），不影响其他文件。
/// 返回 Vec<(路径, 大小字节)> ；文件不存在或不可访问时大小为 0。
#[tauri::command]
pub async fn get_file_sizes(paths: Vec<String>) -> Result<Vec<(String, u64)>, String> {
    let mut result = Vec::with_capacity(paths.len());
    for path in &paths {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        result.push((path.clone(), size));
    }
    Ok(result)
}

/// 获取文件大小警告阈值与硬上限（REQ-ING-013）：前端用于显示阈值信息。
#[tauri::command]
pub async fn get_file_size_limits() -> Result<(u64, u64), String> {
    Ok((FILE_SIZE_WARN_THRESHOLD, FILE_SIZE_HARD_LIMIT))
}

/// 导入文件。返回成功入库的原始文件名列表；格式非法 / 配额触顶 / 未授权 PDF / 路径遍历
/// 等错误经 `Err(可读消息)` 返回。is_pro 从 AppState 读取（REQ-LIC-002 双重拦截）。
#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    import_files_inner(&app, &paths, state.inner()).await
}

/// 导入逻辑（命令与集成测试复用）。
pub async fn import_files_inner<R: Runtime>(
    app: &AppHandle<R>,
    paths: &[String],
    state: &AppState,
) -> Result<Vec<String>, String> {
    let is_pro = *state.is_pro().read().await;
    // REQ-WS-002：获取当前工作空间 ID，配额按工作空间独立计算
    let workspace_id = crate::commands::workspace::get_current_workspace_inner(state)
        .await
        .unwrap_or_else(|_| "default".to_string());
    // REQ-VEC-011：从 settings 表读取分块参数
    let chunk_params = crate::commands::settings::get_chunk_params_inner(state)
        .await
        .unwrap_or_default();
    let service = if chunk_params.chunk_size != echomind_core::import::DEFAULT_CHUNK_TOKENS {
        let mut svc = ImportService::with_chunk_tokens(
            state.storage.clone(),
            state.data_dir.clone(),
            chunk_params.chunk_size,
        );
        svc.workspace_id = workspace_id.clone();
        svc
    } else {
        ImportService::with_workspace(state.storage.clone(), state.data_dir.clone(), workspace_id)
    };
    let mut imported = Vec::new();

    // REQ-ING-023：文件夹拖拽导入 — 检测路径是否为目录，
    // 若是则递归遍历子目录中所有支持格式的文件。
    let expanded_paths = expand_folder_paths(paths);

    // REQ-ING-006：导入进度与取消
    let total = expanded_paths.len();
    let cancel_flag = state.import_cancel_flag();
    state.reset_import_cancel(); // 开始前重置取消标志

    for (idx, raw_path) in expanded_paths.iter().enumerate() {
        let name = display_name(raw_path);

        // 检查取消标志（文件边界退出，已完成部分保留，不产生半成品索引）
        if cancel_flag.load(std::sync::atomic::Ordering::SeqCst) {
            emit_import_progress(app, imported.len(), total, &name, true);
            break;
        }

        // REQ-ING-013：文件大小硬上限检查（安全网，前端已检查但后端强制拦截）
        if let Ok(metadata) = std::fs::metadata(raw_path) {
            let file_size = metadata.len();
            if file_size > FILE_SIZE_HARD_LIMIT {
                emit_status(
                    app,
                    "error",
                    format!(
                        "文件过大，最大支持 10GB：{name}（{}GB）",
                        file_size / 1024 / 1024 / 1024
                    ),
                );
                continue;
            }
        }

        // 发射进度事件（当前文件信息）
        emit_import_progress(app, idx, total, &name, false);
        // 子阶段：正在读取文件
        emit_status_with_phase(
            app,
            "indexing",
            format!("正在读取：{name}"),
            Some("reading"),
        );

        match service.import_one(raw_path, is_pro).await {
            Ok(ImportOutcome::Imported(doc)) => {
                // 判断文件类型：PDF（多模态管线）/ 代码文件（符号感知分块）/ 普通文本
                let index_result = {
                    #[cfg(feature = "pro")]
                    {
                        let ext_lower = Path::new(&doc.file_path)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.to_lowercase())
                            .unwrap_or_default();
                        let is_pdf = ext_lower == "pdf";
                        // REQ-RAG-031：代码文件走符号感知分块路径（tree-sitter AST）
                        let is_code =
                            matches!(ext_lower.as_str(), "rs" | "ts" | "tsx" | "py" | "go");
                        if is_pdf && is_pro {
                            index_pdf_multimodal(app, &service, &doc, state, &name).await
                        } else if is_code && is_pro {
                            // 代码符号感知索引（REQ-RAG-031）：按函数/类边界分块 + 符号索引
                            emit_status(app, "indexing", format!("正在索引代码符号：{name}"));
                            let engine = SymbolEngine;
                            service
                                .index_document_with_symbols(&doc, &engine)
                                .await
                                .map_err(|e| format!("{e:#}"))
                        } else {
                            emit_status_with_phase(
                                app,
                                "indexing",
                                format!("正在分块索引：{name}"),
                                Some("splitting"),
                            );
                            service
                                .index_document(&doc)
                                .await
                                .map_err(|e| format!("{e:#}"))
                        }
                    }
                    #[cfg(not(feature = "pro"))]
                    {
                        emit_status_with_phase(
                            app,
                            "indexing",
                            format!("正在分块索引：{name}"),
                            Some("splitting"),
                        );
                        service
                            .index_document(&doc)
                            .await
                            .map_err(|e| format!("{e:#}"))
                    }
                };

                match index_result {
                    Ok(()) => {
                        // 子阶段：正在加载向量化引擎（首次加载 ONNX 模型可能耗时数秒）
                        emit_status_with_phase(
                            app,
                            "indexing",
                            "正在加载向量化引擎…".to_string(),
                            Some("loading_model"),
                        );
                        match embed_document_chunks(app, state, &doc.id, &name).await {
                            Ok(count) => {
                                emit_status(
                                    app,
                                    "done",
                                    format!("索引完成：{name}（{count} 向量）"),
                                );
                                // **性能优化**：embedder 统一获取一次，复用于领域分类和 proposition 嵌入。
                                // 原实现分别调用 state.embedder().await 两次（各触发 OnceCell 竞争），
                                // 现统一获取一次后 clone 到各 spawn 任务。
                                let shared_embedder = state.embedder().await.ok();

                                // REQ-VEC-013：后台异步执行领域分类（不阻塞导入完成事件）
                                let storage = state.storage.clone();
                                let doc_id = doc.id.clone();
                                let embedder_clone = shared_embedder.clone();
                                tokio::spawn(async move {
                                    if let Some(emb) = embedder_clone
                                        && let Err(e) =
                                            classify_and_update_domain(&storage, &emb, &doc_id)
                                                .await
                                    {
                                        warn!("领域分类失败（doc_id={doc_id}）: {e:#}");
                                    }
                                });
                                // REQ-ING-019：后台异步生成文档摘要（不阻塞导入完成事件）
                                let sum_storage = state.storage.clone();
                                let sum_doc_id = doc.id.clone();
                                let sum_llm_config = state.llm_config().read().await.clone();
                                tokio::spawn(async move {
                                    if let Some(cfg) = sum_llm_config
                                        && let Ok(provider) = OpenAIProvider::new(
                                            cfg.api_key,
                                            cfg.base_url,
                                            cfg.model,
                                        )
                                        && let Ok(chunks) =
                                            sum_storage.list_chunks(&sum_doc_id).await
                                        && let Err(e) = generate_doc_summary_async(
                                            &sum_storage,
                                            &sum_doc_id,
                                            &chunks,
                                            &provider,
                                        )
                                        .await
                                    {
                                        warn!("文档摘要生成失败（doc_id={sum_doc_id}）: {e:#}");
                                    }
                                });
                            }
                            Err(err) => {
                                let _ = state
                                    .storage
                                    .update_doc_status(
                                        &doc.id,
                                        DocStatus::Failed(prefix_error(
                                            ERR_EMBED,
                                            &format!("向量化失败: {err}"),
                                        )),
                                    )
                                    .await;
                                emit_status(
                                    app,
                                    "error",
                                    prefix_error(
                                        ERR_EMBED,
                                        &format!("向量化失败：{name}（{err}）"),
                                    ),
                                );
                            }
                        }
                    }
                    Err(err) => emit_status(
                        app,
                        "error",
                        prefix_error(ERR_PARSE, &format!("索引失败：{name}（{err}）")),
                    ),
                }
                imported.push(name);
            }
            Ok(ImportOutcome::SkippedDuplicate(_)) => {
                emit_status(app, "done", format!("内容已存在，跳过导入：{name}"));
            }
            Ok(ImportOutcome::NameConflict {
                old_doc_id,
                file_name,
            }) => {
                // REQ-ING-012：同名不同内容，返回冲突信息供前端确认
                emit_status(
                    app,
                    "conflict",
                    format!("文件已存在但内容不同：{file_name}"),
                );
                return Err(format!("CONFLICT:{old_doc_id}:{file_name}"));
            }
            Err(err) => {
                // REQ-ERR-001：未携带前缀的导入错误统一标记为 STORAGE
                let msg = err.to_string();
                if has_error_prefix(&msg) {
                    return Err(msg);
                }
                return Err(prefix_error(ERR_STORAGE, &msg));
            }
        }
    }

    Ok(imported)
}

/// 替换文档（REQ-ING-012）：删除旧文档 + 重新导入新文件 + 完整索引管线。
///
/// 用户导入同名不同内容文件时，前端收到 `CONFLICT:{old_doc_id}:{file_name}` 错误，
/// 弹出确认对话框。用户确认后调用此命令完成替换。
#[tauri::command]
pub async fn replace_document(
    app: AppHandle,
    file_path: String,
    old_doc_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    replace_document_inner(&app, &file_path, &old_doc_id, state.inner()).await
}

/// 替换文档逻辑（命令与集成测试复用）。
pub async fn replace_document_inner<R: Runtime>(
    app: &AppHandle<R>,
    file_path: &str,
    old_doc_id: &str,
    state: &AppState,
) -> Result<String, String> {
    let is_pro = *state.is_pro().read().await;
    // REQ-WS-002：获取当前工作空间 ID
    let workspace_id = crate::commands::workspace::get_current_workspace_inner(state)
        .await
        .unwrap_or_else(|_| "default".to_string());
    let service =
        ImportService::with_workspace(state.storage.clone(), state.data_dir.clone(), workspace_id);
    let name = display_name(file_path);

    // 替换：删除旧文档 → 重新导入
    match service
        .replace_and_import(file_path, old_doc_id, is_pro)
        .await
    {
        Ok(ImportOutcome::Imported(doc)) => {
            // 执行索引（与 import_files_inner 相同的管线）
            let index_result = {
                #[cfg(feature = "pro")]
                {
                    let ext_lower = Path::new(&doc.file_path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    let is_pdf = ext_lower == "pdf";
                    let is_code = matches!(ext_lower.as_str(), "rs" | "ts" | "tsx" | "py" | "go");
                    if is_pdf && is_pro {
                        index_pdf_multimodal(app, &service, &doc, state, &name).await
                    } else if is_code && is_pro {
                        emit_status(app, "indexing", format!("正在索引代码符号：{name}"));
                        let engine = SymbolEngine;
                        service
                            .index_document_with_symbols(&doc, &engine)
                            .await
                            .map_err(|e| format!("{e:#}"))
                    } else {
                        emit_status_with_phase(
                            app,
                            "indexing",
                            format!("正在分块索引：{name}"),
                            Some("splitting"),
                        );
                        service
                            .index_document(&doc)
                            .await
                            .map_err(|e| format!("{e:#}"))
                    }
                }
                #[cfg(not(feature = "pro"))]
                {
                    emit_status_with_phase(
                        app,
                        "indexing",
                        format!("正在分块索引：{name}"),
                        Some("splitting"),
                    );
                    service
                        .index_document(&doc)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
            };

            match index_result {
                Ok(()) => {
                    emit_status_with_phase(
                        app,
                        "indexing",
                        "正在加载向量化引擎…".to_string(),
                        Some("loading_model"),
                    );
                    match embed_document_chunks(app, state, &doc.id, &name).await {
                        Ok(count) => {
                            emit_status(app, "done", format!("替换完成：{name}（{count} 向量）"));

                            Ok(doc.id)
                        }
                        Err(err) => {
                            let _ = state
                                .storage
                                .update_doc_status(
                                    &doc.id,
                                    DocStatus::Failed(prefix_error(
                                        ERR_EMBED,
                                        &format!("向量化失败: {err}"),
                                    )),
                                )
                                .await;
                            emit_status(
                                app,
                                "error",
                                prefix_error(ERR_EMBED, &format!("向量化失败：{name}（{err}）")),
                            );
                            Err(prefix_error(ERR_EMBED, &format!("替换失败：{err}")))
                        }
                    }
                }
                Err(err) => {
                    emit_status(
                        app,
                        "error",
                        prefix_error(ERR_PARSE, &format!("索引失败：{name}（{err}）")),
                    );
                    Err(prefix_error(ERR_PARSE, &format!("替换失败：{err}")))
                }
            }
        }
        Ok(ImportOutcome::SkippedDuplicate(_)) => Err("内容与现有文档相同，无需替换".to_string()),
        Ok(ImportOutcome::NameConflict { .. }) => Err("仍有同名冲突，请重试".to_string()),
        Err(err) => {
            let msg = err.to_string();
            if has_error_prefix(&msg) {
                Err(msg)
            } else {
                Err(prefix_error(ERR_STORAGE, &format!("替换失败：{msg}")))
            }
        }
    }
}
#[cfg(feature = "pro")]
async fn index_pdf_multimodal<R: Runtime>(
    app: &AppHandle<R>,
    service: &ImportService<echomind_infra::sqlite_storage::SqliteStorage>,
    doc: &echomind_models::Document,
    state: &AppState,
    name: &str,
) -> Result<(), String> {
    // 阶段 1：初始化 OCR 引擎（首次使用需下载模型 ~17MB，可能耗时）
    emit_status_with_phase(app, "indexing", "初始化 OCR 引擎…".to_string(), Some("ocr"));
    let ocr_engine = state.ocr_engine().await;
    let page_renderer = state.page_renderer().await;
    let renderer_ok = page_renderer.is_ok();
    let ocr_ok = ocr_engine.is_ok();

    match (page_renderer, ocr_engine) {
        (Ok(renderer), Ok(ocr)) => {
            // 获取 VLM provider（None 表示禁用或配置缺失，将使用 NoVlm 降级）
            let vlm = state
                .vision_provider()
                .await
                .map_err(|e| format!("{e:#}"))?;
            let vlm_on = vlm.is_some();

            // 阶段 2：文本提取 + 图片检测 + 渲染 + OCR + VLM 增强
            let phase_label = if vlm_on {
                "vlm_enhancing"
            } else {
                "text_extracting"
            };
            emit_status_with_phase(
                app,
                "indexing",
                format!("正在多模态索引：{name}"),
                Some(phase_label),
            );

            match vlm {
                Some(ref vlm_provider) => {
                    // VLM 启用：表格→Markdown、甘特图→Mermaid
                    service
                        .index_document_multimodal_with_vlm(doc, renderer, ocr, vlm_provider)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
                None => {
                    // VLM 禁用或配置缺失：NoVlm 占位，管线天然跳过 VLM 阶段
                    let no_vlm = NoVlm;
                    service
                        .index_document_multimodal_with_vlm(doc, renderer, ocr, &no_vlm)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
            }
        }
        _ => {
            // 降级：多模态引擎不可用，回退到纯文本索引
            warn!("多模态引擎初始化失败（renderer={renderer_ok}, ocr={ocr_ok}），降级为纯文本索引");
            emit_status(app, "indexing", format!("正在索引（纯文本）：{name}"));
            service
                .index_document(doc)
                .await
                .map_err(|e| format!("{e:#}"))
        }
    }
}

/// 嵌入写入链路（REQ-VEC-002/003）：分块读取 → 微批次 embed → add_embeddings_batch + 进度推送。
///
/// 性能优化（2026-08-09）：
/// - 批量缓存查询：单次 DB 查询替代 64 次串行 `lookup_embedding_cache`
/// - 批量缓存写入：单事务 INSERT 替代 64 次 `spawn put_embedding_cache`
/// - 批量向量写入：单事务 `add_embeddings_batch` 替代 64 次逐条 `add_embedding`
///   + 64 次 `invalidate_vector_cache`（现在仅失效一次）
///
/// Phase 3 ContextualEmbedding：嵌入文本拼接文档名上下文前缀（`build_contextual_text`），
/// 使向量包含文档上下文信息，提升检索质量（Anthropic Contextual Retrieval）。
/// 失败时由调用方将文档置为 Failed（索引内容保留，可重试）。
pub(crate) async fn embed_document_chunks<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
    doc_id: &str,
    doc_name: &str,
) -> Result<usize, String> {
    let chunks = state
        .storage
        .list_chunks(doc_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if chunks.is_empty() {
        return Ok(0);
    }
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?;

    let total = chunks.len();
    let mut embedded = 0usize;

    // 微批次处理：每批 EMBED_BATCH_SIZE 条 chunk
    //
    // REQ-RAG-041 Contextual Retrieval：嵌入时拼接文档名上下文前缀（build_contextual_text），
    // 使向量包含文档上下文信息，提升检索精度（Anthropic Contextual Retrieval：失败率 ↓49%）。
    // 当 contextual_retrieval_enabled = false 时，使用纯 chunk 文本嵌入（不含文档名前缀）。
    //
    // REQ-RAG-049 Late Chunking：嵌入时拼接文档前缀摘要（前 500 字符），使 chunk 向量
    // 包含全文语义上下文（Jina AI 2024 Late Chunking 技术）。与 Contextual Retrieval
    // 可组合：Late Chunking 前缀 + 文档名前缀 + chunk 内容。
    // 大简化重构：Late Chunking 已删除，仅保留 Contextual Retrieval
    let use_contextual = state.contextual_retrieval_enabled;

    for batch in chunks.chunks(EMBED_BATCH_SIZE) {
        // 构建嵌入文本 + 计算内容指纹
        let texts_and_hashes: Vec<(String, String)> = batch
            .iter()
            .map(|c| {
                let text = if use_contextual {
                    build_contextual_text(doc_name, &c.content)
                } else {
                    c.content.clone()
                };
                let hash = format!("{:x}", Md5::digest(text.as_bytes()));
                (text, hash)
            })
            .collect();

        // 步骤 1：批量查询嵌入缓存（1 次 spawn_blocking 替代 64 次）
        let hashes: Vec<String> = texts_and_hashes.iter().map(|(_, h)| h.clone()).collect();
        let cache_hits = state
            .storage
            .lookup_embedding_cache_batch(&hashes)
            .await
            .map_err(|e| format!("{e:#}"))?;

        let mut cache_hit_map: std::collections::HashMap<usize, Vec<f32>> =
            std::collections::HashMap::with_capacity(cache_hits.len());
        for (idx, vector) in cache_hits {
            cache_hit_map.insert(idx, vector);
        }

        // 步骤 2：收集缓存未命中文本，批量推理
        let mut miss_indices: Vec<usize> = Vec::new();
        let mut miss_texts: Vec<String> = Vec::new();
        for (i, (text, _)) in texts_and_hashes.iter().enumerate() {
            if !cache_hit_map.contains_key(&i) {
                miss_indices.push(i);
                miss_texts.push(text.clone());
            }
        }

        let mut all_vectors: Vec<Vec<f32>> = vec![vec![]; batch.len()];

        // 填充缓存命中的向量
        for (idx, vector) in &cache_hit_map {
            all_vectors[*idx] = vector.clone();
        }

        // 对缓存未命中文本执行 ONNX 推理
        if !miss_texts.is_empty() {
            let computed = embedder
                .embed_batch(&miss_texts)
                .await
                .map_err(|e| format!("{e:#}"))?;

            // 填充推理结果 + 收集缓存写入项
            let mut cache_writes: Vec<(String, Vec<f32>)> = Vec::with_capacity(miss_indices.len());
            for (miss_idx, vector) in miss_indices.iter().zip(computed) {
                all_vectors[*miss_idx] = vector.clone();
                let hash = &texts_and_hashes[*miss_idx].1;
                cache_writes.push((hash.clone(), vector));
            }

            // 步骤 3：批量写入缓存（1 次 spawn_blocking 替代 64 次 spawn）
            if !cache_writes.is_empty() {
                let storage = state.storage.clone();
                if let Err(e) = storage.put_embedding_cache_batch(&cache_writes).await {
                    warn!("批量写入嵌入缓存失败（不影响导入流程）: {e:#}");
                }
            }
        }

        // 步骤 4：批量写入所有 embeddings（1 次 spawn_blocking 替代 64 次）
        let embeddings_to_write: Vec<(String, Vec<f32>)> = batch
            .iter()
            .zip(all_vectors.iter())
            .map(|(chunk, vector)| (chunk.id.clone(), vector.clone()))
            .collect();
        state
            .storage
            .add_embeddings_batch(&embeddings_to_write)
            .await
            .map_err(|e| format!("{e:#}"))?;

        embedded += batch.len();

        // 推送向量化进度（每批一次）
        let _ = app.emit(
            "embedding_progress",
            EmbeddingProgressPayload {
                doc_id: doc_id.to_string(),
                doc_name: doc_name.to_string(),
                embedded,
                total,
            },
        );
    }
    Ok(total)
}

/// 重试索引失败文档（REQ-VEC-005）。
/// 重置状态为 Pending → 清理旧 chunks/embeddings → 重新索引 + 嵌入（自动退避重试，上限 3 次）。
/// 手动重试入口，前端 Failed 文档点击「重试」按钮调用。
#[tauri::command]
pub async fn retry_index(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    retry_index_inner(&app, &id, state.inner()).await
}

/// 重试逻辑（命令与集成测试复用）。
///
/// # 流程
/// 1. 查找文档，校验存在性
/// 2. 重置状态为 Pending
/// 3. 清理旧 chunks 与 embeddings（`delete_chunks_by_doc`）
/// 4. 重新索引 + 嵌入（自动退避重试，上限 `MAX_RETRY_ATTEMPTS` 次）
/// 5. 达到上限仍失败则置 Failed 并返回可读原因
pub async fn retry_index_inner<R: Runtime>(
    app: &AppHandle<R>,
    doc_id: &str,
    state: &AppState,
) -> Result<(), String> {
    // Step 1: 查找文档
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let doc = docs
        .iter()
        .find(|d| d.id == doc_id)
        .ok_or_else(|| format!("文档不存在: {doc_id}"))?;
    let name = display_name(&doc.file_path);
    let doc = doc.clone();

    // Step 2: 重置状态为 Pending
    state
        .storage
        .update_doc_status(&doc.id, DocStatus::Pending)
        .await
        .map_err(|e| format!("{e:#}"))?;
    emit_status(app, "indexing", format!("正在重试索引：{name}"));

    // Step 3: 清理旧 chunks 与 embeddings
    state
        .storage
        .delete_chunks_by_doc(&doc.id)
        .await
        .map_err(|e| format!("清理旧分块失败: {e:#}"))?;

    // Step 4: 重新索引 + 嵌入（自动退避重试）
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let mut last_err = String::new();

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        if attempt > 1 {
            // 指数退避：500ms → 1000ms → 2000ms
            let delay_ms = 500u64 * 2u64.pow((attempt - 2) as u32);
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            // 重试前清理上次索引产生的 chunks（避免累积重复分块）
            state
                .storage
                .delete_chunks_by_doc(&doc.id)
                .await
                .map_err(|e| format!("清理旧分块失败: {e:#}"))?;
            emit_status(
                app,
                "indexing",
                format!("自动重试（第 {attempt}/{MAX_RETRY_ATTEMPTS} 次）：{name}"),
            );
        }

        // 索引阶段
        match service.index_document(&doc).await {
            Ok(()) => {
                // 嵌入阶段
                match embed_document_chunks(app, state, &doc.id, &name).await {
                    Ok(count) => {
                        emit_status(app, "done", format!("重试索引完成：{name}（{count} 向量）"));
                        return Ok(());
                    }
                    Err(err) => {
                        last_err = prefix_error(ERR_EMBED, &format!("向量化失败: {err}"));
                        let _ = state
                            .storage
                            .update_doc_status(&doc.id, DocStatus::Failed(last_err.clone()))
                            .await;
                    }
                }
            }
            Err(err) => {
                last_err = prefix_error(ERR_PARSE, &format!("{err:#}"));
                let _ = state
                    .storage
                    .update_doc_status(&doc.id, DocStatus::Failed(last_err.clone()))
                    .await;
            }
        }
    }

    // Step 5: 达到上限仍失败
    emit_status(
        app,
        "error",
        format!("重试失败（已达上限 {MAX_RETRY_ATTEMPTS} 次）：{name}（{last_err}）"),
    );
    Err(format!(
        "重试失败（已达上限 {MAX_RETRY_ATTEMPTS} 次）：{last_err}"
    ))
}

/// 取消正在进行的批量导入（REQ-ING-006）。
/// 设置取消标志后，`import_files_inner` 循环在下一个文件边界退出；
/// 已完成部分保留，未开始部分不再执行，不产生半成品索引。
#[tauri::command]
pub async fn abort_import(state: State<'_, AppState>) -> Result<(), String> {
    state.abort_import();
    Ok(())
}

/// 显式初始化向量化引擎，推送下载进度到前端（REQ-VEC-008-AC-1~4）。
///
/// 前端在配置向导完成或设置面板中调用此命令，
/// 通过 `model_download_progress` 事件实时接收下载进度。
/// 如果模型已缓存，直接推送 `done` 事件。
#[tauri::command]
pub async fn init_embedder(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // AC-4：如果已初始化，直接推送 done 事件
    if state.embedder_initialized().await {
        let _ = app.emit(
            "model_download_progress",
            echomind_infra::local_embedder::DownloadEvent::Done,
        );
        return Ok(());
    }

    // 创建进度回调（通过 Tauri 事件推送）
    let app_handle = app.clone();
    let progress: echomind_infra::local_embedder::DownloadProgressFn =
        std::sync::Arc::new(move |event| {
            let _ = app_handle.emit("model_download_progress", event);
        });

    state
        .init_embedder_with_progress(progress)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 检查向量化引擎下载状态（用于首启向导判断）。
///
/// 返回 `ready` / `needs_download` / `partial_download` 三态：
/// - `ready`: 模型文件已齐备，可直接加载
/// - `needs_download`: 模型文件缺失，需从头下载
/// - `partial_download`: 存在 .partial 文件，可断点续传
#[tauri::command]
pub async fn check_embedder_status(
    state: State<'_, AppState>,
) -> Result<echomind_infra::local_embedder::EmbedderStatus, String> {
    let cache_dir = state.data_dir.join("models");
    tokio::task::spawn_blocking(move || {
        echomind_infra::local_embedder::LocalEmbedder::check_status(&cache_dir)
    })
    .await
    .map_err(|e| format!("检查状态失败: {e}"))
}

/// 获取模型缓存信息（REQ-VEC-008-AC-5）。
///
/// 返回缓存目录总大小 + 已安装模型列表。
#[tauri::command]
pub async fn get_model_cache_info(
    state: State<'_, AppState>,
) -> Result<echomind_infra::local_embedder::ModelCacheInfo, String> {
    let cache_dir = state.data_dir.join("models");
    let info = tokio::task::spawn_blocking(move || {
        echomind_infra::local_embedder::LocalEmbedder::get_cache_info(&cache_dir)
    })
    .await
    .map_err(|e| format!("缓存信息查询任务失败: {e:#}"))?;
    Ok(info)
}

/// 清理模型缓存（REQ-VEC-008-AC-6）。
///
/// `model_name` 为 `None` 时删除全部模型目录。
/// 返回删除的字节数。
#[tauri::command]
pub async fn clear_model_cache(
    model_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let cache_dir = state.data_dir.join("models");
    let freed = tokio::task::spawn_blocking(move || {
        echomind_infra::local_embedder::LocalEmbedder::clear_cache(
            &cache_dir,
            model_name.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("缓存清理任务失败: {e:#}"))?;
    Ok(freed)
}

// ------------------------------------------------------------------
// 索引重建（REQ-VEC-009）
// ------------------------------------------------------------------

/// 重建已索引文档的索引（REQ-VEC-009）。
///
/// 删除该文档的全部 chunks 与向量，重新执行解析 → 分块 → 嵌入 → 存储链路。
/// 适用于嵌入模型升级或分块策略变更后重新索引。
/// 重建期间文档状态标记为 Processing，重建完成后恢复 Indexed。
///
/// 内部复用 `retry_index_inner` 逻辑（清理旧 chunks → 重新索引 + 嵌入）。
#[tauri::command]
pub async fn rebuild_index(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    rebuild_index_inner(&app, &id, state.inner()).await
}

/// `rebuild_index` 的逻辑实现（命令与集成测试复用）。
///
/// 封装 `retry_index_inner`，语义上表示「重建索引」而非「重试失败」。
/// 适用于 Indexed 状态文档的主动重建。
pub async fn rebuild_index_inner<R: Runtime>(
    app: &AppHandle<R>,
    doc_id: &str,
    state: &AppState,
) -> Result<(), String> {
    retry_index_inner(app, doc_id, state).await
}

// ------------------------------------------------------------------
// 文件夹拖拽导入（REQ-ING-023）
// ------------------------------------------------------------------

/// 跳过的隐藏目录名。
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    ".DS_Store",
    "__pycache__",
    ".svn",
    ".hg",
    "target",
    "dist",
    "build",
    ".idea",
    ".vscode",
];

/// 递归遍历深度上限。
const MAX_DEPTH: usize = 5;

/// 展开路径列表：如果路径是目录，递归遍历其中所有支持格式的文件。
///
/// REQ-ING-023：文件夹拖拽导入
/// - 递归遍历深度上限 5 层
/// - 跳过隐藏文件和目录（`.git` / `node_modules` / `.DS_Store` 等）
/// - 文件扩展名必须在 ALLOWED_EXTENSIONS 中
fn expand_folder_paths(paths: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for path in paths {
        let p = std::path::Path::new(path);
        if p.is_dir() {
            traverse_dir(p, 0, &mut result);
        } else if p.is_file() {
            result.push(path.clone());
        }
    }
    result
}

/// 递归遍历目录，收集所有支持格式的文件。
fn traverse_dir(dir: &std::path::Path, depth: usize, result: &mut Vec<String>) {
    if depth >= MAX_DEPTH {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // 跳过隐藏文件和目录
        if name_str.starts_with('.') || SKIP_DIRS.contains(&name_str.as_ref()) {
            continue;
        }

        if path.is_dir() {
            traverse_dir(&path, depth + 1, result);
        } else if path.is_file() {
            // 仅收集支持格式的文件
            if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && echomind_core::import::ALLOWED_EXTENSIONS.contains(&ext)
                && let Some(path_str) = path.to_str()
            {
                result.push(path_str.to_string());
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod folder_import_tests {
    use super::*;

    /// TC-ING-FOLDER-001: 文件夹展开 — 目录中的支持格式文件被收集
    #[test]
    fn tc_ing_folder_001_dir_expands_to_files() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // 创建测试文件
        std::fs::write(dir_path.join("test1.md"), "# Test 1").unwrap();
        std::fs::write(dir_path.join("test2.txt"), "Test 2").unwrap();
        std::fs::write(dir_path.join("readme.md"), "# README").unwrap();

        let paths = vec![dir_path.to_str().unwrap().to_string()];
        let expanded = expand_folder_paths(&paths);

        assert!(expanded.len() >= 3, "应收集 3 个支持格式的文件");
        assert!(
            expanded.iter().any(|p| p.ends_with("test1.md")),
            "应包含 test1.md"
        );
    }

    /// TC-ING-FOLDER-002: 递归深度上限 5 层
    #[test]
    fn tc_ing_folder_002_max_depth_5() {
        let dir = tempfile::tempdir().unwrap();
        let mut current = dir.path().to_path_buf();

        // 创建 6 层深度目录（超过 MAX_DEPTH=5）
        for i in 0..7 {
            current = current.join(format!("level{i}"));
            std::fs::create_dir_all(&current).unwrap();
        }
        std::fs::write(current.join("deep.md"), "# Deep").unwrap();

        let paths = vec![dir.path().to_str().unwrap().to_string()];
        let expanded = expand_folder_paths(&paths);

        // 深度 6+ 的文件不应被收集
        assert!(
            !expanded.iter().any(|p| p.ends_with("deep.md")),
            "超过 MAX_DEPTH 的文件不应被收集"
        );
    }

    /// TC-ING-FOLDER-003: 跳过隐藏文件和目录
    #[test]
    fn tc_ing_folder_003_skip_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // 创建正常文件
        std::fs::write(dir_path.join("visible.md"), "# Visible").unwrap();

        // 创建隐藏文件
        std::fs::write(dir_path.join(".hidden.md"), "# Hidden").unwrap();

        // 创建跳过目录
        std::fs::create_dir_all(dir_path.join(".git")).unwrap();
        std::fs::write(dir_path.join(".git").join("config.md"), "# Git config").unwrap();

        std::fs::create_dir_all(dir_path.join("node_modules")).unwrap();
        std::fs::write(dir_path.join("node_modules").join("lib.md"), "# Lib").unwrap();

        let paths = vec![dir_path.to_str().unwrap().to_string()];
        let expanded = expand_folder_paths(&paths);

        assert!(
            expanded.iter().any(|p| p.ends_with("visible.md")),
            "正常文件应被收集"
        );
        assert!(
            !expanded.iter().any(|p| p.contains(".hidden")),
            "隐藏文件不应被收集"
        );
        assert!(
            !expanded.iter().any(|p| p.contains(".git")),
            ".git 目录中的文件不应被收集"
        );
        assert!(
            !expanded.iter().any(|p| p.contains("node_modules")),
            "node_modules 目录中的文件不应被收集"
        );
    }

    /// TC-ING-FOLDER-004: 非目录路径直接保留
    #[test]
    fn tc_ing_folder_004_file_path_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("single.md");
        std::fs::write(&file_path, "# Single").unwrap();

        let paths = vec![file_path.to_str().unwrap().to_string()];
        let expanded = expand_folder_paths(&paths);

        assert_eq!(expanded.len(), 1, "单个文件路径应直接保留");
    }

    /// TC-ING-FOLDER-005: 混合路径（文件 + 目录）
    #[test]
    fn tc_ing_folder_005_mixed_paths() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path();

        // 目录中有文件
        std::fs::write(dir_path.join("in_dir.md"), "# In Dir").unwrap();

        // 独立文件
        let standalone = dir_path.join("standalone.txt");
        std::fs::write(&standalone, "Standalone").unwrap();

        let paths = vec![
            dir_path.to_str().unwrap().to_string(),
            standalone.to_str().unwrap().to_string(),
        ];
        let expanded = expand_folder_paths(&paths);

        // 目录展开的文件 + 独立文件
        assert!(expanded.len() >= 2, "应包含目录展开的文件和独立文件");
    }
}
