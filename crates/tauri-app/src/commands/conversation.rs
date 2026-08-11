//! conversation 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 新建会话，返回唯一 ID（占位标题「新会话」）。
#[tauri::command]
pub async fn create_conversation(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    create_conversation_inner(workspace_id, state.inner()).await
}

/// 新建会话逻辑（命令与集成测试复用）。
pub async fn create_conversation_inner(
    workspace_id: String,
    state: &AppState,
) -> Result<String, String> {
    let conversation = Conversation::new(workspace_id, PLACEHOLDER_TITLE.to_string());
    state
        .storage
        .create_conversation(&conversation)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(conversation.id)
}

/// 列出工作区会话（倒序）。
#[tauri::command]
pub async fn get_conversations(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Conversation>, String> {
    get_conversations_inner(&workspace_id, state.inner()).await
}

/// 会话列表逻辑（命令与集成测试复用）。
pub async fn get_conversations_inner(
    workspace_id: &str,
    state: &AppState,
) -> Result<Vec<Conversation>, String> {
    state
        .storage
        .list_conversations(workspace_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 分页加载工作区会话（按创建时间倒序，REQ-NAV-007 会话列表分页）。
///
/// `limit` 为每页条数，`offset` 为偏移量。
/// 返回 `PaginatedResult<Conversation>`（含 `items` 与 `total`）。
#[tauri::command]
pub async fn get_conversations_paginated(
    workspace_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, AppState>,
) -> Result<PaginatedResult<Conversation>, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    get_conversations_paginated_inner(&workspace_id, limit, offset, state.inner()).await
}

/// 分页会话列表逻辑（命令与集成测试复用）。
pub async fn get_conversations_paginated_inner(
    workspace_id: &str,
    limit: usize,
    offset: usize,
    state: &AppState,
) -> Result<PaginatedResult<Conversation>, String> {
    let total = state
        .storage
        .count_conversations(workspace_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let items = state
        .storage
        .list_conversations_paginated(workspace_id, limit, offset)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(PaginatedResult { items, total })
}

/// 列出会话消息（正序，含引用块）。
#[tauri::command]
pub async fn get_messages(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    get_messages_inner(&conversation_id, state.inner()).await
}

/// 消息列表逻辑（命令与集成测试复用）。
pub async fn get_messages_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<Vec<ChatMessage>, String> {
    state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 编辑用户消息并创建新版本（DB 持久化）。
///
/// 当用户编辑一条已发送的问题时，在同一个 `turn_group` 下创建新 `version`，
/// 旧版本保留在 DB 中供分页查看。返回新版本号供前端使用。
///
/// - `conversation_id` — 会话 ID
/// - `turn_group` — 轮次分组 ID（同一问题的不同编辑版本共享）
/// - `new_content` — 编辑后的用户消息文本
///
/// 返回新版本号（旧版本数 + 1）。
#[tauri::command]
pub async fn edit_user_message(
    conversation_id: String,
    turn_group: String,
    new_content: String,
    original_message_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<i32, String> {
    edit_user_message_inner_full(
        &conversation_id,
        &turn_group,
        &new_content,
        original_message_id.as_deref(),
        state.inner(),
    )
    .await
}

/// 编辑用户消息逻辑（命令与集成测试复用）。
pub async fn edit_user_message_inner(
    conversation_id: &str,
    turn_group: &str,
    new_content: &str,
    state: &AppState,
) -> Result<i32, String> {
    edit_user_message_inner_full(conversation_id, turn_group, new_content, None, state).await
}

/// 完整版编辑逻辑（含首次编辑升级）。
///
/// `original_message_id` 为原始 user 消息行 id（前端从 `get_messages` 返回的
/// `ChatMessage.id` 取得）：当 turn_group 无历史（首次编辑）时，先把原始问答
/// 行升级为 version=1，新内容作为 version=2，并持久化 active_version=2，
/// 保证重启后保持「编辑后最新版本」的查看状态（REQ-QA 首次编辑分页）。
pub async fn edit_user_message_inner_full(
    conversation_id: &str,
    turn_group: &str,
    new_content: &str,
    original_message_id: Option<&str>,
    state: &AppState,
) -> Result<i32, String> {
    // 查询当前 turn_group 下的最大版本号
    let messages = state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    let max_version = messages
        .iter()
        .filter(|m| m.turn_group.as_deref() == Some(turn_group))
        .filter_map(|m| m.version)
        .max()
        .unwrap_or(0);

    // 首次编辑（turn_group 无历史）：把原始问答升级为 version=1
    let mut new_version = max_version + 1;
    if max_version == 0
        && let Some(original_id) = original_message_id
    {
        state
            .storage
            .promote_original_turn(conversation_id, original_id, turn_group)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;
        // 原始问答升级为 v1 后，新内容成为 v2
        new_version = 2;
    }

    // 持久化新版本的用户消息（assistant 消息由后续 chat 流完成后持久化）
    let user_msg = ChatMessage {
        id: None,
        role: "user".to_string(),
        content: new_content.to_string(),
        sources: None,
        reasoning: None,
        turn_group: Some(turn_group.to_string()),
        version: Some(new_version),
    };
    state
        .storage
        .add_message(conversation_id, &user_msg)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 持久化活跃版本：编辑后默认查看最新版本（重启保持，REQ-QA 持久化）
    state
        .storage
        .set_turn_active_version(conversation_id, turn_group, new_version)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    Ok(new_version)
}

/// 分页加载会话消息（从最新消息向前加载，REQ-RAG-007 消息懒加载）。
///
/// `limit` 为每页条数，`offset` 为从最新消息向前的偏移量：
/// - `offset=0` 返回最近 `limit` 条消息
/// - `offset=N` 返回倒数第 N+1 到 N+limit 条消息
#[tauri::command]
pub async fn get_messages_paginated(
    conversation_id: String,
    limit: Option<usize>,
    offset: Option<usize>,
    state: State<'_, AppState>,
) -> Result<PaginatedResult<ChatMessage>, String> {
    let limit = limit.unwrap_or(30);
    let offset = offset.unwrap_or(0);
    get_messages_paginated_inner(&conversation_id, limit, offset, state.inner()).await
}

/// 分页消息逻辑（命令与集成测试复用）。
pub async fn get_messages_paginated_inner(
    conversation_id: &str,
    limit: usize,
    offset: usize,
    state: &AppState,
) -> Result<PaginatedResult<ChatMessage>, String> {
    let total = state
        .storage
        .count_messages(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    let items = state
        .storage
        .list_messages_paginated(conversation_id, limit, offset)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(PaginatedResult { items, total })
}

/// 设置轮次的活跃版本号（分支切换状态持久化）。
///
/// 用户在分页器中切换查看不同编辑版本时，活跃版本号被持久化到 DB，
/// 下次加载会话时恢复到最后一次查看的版本。
#[tauri::command]
pub async fn set_turn_active_version(
    conversation_id: String,
    turn_group: String,
    active_version: i32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .storage
        .set_turn_active_version(&conversation_id, &turn_group, active_version)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))
}

/// 获取会话中所有轮次的活跃版本号。
#[tauri::command]
pub async fn get_turn_active_versions(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TurnActiveVersion>, String> {
    state
        .storage
        .get_turn_active_versions(&conversation_id)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))
}

/// 删除会话（级联清理消息与中断令牌）。
#[tauri::command]
pub async fn delete_conversation(id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_conversation_inner(&id, state.inner()).await
}

/// 删除会话逻辑（命令与集成测试复用）。
pub async fn delete_conversation_inner(id: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .delete_conversation(id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state.clear_abort(id).await;
    Ok(())
}

/// 重命名会话（REQ-IX-001：右键菜单「重命名」功能）。
#[tauri::command]
pub async fn rename_conversation(
    id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    rename_conversation_inner(&id, &title, state.inner()).await
}

/// 重命名会话逻辑（命令与集成测试复用）。
pub async fn rename_conversation_inner(
    id: &str,
    title: &str,
    state: &AppState,
) -> Result<(), String> {
    state
        .storage
        .update_conversation_title(id, title)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 停止生成：触发指定会话的中断令牌，流循环立即 break 并中止网络拉取。
///
/// 本地 LLM 模式（Pro）下额外调用 `LocalLlmEngine::abort()`，使 mistral.rs
/// 推理流内部 `spawn` 任务的 `select!` 循环立即退出（S13 流取消集成）。
#[tauri::command]
pub async fn abort_chat(conversation_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.abort_chat(&conversation_id).await;

    // S13：本地模式下同时取消 LocalLlmEngine 内部推理。
    // forward_stream 的 abort_token 只中断 channel 消费侧（select! 循环），
    // 但 mistral.rs 的 spawn 任务仍在生成 token。调用 engine.abort() 触发
    // 引擎内部 CancellationToken，使 spawn 的 select! 立即 break，停止推理。
    #[cfg(feature = "pro")]
    {
        let llm_mode = state.get_llm_mode().await;
        if llm_mode == LlmMode::Local {
            // 引擎可能尚未初始化（用户未配置本地模型），静默忽略
            if let Ok(engine) = state.local_llm().await {
                engine.abort().await;
            }
        }
    }

    Ok(())
}

/// 获取会话的分支树结构（REQ-RAG-039）。
///
/// 从 messages 表加载所有消息，按 `turn_group` 分组、按 `version` 排序，
/// 构建分支树节点。节点的父子关系通过版本号建立（version N 的父为 version N-1）。
/// 活跃路径由 `turn_active_versions` 表决定。
///
/// 复用现有 `turn_group` + `version` 列，无需新增数据库表。
#[tauri::command]
pub async fn get_conversation_tree(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationTree, String> {
    get_conversation_tree_inner(&conversation_id, state.inner()).await
}

/// 分支树构建逻辑（命令与集成测试复用）。
pub async fn get_conversation_tree_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<ConversationTree, String> {
    // 1. 加载会话所有消息
    let messages = state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 2. 加载活跃版本
    let active_versions = state
        .storage
        .get_turn_active_versions(conversation_id)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 3. 按 turn_group 分组，构建节点
    use std::collections::HashMap;
    let mut group_messages: HashMap<String, Vec<&ChatMessage>> = HashMap::new();
    let mut group_order: Vec<String> = Vec::new();

    for msg in &messages {
        let tg = msg.turn_group.clone().unwrap_or_default();
        if tg.is_empty() {
            continue;
        }
        if !group_messages.contains_key(&tg) {
            group_order.push(tg.clone());
        }
        group_messages.entry(tg).or_default().push(msg);
    }

    let mut tree = ConversationTree::empty(conversation_id);
    let mut node_index: HashMap<String, ConversationTreeNode> = HashMap::new();

    // 4. 为每个 turn_group 的每个 version 创建节点
    for tg in &group_order {
        let empty: Vec<&ChatMessage> = Vec::new();
        let msgs = group_messages.get(tg).unwrap_or(&empty);
        // 按 version 排序
        let mut sorted_msgs: Vec<&ChatMessage> = msgs.to_vec();
        sorted_msgs.sort_by_key(|m| m.version.unwrap_or(1));

        // 取 user 消息作为预览
        for msg in &sorted_msgs {
            if msg.role != "user" {
                continue;
            }
            let version = msg.version.unwrap_or(1);
            let node = ConversationTreeNode::new(
                conversation_id,
                tg,
                version,
                0, // created_at 由消息行的时间戳决定，此处简化
                &msg.content,
            );
            node_index.insert(node.node_id.clone(), node);
        }
    }

    // 5. 建立父子关系：同一 turn_group 内，version N 的父为 version N-1
    for tg in &group_order {
        let empty: Vec<&ChatMessage> = Vec::new();
        let msgs = group_messages.get(tg).unwrap_or(&empty);
        let versions: Vec<i32> = {
            let mut v: Vec<i32> = msgs
                .iter()
                .filter(|m| m.role == "user")
                .filter_map(|m| m.version)
                .collect();
            v.sort();
            v.dedup();
            v
        };

        for i in 1..versions.len() {
            let child_id = format!("{}#{}", tg, versions[i]);
            let parent_id = format!("{}#{}", tg, versions[i - 1]);
            if let Some(child) = node_index.get_mut(&child_id) {
                child.parent_message_id = Some(parent_id.clone());
            }
            if let Some(parent) = node_index.get_mut(&parent_id) {
                parent.child_message_ids.push(child_id.clone());
            }
        }
    }

    // 6. 设置活跃子节点（根据 turn_active_versions）
    for av in &active_versions {
        let active_node_id = format!("{}#{}", av.turn_group, av.active_version);
        // 找到活跃节点的父 ID（先 clone 出来，避免借用冲突）
        let parent_id_opt = node_index
            .get(&active_node_id)
            .and_then(|n| n.parent_message_id.clone());
        if let Some(parent_id) = parent_id_opt
            && let Some(parent) = node_index.get_mut(&parent_id)
        {
            parent.active_child = Some(active_node_id.clone());
        }
    }

    // 7. 收集所有节点，确定根节点（无 parent 的节点）
    let mut all_nodes: Vec<ConversationTreeNode> = node_index.into_values().collect();
    all_nodes.sort_by(|a, b| {
        a.turn_group
            .cmp(&b.turn_group)
            .then_with(|| a.version.cmp(&b.version))
    });

    let root_ids: Vec<String> = all_nodes
        .iter()
        .filter(|n| n.is_root())
        .map(|n| n.node_id.clone())
        .collect();

    // 8. 构建活跃路径：从每个根节点沿 active_child 向下遍历
    let mut active_path = Vec::new();
    for root_id in &root_ids {
        let mut current = Some(root_id.clone());
        while let Some(id) = current {
            active_path.push(id.clone());
            current = all_nodes
                .iter()
                .find(|n| n.node_id == id)
                .and_then(|n| n.active_child.clone());
        }
    }

    tree.nodes = all_nodes;
    tree.root_ids = root_ids;
    tree.active_path = active_path;

    Ok(tree)
}

/// 从指定消息创建新分支（REQ-RAG-039）。
///
/// 在指定消息所属的 `turn_group` 下创建新版本，新版本的父节点为指定消息。
/// 这是对 `edit_user_message` 的分支语义封装：复用相同的 DB 持久化逻辑
/// （`promote_original_turn` + `add_message` + `set_turn_active_version`），
/// 但以「分支」而非「编辑」的语义暴露给前端。
///
/// - `conversation_id` — 会话 ID
/// - `message_id` — 要从其分叉的消息 ID（必须为 user 消息）
/// - `new_content` — 新分支的用户消息内容
///
/// 返回新版本号和 turn_group。
#[tauri::command]
pub async fn branch_from_message(
    conversation_id: String,
    message_id: String,
    new_content: String,
    state: State<'_, AppState>,
) -> Result<BranchResult, String> {
    branch_from_message_inner(&conversation_id, &message_id, &new_content, state.inner()).await
}

/// 分支创建逻辑（命令与集成测试复用）。
///
/// 复用 `edit_user_message_inner_full` 的逻辑，但传入 `message_id` 作为
/// `original_message_id`，确保首次编辑时原始消息被升级为 version=1。
pub async fn branch_from_message_inner(
    conversation_id: &str,
    message_id: &str,
    new_content: &str,
    state: &AppState,
) -> Result<BranchResult, String> {
    // 1. 加载消息，找到所属 turn_group
    let messages = state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 找到目标消息
    let target_msg = messages
        .iter()
        .find(|m| m.id.as_deref() == Some(message_id))
        .ok_or_else(|| prefix_error(ERR_VALIDATION, "消息不存在"))?;

    if target_msg.role != "user" {
        return Err(prefix_error(ERR_VALIDATION, "只能从用户消息创建分支"));
    }

    let turn_group = target_msg
        .turn_group
        .clone()
        .unwrap_or_else(generate_turn_group_id);

    // 2. 复用 edit_user_message_inner_full 的逻辑
    let new_version = edit_user_message_inner_full(
        conversation_id,
        &turn_group,
        new_content,
        Some(message_id),
        state,
    )
    .await?;

    Ok(BranchResult {
        new_version,
        turn_group,
    })
}

/// 生成 turn_group ID（与前端 generateTurnGroupId 一致的格式）。
fn generate_turn_group_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("turn-{ts}")
}

// ============================================================================
// 对话全文搜索（REQ-RAG-040, S62）
// ============================================================================

/// 对话全文搜索 IPC 命令（REQ-RAG-040）。
///
/// 使用 FTS5 全文索引搜索所有会话中的消息内容，返回按 BM25 分数降序排列的搜索结果。
/// 用户可在侧栏搜索弹框中切换到「对话」模式搜索消息内容。
///
/// # 参数
/// - `query`: 搜索关键词
/// - `limit`: 返回结果数量上限（默认 50）
///
/// # 返回
/// `Vec<MessageSearchResult>` — 包含匹配消息及其所属会话信息。
#[tauri::command]
pub async fn search_conversations(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<MessageSearchResult>, String> {
    let limit = limit.unwrap_or(50);
    search_conversations_inner(&query, limit, state.inner()).await
}

/// 对话全文搜索逻辑（命令与集成测试复用）。
pub async fn search_conversations_inner(
    query: &str,
    limit: usize,
    state: &AppState,
) -> Result<Vec<MessageSearchResult>, String> {
    state
        .storage
        .search_messages(query, limit)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// Session Strip：按索引范围移除消息（REQ-RAG-046）。
#[tauri::command]
pub async fn strip_messages(
    conversation_id: String,
    from_index: usize,
    to_index: usize,
    replace_with_summary: bool,
    summary_text: Option<String>,
    state: State<'_, AppState>,
) -> Result<StripResult, String> {
    strip_messages_inner(
        &conversation_id,
        from_index,
        to_index,
        replace_with_summary,
        summary_text,
        state.inner(),
    )
    .await
}

/// Session Strip 逻辑（命令与集成测试复用）。
pub async fn strip_messages_inner(
    conversation_id: &str,
    from_index: usize,
    to_index: usize,
    replace_with_summary: bool,
    summary_text: Option<String>,
    state: &AppState,
) -> Result<StripResult, String> {
    let config = StripConfig {
        from_index,
        to_index,
        replace_with_summary,
        summary_text,
    };
    SessionStripper::strip_range(&state.storage, conversation_id, &config)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// Session Strip：保留最后 N 条消息（REQ-RAG-046）。
#[tauri::command]
pub async fn strip_keeping_recent(
    conversation_id: String,
    keep_last_n: usize,
    replace_with_summary: bool,
    summary_text: Option<String>,
    state: State<'_, AppState>,
) -> Result<StripResult, String> {
    strip_keeping_recent_inner(
        &conversation_id,
        keep_last_n,
        replace_with_summary,
        summary_text,
        state.inner(),
    )
    .await
}

/// Session Strip 保留最近逻辑（命令与集成测试复用）。
pub async fn strip_keeping_recent_inner(
    conversation_id: &str,
    keep_last_n: usize,
    replace_with_summary: bool,
    summary_text: Option<String>,
    state: &AppState,
) -> Result<StripResult, String> {
    SessionStripper::strip_keeping_recent(
        &state.storage,
        conversation_id,
        keep_last_n,
        replace_with_summary,
        summary_text.as_deref(),
    )
    .await
    .map_err(|e| format!("{e:#}"))
}

/// Session Strip：预览将被移除的消息（REQ-RAG-046）。
#[tauri::command]
pub async fn preview_strip(
    conversation_id: String,
    from_index: usize,
    to_index: usize,
    state: State<'_, AppState>,
) -> Result<StripPreview, String> {
    preview_strip_inner(&conversation_id, from_index, to_index, state.inner()).await
}

/// Session Strip 预览逻辑（命令与集成测试复用）。
pub async fn preview_strip_inner(
    conversation_id: &str,
    from_index: usize,
    to_index: usize,
    state: &AppState,
) -> Result<StripPreview, String> {
    SessionStripper::preview(&state.storage, conversation_id, from_index, to_index)
        .await
        .map_err(|e| format!("{e:#}"))
}

// ------------------------------------------------------------------
// 单条消息删除（REQ-RAG-013）
// ------------------------------------------------------------------

/// 删除单条消息（REQ-RAG-013）。
///
/// 若 `message_id` 为 user 消息，则连带删除下一条 assistant 消息（保持问答对完整性）。
/// 若为 assistant 消息，仅删除该条。
#[tauri::command]
pub async fn delete_message(
    conversation_id: String,
    message_id: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    delete_message_inner(&conversation_id, &message_id, state.inner()).await
}

/// `delete_message` 的逻辑实现（命令与集成测试复用）。
///
/// 返回实际删除的消息数（1 或 2）。
pub async fn delete_message_inner(
    conversation_id: &str,
    message_id: &str,
    state: &AppState,
) -> Result<usize, String> {
    // 获取会话全部消息，找到目标消息
    let messages = state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 找到目标消息索引
    let target_idx = messages
        .iter()
        .position(|m| m.id.as_deref() == Some(message_id))
        .ok_or_else(|| format!("消息不存在: {message_id}"))?;

    let target = &messages[target_idx];
    let mut ids_to_delete = vec![message_id.to_string()];

    // 若为 user 消息，查找下一条 assistant 消息一并删除
    if target.role == "user"
        && let Some(next) = messages.get(target_idx + 1)
        && next.role == "assistant"
        && let Some(ref next_id) = next.id
    {
        ids_to_delete.push(next_id.clone());
    }

    let count = ids_to_delete.len();
    state
        .storage
        .delete_messages_by_ids(conversation_id, &ids_to_delete)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(count)
}
