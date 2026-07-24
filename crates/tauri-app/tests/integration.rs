#![allow(clippy::unwrap_used)]
//! QA 红灯集成测试（L2：Tauri Mock + 真实 SQLite，体系二阶段 2 先于实现编写）。
//! 覆盖：TC-UI-001 配置持久化（REQ-UI-008）/ TC-UI-002 空知识库拦截（REQ-RAG-003-AC-2）。

use echomind_models::LlmConfig;
use echomind_tauri_app::commands::{chat_inner, get_settings_inner, update_llm_config_inner};
use echomind_tauri_app::state::AppState;
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
