//! local_llm 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 列出已下载的本地 GGUF 模型（REQ-LLM-004 AC-1）。
#[tauri::command]
pub async fn list_local_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    list_local_models_inner(state.inner()).await
}

/// 模型列表逻辑（命令与集成测试复用）。
pub async fn list_local_models_inner(state: &AppState) -> Result<Vec<ModelInfo>, String> {
    state
        .model_manager()
        .list_models()
        .map_err(|e| format!("{e:#}"))
}

/// 获取推荐模型列表（REQ-LLM-004 AC-6）。
#[tauri::command]
pub async fn get_recommended_models()
-> Result<Vec<echomind_infra::model_manager::RecommendedModel>, String> {
    Ok(echomind_infra::model_manager::RECOMMENDED_MODELS.to_vec())
}

/// 下载模型文件（REQ-LLM-004 AC-2）。
///
/// 从指定 URL 下载 GGUF 模型文件，通过 `model_download_progress` 事件推送进度。
/// 下载在后台 task 中执行，完成后模型文件出现在 `list_local_models` 结果中。
#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    url: String,
    filename: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    download_model_inner(app, url, filename, state.inner()).await
}

/// 下载模型逻辑（命令与集成测试复用）。
pub async fn download_model_inner(
    app: AppHandle,
    url: String,
    filename: String,
    state: &AppState,
) -> Result<(), String> {
    use echomind_models::ModelDownloadProgressPayload;

    let mgr = state.model_manager();
    let app_handle = app.clone();
    let filename_for_progress = filename.clone();

    let progress: echomind_infra::model_manager::DownloadProgressFn =
        Box::new(move |downloaded, total, speed| {
            let _ = app_handle.emit(
                "model_download_progress",
                ModelDownloadProgressPayload {
                    filename: filename_for_progress.clone(),
                    downloaded,
                    total,
                    speed,
                },
            );
        });

    mgr.download_model(&url, &filename, progress)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 暂停指定文件的下载（保留 .partial + .meta.json，可恢复）。
#[tauri::command]
pub async fn pause_download(filename: String, state: State<'_, AppState>) -> Result<(), String> {
    pause_download_inner(filename, state.inner()).await
}

/// 暂停下载逻辑（命令与集成测试复用）。
pub async fn pause_download_inner(filename: String, state: &AppState) -> Result<(), String> {
    state
        .robust_downloader()
        .pause(&filename)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 取消指定文件的下载 + 清理临时文件。
#[tauri::command]
pub async fn abort_download(filename: String, state: State<'_, AppState>) -> Result<(), String> {
    abort_download_inner(filename, state.inner()).await
}

/// 取消下载逻辑（命令与集成测试复用）。
pub async fn abort_download_inner(filename: String, state: &AppState) -> Result<(), String> {
    state
        .robust_downloader()
        .abort(&filename)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 获取下载状态（从 .meta.json 读取）。
#[tauri::command]
pub async fn get_download_status(
    filename: String,
    state: State<'_, AppState>,
) -> Result<Option<echomind_models::DownloadStatus>, String> {
    get_download_status_inner(filename, state.inner()).await
}

/// 获取下载状态逻辑（命令与集成测试复用）。
pub async fn get_download_status_inner(
    filename: String,
    state: &AppState,
) -> Result<Option<echomind_models::DownloadStatus>, String> {
    state
        .robust_downloader()
        .get_status(&filename)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 列出所有未完成下载（扫描 .meta.json 文件）。
#[tauri::command]
pub async fn list_pending_downloads(
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::DownloadStatusSummary>, String> {
    list_pending_downloads_inner(state.inner()).await
}

/// 列出未完成下载逻辑（命令与集成测试复用）。
pub async fn list_pending_downloads_inner(
    state: &AppState,
) -> Result<Vec<echomind_models::DownloadStatusSummary>, String> {
    Ok(state.robust_downloader().list_pending().await)
}

/// 清理所有 `.partial` + `.meta.json` 文件，返回释放的字节数。
#[tauri::command]
pub async fn cleanup_partial_downloads(state: State<'_, AppState>) -> Result<u64, String> {
    cleanup_partial_downloads_inner(state.inner()).await
}

/// 清理临时下载文件逻辑（命令与集成测试复用）。
pub async fn cleanup_partial_downloads_inner(state: &AppState) -> Result<u64, String> {
    Ok(state.robust_downloader().cleanup_partials().await)
}

/// 启动时扫描崩溃恢复（检测 .partial + .meta.json 文件）。
#[tauri::command]
pub async fn scan_download_recovery(
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::DownloadManifest>, String> {
    scan_download_recovery_inner(state.inner()).await
}

/// 崩溃恢复扫描逻辑（命令与集成测试复用）。
pub async fn scan_download_recovery_inner(
    state: &AppState,
) -> Result<Vec<echomind_models::DownloadManifest>, String> {
    Ok(state.robust_downloader().scan_for_recovery().await)
}

/// 删除本地模型文件（REQ-LLM-004 AC-3）。
#[tauri::command]
pub async fn delete_model(filename: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_model_inner(filename, state.inner()).await
}

/// 删除模型逻辑（命令与集成测试复用）。
pub async fn delete_model_inner(filename: String, state: &AppState) -> Result<(), String> {
    state
        .model_manager()
        .delete_model(&filename)
        .map_err(|e| format!("{e:#}"))
}

/// 切换 LLM 推理模式（REQ-LLM-003）。
///
/// 写入 settings 表 `llm.mode`，下次 chat 命令调用时即时生效。
/// - `remote`：使用 BYOK 远程 API（现有行为）
/// - `local`：使用本地 mistral.rs 推理（Pro 功能）
#[tauri::command]
pub async fn set_llm_mode(mode: String, state: State<'_, AppState>) -> Result<(), String> {
    set_llm_mode_inner(mode, state.inner()).await
}

/// 模式切换逻辑（命令与集成测试复用）。
///
/// S14：切换到 local 模式后后台预热模型，消除首次对话卡顿。
/// Free 用户切换到 Local 模式被 Pro 门控拦截。
pub async fn set_llm_mode_inner(mode: String, state: &AppState) -> Result<(), String> {
    let llm_mode = match mode.as_str() {
        "remote" => LlmMode::Remote,
        "local" => {
            // Pro 门控：Free 用户不能切换到 Local 模式
            if !*state.is_pro().read().await {
                return Err("PRO_REQUIRED: Local LLM 模式需要 Pro 授权".to_string());
            }
            LlmMode::Local
        }
        _ => return Err(format!("无效的 LLM 模式: {mode}（可选: remote / local）")),
    };
    state
        .set_llm_mode(llm_mode)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // Q11：同步更新 LlmRouter 的 fallback（下次 chat_inner 调用时路由生效）
    let local_model = state
        .storage
        .get_setting("llm.local_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    state
        .llm_router
        .set_fallback(echomind_core::llm_router::LlmChoice::new(
            llm_mode,
            local_model,
        ))
        .await;

    // S14：切换到 local 模式后后台预热（不阻塞模式切换返回）。
    // 若用户未选择模型或模型文件不存在，local_llm() 返回 Err，预热静默跳过。
    #[cfg(feature = "pro")]
    if mode == "local"
        && let Ok(engine) = state.local_llm().await
    {
        tokio::spawn(async move {
            if let Err(e) = engine.warm_up().await {
                eprintln!("模型预热失败（set_llm_mode）: {e:#}");
            }
        });
    }
    Ok(())
}

/// 获取当前 LLM 推理模式（REQ-LLM-003）。
#[tauri::command]
pub async fn get_llm_mode(state: State<'_, AppState>) -> Result<String, String> {
    get_llm_mode_inner(state.inner()).await
}

/// 模式获取逻辑（命令与集成测试复用）。
pub async fn get_llm_mode_inner(state: &AppState) -> Result<String, String> {
    let mode = state.get_llm_mode().await;
    Ok(match mode {
        LlmMode::Remote => "remote".to_string(),
        LlmMode::Local => "local".to_string(),
    })
}

/// 设置当前选中的本地模型文件名（REQ-LLM-003）。
///
/// 写入 settings 表 `llm.local_model`，切换到 Local 模式时使用此模型。
/// 如果已有 LocalLlmEngine 实例且模型文件不同，会销毁旧实例（下次 chat 重新加载）。
#[tauri::command]
pub async fn set_local_model(filename: String, state: State<'_, AppState>) -> Result<(), String> {
    set_local_model_inner(filename, state.inner()).await
}

/// 本地模型设置逻辑（命令与集成测试复用）。
///
/// S14：设置完成后后台预热模型，消除首次对话卡顿。
/// Free 用户被 Pro 门控拦截。
pub async fn set_local_model_inner(filename: String, state: &AppState) -> Result<(), String> {
    // Pro 门控：Free 用户不能设置本地模型
    if !*state.is_pro().read().await {
        return Err("PRO_REQUIRED: 本地模型设置需要 Pro 授权".to_string());
    }
    state
        .storage
        .set_setting("llm.local_model", &filename)
        .await
        .map_err(|e| format!("{e:#}"))?;
    // 销毁已有引擎实例（模型文件可能已变），下次 chat 重新创建
    #[cfg(feature = "pro")]
    {
        state.unload_local_llm().await;
        // S14：后台预热新模型（不阻塞设置操作返回）。
        // local_llm() 仅创建引擎结构（快），warm_up() 加载 GGUF 权重到内存（慢）。
        // 预热失败不影响设置操作——用户首次 chat 时会重新尝试加载并报错。
        if let Ok(engine) = state.local_llm().await {
            tokio::spawn(async move {
                if let Err(e) = engine.warm_up().await {
                    eprintln!("模型预热失败（set_local_model）: {e:#}");
                }
            });
        }
    }
    Ok(())
}

/// 获取本地推理设备类型（REQ-LLM-003 扩展）。
///
/// 返回 `"cpu"` / `"metal"` / `"cuda"` / `"unknown"`。
/// 前端用于显示当前推理设备（CPU/GPU）。
///
/// # 错误
/// - 本地 LLM 引擎未初始化（未选择模型）时返回错误描述。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn get_local_llm_device_kind(state: State<'_, AppState>) -> Result<String, String> {
    get_local_llm_device_kind_inner(state.inner()).await
}

/// 设备类型获取逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn get_local_llm_device_kind_inner(state: &AppState) -> Result<String, String> {
    let engine = state.local_llm().await.map_err(|e| format!("{e:#}"))?;
    Ok(engine.device_kind().to_string())
}

/// 配置 PagedAttention 高效 KV cache 管理（REQ-LLM-003 扩展，S10）。
///
/// 写入 settings 表 `llm.paged_attn` / `llm.block_size` / `llm.gpu_memory_ctx`。
/// 如果已有 LocalLlmEngine 实例，会销毁旧实例（下次 chat 重新加载时应用新配置）。
///
/// # 参数
/// - `enabled`: 是否启用 PagedAttention
/// - `block_size`: KV cache 块大小（支持 8/16/32）
/// - `gpu_memory_ctx`: GPU 上下文 token 数（默认 4096）
///
/// # 注意
/// PagedAttention 仅在 GPU 模式（metal/cuda feature 编译）下生效。
/// CPU 模式下此配置被忽略。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn set_paged_attn(
    enabled: bool,
    block_size: usize,
    gpu_memory_ctx: usize,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_paged_attn_inner(enabled, block_size, gpu_memory_ctx, state.inner()).await
}

/// PagedAttention 配置逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn set_paged_attn_inner(
    enabled: bool,
    block_size: usize,
    gpu_memory_ctx: usize,
    state: &AppState,
) -> Result<(), String> {
    // 参数校验
    if block_size != 8 && block_size != 16 && block_size != 32 {
        return Err(format!("无效的块大小: {block_size}（支持: 8 / 16 / 32）"));
    }
    if gpu_memory_ctx == 0 {
        return Err("GPU 上下文 token 数必须大于 0".to_string());
    }
    state
        .storage
        .set_setting("llm.paged_attn", if enabled { "true" } else { "false" })
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.block_size", &block_size.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.gpu_memory_ctx", &gpu_memory_ctx.to_string())
        .await
        .map_err(|e| format!("{e:#}"))?;
    // 销毁已有引擎实例（PagedAttention 配置变更需要重新加载模型）
    state.unload_local_llm().await;
    Ok(())
}

/// 设置本地推理采样参数（S11，Pro 门控）。
///
/// 将采样参数 JSON 序列化后持久化到 settings 表 `llm.sampling` 键。
/// 如果本地 LLM 引擎已加载，同时即时更新引擎的运行时采样参数（无需重新加载模型）。
///
/// # 参数
/// - `params`: 采样参数。各字段为 `Option`，`None` 表示使用引擎默认值。
///
/// # 参数校验
/// - `temperature`: 0.0 ~ 2.0
/// - `top_p`: 0.0 ~ 1.0
/// - `top_k`: 1 ~ 100
/// - `max_tokens`: > 0
/// - `frequency_penalty`: -2.0 ~ 2.0
/// - `presence_penalty`: -2.0 ~ 2.0
///
/// # 错误
/// 参数范围不合法时返回错误描述字符串。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn set_sampling_params(
    params: LlmSamplingParams,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_sampling_params_inner(params, state.inner()).await
}

/// 采样参数写入逻辑（命令与集成测试复用）。
///
/// 持久化到 settings 表 + 即时更新引擎运行时状态。
#[cfg(feature = "pro")]
pub async fn set_sampling_params_inner(
    params: LlmSamplingParams,
    state: &AppState,
) -> Result<(), String> {
    // 参数校验
    if let Some(temp) = params.temperature
        && !(0.0..=2.0).contains(&temp)
    {
        return Err(format!("temperature 超出有效范围 [0.0, 2.0]：{temp}"));
    }
    if let Some(top_p) = params.top_p
        && !(0.0..=1.0).contains(&top_p)
    {
        return Err(format!("top_p 超出有效范围 [0.0, 1.0]：{top_p}"));
    }
    if let Some(top_k) = params.top_k
        && !(1..=100).contains(&top_k)
    {
        return Err(format!("top_k 超出有效范围 [1, 100]：{top_k}"));
    }
    if let Some(max_tokens) = params.max_tokens
        && max_tokens == 0
    {
        return Err("max_tokens 必须大于 0".to_string());
    }
    if let Some(fp) = params.frequency_penalty
        && !(-2.0..=2.0).contains(&fp)
    {
        return Err(format!("frequency_penalty 超出有效范围 [-2.0, 2.0]：{fp}"));
    }
    if let Some(pp) = params.presence_penalty
        && !(-2.0..=2.0).contains(&pp)
    {
        return Err(format!("presence_penalty 超出有效范围 [-2.0, 2.0]：{pp}"));
    }

    // JSON 序列化持久化
    let json = serde_json::to_string(&params).map_err(|e| format!("采样参数序列化失败: {e}"))?;
    state
        .storage
        .set_setting("llm.sampling", &json)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 即时更新引擎运行时采样参数（如果引擎已加载）
    // 不需要 unload_local_llm — 采样参数可在运行时动态修改
    // 如果引擎未加载（未选择模型），静默跳过 — 下次加载时会从 settings 表读取
    if let Ok(engine) = state.local_llm().await {
        engine.set_sampling_params(params).await;
    }

    Ok(())
}

/// 切换推理内核模式（Phase 3，Pro 门控）。
///
/// 写入 settings 表 `llm.kernel_mode`，下次 chat 命令调用时即时生效。
/// - `mistral`：使用 mistral.rs 引擎（默认，Phase 1/2）
/// - `custom`：使用自研 GEMV 内核（Phase 3，单批次优化）
///
/// 切换到 `custom` 模式后，首次 chat 会触发 `load_custom_weights()`
/// 加载 GGUF 文件并重排权重（使用自研 GgufFile 解析器 + repack_for_gemv）。
///
/// # 错误
///
/// - 无效的模式字符串（非 `mistral` / `custom`）
/// - settings 表写入失败
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn set_kernel_mode(mode: String, state: State<'_, AppState>) -> Result<(), String> {
    set_kernel_mode_inner(mode, state.inner()).await
}

/// 内核模式切换逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn set_kernel_mode_inner(mode: String, state: &AppState) -> Result<(), String> {
    let kernel_mode =
        echomind_infra::local_llm::KernelMode::from_str(&mode).map_err(|e| format!("{e:#}"))?;

    // 持久化到 settings 表
    state
        .storage
        .set_setting("llm.kernel_mode", kernel_mode.as_str())
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 即时更新引擎运行时状态（如果引擎已加载）
    if let Ok(engine) = state.local_llm().await {
        engine.set_kernel_mode(kernel_mode).await;
    }

    Ok(())
}

/// 获取当前推理内核模式（Phase 3，Pro 门控）。
///
/// 从 settings 表读取 `llm.kernel_mode`，返回 `"mistral"` 或 `"custom"`。
/// 未设置时返回默认值 `"mistral"`。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn get_kernel_mode(state: State<'_, AppState>) -> Result<String, String> {
    get_kernel_mode_inner(state.inner()).await
}

/// 内核模式读取逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn get_kernel_mode_inner(state: &AppState) -> Result<String, String> {
    let stored = state
        .storage
        .get_setting("llm.kernel_mode")
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(stored.unwrap_or_else(|| "mistral".to_string()))
}

/// 保存指定会话的 KV cache 到磁盘（REQ-LLM-009）。
///
/// 将当前已加载模型的 KV cache 快照保存到 `{data_dir}/kv_cache/{conversation_id}.emkv`。
/// 用于对话切换时自动保存上下文，消除重复前缀计算。
///
/// **Pro 门控**：仅在 `--features pro` 时可用。Free 用户调用返回错误。
///
/// # 行为
///
/// - 如果 `llm.kv_cache_enabled` 设置为 `false`（默认），直接返回 `Ok(())`（空操作）
/// - 如果本地 LLM 引擎未初始化（未选择模型），直接返回 `Ok(())`（无内容可保存）
/// - 保存的文件使用原子写入（.tmp → rename），防止崩溃损坏
///
/// # 参数
/// - `conversation_id` — 会话 ID（用作文件名，自动清理非法字符）
///
/// # 错误
///
/// - 非 Pro 用户
/// - 目录创建或文件写入失败
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn save_kv_cache(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    save_kv_cache_inner(conversation_id, state.inner()).await
}

/// KV cache 保存逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn save_kv_cache_inner(conversation_id: String, state: &AppState) -> Result<(), String> {
    // 检查是否启用
    let enabled = state
        .storage
        .get_setting("llm.kv_cache_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    if !enabled {
        return Ok(());
    }

    // 获取 KV cache 目录
    let kv_cache_dir = state.kv_cache_dir();

    // 获取本地 LLM 引擎（如果未初始化则静默跳过）
    let engine = match state.local_llm().await {
        Ok(e) => e,
        Err(_) => return Ok(()), // 引擎未初始化，无内容可保存
    };

    engine
        .save_kv_cache(&conversation_id, &kv_cache_dir)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 恢复指定会话的 KV cache（REQ-LLM-009）。
///
/// 从 `{data_dir}/kv_cache/{conversation_id}.emkv` 文件加载 KV cache 快照，
/// 验证模型名匹配后注入到当前引擎实例。
///
/// **Pro 门控**：仅在 `--features pro` 时可用。Free 用户调用返回错误。
///
/// # 行为
///
/// - 如果 `llm.kv_cache_enabled` 设置为 `false`（默认），直接返回 `Ok(false)`（空操作）
/// - 如果缓存文件不存在，返回 `Ok(false)`（cache miss）
/// - 如果模型名不匹配，返回 `Ok(false)`（模型已切换，旧缓存无效）
///
/// # 参数
/// - `conversation_id` — 会话 ID
///
/// # 返回
///
/// - `true` — 缓存命中且模型名匹配
/// - `false` — 缓存未命中或禁用
///
/// # 错误
///
/// - 非 Pro 用户
/// - 文件读取失败（IO 错误，非文件不存在）
/// - 反序列化失败（文件损坏）
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn load_kv_cache(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    load_kv_cache_inner(conversation_id, state.inner()).await
}

/// KV cache 恢复逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn load_kv_cache_inner(
    conversation_id: String,
    state: &AppState,
) -> Result<bool, String> {
    // 检查是否启用
    let enabled = state
        .storage
        .get_setting("llm.kv_cache_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    if !enabled {
        return Ok(false);
    }

    // 获取 KV cache 目录
    let kv_cache_dir = state.kv_cache_dir();

    // 获取本地 LLM 引擎（如果未初始化则 cache miss）
    let engine = match state.local_llm().await {
        Ok(e) => e,
        Err(_) => return Ok(false),
    };

    engine
        .restore_kv_cache(&conversation_id, &kv_cache_dir)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 清除指定会话的 KV cache（REQ-LLM-009）。
///
/// 删除 `{data_dir}/kv_cache/{conversation_id}.emkv` 文件。
/// 不需要模型已加载——纯文件操作。
///
/// **Pro 门控**：仅在 `--features pro` 时可用。
///
/// # 参数
/// - `conversation_id` — 会话 ID
///
/// # 行为
///
/// - 如果文件不存在，静默返回 `Ok(())`（幂等）
/// - 如果 `llm.kv_cache_enabled` 为 `false`，仍执行清除（清理旧数据）
///
/// # 错误
///
/// - 非 Pro 用户
/// - 文件删除失败（IO 错误，非文件不存在）
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn clear_kv_cache(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    clear_kv_cache_inner(conversation_id, state.inner()).await
}

/// KV cache 清除逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn clear_kv_cache_inner(conversation_id: String, state: &AppState) -> Result<(), String> {
    let kv_cache_dir = state.kv_cache_dir();

    // 清理 conversation_id 中的非法字符（与 save/load 一致）
    let safe_id: String = conversation_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_path = kv_cache_dir.join(format!("{safe_id}.emkv"));

    // 文件不存在 = 幂等返回
    if !file_path.exists() {
        return Ok(());
    }

    std::fs::remove_file(&file_path).map_err(|e| format!("删除 KV cache 文件失败: {e}"))?;

    Ok(())
}

/// 获取 KV cache 持久化状态（REQ-LLM-009）。
///
/// 返回 KV cache 启用状态、缓存目录路径、文件数量和总占用磁盘空间。
/// 不需要模型已加载——纯文件系统扫描。
///
/// **Pro 门控**：仅在 `--features pro` 时可用。
///
/// # 返回
///
/// `KvCacheStatus { enabled, cache_dir, file_count, total_size_bytes }`
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn get_kv_cache_status(state: State<'_, AppState>) -> Result<KvCacheStatus, String> {
    get_kv_cache_status_inner(state.inner()).await
}

/// KV cache 状态查询逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn get_kv_cache_status_inner(state: &AppState) -> Result<KvCacheStatus, String> {
    let enabled = state
        .storage
        .get_setting("llm.kv_cache_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");

    let kv_cache_dir = state.kv_cache_dir();
    let cache_dir = kv_cache_dir.to_string_lossy().to_string();

    let (file_count, total_size_bytes) = if kv_cache_dir.exists() {
        let mut count = 0usize;
        let mut total = 0u64;
        if let Ok(entries) = std::fs::read_dir(&kv_cache_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|ext| ext == "emkv") {
                    count += 1;
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }
        (count, total)
    } else {
        (0, 0)
    };

    Ok(KvCacheStatus {
        enabled,
        cache_dir,
        file_count,
        total_size_bytes,
    })
}

/// 设置 KV cache 持久化开关（REQ-LLM-009）。
///
/// 写入 settings 表 `llm.kv_cache_enabled`（默认 `false`）。
/// 启用后，对话切换时自动保存/恢复 KV cache，消除重复前缀计算。
///
/// **Pro 门控**：仅在 `--features pro` 时可用。Free 用户调用返回错误。
///
/// # 参数
/// - `enabled` — 是否启用 KV cache 持久化
///
/// # 错误
///
/// - 非 Pro 用户
/// - settings 表写入失败
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn set_kv_cache_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_kv_cache_enabled_inner(enabled, state.inner()).await
}

/// KV cache 开关写入逻辑（命令与集成测试复用）。
#[cfg(feature = "pro")]
pub async fn set_kv_cache_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("llm.kv_cache_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}
