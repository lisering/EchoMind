#!/usr/bin/env python3
"""Fix commands/chat.rs: Remove speculative RAG, cache, progressive, retrieval_memory, compression"""

filepath = 'crates/tauri-app/src/commands/chat.rs'
with open(filepath, 'r') as f:
    content = f.read()

# 1. Remove settings_keys for deleted features
content = content.replace(
    '            "rag.progressive_injection",\n'
    '            "rag.speculative_enabled",\n'
    '            "rag.retrieval_memory_enabled",\n',
    ''
)
content = content.replace(
    '            "cache.enabled",\n'
    '            "cache.ttl_secs",\n'
    '            "cache.semantic_threshold",\n',
    ''
)

# 2. Remove cache_enabled, cache_ttl_secs, semantic_threshold, cache_now declarations
# These are in the batch settings reading section
# Remove the cache lookup block
old_cache_lookup = '''        let cache_enabled = settings_map
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
        let semantic_threshold = settings_map
            .get("cache.semantic_threshold")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or_else(|| state.cache.current_semantic_threshold());'''
content = content.replace(old_cache_lookup, '')

# 3. Remove cache lookup code (L0 exact + L1 semantic + cached answer streaming)
# Find and remove the cache hit blocks
# L0 exact lookup
content = content.replace(
    '''        // REQ-PERF-001 L0: 精确匹配缓存查找
        if cache_enabled && !embedding_degraded {
            if let Some(hit) = state
                .cache
                .lookup_exact(&query_hash(query), cache_ttl_secs, cache_now)
                .await
                .map_err(|e| classify_llm_error(&format!("{e:#}")))?
            {
                debug!("chat_inner L0 cache hit (exact)");
                emit_chat_sources(app, &hit.sources);
                emit_chat_phase(app, "generating", "从缓存加载回答…");
                for chunk in split_cached_answer(&hit.answer, 40) {
                    let _ = app.emit("chat_token", chunk);
                }
                emit_chat_done(app, None);
                return Ok(());
            }
        }

        // REQ-PERF-001 L1: 语义匹配缓存查找
        if cache_enabled && !embedding_degraded {
            if let Some(hit) = state
                .cache
                .lookup_semantic(&emb, semantic_threshold, cache_ttl_secs, cache_now)
                .await
                .map_err(|e| classify_llm_error(&format!("{e:#}")))?
            {
                debug!("chat_inner L1 cache hit (semantic)");
                emit_chat_sources(app, &hit.sources);
                emit_chat_phase(app, "generating", "从缓存加载回答…");
                for chunk in split_cached_answer(&hit.answer, 40) {
                    let _ = app.emit("chat_token", chunk);
                }
                emit_chat_done(app, None);
                return Ok(());
            }
        }''',
    ''
)

# 4. Remove retrieval_cache_hit (L3)
content = content.replace(
    '''        // REQ-PERF-001 L3: 检索结果缓存
        let retrieval_cache_hit = if cache_enabled {
            match state
                .cache
                .lookup_retrieval(emb, 0.90, cache_ttl_secs, cache_now)
                .await
            {
                Ok(Some(cached_sources)) => {
                    debug!("chat_inner L3 cache hit (retrieval)");
                    Some(cached_sources)
                }
                Ok(None) => None,
                Err(e) => {
                    warn!("L3 retrieval cache lookup failed: {e:#}");
                    None
                }
            }
        } else {
            None
        };''',
    ''
)

# Now we need to find where retrieval_cache_hit is used and remove those branches
# The code uses `if let Some(cached_sources) = retrieval_cache_hit` pattern

# 5. Remove use_parallel_retrieval (references speculative)
content = content.replace(
    'let use_parallel_retrieval = !coordinator_enabled && !agent_enabled && !speculative_enabled;',
    'let use_parallel_retrieval = !coordinator_enabled && !agent_enabled;'
)

# 6. Remove speculative_enabled reading
content = content.replace(
    '''        let speculative_enabled = settings_map
            .get("rag.speculative_enabled")
            .is_some_and(|&v| v == "true");''',
    ''
)

# 7. Remove retrieval_cache_hit usage in retrieval path
content = content.replace(
    '''        let (cr, cached_sources) = if let Some(json) = retrieval_cache_hit {
            // L3 缓存命中：反序列化并跳过检索
            match serde_json::from_str::<Vec<RetrievalResult>>(&json) {
                Ok(sources) => {
                    debug!("chat_inner L3 cache hit: skipping retrieval");
                    (sources, Some(json))
                }
                Err(e) => {
                    warn!("L3 cache deserialization failed, falling back to retrieval: {e:#}");
                    let cr = retriever
                        .retrieve_with_embedding(query, &emb, DEFAULT_TOP_K)
                        .await
                        .map_err(|e| classify_llm_error(&format!("{e:#}")))?;
                    (cr, None)
                }
            }
        } else {
            let cr = retriever
                .retrieve_with_embedding(query, &emb, DEFAULT_TOP_K)
                .await
                .map_err(|e| classify_llm_error(&format!("{e:#}")))?;
            (cr, None)
        };''',
    '''        let cr = retriever
            .retrieve_with_embedding(query, &emb, DEFAULT_TOP_K)
            .await
            .map_err(|e| classify_llm_error(&format!("{e:#}")))?;'''
)

# Remove cached_sources reference in graph expansion check
content = content.replace(
    '''        // 图扩展仅在标准 RAG 路径（非 coordinator/agent/speculative）且启用时执行。
        if !coordinator_enabled && !agent_enabled && !speculative_enabled {''',
    '''        // 图扩展仅在标准 RAG 路径（非 coordinator/agent）且启用时执行。
        if !coordinator_enabled && !agent_enabled {'''
)

# 8. Remove L3 cache insert after retrieval
content = content.replace(
    '''        // REQ-PERF-001 L3: 写入检索结果缓存
        if cache_enabled
            && cached_sources.is_none()
            && let Err(e) = state.cache.insert_retrieval(query, emb, &json).await
        {
            warn!("写入 L3 检索缓存失败: {e:#}");
        }''',
    ''
)

# 9. Remove CoordinatorEngine::with_step_cache -> just use step_cache
content = content.replace(
    'let coordinator = if cache_enabled {\n            CoordinatorEngine::with_step_cache(retriever, provider, state.step_cache.clone())\n        } else {\n            CoordinatorEngine::new(retriever, provider)\n        };',
    'let coordinator = CoordinatorEngine::with_step_cache(retriever, provider, state.step_cache.clone());'
)

# 10. Remove write_query_cache calls
content = content.replace(
    '            write_query_cache(state, query, &result.content, &sources, conversation_id).await;\n',
    ''
)

# 11. Remove AgentEngine::with_step_cache -> just use step_cache
content = content.replace(
    'let agent = if cache_enabled {\n            AgentEngine::with_step_cache(retriever, provider, state.step_cache.clone())\n        } else {\n            AgentEngine::new(retriever, provider)\n        };',
    'let agent = AgentEngine::with_step_cache(retriever, provider, state.step_cache.clone());'
)

# 12. Remove the entire speculative RAG branch and replace with standard RAG only
# The speculative branch starts with "// REQ-PERF-011" check
# We need to find the if/else block that branches on speculative_enabled

# Find the speculative branch and the standard RAG branch
# Pattern: "let (sources, stream) = if speculative_enabled { ... } else { ... };"
# We want to keep only the else branch (standard RAG)

# This is the most complex replacement - let's find the exact pattern
spec_start = content.find('        // REQ-PERF-011：检查 Speculative RAG 是否启用')
if spec_start is not None:
    # Find the matching else
    # The pattern is: if speculative_enabled { ... } else { ... }
    # We need to find the else that matches this if
    # Let's find the standard RAG branch start
    standard_start = content.find('        } else {\n            // ---- 标准 RAG 流程 ----')
    if standard_start is not None:
        # Find the end of the standard RAG branch
        # It ends with the closing "        };"
        standard_end = content.find('        };\n', standard_start + 10)
        if standard_end is not None:
            # Keep only the standard RAG branch content (without the else wrapper)
            standard_content = content[standard_start + len('        } else {\n'):]
            # Find the end (closing "        };")
            end_idx = standard_content.find('\n        };\n')
            if end_idx is not None:
                standard_rag = standard_content[:end_idx]
                # Replace the entire if/else block
                content = content[:spec_start] + standard_rag + content[standard_end + len('\n        };\n'):]

# 13. Remove retrieval_memory recording block
old_retrieval_memory = '''        // REQ-PERF-012：自进化检索记忆 — 记录检索效果
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
        }'''
content = content.replace(old_retrieval_memory, '')

# 14. Remove progressive_info from ChatOutcome matching
content = content.replace(
    '''                ChatOutcome::Answered {
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
                    emit_chat_phase(app, "generating", "正在生成回答…");''',
    '''                ChatOutcome::Answered {
                    sources,
                    stream,
                    ..
                } => {
                    emit_chat_sources(app, &sources);
                    // 阶段 3：LLM 生成回答（首个 token 到达前的连接与推理延迟）
                    emit_chat_phase(app, "generating", "正在生成回答…");'''
)

# 15. Remove compression_ratio and progressive injection from standard RAG branch
content = content.replace(
    '''            // REQ-PERF-002：集成 Prompt 压缩（仅压缩检索片段，不压缩系统提示和用户查询）
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

            ''',
    '            '
)

# 16. Remove cache write after persist_exchange
content = content.replace(
    '''        // REQ-PERF-001：正常回答写入查询缓存（下次相同/相似问题秒回）
        // Bug #4 修复：仅在有检索来源时写缓存，避免 NoContext 固定拒答文案被缓存
        // （此前 sources=None 时仍将"知识库中未找到相关内容"写入缓存，
        //   用户导入文档后仍返回旧缓存答案，直到 TTL 过期）
        if sources.is_some() {
            write_query_cache(state, query, &result.content, &sources, conversation_id).await;
        }''',
    ''
)

# 17. Fix comment about speculative in parallel retrieval
content = content.replace(
    '// 与知识库检索互不依赖 → 并行执行，节省串行等待；coordinator/agent/speculative',
    '// 与知识库检索互不依赖 → 并行执行，节省串行等待；coordinator/agent'
)

with open(filepath, 'w') as f:
    f.write(content)

print("commands/chat.rs fixed")
