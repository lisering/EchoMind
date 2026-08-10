//! settings 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 测试 LLM 连接：极简请求验证 API Key 与 URL 有效性。
#[tauri::command]
pub async fn test_llm_connection(
    api_key: String,
    base_url: String,
    model: String,
) -> Result<String, String> {
    let provider = OpenAIProvider::new(api_key, base_url, model).map_err(|e| format!("{e:#}"))?;
    provider
        .test_connection()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 更新 LLM 配置：写入 settings 加密表并刷新运行态（无需重启）。
#[tauri::command]
pub async fn update_llm_config(
    config: LlmConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    update_llm_config_inner(config, state.inner()).await
}

/// 配置写入逻辑（命令与集成测试复用）。
///
/// REQ-ERR-005：输入校验 — API Key 长度上限 256 字符，Base URL 必须以 http:// 或 https:// 开头。
pub async fn update_llm_config_inner(config: LlmConfig, state: &AppState) -> Result<(), String> {
    // REQ-ERR-005：API Key 长度校验
    if config.api_key.chars().count() > MAX_API_KEY_LENGTH {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("API Key 长度不能超过 {} 字符", MAX_API_KEY_LENGTH),
        ));
    }
    // REQ-ERR-005：Base URL 格式校验（非空时必须以 http:// 或 https:// 开头）
    if !config.base_url.is_empty()
        && !config.base_url.starts_with("http://")
        && !config.base_url.starts_with("https://")
    {
        return Err(prefix_error(ERR_VALIDATION, "Base URL 格式不正确"));
    }
    state
        .storage
        .set_setting("llm.api_key", &config.api_key)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.base_url", &config.base_url)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.model", &config.model)
        .await
        .map_err(|e| format!("{e:#}"))?;
    *state.llm_config().write().await = Some(config);
    Ok(())
}

/// 读取配置状态（Key 仅脱敏返回，严禁回传明文，安全官要求）。
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    get_settings_inner(state.inner()).await
}

/// 配置读取逻辑（命令与集成测试复用）。
/// VLM 开关状态从 settings 表读取（REQ-MM-003 前端接线）。
pub async fn get_settings_inner(state: &AppState) -> Result<SettingsPayload, String> {
    let config = state.llm_config().read().await.clone();
    let vlm_enabled = state
        .storage
        .get_setting("mm.vlm_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    let hybrid_search = state
        .storage
        .get_setting("rag.hybrid_search")
        .await
        .map_err(|e| format!("{e:#}"))?
        // 默认启用混合检索（未设置时视为 true）
        .is_none_or(|v| v != "false");
    let rerank_enabled = state
        .storage
        .get_setting("rag.rerank_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        // 默认启用层次化重排（未设置时视为 true）
        // Bug #5 修复：统一使用 is_none_or 风格，与 hybrid_search 等其他布尔设置一致
        .is_none_or(|v| v != "false");
    let hyde_enabled = state
        .storage
        .get_setting("rag.hyde_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        // 默认关闭 HyDE 查询改写（未设置时视为 false）
        .is_some_and(|v| v == "true");
    // REQ-VEC-012：读取当前嵌入模型标识（未设置时默认 all-MiniLM-L6-v2）
    let embedding_model = state
        .storage
        .get_setting("vec.embedding_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "all-MiniLM-L6-v2".to_string());
    // REQ-RAG-022：读取 Agentic RAG 开关（默认关闭）
    let agent_enabled = state
        .storage
        .get_setting("rag.agent_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    // REQ-RAG-017：读取上下文 token 限制（默认 4096）
    let context_token_limit = state
        .storage
        .get_setting("rag.context_token_limit")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4096);
    // REQ-LLM-003：读取 LLM 推理模式（默认 "remote"）
    let llm_mode = state
        .storage
        .get_setting("llm.mode")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    // REQ-LLM-003：读取当前选中的本地模型文件名
    let local_model = state
        .storage
        .get_setting("llm.local_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    // REQ-LLM-003 扩展：读取 PagedAttention 配置（默认关闭）
    let llm_paged_attn = state
        .storage
        .get_setting("llm.paged_attn")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    let llm_block_size = state
        .storage
        .get_setting("llm.block_size")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(32);
    let llm_gpu_memory_ctx = state
        .storage
        .get_setting("llm.gpu_memory_ctx")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(4096);
    // S11：读取采样参数（JSON 序列化存储在 settings 表 llm.sampling 键）
    let llm_sampling = state
        .storage
        .get_setting("llm.sampling")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|s| serde_json::from_str::<LlmSamplingParams>(&s).ok());
    // 读取 token 预算上限（0 = 不限制）
    let token_budget = state
        .storage
        .get_setting("usage.token_budget")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    // 读取缓存设置（REQ-PERF-001）
    let cache_enabled = state
        .storage
        .get_setting("cache.enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .map(|v| v == "true")
        .unwrap_or(true);
    let cache_ttl_secs = state
        .storage
        .get_setting("cache.ttl_secs")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(86400);
    let cache_semantic_threshold = state
        .storage
        .get_setting("cache.semantic_threshold")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.92);
    let cache_privacy_mode = state
        .storage
        .get_setting("cache.privacy_mode")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    let quality_gate_enabled = state
        .storage
        .get_setting("rag.quality_gate_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    let sub_agent_enabled = state
        .storage
        .get_setting("rag.sub_agent_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    let progressive_injection = state
        .storage
        .get_setting("rag.progressive_injection")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    let speculative_enabled = state.speculative_enabled;
    let graph_retriever_enabled = state.graph_retriever_enabled;
    let contextual_retrieval = state.contextual_retrieval_enabled;
    Ok(match config {
        Some(c) => SettingsPayload {
            has_llm_config: true,
            base_url: c.base_url,
            model: c.model,
            api_key_masked: mask_api_key(&c.api_key),
            vlm_enabled,
            hybrid_search,
            rerank_enabled,
            hyde_enabled,
            agent_enabled,
            embedding_model,
            context_token_limit,
            llm_mode,
            local_model,
            llm_paged_attn,
            llm_block_size,
            llm_gpu_memory_ctx,
            llm_sampling,
            token_budget,
            cache_enabled,
            cache_ttl_secs,
            cache_semantic_threshold,
            cache_privacy_mode,
            quality_gate_enabled,
            sub_agent_enabled,
            progressive_injection,
            speculative_enabled,
            graph_retriever_enabled,
            contextual_retrieval,
        },
        None => SettingsPayload {
            has_llm_config: false,
            base_url: String::new(),
            model: String::new(),
            api_key_masked: String::new(),
            vlm_enabled,
            hybrid_search,
            rerank_enabled,
            hyde_enabled,
            agent_enabled,
            embedding_model,
            context_token_limit,
            llm_mode,
            local_model,
            llm_paged_attn,
            llm_block_size,
            llm_gpu_memory_ctx,
            llm_sampling,
            token_budget,
            cache_enabled,
            cache_ttl_secs,
            cache_semantic_threshold,
            cache_privacy_mode,
            quality_gate_enabled,
            sub_agent_enabled,
            progressive_injection,
            speculative_enabled,
            graph_retriever_enabled,
            contextual_retrieval,
        },
    })
}

/// 切换 VLM 图片理解增强开关（REQ-MM-003）。
/// 写入 settings 表 `mm.vlm_enabled`，vision_provider() 下次调用时即时生效。
#[tauri::command]
pub async fn set_vlm_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_vlm_enabled_inner(enabled, state.inner()).await
}

/// VLM 开关写入逻辑（命令与集成测试复用）。
pub async fn set_vlm_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("mm.vlm_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换混合检索开关（REQ-RAG-010）。
/// 写入 settings 表 `rag.hybrid_search`，下次 chat 命令调用时即时生效。
/// 默认启用（未设置时视为 true）：向量 + 关键词 RRF 融合检索。
/// 关闭后退化为纯向量检索（等价于 VectorRetriever）。
#[tauri::command]
pub async fn set_hybrid_search(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_hybrid_search_inner(enabled, state.inner()).await
}

/// 混合检索开关写入逻辑（命令与集成测试复用）。
pub async fn set_hybrid_search_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.hybrid_search", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 设置对话上下文 token 限制（REQ-RAG-017 AC-4）。
/// 写入 settings 表 `rag.context_token_limit`，下次 chat 命令调用时即时生效。
/// 范围 2048-32768，超出的值被拒绝并提示合法范围。
#[tauri::command]
pub async fn set_context_token_limit(
    limit: usize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_context_token_limit_inner(limit, state.inner()).await
}

/// 上下文 token 限制写入逻辑（命令与集成测试复用）。
pub async fn set_context_token_limit_inner(limit: usize, state: &AppState) -> Result<(), String> {
    if !(2048..=32768).contains(&limit) {
        return Err("上下文 token 限制范围为 2048-32768".to_string());
    }
    state
        .storage
        .set_setting("rag.context_token_limit", &limit.to_string())
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换 Cross-Encoder 重排序开关（REQ-RAG-020）。
///
/// 写入 settings 表 `rag.rerank_enabled`，下次 chat 命令调用时即时生效。
/// 启用后，检索管线在 RRF 融合后、Chunk Expansion 前插入 Cross-Encoder 精排阶段，
/// 对 top-N 候选使用 bge-reranker-base 模型逐对打分重排，取 top-k 后再扩展相邻 chunk。
/// 首次启用时触发模型下载（~280MB），后续从本地缓存加载。
///
/// # 错误
/// 返回写入失败的错误描述字符串。
#[tauri::command]
pub async fn set_rerank_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_rerank_enabled_inner(enabled, state.inner()).await
}

/// 重排序开关写入逻辑（命令与集成测试复用）。
pub async fn set_rerank_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.rerank_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换 HyDE 查询改写开关（REQ-RAG-021）。
///
/// 写入 settings 表 `rag.hyde_enabled`，下次 chat 命令调用时即时生效。
/// 启用后，检索管线在向量检索前插入 HyDE 查询改写阶段：使用用户 LLM 配置
/// 生成假设性答案文档，用该文档的嵌入替代原始查询嵌入进行向量检索。
/// 关键词检索仍使用原始查询（精确匹配优势）。
/// HyDE 无额外模型下载——复用用户已配置的 LLM 端点（BYOK）。
///
/// # 隐私边界
///
/// 查询文本仅发送到用户自行配置的 LLM 端点（与 chat 命令相同），
/// 符合「隐私不出域」原则。
///
/// # 错误
/// 返回写入失败的错误描述字符串。
#[tauri::command]
pub async fn set_hyde_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_hyde_enabled_inner(enabled, state.inner()).await
}

/// HyDE 查询改写开关写入逻辑（命令与集成测试复用）。
pub async fn set_hyde_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.hyde_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换嵌入模型（REQ-VEC-012）。
///
/// 将模型标识持久化到 settings 表 `vec.embedding_model`，
/// 并销毁当前 embedder 实例（下次调用 `embedder()` 时用新模型重新初始化）。
///
/// **注意**：切换模型后，已索引文档的向量维度可能与新模型不匹配，
/// 建议用户重建索引以获得最佳检索效果。本命令不自动触发重建。
///
/// # 支持的模型标识
/// - `all-MiniLM-L6-v2`（384 维，英文通用场景，默认）
/// - `bge-small-zh-v1.5`（512 维，中文优化场景）
/// - `e5-small-v2`（384 维，多语言场景）
///
/// # 错误
/// 返回写入失败或模型标识不合法的错误描述字符串。
#[tauri::command]
pub async fn set_embedding_model(model: String, state: State<'_, AppState>) -> Result<(), String> {
    set_embedding_model_inner(&model, state.inner()).await
}

/// 嵌入模型切换逻辑（命令与集成测试复用）。
///
/// 验证模型标识合法性 → 持久化到 settings 表 → 销毁当前 embedder 实例。
/// 返回 `Ok(())` 表示切换成功，下次调用 `embedder()` 时用新模型初始化。
pub async fn set_embedding_model_inner(model: &str, state: &AppState) -> Result<(), String> {
    // 验证模型标识合法性（预设模型或 custom:{name} 格式，REQ-VEC-014）
    let valid = ["all-MiniLM-L6-v2", "bge-small-zh-v1.5", "e5-small-v2"];
    if !valid.contains(&model) && !model.starts_with("custom:") {
        return Err(format!(
            "不支持的嵌入模型: {model}，支持的模型: {valid:?} 或 custom:{{name}}"
        ));
    }
    state
        .set_embedding_model(model)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// API Key 脱敏：仅保留末四位；不足四位整体遮蔽。
pub(crate) fn mask_api_key(api_key: &str) -> String {
    let char_count = api_key.chars().count();
    if char_count <= 4 {
        return "****".to_string();
    }
    let tail: String = api_key.chars().skip(char_count - 4).collect();
    format!("****{tail}")
}

/// 查询用户界面语言偏好（REQ-I18N-001）。
/// 从 settings 表读取 `ui.locale`，未设置时返回空字符串（前端自动检测系统语言）。
#[tauri::command]
pub async fn get_locale(state: State<'_, AppState>) -> Result<String, String> {
    get_locale_inner(state.inner()).await
}

/// 语言偏好查询逻辑（命令与集成测试复用）。
pub async fn get_locale_inner(state: &AppState) -> Result<String, String> {
    state
        .storage
        .get_setting("ui.locale")
        .await
        .map_err(|e| format!("{e:#}"))
        .map(|v| v.unwrap_or_default())
}

/// 设置用户界面语言偏好（REQ-I18N-001）。
/// 持久化到 settings 表 `ui.locale`，下次启动时通过 get_locale 恢复。
#[tauri::command]
pub async fn set_locale(locale: String, state: State<'_, AppState>) -> Result<(), String> {
    set_locale_inner(&locale, state.inner()).await
}

/// 语言偏好设置逻辑（命令与集成测试复用）。
pub async fn set_locale_inner(locale: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("ui.locale", locale)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 查询用户界面主题偏好（REQ-UI-011）。
/// 从 settings 表读取 `ui.theme`，未设置时返回 "dark"（默认暗色主题）。
#[tauri::command]
pub async fn get_theme(state: State<'_, AppState>) -> Result<String, String> {
    get_theme_inner(state.inner()).await
}

/// 主题偏好查询逻辑（命令与集成测试复用）。
pub async fn get_theme_inner(state: &AppState) -> Result<String, String> {
    state
        .storage
        .get_setting("ui.theme")
        .await
        .map_err(|e| format!("{e:#}"))
        .map(|v| v.unwrap_or_else(|| "dark".to_string()))
}

/// 设置用户界面主题偏好（REQ-UI-011）。
/// 持久化到 settings 表 `ui.theme`，下次启动时通过 get_theme 恢复。
/// 支持值："dark" / "light" / "system"。
#[tauri::command]
pub async fn set_theme(theme: String, state: State<'_, AppState>) -> Result<(), String> {
    set_theme_inner(&theme, state.inner()).await
}

/// 主题偏好设置逻辑（命令与集成测试复用）。
pub async fn set_theme_inner(theme: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("ui.theme", theme)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 查询侧栏折叠状态（REQ-NAV-001 AC-4）。
/// 从 settings 表读取 `ui.sidebar_collapsed`，未设置时返回 false（默认展开）。
#[tauri::command]
pub async fn get_sidebar_collapsed(state: State<'_, AppState>) -> Result<bool, String> {
    get_sidebar_collapsed_inner(state.inner()).await
}

/// 侧栏折叠状态查询逻辑（命令与集成测试复用）。
pub async fn get_sidebar_collapsed_inner(state: &AppState) -> Result<bool, String> {
    let val = state
        .storage
        .get_setting("ui.sidebar_collapsed")
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(val.as_deref() == Some("true"))
}

/// 持久化侧栏折叠状态（REQ-NAV-001 AC-4）。
/// 写入 settings 表 `ui.sidebar_collapsed`，下次启动时通过 get_sidebar_collapsed 恢复。
#[tauri::command]
pub async fn set_sidebar_collapsed(
    collapsed: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_sidebar_collapsed_inner(collapsed, state.inner()).await
}

/// 侧栏折叠状态持久化逻辑（命令与集成测试复用）。
pub async fn set_sidebar_collapsed_inner(collapsed: bool, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting(
            "ui.sidebar_collapsed",
            if collapsed { "true" } else { "false" },
        )
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================================
// S69: Token 预算配置（Cherry Studio 借鉴 — token-budget 驱动 in-loop compaction）
// ============================================================================

/// 获取 Token 预算配置。
#[tauri::command]
pub async fn get_token_budget_config(
    state: State<'_, AppState>,
) -> Result<echomind_models::TokenBudgetConfig, String> {
    get_token_budget_config_inner(state.inner()).await
}

/// Token 预算配置读取逻辑（命令与集成测试复用）。
pub async fn get_token_budget_config_inner(
    state: &AppState,
) -> Result<echomind_models::TokenBudgetConfig, String> {
    let max_tokens = state
        .storage
        .get_setting("compaction.max_tokens")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or_else(echomind_models::default_context_token_limit);

    let threshold = state
        .storage
        .get_setting("compaction.threshold")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.8);

    let keep_ratio = state
        .storage
        .get_setting("compaction.recent_keep_ratio")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(0.67);

    let min_msgs = state
        .storage
        .get_setting("compaction.min_messages")
        .await
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(3);

    Ok(echomind_models::TokenBudgetConfig {
        max_tokens,
        compaction_threshold: threshold,
        recent_keep_ratio: keep_ratio,
        min_messages_to_compact: min_msgs,
    })
}

/// 设置 Token 预算配置。
#[tauri::command]
pub async fn set_token_budget_config(
    config: echomind_models::TokenBudgetConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_token_budget_config_inner(config, state.inner()).await
}

/// Token 预算配置写入逻辑（命令与集成测试复用）。
pub async fn set_token_budget_config_inner(
    config: echomind_models::TokenBudgetConfig,
    state: &AppState,
) -> Result<(), String> {
    state
        .storage
        .set_setting("compaction.max_tokens", &config.max_tokens.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "compaction.threshold",
            &config.compaction_threshold.to_string(),
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "compaction.recent_keep_ratio",
            &config.recent_keep_ratio.to_string(),
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "compaction.min_messages",
            &config.min_messages_to_compact.to_string(),
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

// ============================================================================
// Q08: Budget Tracking (QM 借鉴 — LLM API 费用控制和速率限制)
// ============================================================================

/// 获取预算统计（QM 借鉴）。
#[tauri::command]
pub async fn get_budget_stats(
    state: State<'_, AppState>,
) -> Result<echomind_models::BudgetStats, String> {
    get_budget_stats_inner(state.inner()).await
}

/// 预算统计读取逻辑（命令与集成测试复用）。
pub async fn get_budget_stats_inner(
    state: &AppState,
) -> Result<echomind_models::BudgetStats, String> {
    let principal = "default_user"; // 当前使用固定主体，后续可扩展用户系统
    state
        .storage
        .get_budget_stats(principal)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 设置预算限制（QM 借鉴）。
#[tauri::command]
pub async fn set_budget_limit(
    daily_limit_usd: f64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_budget_limit_inner(daily_limit_usd, state.inner()).await
}

/// 预算限制设置逻辑（命令与集成测试复用）。
pub async fn set_budget_limit_inner(daily_limit_usd: f64, state: &AppState) -> Result<(), String> {
    let principal = "default_user"; // 当前使用固定主体，后续可扩展用户系统
    state
        .storage
        .set_budget_limit(principal, daily_limit_usd)
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================================
// S4 v1.6: 窗口关闭行为 — 最小化到托盘设置（REQ-WIN-003）
// ============================================================================

/// 获取「关闭窗口时最小化到托盘」设置（REQ-WIN-003 v1.6）。
///
/// 默认关闭（false）。开启后，点击窗口关闭按钮将隐藏窗口而非退出应用。
#[tauri::command]
pub async fn get_close_to_tray(state: State<'_, AppState>) -> Result<bool, String> {
    get_close_to_tray_inner(state.inner()).await
}

/// 关闭到托盘设置读取逻辑（命令与集成测试复用）。
pub async fn get_close_to_tray_inner(state: &AppState) -> Result<bool, String> {
    let val = state
        .storage
        .get_setting("window.close_to_tray")
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(val.is_some_and(|v| v == "true"))
}

/// 设置「关闭窗口时最小化到托盘」（REQ-WIN-003 v1.6）。
///
/// 持久化到 settings 表 `window.close_to_tray` 键。
#[tauri::command]
pub async fn set_close_to_tray(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_close_to_tray_inner(enabled, state.inner()).await
}

/// 关闭到托盘设置写入逻辑（命令与集成测试复用）。
pub async fn set_close_to_tray_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting(
            "window.close_to_tray",
            if enabled { "true" } else { "false" },
        )
        .await
        .map_err(|e| format!("{e:#}"))
}
