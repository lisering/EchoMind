#![allow(clippy::unwrap_used)]
//! QA 红灯集成测试（L2：Tauri Mock + 真实 SQLite，体系二阶段 2 先于实现编写）。
//! 覆盖：TC-UI-001 配置持久化（REQ-UI-008）/ TC-UI-002 空知识库拦截（REQ-RAG-003-AC-2）。

use echomind_models::LlmConfig;
use echomind_tauri_app::commands::{
    chat_inner, create_conversation_inner, delete_conversation_inner, forward_stream,
    get_conversations_inner, get_messages_inner, get_settings_inner, persist_exchange,
    update_llm_config_inner,
};
use echomind_tauri_app::state::AppState;
use futures::StreamExt;
use tempfile::TempDir;

fn test_config() -> LlmConfig {
    LlmConfig {
        api_key: "sk-test-1234567890abcdef".to_string(),
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-chat".to_string(),
    }
}

/// TC-UI-001 配置持久化：update_llm_config → 重建 AppState（模拟重启）→ get_settings 数据仍在（REQ-UI-008-AC-1/AC-3）。
#[tokio::test]
async fn tc_ui_001_llm_config_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        update_llm_config_inner(test_config(), &state)
            .await
            .unwrap();
    } // 状态与连接池销毁，模拟应用退出

    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();

    assert!(settings.has_llm_config, "重启后配置必须仍在");
    assert_eq!(settings.base_url, "https://api.deepseek.com");
    assert_eq!(settings.model, "deepseek-chat");
    assert!(
        settings.api_key_masked.contains("cdef"),
        "脱敏 Key 应保留末四位，实际: {}",
        settings.api_key_masked
    );
    assert!(
        !settings.api_key_masked.contains("sk-test-1234567890"),
        "脱敏 Key 不得包含完整明文前缀"
    );

    // 运行态配置亦已从 settings 表恢复（chat 无需重启即可用）
    let runtime_cfg = restarted.llm_config.read().await.clone();
    let runtime_cfg = runtime_cfg.unwrap_or_else(|| panic!("运行态配置必须已恢复"));
    assert_eq!(
        runtime_cfg.api_key, "sk-test-1234567890abcdef",
        "运行态应持有完整解密 Key"
    );
    assert_eq!(runtime_cfg.model, "deepseek-chat");
}

/// TC-UI-002 空知识库拦截：chat 返回明确错误提示（REQ-RAG-003-AC-2）。
#[tokio::test]
async fn tc_ui_002_empty_knowledge_base_chat_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    // 配置 LLM（排除「未配置」干扰项，单独验证空库分支）
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = chat_inner(&handle, "任意问题", &[], "conv-1", &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("知识库为空"),
        "空知识库必须返回引导导入的错误提示，实际: {err}"
    );
}

// ================== Phase 6：会话生命周期（REQ-RAG-005/006） ==================

/// TC-RAG-003 会话 CRUD：新建 → 列表 → 删除级联清理（REQ-RAG-006-AC-1）。
#[tokio::test]
async fn tc_rag_003_conversation_crud() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
    assert!(!id.is_empty(), "新建会话必须返回唯一 ID");

    let list = get_conversations_inner("default", &state).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].title, "新会话", "初始标题应为占位文案");

    // 写入一轮问答后删除，验证级联清理
    persist_exchange(&state, &id, "级联删除验证问题", "级联删除验证回答", None)
        .await
        .unwrap();
    delete_conversation_inner(&id, &state).await.unwrap();

    assert!(
        get_conversations_inner("default", &state)
            .await
            .unwrap()
            .is_empty(),
        "删除后列表必须为空"
    );
    assert!(
        get_messages_inner(&id, &state).await.unwrap().is_empty(),
        "删除会话必须级联清理其消息"
    );
}

/// TC-RAG-004 消息持久化：完成一次对话 → messages 新增 user + assistant 两条，标题自动提取（REQ-RAG-006-AC-2）。
#[tokio::test]
async fn tc_rag_004_message_persistence_and_title() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    persist_exchange(
        &state,
        &id,
        "什么是本地知识库？",
        "本地知识库是将文档索引后…",
        None,
    )
    .await
    .unwrap();

    let messages = get_messages_inner(&id, &state).await.unwrap();
    assert_eq!(messages.len(), 2, "一次完整对话必须落库两条消息");
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "什么是本地知识库？");
    assert_eq!(messages[1].role, "assistant");
    assert!(messages[1].content.contains("本地知识库"));

    let convs = get_conversations_inner("default", &state).await.unwrap();
    assert_eq!(
        convs[0].title, "什么是本地知识库？",
        "首轮问答后标题必须自动提取"
    );
}

/// TC-RAG-005 流控制：abort_chat 触发取消令牌，慢速流被中断且无 Panic（REQ-RAG-005-AC-1）。
#[tokio::test]
async fn tc_rag_005_abort_interrupts_stream() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 子验证 1：命令级 abort_chat 必须触发已注册令牌
    let token = state.abort_token_for("conv-x").await;
    state.abort_chat("conv-x").await;
    assert!(token.is_cancelled(), "abort_chat 必须触发取消令牌");

    // 子验证 2：慢速无限流在 abort 后中断，部分内容保留，无 Panic
    let token = state.abort_token_for("conv-y").await;
    let cancel = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        cancel.cancel();
    });

    let slow_stream = futures::stream::unfold(0u32, |i| async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        Some((Ok::<String, anyhow::Error>(format!("tok{i}")), i + 1))
    })
    .boxed();

    let result = forward_stream(&handle, slow_stream, token).await.unwrap();

    assert!(!result.completed, "流必须被标记为中断（未完成）");
    assert!(!result.content.is_empty(), "中断前已生成的部分内容必须保留");
}
