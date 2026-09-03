#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Conversation 相关集成测试 — 会话导出/Token 用量/成本追踪/协调模式/编辑版本持久化。

use super::common::*;
use super::*;

// ============================================================================
// 导出功能集成测试（REQ-EXP-001）
// ============================================================================

/// TC-IPC-EXP-001：导出对话为 Markdown——基本格式（REQ-EXP-001）。
#[tokio::test]
async fn tc_ipc_exp_001_export_conversation_markdown() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 添加消息
    let user_msg = ChatMessage {
        id: None,
        role: "user".to_string(),
        content: "什么是 Rust？".to_string(),
        sources: None,
        reasoning: None,
        ..Default::default()
    };
    state
        .storage
        .add_message(&conv_id, &user_msg)
        .await
        .unwrap();

    let assistant_msg = ChatMessage {
        id: None,
        role: "assistant".to_string(),
        content: "Rust 是一门系统编程语言。".to_string(),
        sources: None,
        reasoning: Some("先分析 Rust 的核心特性，再组织答案。".to_string()),
        ..Default::default()
    };
    state
        .storage
        .add_message(&conv_id, &assistant_msg)
        .await
        .unwrap();

    // 推理思考过程（reasoning）落库往返验证：历史消息加载可重现思考过程
    let loaded = state.storage.list_messages(&conv_id).await.unwrap();
    let loaded_assistant = loaded
        .iter()
        .find(|m| m.role == "assistant")
        .expect("应加载到助手消息");
    assert_eq!(
        loaded_assistant.reasoning.as_deref(),
        Some("先分析 Rust 的核心特性，再组织答案。"),
        "reasoning 应持久化并可读回（历史消息重现思考过程）"
    );

    let (md, _filename) = export_conversation_markdown_inner(&conv_id, &state)
        .await
        .unwrap();

    assert!(md.contains("什么是 Rust"), "导出应包含用户消息");
    assert!(md.contains("Rust 是一门系统编程语言"), "导出应包含助手消息");
}

/// TC-IPC-EXP-002：save_text_file 保存文件（REQ-EXP-001）。
#[tokio::test]
async fn tc_ipc_exp_002_save_text_file() {
    let dir = TempDir::new().unwrap();
    let test_file = dir.path().join("test-export.md");
    let test_content = "# Test Export\n\nThis is test content.";

    save_text_file_inner(test_file.to_str().unwrap(), test_content)
        .await
        .unwrap();

    let read = std::fs::read_to_string(&test_file).unwrap();
    assert_eq!(read, test_content, "保存的文件内容应匹配");
}

// ============================================================================
// REQ-RAG-017 对话上下文长度管理 集成测试
// ============================================================================

// =============================================================
// Token 用量追踪集成测试（Session 27）
// =============================================================

/// TC-TOKEN-001：record_token_usage 正确累加 token 用量。
#[tokio::test]
async fn tc_token_001_record_usage_accumulates() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 初始状态：无用量记录
    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.total_prompt_tokens, 0);
    assert_eq!(cost.total_completion_tokens, 0);
    assert_eq!(cost.exchange_count, 0);

    // 记录第一次用量
    let usage1 = TokenUsage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
    };
    record_token_usage_inner(&state, &Some(usage1)).await;

    // 验证累加
    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.total_prompt_tokens, 100, "第一次 prompt 累加");
    assert_eq!(cost.total_completion_tokens, 50, "第一次 completion 累加");
    assert_eq!(cost.total_tokens, 150, "第一次 total 累加");
    assert_eq!(cost.exchange_count, 1, "exchange_count 应为 1");

    // 记录第二次用量
    let usage2 = TokenUsage {
        prompt_tokens: 200,
        completion_tokens: 100,
        total_tokens: 300,
    };
    record_token_usage_inner(&state, &Some(usage2)).await;

    // 验证再次累加
    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.total_prompt_tokens, 300, "第二次 prompt 累加 100+200");
    assert_eq!(
        cost.total_completion_tokens, 150,
        "第二次 completion 累加 50+100"
    );
    assert_eq!(cost.total_tokens, 450, "第二次 total 累加 150+300");
    assert_eq!(cost.exchange_count, 2, "exchange_count 应为 2");
}

/// TC-TOKEN-002：None 用量不增加计数器（本地推理模式）。
#[tokio::test]
async fn tc_token_002_none_usage_no_change() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 记录一次 None（本地推理模式无 usage 数据）
    record_token_usage_inner(&state, &None).await;

    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.total_prompt_tokens, 0, "None 不应累加");
    assert_eq!(cost.exchange_count, 0, "None 不应增加 exchange_count");
}

// =============================================================
// 成本追踪 UI 集成测试（Session 30）
// =============================================================

/// TC-COST-001：set_token_budget 持久化到 settings 表。
#[tokio::test]
async fn tc_cost_001_token_budget_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 设置预算
    set_token_budget_inner(100000, &state).await.unwrap();

    // 验证持久化
    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.token_budget, 100000, "预算应持久化");

    // 验证通过 get_settings 也能读到
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(settings.token_budget, 100000, "get_settings 应返回预算");

    // 设置为 0（不限制）
    set_token_budget_inner(0, &state).await.unwrap();
    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.token_budget, 0, "预算 0 = 不限制");
}

/// TC-COST-002：get_conversation_cost 返回正确累计。
#[tokio::test]
async fn tc_cost_002_cost_reflects_usage() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 设置预算
    set_token_budget_inner(50000, &state).await.unwrap();

    // 记录多次用量
    for _ in 0..3 {
        record_token_usage_inner(
            &state,
            &Some(TokenUsage {
                prompt_tokens: 1000,
                completion_tokens: 500,
                total_tokens: 1500,
            }),
        )
        .await;
    }

    let cost = get_conversation_cost_inner(&state).await.unwrap();
    assert_eq!(cost.total_prompt_tokens, 3000, "3次 × 1000");
    assert_eq!(cost.total_completion_tokens, 1500, "3次 × 500");
    assert_eq!(cost.total_tokens, 4500, "3次 × 1500");
    assert_eq!(cost.exchange_count, 3, "3次对话");
    assert_eq!(cost.token_budget, 50000, "预算不变");
}

// ================== 编辑版本持久化测试（AC-QA-006 DB 持久化） ==================

/// TC-EDIT-DB-001: edit_user_message 创建新版本并持久化到 DB。
///
/// 验证：
/// 1. 首次编辑创建 version=2
/// 2. 二次编辑创建 version=3
/// 3. list_messages 返回所有版本的消息
/// 4. 旧版本消息保留在 DB 中
#[tokio::test]
async fn tc_edit_db_001_edit_creates_new_version() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 初始问答（无 turn_group，视为无版本管理）
    persist_exchange(
        &state,
        &conv_id,
        "原始问题",
        "原始回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 编辑：创建 turn_group + version=1（首次编辑该 turn_group）
    let turn_group = "turn-test-001";
    let v1 = edit_user_message_inner(&conv_id, turn_group, "编辑后的问题v1", &state)
        .await
        .unwrap();
    assert_eq!(v1, 1, "首次编辑应返回 version=1");

    // 再次编辑：version=2
    let v2 = edit_user_message_inner(&conv_id, turn_group, "编辑后的问题v2", &state)
        .await
        .unwrap();
    assert_eq!(v2, 2, "二次编辑应返回 version=2");

    // 验证 DB 中有所有版本的消息
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    // 初始 2 条 + v1 user + v2 user = 4 条
    assert_eq!(messages.len(), 4, "应有 4 条消息（初始2 + 编辑2）");

    // 验证编辑版本有正确的 turn_group 和 version
    let edited_msgs: Vec<_> = messages
        .iter()
        .filter(|m| m.turn_group.as_deref() == Some(turn_group))
        .collect();
    assert_eq!(edited_msgs.len(), 2, "应有 2 条带 turn_group 的消息");
    assert_eq!(edited_msgs[0].version, Some(1));
    assert_eq!(edited_msgs[0].content, "编辑后的问题v1");
    assert_eq!(edited_msgs[1].version, Some(2));
    assert_eq!(edited_msgs[1].content, "编辑后的问题v2");
}

/// TC-EDIT-DB-002: 无 turn_group 的旧消息保持兼容性。
///
/// 验证旧消息（无 turn_group）的 turn_group 和 version 字段为 None。
#[tokio::test]
async fn tc_edit_db_002_legacy_messages_have_no_turn_group() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    persist_exchange(
        &state,
        &conv_id,
        "普通问题",
        "普通回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(messages.len(), 2);
    // 旧消息不应有 turn_group / version
    for msg in &messages {
        assert!(msg.turn_group.is_none(), "旧消息不应有 turn_group");
        assert!(msg.version.is_none(), "旧消息不应有 version");
    }
}

/// TC-EDIT-DB-004: 首次编辑（无 turn_group 的原始问答）产生 v1+v2 版本组。
///
/// 验证（REQ-QA 首次编辑分页）：
/// 1. 原始 user/assistant 行原地升级为 turn_group + version=1
/// 2. 新内容作为 version=2 追加
/// 3. active_version 持久化为 2（重启后保持编辑后的查看状态）
/// 4. 从 get_messages 可拿到消息行 id（供前端定位原始行）
#[tokio::test]
async fn tc_edit_db_004_first_edit_promotes_original_to_v1() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 原始问答（无 turn_group）
    persist_exchange(
        &state,
        &conv_id,
        "原始问题",
        "原始回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 找到原始 user 消息行 id
    let before = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(before.len(), 2);
    let original_user_id = before
        .iter()
        .find(|m| m.role == "user")
        .and_then(|m| m.id.clone())
        .expect("get_messages 应返回消息行 id");

    // 首次编辑：传 original_message_id → 原始行升级 v1，新内容 v2
    let turn_group = "turn-first-edit";
    let v = edit_user_message_inner_full(
        &conv_id,
        turn_group,
        "编辑后的问题",
        Some(&original_user_id),
        &state,
    )
    .await
    .unwrap();
    assert_eq!(v, 2, "首次编辑应返回 version=2");

    // 验证 DB 状态：原始行升级为 v1，新内容为 v2
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    let turn_msgs: Vec<_> = messages
        .iter()
        .filter(|m| m.turn_group.as_deref() == Some(turn_group))
        .collect();
    assert_eq!(
        turn_msgs.len(),
        3,
        "应有 3 条 turn_group 消息（user v1 + assistant v1 + user v2）"
    );

    let user_v1 = turn_msgs
        .iter()
        .find(|m| m.role == "user" && m.version == Some(1))
        .expect("应存在 user v1");
    assert_eq!(user_v1.content, "原始问题", "user v1 应为原始问题");
    let assistant_v1 = turn_msgs
        .iter()
        .find(|m| m.role == "assistant" && m.version == Some(1))
        .expect("应存在 assistant v1");
    assert_eq!(
        assistant_v1.content, "原始回答",
        "assistant v1 应为原始回答"
    );
    let user_v2 = turn_msgs
        .iter()
        .find(|m| m.role == "user" && m.version == Some(2))
        .expect("应存在 user v2");
    assert_eq!(user_v2.content, "编辑后的问题", "user v2 应为编辑后问题");

    // 原始行 id 已被升级（同一行），不再是无 turn_group 的消息
    assert_eq!(
        messages.iter().filter(|m| m.turn_group.is_none()).count(),
        0,
        "原始行升级后不应再有未分组消息"
    );

    // 验证 active_version 持久化为 2
    let active = state
        .storage
        .get_turn_active_versions(&conv_id)
        .await
        .unwrap();
    let entry = active
        .iter()
        .find(|a| a.turn_group == turn_group)
        .expect("应存在 active 记录");
    assert_eq!(entry.active_version, 2, "编辑后 active 应为 2");
}

/// TC-EDIT-DB-005: 首次编辑时原始消息 id 无效 → 返回错误且不产生部分数据。
#[tokio::test]
async fn tc_edit_db_005_first_edit_invalid_original_id_errors() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    persist_exchange(
        &state,
        &conv_id,
        "原始问题",
        "原始回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let result = edit_user_message_inner_full(
        &conv_id,
        "turn-bad-id",
        "编辑后的问题",
        Some("不存在的消息id"),
        &state,
    )
    .await;
    assert!(result.is_err(), "无效原始 id 应返回错误");

    // 不产生任何 turn_group 数据（事务回滚）
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    assert_eq!(
        messages.iter().filter(|m| m.turn_group.is_some()).count(),
        0,
        "失败后不应产生 turn_group 消息"
    );
}

/// TC-EDIT-DB-006: 多问答对场景下首次编辑的边界。
///
/// 验证（REQ-RAG-026 边界）：
/// 1. 被编辑 user 行无回答（后一问已有回答）时，不误升级下一问答对的 assistant
/// 2. promote_original_turn 幂等：重复调用同一 turn_group 不报错（重试自愈）
#[tokio::test]
async fn tc_edit_db_006_first_edit_boundaries() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 两个问答对（均无 turn_group）：Q1/A1, Q2/A2
    persist_exchange(&state, &conv_id, "问题一", "回答一", None, None, None, None)
        .await
        .unwrap();
    persist_exchange(&state, &conv_id, "问题二", "回答二", None, None, None, None)
        .await
        .unwrap();

    let before = get_messages_inner(&conv_id, &state).await.unwrap();
    let q2_id = before
        .iter()
        .find(|m| m.role == "user" && m.content == "问题二")
        .and_then(|m| m.id.clone())
        .expect("应找到问题二行 id");

    // 编辑问题二：只应升级 Q2+A2，不应触碰 Q1/A1
    let turn_group = "turn-boundary";
    let v =
        edit_user_message_inner_full(&conv_id, turn_group, "编辑后的问题二", Some(&q2_id), &state)
            .await
            .unwrap();
    assert_eq!(v, 2);

    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    // turn_group 应恰好 3 条：Q2(v1) + A2(v1) + 新问题(v2)
    let turn_msgs: Vec<_> = messages
        .iter()
        .filter(|m| m.turn_group.as_deref() == Some(turn_group))
        .collect();
    assert_eq!(turn_msgs.len(), 3, "turn_group 应有 3 条消息");
    // Q1/A1 保持无 turn_group
    let q1 = messages.iter().find(|m| m.content == "问题一").unwrap();
    let a1 = messages.iter().find(|m| m.content == "回答一").unwrap();
    assert!(q1.turn_group.is_none(), "Q1 不应被升级");
    assert!(a1.turn_group.is_none(), "A1 不应被升级");

    // 幂等：同一 turn_group 重复 promote 不报错（模拟 add_message 半失败后的重试）
    state
        .storage
        .promote_original_turn(&conv_id, &q2_id, turn_group)
        .await
        .expect("重复 promote 应幂等成功");
}

/// TC-EDIT-DB-003: DB 迁移后旧数据库的消息保持可用。
///
/// 验证迁移后旧消息可正常读取（turn_group 默认 ''，version 默认 1）。
#[tokio::test]
async fn tc_edit_db_003_migration_preserves_old_messages() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 写入消息（无 turn_group）
    persist_exchange(
        &state,
        &conv_id,
        "迁移前的问题",
        "迁移前的回答",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 重新初始化（模拟重启，触发迁移）
    let state2 = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 验证消息仍然可读
    let messages = get_messages_inner(&conv_id, &state2).await.unwrap();
    assert_eq!(messages.len(), 2, "迁移后消息数量应不变");
    assert_eq!(messages[0].content, "迁移前的问题");
    assert_eq!(messages[1].content, "迁移前的回答");
}
