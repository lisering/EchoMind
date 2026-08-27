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

/// REQ-PERF-018：合并图扩展结果到标准检索结果（确定性，纯函数）。
///
/// 并行与串行路径复用同一合并逻辑，保证结果一致：
/// - 已存在的 chunk：保留较高分数（图扩展可能发现更高置信度的路径）
/// - 新 chunk：追加到结果列表
/// - 重新按分数降序排序，截断 `top_k * 2`
///
/// # 参数
/// - `rr`: 标准检索结果（原地修改）
/// - `graph_results`: 图扩展结果
/// - `top_k`: 截断基准
pub(crate) fn merge_graph_results(
    rr: &mut Vec<RetrievalResult>,
    graph_results: Vec<RetrievalResult>,
    top_k: usize,
) {
    if graph_results.is_empty() {
        return;
    }
    let existing_ids: std::collections::HashSet<String> =
        rr.iter().map(|r| r.chunk.id.clone()).collect();
    for gr in graph_results {
        if existing_ids.contains(&gr.chunk.id) {
            if let Some(existing) = rr.iter_mut().find(|r| r.chunk.id == gr.chunk.id)
                && gr.score > existing.score
            {
                existing.score = gr.score;
            }
        } else {
            rr.push(gr);
        }
    }
    rr.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rr.truncate(top_k * 2);
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

    // 演示模式检测（REQ-RAG-051）：无需 LLM Key，使用关键词匹配模板回复
    let demo_mode = state
        .storage
        .get_setting("rag.demo_mode")
        .await
        .map(|v| v.is_some_and(|v| v == "true"))
        .unwrap_or(false);
    if demo_mode {
        return handle_demo_mode(app, query, state, conversation_id, turn_group, version).await;
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
    //
    // **P2-6 弹性降级**：嵌入引擎初始化失败/超时时不再直接返回错误，而是降级到
    // 纯关键词搜索（BM25）模式。用户仍可获取基于关键词的 RAG 回答，并收到降级提示。
    emit_chat_phase(app, "preparing", "初始化向量化引擎…");
    let mut embedding_degraded = false;
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
                // P2-6：降级到关键词搜索模式（不返回错误）
                warn!("向量化引擎不可用，降级为关键词搜索: {e:#}");
                embedding_degraded = true;
                emit_chat_phase(app, "preparing", "向量化引擎不可用，降级为关键词搜索…");
            }
            Err(_) => {
                // P2-6：超时也降级到关键词搜索模式
                warn!("向量化引擎初始化超时（{timeout_secs}s），降级为关键词搜索");
                embedding_degraded = true;
                emit_chat_phase(app, "preparing", "向量化引擎超时，降级为关键词搜索…");
            }
        }
    }
    let embedder = if !embedding_degraded {
        match state.embedder().await {
            Ok(e) => {
                let e = e.clone();
                debug!("chat_inner embedder ready: {}ms", t0.elapsed().as_millis());
                Some(e)
            }
            Err(e) => {
                // P2-6：embedder 获取失败也降级
                warn!("向量化引擎获取失败，降级为关键词搜索: {e:#}");
                embedding_degraded = true;
                None
            }
        }
    } else {
        None
    };

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
            "rag.false",
            "rag.retrieval_memory_enabled",
            "rag.graph_retriever_enabled",
            "rag.false",
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

    // 查询嵌入：用于向量检索（缓存已删除，不再有 L0/L1/L3 缓存层）
    let query_embedding: Option<Vec<f32>> = if !embedding_degraded {
        Some(
            embedder
                .as_ref()
                .ok_or_else(|| prefix_error(ERR_EMBED, "向量化引擎不可用"))?
                .embed(query)
                .await
                .map_err(|e| prefix_error(ERR_EMBED, &format!("查询向量化失败: {e:#}")))?,
        )
    } else {
        // P2-6：嵌入降级模式，无查询嵌入
        None
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

    // P2-6：嵌入降级路径 — 纯关键词搜索（BM25）+ 简单 RAG prompt + LLM 流式对话
    //
    // 当 ONNX 嵌入引擎初始化失败/超时时，不阻塞用户对话。降级为纯关键词检索，
    // 精度略低于向量检索但功能可用。前端通过 chat_phase 事件收到降级提示。
    if embedding_degraded {
        emit_chat_phase(app, "retrieving", "关键词搜索中（向量检索降级）…");
        let keyword_results = state
            .storage
            .keyword_search(query, rag_top_k)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("关键词检索失败: {e:#}")))?;

        if keyword_results.is_empty() {
            // 关键词也未命中
            emit_chat_error(app, "知识库中未找到相关内容".to_string());
            emit_chat_done(app, None);
            return Ok(());
        }

        let sources = keyword_results.clone();
        emit_chat_sources(app, &sources);

        // 构建 RAG prompt（简化版：关键词检索结果 + 历史 + 查询）
        let context_text: String = sources
            .iter()
            .enumerate()
            .map(|(i, r)| format!("[{}] {}\n{}", i + 1, r.doc_name, r.chunk.content))
            .collect::<Vec<_>>()
            .join("\n\n");
        let system_prompt = format!(
            "你是一个知识库助手。请根据以下检索到的资料片段回答用户问题。\
             如果资料中没有答案，请如实告知。\n\n\
             ⚠️ 当前为关键词搜索降级模式（向量检索不可用），检索精度可能降低。\n\n\
             参考资料：\n{context_text}"
        );

        // 上下文压缩（复用已有逻辑：compact_smart 异步压缩）
        let max_context_tokens = settings_map
            .get("rag.context_token_limit")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(4096);
        let compaction = CompactionEngine::new(&provider);
        let compaction_result = compaction
            .compact_smart(
                history,
                max_context_tokens,
                &echomind_compact::VerbatimTailConfig::default(),
            )
            .await
            .map_err(|e| prefix_error(ERR_LLM, &format!("上下文压缩失败: {e:#}")))?;
        let compacted_history = &compaction_result.history;

        emit_chat_phase(app, "generating", "正在生成回答…");
        let stream = provider
            .chat_stream(&system_prompt, compacted_history, query)
            .await
            .map_err(|e| classify_llm_error(&format!("{e:#}")))?;

        let abort_token = state.abort_token_for(conversation_id).await;
        let result =
            forward_stream_tracked(app, stream, abort_token, usage_handle, state, llm_mode)
                .await
                .map_err(|e| classify_llm_error(&e))?;
        record_token_usage_inner(state, &result.token_usage).await;

        persist_exchange(
            state,
            conversation_id,
            query,
            &result.content,
            Some(sources),
            None,
            turn_group,
            version,
        )
        .await?;
        state.clear_abort(conversation_id).await;
        return Ok(());
    }

    // ── 正常路径（嵌入可用）──

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
        // P2-6：降级路径已提前 return，此处 embedder 必为 Some
        let emb = embedder
            .as_ref()
            .ok_or_else(|| prefix_error(ERR_EMBED, "向量化引擎不可用（降级路径未正确触发）"))?
            .clone();
        let mut r = HybridRetriever::new(emb, state.storage.clone());
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
    let use_parallel_retrieval = !coordinator_enabled && !agent_enabled;
    let (compaction_result, prefetched_sources) = if use_parallel_retrieval {
        emit_chat_phase(app, "retrieving", "检索知识库…");
        // 并行 compaction + 检索（使用预计算嵌入）
        // Q03：Synchronous 触发 → 调用 compact()；None/Background → 跳过压缩
        {
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
            // REQ-PERF-018：图扩展与主检索并行执行（BFS 图遍历与向量+BM25 互不依赖）。
            // 图扩展仅在标准 RAG 路径（非 coordinator/agent/speculative）且启用时执行。
            let graph_fut = async {
                if graph_retriever_enabled {
                    let graph_retriever =
                        echomind_core::graph_retriever::GraphRetriever::new(state.storage.clone());
                    graph_retriever.expand(query, rag_top_k).await
                } else {
                    Ok(Vec::<RetrievalResult>::new())
                }
            };
            let (cr, rr, graph_result) = tokio::join!(compact_fut, retrieve_fut, graph_fut);
            let cr = cr.map_err(|e| prefix_error(ERR_LLM, &format!("上下文压缩失败: {e:#}")))?;
            let mut rr = rr.map_err(|e| classify_llm_error(&format!("检索失败: {e:#}")))?;

            // REQ-RAG-027：知识图谱图遍历检索 — 沿实体关系图边扩展到关联 chunk
            // 图扩展作为 RRF 融合的第四路检索通道，为标准检索结果提供图扩展加成。
            // REQ-PERF-018：图扩展已在检索阶段并行完成，此处仅做确定性合并。
            if graph_retriever_enabled && !rr.is_empty() {
                match graph_result {
                    Ok(graph_results) if !graph_results.is_empty() => {
                        debug!(
                            "图扩展返回 {} 个关联 chunk，合并到检索结果",
                            graph_results.len()
                        );
                        merge_graph_results(&mut rr, graph_results, rag_top_k);
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
        let coordinator = CoordinatorEngine::new(retriever, provider);
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
                let result =
                    forward_stream_tracked(app, stream, abort_token, None, state, llm_mode)
                        .await
                        .map_err(|e| classify_llm_error(&e))?;
                record_token_usage_inner(state, &result.token_usage).await;
                // REQ-PERF-001：正常回答写入查询缓存（下次相同/相似问题秒回）
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
        let agent = AgentEngine::new(retriever, provider);
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
                let result =
                    forward_stream_tracked(app, stream, abort_token, None, state, llm_mode)
                        .await
                        .map_err(|e| classify_llm_error(&e))?;
                // 记录 token 用量到累计计数器
                record_token_usage_inner(state, &result.token_usage).await;
                // REQ-PERF-001：正常回答写入查询缓存（下次相同/相似问题秒回）
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
        // ---- 标准 RAG 流程 ----
        let (sources, stream) = {
            let mut engine = ChatEngine::new(retriever, provider);

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
                    sources, stream, ..
                } => {
                    emit_chat_sources(app, &sources);
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

        // 推理内容：事件发射 + 落库累积（历史消息重现思考过程）
        let reasoning_collector = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        spawn_reasoning_forwarder(
            app,
            reasoning_handle.clone(),
            Some(reasoning_collector.clone()),
        );

        let abort_token = state.abort_token_for(conversation_id).await;
        let result =
            forward_stream_tracked(app, stream, abort_token, usage_handle, state, llm_mode)
                .await
                .map_err(|e| classify_llm_error(&e))?;
        // 记录 token 用量到累计计数器
        record_token_usage_inner(state, &result.token_usage).await;
        // 正常结束或被中断：均已生成内容照常落库（REQ-RAG-005/006）
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

/// 首 token 超时（秒）：P2-5 弹性设计增强。
///
/// 如果 LLM 端点在 60 秒内未返回任何 token，判定为连接异常（而非正常思考延迟），
/// 提前返回 `NETWORK:` 错误，避免用户等待 300s 整体超时。
///
/// 300s 整体超时仍由 reqwest client `.timeout()` 兜底（防网络层永久挂起）。
const FIRST_TOKEN_TIMEOUT_SECS: u64 = 60;

/// 包装 `forward_stream` 并跟踪远程 LLM 成功/失败（P2-2 弹性降级）。
///
/// 当 `llm_mode == Remote` 时：
/// - 成功完成（`completed == true`）：调用 `record_remote_success()` 重置计数
/// - 失败（`Err`）：调用 `record_remote_failure()`，如果达到阈值自动切换到 Local
///   并通过 `llm_fallback` 事件通知前端
/// - 中断（`completed == false`）：不记录成功也不记录失败（用户主动取消）
///
/// Local 模式不触发任何跟踪。
async fn forward_stream_tracked<R: Runtime>(
    app: &AppHandle<R>,
    stream: futures::stream::BoxStream<'static, anyhow::Result<String>>,
    abort_token: tokio_util::sync::CancellationToken,
    usage_handle: Option<std::sync::Arc<tokio::sync::Mutex<Option<TokenUsage>>>>,
    state: &AppState,
    llm_mode: LlmMode,
) -> Result<ForwardResult, String> {
    let result = forward_stream(app, stream, abort_token, usage_handle).await;

    // P2-2：仅跟踪 Remote 模式的成功/失败
    if llm_mode == LlmMode::Remote {
        match &result {
            Ok(fr) if fr.completed => {
                // 成功完成：重置计数
                state.llm_router.record_remote_success();
            }
            Ok(_) => {
                // 中断（completed=false）：用户主动取消，不计为失败
            }
            Err(_) => {
                // 失败：递增计数，达阈值自动切换
                let switched = state.llm_router.record_remote_failure().await;
                if switched {
                    let _ = app.emit(
                        "llm_fallback",
                        "远程 LLM 连续失败 3 次，已自动切换到本地模型",
                    );
                    info!("远程 LLM 连续失败 3 次，已自动切换 fallback 到 Local 模式");
                }
            }
        }
    }

    result
}

/// 将 token 流逐条转发为 chat_token；abort 令牌触发立即中断（丢弃流即中止 HTTP 连接）。
///
/// `usage_handle` 在流消费完毕后从中读取 API 报告的 token 用量并随 `chat_done` 事件推送前端。
///
/// **P2-5 首 token 超时**：在收到第一个 token 之前，如果 `FIRST_TOKEN_TIMEOUT_SECS`
/// 秒内无数据到达，返回 `NETWORK:` 前缀错误（"LLM 首次响应超时"），避免用户等待
/// 300s 整体超时。收到首 token 后此超时不再触发。
pub async fn forward_stream<R: Runtime>(
    app: &AppHandle<R>,
    mut stream: BoxStream<'static, anyhow::Result<String>>,
    abort_token: CancellationToken,
    usage_handle: Option<std::sync::Arc<tokio::sync::Mutex<Option<TokenUsage>>>>,
) -> Result<ForwardResult, String> {
    let mut content = String::new();
    let mut completed = false;
    let t_first_token = std::time::Instant::now();

    // P2-5：首 token 超时检测。
    // 在收到第一个 token 之前，用 tokio::time::timeout 包装 stream.next()。
    // 收到首 token 后直接使用裸 stream.next()（不再有超时分支）。
    // 300s 整体超时仍由 reqwest client `.timeout()` 兜底。
    let first_token_deadline = std::time::Duration::from_secs(FIRST_TOKEN_TIMEOUT_SECS);

    // 阶段 1：等待首 token（带 60s 超时）
    let first_item = tokio::select! {
        biased;
        _ = abort_token.cancelled() => {
            // 在首 token 到达前被中断
            emit_chat_done(app, None);
            return Ok(ForwardResult {
                completed: false,
                content: String::new(),
                token_usage: None,
            });
        }
        result = tokio::time::timeout(first_token_deadline, stream.next()) => {
            match result {
                Ok(item) => item,
                Err(_) => {
                    // P2-5：首 token 超时
                    emit_chat_error(app, "LLM 响应超时，请检查网络或 API 状态".to_string());
                    emit_chat_done(app, None);
                    return Err(format!(
                        "NETWORK: LLM 首次响应超时（{FIRST_TOKEN_TIMEOUT_SECS}s 无数据），请检查网络连接或 API 端点状态"
                    ));
                }
            }
        }
    };

    // 处理首 token
    match first_item {
        None => {
            // 流在首 token 前就结束了（空响应）
            completed = true;
        }
        Some(Ok(token)) => {
            debug!(
                "forward_stream first token: {}ms (from forward_stream start)",
                t_first_token.elapsed().as_millis()
            );
            content.push_str(&token);
            emit_chat_token(app, token);
        }
        Some(Err(err)) => {
            // Bug #3 修复：流式错误时补发 chat_done 事件
            emit_chat_done(app, None);
            return Err(format!("{err:#}"));
        }
    }

    // 阶段 2：首 token 已收到，后续 token 无首 token 超时（300s 整体超时由 reqwest 兜底）
    if !completed {
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
    // 性能优化：3 次串行 get_setting → 1 次批量读取
    let settings = state
        .storage
        .get_settings_batch(&[
            "usage.total_prompt_tokens",
            "usage.total_completion_tokens",
            "usage.exchange_count",
        ])
        .await
        .unwrap_or_default();
    let settings_map: std::collections::HashMap<&str, &str> = settings
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    let cur_prompt: u64 = settings_map
        .get("usage.total_prompt_tokens")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let cur_completion: u64 = settings_map
        .get("usage.total_completion_tokens")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let cur_exchange: u32 = settings_map
        .get("usage.exchange_count")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    // 累加
    let new_prompt = cur_prompt + usage.prompt_tokens as u64;
    let new_completion = cur_completion + usage.completion_tokens as u64;
    let new_total = new_prompt + new_completion;
    let new_exchange = cur_exchange.saturating_add(1);

    // 性能优化：4 次串行 set_setting → 1 次批量事务写入（容错：失败仅日志）
    let prompt_str = new_prompt.to_string();
    let completion_str = new_completion.to_string();
    let total_str = new_total.to_string();
    let exchange_str = new_exchange.to_string();
    if let Err(e) = state
        .storage
        .set_settings_batch(&[
            ("usage.total_prompt_tokens", prompt_str.as_str()),
            ("usage.total_completion_tokens", completion_str.as_str()),
            ("usage.total_tokens", total_str.as_str()),
            ("usage.exchange_count", exchange_str.as_str()),
        ])
        .await
    {
        warn!("记录 token 用量失败: {e:#}");
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::assertions_on_constants
    )]

    use super::FIRST_TOKEN_TIMEOUT_SECS;
    use super::merge_graph_results;
    use echomind_models::{Chunk, RetrievalResult};

    /// 辅助：构造带指定分数与 chunk_id 的 RetrievalResult。
    fn mk_result(id: &str, score: f32) -> RetrievalResult {
        RetrievalResult {
            chunk: Chunk {
                id: id.to_string(),
                doc_id: "doc".to_string(),
                content: String::new(),
                token_count: 1,
                sequence: 0,
            },
            score,
            doc_name: "doc.md".to_string(),
        }
    }

    /// TC-PARALLEL-001：合并已有 chunk 保留较高分数 + 新 chunk 追加 + 降序截断。
    #[test]
    fn tc_parallel_001_merge_boosts_and_appends() {
        let mut rr = vec![mk_result("a", 0.8), mk_result("b", 0.5)];
        let graph = vec![mk_result("a", 0.9), mk_result("c", 0.7)];

        merge_graph_results(&mut rr, graph, 3);

        // a 分数从 0.8 → 0.9，c 追加，截断 3*2=6（不截断 3 项）
        assert_eq!(rr.len(), 3);
        assert_eq!(rr[0].chunk.id, "a", "a 分数提升后应排第一");
        assert!((rr[0].score - 0.9).abs() < 1e-6);
        assert_eq!(rr[1].chunk.id, "c");
        assert_eq!(rr[2].chunk.id, "b");
    }

    /// TC-PARALLEL-002：空图结果不修改标准检索结果（确定性）。
    #[test]
    fn tc_parallel_002_empty_graph_noop() {
        let mut rr = vec![mk_result("a", 0.8), mk_result("b", 0.5)];
        merge_graph_results(&mut rr, vec![], 3);
        assert_eq!(rr.len(), 2);
        assert_eq!(rr[0].chunk.id, "a");
    }

    /// TC-PARALLEL-003：截断 top_k*2 生效。
    #[test]
    fn tc_parallel_003_truncates_to_2k() {
        let mut rr = (0..5)
            .map(|i| mk_result(&format!("r{i}"), 0.9 - i as f32 * 0.1))
            .collect::<Vec<_>>();
        let graph = (0..3)
            .map(|i| mk_result(&format!("g{i}"), 0.5 - i as f32 * 0.05))
            .collect::<Vec<_>>();
        merge_graph_results(&mut rr, graph, 3);
        assert_eq!(rr.len(), 6, "截断为 top_k*2 = 6");
        // 分数降序
        for w in rr.windows(2) {
            assert!(w[0].score >= w[1].score);
        }
    }

    /// TC-FIRST-TOKEN-001: 首 token 超时常量应为 60 秒。
    #[test]
    fn tc_first_token_001_timeout_is_60_secs() {
        assert_eq!(FIRST_TOKEN_TIMEOUT_SECS, 60, "首 token 超时应为 60 秒");
    }

    /// TC-FIRST-TOKEN-002: 首 token 超时应小于整体超时 300 秒。
    #[test]
    fn tc_first_token_002_less_than_overall_timeout() {
        // 整体超时 REQUEST_TIMEOUT_SECS = 300s（定义在 openai_provider.rs）
        // 首 token 超时必须 < 整体超时，否则永远不会触发
        assert!(
            FIRST_TOKEN_TIMEOUT_SECS < 300,
            "首 token 超时 ({FIRST_TOKEN_TIMEOUT_SECS}s) 必须小于整体超时 (300s)"
        );
    }

    /// TC-FIRST-TOKEN-003: 首 token 超时应 ≥ 30 秒（容忍慢 API）。
    #[test]
    fn tc_first_token_003_at_least_30_secs() {
        // 30s 下限：DeepSeek/OpenAI 等远程 API 在高负载时首 token 可能 10-20s
        assert!(
            FIRST_TOKEN_TIMEOUT_SECS >= 30,
            "首 token 超时应 ≥ 30s（容忍慢 API），实际: {FIRST_TOKEN_TIMEOUT_SECS}s"
        );
    }
}

// ============================================================================
// REQ-RAG-051: 无 Key 演示模式
// ============================================================================

/// 演示模式预设回答模板（基于关键词匹配）。
///
/// 用户无需 API Key 即可体验基础 RAG 功能。根据查询关键词匹配预设回答。
fn demo_mode_response(query: &str) -> String {
    let q = query.to_lowercase();

    // 关键词匹配规则
    if q.contains("echomind") || q.contains("什么是") || q.contains("介绍") {
        "EchoMind 是一款本地优先的智能知识库助手，采用 RAG（检索增强生成）技术，\
         让你可以在不泄露数据的前提下，与自己的文档对话。\n\n\
         核心特性：\n\
         - 本地嵌入：文档向量化在本地完成，不上传云端\n\
         - BYOK：自带 API Key，使用你自己的 LLM 端点\n\
         - 混合检索：向量 + 关键词双路检索，精度更高\n\
         - Agent 模式：复杂问题自动多步推理\n\n\
         > 当前为演示模式，配置 API Key 后可解锁完整 AI 对话功能。"
            .to_string()
    } else if q.contains("rag") || q.contains("检索增强") || q.contains("原理") {
        "RAG（Retrieval-Augmented Generation，检索增强生成）是一种 AI 技术，\
         它先从知识库中检索相关文档片段，然后将这些片段作为上下文传给 LLM 生成回答。\n\n\
         优势：\n\
         - 减少幻觉：LLM 基于检索到的事实回答，而非凭空生成\n\
         - 知识更新：只需更新知识库，无需重新训练模型\n\
         - 数据隐私：文档留在本地，不进入模型训练\n\n\
         > 当前为演示模式，配置 API Key 后可体验真实 RAG 对话。"
            .to_string()
    } else if q.contains("隐私") || q.contains("安全") || q.contains("数据") {
        "EchoMind 的隐私安全设计：\n\n\
         1. 本地优先：文档解析、分块、向量化全在本地完成\n\
         2. BYOK 模式：使用你自己的 API Key，我们不触碰你的数据\n\
         3. 无追踪：不收集任何用户数据，不嵌入分析 SDK\n\
         4. 开源审计：代码完全开源，可自行审计验证\n\n\
         > 当前为演示模式回答。配置 API Key 后可进行真实对话。"
            .to_string()
    } else {
        format!(
            "你好！你问的是「{query}」。\n\n\
             当前处于演示模式，我只能基于预设模板回答关于 EchoMind、RAG 技术和隐私安全的问题。\n\n\
             配置 API Key 后，我可以基于你的知识库文档进行真实的 RAG 检索和回答。\n\n\
             试试问我：\n\
             - 「什么是 EchoMind？」\n\
             - 「RAG 的原理是什么？」\n\
             - 「隐私安全如何保障？」"
        )
    }
}

/// 演示模式对话处理（REQ-RAG-051 AC-3）。
///
/// 跳过 LLM 调用，使用关键词匹配模板回复。仍执行关键词检索以展示 RAG 检索来源。
async fn handle_demo_mode<R: Runtime>(
    app: &AppHandle<R>,
    query: &str,
    state: &AppState,
    conversation_id: &str,
    turn_group: Option<&str>,
    version: Option<i32>,
) -> Result<(), String> {
    emit_chat_phase(app, "retrieving", "演示模式：关键词检索中…");

    // 关键词检索（不依赖向量化引擎，避免模型下载）
    let results = state
        .storage
        .keyword_search(query, 5)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("关键词检索失败: {e:#}")))?;

    // 推送检索来源
    if !results.is_empty() {
        emit_chat_sources(app, &results);
    }

    emit_chat_phase(app, "generating", "演示模式：生成回答中…");

    // 生成预设回答
    let answer = demo_mode_response(query);

    // 逐字推送（模拟流式输出）
    for ch in answer.chars() {
        emit_chat_token(app, ch.to_string());
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    // 落库
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
        let _ = state.storage.add_message(conversation_id, &user_msg).await;
    }

    let assistant_msg = ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: answer,
        sources: if results.is_empty() {
            None
        } else {
            Some(results.clone())
        },
        reasoning: None,
        turn_group: turn_group.map(|s| s.to_string()),
        version,
    };
    let _ = state
        .storage
        .add_message(conversation_id, &assistant_msg)
        .await;

    emit_chat_done(app, None);
    Ok(())
}

/// 检查是否处于演示模式（REQ-RAG-051 AC-1）。
///
/// 从 settings 表读取 `rag.demo_mode` 键。
#[tauri::command]
pub async fn is_demo_mode(state: State<'_, AppState>) -> Result<bool, String> {
    is_demo_mode_inner(state.inner()).await
}

/// 演示模式查询逻辑（命令与集成测试复用）。
pub async fn is_demo_mode_inner(state: &AppState) -> Result<bool, String> {
    let val = state
        .storage
        .get_setting("rag.demo_mode")
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(val.is_some_and(|v| v == "true"))
}

/// 退出演示模式（REQ-RAG-051 AC-5）。
///
/// 设置 `rag.demo_mode = false` 并清除示例文档。
#[tauri::command]
pub async fn exit_demo_mode(state: State<'_, AppState>) -> Result<(), String> {
    exit_demo_mode_inner(state.inner()).await
}

/// 退出演示模式逻辑（命令与集成测试复用）。
pub async fn exit_demo_mode_inner(state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("rag.demo_mode", "false")
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 清除示例文档（标记为 demo 的文档）
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    for doc in docs {
        if doc.file_path.starts_with("demo://") {
            let _ = state.storage.delete_document(&doc.id).await;
        }
    }

    Ok(())
}

/// 加载示例文档（REQ-RAG-051 AC-2）。
///
/// 将 3 个预设示例文档导入知识库，标记为 `[Demo]` 前缀。
#[tauri::command]
pub async fn load_demo_documents(state: State<'_, AppState>) -> Result<(), String> {
    load_demo_documents_inner(state.inner()).await
}

/// 加载示例文档逻辑（命令与集成测试复用）。
pub async fn load_demo_documents_inner(state: &AppState) -> Result<(), String> {
    let demo_docs = [
        (
            "[Demo] EchoMind 介绍",
            "EchoMind 是一款本地优先的智能知识库助手，采用 RAG（检索增强生成）技术。\n\n\
             核心特性：\n\
             - 本地嵌入：文档向量化在本地完成，不上传云端\n\
             - BYOK：自带 API Key，使用你自己的 LLM 端点\n\
             - 混合检索：向量 + 关键词双路检索，精度更高\n\
             - Agent 模式：复杂问题自动多步推理\n\n\
             EchoMind 让你可以在不泄露数据的前提下，与自己的文档对话。",
        ),
        (
            "[Demo] RAG 技术概述",
            "RAG（Retrieval-Augmented Generation，检索增强生成）是一种 AI 技术。\n\n\
             工作流程：\n\
             1. 文档分块：将长文档切分为语义连贯的小段落\n\
             2. 向量化：用嵌入模型将文本转为向量\n\
             3. 检索：用户提问时，从向量库中检索最相关的片段\n\
             4. 生成：将检索到的片段作为上下文传给 LLM 生成回答\n\n\
             优势：减少幻觉、知识更新快、数据隐私保护。",
        ),
        (
            "[Demo] 隐私安全说明",
            "EchoMind 的隐私安全设计：\n\n\
             1. 本地优先：文档解析、分块、向量化全在本地完成\n\
             2. BYOK 模式：使用你自己的 API Key，我们不触碰你的数据\n\
             3. 无追踪：不收集任何用户数据，不嵌入分析 SDK\n\
             4. 开源审计：代码完全开源，可自行审计验证\n\n\
             你的文档数据始终留在本地，查询时仅将检索到的片段发送到你配置的 LLM 端点。",
        ),
    ];

    for (title, content) in &demo_docs {
        // 使用 Document::new 创建文档（自动生成 UUID + 时间戳）
        let mut doc = echomind_models::Document::new(
            format!("demo://{title}"),
            // 简单 hash（演示模式不需要精确去重）
            format!("demo_{:x}", md5::Md5::digest(content.as_bytes())),
        );
        doc.status = echomind_models::DocStatus::Indexed;
        let _ = state.storage.add_document(&doc).await;

        // 创建单个 chunk（演示模式不做向量嵌入，仅存文本供关键词检索）
        let chunk = echomind_models::Chunk::new(
            doc.id.clone(),
            content.to_string(),
            content.split_whitespace().count(),
            0,
        );
        let _ = state.storage.add_chunk(&chunk).await;
    }

    // 设置演示模式标志
    state
        .storage
        .set_setting("rag.demo_mode", "true")
        .await
        .map_err(|e| format!("{e:#}"))?;

    Ok(())
}
