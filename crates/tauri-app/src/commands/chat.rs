//! chat 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;
use echomind_core::LLMProvider;
use echomind_core::budget::estimate_cost_usd;

/// 流式对话。事件序列：`chat_sources?` → `chat_token*` → `chat_done`；
/// 失败先推 `chat_error` 再返回 Err；中断时推「已中断」+ `chat_done` 并照常落库。
///
/// REQ-ERR-001：未携带错误前缀的异常统一包装为 `UNKNOWN:` 前缀。
///
/// **Panic 安全**：使用 `catch_unwind` 捕获 `chat_inner` 中的 panic（如 ONNX 运行时崩溃），
/// 确保前端始终收到 `chat_error` 事件，不会永久停留在「初始化向量化引擎」阶段。
///
/// **S5: SessionCoordinator** — `session_coordinator.run()` 包装对话执行，
/// 同一 conversation_id 的请求串行化（防止并发写入消息历史损坏），
/// 不同 conversation_id 并发执行互不阻塞。
#[tauri::command]
pub async fn chat(
    app: AppHandle,
    query: String,
    history: Vec<ChatMessage>,
    conversation_id: String,
    turn_group: Option<String>,
    version: Option<i32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let state_inner = state.inner();

    // S5: SessionCoordinator — 会话级串行化
    let coord = state_inner.session_coordinator.clone();

    // Clone owned data for Fn closure（每次调用需 clone）
    let app_c = app.clone();
    let query_c = query.clone();
    let history_c = history.clone();
    let conv_id_c = conversation_id.clone();
    let tg_c = turn_group.clone();
    let ver_c = version;

    let coord_result = coord
        .run(&conversation_id, move || {
            let app = app_c.clone();
            let query = query_c.clone();
            let history = history_c.clone();
            let conv_id = conv_id_c.clone();
            let tg = tg_c.clone();
            let ver = ver_c;
            async move {
                // Panic 恢复：捕获 chat_inner 中的 panic
                let inner_future = std::panic::AssertUnwindSafe(chat_inner(
                    &app,
                    &query,
                    &history,
                    &conv_id,
                    tg.as_deref(),
                    ver,
                    state_inner,
                ));
                inner_future
                    .catch_unwind()
                    .await
                    .map_err(|panic_payload| {
                        let msg = panic_payload
                            .downcast_ref::<&str>()
                            .copied()
                            .or_else(|| panic_payload.downcast_ref::<String>().map(String::as_str))
                            .unwrap_or("未知 panic");
                        anyhow::anyhow!("UNKNOWN: 对话引擎内部错误（panic）: {msg}")
                    })
                    .and_then(|r| r.map_err(|e| anyhow::anyhow!(e)))
            }
        })
        .await;

    // 转换 anyhow::Result<()> → Result<(), String>
    coord_result.map_err(|e| {
        let err_str = e.to_string();
        if has_error_prefix(&err_str) {
            err_str
        } else {
            prefix_error(ERR_UNKNOWN, &err_str)
        }
    })?;

    Ok(())
}

/// 对话编排（命令与集成测试复用）：空库拦截 → 配置检查 → 引擎初始化 → 检索对话 → 落库。
pub async fn chat_inner<R: Runtime>(
    app: &AppHandle<R>,
    query: &str,
    history: &[ChatMessage],
    conversation_id: &str,
    turn_group: Option<&str>,
    version: Option<i32>,
    state: &AppState,
) -> Result<(), String> {
    // REQ-ERR-005：查询长度校验（防御性输入校验）
    if query.chars().count() > MAX_QUERY_LENGTH {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("问题过长，请缩减至 {} 字符以内", MAX_QUERY_LENGTH),
        ));
    }

    // 空知识库前置拦截（REQ-RAG-003-AC-2）：明确引导，不触碰向量化引擎与网络
    let t0 = std::time::Instant::now();
    let doc_count = state
        .storage
        .count_documents()
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;
    if doc_count == 0 {
        return Err(prefix_error(
            ERR_VALIDATION,
            "知识库为空，请先通过左下角 + 号导入文档",
        ));
    }

    let llm_config = state
        .llm_config()
        .read()
        .await
        .clone()
        .ok_or_else(|| prefix_error(ERR_VALIDATION, "未配置 LLM，请完成初始配置向导"))?;

    // 阶段 1：初始化向量化引擎（首次使用需下载模型，可能耗时较长）
    //
    // **Bug 修复（V1）**：此前 `state.embedder().await` 无超时保护，当 ONNX 模型下载
    // 因网络问题挂起时（如 HuggingFace 不可达），chat 命令永不返回，前端永久停留在
    // 「初始化向量化引擎」。现增加两层保护：
    //   1. 进度回调：未初始化时通过 `init_embedder_with_progress` 推送下载进度到前端
    //   2. 超时保护：`tokio::time::timeout` 包装初始化调用，超时后返回 `EMBED:` 错误
    emit_chat_phase(app, "preparing", "初始化向量化引擎…");
    if !state.embedder_initialized().await {
        // 慢路径：首次初始化，推送下载进度到前端
        let app_for_progress = app.clone();
        let progress: echomind_infra::local_embedder::DownloadProgressFn = Arc::new(move |event| {
            let _ = app_for_progress.emit("model_download_progress", event);
        });
        // 超时保护：防止网络不可达时永久阻塞
        let timeout_secs = embedder_init_timeout();
        let init_result = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            state.init_embedder_with_progress(progress),
        )
        .await;
        match init_result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                return Err(format!("EMBED: 向量化引擎不可用: {e:#}"));
            }
            Err(_) => {
                return Err(format!(
                    "EMBED: 向量化引擎初始化超时（{timeout_secs} 秒），\
                     请检查网络连接后重试。如在境内，可在设置中手动初始化向量化引擎\
                     （镜像源自动回退）"
                ));
            }
        }
    }
    let embedder = state
        .embedder()
        .await
        .map_err(|e| prefix_error(ERR_EMBED, &format!("向量化引擎不可用: {e:#}")))?;
    let embedder = embedder.clone();
    debug!("chat_inner embedder ready: {}ms", t0.elapsed().as_millis());

    // 兜底：会话不存在时幂等创建（正常流程前端先 create_conversation）
    let conversation = Conversation::with_id(
        conversation_id.to_string(),
        DEFAULT_WORKSPACE.to_string(),
        derive_title(query),
    );
    state
        .storage
        .create_conversation(&conversation)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // **性能优化**：批量读取全部 settings（1 次 DB 查询替代 6 次串行查询）。
    // 原实现每次 get_setting 触发独立的 spawn_blocking + DB 连接获取，
    // 6 次串行 ≈ 6 × 2ms = 12ms 额外延迟。批量读取 ≈ 2ms。
    let settings = state
        .storage
        .get_settings_batch(&[
            "rag.hybrid_search",
            "rag.rerank_enabled",
            "rag.hyde_enabled",
            "rag.context_token_limit",
            "rag.coordinator_enabled",
            "rag.sub_agent_enabled",
            "rag.agent_enabled",
            "rag.progressive_injection",
            "rag.speculative_enabled",
            "rag.retrieval_memory_enabled",
            "rag.graph_retriever_enabled",
            "rag.quality_gate_enabled",
            "rag.web_search_enabled",
            "rag.top_k",
            "rag.score_threshold",
            "rag.chunk_expansion_enabled",
            "memory.enabled",
            "cache.enabled",
            "cache.ttl_secs",
            "cache.semantic_threshold",
        ])
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;
    let settings_map: std::collections::HashMap<&str, &str> = settings
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // REQ-RAG-014：检索参数从 settings 读取（可配置）
    let rag_top_k = settings_map
        .get("rag.top_k")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_TOP_K)
        .clamp(1, 20);
    let rag_score_threshold = settings_map
        .get("rag.score_threshold")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let rag_chunk_expansion_enabled = settings_map
        .get("rag.chunk_expansion_enabled")
        .is_none_or(|&v| v != "false");
    debug!(
        "RAG params: top_k={}, score_threshold={:.2}, chunk_expansion={}",
        rag_top_k, rag_score_threshold, rag_chunk_expansion_enabled
    );

    // **性能优化（秒出答案）**：查询嵌入只计算一次，复用于 L1 语义缓存、
    // L3 检索缓存和向量检索，避免 3 次冗余 ONNX 推理（省 ~100-200ms）。
    //
    // 原实现：L1 缓存查找 embed 1 次 → L3 缓存查找 embed 1 次 → 检索内部 embed 1 次 = 3 次
    // 优化后：在缓存检查前统一 embed 1 次，后续全部复用
    let cache_enabled = settings_map
        .get("cache.enabled")
        .is_none_or(|&v| v != "false");
    let cache_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let cache_ttl_secs = settings_map
        .get("cache.ttl_secs")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(86400);
    let query_embedding_result: Option<Vec<f32>>;

    if cache_enabled {
        let semantic_threshold = settings_map
            .get("cache.semantic_threshold")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(0.92);

        // L0 精确匹配（哈希，无嵌入开销）→ 未命中再走 L1 语义匹配（需查询嵌入）
        let exact_hit = state
            .cache
            .lookup_exact(&query_hash(query), cache_ttl_secs, cache_now)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("缓存查询失败: {e:#}")))?;

        let (hit, emb) = if exact_hit.is_some() {
            (exact_hit, None)
        } else {
            // **性能优化**：嵌入计算一次，后续 L3 缓存查找和向量检索复用
            let emb = embedder
                .embed(query)
                .await
                .map_err(|e| prefix_error(ERR_EMBED, &format!("查询向量化失败: {e:#}")))?;
            let hit = state
                .cache
                .lookup_semantic(&emb, semantic_threshold, cache_ttl_secs, cache_now)
                .await
                .map_err(|e| prefix_error(ERR_STORAGE, &format!("缓存查询失败: {e:#}")))?;
            (hit, Some(emb))
        };

        if let Some(hit) = hit
            && let Some(answer) = hit.answer_text
            && !answer.is_empty()
        {
            // 命中：走与正常回答一致的事件流（sources → token* → done）
            emit_chat_phase(app, "retrieving", "命中语义缓存…");
            emit_chat_phase(app, "generating", "正在生成回答…");
            let sources = hit
                .sources_json
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<RetrievalResult>>(s).ok());
            if let Some(s) = &sources {
                emit_chat_sources(app, s);
            }
            // 分块推送 token，模拟流式（前端打字效果与正常回答一致）
            for chunk in split_cached_answer(&answer, 40) {
                let _ = app.emit("chat_token", chunk);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
            emit_chat_done(app, None);
            // 缓存命中：无推理流，reasoning = None
            persist_exchange(
                state,
                conversation_id,
                query,
                &answer,
                sources,
                None,
                turn_group,
                version,
            )
            .await?;
            state.clear_abort(conversation_id).await;
            return Ok(());
        }
        query_embedding_result = emb;
    } else {
        // 缓存禁用：在 embedder 被 move 进 retriever 之前预计算嵌入
        query_embedding_result = Some(
            embedder
                .embed(query)
                .await
                .map_err(|e| prefix_error(ERR_EMBED, &format!("查询向量化失败: {e:#}")))?,
        );
    }
    // query_embedding_result 现在持有预计算的查询嵌入（Some）或未计算（None，仅 L0 命中但无答案场景，理论上不发生）
    let query_embedding = query_embedding_result;

    let retriever = {
        let hybrid_enabled = settings_map
            .get("rag.hybrid_search")
            // 默认启用混合检索（未设置时视为 true）
            .is_none_or(|&v| v != "false");
        let rerank_enabled = settings_map
            .get("rag.rerank_enabled")
            // **性能优化**：默认关闭重排序（省 ~200ms Cross-Encoder 推理）。
            // 重排序提升精度但增加延迟，用户可在设置中显式启用。
            .is_some_and(|&v| v == "true");
        let hyde_enabled = settings_map
            .get("rag.hyde_enabled")
            // 默认关闭 HyDE 查询改写（未设置时视为 false）
            .is_some_and(|&v| v == "true");
        let mut r = HybridRetriever::new(embedder, state.storage.clone());
        r.set_hybrid_enabled(hybrid_enabled);
        if rerank_enabled {
            // 懒加载 Cross-Encoder 重排序引擎（首次启用触发模型下载 ~280MB）。
            // **降级策略**：模型不可用（未下载/下载失败）时不阻塞对话，
            // 仅告警并继续走无重排的混合检索（精度略降，功能可用）。
            match state.reranker().await {
                Ok(reranker) => {
                    r.set_reranker(Some(Arc::new(reranker.clone())));
                }
                Err(e) => {
                    warn!("重排序引擎不可用，降级为无重排检索: {e:#}");
                }
            }
        }
        if hyde_enabled {
            // HyDE 查询改写（REQ-RAG-021）：使用用户 LLM 配置生成假设性答案文档
            // 复用已有 LLM 配置（api_key / base_url / model），无额外模型下载
            match HydeRewriter::new(
                llm_config.api_key.clone(),
                llm_config.base_url.clone(),
                llm_config.model.clone(),
            ) {
                Ok(rewriter) => r.set_rewriter(Some(Arc::new(rewriter))),
                Err(e) => warn!("HyDE 改写器初始化失败，跳过查询改写: {e:#}"),
            }
        }
        r
    };
    // Q11：使用 LlmRouter 路由到正确的后端（借鉴 QM HarnessRouter）
    // Router 根据 fallback（由 set_llm_mode 更新）和 per-conversation last_mode 决策
    let (llm_choice, router_verdict) = state
        .llm_router
        .resolve(conversation_id, None)
        .await
        .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))?;
    let llm_mode = llm_choice.mode;
    // 模式变更时记录日志（调用方可据此重置 KV cache 等会话状态）
    if router_verdict == echomind_core::llm_router::RouterVerdict::ModeChanged {
        info!("会话 {conversation_id} LLM 后端切换 → {llm_mode:?}");
    }
    let usage_handle: Option<std::sync::Arc<tokio::sync::Mutex<Option<TokenUsage>>>>;
    let provider: LlmProvider = if llm_mode == LlmMode::Local {
        #[cfg(feature = "pro")]
        {
            let engine = state
                .local_llm()
                .await
                .map_err(|e| prefix_error(ERR_LLM, &format!("本地推理引擎不可用: {e:#}")))?;
            usage_handle = None;
            LlmProvider::Local(engine)
        }
        #[cfg(not(feature = "pro"))]
        {
            let _ = usage_handle;
            return Err(prefix_error(ERR_PRO_REQUIRED, "本地推理是 Pro 版功能"));
        }
    } else {
        let p = OpenAIProvider::new(
            llm_config.api_key.clone(),
            llm_config.base_url.clone(),
            llm_config.model.clone(),
        )
        .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))?;
        usage_handle = Some(p.usage_handle());
        LlmProvider::Remote(p)
    };

    // S4: Q06/S71 Shadow Screen 集成 — Strict 模式下对用户 query 执行 LLM 安全分类。
    //
    // Shadow 筛查**不影响**对话流（安全隔离），仅收集 agree/disagree 统计。
    // LLM 不可用 / 超时 5s → 降级为 Unscreened，不阻断对话。
    {
        let posture = echomind_core::security::SecurityPosture::from_u8(
            state
                .security_posture
                .load(std::sync::atomic::Ordering::SeqCst),
        );
        echomind_core::security::execute_shadow_screen(
            posture,
            query,
            &provider,
            &state.shadow_screen_collector,
        )
        .await;
    }

    // REQ-RAG-017：对话上下文长度管理 — 压缩超限历史消息（替代纯截断策略）
    // 复用上方批量读取的 settings_map（避免再次 DB 查询）
    let context_token_limit = settings_map
        .get("rag.context_token_limit")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4096);

    // REQ-RAG-028：检查质量门控是否启用（复用批量读取结果）
    let quality_gate_enabled = settings_map
        .get("rag.quality_gate_enabled")
        .is_some_and(|&v| v == "true");

    // REQ-PERF-011：检查 Speculative RAG 是否启用（复用批量读取结果，运行时可切换）
    let speculative_enabled = settings_map
        .get("rag.speculative_enabled")
        .is_some_and(|&v| v == "true");

    // REQ-RAG-025：检查多代理协调模式是否启用（复用批量读取结果）
    let coordinator_enabled = settings_map
        .get("rag.coordinator_enabled")
        .is_some_and(|&v| v == "true");

    // REQ-RAG-025 扩展：检查子代理舰队模式是否启用（仅在 coordinator 模式下生效）
    let sub_agent_enabled = settings_map
        .get("rag.sub_agent_enabled")
        .is_some_and(|&v| v == "true");

    // REQ-RAG-022：检查 Agentic RAG 模式是否启用（复用批量读取结果）
    let agent_enabled = settings_map
        .get("rag.agent_enabled")
        .is_some_and(|&v| v == "true");

    // REQ-RAG-027：检查知识图谱图遍历检索是否启用（复用批量读取结果）
    let graph_retriever_enabled = settings_map
        .get("rag.graph_retriever_enabled")
        .is_some_and(|&v| v == "true");

    let compaction = CompactionEngine::new(&provider);

    // Q04：修复悬空 tool_call（借鉴 QM healDanglingCalls）
    //
    // Agent 模式下 tool_call 被中断时（如用户取消、进程崩溃），
    // assistant 消息中包含 Action 但后续没有对应的 Observation。
    // 在压缩前自动补全 [interrupted] 占位消息，确保压缩后的历史
    // 不会包含半完成的 ReAct 循环。
    let mut healed_history = history.to_vec();
    echomind_compact::heal_dangling_calls(&mut healed_history);
    let history: &[ChatMessage] = &healed_history;

    // Q03：双阈值压缩配置（借鉴 QM COMPACT_SOFT/HARD_FRACTION）
    // Soft 阈值（70%）触发后台异步压缩（不阻塞当前轮次）
    // Hard 阈值（90%）触发同步压缩（阻塞兜底）
    let dual_config = echomind_compact::DualThresholdConfig::from_settings(
        settings_map
            .get("compaction.soft_threshold")
            .and_then(|v| v.parse::<f64>().ok()),
        settings_map
            .get("compaction.hard_threshold")
            .and_then(|v| v.parse::<f64>().ok()),
        settings_map
            .get("compaction.max_entries")
            .and_then(|v| v.parse::<usize>().ok()),
        Some(context_token_limit),
    );
    let compaction_trigger = compaction.check_compaction_needed(history, &dual_config);

    // Q03：Background 触发 — 后台异步压缩（不阻塞当前轮次）
    // S68：使用 schedule_background_compaction 统一封装 try_acquire + run + release
    if compaction_trigger.is_background() {
        let bg_history = history.to_vec();
        let bg_max_tokens = context_token_limit;
        let bg_pending = std::sync::Arc::clone(&state.compaction_pending);
        let bg_conv_id = conversation_id.to_string();
        let bg_mode = llm_mode;

        if bg_mode == LlmMode::Local {
            #[cfg(feature = "pro")]
            {
                if let Ok(engine) = state.local_llm().await {
                    // S68：schedule_background_compaction 处理 try_acquire，
                    // 返回的 future 包含 run + release（自动管理锁）。
                    // 由于 LLMProvider async fn in trait 的 future 不保证 Send，
                    // 无法直接 tokio::spawn 返回的 future。
                    // 改为：在主流程调用 schedule 做 try_acquire（极速），
                    // 然后用具体类型 spawn run + release。
                    if echomind_compact::schedule_background_compaction(
                        bg_conv_id.clone(),
                        bg_history.clone(),
                        bg_max_tokens,
                        engine,
                        bg_pending.clone(),
                    )
                    .await
                    .is_some()
                    {
                        // try_acquire 成功，spawn run + release
                        let bg_engine = state.local_llm().await.ok();
                        tokio::spawn(async move {
                            if let Some(engine) = bg_engine {
                                echomind_compact::run_background_compaction(
                                    &bg_conv_id,
                                    &bg_history,
                                    bg_max_tokens,
                                    &engine,
                                )
                                .await;
                            } else {
                                warn!("后台压缩：本地 LLM 引擎不可用，跳过");
                            }
                            echomind_compact::release_background_compaction(
                                &bg_conv_id,
                                bg_pending,
                            )
                            .await;
                        });
                    }
                } else {
                    warn!("后台压缩：本地 LLM 引擎不可用，跳过");
                }
            }
            #[cfg(not(feature = "pro"))]
            {
                let _ = (bg_conv_id, bg_history, bg_max_tokens, bg_pending);
            }
        } else {
            // Remote 路径：创建新 OpenAIProvider 后调度
            let bg_config = llm_config.clone();
            if let Ok(bg_provider) = OpenAIProvider::new(
                bg_config.api_key.clone(),
                bg_config.base_url.clone(),
                bg_config.model.clone(),
            ) && echomind_compact::schedule_background_compaction(
                bg_conv_id.clone(),
                bg_history.clone(),
                bg_max_tokens,
                bg_provider,
                bg_pending.clone(),
            )
            .await
            .is_some()
            {
                // try_acquire 成功，spawn run + release
                tokio::spawn(async move {
                    match OpenAIProvider::new(
                        bg_config.api_key.clone(),
                        bg_config.base_url.clone(),
                        bg_config.model.clone(),
                    ) {
                        Ok(bg_provider) => {
                            echomind_compact::run_background_compaction(
                                &bg_conv_id,
                                &bg_history,
                                bg_max_tokens,
                                &bg_provider,
                            )
                            .await;
                        }
                        Err(e) => {
                            warn!("后台压缩 provider 创建失败: {e:#}");
                        }
                    }
                    echomind_compact::release_background_compaction(&bg_conv_id, bg_pending).await;
                });
            }
        }
        // Background 模式：当前轮次使用未压缩历史（后台压缩完成后下轮生效）
    }

    // **性能优化（秒出答案）**：标准 RAG 路径下，上下文压缩（仅长历史超限才调 LLM）
    // 与知识库检索互不依赖 → 并行执行，节省串行等待；coordinator/agent/speculative
    // 分支各自内部检索，不预检索（避免浪费）。
    //
    // **关键优化**：复用上方预计算的 query_embedding，避免冗余 ONNX 推理。
    // 原实现此处重新 embed 查询 2 次（L3 缓存查找 + 检索内部），现已全部消除。
    let use_parallel_retrieval = !coordinator_enabled && !agent_enabled && !speculative_enabled;
    let (compaction_result, prefetched_sources) = if use_parallel_retrieval {
        emit_chat_phase(app, "retrieving", "检索知识库…");
        // L3 检索结果缓存（REQ-PERF-001）：复用预计算嵌入，跳过 embed + 混合检索 + 重排
        let retrieval_cache_hit = if cache_enabled {
            if let Some(ref emb) = query_embedding {
                match state
                    .cache
                    .lookup_retrieval(emb, 0.90, cache_ttl_secs, cache_now)
                    .await
                {
                    Ok(Some(json)) => serde_json::from_str::<Vec<RetrievalResult>>(&json)
                        .ok()
                        .filter(|v: &Vec<RetrievalResult>| !v.is_empty()),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some(cached_sources) = retrieval_cache_hit {
            // 命中：跳过检索，仅执行上下文压缩
            let cr = if compaction_trigger.is_synchronous() {
                compaction
                    .compact_smart(
                        history,
                        context_token_limit,
                        &echomind_compact::VerbatimTailConfig::default(),
                    )
                    .await
                    .map_err(|e| prefix_error(ERR_LLM, &format!("上下文压缩失败: {e:#}")))?
            } else {
                // Q03：None 或 Background → 当前轮次不压缩（后台压缩完成后下轮生效）
                echomind_models::CompactionResult {
                    history: history.to_vec(),
                    info: None,
                }
            };
            debug!("L3 检索结果缓存命中，跳过检索");
            (cr, Some(cached_sources))
        } else {
            // 未命中：并行 compaction + 检索（使用预计算嵌入），完成后写 L3 缓存
            // Q03：Synchronous 触发 → 调用 compact()；None/Background → 跳过压缩
            let compact_fut = async {
                if compaction_trigger.is_synchronous() {
                    compaction
                        .compact_smart(
                            history,
                            context_token_limit,
                            &echomind_compact::VerbatimTailConfig::default(),
                        )
                        .await
                } else {
                    Ok(echomind_models::CompactionResult {
                        history: history.to_vec(),
                        info: None,
                    })
                }
            };
            // **性能优化**：使用 retrieve_with_embedding 复用预计算嵌入（省 1 次 ONNX 推理 ~50-100ms）
            // async 块创建单一 future 类型，避免 match 臂 opaque 类型不匹配
            let retrieve_fut = async {
                match &query_embedding {
                    Some(emb) => {
                        retriever
                            .retrieve_with_embedding(query, emb, rag_top_k)
                            .await
                    }
                    None => retriever.retrieve(query, rag_top_k).await,
                }
            };
            let (cr, rr) = tokio::join!(compact_fut, retrieve_fut);
            let cr = cr.map_err(|e| prefix_error(ERR_LLM, &format!("上下文压缩失败: {e:#}")))?;
            let mut rr = rr.map_err(|e| classify_llm_error(&format!("检索失败: {e:#}")))?;

            // REQ-RAG-027：知识图谱图遍历检索 — 沿实体关系图边扩展到关联 chunk
            // 图扩展作为 RRF 融合的第四路检索通道，为标准检索结果提供图扩展加成。
            // 仅在标准 RAG 路径（非 coordinator/agent/speculative）且启用时执行。
            if graph_retriever_enabled && !rr.is_empty() {
                let graph_retriever =
                    echomind_core::graph_retriever::GraphRetriever::new(state.storage.clone());
                match graph_retriever.expand(query, rag_top_k).await {
                    Ok(graph_results) if !graph_results.is_empty() => {
                        debug!(
                            "图扩展返回 {} 个关联 chunk，合并到检索结果",
                            graph_results.len()
                        );
                        // 合并图扩展结果到标准检索结果：
                        // - 已存在的 chunk：保留较高分数（图扩展可能发现更高置信度的路径）
                        // - 新 chunk：追加到结果列表
                        use std::collections::HashSet;
                        let existing_ids: HashSet<String> =
                            rr.iter().map(|r| r.chunk.id.clone()).collect();
                        for gr in graph_results {
                            if existing_ids.contains(&gr.chunk.id) {
                                // 已存在：boost 分数（取最大值）
                                if let Some(existing) =
                                    rr.iter_mut().find(|r| r.chunk.id == gr.chunk.id)
                                    && gr.score > existing.score
                                {
                                    existing.score = gr.score;
                                }
                            } else {
                                // 新 chunk：追加
                                rr.push(gr);
                            }
                        }
                        // 重新按分数降序排序
                        rr.sort_by(|a, b| {
                            b.score
                                .partial_cmp(&a.score)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        // 截取 top_k
                        rr.truncate(rag_top_k * 2);
                    }
                    Ok(_) => {
                        debug!("图扩展无结果，保持标准检索结果");
                    }
                    Err(e) => {
                        warn!("图扩展失败，降级为标准检索: {e:#}");
                    }
                }
            }
            // REQ-RAG-014：用户可配置的分数阈值过滤
            if rag_score_threshold > 0.0 {
                rr.retain(|r| r.score >= rag_score_threshold);
            }
            // 写入 L3 缓存：复用预计算嵌入（不再重新 embed）
            if cache_enabled
                && !rr.is_empty()
                && let Some(ref emb) = query_embedding
                && let Ok(json) = serde_json::to_string(&rr)
                && let Err(e) = state.cache.insert_retrieval(query, emb, &json).await
            {
                warn!("写入检索结果缓存失败: {e:#}");
            }
            (cr, Some(rr))
        }
    } else {
        (
            if compaction_trigger.is_synchronous() {
                compaction
                    .compact_smart(
                        history,
                        context_token_limit,
                        &echomind_compact::VerbatimTailConfig::default(),
                    )
                    .await
                    .map_err(|e| prefix_error(ERR_LLM, &format!("上下文压缩失败: {e:#}")))?
            } else {
                // Q03：None 或 Background → 当前轮次不压缩
                echomind_models::CompactionResult {
                    history: history.to_vec(),
                    info: None,
                }
            },
            None,
        )
    };
    let compacted_history = &compaction_result.history;
    if let Some(info) = &compaction_result.info {
        let _ = app.emit("chat_context_compacted", info.clone());
    }

    // 推理模型思考过程（reasoning_content）转发句柄：
    // 必须在 provider 被 move 进引擎之前克隆，后台任务据此转发 chat_reasoning 事件
    let reasoning_handle = provider.reasoning_rx_handle();

    if coordinator_enabled {
        // 多代理协调模式（REQ-RAG-025）
        // P2-1 StepCache：缓存启用时注入步骤级缓存（分解/检索/综合分析复用）
        let coordinator = if cache_enabled {
            CoordinatorEngine::with_step_cache(retriever, provider, state.step_cache.clone())
        } else {
            CoordinatorEngine::new(retriever, provider)
        };
        // REQ-RAG-025 扩展：子代理舰队模式（仅在 coordinator 模式下生效）
        let coordinator = if sub_agent_enabled {
            coordinator.with_sub_agent_timeout(120)
        } else {
            coordinator
        };
        emit_chat_phase(app, "retrieving", "协调模式：分解查询并并行检索…");
        let outcome = coordinator
            .run(compacted_history, query)
            .await
            .map_err(|e| classify_llm_error(&format!("Coordinator 执行失败: {e:#}")))?;

        // 推送阶段进度信息
        for phase in &outcome.phases {
            emit_coordinator_phase(app, phase);
        }

        // 推送聚合引用来源
        let sources = if outcome.sources.is_empty() {
            None
        } else {
            emit_chat_sources(app, &outcome.sources);
            Some(outcome.sources.clone())
        };

        // 最终答案流式输出
        match outcome.answer_stream {
            Some(stream) => {
                emit_chat_phase(app, "generating", "正在生成回答…");
                // 推理内容：事件发射 + 落库累积（历史消息重现思考过程）
                let reasoning_collector = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                spawn_reasoning_forwarder(
                    app,
                    reasoning_handle.clone(),
                    Some(reasoning_collector.clone()),
                );
                let abort_token = state.abort_token_for(conversation_id).await;
                let result = forward_stream(app, stream, abort_token, None)
                    .await
                    .map_err(|e| classify_llm_error(&e))?;
                record_token_usage_inner(state, &result.token_usage).await;
                // REQ-PERF-001：正常回答写入查询缓存（下次相同/相似问题秒回）
                write_query_cache(state, query, &result.content, &sources, conversation_id).await;
                let reasoning = reasoning_collector
                    .lock()
                    .ok()
                    .map(|g| g.clone())
                    .filter(|s| !s.is_empty());
                persist_exchange(
                    state,
                    conversation_id,
                    query,
                    &result.content,
                    sources,
                    reasoning,
                    turn_group,
                    version,
                )
                .await?;
                state.clear_abort(conversation_id).await;
                Ok(())
            }
            None => {
                emit_chat_error(app, "知识库中未找到相关内容".to_string());
                emit_chat_done(app, None);
                Ok(())
            }
        }
    } else if agent_enabled {
        // Agentic RAG 多步推理流程（REQ-RAG-022）
        // P2-1 StepCache：缓存启用时注入步骤级缓存（search_kb 工具结果复用）
        let agent = if cache_enabled {
            AgentEngine::with_step_cache(retriever, provider, state.step_cache.clone())
        } else {
            AgentEngine::new(retriever, provider)
        };
        // REQ-RAG-032：Pro 版本注入代码执行器，启用 execute_code 工具
        #[cfg(feature = "pro")]
        let agent = agent.with_executor(std::sync::Arc::new(
            echomind_infra::wasm_executor::WasmExecutor::with_defaults(),
        ));
        emit_chat_phase(app, "retrieving", "Agent 多步推理…");
        let outcome = agent
            .run(compacted_history, query)
            .await
            .map_err(|e| classify_llm_error(&format!("Agent 推理失败: {e:#}")))?;

        // 推送中间推理步骤（REQ-RAG-022 AC-2）
        for step in &outcome.steps {
            emit_agent_step(app, step);
        }

        // 推送聚合引用来源（REQ-RAG-022 AC-7）
        let sources = if outcome.sources.is_empty() {
            None
        } else {
            emit_chat_sources(app, &outcome.sources);
            Some(outcome.sources.clone())
        };

        // 最终答案流式输出（REQ-RAG-022 AC-3）
        match outcome.answer_stream {
            Some(stream) => {
                emit_chat_phase(app, "generating", "正在生成回答…");
                // 推理内容：事件发射 + 落库累积（历史消息重现思考过程）
                let reasoning_collector = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
                spawn_reasoning_forwarder(
                    app,
                    reasoning_handle.clone(),
                    Some(reasoning_collector.clone()),
                );
                let abort_token = state.abort_token_for(conversation_id).await;
                let result = forward_stream(app, stream, abort_token, None)
                    .await
                    .map_err(|e| classify_llm_error(&e))?;
                // 记录 token 用量到累计计数器
                record_token_usage_inner(state, &result.token_usage).await;
                // REQ-PERF-001：正常回答写入查询缓存（下次相同/相似问题秒回）
                write_query_cache(state, query, &result.content, &sources, conversation_id).await;
                let reasoning = reasoning_collector
                    .lock()
                    .ok()
                    .map(|g| g.clone())
                    .filter(|s| !s.is_empty());
                persist_exchange(
                    state,
                    conversation_id,
                    query,
                    &result.content,
                    sources,
                    reasoning,
                    turn_group,
                    version,
                )
                .await?;
                state.clear_abort(conversation_id).await;
                Ok(())
            }
            None => {
                // 知识库未命中（降级为 NoContext）
                emit_chat_error(app, "知识库中未找到相关内容".to_string());
                emit_chat_done(app, None);
                Ok(())
            }
        }
    } else {
        // REQ-PERF-011：检查 Speculative RAG 是否启用（复用上方批量读取结果）
        let (sources, stream) = if speculative_enabled {
            // ---- Speculative RAG 流程（小模型草稿 → 大模型验证）----
            emit_chat_phase(app, "retrieving", "检索知识库…");
            let t_retrieve = std::time::Instant::now();
            let spec_sources = retriever
                .retrieve(query, DEFAULT_TOP_K)
                .await
                .map_err(|e| classify_llm_error(&format!("{e:#}")))?;
            debug!(
                "chat_inner speculative retrieval: {}ms",
                t_retrieve.elapsed().as_millis()
            );

            if spec_sources.is_empty() {
                // 空上下文拦截
                let no_ctx_stream = futures::stream::once(async move {
                    Ok::<String, anyhow::Error>(echomind_core::chat::NO_CONTEXT_MESSAGE.to_string())
                })
                .boxed();
                (None, no_ctx_stream)
            } else {
                // 创建草稿 provider（与验证 provider 使用相同配置）
                // 草稿用简化 prompt（top-1 chunk），验证用完整 prompt + 草稿 prefix
                let draft_provider = if llm_mode == LlmMode::Local {
                    #[cfg(feature = "pro")]
                    {
                        LlmProvider::Local(state.local_llm().await.map_err(|e| {
                            prefix_error(ERR_LLM, &format!("草稿模型不可用: {e:#}"))
                        })?)
                    }
                    #[cfg(not(feature = "pro"))]
                    {
                        let _ = &llm_config;
                        LlmProvider::Remote(
                            OpenAIProvider::new(
                                llm_config.api_key.clone(),
                                llm_config.base_url.clone(),
                                llm_config.model.clone(),
                            )
                            .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))?,
                        )
                    }
                } else {
                    LlmProvider::Remote(
                        OpenAIProvider::new(
                            llm_config.api_key.clone(),
                            llm_config.base_url.clone(),
                            llm_config.model.clone(),
                        )
                        .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))?,
                    )
                };

                let spec_engine = echomind_core::speculative_rag::SpeculativeRagEngine::new(
                    draft_provider,
                    provider,
                );

                emit_chat_phase(app, "generating", "Speculative RAG 生成中…");
                debug!(
                    "chat_inner speculative total pre-LLM: {}ms",
                    t0.elapsed().as_millis()
                );
                let spec_outcome = spec_engine
                    .speculate(&spec_sources, compacted_history, query)
                    .await
                    .map_err(|e| classify_llm_error(&format!("{e:#}")))?;

                emit_chat_sources(app, &spec_sources);

                let spec_stream = match spec_outcome {
                    echomind_core::speculative_rag::SpeculativeOutcome::DraftAccepted {
                        stream,
                        ..
                    } => {
                        debug!("chat_inner speculative: draft accepted");
                        stream
                    }
                    echomind_core::speculative_rag::SpeculativeOutcome::DraftCorrected {
                        stream,
                        ..
                    } => {
                        debug!("chat_inner speculative: draft corrected");
                        stream
                    }
                    echomind_core::speculative_rag::SpeculativeOutcome::FallbackDirect {
                        stream,
                    } => {
                        debug!("chat_inner speculative: fallback to direct");
                        stream
                    }
                };

                (Some(spec_sources), spec_stream)
            }
        } else {
            // ---- 标准 RAG 流程 ----
            let mut engine = ChatEngine::new(retriever, provider);

            // REQ-PERF-002：集成 Prompt 压缩（仅压缩检索片段，不压缩系统提示和用户查询）
            let compression_ratio = state.compression_ratio;
            if compression_ratio > 1.0 {
                let compressor: Arc<dyn echomind_core::PromptCompressor> =
                    Arc::new(echomind_core::prompt_compressor::RuleBasedCompressor::new());
                engine = engine.with_compressor(compressor, compression_ratio);
                debug!("chat_inner prompt compression enabled (ratio={compression_ratio})");
            }

            // REQ-PERF-010：渐进式上下文注入（初始注入 top-2 → 检测"不确定"→ 追加）
            let progressive_enabled = settings_map
                .get("rag.progressive_injection")
                .is_some_and(|&v| v == "true");
            if progressive_enabled {
                engine = engine.with_progressive(2);
                debug!("chat_inner progressive injection enabled (initial=2)");
            }

            // REQ-RAG-028：质量门控（检索后评估结果质量，低质量时记录告警）
            if quality_gate_enabled {
                engine =
                    engine.with_quality_gate(echomind_core::quality_gate::GateConfig::default());
                debug!("chat_inner quality gate enabled (threshold=0.6)");
            }

            // REQ-RAG-032：持久化记忆注入（检索相关跨会话记忆，注入到 system prompt）
            let memory_enabled = settings_map
                .get("memory.enabled")
                .is_some_and(|&v| v == "true");
            if memory_enabled {
                let memory_store =
                    echomind_core::memory_store::MemoryStore::new(state.storage.clone());
                engine = engine.with_memory(std::sync::Arc::new(memory_store));
                debug!("chat_inner memory injection enabled");
            }

            // 标准 RAG：检索已与上下文压缩并行完成（见 compaction 处），
            // 此处直接使用预检索结果，跳过内部检索
            let t_retrieve = std::time::Instant::now();
            let prefetched = prefetched_sources.ok_or_else(|| {
                prefix_error(ERR_UNKNOWN, "标准 RAG 分支缺少预检索结果（内部状态不一致）")
            })?;

            // REQ-RAG-036：网页搜索补充 — 本地检索不足时搜索互联网
            //
            // 当 web_search_enabled 且本地检索 top-1 score < 阈值时，
            // 调用 DuckDuckGoProvider 搜索互联网，通过 RRF 融合到本地结果中。
            // 搜索失败时优雅降级为仅使用本地结果。
            let web_search_enabled = settings_map
                .get("rag.web_search_enabled")
                .is_some_and(|&v| v == "true");
            let prefetched = if web_search_enabled
                && echomind_core::web_search::should_search(
                    &prefetched,
                    echomind_core::web_search::DEFAULT_SEARCH_THRESHOLD,
                ) {
                match DuckDuckGoProvider::new() {
                    Ok(provider) => {
                        let provider_arc: Arc<dyn echomind_core::WebSearchProvider> =
                            Arc::new(provider);
                        let fused = echomind_core::web_search::search_and_fuse(
                            &provider_arc,
                            query,
                            prefetched,
                            DEFAULT_TOP_K,
                        )
                        .await;
                        debug!(
                            "chat_inner web search fused, {} results after fusion",
                            fused.len()
                        );
                        fused
                    }
                    Err(e) => {
                        warn!("DuckDuckGo provider 初始化失败，跳过网页搜索: {e:#}");
                        prefetched
                    }
                }
            } else {
                prefetched
            };

            let outcome = engine
                .chat_with_sources(compacted_history, query, prefetched)
                .await
                .map_err(|e| classify_llm_error(&format!("{e:#}")))?;
            debug!(
                "chat_inner retrieval (embed+search+expand): {}ms",
                t_retrieve.elapsed().as_millis()
            );

            match outcome {
                ChatOutcome::Answered {
                    sources,
                    stream,
                    progressive_info,
                    ..
                } => {
                    emit_chat_sources(app, &sources);
                    if let Some(info) = &progressive_info {
                        debug!(
                            "chat_inner progressive: injected {}/{} sources, can_expand={}",
                            info.injected_count, info.total_sources, info.can_expand
                        );
                    }
                    // 阶段 3：LLM 生成回答（首个 token 到达前的连接与推理延迟）
                    emit_chat_phase(app, "generating", "正在生成回答…");
                    debug!(
                        "chat_inner total pre-LLM: {}ms (from start)",
                        t0.elapsed().as_millis()
                    );
                    (Some(sources), stream)
                }
                ChatOutcome::NoContext { stream } => (None, stream),
            }
        };

        // REQ-PERF-012：自进化检索记忆 — 记录检索效果
        if state.retrieval_memory_enabled {
            let hybrid_enabled = settings_map
                .get("rag.hybrid_search")
                .is_none_or(|&v| v != "false");
            let rerank_enabled = settings_map
                .get("rag.rerank_enabled")
                // 默认启用（与主判定一致）
                .is_none_or(|&v| v != "false");
            let method = if !hybrid_enabled {
                echomind_core::retrieval_memory::RetrievalMethod::VectorOnly
            } else if rerank_enabled {
                echomind_core::retrieval_memory::RetrievalMethod::HybridRerank
            } else {
                echomind_core::retrieval_memory::RetrievalMethod::Hybrid
            };
            let results = sources.clone().unwrap_or_default();
            if let Err(e) =
                echomind_core::retrieval_memory::RetrievalMemoryEngine::new(state.storage.clone())
                    .record_retrieval(query, method, &results)
                    .await
            {
                warn!("retrieval memory record failed: {e:#}");
            }
        }

        // 推理内容：事件发射 + 落库累积（历史消息重现思考过程）
        let reasoning_collector = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        spawn_reasoning_forwarder(
            app,
            reasoning_handle.clone(),
            Some(reasoning_collector.clone()),
        );

        let abort_token = state.abort_token_for(conversation_id).await;
        let result = forward_stream(app, stream, abort_token, usage_handle)
            .await
            .map_err(|e| classify_llm_error(&e))?;
        // 记录 token 用量到累计计数器
        record_token_usage_inner(state, &result.token_usage).await;
        // 正常结束或被中断：均已生成内容照常落库（REQ-RAG-005/006）
        // REQ-PERF-001：正常回答写入查询缓存（下次相同/相似问题秒回）
        // Bug #4 修复：仅在有检索来源时写缓存，避免 NoContext 固定拒答文案被缓存
        // （此前 sources=None 时仍将"知识库中未找到相关内容"写入缓存，
        //   用户导入文档后仍返回旧缓存答案，直到 TTL 过期）
        if sources.is_some() {
            write_query_cache(state, query, &result.content, &sources, conversation_id).await;
        }
        let reasoning = reasoning_collector
            .lock()
            .ok()
            .map(|g| g.clone())
            .filter(|s| !s.is_empty());
        persist_exchange(
            state,
            conversation_id,
            query,
            &result.content,
            sources,
            reasoning,
            turn_group,
            version,
        )
        .await?;
        state.clear_abort(conversation_id).await;
        Ok(())
    }
}

/// 将 token 流逐条转发为 chat_token；abort 令牌触发立即中断（丢弃流即中止 HTTP 连接）。
///
/// `usage_handle` 在流消费完毕后从中读取 API 报告的 token 用量并随 `chat_done` 事件推送前端。
pub async fn forward_stream<R: Runtime>(
    app: &AppHandle<R>,
    mut stream: BoxStream<'static, anyhow::Result<String>>,
    abort_token: CancellationToken,
    usage_handle: Option<std::sync::Arc<tokio::sync::Mutex<Option<TokenUsage>>>>,
) -> Result<ForwardResult, String> {
    let mut content = String::new();
    let mut completed = false;
    let t_first_token = std::time::Instant::now();
    let mut first_token_logged = false;

    loop {
        tokio::select! {
            biased;
            _ = abort_token.cancelled() => break,
            item = stream.next() => {
            match item {
                None => {
                    completed = true;
                    break;
                }
                Some(Ok(token)) => {
                    if !first_token_logged {
                        debug!(
                            "forward_stream first token: {}ms (from forward_stream start)",
                            t_first_token.elapsed().as_millis()
                        );
                        first_token_logged = true;
                    }
                    content.push_str(&token);
                    emit_chat_token(app, token);
                }
                Some(Err(err)) => {
                    // Bug #3 修复：流式错误时补发 chat_done 事件，防止前端永久卡在 streaming 状态
                    // （此前直接 return Err 跳过了下方的 emit_chat_done，前端收不到完成信号）
                    emit_chat_done(app, None);
                    return Err(format!("{err:#}"));
                }
            }
            }
        }
    }

    if !completed {
        // 中断语义（REQ-RAG-005 冻结契约）：已生成内容保留并标记
        emit_chat_error(app, "⏹ 生成已中断".to_string());
    }
    let token_usage = if let Some(handle) = &usage_handle {
        handle.lock().await.take()
    } else {
        None
    };
    emit_chat_done(app, token_usage.clone());
    Ok(ForwardResult {
        completed,
        content,
        token_usage,
    })
}

/// 一轮问答落库（REQ-RAG-006）：首轮自动提取标题；空回答（秒中断）跳过 assistant 记录。
///
/// 使用 KeyedQueue 按 conversation_id 序列化（Q09 借鉴 QM createKeyedQueue），
/// 消除同一会话并发 persist_exchange 调用的 SQLite 写入竞态。
///
/// # 参数
/// - `reasoning`: 助手消息的推理思考过程（reasoning_content，推理模型才有），
///   落库持久化，历史消息加载时可重现思考过程（P2-1 之后的修复）。
#[allow(clippy::too_many_arguments)]
pub async fn persist_exchange(
    state: &AppState,
    conversation_id: &str,
    query: &str,
    answer: &str,
    sources: Option<Vec<RetrievalResult>>,
    reasoning: Option<String>,
    turn_group: Option<&str>,
    version: Option<i32>,
) -> Result<(), String> {
    // 按 conversation_id 序列化：同一会话的 persist_exchange 串行执行，
    // 不同会话并行执行，不影响跨会话性能。
    let key = conversation_id.to_string();
    state
        .persist_queue
        .run(key, || async {
            persist_exchange_inner(
                state,
                conversation_id,
                query,
                answer,
                sources,
                reasoning,
                turn_group,
                version,
            )
            .await
        })
        .await
}

/// persist_exchange 的内部实现（无序列化，直接写入 SQLite）。
///
/// 由 `persist_exchange` 通过 KeyedQueue 调用，确保同会话串行。
#[allow(clippy::too_many_arguments)]
async fn persist_exchange_inner(
    state: &AppState,
    conversation_id: &str,
    query: &str,
    answer: &str,
    sources: Option<Vec<RetrievalResult>>,
    reasoning: Option<String>,
    turn_group: Option<&str>,
    version: Option<i32>,
) -> Result<(), String> {
    // 首轮问答后提取标题（仅在尚无历史消息时执行，避免覆盖后续标题）
    // Bug #8 修复：改用 count_messages 轻量查询，避免加载+反序列化全部消息
    let msg_count = state
        .storage
        .count_messages(conversation_id)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;
    if msg_count == 0 {
        // 立即设置 derive_title 作为初始标题（快速，无 LLM 调用）
        state
            .storage
            .update_conversation_title(conversation_id, &derive_title(query))
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

        // Q10：LLM 辅助标题生成（借鉴 QM generateTitle）
        // 仅在 Remote (OpenAI) 模式下尝试，避免本地模型额外加载开销
        // 10 秒超时，失败/超时/None 均静默降级为 derive_title
        let llm_mode = state.get_llm_mode().await;
        if llm_mode == LlmMode::Remote {
            let llm_config = state.llm_config().read().await.clone();
            if let Some(config) = llm_config
                && let Ok(provider) = OpenAIProvider::new(
                    config.api_key.clone(),
                    config.base_url.clone(),
                    config.model.clone(),
                )
            {
                // 截断 answer 防止超长转录消耗过多 token
                let answer_excerpt: String = answer.chars().take(200).collect();
                let transcript = format!("用户: {query}\n助手: {answer_excerpt}");
                match tokio::time::timeout(
                    std::time::Duration::from_secs(10),
                    provider.generate_title(&transcript),
                )
                .await
                {
                    Ok(Ok(Some(title))) if !title.is_empty() => {
                        if let Err(e) = state
                            .storage
                            .update_conversation_title(conversation_id, &title)
                            .await
                        {
                            warn!("LLM 标题更新失败，保留 derive_title: {e:#}");
                        }
                    }
                    // None / 空标题 / 不支持：保留 derive_title
                    Ok(Ok(_)) => {}
                    Ok(Err(e)) => {
                        warn!("LLM 标题生成失败，保留 derive_title: {e:#}");
                    }
                    Err(_) => {
                        warn!("LLM 标题生成超时（10s），保留 derive_title");
                    }
                }
            }
        }
    }

    // 当 turn_group 为 Some 时，用户消息已由 edit_user_message IPC 保存，
    // 此处仅保存 assistant 消息（避免重复）
    if turn_group.is_none() {
        let user_msg = ChatMessage {
            id: None,
            role: "user".to_string(),
            content: query.to_string(),
            sources: None,
            reasoning: None,
            turn_group: turn_group.map(|s| s.to_string()),
            version,
        };
        state
            .storage
            .add_message(conversation_id, &user_msg)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;
    }

    if !answer.is_empty() {
        let assistant_msg = ChatMessage {
            id: None,
            role: "assistant".to_string(),
            content: answer.to_string(),
            sources,
            reasoning,
            turn_group: turn_group.map(|s| s.to_string()),
            version,
        };
        state
            .storage
            .add_message(conversation_id, &assistant_msg)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;
    }
    Ok(())
}

/// 导出会话为 Markdown 字符串（REQ-EXP-001）。
///
/// 加载会话元数据与全部消息，生成 Markdown 格式的导出内容。
/// 前端拿到 Markdown 后通过 Tauri dialog 的 save 对话框选择保存位置，
/// 再调用 `save_text_file` 写入文件。
///
/// # 返回
/// `(markdown_content, default_filename)` — Markdown 字符串和默认文件名（会话标题清洗后）。
#[tauri::command]
pub async fn export_conversation_markdown(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(String, String), String> {
    export_conversation_markdown_inner(&conversation_id, state.inner()).await
}

/// 导出会话为 Markdown 逻辑（命令与集成测试复用）。
///
/// # 返回
/// `(markdown_content, default_filename)` — Markdown 字符串和默认文件名。
/// 会话不存在时返回 `Err("会话不存在")`。
pub async fn export_conversation_markdown_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<(String, String), String> {
    let conversation = state
        .storage
        .get_conversation(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    let conversation = conversation.ok_or_else(|| format!("会话不存在: {conversation_id}"))?;

    let messages = state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    let markdown = export_conversation_to_markdown(&conversation, &messages);
    let filename = sanitize_filename(&conversation.title);
    Ok((markdown, filename))
}

/// 保存文本文件到指定路径（REQ-EXP-001 辅助命令）。
///
/// 前端通过 Tauri dialog 的 save 对话框获取保存路径后调用本命令写入文件。
/// 文件以 UTF-8 编码写入。
#[tauri::command]
pub async fn save_text_file(path: String, content: String) -> Result<(), String> {
    save_text_file_inner(&path, &content).await
}

/// 保存文本文件逻辑（命令与集成测试复用）。
///
/// 将 `content` 以 UTF-8 编码写入 `path` 指定的文件。
/// 路径的父目录必须存在（本函数不自动创建目录）。
pub async fn save_text_file_inner(path: &str, content: &str) -> Result<(), String> {
    tokio::fs::write(path, content.as_bytes())
        .await
        .map_err(|e| format!("文件保存失败: {e:#}"))
}

/// 读取文本文件内容（REQ-EXP-003 辅助命令）。
///
/// 前端通过 Tauri dialog 的 open 对话框获取文件路径后调用本命令读取文件。
/// 文件以 UTF-8 编码读取。
#[tauri::command]
pub async fn read_text_file(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("文件读取失败: {e:#}"))
}

/// 记录单次对话的 token 用量到累计计数器。
///
/// 在 `forward_stream` 返回后由 `chat_inner` 调用。读取 settings 表中的累计计数器，
/// 加上本次用量后写回。错误仅日志，不影响主流程。
pub async fn record_token_usage_inner(state: &AppState, usage: &Option<TokenUsage>) {
    let Some(usage) = usage else {
        return; // 本地推理模式无 usage 数据，跳过
    };
    // 读取现有累计值
    let cur_prompt: u64 = state
        .storage
        .get_setting("usage.total_prompt_tokens")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let cur_completion: u64 = state
        .storage
        .get_setting("usage.total_completion_tokens")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let cur_exchange: u32 = state
        .storage
        .get_setting("usage.exchange_count")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    // 累加
    let new_prompt = cur_prompt + usage.prompt_tokens as u64;
    let new_completion = cur_completion + usage.completion_tokens as u64;
    let new_total = new_prompt + new_completion;
    let new_exchange = cur_exchange.saturating_add(1);

    // 写回（容错：失败仅日志，不阻断主流程）
    if let Err(e) = state
        .storage
        .set_setting("usage.total_prompt_tokens", &new_prompt.to_string())
        .await
    {
        warn!("记录 token 用量失败(prompt): {e:#}");
    }
    if let Err(e) = state
        .storage
        .set_setting("usage.total_completion_tokens", &new_completion.to_string())
        .await
    {
        warn!("记录 token 用量失败(completion): {e:#}");
    }
    if let Err(e) = state
        .storage
        .set_setting("usage.total_tokens", &new_total.to_string())
        .await
    {
        warn!("记录 token 用量失败(total): {e:#}");
    }
    if let Err(e) = state
        .storage
        .set_setting("usage.exchange_count", &new_exchange.to_string())
        .await
    {
        warn!("记录 token 用量失败(exchange): {e:#}");
    }

    // Q08: 记录预算使用情况（QM 借鉴）
    if let Err(e) = record_budget_usage_inner(state, usage).await {
        eprintln!("记录预算使用失败: {e:#}");
    }
}

/// Q08: 记录预算使用情况（QM 借鉴）。
///
/// 在 token 使用记录后调用，计算 LLM API 费用并更新预算追踪器。
async fn record_budget_usage_inner(state: &AppState, usage: &TokenUsage) -> Result<(), String> {
    // 获取 LLM 配置（模型名称用于记录）
    let llm_config = state.llm_config().read().await.clone();
    let Some(config) = llm_config else {
        return Err("未配置 LLM".to_string());
    };

    // 获取价格配置（环境变量优先，默认值兜底）
    let usd_per_mtok = std::env::var("LLM_COST_PER_MTOK")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(5.0); // 默认 $5.0/MTok

    // 估算成本（使用 QM 的成本估算方法）
    let input_cost = estimate_cost_usd(usage.prompt_tokens as usize, usd_per_mtok);
    let output_cost = estimate_cost_usd(usage.completion_tokens as usize, usd_per_mtok);
    let total_cost = input_cost + output_cost;

    // 检查预算限制
    let principal = "default_user";
    match state.budget_tracker.check(principal).await {
        echomind_core::budget::BudgetCheck::Denied { spent, limit } => {
            eprintln!(
                "超出预算限制：已使用 {:.4} USD，限制 {:.4} USD",
                spent, limit
            );
            return Err(format!(
                "超出预算限制：已使用 {:.4} USD，限制 {:.4} USD",
                spent, limit
            ));
        }
        echomind_core::budget::BudgetCheck::Allowed => {
            // 预算检查通过，记录使用
        }
    }

    // 记录到数据库
    state
        .storage
        .record_budget_usage(
            principal,
            usage.prompt_tokens as usize,
            usage.completion_tokens as usize,
            total_cost,
            &config.model,
        )
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 更新内存中的预算追踪器
    state.budget_tracker.record(principal, total_cost).await;

    Ok(())
}

/// 获取累计对话成本（token 用量统计）。
///
/// 从 settings 表读取累计计数器，返回 `ConversationCost` 结构。
/// 前端据此展示累计 token 消耗与预算使用进度条。
#[tauri::command]
pub async fn get_conversation_cost(state: State<'_, AppState>) -> Result<ConversationCost, String> {
    get_conversation_cost_inner(state.inner()).await
}

/// 对话成本读取逻辑（命令与集成测试复用）。
pub async fn get_conversation_cost_inner(state: &AppState) -> Result<ConversationCost, String> {
    let total_prompt_tokens = state
        .storage
        .get_setting("usage.total_prompt_tokens")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let total_completion_tokens = state
        .storage
        .get_setting("usage.total_completion_tokens")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let total_tokens = state
        .storage
        .get_setting("usage.total_tokens")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(total_prompt_tokens + total_completion_tokens);
    let exchange_count = state
        .storage
        .get_setting("usage.exchange_count")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    let token_budget = state
        .storage
        .get_setting("usage.token_budget")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    Ok(ConversationCost {
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
        exchange_count,
        token_budget,
    })
}

/// 设置 token 预算上限。
///
/// `budget` 为 0 表示不限制。写入 settings 表 `usage.token_budget` 键。
/// `get_settings` 和 `get_conversation_cost` 返回此值。
#[tauri::command]
pub async fn set_token_budget(budget: u64, state: State<'_, AppState>) -> Result<(), String> {
    set_token_budget_inner(budget, state.inner()).await
}

/// Token 预算写入逻辑（命令与集成测试复用）。
pub async fn set_token_budget_inner(budget: u64, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("usage.token_budget", &budget.to_string())
        .await
        .map_err(|e| format!("{e:#}"))
}

// ==================================================================
// B05: Durable Prompt Admission — 持久化提示接纳 IPC 命令
// ==================================================================

// ============================================================================
// Durable Prompt Admission（B05）— S10: 开发者工具门控
// ============================================================================

/// 接收待处理输入（B05 Durable Prompt Admission）。
///
/// 将用户输入存入 `pending_inputs` 表，等待后续提升为正式消息。
/// 投递模式 `steer` 优先中断当前生成，`queue` 排队等待。
///
/// # 参数
/// - `conversation_id`: 所属会话 ID
/// - `content`: 用户输入内容
/// - `delivery`: 投递模式（`"steer"` 优先中断 / `"queue"` 排队等待）
///
/// # 返回
/// 新创建的接纳记录 ID。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn admit_input(
    conversation_id: String,
    content: String,
    delivery: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    admit_input_inner(&conversation_id, &content, &delivery, state.inner()).await
}

/// `admit_input` 的逻辑实现（命令与集成测试复用）。
pub async fn admit_input_inner(
    conversation_id: &str,
    content: &str,
    delivery: &str,
    state: &AppState,
) -> Result<String, String> {
    state
        .storage
        .admit_input(conversation_id, content, delivery)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 提升接纳记录为正式消息（B05 Durable Prompt Admission）。
///
/// 将指定接纳记录标记为已提升（设置 `promoted_seq`），
/// 使其成为消息历史的一部分。
///
/// # 参数
/// - `input_id`: 接纳记录 ID
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn promote_input(input_id: String, state: State<'_, AppState>) -> Result<(), String> {
    promote_input_inner(&input_id, state.inner()).await
}

/// `promote_input` 的逻辑实现（命令与集成测试复用）。
pub async fn promote_input_inner(input_id: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .promote_input(input_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 获取会话的待处理输入列表（B05 Durable Prompt Admission）。
///
/// 返回指定会话中所有未提升的接纳记录，按优先级排序：
/// `steer` 模式排在前面，然后按创建时间 FIFO 排序。
///
/// # 参数
/// - `conversation_id`: 所属会话 ID
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn get_pending_inputs(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<PendingInput>, String> {
    get_pending_inputs_inner(&conversation_id, state.inner()).await
}

/// `get_pending_inputs` 的逻辑实现（命令与集成测试复用）。
pub async fn get_pending_inputs_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<Vec<PendingInput>, String> {
    state
        .storage
        .get_pending_inputs(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================================
// Session Todo 持久化（B08 会话待办持久化，REQ-RAG-044）— S10: 开发者工具门控
// ============================================================================

/// 创建会话 Todo 项（REQ-RAG-044）。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn add_session_todo(
    conversation_id: String,
    content: String,
    position: i64,
    state: State<'_, AppState>,
) -> Result<String, String> {
    add_session_todo_inner(&conversation_id, &content, position, state.inner()).await
}

/// `add_session_todo` 的逻辑实现（命令与集成测试复用）。
pub async fn add_session_todo_inner(
    conversation_id: &str,
    content: &str,
    position: i64,
    state: &AppState,
) -> Result<String, String> {
    let todo = SessionTodo::new(conversation_id.to_string(), content.to_string(), position);
    let id = todo.id.clone();
    state
        .storage
        .add_session_todo(&todo)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(id)
}

/// 更新 Todo 状态（REQ-RAG-044）。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn update_todo_status(
    todo_id: String,
    status: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    update_todo_status_inner(&todo_id, &status, state.inner()).await
}

/// `update_todo_status` 的逻辑实现。
pub async fn update_todo_status_inner(
    todo_id: &str,
    status: &str,
    state: &AppState,
) -> Result<(), String> {
    let todo_status = TodoStatus::from_db_str(status).ok_or_else(|| {
        format!("无效的 Todo 状态: {status}（应为 pending/in_progress/completed）")
    })?;
    state
        .storage
        .update_todo_status(todo_id, &todo_status)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 获取会话 Todo 列表（REQ-RAG-044）。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn get_session_todos(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SessionTodo>, String> {
    get_session_todos_inner(&conversation_id, state.inner()).await
}

/// `get_session_todos` 的逻辑实现。
pub async fn get_session_todos_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<Vec<SessionTodo>, String> {
    state
        .storage
        .get_session_todos(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除单个 Todo 项（REQ-RAG-044）。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn delete_session_todo(
    todo_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_session_todo_inner(&todo_id, state.inner()).await
}

/// `delete_session_todo` 的逻辑实现。
pub async fn delete_session_todo_inner(todo_id: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .delete_session_todo(todo_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除会话的全部 Todo 项（REQ-RAG-044）。
#[cfg(debug_assertions)]
#[tauri::command]
pub async fn delete_session_todos(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_session_todos_inner(&conversation_id, state.inner()).await
}

/// `delete_session_todos` 的逻辑实现。
pub async fn delete_session_todos_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<(), String> {
    state
        .storage
        .delete_session_todos(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))
}
