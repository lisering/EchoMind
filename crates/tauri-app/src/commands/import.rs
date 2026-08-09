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
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let mut imported = Vec::new();

    // REQ-ING-006：导入进度与取消
    let total = paths.len();
    let cancel_flag = state.import_cancel_flag();
    state.reset_import_cancel(); // 开始前重置取消标志

    for (idx, raw_path) in paths.iter().enumerate() {
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
                            emit_status(app, "indexing", format!("正在索引：{name}"));
                            service
                                .index_document(&doc)
                                .await
                                .map_err(|e| format!("{e:#}"))
                        }
                    }
                    #[cfg(not(feature = "pro"))]
                    {
                        emit_status(app, "indexing", format!("正在索引：{name}"));
                        service
                            .index_document(&doc)
                            .await
                            .map_err(|e| format!("{e:#}"))
                    }
                };

                match index_result {
                    Ok(()) => match embed_document_chunks(app, state, &doc.id, &name).await {
                        Ok(count) => {
                            emit_status(app, "done", format!("索引完成：{name}（{count} 向量）"));
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
                                        classify_and_update_domain(&storage, &emb, &doc_id).await
                                {
                                    warn!("领域分类失败（doc_id={doc_id}）: {e:#}");
                                }
                            });
                            // REQ-PERF-007：后台异步嵌入 proposition（不阻塞导入完成事件）
                            let prop_storage = state.storage.clone();
                            let prop_doc_id = doc.id.clone();
                            let prop_embedder = shared_embedder.clone();
                            tokio::spawn(async move {
                                if let Some(emb) = prop_embedder
                                    && let Err(e) =
                                        embed_propositions(&prop_storage, &emb, &prop_doc_id).await
                                {
                                    warn!("Proposition 嵌入失败（doc_id={prop_doc_id}）: {e}");
                                }
                            });
                            // REQ-ING-019：后台异步生成文档摘要（不阻塞导入完成事件）
                            let sum_storage = state.storage.clone();
                            let sum_doc_id = doc.id.clone();
                            let sum_llm_config = state.llm_config().read().await.clone();
                            tokio::spawn(async move {
                                if let Some(cfg) = sum_llm_config
                                    && let Ok(provider) =
                                        OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model)
                                    && let Ok(chunks) = sum_storage.list_chunks(&sum_doc_id).await
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
                                prefix_error(ERR_EMBED, &format!("向量化失败：{name}（{err}）")),
                            );
                        }
                    },
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

    // **性能/一致性**：知识库已变更 → 清空查询缓存，防止缓存答案引用已删除/过期的文档
    if !imported.is_empty() {
        if let Err(e) = state.cache.clear_all().await {
            warn!("导入后清空查询缓存失败: {e:#}");
        }
        // P2-1 StepCache：知识库变更 → 清空步骤级缓存（检索结果可能引用旧文档）
        state.step_cache.clear();
    }

    Ok(imported)
}

/// Pro 版 PDF 多模态索引（REQ-MM-004）：文本提取 → 图片渲染 → OCR → VLM 增强 → 分块入库。
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

/// 嵌入写入链路（REQ-VEC-002/003）：分块读取 → 微批次 embed → add_embedding + 进度推送。
///
/// GB 级文档加速（路径 4）：
/// - 将全部 chunks 按 `EMBED_BATCH_SIZE` 分为微批次
/// - 每批调用 `embed_batch`（内部使用并行会话池，路径 2）
/// - 每批完成后发射 `embedding_progress` 事件，前端渲染进度条
/// - 逐批写入 `add_embedding`，已向量化的 chunk 立即可用于向量检索
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

        // 步骤 1：查询嵌入缓存
        let mut cache_hits: Vec<(usize, Vec<f32>)> = Vec::new(); // (batch_index, embedding)
        let mut miss_indices: Vec<usize> = Vec::new();
        let mut miss_texts: Vec<String> = Vec::new();

        for (i, (_text, hash)) in texts_and_hashes.iter().enumerate() {
            match state
                .storage
                .lookup_embedding_cache(hash)
                .await
                .map_err(|e| format!("{e:#}"))?
            {
                Some(cached) => cache_hits.push((i, cached)),
                None => {
                    miss_indices.push(i);
                    miss_texts.push(texts_and_hashes[i].0.clone());
                }
            }
        }

        // 步骤 2：对缓存未命中文本执行 ONNX 推理
        let mut all_vectors: Vec<Vec<f32>> = vec![vec![]; batch.len()];
        if !miss_texts.is_empty() {
            let computed = embedder
                .embed_batch(&miss_texts)
                .await
                .map_err(|e| format!("{e:#}"))?;
            for (miss_idx, vector) in miss_indices.iter().zip(computed) {
                all_vectors[*miss_idx] = vector;
                // 写回缓存（异步，不阻塞批次处理）
                let hash = &texts_and_hashes[*miss_idx].1;
                let emb = all_vectors[*miss_idx].clone();
                let storage = state.storage.clone();
                let hash_clone = hash.clone();
                tokio::spawn(async move {
                    let _ = storage.put_embedding_cache(&hash_clone, &emb).await;
                });
            }
        }

        // 步骤 3：填充缓存命中的向量
        for (hit_idx, vector) in cache_hits {
            all_vectors[hit_idx] = vector;
        }

        // 步骤 4：写入所有 embeddings
        for (chunk, vector) in batch.iter().zip(all_vectors.iter()) {
            state
                .storage
                .add_embedding(&chunk.id, vector)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
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

/// Proposition 嵌入写入链路（REQ-PERF-007）：列出 proposition → 批量 embed → 写入。
///
/// 在 `embed_document_chunks` 完成后调用，为文档的所有 proposition 计算嵌入向量。
/// 失败时不阻塞导入流程（proposition 检索不可用时降级为 chunk 级检索）。
async fn embed_propositions(
    storage: &SqliteStorage,
    embedder: &echomind_infra::local_embedder::LocalEmbedder,
    doc_id: &str,
) -> Result<usize, String> {
    let propositions = storage
        .list_propositions_by_doc(doc_id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    if propositions.is_empty() {
        return Ok(0);
    }

    let mut embeddings: Vec<(String, Vec<f32>)> = Vec::with_capacity(propositions.len());

    // 微批次处理 proposition 嵌入
    for batch in propositions.chunks(EMBED_BATCH_SIZE) {
        let texts: Vec<String> = batch.iter().map(|p| p.content.clone()).collect();
        let vectors = embedder
            .embed_batch(&texts)
            .await
            .map_err(|e| format!("{e:#}"))?;

        for (prop, vector) in batch.iter().zip(vectors) {
            embeddings.push((prop.id.clone(), vector));
        }
    }

    storage
        .add_proposition_embeddings(&embeddings)
        .await
        .map_err(|e| format!("{e:#}"))?;

    Ok(embeddings.len())
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
