//! settings 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
//!
//! S09: `update_setting(key, value)` 统一设置命令 — 合并 ~20 个 set_xxx 命令为单一入口。
//! 旧 set_xxx 命令标记 `#[deprecated]`，已从 `generate_handler!` 移除，但保留 `*_inner` 逻辑供测试复用。
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
// Q08: Budget Tracking (QM 借鉴) — S10: 开发者工具门控
// ============================================================================

/// 获取预算统计（QM 借鉴）。
#[cfg(debug_assertions)]
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
#[cfg(debug_assertions)]
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

// ─── RAG 检索参数（REQ-RAG-014）────────────────────────────────────────────

/// 获取 RAG 检索参数（REQ-RAG-014）。
///
/// 从 settings 表读取 `rag.top_k` / `rag.score_threshold` /
/// `rag.chunk_expansion_enabled` / `rag.chunk_expansion_window`，返回 `RagParams`。
#[tauri::command]
pub async fn get_rag_params(state: State<'_, AppState>) -> Result<RagParams, String> {
    get_rag_params_inner(state.inner()).await
}

/// RAG 参数读取逻辑（命令与集成测试复用）。
pub async fn get_rag_params_inner(state: &AppState) -> Result<RagParams, String> {
    let top_k = state
        .storage
        .get_setting("rag.top_k")
        .await
        .map_err(|e| format!("{e:#}"))?;
    let score_threshold = state
        .storage
        .get_setting("rag.score_threshold")
        .await
        .map_err(|e| format!("{e:#}"))?;
    let chunk_expansion_enabled = state
        .storage
        .get_setting("rag.chunk_expansion_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?;
    let chunk_expansion_window = state
        .storage
        .get_setting("rag.chunk_expansion_window")
        .await
        .map_err(|e| format!("{e:#}"))?;

    let mut params = RagParams::default();
    if let Some(v) = top_k
        && let Ok(n) = v.parse::<usize>()
    {
        params.top_k = n;
    }
    if let Some(v) = score_threshold
        && let Ok(f) = v.parse::<f32>()
    {
        params.score_threshold = f;
    }
    if let Some(v) = chunk_expansion_enabled {
        params.chunk_expansion_enabled = v == "true";
    }
    if let Some(v) = chunk_expansion_window
        && let Ok(n) = v.parse::<usize>()
    {
        params.chunk_expansion_window = n;
    }
    Ok(params.clamped())
}

/// 设置 RAG 检索参数（REQ-RAG-014）。
///
/// 持久化到 settings 表，对新查询生效。
#[tauri::command]
pub async fn set_rag_params(params: RagParams, state: State<'_, AppState>) -> Result<(), String> {
    set_rag_params_inner(params, state.inner()).await
}

/// RAG 参数写入逻辑（命令与集成测试复用）。
pub async fn set_rag_params_inner(params: RagParams, state: &AppState) -> Result<(), String> {
    let params = params.clamped();
    state
        .storage
        .set_setting("rag.top_k", &params.top_k.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("rag.score_threshold", &params.score_threshold.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "rag.chunk_expansion_enabled",
            if params.chunk_expansion_enabled {
                "true"
            } else {
                "false"
            },
        )
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting(
            "rag.chunk_expansion_window",
            &params.chunk_expansion_window.to_string(),
        )
        .await
        .map_err(|e| format!("{e:#}"))
}

// ─── LLM 生成参数（REQ-RAG-015）────────────────────────────────────────────

/// 获取 LLM 生成参数（REQ-RAG-015）。
///
/// 从 settings 表读取 `llm.temperature` / `llm.max_tokens` / `llm.top_p`。
#[tauri::command]
pub async fn get_generation_params(state: State<'_, AppState>) -> Result<GenerationParams, String> {
    get_generation_params_inner(state.inner()).await
}

/// 生成参数读取逻辑（命令与集成测试复用）。
pub async fn get_generation_params_inner(state: &AppState) -> Result<GenerationParams, String> {
    let temperature = state
        .storage
        .get_setting("llm.temperature")
        .await
        .map_err(|e| format!("{e:#}"))?;
    let max_tokens = state
        .storage
        .get_setting("llm.max_tokens")
        .await
        .map_err(|e| format!("{e:#}"))?;
    let top_p = state
        .storage
        .get_setting("llm.top_p")
        .await
        .map_err(|e| format!("{e:#}"))?;

    let mut params = GenerationParams::default();
    if let Some(v) = temperature
        && let Ok(f) = v.parse::<f32>()
    {
        params.temperature = f;
    }
    if let Some(v) = max_tokens
        && let Ok(n) = v.parse::<usize>()
    {
        params.max_tokens = n;
    }
    if let Some(v) = top_p
        && let Ok(f) = v.parse::<f32>()
    {
        params.top_p = f;
    }
    Ok(params.clamped())
}

/// 设置 LLM 生成参数（REQ-RAG-015）。
///
/// 持久化到 settings 表，对新查询生效。
#[tauri::command]
pub async fn set_generation_params(
    params: GenerationParams,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_generation_params_inner(params, state.inner()).await
}

/// 生成参数写入逻辑（命令与集成测试复用）。
pub async fn set_generation_params_inner(
    params: GenerationParams,
    state: &AppState,
) -> Result<(), String> {
    let params = params.clamped();
    state
        .storage
        .set_setting("llm.temperature", &params.temperature.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.max_tokens", &params.max_tokens.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.top_p", &params.top_p.to_string())
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================
// REQ-VEC-011 分块参数可视化配置
// ============================================================

/// 分块参数（REQ-VEC-011）。
///
/// 持久化到 settings 表 `vec.chunk_size` 和 `vec.chunk_overlap`。
/// 新导入文档使用此参数分块；已索引文档不受影响。
/// 范围：chunk_size 128-2048（默认 256），overlap 0-256（默认 32）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkParams {
    /// 分块 token 窗口（128-2048，默认 256）
    pub chunk_size: usize,
    /// 重叠 token 数（0-256，默认 32，必须 < chunk_size）
    pub overlap: usize,
}

impl Default for ChunkParams {
    fn default() -> Self {
        Self {
            chunk_size: echomind_core::import::DEFAULT_CHUNK_TOKENS,
            overlap: echomind_core::splitter::DEFAULT_OVERLAP_TOKENS,
        }
    }
}

impl ChunkParams {
    /// 校验参数范围，超出范围时返回错误。
    pub fn validate(&self) -> Result<(), String> {
        if self.chunk_size < 128 || self.chunk_size > 2048 {
            return Err(format!(
                "VALIDATION: 分块大小必须在 128-2048 范围内（当前 {}）",
                self.chunk_size
            ));
        }
        if self.overlap > 256 {
            return Err(format!(
                "VALIDATION: 重叠大小必须在 0-256 范围内（当前 {}）",
                self.overlap
            ));
        }
        if self.overlap >= self.chunk_size {
            return Err(format!(
                "VALIDATION: 重叠大小必须小于分块大小（overlap={} >= chunk_size={}）",
                self.overlap, self.chunk_size
            ));
        }
        Ok(())
    }
}

/// 获取分块参数（REQ-VEC-011）。
#[tauri::command]
pub async fn get_chunk_params(state: State<'_, AppState>) -> Result<ChunkParams, String> {
    get_chunk_params_inner(state.inner()).await
}

/// 获取分块参数逻辑（命令与集成测试复用）。
pub async fn get_chunk_params_inner(state: &AppState) -> Result<ChunkParams, String> {
    let chunk_size = state
        .storage
        .get_setting("vec.chunk_size")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(echomind_core::import::DEFAULT_CHUNK_TOKENS);
    let overlap = state
        .storage
        .get_setting("vec.chunk_overlap")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(echomind_core::splitter::DEFAULT_OVERLAP_TOKENS);
    Ok(ChunkParams {
        chunk_size,
        overlap,
    })
}

/// 设置分块参数（REQ-VEC-011）。
///
/// 参数变更后新导入文档使用新参数；已索引文档不受影响（需手动重建索引）。
#[tauri::command]
pub async fn set_chunk_params(
    params: ChunkParams,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_chunk_params_inner(params, state.inner()).await
}

/// 设置分块参数逻辑（命令与集成测试复用）。
pub async fn set_chunk_params_inner(params: ChunkParams, state: &AppState) -> Result<(), String> {
    params.validate()?;
    state
        .storage
        .set_setting("vec.chunk_size", &params.chunk_size.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("vec.chunk_overlap", &params.overlap.to_string())
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================================
// v1.13: 开机自启（REQ-WIN-004）
// ============================================================================

/// 获取开机自启状态。
#[tauri::command]
pub async fn get_autostart(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    get_autostart_inner(app, state.inner()).await
}

/// 获取开机自启状态逻辑（命令与集成测试复用）。
pub async fn get_autostart_inner(app: tauri::AppHandle, state: &AppState) -> Result<bool, String> {
    // 优先从 settings 表读取持久化状态
    let persisted = state
        .storage
        .get_setting("app.autostart")
        .await
        .map_err(|e| format!("{e:#}"))?;
    if let Some(val) = persisted {
        return Ok(val == "true");
    }
    // 回退到 autostart 插件实际状态
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    match autostart.is_enabled() {
        Ok(enabled) => Ok(enabled),
        Err(_) => Ok(false),
    }
}

/// 设置开机自启。
#[tauri::command]
pub async fn set_autostart(
    enabled: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_autostart_inner(enabled, app, state.inner()).await
}

/// 设置开机自启逻辑（命令与集成测试复用）。
pub async fn set_autostart_inner(
    enabled: bool,
    app: tauri::AppHandle,
    state: &AppState,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enabled {
        autostart
            .enable()
            .map_err(|e| format!("AUTOSTART: 启用开机自启失败: {e}"))?;
    } else {
        autostart
            .disable()
            .map_err(|e| format!("AUTOSTART: 禁用开机自启失败: {e}"))?;
    }
    // 持久化到 settings 表
    state
        .storage
        .set_setting("app.autostart", if enabled { "true" } else { "false" })
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================================
// v1.13: 应用更新检查（REQ-HELP-004）
// ============================================================================

/// 检查 GitHub Releases 是否有新版本。
#[tauri::command]
pub async fn check_for_updates(
    state: State<'_, AppState>,
) -> Result<echomind_core::update_check::UpdateCheckResult, String> {
    check_for_updates_inner(state.inner()).await
}

/// 检查更新逻辑（命令与集成测试复用）。
///
/// 24 小时内不重复检查。网络不可用时返回 has_update=false，不报错。
///
/// 铁律一合规：reqwest 使用 `.no_proxy()` 确保直连。
pub async fn check_for_updates_inner(
    state: &AppState,
) -> Result<echomind_core::update_check::UpdateCheckResult, String> {
    use echomind_core::update_check;

    let current_version = env!("CARGO_PKG_VERSION");

    // 检查 24 小时内是否已检查过
    let last_check = state
        .storage
        .get_setting("update.last_check")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    // 如果 auto_check 为 false 且 24 小时内已检查，跳过
    let auto_check = state
        .storage
        .get_setting("update.auto_check")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_none_or(|v| v != "false");

    if !auto_check && !update_check::should_check(last_check) {
        return Ok(update_check::UpdateCheckResult {
            has_update: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_notes: None,
            download_url: None,
        });
    }

    // 调用 GitHub Releases API（铁律一：no_proxy 直连）
    let result = fetch_github_release(current_version).await;

    // 更新最后检查时间
    let now = update_check::current_timestamp();
    let _ = state
        .storage
        .set_setting("update.last_check", &now.to_string())
        .await;

    match result {
        Some(r) => Ok(r),
        None => Ok(update_check::UpdateCheckResult {
            has_update: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_notes: None,
            download_url: None,
        }),
    }
}

/// GitHub Release 信息（反序列化 GitHub API 响应）。
#[derive(Debug, serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: Option<String>,
    html_url: Option<String>,
}

/// GitHub API 请求超时（秒）。
const GITHUB_API_TIMEOUT_SECS: u64 = 15;

/// GitHub 仓库 releases API URL。
const GITHUB_RELEASES_API: &str = "https://api.github.com/repos/EchoMind/EchoMind/releases/latest";

/// 调用 GitHub Releases API 检查新版本。
///
/// 网络不可用时返回 `None`，不报错。
async fn fetch_github_release(
    current_version: &str,
) -> Option<echomind_core::update_check::UpdateCheckResult> {
    use echomind_core::update_check;

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(GITHUB_API_TIMEOUT_SECS))
        .build()
        .ok()?;

    let response = client
        .get(GITHUB_RELEASES_API)
        .header("User-Agent", "EchoMind-Update-Checker")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let release: GitHubRelease = response.json().await.ok()?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let has_update = update_check::is_newer_version(current_version, &latest_version);

    Some(update_check::UpdateCheckResult {
        has_update,
        current_version: current_version.to_string(),
        latest_version,
        release_notes: release.body,
        download_url: release.html_url,
    })
}

/// 获取更新检查配置。
#[tauri::command]
pub async fn get_update_check_config(
    state: State<'_, AppState>,
) -> Result<echomind_core::update_check::UpdateCheckConfig, String> {
    get_update_check_config_inner(state.inner()).await
}

/// 获取更新检查配置逻辑（命令与集成测试复用）。
pub async fn get_update_check_config_inner(
    state: &AppState,
) -> Result<echomind_core::update_check::UpdateCheckConfig, String> {
    let auto_check = state
        .storage
        .get_setting("update.auto_check")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_none_or(|v| v != "false");

    let last_check = state
        .storage
        .get_setting("update.last_check")
        .await
        .map_err(|e| format!("{e:#}"))?
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    Ok(echomind_core::update_check::UpdateCheckConfig {
        auto_check,
        last_check,
    })
}

/// 设置自动检查更新开关。
#[tauri::command]
pub async fn set_update_check_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_update_check_enabled_inner(enabled, state.inner()).await
}

/// 设置自动检查更新开关逻辑（命令与集成测试复用）。
pub async fn set_update_check_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("update.auto_check", if enabled { "true" } else { "false" })
        .await
        .map_err(|e| format!("{e:#}"))
}

// ------------------------------------------------------------------
// 导入历史记录（REQ-ING-011）
// ------------------------------------------------------------------

/// 查询导入历史记录（最近 100 条，按时间倒序）。
#[tauri::command]
pub async fn get_import_history(
    result_filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::ImportLogEntry>, String> {
    state
        .storage
        .get_import_logs(result_filter.as_deref())
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 清空导入历史记录。
#[tauri::command]
pub async fn clear_import_history(state: State<'_, AppState>) -> Result<(), String> {
    state
        .storage
        .clear_import_logs()
        .await
        .map_err(|e| format!("{e:#}"))
}

// ------------------------------------------------------------------
// 智能模式（S5 审计 P0-1）
// ------------------------------------------------------------------

/// 智能模式开启时自动设置的优化项。
const SMART_MODE_SETTINGS: &[(&str, &str)] = &[
    ("rag.hybrid_search", "true"),
    ("rag.rerank_enabled", "true"),
    ("rag.hyde_enabled", "true"),
    ("rag.graph_retriever_enabled", "true"),
    ("rag.quality_gate_enabled", "true"),
    ("compression.ratio", "2.0"),
    ("rag.contextual_retrieval", "true"),
];

/// 设置智能模式开关。
///
/// 开启时：备份当前设置值到 `smart_mode_backup.*` 键，然后设置全部优化项。
/// 关闭时：从 `smart_mode_backup.*` 键恢复用户手动设置值。
#[tauri::command]
pub async fn set_smart_mode(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_smart_mode_inner(enabled, state.inner()).await
}

/// `set_smart_mode` 的逻辑实现（命令与集成测试复用）。
pub async fn set_smart_mode_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    if enabled {
        // 备份当前值
        for &(key, _) in SMART_MODE_SETTINGS {
            let current = state
                .storage
                .get_setting(key)
                .await
                .map_err(|e| format!("{e:#}"))?
                .unwrap_or_else(|| "false".to_string());
            let backup_key = format!("smart_mode_backup.{key}");
            state
                .storage
                .set_setting(&backup_key, &current)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
        // 设置优化值
        for &(key, val) in SMART_MODE_SETTINGS {
            state
                .storage
                .set_setting(key, val)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
    } else {
        // 恢复备份值
        for &(key, _) in SMART_MODE_SETTINGS {
            let backup_key = format!("smart_mode_backup.{key}");
            let backup_val = state
                .storage
                .get_setting(&backup_key)
                .await
                .map_err(|e| format!("{e:#}"))?;
            if let Some(val) = backup_val {
                state
                    .storage
                    .set_setting(key, &val)
                    .await
                    .map_err(|e| format!("{e:#}"))?;
            }
        }
    }

    state
        .storage
        .set_setting("smart_mode.enabled", if enabled { "true" } else { "false" })
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 查询智能模式是否启用。
#[tauri::command]
pub async fn get_smart_mode(state: State<'_, AppState>) -> Result<bool, String> {
    get_smart_mode_inner(state.inner()).await
}

/// `get_smart_mode` 的逻辑实现（命令与集成测试复用）。
pub async fn get_smart_mode_inner(state: &AppState) -> Result<bool, String> {
    // 默认启用（首次安装时无设置项，返回 true）
    let val = state
        .storage
        .get_setting("smart_mode.enabled")
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(val.is_none_or(|v| v != "false"))
}

// ============================================================================
// S09: 统一设置命令 — update_setting(key, value) + get_setting(key)
// ============================================================================

/// 支持通过 `update_setting` 更新的设置键白名单。
///
/// 每个键映射到一个 `*_inner` 函数，实现类型解析 + 验证 + 持久化。
/// 不在此白名单中的键返回 `ERR_VALIDATION` 错误（防止任意键注入）。
const UPDATEABLE_KEYS: &[&str] = &[
    "rag.hybrid_search",
    "rag.rerank_enabled",
    "rag.hyde_enabled",
    "rag.agent_enabled",
    "rag.coordinator_enabled",
    "rag.sub_agent_enabled",
    "vec.embedding_model",
    "compression.ratio",
    "rag.context_token_limit",
    "mm.vlm_enabled",
    "rag.progressive_injection",
    "rag.speculative_enabled",
    "rag.quality_gate_enabled",
    "rag.graph_retriever_enabled",
    "memory.enabled",
    "rag.web_search_enabled",
    "rag.contextual_retrieval",
    "window.close_to_tray",
    "ui.sidebar_collapsed",
    "app.autostart",
    "rag.retrieval_memory_enabled",
    "update.auto_check",
];

/// 统一设置命令（S09 IPC 精简）。
///
/// 将 ~20 个 `set_xxx` 命令合并为单一入口。前端通过 `settingsApi.update(key, value)` 调用。
/// 内部按 `key` 分发到现有 `*_inner` 函数，复用全部验证逻辑。
///
/// # 参数
/// - `key`: 设置键（必须在 `UPDATEABLE_KEYS` 白名单中）
/// - `value`: 设置值（字符串，布尔值为 `"true"`/`"false"`，数字为字符串形式）
/// - `app`: Tauri AppHandle（仅 `app.autostart` 需要，其他键不使用）
///
/// # 错误
/// - `ERR_VALIDATION`: 不支持的设置键或值类型解析失败
/// - 其他: 底层 `*_inner` 函数返回的错误
#[tauri::command]
pub async fn update_setting(
    key: String,
    value: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    update_setting_inner(&key, &value, &app, state.inner()).await
}

/// `update_setting` 的逻辑实现（命令与集成测试复用）。
///
/// 按 `key` match 分发到对应的 `*_inner` 函数。
/// 布尔值通过 `value == "true"` 判断（宽松匹配：任何非 `"true"` 的值视为 `false`）。
pub async fn update_setting_inner(
    key: &str,
    value: &str,
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<(), String> {
    // 白名单检查
    if !UPDATEABLE_KEYS.contains(&key) {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("不支持的设置键: {key}（可更新的键: {UPDATEABLE_KEYS:?}）"),
        ));
    }

    let enabled = value == "true";

    match key {
        // ── 布尔开关类 ──
        "rag.hybrid_search" => set_hybrid_search_inner(enabled, state).await,
        "rag.rerank_enabled" => set_rerank_enabled_inner(enabled, state).await,
        "rag.hyde_enabled" => set_hyde_enabled_inner(enabled, state).await,
        "rag.agent_enabled" => set_agent_enabled_inner(enabled, state).await,
        "rag.coordinator_enabled" => set_coordinator_mode_inner(enabled, state).await,
        "rag.sub_agent_enabled" => set_sub_agent_enabled_inner(enabled, state).await,
        "mm.vlm_enabled" => set_vlm_enabled_inner(enabled, state).await,
        "rag.progressive_injection" => set_progressive_injection_inner(enabled, state).await,
        "rag.speculative_enabled" => set_speculative_enabled_inner(enabled, state).await,
        "rag.quality_gate_enabled" => set_quality_gate_enabled_inner(enabled, state).await,
        "rag.graph_retriever_enabled" => set_graph_retriever_enabled_inner(enabled, state).await,
        "memory.enabled" => set_memory_enabled_inner(enabled, state).await,
        "rag.web_search_enabled" => set_web_search_enabled_inner(enabled, state).await,
        "rag.contextual_retrieval" => set_contextual_retrieval_inner(enabled, state).await,
        "window.close_to_tray" => set_close_to_tray_inner(enabled, state).await,
        "ui.sidebar_collapsed" => set_sidebar_collapsed_inner(enabled, state).await,
        "rag.retrieval_memory_enabled" => set_retrieval_memory_enabled_inner(enabled, state).await,
        "update.auto_check" => set_update_check_enabled_inner(enabled, state).await,

        // ── 数值类（需 parse） ──
        "compression.ratio" => {
            let ratio = value
                .parse::<f32>()
                .map_err(|_| prefix_error(ERR_VALIDATION, &format!("压缩比必须为数字: {value}")))?;
            set_compression_ratio_inner(ratio, state).await
        }
        "rag.context_token_limit" => {
            let limit = value.parse::<usize>().map_err(|_| {
                prefix_error(ERR_VALIDATION, &format!("token 限制必须为正整数: {value}"))
            })?;
            set_context_token_limit_inner(limit, state).await
        }

        // ── 字符串类 ──
        "vec.embedding_model" => set_embedding_model_inner(value, state).await,

        // ── 需要 AppHandle 的命令 ──
        "app.autostart" => set_autostart_inner(enabled, app.clone(), state).await,

        // 白名单检查已保证不会走到这里
        _ => Err(prefix_error(
            ERR_VALIDATION,
            &format!("不支持的设置键: {key}"),
        )),
    }
}

/// 统一读取命令（S09 IPC 精简）。
///
/// 从 settings 表读取指定键的值，返回原始字符串。
/// 复杂结构（如 `get_settings` 返回的 `SettingsPayload`）仍使用各自的专用命令。
///
/// # 参数
/// - `key`: 设置键
///
/// # 返回
/// 设置值的字符串形式。键不存在时返回空字符串。
#[tauri::command]
pub async fn get_setting(key: String, state: State<'_, AppState>) -> Result<String, String> {
    get_setting_inner(&key, state.inner()).await
}

/// `get_setting` 的逻辑实现（命令与集成测试复用）。
pub async fn get_setting_inner(key: &str, state: &AppState) -> Result<String, String> {
    state
        .storage
        .get_setting(key)
        .await
        .map_err(|e| format!("{e:#}"))
        .map(|v| v.unwrap_or_default())
}

// ============================================================================
// S09: 已废弃的 #[tauri::command] 包装器（已从 generate_handler! 移除）
// ============================================================================
//
// 以下旧 set_xxx 命令的 #[tauri::command] 包装器已从 generate_handler! 移除。
// 它们的 *_inner 逻辑通过 update_setting 统一入口调用。
// 前端已迁移到 settingsApi.update(key, value)。
//
// 被移除的命令清单（22 个）：
//   set_hybrid_search, set_rerank_enabled, set_hyde_enabled, set_agent_enabled,
//   set_coordinator_mode, set_sub_agent_enabled, set_embedding_model,
//   set_compression_ratio, set_context_token_limit, set_vlm_enabled,
//   set_progressive_injection, set_speculative_enabled, set_quality_gate_enabled,
//   set_graph_retriever_enabled, set_memory_enabled, set_web_search_enabled,
//   set_contextual_retrieval, set_close_to_tray, set_sidebar_collapsed,
//   set_autostart, set_retrieval_memory_enabled, set_update_check_enabled
