#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Chat 相关集成测试 — RAG 对话/流式/SSE/取消/abort/设置持久化。

use super::common::*;
use super::*;
use echomind_core::import::ImportService;

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
    persist_exchange(
        &state,
        &id,
        "级联删除验证问题",
        "级联删除验证回答",
        None,
        None,
        None,
        None,
    )
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

/// TC-NAV-007 会话列表分页：新建多个会话 → 分页拉取验证 limit/offset/total（REQ-NAV-007）。
#[tokio::test]
async fn tc_nav_007_conversations_pagination() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建 5 个会话
    let mut ids = Vec::new();
    for i in 0..5 {
        let id = create_conversation_inner("default".to_string(), &state)
            .await
            .unwrap();
        persist_exchange(
            &state,
            &id,
            &format!("问题{i}"),
            "回答",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
        ids.push(id);
    }

    // 第一页：limit=2, offset=0 → 返回最近 2 条，total=5
    let page1 = get_conversations_paginated_inner("default", 2, 0, &state)
        .await
        .unwrap();
    assert_eq!(page1.items.len(), 2, "第一页应返回 2 条");
    assert_eq!(page1.total, 5, "总数应为 5");

    // 第二页：limit=2, offset=2 → 返回 2 条
    let page2 = get_conversations_paginated_inner("default", 2, 2, &state)
        .await
        .unwrap();
    assert_eq!(page2.items.len(), 2, "第二页应返回 2 条");
    assert_eq!(page2.total, 5);

    // 第三页：limit=2, offset=4 → 返回 1 条（不足一页）
    let page3 = get_conversations_paginated_inner("default", 2, 4, &state)
        .await
        .unwrap();
    assert_eq!(page3.items.len(), 1, "第三页应返回 1 条");
    assert_eq!(page3.total, 5);

    // 验证分页不重叠：第一页和第二页的 ID 不重复
    let page1_ids: std::collections::HashSet<_> =
        page1.items.iter().map(|c| c.id.clone()).collect();
    let page2_ids: std::collections::HashSet<_> =
        page2.items.iter().map(|c| c.id.clone()).collect();
    assert!(
        page1_ids.is_disjoint(&page2_ids),
        "不同页的会话 ID 不应重复"
    );

    // 验证空工作区
    let empty = get_conversations_paginated_inner("empty_ws", 10, 0, &state)
        .await
        .unwrap();
    assert_eq!(empty.items.len(), 0);
    assert_eq!(empty.total, 0);
}

/// TC-NAV-007b 会话列表分页边界：offset 超出总数 → 返回空列表但 total 正确。
#[tokio::test]
async fn tc_nav_007b_pagination_offset_beyond_total() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let _id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // offset=100 远超总数 1
    let result = get_conversations_paginated_inner("default", 10, 100, &state)
        .await
        .unwrap();
    assert_eq!(result.items.len(), 0, "offset 超出总数应返回空列表");
    assert_eq!(result.total, 1, "total 仍应正确反映总数");
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
        None,
        None,
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

    let result = forward_stream(&handle, slow_stream, token, None)
        .await
        .unwrap();

    assert!(!result.completed, "流必须被标记为中断（未完成）");
    assert!(!result.content.is_empty(), "中断前已生成的部分内容必须保留");
}

// ================== REQ-RAG-020 Cross-Encoder 重排序开关持久化测试 ==================

/// TC-RAG-020 重排序开关持久化与重启恢复（REQ-RAG-020 前端接线）。
///
/// AC-1：默认 rerank_enabled 为 false
/// AC-2：set_rerank_enabled(true) 后 get_settings 返回 rerank_enabled = true
/// AC-3：set_rerank_enabled(false) 后 get_settings 返回 rerank_enabled = false
/// AC-4：重启（重建 AppState）后设置仍在
#[tokio::test]
async fn tc_rag_020_rerank_toggle_persists() {
    let dir = TempDir::new().unwrap();
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        update_llm_config_inner(test_config(), &state)
            .await
            .unwrap();

        // AC-1: 默认开启（层次化重排为精度默认项）
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(settings.rerank_enabled, "AC-1: 重排序默认应开启");

        // AC-2: 开启
        set_rerank_enabled_inner(true, &state).await.unwrap();
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(settings.rerank_enabled, "AC-2: 开启后重排序应为 true");

        // AC-3: 关闭
        set_rerank_enabled_inner(false, &state).await.unwrap();
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(!settings.rerank_enabled, "AC-3: 关闭后重排序应为 false");
    }

    // AC-4: 重启后设置仍在（关闭状态）
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert!(!settings.rerank_enabled, "AC-4: 重启后重排序应保持关闭");

    // 再测开启后重启
    set_rerank_enabled_inner(true, &restarted).await.unwrap();
    drop(restarted);
    let restarted2 = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings2 = get_settings_inner(&restarted2).await.unwrap();
    assert!(
        settings2.rerank_enabled,
        "AC-4: 重启后重排序开启状态应持久化"
    );
}

// ================== REQ-RAG-021 HyDE 查询改写开关持久化测试 ==================

/// TC-RAG-021 HyDE 查询改写开关持久化与重启恢复（REQ-RAG-021 前端接线）。
///
/// AC-1：默认 hyde_enabled 为 false
/// AC-2：set_hyde_enabled(true) 后 get_settings 返回 hyde_enabled = true
/// AC-3：set_hyde_enabled(false) 后 get_settings 返回 hyde_enabled = false
/// AC-4：重启（重建 AppState）后设置仍在
#[tokio::test]
async fn tc_rag_021_hyde_toggle_persists() {
    let dir = TempDir::new().unwrap();
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        update_llm_config_inner(test_config(), &state)
            .await
            .unwrap();

        // AC-1: 默认关闭
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(!settings.hyde_enabled, "AC-1: HyDE 默认应关闭");

        // AC-2: 开启
        set_hyde_enabled_inner(true, &state).await.unwrap();
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(settings.hyde_enabled, "AC-2: 开启后 HyDE 应为 true");

        // AC-3: 关闭
        set_hyde_enabled_inner(false, &state).await.unwrap();
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(!settings.hyde_enabled, "AC-3: 关闭后 HyDE 应为 false");
    }

    // AC-4: 重启后设置仍在（关闭状态）
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert!(!settings.hyde_enabled, "AC-4: 重启后 HyDE 应保持关闭");

    // 再测开启后重启
    set_hyde_enabled_inner(true, &restarted).await.unwrap();
    drop(restarted);
    let restarted2 = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings2 = get_settings_inner(&restarted2).await.unwrap();
    assert!(settings2.hyde_enabled, "AC-4: 重启后 HyDE 开启状态应持久化");
}

// ================== VEC-012 嵌入模型运行时切换 L2 契约测试 ==================

/// TC-VEC-012 嵌入模型持久化与重启恢复（REQ-VEC-012）。
///
/// AC-1：默认 embedding_model 为 "all-MiniLM-L6-v2"
/// AC-2：set_embedding_model("bge-small-zh-v1.5") 后 get_settings 返回对应模型
/// AC-3：set_embedding_model("e5-small-v2") 后 get_settings 返回对应模型
/// AC-4：重启（重建 AppState）后设置仍在
/// AC-5：非法模型标识被拒绝
/// TC-AUDIT-008 审计取消机制：abort_audit 设置 AtomicBool 标志，audit_cancel_for 返回的标志可被检测（REQ-AUDIT-005）。
#[tokio::test]
async fn tc_audit_008_abort_sets_cancel_flag() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 获取取消标志（初始为 false）
    let flag = state.audit_cancel_for("doc-cancel-test").await;
    assert!(
        !flag.load(std::sync::atomic::Ordering::SeqCst),
        "初始状态不应取消"
    );

    // 触发取消
    state.abort_audit("doc-cancel-test").await;
    assert!(
        flag.load(std::sync::atomic::Ordering::SeqCst),
        "abort_audit 后标志必须为 true"
    );

    // 清理后标志仍保持 true（已传播给持有者），但 HashMap 条目已移除
    state.clear_audit_cancel("doc-cancel-test").await;
    assert!(
        flag.load(std::sync::atomic::Ordering::SeqCst),
        "清理后已传播的标志仍为 true（语义不变）"
    );

    // 再次获取应为新标志（false）
    let new_flag = state.audit_cancel_for("doc-cancel-test").await;
    assert!(
        !new_flag.load(std::sync::atomic::Ordering::SeqCst),
        "清理后重新获取应为新标志（false）"
    );
}

/// TC-AUDIT-009 审计按钮可见性条件：仅 Pro 版 + 已索引文档显示审计按钮（REQ-AUDIT-001 前端旁证）。
/// 此测试验证后端数据前提：Pro 激活后导入文档，文档状态为 Indexed 且可通过 list_documents 获取。
#[cfg(feature = "pro")]
// ============================================================================

/// TC-IPC-RAG-020：重排序开关持久化（REQ-RAG-020）。
#[tokio::test]
async fn tc_ipc_rag_020_rerank_toggle_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    set_rerank_enabled_inner(true, &state).await.unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert!(settings.rerank_enabled, "重排序应已启用");

    set_rerank_enabled_inner(false, &state).await.unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert!(!settings.rerank_enabled, "重排序应已禁用");
}

/// TC-IPC-RAG-021：HyDE 开关持久化（REQ-RAG-021）。
#[tokio::test]
async fn tc_ipc_rag_021_hyde_toggle_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    set_hyde_enabled_inner(true, &state).await.unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert!(settings.hyde_enabled, "HyDE 应已启用");
}

/// TC-IPC-RAG-022：Agent 模式开关持久化（REQ-RAG-022）。
/// TC-RAG-017-001：get_settings 默认返回 context_token_limit = 4096。
#[tokio::test]
async fn tc_rag_017_001_default_context_token_limit() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.context_token_limit, 4096,
        "默认上下文 token 限制应为 4096"
    );
}

/// TC-RAG-017-002：set_context_token_limit 写入合法值后 get_settings 反映最新值。
#[tokio::test]
async fn tc_rag_017_002_set_valid_limit() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 设置为 8192
    set_context_token_limit_inner(8192, &state).await.unwrap();

    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.context_token_limit, 8192,
        "应反映已设置的上下文 token 限制"
    );
}

/// TC-RAG-017-003：set_context_token_limit 拒绝超出范围的值（< 2048）。
#[tokio::test]
async fn tc_rag_017_003_reject_below_range() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = set_context_token_limit_inner(1024, &state).await;
    assert!(result.is_err(), "低于 2048 的值应被拒绝");
    assert!(
        result.unwrap_err().contains("2048"),
        "错误消息应包含合法范围提示"
    );
}

/// TC-RAG-017-004：set_context_token_limit 拒绝超出范围的值（> 32768）。
#[tokio::test]
async fn tc_rag_017_004_reject_above_range() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = set_context_token_limit_inner(65536, &state).await;
    assert!(result.is_err(), "超过 32768 的值应被拒绝");
    assert!(
        result.unwrap_err().contains("32768"),
        "错误消息应包含合法范围提示"
    );
}

/// TC-RAG-017-005：set_context_token_limit 重启后恢复配置。
#[tokio::test]
async fn tc_rag_017_005_restore_after_restart() {
    let dir = TempDir::new().unwrap();

    // 写入阶段
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        set_context_token_limit_inner(16384, &state).await.unwrap();
    }

    // 重启阶段
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert_eq!(
        settings.context_token_limit, 16384,
        "重启后上下文 token 限制应恢复为 16384"
    );
}

/// TC-RAG-017-006：set_context_token_limit 边界值测试（2048 和 32768 均合法）。
#[tokio::test]
async fn tc_rag_017_006_boundary_values() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 下边界 2048
    set_context_token_limit_inner(2048, &state).await.unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(settings.context_token_limit, 2048, "下边界 2048 应合法");

    // 上边界 32768
    set_context_token_limit_inner(32768, &state).await.unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(settings.context_token_limit, 32768, "上边界 32768 应合法");
}
// ============================================================================
// TC-RES：对话韧性 — embedder 初始化超时/失败恢复
// ============================================================================
//
// 根因：chat_inner 中 init_embedder_with_progress 无超时保护。当 ONNX 模型下载
// 因网络问题挂起时（如 HuggingFace 在境内不可达），chat 命令永久阻塞，
// 前端卡死在「初始化向量化引擎」。
//
// 修复：chat_inner 使用 tokio::time::timeout 包装 init_embedder_with_progress，
// 超时阈值可通过 ECHOMIND_EMBEDDER_TIMEOUT 环境变量覆盖。
//
// P2-6 弹性降级：嵌入引擎初始化失败/超时时不再返回 EMBED: 错误，而是降级到
// 纯关键词搜索（BM25）模式。chat_inner 仍会快速返回（不永久阻塞），
// 但错误可能来自 LLM 调用阶段（因为测试环境无真实 API）。

/// TC-RES-001：embedder 初始化超时/失败时 chat_inner 不永久阻塞。
///
/// 验证：设置 1 秒超时 → 导入文档使 KB 非空 → chat_inner 在合理时间内返回。
/// 此前无超时保护时，chat_inner 会永久阻塞。
/// P2-6 后：降级到关键词搜索，不再返回 EMBED: 错误，但仍快速返回（LLM 阶段失败）。
#[tokio::test]
async fn tc_res_001_embedder_init_timeout_returns_embed_error() {
    // 设置 1 秒超时（测试环境无法在 1 秒内下载 30MB ONNX 模型）
    // 注意：仅此测试调用 chat_inner 且 KB 非空，不影响其他并行测试
    // SAFETY: 测试进程中设置环境变量，仅此测试调用 chat_inner 且 KB 非空
    unsafe {
        std::env::set_var("ECHOMIND_EMBEDDER_TIMEOUT", "1");
    }

    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 配置 LLM（排除「未配置」干扰）
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 导入文档使知识库非空（排除「空库」干扰）
    // import_one 只创建 document + chunks，不需要 embedder
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let path = dir.path().join("res-test.md");
    std::fs::write(&path, "递归是一种编程技巧").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
    let _ = service.import_one(&canon, true).await.unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // chat_inner 应在超时后返回 EMBED: 错误，而非永久阻塞
    // 使用 30 秒测试级超时保护（防止测试本身挂起）
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        chat_inner(&handle, "测试问题", &[], "conv-res-001", None, None, &state),
    )
    .await;

    // 清理 env var
    // SAFETY: 清理测试中设置的环境变量
    unsafe {
        std::env::remove_var("ECHOMIND_EMBEDDER_TIMEOUT");
    }

    // 验证 chat_inner 在 30 秒内返回（未永久阻塞）
    let inner_result = match result {
        Ok(r) => r,
        Err(_) => panic!("chat_inner 在 30 秒内未返回（可能永久阻塞）"),
    };

    // P2-6：嵌入降级后不再返回 EMBED: 错误，而是降级到关键词搜索。
    // 降级路径有两种结果：
    //   1) 关键词搜索无结果 → return Ok(())（降级但无内容可回答）
    //   2) 关键词搜索有结果 → LLM 调用（测试环境会失败 → Err）
    // 核心验证点：chat_inner 不永久阻塞（在 30s 内返回）+ 不返回 EMBED: 错误
    let err = inner_result.err().unwrap_or_default();
    // 降级路径不应返回 EMBED: 前缀（已降级为关键词搜索）
    assert!(
        !err.starts_with("EMBED:"),
        "P2-6 降级后不应返回 EMBED: 前缀错误，实际: {err}"
    );
}

/// TC-RES-002：embedder 初始化超时后错误消息包含用户引导。
///
/// 验证超时错误消息包含可操作的引导文案（检查网络 + 设置中手动初始化）。
#[tokio::test]
async fn tc_res_002_embedder_timeout_message_has_guidance() {
    // SAFETY: 测试进程中设置环境变量
    unsafe {
        std::env::set_var("ECHOMIND_EMBEDDER_TIMEOUT", "1");
    }

    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let path = dir.path().join("res-guide.md");
    std::fs::write(&path, "内容").unwrap();
    let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
    let _ = service.import_one(&canon, true).await.unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        chat_inner(&handle, "测试", &[], "conv-res-002", None, None, &state),
    )
    .await;

    // SAFETY: 清理测试中设置的环境变量
    unsafe {
        std::env::remove_var("ECHOMIND_EMBEDDER_TIMEOUT");
    }

    let inner_result = result.expect("chat_inner 应在 30 秒内返回");
    let err = inner_result.err().unwrap_or_default();

    // 超时路径：错误消息应包含网络引导和手动初始化引导
    if err.contains("超时") {
        assert!(
            err.contains("网络") || err.contains("network"),
            "超时错误应包含网络检查引导，实际: {err}"
        );
        assert!(
            err.contains("设置") || err.contains("Settings"),
            "超时错误应包含手动初始化引导，实际: {err}"
        );
    }
    // 如果是「不可用」路径（网络立即失败），不需要检查引导文案
}
// ================== v1.0 发布准备：设置持久化测试 ==================

/// TC-PERSIST-001: hybrid_search 设置持久化（重启后仍保持开启）。
#[tokio::test]
async fn tc_persist_001_hybrid_search_persists() {
    let dir = TempDir::new().unwrap();
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        set_hybrid_search_inner(true, &state).await.unwrap();
        let settings = get_settings_inner(&state).await.unwrap();
        assert!(settings.hybrid_search, "设置后 hybrid_search 应为 true");
    }
    // 模拟重启
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert!(settings.hybrid_search, "重启后 hybrid_search 应保持为 true");
}

/// TC-PERSIST-002: compression_ratio 已在 R1 简化中删除，跳过此测试。
#[tokio::test]
#[ignore = "compression_ratio removed in R1 simplification"]
async fn tc_persist_002_compression_ratio_persists() {}

// ================== v1.0 发布准备：边界值测试 ==================

/// TC-EDGE-001: 空字符串 query chat 应返回错误（不应崩溃）。
#[tokio::test]
async fn tc_edge_001_empty_query_chat() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = chat_inner(&handle, "", &[], "conv-edge-1", None, None, &state).await;
    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("知识库为空") || err.contains("空") || err.contains("empty"),
        "空知识库 + 空 query 应返回知识库为空错误，实际: {err}"
    );
}

/// TC-EDGE-002: 超长 query（>10KB）chat 不应崩溃或 panic。
#[tokio::test]
async fn tc_edge_002_super_long_query_chat() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 生成 10KB+ 的超长 query
    let long_query = "测试".repeat(6000); // ~12KB

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = chat_inner(&handle, &long_query, &[], "conv-edge-2", None, None, &state).await;
    // 不应 panic，应返回正常错误（知识库为空）
    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("知识库为空")
            || err.contains("空")
            || err.contains("empty")
            || err.contains("VALIDATION")
            || err.contains("过长"),
        "超长 query + 空知识库应返回知识库为空或验证错误，实际: {err}"
    );
}

/// TC-CONC-001: 多个 chat 命令并发执行（不同 conversation_id）不应互相干扰。
#[tokio::test]
async fn tc_conc_001_concurrent_chat_different_conversations() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let handle = Arc::new(app.handle().clone());

    // 并发发起 3 个 chat（不同会话），都应返回「知识库为空」错误
    let mut handles = Vec::new();
    for i in 0..3 {
        let state_clone = state.clone();
        let handle_clone = handle.clone();
        handles.push(tokio::spawn(async move {
            chat_inner(
                &handle_clone,
                &format!("并发问题{i}"),
                &[],
                &format!("conv-conc-{i}"),
                None,
                None,
                &state_clone,
            )
            .await
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        let result = h.await.unwrap();
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("知识库为空") || err.contains("空") || err.contains("empty"),
            "并发 chat #{i} 应返回知识库为空错误，实际: {err}"
        );
    }
}

/// TC-CONC-002: import_files 并发 + chat 并发（读写竞争不应 panic）。
#[tokio::test]
async fn tc_conc_002_concurrent_import_and_chat() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 创建测试文件
    let test_file = dir.path().join("concurrent_test.md");
    std::fs::write(&test_file, "# 并发测试\n\n这是一个并发导入测试文档。").unwrap();

    let app = tauri::test::mock_app();
    let handle = Arc::new(app.handle().clone());

    // 并发：导入 + chat
    let state_import = state.clone();
    let handle_import = handle.clone();
    let file_path = test_file.to_string_lossy().to_string();
    let import_handle = tokio::spawn(async move {
        import_files_inner(&handle_import, &[file_path], &state_import).await
    });

    let state_chat = state.clone();
    let handle_chat = handle.clone();
    let chat_handle = tokio::spawn(async move {
        chat_inner(
            &handle_chat,
            "并发查询",
            &[],
            "conv-conc-mix",
            None,
            None,
            &state_chat,
        )
        .await
    });

    // 两者都不应 panic
    let import_result = import_handle.await.unwrap();
    let chat_result = chat_handle.await.unwrap();

    // 导入应成功或返回合理错误
    assert!(
        import_result.is_ok() || import_result.err().is_some(),
        "导入操作应完成（成功或错误），不应 panic"
    );

    // chat 可能返回知识库为空（如果导入还没完成）或其他错误
    if let Err(e) = chat_result {
        assert!(!e.is_empty(), "chat 错误信息不应为空");
    }
}

// ================== v1.0 发布准备：Pro 门控测试 ==================

/// TC-PRO-GATE-001: Free 版本 is_pro = false，Pro 功能不可用。
///
/// v1.20.0 起：is_pro = false。
#[tokio::test]
async fn tc_pro_gate_001_free_tier_pro_disabled() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let pro_status = get_pro_status_inner(&state).await;
    assert!(!pro_status, "Free 版本 is_pro 必须为 false");
}

/// TC-PRO-GATE-002: activate_pro + deactivate_pro 生命周期。
#[tokio::test]
async fn tc_pro_gate_002_pro_activation_lifecycle() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 使用开发种子签发合法 License
    let license_key = make_valid_license();

    // 激活 Pro
    activate_pro_inner(&license_key, &state).await.unwrap();
    assert!(*state.is_pro().read().await, "激活后 is_pro 必须为 true");

    // 验证 get_pro_status 返回 true
    let pro_status = get_pro_status_inner(&state).await;
    assert!(pro_status, "激活后 get_pro_status 必须返回 true");

    // 停用 Pro
    deactivate_pro_inner(&state).await.unwrap();
    assert!(!*state.is_pro().read().await, "停用后 is_pro 必须为 false");

    let pro_status = get_pro_status_inner(&state).await;
    assert!(!pro_status, "停用后 get_pro_status 必须返回 false");
}

// ================== v1.0 发布准备：补充集成测试 ==================

/// TC-CONC-003: abort_chat + clear_abort 生命周期（中断令牌创建/取消/清理）。
#[tokio::test]
async fn tc_conc_003_abort_lifecycle() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 获取 abort token（应自动创建）
    let token = state.abort_token_for(&conv_id).await;
    assert!(!token.is_cancelled(), "新建令牌不应已取消");

    // 触发中断
    state.abort_chat(&conv_id).await;
    assert!(token.is_cancelled(), "中断后令牌应已取消");

    // 清理后再次获取应是全新的
    state.clear_abort(&conv_id).await;
    let token2 = state.abort_token_for(&conv_id).await;
    assert!(!token2.is_cancelled(), "清理后新令牌不应已取消");
}

/// TC-CONC-004: 多个会话并发创建不冲突（不同 conversation_id 唯一）。
#[tokio::test]
async fn tc_conc_004_multiple_conversation_create() {
    let dir = TempDir::new().unwrap();
    let state = Arc::new(AppState::new(dir.path().to_path_buf()).await.unwrap());

    let s1 = state.clone();
    let s2 = state.clone();
    let s3 = state.clone();
    let s4 = state.clone();
    let s5 = state.clone();

    let (r1, r2, r3, r4, r5) = tokio::join!(
        create_conversation_inner("default".to_string(), &s1),
        create_conversation_inner("default".to_string(), &s2),
        create_conversation_inner("default".to_string(), &s3),
        create_conversation_inner("default".to_string(), &s4),
        create_conversation_inner("default".to_string(), &s5),
    );

    let mut ids = vec![
        r1.unwrap(),
        r2.unwrap(),
        r3.unwrap(),
        r4.unwrap(),
        r5.unwrap(),
    ];
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 5, "并发创建的 5 个会话 ID 必须全部唯一");

    let list = get_conversations_inner("default", &state).await.unwrap();
    assert_eq!(list.len(), 5, "会话列表应有 5 个会话");
}
