//! 持久化记忆系统 + Scratch 层整合 + Burst Buffer（REQ-RAG-033, Q01/Q02）。
use super::super::*;

/// 启用/禁用持久化记忆系统。
///
/// 持久化到 settings 表 `memory.enabled` 键，下次 chat 命令调用时即时生效。
/// 启用后：ChatEngine 检索相关跨会话记忆注入 system prompt；AutoDream 后台自动整合记忆。
#[tauri::command]
pub async fn set_memory_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_memory_enabled_inner(enabled, state.inner()).await
}

/// 记忆开关逻辑（命令与集成测试复用）。
pub async fn set_memory_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("memory.enabled", if enabled { "true" } else { "false" })
        .await
        .map_err(|e| format!("{e:#}"))?;
    // 运行时即时更新 AppState 字段
    // 注意：AppState 字段不可变，需通过内部 RwLock 或 AtomicBool 更新
    // 当前实现：下次 chat 命令从 settings batch 读取，state.memory_enabled 在重启后更新
    // 未来优化：使用 AtomicBool 字段实现即时生效
    Ok(())
}

/// 获取所有记忆条目（可按层级过滤）。
///
/// `tier` 为 `None` 时返回所有层级的记忆，按 importance DESC 排序。
#[tauri::command]
pub async fn get_memories(
    tier: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryEntry>, String> {
    get_memories_inner(tier.as_deref(), state.inner()).await
}

/// 记忆查询逻辑（命令与集成测试复用）。
pub async fn get_memories_inner(
    tier: Option<&str>,
    state: &AppState,
) -> Result<Vec<MemoryEntry>, String> {
    let tier_enum = match tier {
        Some("wing") => Some(MemoryTier::Wing),
        Some("hall") => Some(MemoryTier::Hall),
        Some("room") => Some(MemoryTier::Room),
        Some(_) => return Err("无效的层级，可选值：wing / hall / room".to_string()),
        None => None,
    };
    let mut entries = state
        .storage
        .get_memory_entries(tier_enum.as_ref())
        .await
        .map_err(|e| format!("{e:#}"))?;
    entries.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(entries)
}

/// 用户手动置顶记忆（直接创建 Room 层，importance 1.0）。
///
/// 用于用户明确希望永久记住的关键信息（如个人偏好、重要决定）。
#[tauri::command]
pub async fn pin_memory(
    conversation_id: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<MemoryEntry, String> {
    pin_memory_inner(&conversation_id, &content, state.inner()).await
}

/// 置顶记忆逻辑（命令与集成测试复用）。
pub async fn pin_memory_inner(
    conversation_id: &str,
    content: &str,
    state: &AppState,
) -> Result<MemoryEntry, String> {
    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    memory_store
        .pin_memory(conversation_id, content)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 手动提升记忆层级（Wing → Hall → Room）。
///
/// 每次提升 importance += 0.1（上限 1.0）。Room 层无法再提升。
#[tauri::command]
pub async fn promote_memory(memory_id: String, state: State<'_, AppState>) -> Result<(), String> {
    promote_memory_inner(&memory_id, state.inner()).await
}

/// 记忆提升逻辑（命令与集成测试复用）。
pub async fn promote_memory_inner(memory_id: &str, state: &AppState) -> Result<(), String> {
    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    memory_store
        .promote(memory_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除指定记忆条目。
#[tauri::command]
pub async fn delete_memory(memory_id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_memory_inner(&memory_id, state.inner()).await
}

/// 删除记忆逻辑（命令与集成测试复用）。
pub async fn delete_memory_inner(memory_id: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .delete_memory_entry(memory_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 清空记忆条目（可按层级过滤）。
///
/// `tier` 为 `None` 时清空所有层级。返回删除的行数。
#[tauri::command]
pub async fn clear_memories(
    tier: Option<String>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    clear_memories_inner(tier.as_deref(), state.inner()).await
}

/// 清空记忆逻辑（命令与集成测试复用）。
pub async fn clear_memories_inner(tier: Option<&str>, state: &AppState) -> Result<usize, String> {
    let tier_enum = match tier {
        Some("wing") => Some(MemoryTier::Wing),
        Some("hall") => Some(MemoryTier::Hall),
        Some("room") => Some(MemoryTier::Room),
        Some(_) => return Err("无效的层级，可选值：wing / hall / room".to_string()),
        None => None,
    };
    state
        .storage
        .clear_memory_entries(tier_enum.as_ref())
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 手动触发 Scratch 层记忆整合（Q01 借鉴 QM consolidation）。
///
/// 读取 scratch_logs 表中的临时事实，通过 LLM 审查后执行 UPDATE/DELETE/ADD 动作，
/// 将有价值的事实 promote 到长期记忆层（Wing/Hall/Room）。
/// 整合完成后清除已处理的 scratch 条目。
///
/// 需要已配置 LLM（api_key + base_url + model）。未配置时返回错误。
#[tauri::command]
pub async fn trigger_memory_consolidation(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    trigger_memory_consolidation_inner(state.inner()).await
}

/// 记忆整合逻辑（命令与集成测试复用）。
pub async fn trigger_memory_consolidation_inner(
    state: &AppState,
) -> Result<serde_json::Value, String> {
    // 初始化 LLM Provider
    let llm_config = state.llm_config().read().await.clone();
    let provider = match llm_config {
        Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
            Ok(p) => p,
            Err(e) => {
                return Err(format!("LLM 初始化失败: {e:#}"));
            }
        },
        None => {
            return Err("未配置 LLM：请完成初始配置向导".to_string());
        }
    };

    // 执行 Scratch 整合
    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    let result = memory_store
        .consolidate_scratch(&provider, 14)
        .await
        .map_err(|e| format!("记忆整合失败: {e:#}"))?;

    Ok(serde_json::json!({
        "actions_count": result.actions.len(),
        "expired_cleaned": result.expired_cleaned,
        "remaining_scratch": result.remaining_scratch,
    }))
}

/// 获取 Scratch 层日志条目（Q01 借鉴 QM scratch-promote）。
///
/// 返回按创建时间正序排列的 scratch 日志，可选限制数量。
/// `limit` 为 `None` 时返回全部条目。
#[tauri::command]
pub async fn get_scratch_logs(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::ScratchLogEntry>, String> {
    get_scratch_logs_inner(limit, state.inner()).await
}

/// Scratch 日志查询逻辑（命令与集成测试复用）。
pub async fn get_scratch_logs_inner(
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<echomind_models::ScratchLogEntry>, String> {
    state
        .storage
        .get_scratch_logs(limit)
        .await
        .map_err(|e| format!("{e:#}"))
}

// ============================================================
// Burst Buffer IPC 命令（Q02 借鉴 QM createBurstBuffer）
// ============================================================

/// 推入一轮对话到 Burst Buffer（Q02 借鉴 QM createBurstBuffer）。
///
/// 前端在 `chat_done` 事件后调用此命令，将本轮对话推入 burst buffer。
/// 如果满足 flush 条件（静默窗口 / 最大轮次），自动异步触发 flush。
///
/// # 参数
/// - `conversation_id`: 会话 ID
/// - `message_seq`: 消息序号（该会话中的第几轮，从 1 开始）
/// - `user_msg`: 用户消息
/// - `assistant_reply`: 助手回复
///
/// # 返回
/// JSON 对象：`{ "pending": N, "flushed": bool, "extracted": M }`
/// - `pending`: push 后 buffer 中的待处理轮次
/// - `flushed`: 是否触发了 flush
/// - `extracted`: flush 提取的记忆数（未 flush 时为 0）
#[tauri::command]
pub async fn push_burst_turn(
    conversation_id: String,
    message_seq: usize,
    user_msg: String,
    assistant_reply: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    push_burst_turn_inner(
        &conversation_id,
        message_seq,
        &user_msg,
        &assistant_reply,
        state.inner(),
    )
    .await
}

/// Burst Buffer push 逻辑（命令与集成测试复用）。
pub async fn push_burst_turn_inner(
    conversation_id: &str,
    message_seq: usize,
    user_msg: &str,
    assistant_reply: &str,
    state: &AppState,
) -> Result<serde_json::Value, String> {
    use echomind_models::ProvenanceTag;

    let provenance = ProvenanceTag::new(
        conversation_id.to_string(),
        message_seq,
        format!("对话：{conversation_id} 的第 {message_seq} 轮"),
    );

    let mut buf = state.memory_burst_buffer.lock().await;
    buf.push(
        user_msg.to_string(),
        assistant_reply.to_string(),
        provenance,
    );

    let pending = buf.pending_count();
    let should_flush = buf.should_flush();

    let extracted = if should_flush {
        // 初始化 LLM Provider
        let llm_config = state.llm_config().read().await.clone();
        let provider = match llm_config {
            Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
                Ok(p) => p,
                Err(_) => {
                    // LLM 初始化失败，不阻塞 push，返回未 flush 状态
                    return Ok(serde_json::json!({
                        "pending": pending,
                        "flushed": false,
                        "extracted": 0,
                        "error": "LLM 初始化失败，跳过 flush"
                    }));
                }
            },
            None => {
                // 未配置 LLM，不阻塞 push
                return Ok(serde_json::json!({
                    "pending": pending,
                    "flushed": false,
                    "extracted": 0,
                    "error": "未配置 LLM"
                }));
            }
        };

        let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
        buf.flush(&memory_store, &provider)
            .await
            .unwrap_or_default()
    } else {
        0
    };

    Ok(serde_json::json!({
        "pending": if should_flush { 0 } else { pending },
        "flushed": should_flush,
        "extracted": extracted
    }))
}

/// 手动触发 Burst Buffer flush（Q02）。
///
/// 将 buffer 中所有 pending 轮次聚合后调用 LLM 提取记忆，写入 scratch 层。
/// 如果 buffer 为空或 LLM 未配置，返回 `extracted: 0`。
///
/// # 返回
/// JSON 对象：`{ "extracted": M, "pending_before": N }`
#[tauri::command]
pub async fn flush_memory_burst_buffer(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    flush_memory_burst_buffer_inner(state.inner()).await
}

/// Burst Buffer flush 逻辑（命令与集成测试复用）。
pub async fn flush_memory_burst_buffer_inner(
    state: &AppState,
) -> Result<serde_json::Value, String> {
    let mut buf = state.memory_burst_buffer.lock().await;
    let pending_before = buf.pending_count();

    if pending_before == 0 {
        return Ok(serde_json::json!({
            "extracted": 0,
            "pending_before": 0
        }));
    }

    // 初始化 LLM Provider
    let llm_config = state.llm_config().read().await.clone();
    let provider = match llm_config {
        Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
            Ok(p) => p,
            Err(e) => {
                return Ok(serde_json::json!({
                    "extracted": 0,
                    "pending_before": pending_before,
                    "error": format!("LLM 初始化失败: {e:#}")
                }));
            }
        },
        None => {
            return Ok(serde_json::json!({
                "extracted": 0,
                "pending_before": pending_before,
                "error": "未配置 LLM"
            }));
        }
    };

    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    let extracted = match buf.flush(&memory_store, &provider).await {
        Ok(n) => n,
        Err(e) => {
            return Ok(serde_json::json!({
                "extracted": 0,
                "pending_before": pending_before,
                "error": format!("flush 失败: {e:#}")
            }));
        }
    };

    Ok(serde_json::json!({
        "extracted": extracted,
        "pending_before": pending_before
    }))
}

/// 查询 Burst Buffer 状态（Q02）。
///
/// 返回 buffer 中的 pending 轮次数和是否满足 flush 条件。
///
/// # 返回
/// JSON 对象：`{ "pending": N, "should_flush": bool }`
#[tauri::command]
pub async fn get_burst_buffer_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    get_burst_buffer_status_inner(state.inner()).await
}

/// Burst Buffer 状态查询逻辑（命令与集成测试复用）。
pub async fn get_burst_buffer_status_inner(state: &AppState) -> Result<serde_json::Value, String> {
    let buf = state.memory_burst_buffer.lock().await;
    Ok(serde_json::json!({
        "pending": buf.pending_count(),
        "should_flush": buf.should_flush()
    }))
}
