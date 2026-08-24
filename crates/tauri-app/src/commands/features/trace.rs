//! 日志级别 / 诊断信息导出 / 全量数据备份与恢复（REQ-OBS-001/002, REQ-EXP-002/003）。
use super::super::*;

/// 备份数据结构（JSON 序列化）
#[derive(serde::Serialize, serde::Deserialize)]
struct BackupData {
    /// 备份格式版本号
    version: u32,
    /// 导出时间戳（ISO 8601）
    exported_at: String,
    /// 全部会话
    conversations: Vec<Conversation>,
    /// 全部消息（按会话 ID 分组）
    messages: std::collections::HashMap<String, Vec<ChatMessage>>,
    /// 全部文档
    documents: Vec<Document>,
    /// 全部设置项（键值对）
    settings: std::collections::HashMap<String, String>,
}

/// 获取当前日志级别（REQ-OBS-001 AC-5）。
///
/// 从 settings 表 `log.level` 键读取，默认 `"info"`。
///
/// # 返回
///
/// `"debug"` / `"info"` / `"warn"` / `"error"`
#[tauri::command]
pub async fn get_log_level(state: State<'_, AppState>) -> Result<String, String> {
    get_log_level_inner(state.inner()).await
}

/// 日志级别查询逻辑（命令与集成测试复用）。
pub async fn get_log_level_inner(state: &AppState) -> Result<String, String> {
    let level = state
        .storage
        .get_setting("log.level")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "info".to_string());
    Ok(level)
}

/// 设置日志级别（REQ-OBS-001 AC-5）。
///
/// 持久化到 settings 表 `log.level` 键，同时通过 `LocalLogger::set_level()`
/// 动态更新运行时过滤级别（无需重启应用）。
///
/// # 参数
///
/// - `level` — `"debug"` / `"info"` / `"warn"` / `"error"`（不区分大小写）
///
/// # 错误
///
/// - 无效的日志级别
/// - settings 表写入失败
/// - 日志系统未初始化（降级为仅持久化，下次启动生效）
#[tauri::command]
pub async fn set_log_level(level: String, state: State<'_, AppState>) -> Result<(), String> {
    set_log_level_inner(level, state.inner()).await
}

/// 日志级别设置逻辑（命令与集成测试复用）。
pub async fn set_log_level_inner(level: String, state: &AppState) -> Result<(), String> {
    // 验证级别有效性
    let normalized = level.to_lowercase();
    match normalized.as_str() {
        "debug" | "info" | "warn" | "error" => {}
        _ => {
            return Err(format!(
                "无效的日志级别: {level}（可选: debug/info/warn/error）"
            ));
        }
    }

    // 持久化到 settings 表
    state
        .storage
        .set_setting("log.level", &normalized)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 动态更新运行时过滤级别（日志系统可能未初始化，降级为下次启动生效）
    if let Err(e) = echomind_infra::local_logger::LocalLogger::set_level(&normalized) {
        eprintln!("运行时切换日志级别失败（下次启动将生效）: {e}");
    }

    Ok(())
}

/// 导出日志文件内容（REQ-OBS-001）。
///
/// 读取最近 `tail_lines` 行日志（默认 100 行），返回文本内容。
///
/// # 参数
///
/// - `tail_lines` — 返回最近 N 行日志（默认 100）
///
/// # 返回
///
/// 日志文本内容（JSON Lines 格式，每行一个 JSON 对象）
#[tauri::command]
pub async fn export_logs(
    tail_lines: Option<usize>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    export_logs_inner(tail_lines, state.inner()).await
}

/// 日志导出逻辑（命令与集成测试复用）。
pub async fn export_logs_inner(
    tail_lines: Option<usize>,
    state: &AppState,
) -> Result<String, String> {
    let n = tail_lines.unwrap_or(100);
    let log_dir = state.logs_dir();

    if !log_dir.exists() {
        return Ok("(日志目录不存在)".to_string());
    }

    echomind_infra::local_logger::LocalLogger::read_logs_from_dir(&log_dir, n)
        .map_err(|e| format!("读取日志文件失败: {e:#}"))
}

/// 导出错误日志（REQ-ERR-005 v1.6）。
///
/// 读取最近 1000 行日志，过滤出 ERROR 级别的条目，返回为 JSON Lines 格式字符串。
/// 每行一个 JSON 对象，包含时间戳、级别、目标模块、消息内容。
/// 不包含用户文档内容或 API Key 明文。
///
/// # 返回
///
/// JSON Lines 格式字符串（每行一个 ERROR 级别日志条目）。无错误日志时返回空字符串。
#[tauri::command]
pub async fn export_error_logs(state: State<'_, AppState>) -> Result<String, String> {
    export_error_logs_inner(state.inner()).await
}

/// 错误日志导出逻辑（命令与集成测试复用）。
pub async fn export_error_logs_inner(state: &AppState) -> Result<String, String> {
    let log_dir = state.logs_dir();

    if !log_dir.exists() {
        return Ok(String::new());
    }

    let logs = echomind_infra::local_logger::LocalLogger::read_logs_from_dir(&log_dir, 1000)
        .map_err(|e| format!("读取日志文件失败: {e:#}"))?;

    // 过滤 ERROR 级别日志行
    // tracing JSON Lines 格式：{"timestamp":"...","level":"ERROR","target":"...","message":"..."}
    let error_lines: String = logs
        .lines()
        .filter(|line| line.contains("\"level\":\"ERROR\"") || line.contains("\"level\":\"error\""))
        .collect::<Vec<&str>>()
        .join("\n");

    Ok(error_lines)
}

/// 导出诊断信息（REQ-OBS-002）。
///
/// 收集系统信息、应用版本、数据库规模、知识库规模、嵌入维度、
/// LLM 配置（脱敏）、最近 100 行日志，聚合为 JSON 字符串。
///
/// **隐私铁律**：不包含 API Key 明文、不包含用户文档内容或对话内容。
///
/// # 返回
///
/// JSON 格式字符串，包含：
/// - `app_version` — 应用版本
/// - `timestamp` — 导出时间（ISO 8601）
/// - `system` — 操作系统、架构、CPU 核心数、内存（MB）
/// - `database` — 数据库路径、大小
/// - `knowledge_base` — 文档数、chunk 数、嵌入维度
/// - `llm_config` — LLM 配置（脱敏）
/// - `recent_logs` — 最近 100 行日志
#[tauri::command]
pub async fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    export_diagnostics_inner(state.inner()).await
}

/// 诊断信息导出逻辑（命令与集成测试复用）。
pub async fn export_diagnostics_inner(state: &AppState) -> Result<String, String> {
    // 收集系统信息
    let doc_count = state
        .storage
        .count_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let chunk_count = state
        .storage
        .count_chunks()
        .await
        .map_err(|e| format!("{e:#}"))?;

    // LLM 配置（脱敏）
    let llm_config = state.llm_config().read().await.clone();
    let (api_key_masked, model, base_url) = match &llm_config {
        Some(cfg) => (
            Some(mask_api_key(&cfg.api_key)),
            Some(cfg.model.clone()),
            Some(cfg.base_url.clone()),
        ),
        None => (None, None, None),
    };

    // LLM 模式
    let llm_mode = state.get_llm_mode().await;
    let mode_str = match llm_mode {
        LlmMode::Remote => "remote",
        LlmMode::Local => "local",
    };

    let db_path = state.data_dir.join("echomind.db");

    let diagnostics = echomind_infra::local_logger::collect_diagnostics(
        env!("CARGO_PKG_VERSION"),
        &state.data_dir,
        &db_path,
        doc_count,
        chunk_count,
        384, // all-MiniLM-L6-v2 嵌入维度
        api_key_masked.as_deref(),
        model.as_deref(),
        base_url.as_deref(),
        Some(mode_str),
    );

    serde_json::to_string_pretty(&diagnostics).map_err(|e| format!("序列化诊断信息失败: {e:#}"))
}

// ============================================================
// 全量数据备份与恢复（REQ-EXP-002/003）
// ============================================================

/// 导出全量数据为 JSON 字符串（REQ-EXP-002）。
///
/// 导出内容：会话、消息、文档元数据、设置项。
/// 不导出：嵌入向量（体积过大）、文档原始文件副本（由用户单独保管）。
/// API Key 以脱敏形式导出（**** + 后 4 位），不包含明文。
///
/// # 返回
/// JSON 字符串，前端通过 `save_text_file` 保存到用户选择的路径。
#[tauri::command]
pub async fn export_backup(state: State<'_, AppState>) -> Result<String, String> {
    export_backup_inner(state.inner()).await
}

/// 备份导出逻辑（命令与集成测试复用）。
pub async fn export_backup_inner(state: &AppState) -> Result<String, String> {
    let storage = &state.storage;

    // 1. 导出会话
    let conversations = storage
        .list_conversations(DEFAULT_WORKSPACE)
        .await
        .map_err(|e| format!("{ERR_STORAGE}: 导出会话失败: {e:#}"))?;

    // 2. 导出消息（按会话分组）
    let mut messages = std::collections::HashMap::new();
    for conv in &conversations {
        let msgs = storage
            .list_messages(&conv.id)
            .await
            .map_err(|e| format!("{ERR_STORAGE}: 导出消息失败: {e:#}"))?;
        messages.insert(conv.id.clone(), msgs);
    }

    // 3. 导出文档
    let documents = storage
        .list_documents()
        .await
        .map_err(|e| format!("{ERR_STORAGE}: 导出文档失败: {e:#}"))?;

    // 4. 导出设置（脱敏 API Key）
    let mut settings = std::collections::HashMap::new();
    // 列出已知设置键
    let known_keys = [
        "llm.api_key",
        "llm.base_url",
        "llm.model",
        "llm.mode",
        "llm.local_model",
        "ui.theme",
        "ui.locale",
        "ui.sidebar_collapsed",
        "rag.hybrid_search",
        "rag.rerank_enabled",
        "rag.hyde_enabled",
        "rag.contextual_retrieval",
        "rag.graph_retriever_enabled",
        "rag.agent_enabled",
        "rag.sub_agent_enabled",
        "rag.web_search_enabled",
        "rag.progressive_injection",
        "rag.speculative_enabled",
        "rag.retrieval_memory_enabled",
        "memory.enabled",
        "cache.enabled",
        "cache.ttl_secs",
        "cache.semantic_threshold",
        "cache.privacy_mode",
        "compression.ratio",
        "log.level",
        "security.posture",
        "security.auto_lock_timeout",
        "security.pii_detection_enabled",
        "security.clipboard_clear_timeout",
        "vlm.enabled",
        "export.pdf_page_size",
        "export.pdf_include_sources",
        "update_check.enabled",
        "autostart.enabled",
        "llm.kv_cache_enabled",
        "llm.kernel_mode",
        "llm.paged_attn",
        "license.is_pro",
        "license.key",
    ];
    for key in &known_keys {
        if let Ok(Some(value)) = storage.get_setting(key).await {
            // 脱敏 API Key（REQ-EXP-002 AC-6）
            if *key == "llm.api_key" && value.len() > 4 {
                settings.insert(
                    key.to_string(),
                    format!("****{}", &value[value.len() - 4..]),
                );
            } else if *key == "license.key" {
                // 许可证密钥不导出（敏感信息）
                continue;
            } else {
                settings.insert(key.to_string(), value);
            }
        }
    }

    // 5. 构建备份数据
    let backup = BackupData {
        version: 1,
        exported_at: format!(
            "{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        ),
        conversations,
        messages,
        documents,
        settings,
    };

    serde_json::to_string_pretty(&backup)
        .map_err(|e| format!("{ERR_PARSE}: 序列化备份数据失败: {e:#}"))
}

/// 从 JSON 备份恢复数据（REQ-EXP-003）。
///
/// 恢复前需用户在前端确认对话框中同意覆盖现有数据。
/// 恢复流程：解析 JSON → 校验版本 → 写入会话/消息/文档/设置。
/// 恢复后建议用户重启应用以加载新数据。
///
/// # 参数
/// - `content` — JSON 格式的备份数据字符串
///
/// # 返回
/// 恢复统计信息 JSON（会话数/消息数/文档数/设置项数）
#[tauri::command]
pub async fn import_backup(content: String, state: State<'_, AppState>) -> Result<String, String> {
    import_backup_inner(&content, state.inner()).await
}

/// 备份恢复逻辑（命令与集成测试复用）。
pub async fn import_backup_inner(content: &str, state: &AppState) -> Result<String, String> {
    // 1. 解析 JSON
    let backup: BackupData = serde_json::from_str(content)
        .map_err(|e| format!("{ERR_PARSE}: 解析备份数据失败: {e:#}"))?;

    // 2. 校验版本号
    if backup.version != 1 {
        return Err(format!(
            "{ERR_VALIDATION}: 不支持的备份版本 {}，当前仅支持版本 1",
            backup.version
        ));
    }

    let storage = &state.storage;

    // 3. 恢复设置（最先恢复，确保配置就绪）
    let mut settings_count = 0u32;
    for (key, value) in &backup.settings {
        // 跳过脱敏的 API Key（**** 开头），保留现有配置
        if key == "llm.api_key" && value.starts_with("****") {
            continue;
        }
        storage
            .set_setting(key, value)
            .await
            .map_err(|e| format!("{ERR_STORAGE}: 恢复设置项失败: {e:#}"))?;
        settings_count += 1;
    }

    // 4. 恢复文档
    let mut doc_count = 0u32;
    for doc in &backup.documents {
        // 检查文档是否已存在（幂等恢复）
        if let Ok(Some(_)) = storage.find_document_by_hash(&doc.file_hash).await {
            continue;
        }
        storage
            .add_document(doc)
            .await
            .map_err(|e| format!("{ERR_STORAGE}: 恢复文档失败: {e:#}"))?;
        doc_count += 1;
    }

    // 5. 恢复会话
    let mut conv_count = 0u32;
    for conv in &backup.conversations {
        storage
            .create_conversation(conv)
            .await
            .map_err(|e| format!("{ERR_STORAGE}: 恢复会话失败: {e:#}"))?;
        conv_count += 1;
    }

    // 6. 恢复消息
    let mut msg_count = 0u32;
    for (conv_id, msgs) in &backup.messages {
        for msg in msgs {
            storage
                .add_message(conv_id, msg)
                .await
                .map_err(|e| format!("{ERR_STORAGE}: 恢复消息失败: {e:#}"))?;
            msg_count += 1;
        }
    }

    info!(
        "备份恢复完成: {} 会话, {} 消息, {} 文档, {} 设置项",
        conv_count, msg_count, doc_count, settings_count
    );

    // 7. 返回统计信息
    let stats = serde_json::json!({
        "conversations": conv_count,
        "messages": msg_count,
        "documents": doc_count,
        "settings": settings_count,
    });
    serde_json::to_string(&stats).map_err(|e| format!("{ERR_PARSE}: 序列化统计信息失败: {e:#}"))
}
