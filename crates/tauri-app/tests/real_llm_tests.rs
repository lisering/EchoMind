#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 真实 LLM 端到端集成测试（REQ-OBS-001 可观测性 + RAG 全链路验证）。
//!
//! ## 环境变量
//!
//! 测试仅在以下环境变量全部设置时运行：
//!
//! | 变量 | 说明 |
//! |---|---|
//! | `ECHOMIND_REAL_LLM` | 设为 `1` 启用真实 LLM 测试 |
//! | `ECHOMIND_LLM_API_KEY` | LLM API Key（如 `sk-xxx`） |
//! | `ECHOMIND_LLM_BASE_URL` | OpenAI 兼容端点（如 `https://api.deepseek.com`） |
//! | `ECHOMIND_LLM_MODEL` | 模型名（如 `deepseek-chat`） |
//!
//! ## 运行方式
//!
//! ```bash
//! ECHOMIND_REAL_LLM=1 \
//! ECHOMIND_LLM_API_KEY=sk-xxx \
//! ECHOMIND_LLM_BASE_URL=https://api.deepseek.com \
//! ECHOMIND_LLM_MODEL=deepseek-chat \
//! cargo test --features pro --test real_llm_tests -- --ignored --nocapture
//! ```
//!
//! ## 隐私
//!
//! 测试仅使用 `tests/fixtures/sample.md`（EchoMind 项目自述文档），不含敏感用户数据。

use std::time::{Duration, Instant};

use echomind_core::Storage;
use echomind_models::LlmConfig;
use echomind_tauri_app::commands::{
    chat_inner, get_messages_inner, import_files_inner, update_llm_config_inner,
};
use echomind_tauri_app::state::AppState;
use tempfile::TempDir;

/// 检查环境变量是否配置，未配置则跳过测试。
fn check_env() -> Option<(String, String, String)> {
    if std::env::var("ECHOMIND_REAL_LLM").ok().as_deref() != Some("1") {
        return None;
    }
    let api_key = std::env::var("ECHOMIND_LLM_API_KEY").ok()?;
    let base_url = std::env::var("ECHOMIND_LLM_BASE_URL").ok()?;
    let model = std::env::var("ECHOMIND_LLM_MODEL").ok()?;
    if api_key.is_empty() || base_url.is_empty() || model.is_empty() {
        return None;
    }
    Some((api_key, base_url, model))
}

/// 等待文档索引完成（轮询状态，超时 120 秒）。
///
/// 首次调用会触发 ONNX 模型下载（~30MB），可能耗时较长。
async fn wait_for_indexing(state: &AppState, timeout_secs: u64) -> anyhow::Result<()> {
    let start = Instant::now();
    loop {
        let docs = state.storage.list_documents().await?;
        if docs.iter().all(|d| {
            matches!(
                d.status,
                echomind_models::DocStatus::Indexed | echomind_models::DocStatus::Failed(_)
            )
        }) {
            // 检查是否有失败的
            for d in &docs {
                if let echomind_models::DocStatus::Failed(ref reason) = d.status {
                    return Err(anyhow::anyhow!("文档索引失败: {reason}"));
                }
            }
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(timeout_secs) {
            return Err(anyhow::anyhow!(
                "等待索引完成超时（{timeout_secs}秒），文档状态: {docs:?}"
            ));
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// TC-REAL-LLM-001：真实 LLM RAG 全链路测试。
///
/// 验证：导入文档 → 嵌入 → 检索 → LLM 流式生成 → 持久化。
#[tokio::test]
#[ignore = "需要真实 LLM API Key，运行方式见文件头注释"]
async fn tc_real_llm_001_rag_full_chain() {
    let (api_key, base_url, model) = match check_env() {
        Some(env) => env,
        None => {
            eprintln!("跳过：未设置 ECHOMIND_REAL_LLM=1 或缺少 API Key/Base URL/Model 环境变量");
            return;
        }
    };

    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 1. 配置真实 LLM 端点
    let config = LlmConfig {
        api_key,
        base_url,
        model,
    };
    update_llm_config_inner(config, &state).await.unwrap();

    // 2. 导入测试文档
    let sample_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/sample.md");
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let imported = import_files_inner(
        &handle,
        &[sample_path.to_string_lossy().into_owned()],
        &state,
    )
    .await
    .expect("导入文档失败");

    assert_eq!(imported.len(), 1, "应导入 1 个文档");

    // 3. 等待索引完成（首次会下载 ONNX 模型，超时设为 300 秒）
    wait_for_indexing(&state, 300).await.expect("索引未完成");

    // 验证文档已索引
    let docs = state.storage.list_documents().await.unwrap();
    assert!(!docs.is_empty(), "知识库应至少有 1 个文档");
    let doc = &docs[0];
    assert!(
        matches!(doc.status, echomind_models::DocStatus::Indexed),
        "文档应已索引，实际状态: {:?}",
        doc.status
    );

    // 4. 发送 RAG 查询
    let query = "EchoMind 的核心特性有哪些？";
    let conversation_id = "conv-real-llm-001";
    let result = chat_inner(&handle, query, &[], conversation_id, None, None, &state).await;

    // 验证 chat_inner 成功完成（不返回错误）
    if let Err(ref e) = result {
        eprintln!("chat_inner 返回错误: {e}");
        panic!("RAG 查询应成功完成，但返回错误: {e}");
    }

    // 5. 验证消息持久化（user + assistant）
    let messages = get_messages_inner(conversation_id, &state).await.unwrap();
    assert_eq!(
        messages.len(),
        2,
        "应有 2 条消息（user + assistant），实际: {messages:?}"
    );

    // 验证 user 消息
    assert_eq!(messages[0].role, "user");
    assert!(
        messages[0].content.contains("EchoMind"),
        "用户消息应包含查询内容"
    );

    // 验证 assistant 消息
    assert_eq!(messages[1].role, "assistant");
    assert!(
        !messages[1].content.is_empty(),
        "助手回复不应为空（LLM 应生成内容）"
    );

    // 助手回复应包含与查询相关的关键词（验证 RAG 上下文注入效果）
    let response_lower = messages[1].content.to_lowercase();
    let has_keywords = response_lower.contains("local")
        || response_lower.contains("本地")
        || response_lower.contains("embedding")
        || response_lower.contains("嵌入")
        || response_lower.contains("sqlite")
        || response_lower.contains("加密")
        || response_lower.contains("privacy")
        || response_lower.contains("隐私");
    assert!(
        has_keywords,
        "助手回复应包含与查询相关的关键词，实际回复前 200 字: {}",
        messages[1].content.chars().take(200).collect::<String>()
    );

    eprintln!(
        "RAG 查询成功！助手回复前 100 字: {}",
        messages[1].content.chars().take(100).collect::<String>()
    );
}

/// TC-REAL-LLM-002：真实 LLM 流式响应验证。
///
/// 验证：chat_inner 调用后流式生成完成，内容非空且非错误消息。
#[tokio::test]
#[ignore = "需要真实 LLM API Key，运行方式见文件头注释"]
async fn tc_real_llm_002_stream_response_non_empty() {
    let (api_key, base_url, model) = match check_env() {
        Some(env) => env,
        None => {
            eprintln!("跳过：未设置 ECHOMIND_REAL_LLM=1 或缺少环境变量");
            return;
        }
    };

    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 配置 LLM
    update_llm_config_inner(
        LlmConfig {
            api_key,
            base_url,
            model,
        },
        &state,
    )
    .await
    .unwrap();

    // 导入文档
    let sample_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/sample.md");
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    import_files_inner(
        &handle,
        &[sample_path.to_string_lossy().into_owned()],
        &state,
    )
    .await
    .expect("导入文档失败");

    wait_for_indexing(&state, 300).await.expect("索引未完成");

    // 发送查询
    let conversation_id = "conv-real-llm-002";
    let query = "EchoMind 使用什么数据库？";
    chat_inner(&handle, query, &[], conversation_id, None, None, &state)
        .await
        .expect("RAG 查询失败");

    // 验证持久化的助手消息
    let messages = get_messages_inner(conversation_id, &state).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].role, "assistant");

    // 验证助手回复内容合理（应提及 SQLite 或数据库）
    let content = &messages[1].content.to_lowercase();
    assert!(
        content.contains("sqlite") || content.contains("数据库"),
        "助手回复应提及 SQLite 或数据库，实际: {content}"
    );

    // 验证回复不是错误消息
    assert!(
        !content.contains("error") || content.contains("error handling"),
        "助手回复不应是错误消息"
    );
    assert!(
        messages[1].content.len() > 20,
        "助手回复应有一定长度（>20 字），实际: {messages:?}"
    );
}

/// TC-REAL-LLM-003：多轮对话上下文保持验证。
///
/// 验证：第一轮对话后，第二轮对话能引用上下文。
#[tokio::test]
#[ignore = "需要真实 LLM API Key，运行方式见文件头注释"]
async fn tc_real_llm_003_multi_turn_context() {
    let (api_key, base_url, model) = match check_env() {
        Some(env) => env,
        None => {
            eprintln!("跳过：未设置 ECHOMIND_REAL_LLM=1 或缺少环境变量");
            return;
        }
    };

    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    update_llm_config_inner(
        LlmConfig {
            api_key,
            base_url,
            model,
        },
        &state,
    )
    .await
    .unwrap();

    let sample_path = std::env::current_dir()
        .unwrap()
        .join("tests/fixtures/sample.md");
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    import_files_inner(
        &handle,
        &[sample_path.to_string_lossy().into_owned()],
        &state,
    )
    .await
    .expect("导入文档失败");

    wait_for_indexing(&state, 300).await.expect("索引未完成");

    let conversation_id = "conv-real-llm-003";

    // 第一轮
    let query1 = "EchoMind 的架构分几层？";
    chat_inner(&handle, query1, &[], conversation_id, None, None, &state)
        .await
        .expect("第一轮查询失败");

    let messages_after_1 = get_messages_inner(conversation_id, &state).await.unwrap();
    assert_eq!(messages_after_1.len(), 2, "第一轮后应有 2 条消息");

    // 第二轮（带历史）
    let history = messages_after_1.clone();
    let query2 = "每一层的职责是什么？";
    chat_inner(
        &handle,
        query2,
        &history,
        conversation_id,
        None,
        None,
        &state,
    )
    .await
    .expect("第二轮查询失败");

    let messages_after_2 = get_messages_inner(conversation_id, &state).await.unwrap();
    assert_eq!(
        messages_after_2.len(),
        4,
        "两轮后应有 4 条消息（2 user + 2 assistant）"
    );

    // 验证第二轮助手回复引用了上下文
    let response2 = &messages_after_2[3].content.to_lowercase();
    assert!(!response2.is_empty(), "第二轮助手回复不应为空");
    assert!(
        response2.contains("models")
            || response2.contains("core")
            || response2.contains("infra")
            || response2.contains("tauri")
            || response2.contains("契约")
            || response2.contains("适配")
            || response2.contains("逻辑"),
        "第二轮回复应提及架构各层，实际: {response2}"
    );
}
