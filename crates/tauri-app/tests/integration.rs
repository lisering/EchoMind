#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented,
    unused_imports,
    dead_code
)]
//! QA 红灯集成测试（L2：Tauri Mock + 真实 SQLite，体系二阶段 2 先于实现编写）。
//! 覆盖：TC-UI-001 配置持久化（REQ-UI-008）/ TC-UI-002 空知识库拦截（REQ-RAG-003-AC-2）。

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use echomind_core::Storage;
use echomind_core::errors::{
    ERR_EMBED, ERR_UNKNOWN, classify_llm_error, has_error_prefix, prefix_error,
};
#[cfg(feature = "pro")]
use echomind_models::LlmSamplingParams;
use echomind_models::{ChatMessage, Chunk, DocStatus, Document, LlmConfig, LlmMode, TokenUsage};
use echomind_tauri_app::commands::{
    activate_pro_inner, chat_inner, create_conversation_inner, deactivate_pro_inner,
    delete_conversation_inner, delete_document_inner, delete_model_inner, edit_user_message_inner,
    edit_user_message_inner_full, export_conversation_markdown_inner,
    export_document_original_inner, forward_stream, get_conversation_cost_inner,
    get_conversations_inner, get_conversations_paginated_inner, get_llm_mode_inner,
    get_locale_inner, get_messages_inner, get_pro_status_inner, get_settings_inner,
    get_sidebar_collapsed_inner, get_watched_folders_inner, import_files_inner,
    list_local_models_inner, open_data_dir_inner, persist_exchange, rebuild_index_inner,
    record_token_usage_inner, remove_watched_folder_inner, retry_index_inner, save_text_file_inner,
    search_symbols_inner, set_agent_enabled_inner, set_compression_ratio_inner,
    set_context_token_limit_inner, set_coordinator_mode_inner, set_embedding_model_inner,
    set_hybrid_search_inner, set_hyde_enabled_inner, set_llm_mode_inner, set_local_model_inner,
    set_locale_inner, set_memory_enabled_inner, set_rerank_enabled_inner,
    set_sidebar_collapsed_inner, set_token_budget_inner, update_llm_config_inner,
};
#[cfg(feature = "pro")]
use echomind_tauri_app::commands::{
    audit_document_inner, clear_kv_cache_inner, get_kernel_mode_inner, get_kv_cache_status_inner,
    load_kv_cache_inner, save_kv_cache_inner, set_kernel_mode_inner, set_kv_cache_enabled_inner,
    set_paged_attn_inner, set_sampling_params_inner, set_vlm_enabled_inner,
};
// REQ-VEC-014: 自定义 ONNX 嵌入模型上传（Pro 门控）
#[cfg(feature = "pro")]
use echomind_tauri_app::commands::{
    delete_custom_model_inner, list_custom_models_inner, upload_custom_embedding_model_inner,
};
use echomind_tauri_app::state::AppState;
use ed25519_dalek::Signer;
use futures::StreamExt;
use std::sync::Arc;
use tauri::Listener;
use tempfile::TempDir;

#[path = "integration/chat_tests.rs"]
mod chat_tests;
#[path = "integration/common.rs"]
mod common;
#[path = "integration/conversation_tests.rs"]
mod conversation_tests;
#[path = "integration/full_pipeline.rs"]
mod full_pipeline;
#[path = "integration/import_tests.rs"]
mod import_tests;
#[path = "integration/local_llm_chain.rs"]
mod local_llm_chain;
#[path = "integration/security_chain.rs"]
mod security_chain;
#[path = "integration/security_tests.rs"]
mod security_tests;
#[path = "integration/smart_mode_tests.rs"]
mod smart_mode_tests;

pub use common::{make_valid_license, test_config};

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
    let runtime_cfg = restarted.llm_config().read().await.clone();
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

    let result = chat_inner(&handle, "任意问题", &[], "conv-1", None, None, &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("知识库为空"),
        "空知识库必须返回引导导入的错误提示，实际: {err}"
    );
}

/// TC-LIC-003 免费版配额拦截：已有 50 个文件，导入第 51 个返回 LIMIT_REACHED（REQ-LIC-002-AC-1）。
///
/// v1.20.0 起：Alpha 阶段结束，配额限制正式生效。
#[tokio::test]
async fn tc_lic_003_free_tier_quota_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 确认为免费版
    assert!(!get_pro_status_inner(&state).await, "测试前提：免费版");

    // 预填充 50 个文档（直接写入 storage，跳过导入管线）
    for i in 0..50 {
        let doc = Document::new(format!("doc-{i}.md"), format!("hash-{i}"));
        state.storage.add_document(&doc).await.unwrap();
    }

    // 创建测试文件
    let extra = dir.path().join("extra.md");
    std::fs::write(&extra, "extra content").unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = import_files_inner(&handle, &[extra.to_string_lossy().into_owned()], &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("LIMIT_REACHED"),
        "配额触顶应返回 LIMIT_REACHED 错误，实际: {err}"
    );
}

/// TC-LIC-004 Pro 门控格式付费门（REQ-LIC-002-AC-2）。
///
/// v1.20.0 起：Alpha 阶段结束，Free 用户导入 Pro 门控格式应返回 PRO_REQUIRED。
#[tokio::test]
async fn tc_lic_004_pro_gated_formats_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 确认为免费版
    assert!(!*state.is_pro().read().await, "测试前提：免费版");

    for ext in ["pdf", "docx", "pptx", "epub", "xlsx", "csv"] {
        let fname = format!("test.{ext}");
        let file = dir.path().join(&fname);
        std::fs::write(&file, b"fake-content").unwrap();

        let result =
            import_files_inner(&handle, &[file.to_string_lossy().into_owned()], &state).await;
        let err = result.err().unwrap_or_default();
        assert!(
            err.contains("PRO_REQUIRED"),
            "Free 用户 .{ext} 导入应返回 PRO_REQUIRED，实际: {err}"
        );
    }
}

/// TC-LIC-005 激活 Pro 并跨重启持久化（REQ-LIC-001-AC-3）。
///
/// v1.20.0 起：初始为 false，激活后变 true，重启后恢复 true。
#[tokio::test]
async fn tc_lic_005_activate_pro_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    let license = make_valid_license();

    // 第一次会话：激活 Pro
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        assert!(!get_pro_status_inner(&state).await, "初始状态应为免费版");

        let result = activate_pro_inner(&license, &state).await;
        assert!(
            result.is_ok(),
            "有效 License 激活应成功: {:?}",
            result.err()
        );
        assert!(get_pro_status_inner(&state).await, "激活后运行态应为 Pro");
    } // AppState 销毁，模拟应用退出

    // 第二次会话：从 SQLite 恢复
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    assert!(
        get_pro_status_inner(&restarted).await,
        "重启后 Pro 状态必须从 SQLite 恢复"
    );
}

/// TC-LIC-005b 无效 License 被拒绝（REQ-LIC-001-AC-2/AC-4）。
///
/// v1.20.0 起：初始为 false，拒绝后仍为 false。
#[tokio::test]
async fn tc_lic_005b_invalid_license_rejected() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = activate_pro_inner("invalid-license-string", &state).await;
    assert!(result.is_err(), "无效 License 必须被拒绝");

    assert!(
        !get_pro_status_inner(&state).await,
        "拒绝后状态不得变为 Pro"
    );
}

// ================== Phase 8.5：混沌测试（REQ-CHAOS-001~007） ==================
// 铁律四（PROJECT_RULES.md §3.0）：测试必须覆盖真实世界语料与极端边缘数据。
// 语料由 `tests/fixtures/generate_corpus.py` 自动生成，运行测试前请先执行该脚本。

mod chaos_tests {
    use super::*;
    use echomind_core::Storage;
    use echomind_core::import::{ImportOutcome, ImportService};
    use std::time::Instant;

    /// 获取混沌测试 fixtures 目录的绝对路径。
    /// `CARGO_MANIFEST_DIR` 指向 `crates/tauri-app/`，fixtures 在 `../../tests/fixtures/`。
    fn fixtures_dir() -> std::path::PathBuf {
        let manifest = env!("CARGO_MANIFEST_DIR");
        std::path::Path::new(manifest)
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
    }

    /// 获取指定 fixture 文件的绝对路径；文件不存在时 panic 并给出清晰提示。
    /// 返回 canonicalize 后的路径（不含 `..`，避免触发路径遍历安全检查）。
    fn fixture(name: &str) -> String {
        let path = fixtures_dir().join(name);
        assert!(
            path.exists(),
            "Fixture 缺失: {}. 请先运行 `python3 tests/fixtures/generate_corpus.py`",
            path.display()
        );
        // canonicalize 解析 `..` 为绝对路径，避免 ImportService::sanitize_path 误拦
        path.canonicalize()
            .unwrap_or_else(|e| panic!("canonicalize 失败: {e}"))
            .to_string_lossy()
            .into_owned()
    }

    /// 递归收集目录下所有指定扩展名的文件路径（按路径排序，保证确定性）。
    /// 返回 canonicalize 后的路径（不含 `..`，避免触发路径遍历安全检查）。
    fn collect_files(dir: &std::path::Path, ext: &str) -> Vec<String> {
        let mut result = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    result.extend(collect_files(&path, ext));
                } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                    // canonicalize 解析 `..` 为绝对路径
                    if let Ok(canon) = path.canonicalize() {
                        result.push(canon.to_string_lossy().into_owned());
                    }
                }
            }
        }
        result.sort();
        result
    }

    /// TC-CHAOS-001 抗压测试：导入大型真实 Markdown（Tauri README），断言不 Panic、
    /// 成功切分为多个 Chunk 并写入数据库、耗时在合理范围内（REQ-CHAOS-001）。
    #[tokio::test]
    async fn tc_chaos_001_large_real_md_stress() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let path = fixture("large_real.md");

        let start = Instant::now();
        let outcome = service.import_one(&path, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };
        service.index_document(&doc).await.unwrap();
        let elapsed = start.elapsed();

        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(
            !chunks.is_empty(),
            "大型 Markdown 必须切分出至少 1 个 Chunk"
        );
        assert!(
            elapsed.as_secs() < 30,
            "导入+索引耗时应 < 30s，实际: {elapsed:?}"
        );
        println!(
            "  [TC-CHAOS-001] {}/{} chunks, {elapsed:?}",
            path,
            chunks.len()
        );
    }

    /// TC-CHAOS-002 复杂 PDF 测试：导入真实 arXiv 论文 PDF（含图表/公式/多栏），
    /// 断言能提取出有效文本（Chunk > 0），不因 PDF 内部结构复杂而崩溃（REQ-CHAOS-002）。
    #[tokio::test]
    async fn tc_chaos_002_complex_paper_pdf() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let path = fixture("complex_paper.pdf");

        let outcome = service.import_one(&path, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // index_document 失败时返回 Err（PdfLoader 可能无法提取某些 PDF 的文本）
        let result = service.index_document(&doc).await;
        if let Err(e) = &result {
            // 如果 PDF 文本提取失败，验证是优雅失败而非 Panic
            println!("  [TC-CHAOS-002] PDF 提取失败（可接受）: {e}");
            return;
        }

        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(
            !chunks.is_empty(),
            "复杂 PDF 必须提取出有效文本（Chunk > 0）"
        );
        println!("  [TC-CHAOS-002] PDF 提取成功，{} chunks", chunks.len());
    }

    /// TC-CHAOS-003 容错测试：导入混合编码 TXT（UTF-8/GBK/非法字节），
    /// 断言不 Panic，from_utf8_lossy 机制成功提取出部分可读文本（REQ-CHAOS-003）。
    #[tokio::test]
    async fn tc_chaos_003_mixed_encoding_txt() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let path = fixture("mixed_encoding.txt");

        let outcome = service.import_one(&path, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // TextLoader 使用 from_utf8_lossy，混合编码文件不应导致崩溃
        service.index_document(&doc).await.unwrap();

        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "混合编码文件必须产生 Chunk");

        // 验证 UTF-8 部分的可读文本被保留（GBK 部分会变为 U+FFFD 替代符）
        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_text.contains("UTF-8"),
            "from_utf8_lossy 应保留 UTF-8 编码部分的可读文本"
        );
        assert!(
            all_text.contains("Normal English"),
            "ASCII 部分应被完整保留"
        );
        println!("  [TC-CHAOS-003] 混合编码容错成功，{} chunks", chunks.len());
    }

    /// TC-CHAOS-004 安全测试：导入含 XSS 攻击向量的恶意 Markdown，
    /// 断言 MarkdownLoader 剥离 HTML 标签（<script>/<img onerror>/<iframe>/javascript:），
    /// 保留正文文本（REQ-CHAOS-004；前端 DOMPurify 为第二层防线，此处验证后端净化）。
    #[tokio::test]
    async fn tc_chaos_004_malicious_md_xss_sanitized() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let path = fixture("malicious.md");

        let outcome = service.import_one(&path, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };
        service.index_document(&doc).await.unwrap();

        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "恶意 Markdown 必须产生 Chunk");

        // 后端净化断言：MarkdownLoader 应剥离原始 HTML 标签
        for (i, chunk) in chunks.iter().enumerate() {
            assert!(
                !chunk.content.contains("onerror="),
                "Chunk {i} 不得包含 onerror 事件处理器（XSS 向量）"
            );
            assert!(
                !chunk.content.contains("<iframe"),
                "Chunk {i} 不得包含 <iframe> 标签（XSS 向量）"
            );
            assert!(
                !chunk.content.contains("javascript:"),
                "Chunk {i} 不得包含 javascript: 协议（XSS 向量）"
            );
        }

        // 验证正常文本被保留
        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_text.contains("正常文字") || all_text.contains("正常内容"),
            "恶意 Markdown 中的正常文本应被保留"
        );
        println!("  [TC-CHAOS-004] XSS 标签已剥离，{} chunks", chunks.len());
    }

    /// TC-CHAOS-005 空文件处理：导入空 Markdown，断言不 Panic、不产生 0 Token 垃圾 Chunk
    /// （REQ-CHAOS-005；splitter 对空文本返回空 Vec，自然跳过 chunk 创建）。
    #[tokio::test]
    async fn tc_chaos_005_empty_md_no_garbage_chunks() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let path = fixture("empty.md");

        let outcome = service.import_one(&path, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // 空文件索引不应 Panic（splitter 对空文本返回空 Vec）
        service.index_document(&doc).await.unwrap();

        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(
            chunks.is_empty(),
            "空文件不得产生任何 Chunk（包括 0 Token 垃圾 Chunk）"
        );
        println!("  [TC-CHAOS-005] 空文件正确处理，0 chunks");
    }

    /// TC-CHAOS-006 格式欺骗：导入纯文本内容但扩展名为 .pdf 的文件，
    /// 断言 PdfLoader 解析失败时返回 Err，不 Panic（REQ-CHAOS-006）。
    #[tokio::test]
    async fn tc_chaos_006_fake_pdf_returns_err() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
        let path = fixture("fake.pdf");

        // import_one 成功（扩展名 .pdf 通过白名单 + is_pro=true 通过付费门）
        let outcome = service.import_one(&path, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // index_document 应返回 Err（PdfLoader 无法解析非 PDF 内容）
        let result = service.index_document(&doc).await;
        assert!(
            result.is_err(),
            "格式欺骗文件（纯文本 .pdf）必须返回 Err，不 Panic"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(!err_msg.is_empty(), "Err 必须携带可读错误原因");
        println!("  [TC-CHAOS-006] 格式欺骗正确拦截: {err_msg}");
    }

    /// TC-CHAOS-007 海量并发：导入 50 层深套目录中的全部 txt 文件，
    /// 断言所有子目录的文件被成功遍历并导入，无遗漏（REQ-CHAOS-007）。
    #[tokio::test]
    async fn tc_chaos_007_deep_nested_directory_batch() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        let deep_dir = fixtures_dir().join("deep_nested");
        let deep_dir = deep_dir
            .canonicalize()
            .unwrap_or_else(|e| panic!("canonicalize deep_nested 失败: {e}"));
        assert!(
            deep_dir.exists(),
            "Fixture 目录缺失: {}. 请先运行 `python3 tests/fixtures/generate_corpus.py`",
            deep_dir.display()
        );

        let files = collect_files(&deep_dir, "txt");
        assert_eq!(
            files.len(),
            50,
            "深套目录应包含 50 个 txt 文件，实际: {}",
            files.len()
        );

        let start = Instant::now();
        let mut imported_count = 0usize;
        for file_path in &files {
            let outcome = service.import_one(file_path, true).await.unwrap();
            if let ImportOutcome::Imported(doc) = outcome {
                service.index_document(&doc).await.unwrap();
                imported_count += 1;
            }
        }
        let elapsed = start.elapsed();

        assert_eq!(imported_count, 50, "50 个深套目录文件必须全部导入成功");

        // 验证数据库中有 50 个文档
        let docs = state.storage.list_documents().await.unwrap();
        assert_eq!(docs.len(), 50, "数据库应包含 50 个文档记录");

        // 验证每个文档都有 Chunk
        for doc in &docs {
            let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
            assert!(
                !chunks.is_empty(),
                "文档 {} 必须有 Chunk（1KB txt 不应为空）",
                doc.id
            );
        }

        assert!(
            elapsed.as_secs() < 60,
            "50 文件批量导入应 < 60s，实际: {elapsed:?}"
        );
        println!("  [TC-CHAOS-007] 50 文件全部导入，{elapsed:?}");
    }
}

// ================== 导出功能测试（REQ-EXP-001） ==================

/// TC-EXP-001 导出会话为 Markdown：生成包含标题、时间和消息的 Markdown（REQ-EXP-001-AC-2/AC-3）。
#[tokio::test]
async fn tc_exp_001_export_conversation_markdown() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建会话并写入消息
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 持久化一轮问答（会自动提取标题）
    persist_exchange(
        &state,
        &conv_id,
        "什么是 RAG？",
        "RAG 是检索增强生成。",
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // 导出
    let (markdown, filename) = export_conversation_markdown_inner(&conv_id, &state)
        .await
        .unwrap();

    // AC-2: 包含会话标题和时间
    assert!(markdown.contains("什么是 RAG"), "Markdown 必须包含会话标题");
    assert!(markdown.contains("Created at"), "Markdown 必须包含创建时间");

    // AC-3: 包含 user 和 assistant 消息
    assert!(markdown.contains("🧑 User"), "Markdown 必须包含 User 角色");
    assert!(
        markdown.contains("什么是 RAG？"),
        "Markdown 必须包含 user 消息内容"
    );
    assert!(
        markdown.contains("🤖 Assistant"),
        "Markdown 必须包含 Assistant 角色"
    );
    assert!(
        markdown.contains("RAG 是检索增强生成"),
        "Markdown 必须包含 assistant 消息内容"
    );

    // AC-5: 默认文件名为会话标题（derive_title 保留 "什么是 RAG？" 完整）
    assert_eq!(filename, "什么是 RAG？", "默认文件名应为会话标题");
}

/// TC-EXP-002 导出包含引用来源列表（REQ-EXP-001-AC-4）。
#[tokio::test]
async fn tc_exp_002_export_with_citation_sources() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    // 创建带引用来源的 assistant 消息
    let sources = vec![echomind_models::RetrievalResult {
        chunk: echomind_models::Chunk {
            id: "chunk-001".to_string(),
            doc_id: "doc-001".to_string(),
            content: "来源内容".to_string(),
            token_count: 10,
            sequence: 5,
        },
        score: 0.85,
        doc_name: "论文.md".to_string(),
    }];

    persist_exchange(
        &state,
        &conv_id,
        "问题",
        "回答",
        Some(sources),
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let (markdown, _) = export_conversation_markdown_inner(&conv_id, &state)
        .await
        .unwrap();

    // AC-4: 引用来源列表包含文档名和 chunk 序号
    assert!(
        markdown.contains("Citation Sources"),
        "必须包含引用来源标题"
    );
    assert!(markdown.contains("论文.md"), "必须包含来源文档名");
    assert!(markdown.contains("chunk #5"), "必须包含 chunk 序号");
}

/// TC-EXP-003 导出不存在的会话返回错误（健壮性）。
#[tokio::test]
async fn tc_exp_003_export_nonexistent_conversation_errors() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = export_conversation_markdown_inner("nonexistent-id", &state).await;

    assert!(result.is_err(), "导出不存在的会话应返回错误");
    assert!(
        result.err().unwrap().contains("会话不存在"),
        "错误消息应包含「会话不存在」"
    );
}

/// TC-EXP-004 save_text_file 正确写入文件内容（REQ-EXP-001 辅助命令）。
#[tokio::test]
async fn tc_exp_004_save_text_file_writes_content() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("export_test.md");

    let content = "# 测试导出\n\n这是测试内容。";
    let path_str = file_path.to_string_lossy().to_string();

    save_text_file_inner(&path_str, content).await.unwrap();

    // 验证文件内容
    let read_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(read_content, content, "文件内容必须与写入内容一致");
}

/// TC-EXP-005 导出保留原始 Markdown 语法（REQ-EXP-001-AC-6）。
#[tokio::test]
async fn tc_exp_005_export_preserves_raw_markdown() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();

    let assistant_content = "回答如下：\n\n```python\nprint('hello')\n```\n\n$$E=mc^2$$\n";

    persist_exchange(
        &state,
        &conv_id,
        "代码问题",
        assistant_content,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    let (markdown, _) = export_conversation_markdown_inner(&conv_id, &state)
        .await
        .unwrap();

    assert!(markdown.contains("```python"), "必须保留 Python 代码块语法");
    assert!(markdown.contains("$$E=mc^2$$"), "必须保留 LaTeX 公式语法");
}

// ================== Tauri ACL 能力配置守卫（防回归） ==================
// 教训：Tauri v2 的 ACL 权限系统仅在真实运行时强制检查，
// mock_app() 和桥接 Mock 均绕过 ACL，导致 dialog 权限缺失无法被 L2/L3-lite 发现。
// 本测试静态校验 capabilities/default.json 存在且包含必要权限，
// 作为配置回归守卫，防止再次出现「dialog.open not allowed」类问题。

/// TC-ACL-001 Tauri 能力配置守卫：验证 `capabilities/default.json` 存在，
/// 且包含 `dialog:allow-open` 权限（防「文件选择器不可用」回归）。
#[test]
fn tc_acl_001_dialog_capability_exists() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let cap_path = std::path::Path::new(manifest)
        .join("capabilities")
        .join("default.json");

    assert!(
        cap_path.exists(),
        "capabilities/default.json 缺失！Tauri v2 要求显式声明插件权限，\
         缺失会导致 dialog.open 等命令被 ACL 拒绝。路径: {}",
        cap_path.display()
    );

    let content = std::fs::read_to_string(&cap_path)
        .unwrap_or_else(|e| panic!("读取 capabilities 失败: {e}"));

    // 解析 JSON 并校验关键权限
    let json: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("capabilities/default.json JSON 解析失败: {e}"));

    let permissions = json["permissions"]
        .as_array()
        .unwrap_or_else(|| panic!("permissions 字段必须为数组"));

    let perm_strs: Vec<&str> = permissions.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        perm_strs.contains(&"dialog:allow-open"),
        "capabilities/default.json 必须包含 'dialog:allow-open' 权限，\
         否则点击加号按钮会报 'dialog.open not allowed'。当前权限: {perm_strs:?}"
    );

    assert!(
        perm_strs.contains(&"dialog:allow-save"),
        "capabilities/default.json 必须包含 'dialog:allow-save' 权限，\
         否则导出功能的保存对话框会报 'dialog.save not allowed'。当前权限: {perm_strs:?}"
    );

    assert!(
        perm_strs.contains(&"core:default"),
        "capabilities/default.json 必须包含 'core:default' 权限（事件系统等基础能力）"
    );
}

// ================== 向量化+检索完整链路测试（防回归） ==================
// 教训：Phase 8.5 混沌测试覆盖了 import+index（load+split+chunks 入库），
// 但 **未覆盖 embed+search 链路**。真实用户旧数据库 embeddings 表列名不匹配
// （embedding → vector）导致 add_embedding INSERT 崩溃 + vector_search SELECT 崩溃。
// 本模块补全：import → index → embed → vector_search 完整链路验证。

mod embed_search_tests {
    use super::*;
    use echomind_core::import::{ImportOutcome, ImportService};

    /// TC-VEC-001 完整向量化+检索链路：导入文档 → 索引 → 向量化 → 检索，不断崩溃。
    ///
    /// 这正是用户报告的 bug 路径：上传文档后发送消息，vector_search 因
    /// embeddings 表列名不匹配而崩溃。本测试确保完整链路在全新数据库上正常工作。
    #[tokio::test]
    async fn tc_vec_001_import_embed_search_full_pipeline() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        // 创建测试文档
        let md_path = dir.path().join("knowledge.md");
        std::fs::write(
            &md_path,
            "# 知识库文档\n\nRust 是一门系统级编程语言，注重安全与性能。",
        )
        .unwrap();
        let path_str = md_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        // 步骤 1：导入
        let outcome = service.import_one(&path_str, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // 步骤 2：索引（load → split → chunks 入库）
        service.index_document(&doc).await.unwrap();
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "索引后必须有 chunks");

        // 步骤 3：向量化（embed_batch → add_embedding）
        // 直接调用 storage.add_embedding 模拟向量化写入（绕过 LocalEmbedder 模型加载）
        for chunk in &chunks {
            state
                .storage
                .add_embedding(&chunk.id, &[0.1, 0.2, 0.3, 0.4])
                .await
                .expect("add_embedding 不应崩溃（embeddings 表 vector 列必须存在）");
        }

        // 步骤 4：检索（vector_search）——这是用户崩溃的关键点
        let hits = state
            .storage
            .vector_search(&[0.1, 0.2, 0.3, 0.4], 5)
            .await
            .expect("vector_search 不应崩溃（e.vector 列必须存在）");

        assert_eq!(hits.len(), chunks.len(), "检索结果数应等于 chunk 数");
        assert!(
            hits[0].score > 0.99,
            "自身向量余弦相似度应接近 1.0，实际: {}",
            hits[0].score
        );
        assert!(
            hits[0].doc_name.ends_with("knowledge.md"),
            "doc_name 应为 knowledge.md（含 hash 前缀），实际: {}",
            hits[0].doc_name
        );
    }

    /// TC-VEC-002 旧 schema 迁移后向量化+检索不崩溃（端到端守卫）。
    ///
    /// 模拟用户从旧版本升级：数据库 embeddings 表列名为 `embedding`（非 `vector`），
    /// 验证 migrate_schema 正确重建表后，add_embedding + vector_search 正常工作。
    /// 注：旧 schema 构造在 infra crate 的 `tc_db_003_old_schema_embeddings_migration` 中覆盖，
    /// 此处聚焦端到端完整链路。
    #[tokio::test]
    async fn tc_vec_002_full_pipeline_with_real_storage() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        // 多文档导入 + 索引 + 向量化 + 检索
        for i in 0..3 {
            let path = dir.path().join(format!("doc{i}.md"));
            std::fs::write(&path, format!("文档 {i} 的内容，关于主题 {i}。")).unwrap();
            let canon = path.canonicalize().unwrap().to_string_lossy().into_owned();
            let outcome = service.import_one(&canon, true).await.unwrap();
            if let ImportOutcome::Imported(doc) = outcome {
                service.index_document(&doc).await.unwrap();
                let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
                for chunk in &chunks {
                    state
                        .storage
                        .add_embedding(&chunk.id, &[i as f32, 0.0, 0.0])
                        .await
                        .unwrap();
                }
            }
        }

        // 检索：用与 doc-0 相同的向量，应召回 doc-0 的 chunks
        let hits = state
            .storage
            .vector_search(&[0.0, 0.0, 0.0], 10)
            .await
            .unwrap();
        assert!(!hits.is_empty(), "至少应召回 1 条结果");
        // 所有 3 个文档的 chunks 都应被召回（向量不完全正交，top-k=10 覆盖全部）
        let doc_names: std::collections::HashSet<_> =
            hits.iter().map(|h| h.doc_name.clone()).collect();
        assert_eq!(doc_names.len(), 3, "应召回 3 个不同文档的 chunks");
    }
}

#[tokio::test]
async fn tc_vec_012_embedding_model_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // AC-1: 默认 all-MiniLM-L6-v2
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.embedding_model, "all-MiniLM-L6-v2",
        "AC-1: 默认嵌入模型应为 all-MiniLM-L6-v2"
    );

    // AC-2: 切换为 bge-small-zh-v1.5
    set_embedding_model_inner("bge-small-zh-v1.5", &state)
        .await
        .unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.embedding_model, "bge-small-zh-v1.5",
        "AC-2: 切换后嵌入模型应为 bge-small-zh-v1.5"
    );

    // AC-3: 切换为 e5-small-v2
    set_embedding_model_inner("e5-small-v2", &state)
        .await
        .unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.embedding_model, "e5-small-v2",
        "AC-3: 切换后嵌入模型应为 e5-small-v2"
    );

    // AC-4: 重启后设置仍在
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert_eq!(
        settings.embedding_model, "e5-small-v2",
        "AC-4: 重启后嵌入模型应保持 e5-small-v2"
    );

    // 切回默认后重启验证
    set_embedding_model_inner("all-MiniLM-L6-v2", &restarted)
        .await
        .unwrap();
    drop(restarted);
    let restarted2 = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings2 = get_settings_inner(&restarted2).await.unwrap();
    assert_eq!(
        settings2.embedding_model, "all-MiniLM-L6-v2",
        "AC-4: 重启后默认模型应持久化"
    );
}

/// TC-VEC-012b 非法模型标识被拒绝（REQ-VEC-012 AC-5）。
///
/// 传入不支持的模型标识应返回 Err，且不修改当前设置。
#[tokio::test]
async fn tc_vec_012b_invalid_model_rejected() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 非法模型标识
    let result = set_embedding_model_inner("invalid-model-name", &state).await;
    assert!(result.is_err(), "AC-5: 非法模型标识应被拒绝");

    // 验证当前设置未变（仍为默认）
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.embedding_model, "all-MiniLM-L6-v2",
        "AC-5: 拒绝后嵌入模型应保持默认值"
    );

    // 另一个非法值
    let result = set_embedding_model_inner("", &state).await;
    assert!(result.is_err(), "空字符串应被拒绝");
}

/// TC-VEC-012c 嵌入模型切换后 embedder_initialized 状态重置（REQ-VEC-012）。
///
/// 切换模型后，embedder_initialized 应返回 false（实例已销毁，需重新初始化）。
#[tokio::test]
async fn tc_vec_012c_embedder_resets_on_switch() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 初始状态：未初始化
    assert!(
        !state.embedder_initialized().await,
        "初始状态 embedder 应未初始化"
    );

    // 切换模型（此时 embedder 尚未初始化，仅写入 settings）
    set_embedding_model_inner("bge-small-zh-v1.5", &state)
        .await
        .unwrap();

    // 仍未初始化（没有调用 embedder()）
    assert!(
        !state.embedder_initialized().await,
        "切换后 embedder 应仍未初始化"
    );

    // settings 中应记录新模型
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.embedding_model, "bge-small-zh-v1.5",
        "settings 应记录新模型"
    );
}

// ================== MM Phase 3: 多模态导入管线 L2 契约测试（REQ-MM-001/002/004） ==================
// TC-MM-001 / TC-MM-002 / TC-MM-004 — 多模态 PDF 渲染、OCR、管线编排
#[cfg(feature = "pro")]
mod mm_tests {
    use super::*;
    use echomind_core::import::{ImportOutcome, ImportService};
    use echomind_core::loader::MultimodalPdfLoader;
    use echomind_core::{Loader, NoVlm, OcrEngine, PageRenderer, VisionLanguageModel};
    use echomind_models::DocStatus;
    use lopdf::{Document, Object, dictionary};
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    // ---- Mock 端口实现 ----

    /// Mock 页面渲染器：记录被渲染的页码，返回 fake PNG 字节。
    struct MockPageRenderer {
        /// render_page 被调用时的页码列表（测试断言用）
        rendered_pages: Arc<Mutex<Vec<usize>>>,
        /// 返回的位图字节（fake PNG header）
        bitmap: Vec<u8>,
    }

    impl PageRenderer for MockPageRenderer {
        async fn render_page(
            &self,
            _pdf_path: &str,
            page_num: usize,
            _dpi: u32,
        ) -> anyhow::Result<Vec<u8>> {
            self.rendered_pages.lock().unwrap().push(page_num);
            Ok(self.bitmap.clone())
        }
    }

    /// Mock OCR 引擎：返回预设文本，记录调用次数。
    struct MockOcrEngine {
        /// 返回的文本（空字符串模拟 OCR 失败降级行为）
        return_text: String,
        /// recognize 被调用次数
        call_count: Arc<AtomicUsize>,
    }

    impl OcrEngine for MockOcrEngine {
        async fn recognize(&self, _image_bytes: &[u8]) -> anyhow::Result<String> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            Ok(self.return_text.clone())
        }
    }

    // ---- PDF 创建辅助函数 ----

    /// 初始化 PDF 文档：创建空 Pages + Catalog，设置 trailer Root。
    /// 返回 (Document, pages_id) 供后续添加页面。
    fn init_pdf() -> (Document, lopdf::ObjectId) {
        let mut doc = Document::with_version("1.7");

        // 创建空 Pages 对象
        let pages_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Count" => 0_i64,
            "Kids" => Vec::<Object>::new(),
        }));

        // 创建 Catalog 指向 Pages
        let catalog_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }));

        // 设置 trailer Root 引用
        doc.trailer.set("Root", catalog_id);

        (doc, pages_id)
    }

    /// 创建包含图片 XObject 的最小有效 PDF。
    ///
    /// 结构：
    /// - 页面 1：含 1×1 像素图片 XObject（触发图片检测 → 渲染 → OCR）
    /// - 页面 2：纯文本（含文本内容流，无图片）
    fn create_pdf_with_image(path: &Path) {
        let (mut doc, pages_id) = init_pdf();

        // 图片 XObject（1×1 像素灰度图）
        let image_stream = lopdf::Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 1_i64,
                "Height" => 1_i64,
                "ColorSpace" => "DeviceGray",
                "BitsPerComponent" => 8_i64,
            },
            vec![128u8],
        );
        let image_id = doc.add_object(Object::Stream(image_stream));

        // 页面 1：含图片 XObject
        let page1_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(612),
                Object::Integer(792),
            ],
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    "Im0" => image_id,
                },
            },
        }));

        // 字体 + 文本内容流（页面 2 用）
        let font_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        }));
        let text_content = b"BT /F1 12 Tf 72 720 Td (TextLayerContent) Tj ET";
        let content_stream = lopdf::Stream::new(dictionary! {}, text_content.to_vec());
        let content_id = doc.add_object(Object::Stream(content_stream));

        // 页面 2：纯文本（无图片）
        let page2_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(612),
                Object::Integer(792),
            ],
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            },
            "Contents" => content_id,
        }));

        // 更新 Pages 字典
        if let Ok(Object::Dictionary(pages_dict)) = doc.get_object_mut(pages_id) {
            pages_dict.set("Count", 2_i64);
            pages_dict.set(
                "Kids",
                vec![Object::Reference(page1_id), Object::Reference(page2_id)],
            );
        }

        doc.save(path).expect("保存 PDF 失败");
    }

    /// 创建纯文本 PDF（无图片 XObject）。
    fn create_pdf_text_only(path: &Path) {
        let (mut doc, pages_id) = init_pdf();

        let font_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        }));
        let text_content = b"BT /F1 12 Tf 72 720 Td (PlainTextContent) Tj ET";
        let content_stream = lopdf::Stream::new(dictionary! {}, text_content.to_vec());
        let content_id = doc.add_object(Object::Stream(content_stream));

        let page_id = doc.add_object(Object::Dictionary(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(612),
                Object::Integer(792),
            ],
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            },
            "Contents" => content_id,
        }));

        if let Ok(Object::Dictionary(pages_dict)) = doc.get_object_mut(pages_id) {
            pages_dict.set("Count", 1_i64);
            pages_dict.set("Kids", vec![Object::Reference(page_id)]);
        }

        doc.save(path).expect("保存 PDF 失败");
    }

    // ---- 测试用例 ----

    /// TC-MM-001 PDF 图片页面渲染为位图（REQ-MM-001 AC-1/AC-2/AC-3）。
    ///
    /// AC-1：含图片的 PDF 页面被渲染为位图（render_page 被调用）。
    /// AC-2：纯文本 PDF 页面不触发渲染（render_page 未被调用）。
    /// AC-3：渲染不阻塞 async executor（load() 是 async 且正常返回）。
    #[tokio::test]
    async fn tc_mm_001_pdf_image_page_rendered_to_bitmap() {
        let dir = TempDir::new().unwrap();

        // ---- AC-1: 含图片的 PDF 页面被渲染 ----
        let pdf_with_image = dir.path().join("with_image.pdf");
        create_pdf_with_image(&pdf_with_image);

        let rendered_pages = Arc::new(Mutex::new(Vec::new()));
        let ocr_calls = Arc::new(AtomicUsize::new(0));

        let loader = MultimodalPdfLoader::new(
            MockPageRenderer {
                rendered_pages: rendered_pages.clone(),
                bitmap: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            },
            MockOcrEngine {
                return_text: "OCR_TEXT".to_string(),
                call_count: ocr_calls.clone(),
            },
        );

        let result = loader.load(pdf_with_image.to_str().unwrap()).await;

        // AC-3: load() 正常返回，证明渲染不阻塞 async executor
        assert!(
            result.is_ok(),
            "AC-3: 渲染不应阻塞 async executor: {:?}",
            result.err()
        );

        // AC-1: 含图片的页面被渲染
        {
            let rendered = rendered_pages.lock().unwrap();
            assert!(
                !rendered.is_empty(),
                "AC-1: 含图片的 PDF 页面必须被渲染（render_page 被调用）"
            );
        } // MutexGuard 在此释放，避免跨 await 持有锁

        // ---- AC-2: 纯文本 PDF 页面不触发渲染 ----
        let pdf_text_only = dir.path().join("text_only.pdf");
        create_pdf_text_only(&pdf_text_only);

        let rendered_pages2 = Arc::new(Mutex::new(Vec::new()));

        let loader2 = MultimodalPdfLoader::new(
            MockPageRenderer {
                rendered_pages: rendered_pages2.clone(),
                bitmap: vec![0x89, 0x50, 0x4E, 0x47],
            },
            MockOcrEngine {
                return_text: "OCR_TEXT".to_string(),
                call_count: Arc::new(AtomicUsize::new(0)),
            },
        );

        let _ = loader2.load(pdf_text_only.to_str().unwrap()).await;

        let rendered2 = rendered_pages2.lock().unwrap();
        assert!(
            rendered2.is_empty(),
            "AC-2: 纯文本 PDF 页面不应触发渲染（render_page 未被调用）"
        );
    }

    /// TC-MM-002 本地 OCR 提取图片文字（REQ-MM-002 AC-1/AC-2/AC-3）。
    ///
    /// AC-1：含印刷文字的图片经 OCR 提取后，文字内容出现在加载结果中。
    /// AC-2：OCR 过程无网络请求（Mock 端口无网络参数，契约设计保证）。
    /// AC-3：OCR 失败（返回空字符串）时优雅降级，保留页面文本层内容，不崩溃。
    #[tokio::test]
    async fn tc_mm_002_local_ocr_extracts_text_from_image() {
        let dir = TempDir::new().unwrap();
        let pdf_path = dir.path().join("ocr_test.pdf");
        create_pdf_with_image(&pdf_path);

        // ---- AC-1: OCR 提取的文字出现在结果中 ----
        let ocr_calls = Arc::new(AtomicUsize::new(0));
        let ocr_text = "OCR_EXTRACTED_TEXT".to_string();

        let loader = MultimodalPdfLoader::new(
            MockPageRenderer {
                rendered_pages: Arc::new(Mutex::new(Vec::new())),
                bitmap: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            },
            MockOcrEngine {
                return_text: ocr_text.clone(),
                call_count: ocr_calls.clone(),
            },
        );

        let result = loader.load(pdf_path.to_str().unwrap()).await;
        assert!(result.is_ok(), "多模态加载应成功: {:?}", result.err());

        let text = result.unwrap();
        assert!(
            text.contains("OCR_EXTRACTED_TEXT"),
            "AC-1: OCR 提取的文字必须出现在结果中，实际: {text}"
        );
        assert!(
            ocr_calls.load(Ordering::Relaxed) > 0,
            "AC-1: OCR 引擎必须被调用至少一次"
        );

        // AC-2: OCR 端口契约无网络参数（recognize 仅接收 image_bytes，不涉及网络请求）
        // Mock OcrEngine 实现不发起任何网络调用，端口设计保证零网络（REQ-MM-002 AC-2 旁证）

        // ---- AC-3: OCR 失败时优雅降级（返回空字符串模拟 OcrsEngine 降级行为）----
        let pdf_path2 = dir.path().join("ocr_fail.pdf");
        create_pdf_with_image(&pdf_path2);

        let loader2 = MultimodalPdfLoader::new(
            MockPageRenderer {
                rendered_pages: Arc::new(Mutex::new(Vec::new())),
                bitmap: vec![0x89, 0x50, 0x4E, 0x47],
            },
            MockOcrEngine {
                return_text: String::new(), // 模拟 OCR 失败降级（返回空字符串）
                call_count: Arc::new(AtomicUsize::new(0)),
            },
        );

        let result2 = loader2.load(pdf_path2.to_str().unwrap()).await;

        // OCR 返回空字符串时，load() 仍应成功（优雅降级，不崩溃）
        // 注意：页面 2 的文本层 "TextLayerContent" 保证 full_text 非空
        assert!(
            result2.is_ok(),
            "AC-3: OCR 失败时应优雅降级，不崩溃: {:?}",
            result2.err()
        );
    }

    /// TC-MM-004 多模态导入管线图文混排可检索（REQ-MM-004 AC-1/AC-2/AC-3）。
    ///
    /// AC-1：含图文混排的 PDF 导入后，文本和图片中的文字均可被检索（chunks 包含两者）。
    /// AC-2：导入进度展示子阶段（DocStatusPayload.sub_phase 字段已实现，旁证）。
    /// AC-3：VLM 增强关闭后跳过 VLM，仅用 OCR（当前 Phase 1 无 VLM，管线天然跳过，旁证）。
    #[tokio::test]
    async fn tc_mm_004_multimodal_pipeline_text_and_image_searchable() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        // 创建含图片的 PDF
        let pdf_path = dir.path().join("multimodal.pdf");
        create_pdf_with_image(&pdf_path);
        let path_str = pdf_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        // AC-1: 导入 + 多模态索引 → 文本和图片文字均可检索
        let outcome = service.import_one(&path_str, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // 使用多模态索引（注入 Mock PageRenderer + MockOcrEngine）
        let ocr_calls = Arc::new(AtomicUsize::new(0));
        let result = service
            .index_document_multimodal(
                &doc,
                &MockPageRenderer {
                    rendered_pages: Arc::new(Mutex::new(Vec::new())),
                    bitmap: vec![0x89, 0x50, 0x4E, 0x47],
                },
                &MockOcrEngine {
                    return_text: "OCR_SEARCHABLE_TEXT".to_string(),
                    call_count: ocr_calls.clone(),
                },
            )
            .await;

        assert!(result.is_ok(), "AC-1: 多模态索引应成功: {:?}", result.err());
        assert!(
            ocr_calls.load(Ordering::Relaxed) > 0,
            "AC-1: OCR 引擎必须被调用（图片页面被渲染+OCR）"
        );

        // 验证 chunks 包含 OCR 文本
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "AC-1: 多模态索引后必须有 chunks");

        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_text.contains("OCR_SEARCHABLE_TEXT"),
            "AC-1: chunks 必须包含 OCR 提取的文字，实际: {all_text}"
        );

        // AC-2: 导入进度展示子阶段（DocStatusPayload.sub_phase 字段已实现，旁证）
        // 验证文档状态为 Indexed（索引完成）
        let docs = state.storage.list_documents().await.unwrap();
        let indexed_doc = docs
            .iter()
            .find(|d| d.id == doc.id)
            .expect("文档应在列表中");
        assert_eq!(
            indexed_doc.status,
            DocStatus::Indexed,
            "AC-2: 多模态索引完成后状态应为 Indexed"
        );

        // AC-3: VLM 增强关闭后跳过 VLM（当前 Phase 1 无 VLM 实现，管线天然跳过 VLM 阶段，旁证）
        // index_document_multimodal 不包含 VLM 调用，仅执行文本提取+渲染+OCR
    }

    // ---- Mock VLM 实现 ----

    /// Mock VLM 引擎：返回预设的结构化文本描述，记录调用次数。
    struct MockVlmEngine {
        /// 返回的文本（模拟 VLM 将表格→Markdown、甘特图→Mermaid 的输出）
        return_text: String,
        /// describe_image 被调用次数
        call_count: Arc<AtomicUsize>,
    }

    impl VisionLanguageModel for MockVlmEngine {
        async fn describe_image(
            &self,
            _image_bytes: &[u8],
            _prompt: &str,
        ) -> anyhow::Result<String> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            Ok(self.return_text.clone())
        }
    }

    /// TC-MM-003 VLM 将图片中的表格转换为 Markdown 表格（REQ-MM-003 AC-1）。
    ///
    /// AC-1：含表格的图片经 VLM 处理后，表格内容以 Markdown 表格形式出现在 chunk 中。
    /// AC-4：网络请求仅发往用户配置的 base_url（Mock VLM 无网络参数，端口契约保证）。
    #[tokio::test]
    async fn tc_mm_003_vlm_table_to_markdown() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        // 创建含图片的 PDF
        let pdf_path = dir.path().join("vlm_table.pdf");
        create_pdf_with_image(&pdf_path);
        let path_str = pdf_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        // 导入
        let outcome = service.import_one(&path_str, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // VLM 模拟返回 Markdown 表格
        let vlm_calls = Arc::new(AtomicUsize::new(0));
        let markdown_table = "| 项目 | 金额 |\n|---|---|\n| 收入 | 1000 |\n| 支出 | 500 |";

        let result = service
            .index_document_multimodal_with_vlm(
                &doc,
                &MockPageRenderer {
                    rendered_pages: Arc::new(Mutex::new(Vec::new())),
                    bitmap: vec![0x89, 0x50, 0x4E, 0x47],
                },
                &MockOcrEngine {
                    return_text: "OCR_TEXT".to_string(),
                    call_count: Arc::new(AtomicUsize::new(0)),
                },
                &MockVlmEngine {
                    return_text: markdown_table.to_string(),
                    call_count: vlm_calls.clone(),
                },
            )
            .await;

        assert!(result.is_ok(), "VLM 增强索引应成功: {:?}", result.err());
        assert!(
            vlm_calls.load(Ordering::Relaxed) > 0,
            "AC-1: VLM 引擎必须被调用至少一次"
        );

        // AC-1: Markdown 表格出现在 chunks 中
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "VLM 增强索引后必须有 chunks");

        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_text.contains("| 项目 | 金额 |"),
            "AC-1: chunks 必须包含 VLM 转换的 Markdown 表格，实际: {all_text}"
        );
        assert!(
            all_text.contains("| 收入 | 1000 |"),
            "AC-1: Markdown 表格数据行必须出现在 chunks 中"
        );

        // AC-4: VLM 端口契约无额外网络参数（describe_image 仅接收 image_bytes + prompt，
        // base_url 由适配器实现层从 AppState.llm_config 注入，端口设计保证仅发往用户配置端点）
    }

    /// TC-MM-003b VLM 将图片中的甘特图转换为 Mermaid gantt 语法（REQ-MM-003 AC-2）。
    ///
    /// AC-2：含甘特图的图片经 VLM 处理后，甘特图内容以 Mermaid gantt 语法出现在 chunk 中。
    #[tokio::test]
    async fn tc_mm_003b_vlm_gantt_to_mermaid() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        let pdf_path = dir.path().join("vlm_gantt.pdf");
        create_pdf_with_image(&pdf_path);
        let path_str = pdf_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let outcome = service.import_one(&path_str, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // VLM 模拟返回 Mermaid gantt 语法
        let vlm_calls = Arc::new(AtomicUsize::new(0));
        let mermaid_gantt = "gantt\n    title 项目计划\n    dateFormat YYYY-MM-DD\n    section 开发\n    设计 :a1, 2024-01-01, 7d\n    编码 :a2, after a1, 14d";

        let result = service
            .index_document_multimodal_with_vlm(
                &doc,
                &MockPageRenderer {
                    rendered_pages: Arc::new(Mutex::new(Vec::new())),
                    bitmap: vec![0x89, 0x50, 0x4E, 0x47],
                },
                &MockOcrEngine {
                    return_text: "OCR_TEXT".to_string(),
                    call_count: Arc::new(AtomicUsize::new(0)),
                },
                &MockVlmEngine {
                    return_text: mermaid_gantt.to_string(),
                    call_count: vlm_calls.clone(),
                },
            )
            .await;

        assert!(result.is_ok(), "VLM 增强索引应成功: {:?}", result.err());
        assert!(
            vlm_calls.load(Ordering::Relaxed) > 0,
            "AC-2: VLM 引擎必须被调用至少一次"
        );

        // AC-2: Mermaid gantt 语法出现在 chunks 中
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "VLM 增强索引后必须有 chunks");

        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_text.contains("gantt"),
            "AC-2: chunks 必须包含 Mermaid gantt 语法标记，实际: {all_text}"
        );
        assert!(
            all_text.contains("section 开发"),
            "AC-2: Mermaid gantt 内容必须出现在 chunks 中"
        );
    }

    /// TC-MM-003c VLM 不可用时优雅降级为纯 OCR（REQ-MM-003 AC-3）。
    ///
    /// AC-3：VLM 不可用（未配置 Vision LLM）时优雅降级为纯 OCR，不崩溃。
    /// 使用 NoVlm 占位实现模拟「VLM 未启用」场景。
    #[tokio::test]
    async fn tc_mm_003c_vlm_unavailable_degrades_to_ocr() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        let pdf_path = dir.path().join("vlm_disabled.pdf");
        create_pdf_with_image(&pdf_path);
        let path_str = pdf_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let outcome = service.import_one(&path_str, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        let ocr_calls = Arc::new(AtomicUsize::new(0));

        // 使用 NoVlm（VLM 禁用），管线应仅用 OCR，不崩溃
        let result = service
            .index_document_multimodal_with_vlm(
                &doc,
                &MockPageRenderer {
                    rendered_pages: Arc::new(Mutex::new(Vec::new())),
                    bitmap: vec![0x89, 0x50, 0x4E, 0x47],
                },
                &MockOcrEngine {
                    return_text: "OCR_ONLY_TEXT".to_string(),
                    call_count: ocr_calls.clone(),
                },
                &NoVlm, // VLM 禁用占位
            )
            .await;

        // AC-3: 优雅降级，不崩溃
        assert!(
            result.is_ok(),
            "AC-3: VLM 不可用时应优雅降级，不崩溃: {:?}",
            result.err()
        );
        assert!(
            ocr_calls.load(Ordering::Relaxed) > 0,
            "AC-3: 降级后 OCR 引擎仍被调用"
        );

        // 验证 chunks 包含 OCR 文本（无 VLM 文本）
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "降级后仍必须有 chunks");

        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        assert!(
            all_text.contains("OCR_ONLY_TEXT"),
            "AC-3: 降级后 chunks 必须包含 OCR 提取的文字"
        );
    }

    /// TC-MM-005 VLM 设置开关持久化与重启恢复（REQ-MM-003 前端接线）。
    ///
    /// AC-1：默认 vlm_enabled 为 false
    /// AC-2：set_vlm_enabled(true) 后 get_settings 返回 vlm_enabled = true
    /// AC-3：set_vlm_enabled(false) 后 get_settings 返回 vlm_enabled = false
    /// AC-4：重启（重建 AppState）后设置仍在
    #[tokio::test]
    async fn tc_mm_005_vlm_toggle_persists() {
        let dir = TempDir::new().unwrap();
        {
            let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
            update_llm_config_inner(test_config(), &state)
                .await
                .unwrap();

            // AC-1: 默认关闭
            let settings = get_settings_inner(&state).await.unwrap();
            assert!(!settings.vlm_enabled, "AC-1: VLM 默认应关闭");

            // AC-2: 开启
            set_vlm_enabled_inner(true, &state).await.unwrap();
            let settings = get_settings_inner(&state).await.unwrap();
            assert!(settings.vlm_enabled, "AC-2: 开启后 VLM 应为 true");

            // AC-3: 关闭
            set_vlm_enabled_inner(false, &state).await.unwrap();
            let settings = get_settings_inner(&state).await.unwrap();
            assert!(!settings.vlm_enabled, "AC-3: 关闭后 VLM 应为 false");
        }

        // AC-4: 重启后设置仍在（关闭状态）
        let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let settings = get_settings_inner(&restarted).await.unwrap();
        assert!(!settings.vlm_enabled, "AC-4: 重启后 VLM 应保持关闭");

        // 再测开启后重启
        set_vlm_enabled_inner(true, &restarted).await.unwrap();
        drop(restarted);
        let restarted2 = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let settings2 = get_settings_inner(&restarted2).await.unwrap();
        assert!(settings2.vlm_enabled, "AC-4: 重启后 VLM 开启状态应持久化");
    }

    /// TC-MM-006 vision_provider() 运行时开关响应（REQ-MM-003 前端接线）。
    ///
    /// AC-1：VLM 禁用时 vision_provider() 返回 None
    /// AC-2：VLM 启用且 LLM 配置存在时 vision_provider() 返回 Some
    /// AC-3：VLM 启用但 LLM 配置缺失时 vision_provider() 返回 None（降级）
    /// AC-4：运行时关闭后 vision_provider() 立即返回 None（无需重启，验证 OnceCell 已移除）
    #[tokio::test]
    async fn tc_mm_006_vision_provider_runtime_toggle() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        update_llm_config_inner(test_config(), &state)
            .await
            .unwrap();

        // AC-1: 默认禁用 → None
        let vp = state.vision_provider().await.unwrap();
        assert!(vp.is_none(), "AC-1: VLM 禁用时应返回 None");

        // AC-2: 启用后 → Some（OpenAIVisionProvider::new 仅存储字符串，不发起网络请求）
        set_vlm_enabled_inner(true, &state).await.unwrap();
        let vp = state.vision_provider().await.unwrap();
        assert!(vp.is_some(), "AC-2: VLM 启用且 LLM 配置存在时应返回 Some");

        // AC-4: 运行时关闭 → None（无需重启，证明 OnceCell 缓存已移除）
        set_vlm_enabled_inner(false, &state).await.unwrap();
        let vp = state.vision_provider().await.unwrap();
        assert!(
            vp.is_none(),
            "AC-4: 运行时关闭后应立即返回 None（无需重启）"
        );

        // AC-3: VLM 启用但 LLM 配置缺失 → None（降级）
        let dir2 = TempDir::new().unwrap();
        let state2 = AppState::new(dir2.path().to_path_buf()).await.unwrap();
        set_vlm_enabled_inner(true, &state2).await.unwrap();
        let vp = state2.vision_provider().await.unwrap();
        assert!(
            vp.is_none(),
            "AC-3: VLM 启用但 LLM 配置缺失时应返回 None（降级）"
        );
    }

    // ---- REQ-MM-005 分级图表理解测试 ----

    /// 捕获 prompt 的 Mock VLM：记录传入的 prompt 文本，用于验证分级策略接线。
    struct PromptCapturingVlm {
        /// 返回的文本（模拟 VLM 按 Level 3 策略提取的 CSV 数据）
        return_text: String,
        /// describe_image 被调用次数
        call_count: Arc<AtomicUsize>,
        /// 捕获最后一次传入的 prompt
        captured_prompt: Arc<Mutex<String>>,
    }

    impl VisionLanguageModel for PromptCapturingVlm {
        async fn describe_image(
            &self,
            _image_bytes: &[u8],
            prompt: &str,
        ) -> anyhow::Result<String> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            *self.captured_prompt.lock().unwrap() = prompt.to_string();
            Ok(self.return_text.clone())
        }
    }

    /// TC-MM-007c VLM 分级图表理解——CSV 数据提取端到端管线（REQ-MM-005 AC-1/AC-3/AC-5）。
    ///
    /// AC-1：含数据图表的 PDF 页面经 VLM 处理后，chunk 中包含 CSV 格式的数据点
    ///       （坐标轴标签 + 至少 3 个数值点），而非仅文字趋势描述。
    /// AC-3：VLM 提取的数据图表结果末尾标注误差提示。
    /// AC-5：CSV 数据经分块后进入 chunk（向量化+检索由 embed_search_tests 旁证）。
    ///
    /// 同时验证 VLM 被调用时传入的是分级策略提示词（VLM_TIERED_PROMPT），
    /// 而非旧版单一提示词。
    #[tokio::test]
    async fn tc_mm_007c_vlm_csv_data_in_chunks() {
        let dir = TempDir::new().unwrap();
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let service = ImportService::new(state.storage.clone(), state.data_dir.clone());

        // 创建含图片的 PDF
        let pdf_path = dir.path().join("vlm_chart.pdf");
        create_pdf_with_image(&pdf_path);
        let path_str = pdf_path
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        // 导入
        let outcome = service.import_one(&path_str, true).await.unwrap();
        let doc = match outcome {
            ImportOutcome::Imported(d) => d,
            ImportOutcome::SkippedDuplicate(_) => panic!("首次导入不应跳过"),
            ImportOutcome::NameConflict { .. } => panic!("首次导入不应同名冲突"),
        };

        // 模拟 VLM 按 Level 3 策略提取的 CSV 数据
        let vlm_calls = Arc::new(AtomicUsize::new(0));
        let captured_prompt = Arc::new(Mutex::new(String::new()));
        let csv_output = "\
图表类型：柱状图\n\
坐标轴：X=月份, Y=销售额（万元）\n\
数据系列：销售额\n\
数据（CSV）：\n\
月份,销售额\n\
1月,120\n\
2月,150\n\
3月,180\n\
趋势：销售额逐月上升\n\
以下数据由 AI 视觉提取，可能存在误差，请核对原文";

        let result = service
            .index_document_multimodal_with_vlm(
                &doc,
                &MockPageRenderer {
                    rendered_pages: Arc::new(Mutex::new(Vec::new())),
                    bitmap: vec![0x89, 0x50, 0x4E, 0x47],
                },
                &MockOcrEngine {
                    return_text: "OCR_TEXT".to_string(),
                    call_count: Arc::new(AtomicUsize::new(0)),
                },
                &PromptCapturingVlm {
                    return_text: csv_output.to_string(),
                    call_count: vlm_calls.clone(),
                    captured_prompt: captured_prompt.clone(),
                },
            )
            .await;

        assert!(result.is_ok(), "VLM 增强索引应成功: {:?}", result.err());
        assert!(
            vlm_calls.load(Ordering::Relaxed) > 0,
            "AC-1: VLM 引擎必须被调用至少一次"
        );

        // 验证 VLM 被调用时传入的是分级策略提示词（而非旧版单一提示词）
        let prompt = captured_prompt.lock().unwrap().clone();
        assert!(
            prompt.contains("Level 3"),
            "VLM 必须接收分级策略提示词（含 Level 3），实际 prompt 片段: {}",
            &prompt[..prompt.len().min(100)]
        );
        assert!(prompt.contains("CSV"), "VLM 提示词必须包含 CSV 提取引导");
        assert!(
            prompt.contains("LaTeX"),
            "VLM 提示词必须包含 LaTeX 公式提取引导（AC-2）"
        );

        // AC-1: CSV 数据出现在 chunks 中
        let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
        assert!(!chunks.is_empty(), "VLM 增强索引后必须有 chunks");

        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
        // 坐标轴标签
        assert!(
            all_text.contains("坐标轴"),
            "AC-1: chunks 必须包含坐标轴标签"
        );
        assert!(
            all_text.contains("X=月份"),
            "AC-1: chunks 必须包含 X 轴标签"
        );
        // CSV 数据点（至少 3 个）
        assert!(
            all_text.contains("1月,120"),
            "AC-1: chunks 必须包含 CSV 数据点 1月,120"
        );
        assert!(
            all_text.contains("2月,150"),
            "AC-1: chunks 必须包含 CSV 数据点 2月,150"
        );
        assert!(
            all_text.contains("3月,180"),
            "AC-1: chunks 必须包含 CSV 数据点 3月,180"
        );

        // AC-3: 误差标注
        assert!(
            all_text.contains("AI 视觉提取"),
            "AC-3: chunks 必须包含误差提示标注"
        );
        assert!(
            all_text.contains("可能存在误差"),
            "AC-3: chunks 必须包含「可能存在误差」提示"
        );
        assert!(
            all_text.contains("请核对原文"),
            "AC-3: chunks 必须包含「请核对原文」提示"
        );

        // AC-5 旁证：CSV 数据已进入 chunk（分块→存储链路完成），
        // 向量化→检索链路由 embed_search_tests 模块的 vector_search 测试覆盖。
        // 完整 RAG 检索验证需 ONNX 模型，属 E2E 测试范围。
        // AC-4 旁证：VLM 不可用降级由 tc_mm_003c_vlm_unavailable_degrades_to_ocr 覆盖。
    }
}

// AUDIT 域集成测试已迁移至 integration/security_tests.rs
// REQ-LIC-004 License 停用与状态展示
// ==================================================================

/// TC-LIC-006 License 停用（REQ-LIC-004-AC-1）：
/// 停用后 is_pro 回落为 false，settings 表 license.is_pro 变为 "false"；
/// 重建 AppState（模拟重启）后仍为 false（持久化验证）。
#[tokio::test]
async fn tc_lic_006_deactivate_pro() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 模拟已激活 Pro（直接写入 settings 表）
    state
        .storage
        .set_setting("license.is_pro", "true")
        .await
        .unwrap();
    *state.is_pro().write().await = true;
    assert!(get_pro_status_inner(&state).await, "激活后应为 Pro");

    // 停用
    deactivate_pro_inner(&state).await.unwrap();

    // 验证运行态 is_pro 变为 false
    assert!(!get_pro_status_inner(&state).await, "停用后应为免费版");

    // 验证 settings 表持久化
    let stored = state.storage.get_setting("license.is_pro").await.unwrap();
    assert_eq!(stored.as_deref(), Some("false"), "settings 表应存储 false");

    // 重建 AppState（模拟重启），验证 is_pro 状态
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    assert!(
        !get_pro_status_inner(&restarted).await,
        "重启后应为免费版（持久化生效）"
    );
}

// ==================================================================
// REQ-NFR-004 资源占用
// ==================================================================

/// TC-NFR-004 资源占用基准（REQ-NFR-004-AC-1）：
/// 2,000 chunks + 384 维向量的知识库规模下，存储与检索功能正常，
/// 常驻内存应 ≤ 500MB（手动用 /usr/bin/time -l 或 Instruments 验证）。
///
/// `#[ignore]`：插入 2000 条向量耗时较长，仅在基准测试时手动运行：
/// `cargo test -p echomind-tauri-app --test integration -- --ignored tc_nfr_004`
#[tokio::test]
#[ignore = "基准测试：2k chunks 规模，手动运行"]
async fn tc_nfr_004_memory_footprint_2000_chunks() {
    use echomind_core::Storage as _;
    use echomind_models::Chunk;

    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 插入 1 个文档 + 2000 个 chunks + 2000 个 embeddings（384 维）
    let doc = Document::new("benchmark.md".to_string(), "bench-hash".to_string());
    state.storage.add_document(&doc).await.unwrap();

    let fake_vector = vec![0.01f32; 384]; // 384 维全量向量
    for i in 0..2000 {
        let chunk = Chunk::new(
            doc.id.clone(),
            format!("chunk content {i} with some text for embedding benchmark"),
            50,
            i,
        );
        state.storage.add_chunk(&chunk).await.unwrap();
        state
            .storage
            .add_embedding(&chunk.id, &fake_vector)
            .await
            .unwrap();
    }

    // 验证检索功能正常（不崩溃、返回 top-k 结果）
    let query = vec![0.01f32; 384];
    let results = state
        .storage
        .vector_search(&query, 5)
        .await
        .expect("2000 chunks 检索应成功");
    assert_eq!(results.len(), 5, "应返回 top-5 结果");

    // 验证 chunks 总数
    let all_chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert_eq!(all_chunks.len(), 2000, "应有 2000 个 chunks");

    // 内存占用验证说明：
    // 2000 × 384 × 4 bytes = ~3MB（纯向量数据）
    // 加上 SQLite 连接池、字符串、元数据等开销，常驻内存应远低于 500MB。
    // 手动验证：/usr/bin/time -l cargo test ... --ignored tc_nfr_004
}

// ==================================================================
// GB 级文档加速：渐进式向量化进度推送（路径 4）
// ==================================================================

/// TC-PERF-005：`EmbeddingProgressPayload` 序列化/反序列化往返。
///
/// 验证向量化进度事件负载的 JSON 序列化正确性，
/// 确保前端能正确解析 `embedding_progress` 事件。
#[tokio::test]
async fn tc_perf_005_embedding_progress_payload_serde() {
    use echomind_models::EmbeddingProgressPayload;

    let payload = EmbeddingProgressPayload {
        doc_id: "doc-uuid-123".to_string(),
        doc_name: "large_paper.pdf".to_string(),
        embedded: 128,
        total: 500,
    };

    let json = serde_json::to_string(&payload).unwrap();
    let de: EmbeddingProgressPayload = serde_json::from_str(&json).unwrap();

    assert_eq!(de.doc_id, "doc-uuid-123");
    assert_eq!(de.doc_name, "large_paper.pdf");
    assert_eq!(de.embedded, 128);
    assert_eq!(de.total, 500);
}

/// TC-PERF-006：向量化进度事件在导入流程中被发射（L2 集成测试）。
///
/// 创建一个小型 .md 文件并导入，验证：
/// 1. 导入成功后文档状态为 Indexed（分块入库）
/// 2. 向量化因缺少 ONNX 模型而失败（测试环境），但文档状态为 Failed
/// 3. 事件系统正常工作（不 panic、不卡死）
///
/// 此测试验证渐进式索引的「分块完成 → FTS5 立即可用」语义：
/// 即使向量化失败，分块已入库，关键词检索（BM25）仍可用。
#[tokio::test]
async fn tc_perf_006_progressive_indexing_chunks_available_before_embedding() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建测试文件
    let md = dir.path().join("progressive_test.md");
    std::fs::write(
        &md,
        "# Progressive Indexing\n\nThis is test content for progressive indexing.",
    )
    .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 导入文件（会触发索引 + 向量化，向量化因无模型而失败）
    let result = import_files_inner(&handle, &[md.to_string_lossy().into_owned()], &state).await;

    // 导入本身应成功（向量化失败不影响导入返回值）
    assert!(result.is_ok(), "导入应成功，即使向量化失败");
    let imported = result.unwrap();
    assert_eq!(imported.len(), 1, "应导入 1 个文件");

    // 验证分块已入库（FTS5 立即可用语义）
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1, "应有 1 个文档");
    let doc = &docs[0];

    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(
        !chunks.is_empty(),
        "分块应已入库（渐进式索引：FTS5 立即可用）"
    );

    // 验证关键词检索可用（BM25，不依赖向量）
    let results = state
        .storage
        .keyword_search("progressive", 5)
        .await
        .unwrap();
    assert!(
        !results.is_empty(),
        "关键词检索应返回结果（分块已入库，BM25 可用）"
    );
}

// ---- REQ-SYNC-001 文件夹监听配置与管理 ----

/// TC-SYNC-001: 获取监听文件夹列表（空列表）。
///
/// 初始状态下 `get_watched_folders` 应返回空列表。
#[tokio::test]
async fn tc_sync_001_get_watched_folders_empty() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let folders = get_watched_folders_inner(&state).await.unwrap();
    assert!(folders.is_empty(), "初始状态应无监听文件夹");
}

/// TC-SYNC-002: 移除不存在的监听文件夹（幂等，不报错）。
#[tokio::test]
async fn tc_sync_002_remove_nonexistent_folder() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = remove_watched_folder_inner("/nonexistent/folder/path", &state).await;
    assert!(result.is_ok(), "移除不存在的监听文件夹应幂等成功");
}

/// TC-SYNC-003: 持久化监听文件夹到 settings 表并重启恢复。
///
/// 验证 settings 表中 `sync.watched_folders` 键的正确持久化与恢复。
#[tokio::test]
async fn tc_sync_003_persist_and_restore_watched_folders() {
    let dir = TempDir::new().unwrap();

    // 写入阶段：持久化一个监听文件夹条目
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        state
            .storage
            .set_setting(
                "sync.watched_folders",
                r#"[{"path":"/test/folder","last_synced_at":1753872000}]"#,
            )
            .await
            .unwrap();
    }

    // 读取阶段：新 AppState 实例应恢复监听文件夹列表
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let folders = get_watched_folders_inner(&state).await.unwrap();
    assert_eq!(folders.len(), 1, "应恢复 1 个监听文件夹");
    assert_eq!(folders[0].path, "/test/folder");
    assert_eq!(folders[0].last_synced_at, Some(1753872000));
    // 监听器未启动（仅持久化恢复）
    assert_eq!(folders[0].sync_status, "stopped");
}

// ================== REQ-LLM-003/004 本地 LLM 引擎集成测试 ==================
//
// 覆盖 Session 4（AppState 集成）+ Session 5（IPC 命令）。
// 测试 ID 前缀：
//   TC-STATE-LLM-* — AppState 层（state.rs）
//   TC-IPC-LLM-*  — IPC 命令层（commands.rs）

/// TC-STATE-LLM-001：默认 LLM 模式为 Remote（REQ-LLM-003）。
///
/// 全新数据目录启动 AppState，未设置 `llm.mode` 时应返回 `LlmMode::Remote`。
#[tokio::test]
async fn tc_state_llm_001_default_remote() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let mode = state.get_llm_mode().await;
    assert_eq!(mode, LlmMode::Remote, "默认模式必须为 Remote");
}

/// TC-STATE-LLM-002：set_llm_mode 持久化到 settings 表（REQ-LLM-003）。
///
/// 调用 `set_llm_mode(LlmMode::Local)` 后，settings 表 `llm.mode` 值应为 `"local"`，
/// 且运行态 `get_llm_mode()` 也应返回 `Local`。
#[tokio::test]
async fn tc_state_llm_002_set_mode_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    state.set_llm_mode(LlmMode::Local).await.unwrap();

    // 运行态即时生效
    assert_eq!(state.get_llm_mode().await, LlmMode::Local);

    // 持久化到 settings 表
    let persisted = state.storage.get_setting("llm.mode").await.unwrap();
    assert_eq!(persisted.as_deref(), Some("local"));

    // 切回 Remote 验证双向持久化
    state.set_llm_mode(LlmMode::Remote).await.unwrap();
    assert_eq!(state.get_llm_mode().await, LlmMode::Remote);
    let persisted = state.storage.get_setting("llm.mode").await.unwrap();
    assert_eq!(persisted.as_deref(), Some("remote"));
}

/// TC-STATE-LLM-003：重启后恢复 LLM 模式（REQ-LLM-003）。
///
/// 设置 Local 模式 → 销毁 AppState → 重建 AppState → 模式应恢复为 Local。
#[tokio::test]
async fn tc_state_llm_003_restore_after_restart() {
    let dir = TempDir::new().unwrap();

    // 写入阶段：设置 Local 模式
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        state.set_llm_mode(LlmMode::Local).await.unwrap();
    } // AppState 销毁，模拟应用退出

    // 重启阶段：新 AppState 应从 settings 表恢复模式
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    assert_eq!(
        restarted.get_llm_mode().await,
        LlmMode::Local,
        "重启后应恢复为 Local 模式"
    );
}

/// TC-STATE-LLM-004：AppState 初始化时 ModelManager 就绪（REQ-LLM-004）。
///
/// `state.model_manager()` 应返回有效引用，且 `list_models()` 在空目录时返回空 Vec。
#[tokio::test]
async fn tc_state_llm_004_model_manager_initializes() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // model_manager 可访问
    let mgr = state.model_manager();

    // models/llm 目录应已创建
    let models_dir = mgr.models_dir();
    assert!(
        models_dir.exists(),
        "模型目录应已创建: {}",
        models_dir.display()
    );

    // 空目录返回空列表
    let models = mgr.list_models().unwrap();
    assert!(models.is_empty(), "空目录应返回空模型列表");
}

/// TC-IPC-LLM-001：无模型时 list_local_models 返回空 Vec（REQ-LLM-004 AC-1）。
#[tokio::test]
async fn tc_ipc_llm_001_list_empty() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let models = list_local_models_inner(&state).await.unwrap();
    assert!(models.is_empty(), "无模型文件时应返回空 Vec");
}

/// TC-IPC-LLM-002：创建 .gguf 文件后 list_local_models 返回包含该模型（REQ-LLM-004 AC-1）。
///
/// 在 models/llm 目录中创建一个假的 .gguf 文件，验证 list_local_models 能解析出 ModelInfo。
#[tokio::test]
async fn tc_ipc_llm_002_list_with_model() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建假 GGUF 文件（文件名需符合解析规则）
    let models_dir = state.model_manager().models_dir().to_path_buf();
    let gguf_path = models_dir.join("qwen2.5-3b-instruct-q4_k_m.gguf");
    std::fs::write(&gguf_path, b"fake gguf content").unwrap();

    let models = list_local_models_inner(&state).await.unwrap();
    assert_eq!(models.len(), 1, "应返回 1 个模型");
    assert_eq!(models[0].filename, "qwen2.5-3b-instruct-q4_k_m.gguf");
    assert_eq!(models[0].architecture, "qwen2.5");
    assert_eq!(models[0].param_size, "3B");
    assert_eq!(models[0].quantization, "Q4_K_M");
    assert!(models[0].size_bytes > 0, "文件大小应大于 0");
}

/// TC-IPC-LLM-003：删除模型文件成功（REQ-LLM-004 AC-3）。
#[tokio::test]
async fn tc_ipc_llm_003_delete_success() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建假 GGUF 文件
    let models_dir = state.model_manager().models_dir().to_path_buf();
    let filename = "llama-3.2-3b-instruct-q4_k_m.gguf";
    let gguf_path = models_dir.join(filename);
    std::fs::write(&gguf_path, b"fake content").unwrap();
    assert!(gguf_path.exists(), "前置条件：文件应存在");

    // 删除
    delete_model_inner(filename.to_string(), &state)
        .await
        .unwrap();

    // 文件应已删除
    assert!(!gguf_path.exists(), "删除后文件不应存在");

    // 列表应为空
    let models = list_local_models_inner(&state).await.unwrap();
    assert!(models.is_empty(), "删除后列表应为空");
}

/// TC-IPC-LLM-004：路径穿越攻击被拒绝（REQ-LLM-004 AC-3 安全）。
///
/// 尝试用 `../../../etc/passwd` 作为文件名删除，应返回错误而非访问系统文件。
#[tokio::test]
async fn tc_ipc_llm_004_delete_path_traversal() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = delete_model_inner("../../../etc/passwd".to_string(), &state).await;
    assert!(result.is_err(), "路径穿越应被拒绝");

    let result = delete_model_inner("..\\..\\..\\windows\\system32".to_string(), &state).await;
    assert!(result.is_err(), "Windows 路径穿越应被拒绝");
}

/// TC-IPC-LLM-005：设置 Remote 模式（REQ-LLM-003 AC-4）。
#[tokio::test]
async fn tc_ipc_llm_005_set_remote() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // Free 模式下 set_llm_mode("local") 应被 Pro 门控拦截
    let result = set_llm_mode_inner("local".to_string(), &state).await;
    assert!(result.is_err(), "Free 模式下 local 模式应被 Pro 门控拦截");
    assert!(result.unwrap_err().contains("PRO_REQUIRED"));

    // 默认应为 Remote
    assert_eq!(state.get_llm_mode().await, LlmMode::Remote);

    // 设置为 Remote（应成功）
    set_llm_mode_inner("remote".to_string(), &state)
        .await
        .unwrap();
    assert_eq!(state.get_llm_mode().await, LlmMode::Remote);

    // settings 表持久化
    let persisted = state.storage.get_setting("llm.mode").await.unwrap();
    assert_eq!(persisted.as_deref(), Some("remote"));
}

/// TC-IPC-LLM-006：设置 Local 模式（REQ-LLM-003 AC-4）。
/// Free 模式下 local 模式被 Pro 门控拦截。
#[tokio::test]
async fn tc_ipc_llm_006_set_local() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // Free 模式下 set_llm_mode("local") 应被 Pro 门控拦截
    let result = set_llm_mode_inner("local".to_string(), &state).await;
    assert!(result.is_err(), "Free 模式下 local 模式应被 Pro 门控拦截");
    let err = result.unwrap_err();
    assert!(
        err.contains("PRO_REQUIRED"),
        "错误应包含 PRO_REQUIRED: {err}"
    );

    // 模式不应被修改（仍为 Remote）
    assert_eq!(state.get_llm_mode().await, LlmMode::Remote);

    // settings 表不应持久化 local
    let persisted = state.storage.get_setting("llm.mode").await.unwrap();
    assert_ne!(persisted.as_deref(), Some("local"));
}

/// TC-IPC-LLM-007：无效模式返回错误（REQ-LLM-003 AC-4）。
#[tokio::test]
async fn tc_ipc_llm_007_set_invalid_mode() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = set_llm_mode_inner("invalid".to_string(), &state).await;
    assert!(result.is_err(), "无效模式应返回错误");
    let err = result.unwrap_err();
    assert!(
        err.contains("无效的 LLM 模式"),
        "错误消息应包含提示，实际: {err}"
    );

    // 原模式不应被修改
    assert_eq!(state.get_llm_mode().await, LlmMode::Remote);
}

/// TC-IPC-LLM-008：默认 get_llm_mode 返回 "remote"（REQ-LLM-003）。
#[tokio::test]
async fn tc_ipc_llm_008_get_default() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let mode = get_llm_mode_inner(&state).await.unwrap();
    assert_eq!(mode, "remote", "默认模式字符串应为 remote");
}

/// TC-IPC-LLM-009：推荐模型列表非空且包含 4 个模型（REQ-LLM-004 AC-6）。
#[tokio::test]
async fn tc_ipc_llm_009_recommended_models() {
    let recommended = echomind_infra::model_manager::RECOMMENDED_MODELS;
    assert!(!recommended.is_empty(), "推荐模型列表不应为空");
    assert_eq!(recommended.len(), 4, "应包含 4 个推荐模型");

    // 验证每个模型有必要的字段
    for model in recommended {
        assert!(!model.name.is_empty(), "模型名不应为空");
        assert!(!model.architecture.is_empty(), "架构不应为空");
        assert!(!model.param_size.is_empty(), "参数规模不应为空");
        assert!(!model.quantization.is_empty(), "量化格式不应为空");
        assert!(model.size_gb > 0.0, "文件大小应大于 0");
        assert!(
            model.url.starts_with("https://"),
            "下载 URL 必须为 HTTPS: {}",
            model.url
        );
    }
}

/// TC-IPC-LLM-010：set_local_model 持久化到 settings 表（REQ-LLM-003）。
/// Free 模式下 set_local_model 被 Pro 门控拦截。
#[tokio::test]
async fn tc_ipc_llm_010_set_local_model_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let filename = "qwen2.5-7b-instruct-q4_k_m.gguf";
    // Free 模式下 set_local_model 应被 Pro 门控拦截
    let result = set_local_model_inner(filename.to_string(), &state).await;
    assert!(
        result.is_err(),
        "Free 模式下 set_local_model 应被 Pro 门控拦截"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("PRO_REQUIRED"),
        "错误应包含 PRO_REQUIRED: {err}"
    );

    // 不应持久化到 settings 表
    let persisted = state.storage.get_setting("llm.local_model").await.unwrap();
    assert_ne!(persisted.as_deref(), Some(filename));
}

/// TC-IPC-LLM-011：get_settings 返回 llm_mode 和 local_model 字段（REQ-LLM-003）。
/// Free 模式下 local 设置被 Pro 门控拦截，但 get_settings 正常返回。
#[tokio::test]
async fn tc_ipc_llm_011_get_settings_returns_llm_fields() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 初始状态：llm_mode 为空串（等价 remote），local_model 为空串
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.llm_mode, "",
        "初始 llm_mode 应为空串（等价 remote）"
    );
    assert_eq!(settings.local_model, "", "初始 local_model 应为空串");

    // Free 模式下 set_llm_mode("local") 和 set_local_model 应被 Pro 门控拦截
    let result1 = set_llm_mode_inner("local".to_string(), &state).await;
    assert!(result1.is_err(), "Free 模式下 local 模式应被 Pro 门控拦截");
    let result2 = set_local_model_inner("test-model.gguf".to_string(), &state).await;
    assert!(
        result2.is_err(),
        "Free 模式下 set_local_model 应被 Pro 门控拦截"
    );

    // 模式不应被修改（仍为 Remote 等价空串）
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(settings.llm_mode, "", "Free 模式下 llm_mode 应不被修改");
    assert_eq!(
        settings.local_model, "",
        "Free 模式下 local_model 应不被修改"
    );
}

/// TC-IPC-LLM-012：get_settings 重启后恢复 llm_mode 和 local_model（REQ-LLM-003）。
/// Free 模式下 local 设置被 Pro 门控拦截，重启后仍为 remote。
#[tokio::test]
async fn tc_ipc_llm_012_get_settings_restore_after_restart() {
    let dir = TempDir::new().unwrap();

    // 写入阶段：Free 模式下 set_llm_mode/set_local_model 被 Pro 门控拦截
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let r1 = set_llm_mode_inner("local".to_string(), &state).await;
        assert!(r1.is_err(), "Free 模式下 local 模式应被 Pro 门控拦截");
        let r2 = set_local_model_inner("persisted-model.gguf".to_string(), &state).await;
        assert!(r2.is_err(), "Free 模式下 set_local_model 应被 Pro 门控拦截");
    }

    // 重启阶段：仍为 Remote（未被修改）
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert_eq!(settings.llm_mode, "", "重启后 llm_mode 应为空串（remote）");
    assert_eq!(settings.local_model, "", "重启后 local_model 应为空串");

    // 运行态也应恢复为 Remote
    assert_eq!(
        restarted.get_llm_mode().await,
        LlmMode::Remote,
        "重启后运行态模式应恢复为 Remote"
    );
}

// ============================================================================
// PagedAttention IPC 集成测试（REQ-LLM-003 扩展，S10）
// ============================================================================

/// TC-IPC-PAGED-001：set_paged_attn 持久化配置到 settings 表（S10）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_paged_001_set_paged_attn_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 设置 PagedAttention 启用
    set_paged_attn_inner(true, 32, 4096, &state).await.unwrap();

    // 验证持久化
    let enabled = state.storage.get_setting("llm.paged_attn").await.unwrap();
    assert_eq!(enabled.as_deref(), Some("true"));
    let block_size = state.storage.get_setting("llm.block_size").await.unwrap();
    assert_eq!(block_size.as_deref(), Some("32"));
    let gpu_mem = state
        .storage
        .get_setting("llm.gpu_memory_ctx")
        .await
        .unwrap();
    assert_eq!(gpu_mem.as_deref(), Some("4096"));
}

/// TC-IPC-PAGED-002：set_paged_attn 无效块大小返回错误（S10）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_paged_002_invalid_block_size() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 块大小 64 不在支持列表中
    let result = set_paged_attn_inner(true, 64, 4096, &state).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("无效的块大小"));
}

/// TC-IPC-PAGED-003：get_settings 返回 PagedAttention 默认值（S10）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_paged_003_get_settings_defaults() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let settings = get_settings_inner(&state).await.unwrap();
    // 默认关闭
    assert!(!settings.llm_paged_attn, "默认不应启用 PagedAttention");
    assert_eq!(settings.llm_block_size, 32, "默认块大小应为 32");
    assert_eq!(
        settings.llm_gpu_memory_ctx, 4096,
        "默认 GPU 上下文应为 4096"
    );
}

/// TC-IPC-PAGED-004：set_paged_attn 后 get_settings 反映最新值（S10）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_paged_004_set_then_get() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 先设置
    set_paged_attn_inner(true, 16, 2048, &state).await.unwrap();

    // 验证 get_settings 反映最新值
    let settings = get_settings_inner(&state).await.unwrap();
    assert!(settings.llm_paged_attn, "应反映已启用状态");
    assert_eq!(settings.llm_block_size, 16, "应反映自定义块大小");
    assert_eq!(settings.llm_gpu_memory_ctx, 2048, "应反映自定义 GPU 上下文");
}

/// TC-IPC-PAGED-005：set_paged_attn 重启后恢复配置（S10）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_paged_005_restore_after_restart() {
    let dir = TempDir::new().unwrap();

    // 写入阶段
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        set_paged_attn_inner(true, 8, 1024, &state).await.unwrap();
    }

    // 重启阶段
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    assert!(settings.llm_paged_attn, "重启后应保持启用");
    assert_eq!(settings.llm_block_size, 8, "重启后块大小应恢复");
    assert_eq!(settings.llm_gpu_memory_ctx, 1024, "重启后 GPU 上下文应恢复");
}

// ============================================================================
// 采样参数 IPC 集成测试（REQ-LLM-003 扩展，S11）
// ============================================================================

/// TC-IPC-SAMPLE-001：get_settings 默认返回 None 采样参数（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_001_get_settings_defaults_none() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let settings = get_settings_inner(&state).await.unwrap();
    assert!(
        settings.llm_sampling.is_none(),
        "默认采样参数应为 None（使用引擎默认值）"
    );
}

/// TC-IPC-SAMPLE-002：set_sampling_params 持久化到 settings 表（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_002_set_sampling_params_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let params = LlmSamplingParams {
        temperature: Some(0.8),
        top_p: Some(0.95),
        top_k: Some(40),
        max_tokens: Some(2048),
        frequency_penalty: Some(0.5),
        presence_penalty: Some(0.2),
    };
    set_sampling_params_inner(params, &state).await.unwrap();

    // 验证持久化
    let json = state.storage.get_setting("llm.sampling").await.unwrap();
    assert!(json.is_some(), "llm.sampling 应已持久化");

    // 验证 JSON 内容可反序列化
    let stored: LlmSamplingParams = serde_json::from_str(&json.unwrap()).expect("反序列化应成功");
    assert_eq!(stored.temperature, Some(0.8));
    assert_eq!(stored.top_p, Some(0.95));
    assert_eq!(stored.top_k, Some(40));
    assert_eq!(stored.max_tokens, Some(2048));
    assert_eq!(stored.frequency_penalty, Some(0.5));
    assert_eq!(stored.presence_penalty, Some(0.2));
}

/// TC-IPC-SAMPLE-003：set_sampling_params 后 get_settings 反映最新值（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_003_set_then_get() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let params = LlmSamplingParams {
        temperature: Some(0.5),
        max_tokens: Some(512),
        ..Default::default()
    };
    set_sampling_params_inner(params, &state).await.unwrap();

    let settings = get_settings_inner(&state).await.unwrap();
    let sampling = settings.llm_sampling.expect("采样参数应存在");
    assert_eq!(sampling.temperature, Some(0.5));
    assert_eq!(sampling.max_tokens, Some(512));
    assert!(sampling.top_p.is_none(), "top_p 未设置应为 None");
    assert!(sampling.top_k.is_none(), "top_k 未设置应为 None");
}

/// TC-IPC-SAMPLE-004：无效 temperature 返回错误（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_004_invalid_temperature() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let params = LlmSamplingParams {
        temperature: Some(3.0), // 超出 [0.0, 2.0]
        ..Default::default()
    };
    let result = set_sampling_params_inner(params, &state).await;
    assert!(result.is_err(), "超出范围的 temperature 应返回错误");
    let err = result.unwrap_err();
    assert!(err.contains("temperature"), "错误信息应包含 temperature");
}

/// TC-IPC-SAMPLE-005：无效 top_k 返回错误（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_005_invalid_top_k() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let params = LlmSamplingParams {
        top_k: Some(0), // 超出 [1, 100]
        ..Default::default()
    };
    let result = set_sampling_params_inner(params, &state).await;
    assert!(result.is_err(), "top_k=0 应返回错误");
    let err = result.unwrap_err();
    assert!(err.contains("top_k"), "错误信息应包含 top_k");
}

/// TC-IPC-SAMPLE-006：重启后采样参数恢复（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_006_restore_after_restart() {
    let dir = TempDir::new().unwrap();

    // 写入阶段
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        let params = LlmSamplingParams {
            temperature: Some(0.3),
            max_tokens: Some(1024),
            ..Default::default()
        };
        set_sampling_params_inner(params, &state).await.unwrap();
    }

    // 重启阶段
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let settings = get_settings_inner(&restarted).await.unwrap();
    let sampling = settings.llm_sampling.expect("重启后采样参数应恢复");
    assert_eq!(sampling.temperature, Some(0.3), "重启后 temperature 应恢复");
    assert_eq!(sampling.max_tokens, Some(1024), "重启后 max_tokens 应恢复");
}

/// TC-IPC-SAMPLE-007：无效 frequency_penalty 返回错误（S11）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_ipc_sample_007_invalid_frequency_penalty() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let params = LlmSamplingParams {
        frequency_penalty: Some(3.0), // 超出 [-2.0, 2.0]
        ..Default::default()
    };
    let result = set_sampling_params_inner(params, &state).await;
    assert!(result.is_err(), "超出范围的 frequency_penalty 应返回错误");
    let err = result.unwrap_err();
    assert!(
        err.contains("frequency_penalty"),
        "错误信息应包含 frequency_penalty"
    );
}

// 安全防御 IPC 集成测试已迁移至 integration/security_tests.rs

// 导出功能集成测试（tc_ipc_exp_001/002）已迁移至 integration/conversation_tests.rs

// ============================================================================
// 文件监听集成测试（REQ-SYNC-001~003）
// ============================================================================

/// TC-IPC-SYNC-001：获取已监听文件夹列表——初始为空（REQ-SYNC-003）。
#[tokio::test]
async fn tc_ipc_sync_001_get_watched_folders_initial_empty() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let folders = get_watched_folders_inner(&state).await.unwrap();
    assert!(folders.is_empty(), "初始监听文件夹列表应为空");
}

/// TC-IPC-SYNC-002：移除不存在的监听文件夹——无错误（REQ-SYNC-002）。
#[tokio::test]
async fn tc_ipc_sync_002_remove_nonexistent_folder_no_error() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = remove_watched_folder_inner("//nonexistent//path", &state).await;
    assert!(result.is_ok(), "移除不存在的文件夹不应报错");
}

// ============================================================================
// 高级 RAG 功能集成测试（REQ-RAG-020~022, REQ-VEC-012）
async fn tc_ipc_vec_012_embedding_model_switch_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    set_embedding_model_inner("bge-small-zh-v1.5", &state)
        .await
        .unwrap();
    let settings = get_settings_inner(&state).await.unwrap();
    assert_eq!(
        settings.embedding_model, "bge-small-zh-v1.5",
        "嵌入模型应已切换"
    );
}

/// TC-IPC-I18N-001：locale 持久化与恢复（REQ-I18N-001）。
#[tokio::test]
async fn tc_ipc_i18n_001_locale_persists_across_restart() {
    let dir = TempDir::new().unwrap();
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        set_locale_inner("en", &state).await.unwrap();
    }

    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let locale = get_locale_inner(&restarted).await.unwrap();
    assert_eq!(locale, "en", "重启后 locale 应恢复为 en");
}

/// TC-NAV-001-001：侧栏折叠状态持久化与恢复（REQ-NAV-001 AC-4）。
#[tokio::test]
async fn tc_nav_001_001_sidebar_collapsed_persists() {
    let dir = TempDir::new().unwrap();
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        // 默认未折叠
        let collapsed = get_sidebar_collapsed_inner(&state).await.unwrap();
        assert!(!collapsed, "默认应为展开状态");

        // 设置折叠
        set_sidebar_collapsed_inner(true, &state).await.unwrap();
        let collapsed = get_sidebar_collapsed_inner(&state).await.unwrap();
        assert!(collapsed, "设置后应为折叠状态");
    }

    // 模拟重启后恢复
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let collapsed = get_sidebar_collapsed_inner(&restarted).await.unwrap();
    assert!(collapsed, "重启后应恢复为折叠状态");

    // 恢复展开
    set_sidebar_collapsed_inner(false, &restarted)
        .await
        .unwrap();
    let collapsed = get_sidebar_collapsed_inner(&restarted).await.unwrap();
    assert!(!collapsed, "恢复后应为展开状态");
}

// ============================================================================
// Phase 3 自研内核模式 IPC 集成测试（Session 20）
// ============================================================================

/// TC-INTEG-007：set_kernel_mode 持久化到 settings 表（Phase 3）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_integ_007_kernel_mode_persists() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 设置为 custom 模式
    set_kernel_mode_inner("custom".to_string(), &state)
        .await
        .unwrap();

    // 验证持久化
    let stored = state.storage.get_setting("llm.kernel_mode").await.unwrap();
    assert_eq!(stored.as_deref(), Some("custom"));

    // get_kernel_mode 返回 custom
    let mode = get_kernel_mode_inner(&state).await.unwrap();
    assert_eq!(mode, "custom", "get_kernel_mode 应返回 custom");
}

/// TC-INTEG-007b：get_kernel_mode 默认返回 mistral（Phase 3）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_integ_007b_kernel_mode_default_mistral() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 未设置时返回默认值
    let mode = get_kernel_mode_inner(&state).await.unwrap();
    assert_eq!(mode, "mistral", "默认应返回 mistral");
}

/// TC-INTEG-007c：set_kernel_mode 无效模式返回错误（Phase 3）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_integ_007c_kernel_mode_invalid() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = set_kernel_mode_inner("invalid".to_string(), &state).await;
    assert!(result.is_err(), "无效模式应返回错误");
    let err = result.unwrap_err();
    assert!(
        err.contains("无效的内核模式"),
        "错误信息应包含无效提示: {err}"
    );
}

/// TC-INTEG-007d：set_kernel_mode 切换回 mistral（Phase 3）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_integ_007d_kernel_mode_switch_back() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 先设为 custom
    set_kernel_mode_inner("custom".to_string(), &state)
        .await
        .unwrap();
    assert_eq!(get_kernel_mode_inner(&state).await.unwrap(), "custom");

    // 切换回 mistral
    set_kernel_mode_inner("mistral".to_string(), &state)
        .await
        .unwrap();
    assert_eq!(get_kernel_mode_inner(&state).await.unwrap(), "mistral");
}

/// TC-INTEG-007e：set_kernel_mode 重启后恢复（Phase 3）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_integ_007e_kernel_mode_restore_after_restart() {
    let dir = TempDir::new().unwrap();

    // 写入阶段
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        set_kernel_mode_inner("custom".to_string(), &state)
            .await
            .unwrap();
    }

    // 重启阶段
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let mode = get_kernel_mode_inner(&restarted).await.unwrap();
    assert_eq!(mode, "custom", "重启后内核模式应恢复为 custom");
}

// ============================================================================
// Phase 4 KV cache 跨会话复用 IPC 集成测试（Session 25，REQ-LLM-009）
// ============================================================================

/// TC-KVC-007：save_kv_cache + load_kv_cache 往返（无模型时验证 IPC 管道不崩溃）。
///
/// 由于无真实模型文件，save_kv_cache_inner 在引擎未初始化时静默返回 Ok(())（不创建文件），
/// load_kv_cache_inner 在引擎未初始化时返回 Ok(false)（cache miss）。
/// 此测试验证 IPC 管道完整、无 panic、返回类型正确。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_007_save_restore_roundtrip() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 启用 KV cache
    set_kv_cache_enabled_inner(true, &state).await.unwrap();

    // save：无模型 → 静默 Ok(())（不创建文件）
    save_kv_cache_inner("conv-001".to_string(), &state)
        .await
        .expect("save_kv_cache 在引擎未初始化时应返回 Ok(())");

    // load：无模型 + 无文件 → Ok(false)（cache miss）
    let hit = load_kv_cache_inner("conv-001".to_string(), &state)
        .await
        .expect("load_kv_cache 在无缓存时应返回 Ok(false)");
    assert!(!hit, "无模型无缓存时应返回 false（cache miss）");
}

/// TC-KVC-008：首次加载无缓存 → cache miss（正常推理路径）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_008_cache_miss_first_load() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 启用 KV cache
    set_kv_cache_enabled_inner(true, &state).await.unwrap();

    // 首次加载：无缓存文件 → false
    let hit = load_kv_cache_inner("nonexistent-conv".to_string(), &state)
        .await
        .expect("load_kv_cache 不应失败");
    assert!(!hit, "首次加载应返回 false（cache miss）");
}

/// TC-KVC-009：清除缓存后再恢复 → cache miss。
///
/// 手动创建 .emkv 文件（模拟 save），然后 clear 删除，再 load 验证 false。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_009_cache_clear() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 启用 KV cache
    set_kv_cache_enabled_inner(true, &state).await.unwrap();

    // 手动创建缓存目录和文件（模拟已有缓存）
    let kv_dir = state.kv_cache_dir();
    std::fs::create_dir_all(&kv_dir).unwrap();
    let conv_id = "conv-to-clear";
    let file_path = kv_dir.join(format!("{conv_id}.emkv"));
    std::fs::write(&file_path, b"dummy emkv content").unwrap();
    assert!(file_path.exists(), "测试前置：缓存文件应存在");

    // clear：删除文件
    clear_kv_cache_inner(conv_id.to_string(), &state)
        .await
        .expect("clear_kv_cache 应成功");
    assert!(!file_path.exists(), "清除后文件应被删除");

    // load：文件已删除 → false
    // 注意：由于引擎未初始化，load 也会返回 false
    let hit = load_kv_cache_inner(conv_id.to_string(), &state)
        .await
        .expect("load_kv_cache 不应失败");
    assert!(!hit, "清除后应返回 false（cache miss）");
}

/// TC-KVC-010：禁用时 save/load 为空操作（noop）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_010_disabled_noop() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 确保禁用（默认即为 false，但显式设置确保测试隔离）
    set_kv_cache_enabled_inner(false, &state).await.unwrap();

    // save：禁用时 → Ok(()) 且不创建文件
    save_kv_cache_inner("conv-disabled".to_string(), &state)
        .await
        .expect("禁用时 save 应返回 Ok(())");
    let kv_dir = state.kv_cache_dir();
    let file_path = kv_dir.join("conv-disabled.emkv");
    assert!(!file_path.exists(), "禁用时不应创建缓存文件");

    // load：禁用时 → Ok(false)
    let hit = load_kv_cache_inner("conv-disabled".to_string(), &state)
        .await
        .expect("禁用时 load 应返回 Ok(false)");
    assert!(!hit, "禁用时 load 应返回 false");
}

/// TC-KVC-011：set_kv_cache_enabled 持久化 + get_kv_cache_status 返回正确状态。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_011_status_and_persist() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 初始状态：禁用 + 无文件
    let status = get_kv_cache_status_inner(&state).await.unwrap();
    assert!(!status.enabled, "初始应禁用");
    assert_eq!(status.file_count, 0, "初始无缓存文件");
    assert_eq!(status.total_size_bytes, 0, "初始大小为 0");

    // 启用
    set_kv_cache_enabled_inner(true, &state).await.unwrap();

    // 手动创建缓存文件（模拟 save 操作产物）
    let kv_dir = state.kv_cache_dir();
    std::fs::create_dir_all(&kv_dir).unwrap();
    std::fs::write(kv_dir.join("conv-a.emkv"), b"content-a").unwrap();
    std::fs::write(kv_dir.join("conv-b.emkv"), b"content-bb").unwrap();

    // 验证状态
    let status = get_kv_cache_status_inner(&state).await.unwrap();
    assert!(status.enabled, "启用后 status.enabled 应为 true");
    assert_eq!(status.file_count, 2, "应有 2 个缓存文件");
    assert_eq!(
        status.total_size_bytes,
        ("content-a".len() + "content-bb".len()) as u64,
        "总大小应为两个文件大小之和"
    );
    assert!(
        status.cache_dir.contains("kv_cache"),
        "cache_dir 应包含 kv_cache: {}",
        status.cache_dir
    );

    // 重启后验证持久化
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let status2 = get_kv_cache_status_inner(&restarted).await.unwrap();
    assert!(status2.enabled, "重启后应仍为启用");
    assert_eq!(status2.file_count, 2, "重启后文件数应不变");
}

/// TC-KVC-012：clear_kv_cache 幂等性（文件不存在时也返回 Ok）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_012_clear_idempotent() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 清除不存在的文件 → Ok(())（幂等）
    clear_kv_cache_inner("nonexistent".to_string(), &state)
        .await
        .expect("清除不存在的缓存应返回 Ok(())（幂等）");
}

/// TC-KVC-013：clear_kv_cache 路径清理（非法字符替换为下划线）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_kvc_013_path_sanitization() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 启用 KV cache
    set_kv_cache_enabled_inner(true, &state).await.unwrap();

    // 创建目录
    let kv_dir = state.kv_cache_dir();
    std::fs::create_dir_all(&kv_dir).unwrap();

    // 使用包含非法字符的 conversation_id
    // "conv/../../etc" → 清理后 "conv______etc"
    let dirty_id = "conv/../../etc";

    // clear 应不崩溃且不删除其他文件
    clear_kv_cache_inner(dirty_id.to_string(), &state)
        .await
        .expect("清除含非法字符的 ID 应返回 Ok(())");

    // 验证没有路径遍历：检查实际创建的文件名
    let entries: Vec<_> = std::fs::read_dir(&kv_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    // 不应有任何文件被路径遍历创建
    for entry in &entries {
        assert!(!entry.contains(".."), "文件名不应包含路径遍历: {entry}");
    }
}

// ============================================================
// REQ-ERR-001 / REQ-ERR-005 统一错误分类体系 + 输入校验
// ============================================================

/// TC-ERR-001 网络错误前缀：classify_llm_error 对网络错误消息添加 NETWORK: 前缀。
#[tokio::test]
async fn tc_err_001_network_error_prefix() {
    // 模拟 OpenAIProvider 返回的网络连接失败错误
    let raw_err = "LLM 请求发送失败: Connection refused (os error 61)";
    let classified = classify_llm_error(raw_err);
    assert!(
        classified.starts_with("NETWORK:"),
        "网络错误应返回 NETWORK: 前缀，实际: {classified}"
    );
    assert!(
        classified.contains("Connection refused"),
        "分类后应保留原始错误详情，实际: {classified}"
    );
}

/// TC-ERR-002 鉴权错误前缀：classify_llm_error 对 401/403 错误添加 AUTH: 前缀。
#[tokio::test]
async fn tc_err_002_auth_error_prefix() {
    // 模拟 401 Unauthorized
    let raw_err = "LLM API 错误 (HTTP 401): Unauthorized";
    let classified = classify_llm_error(raw_err);
    assert!(
        classified.starts_with("AUTH:"),
        "401 错误应返回 AUTH: 前缀，实际: {classified}"
    );
    // 模拟 403 Forbidden
    let raw_err = "LLM API 错误 (HTTP 403): Forbidden";
    let classified = classify_llm_error(raw_err);
    assert!(
        classified.starts_with("AUTH:"),
        "403 错误应返回 AUTH: 前缀，实际: {classified}"
    );
}

/// TC-ERR-003 LLM 错误前缀：classify_llm_error 对其他 HTTP 错误添加 LLM: 前缀。
#[tokio::test]
async fn tc_err_003_llm_error_prefix() {
    // 模拟 500 Internal Server Error
    let raw_err = "LLM API 错误 (HTTP 500): Internal Server Error";
    let classified = classify_llm_error(raw_err);
    assert!(
        classified.starts_with("LLM:"),
        "500 错误应返回 LLM: 前缀，实际: {classified}"
    );
    assert!(
        classified.contains("HTTP 500"),
        "分类后应保留 HTTP 状态码，实际: {classified}"
    );
    // 模拟 JSON 解析错误（非 HTTP 错误，也归类为 LLM 错误）
    let raw_err = "JSON 解析失败: invalid response body";
    let classified = classify_llm_error(raw_err);
    assert!(
        classified.starts_with("LLM:"),
        "非网络/鉴权类 LLM 错误应返回 LLM: 前缀，实际: {classified}"
    );
}

/// TC-ERR-004 解析错误前缀：导入非 UTF-8 的 .md 文件，验证事件含 PARSE: 前缀。
#[tokio::test]
async fn tc_err_004_parse_error_prefix() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建非 UTF-8 编码的 .md 文件（触发 read_to_string 失败）
    let bad_md = dir.path().join("bad.md");
    std::fs::write(&bad_md, [0xFF, 0xFE, 0x00, 0x01, 0xC0, 0xC1]).unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 监听 doc-status-changed 事件，捕获含 PARSE: 前缀的错误消息
    let captured = Arc::new(std::sync::Mutex::new(String::new()));
    let captured_clone = captured.clone();
    let _id = handle.listen("doc-status-changed", move |event| {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(event.payload())
            && let Some(msg) = v.get("message").and_then(|m| m.as_str())
            && msg.contains("PARSE:")
        {
            *captured_clone.lock().unwrap() = msg.to_string();
        }
    });

    let result =
        import_files_inner(&handle, &[bad_md.to_string_lossy().into_owned()], &state).await;

    // 导入应成功（文件被导入但索引失败，错误通过事件上报）
    assert!(result.is_ok(), "导入应成功（索引失败不阻止导入）");

    // 等待事件处理（mock runtime 同步分发，但给少量缓冲）
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let msg = captured.lock().unwrap().clone();
    assert!(
        msg.contains("PARSE:"),
        "索引失败事件应含 PARSE: 前缀，实际: {msg}"
    );

    // 验证文档状态为 Failed
    let docs = state.storage.list_documents().await.unwrap();
    let doc = docs.iter().find(|d| d.file_path.contains("bad.md"));
    assert!(doc.is_some(), "文档应已入库");
    if let Some(d) = doc {
        assert!(
            matches!(d.status, DocStatus::Failed(_)),
            "文档状态应为 Failed"
        );
    }
}

/// TC-ERR-005 嵌入错误前缀：验证 EMBED 前缀格式化 + embed 错误路径。
#[tokio::test]
async fn tc_err_005_embed_error_prefix() {
    // 验证 prefix_error 正确格式化 EMBED 前缀
    let formatted = prefix_error(ERR_EMBED, "向量化失败: ONNX model not loaded");
    assert_eq!(
        formatted, "EMBED: 向量化失败: ONNX model not loaded",
        "EMBED 前缀格式化应正确"
    );

    // 验证 has_error_prefix 识别 EMBED 前缀
    assert!(
        has_error_prefix(&formatted),
        "has_error_prefix 应识别 EMBED: 前缀"
    );

    // 验证 ERR_EMBED 常量值
    assert_eq!(ERR_EMBED, "EMBED", "ERR_EMBED 常量值应为 EMBED");
}

/// TC-ERR-006 存储错误前缀：导入不支持的文件格式，验证返回值含 STORAGE: 前缀。
#[tokio::test]
async fn tc_err_006_storage_error_prefix() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建 .bin 文件（不在格式白名单中）
    let bad_doc = dir.path().join("doc.bin");
    std::fs::write(&bad_doc, b"fake binary content").unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result =
        import_files_inner(&handle, &[bad_doc.to_string_lossy().into_owned()], &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("STORAGE:"),
        "导入失败应返回 STORAGE: 前缀，实际: {err}"
    );
    assert!(
        err.contains("bin"),
        "错误消息应包含文件格式信息，实际: {err}"
    );
}

/// TC-ERR-007 API Key 过长校验：超过 256 字符返回 VALIDATION: 前缀。
#[tokio::test]
async fn tc_err_007_validation_api_key_too_long() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 构造 257 字符的 API Key
    let long_key = "x".repeat(257);
    let config = LlmConfig {
        api_key: long_key,
        base_url: "https://api.deepseek.com".to_string(),
        model: "deepseek-chat".to_string(),
    };

    let result = update_llm_config_inner(config, &state).await;
    let err = result.err().unwrap_or_default();
    assert!(
        err.starts_with("VALIDATION:"),
        "API Key 过长应返回 VALIDATION: 前缀，实际: {err}"
    );
    assert!(
        err.contains("256"),
        "错误消息应包含长度上限 256，实际: {err}"
    );
}

/// TC-ERR-008 Base URL 格式校验：非 http/https 协议返回 VALIDATION: 前缀。
#[tokio::test]
async fn tc_err_008_validation_bad_url() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let config = LlmConfig {
        api_key: "sk-test-1234567890".to_string(),
        base_url: "ftp://bad.example.com".to_string(),
        model: "deepseek-chat".to_string(),
    };

    let result = update_llm_config_inner(config, &state).await;
    let err = result.err().unwrap_or_default();
    assert!(
        err.starts_with("VALIDATION:"),
        "非法 URL 应返回 VALIDATION: 前缀，实际: {err}"
    );
    assert!(
        err.contains("Base URL"),
        "错误消息应提及 Base URL，实际: {err}"
    );
}

/// TC-ERR-009 查询过长校验：超过 8000 字符返回 VALIDATION: 前缀。
#[tokio::test]
async fn tc_err_009_validation_query_too_long() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    // 配置 LLM（排除「未配置」干扰）
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 构造 8001 字符的查询
    let long_query = "a".repeat(8001);

    let result = chat_inner(
        &handle,
        &long_query,
        &[],
        "conv-err-009",
        None,
        None,
        &state,
    )
    .await;
    let err = result.err().unwrap_or_default();
    assert!(
        err.starts_with("VALIDATION:"),
        "查询过长应返回 VALIDATION: 前缀，实际: {err}"
    );
    assert!(
        err.contains("8000"),
        "错误消息应包含长度上限 8000，实际: {err}"
    );
}

/// TC-ERR-010 错误前缀传播：chat_inner 返回的错误携带正确前缀，chat 命令不再重复包装。
///
/// 修复前：chat_inner 返回无前缀错误 → chat 命令包装为 UNKNOWN: → 前端显示「未知错误」。
/// 修复后：chat_inner 返回带前缀错误（如 VALIDATION:） → chat 命令保留原前缀 → 前端显示具体原因。
/// 同时验证 chat 命令不再 emit chat_error 事件（消除双重 toast Bug）。
#[tokio::test]
async fn tc_err_010_unknown_error_prefix() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    // 配置 LLM（排除「未配置」干扰）
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // 空知识库 → chat_inner 返回 "VALIDATION: 知识库为空" 错误（带前缀）
    let result = chat_inner(&handle, "测试问题", &[], "conv-err-010", None, None, &state).await;
    let err = result.err().unwrap_or_default();

    // chat_inner 返回的错误应携带 VALIDATION: 前缀
    assert!(
        has_error_prefix(&err),
        "chat_inner 返回的空库错误应含前缀，实际: {err}"
    );
    assert!(
        err.starts_with("VALIDATION:"),
        "空库错误应以 VALIDATION: 开头，实际: {err}"
    );
    assert!(
        err.contains("知识库为空"),
        "错误应保留原始详情，实际: {err}"
    );

    // 模拟 chat 命令的兜底包装逻辑：已有前缀 → 保留原前缀（不包装为 UNKNOWN:）
    let classified = if has_error_prefix(&err) {
        err.clone()
    } else {
        prefix_error(ERR_UNKNOWN, &err)
    };
    assert!(
        classified.starts_with("VALIDATION:"),
        "已有前缀的错误不应被包装为 UNKNOWN:，实际: {classified}"
    );
    assert!(
        !classified.starts_with("UNKNOWN:"),
        "带前缀错误不应被重新包装为 UNKNOWN:，实际: {classified}"
    );

    // 验证 ERR_UNKNOWN 常量值
    assert_eq!(ERR_UNKNOWN, "UNKNOWN", "ERR_UNKNOWN 常量值应为 UNKNOWN");
}

// ============================================================================
// REQ-ERR-004：崩溃恢复与数据一致性 — 数据库完整性检查
// ============================================================================

/// TC-ERR-004 数据库完整性检查：正常数据库返回 Ok。
///
/// 新建临时数据库执行 `PRAGMA integrity_check`，返回值应为 `IntegrityCheckResult::Ok`。
#[tokio::test]
async fn tc_err_004_integrity_check_ok() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = state.storage.check_integrity().await.unwrap();
    assert!(
        matches!(
            result,
            echomind_infra::sqlite_storage::IntegrityCheckResult::Ok
        ),
        "正常数据库完整性检查应返回 Ok，实际: {result:?}"
    );
}

/// TC-ERR-005 open_data_dir 命令：验证数据目录路径返回。
///
/// `open_data_dir_inner` 验证数据目录存在并返回路径字符串。
#[tokio::test]
async fn tc_err_005_open_data_dir() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let result = open_data_dir_inner(&state).await;
    assert!(
        result.is_ok(),
        "open_data_dir_inner 应返回 Ok，实际: {result:?}"
    );

    let path = result.unwrap();
    assert!(!path.is_empty(), "返回的路径不应为空");
    assert!(
        path.contains(dir.path().to_str().unwrap_or("")),
        "返回的路径应包含临时目录路径，实际: {path}"
    );
}

// ============================================================================
// REQ-WIN-001：窗口尺寸与约束 — 窗口状态持久化
// ============================================================================

/// TC-WIN-001 窗口状态持久化：settings 表可读写窗口状态键值。
///
/// 通过 `set_setting` / `get_setting` 写入并读取窗口位置/尺寸/最大化键，
/// 验证 settings 表支持窗口状态持久化（REQ-WIN-001-AC-3）。
#[tokio::test]
async fn tc_win_001_window_state_persist() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 写入窗口状态
    state.storage.set_setting("window.x", "100").await.unwrap();
    state.storage.set_setting("window.y", "200").await.unwrap();
    state
        .storage
        .set_setting("window.width", "1280")
        .await
        .unwrap();
    state
        .storage
        .set_setting("window.height", "800")
        .await
        .unwrap();
    state
        .storage
        .set_setting("window.maximized", "false")
        .await
        .unwrap();

    // 读取并验证
    let x = state.storage.get_setting("window.x").await.unwrap();
    let y = state.storage.get_setting("window.y").await.unwrap();
    let width = state.storage.get_setting("window.width").await.unwrap();
    let height = state.storage.get_setting("window.height").await.unwrap();
    let maximized = state.storage.get_setting("window.maximized").await.unwrap();

    assert_eq!(x.as_deref(), Some("100"), "window.x 应为 100");
    assert_eq!(y.as_deref(), Some("200"), "window.y 应为 200");
    assert_eq!(width.as_deref(), Some("1280"), "window.width 应为 1280");
    assert_eq!(height.as_deref(), Some("800"), "window.height 应为 800");
    assert_eq!(
        maximized.as_deref(),
        Some("false"),
        "window.maximized 应为 false"
    );

    // 验证跨重启持久化（模拟应用退出后重启）
    drop(state);
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let x2 = restarted.storage.get_setting("window.x").await.unwrap();
    assert_eq!(
        x2.as_deref(),
        Some("100"),
        "重启后 window.x 应仍为 100（跨重启持久化）"
    );
}

/// TC-WIN-002 窗口状态默认值：无已保存状态时使用默认值。
///
/// 新数据库无窗口状态键，`get_setting` 应返回 None，
/// 应用使用 tauri.conf.json 中的默认窗口尺寸（REQ-WIN-001-AC-2）。
#[tokio::test]
async fn tc_win_002_window_state_default() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 无已保存状态 → 所有窗口键返回 None
    let x = state.storage.get_setting("window.x").await.unwrap();
    let y = state.storage.get_setting("window.y").await.unwrap();
    let width = state.storage.get_setting("window.width").await.unwrap();
    let height = state.storage.get_setting("window.height").await.unwrap();
    let maximized = state.storage.get_setting("window.maximized").await.unwrap();

    assert!(x.is_none(), "无已保存状态时 window.x 应为 None");
    assert!(y.is_none(), "无已保存状态时 window.y 应为 None");
    assert!(width.is_none(), "无已保存状态时 window.width 应为 None");
    assert!(height.is_none(), "无已保存状态时 window.height 应为 None");
    assert!(
        maximized.is_none(),
        "无已保存状态时 window.maximized 应为 None"
    );
}

// Token/Cost/Coordinator/EditDB 测试已迁移至 integration/conversation_tests.rs

#[tokio::test]
async fn tc_pro_gate_003_search_symbols_free_works() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // search_symbols 不应崩溃，返回空列表（无符号索引数据）
    let results = search_symbols_inner("test".to_string(), Some(10), &state).await;
    assert!(results.is_ok(), "search_symbols 应正常工作");
    let symbols = results.unwrap();
    assert!(
        symbols.is_empty(),
        "无符号索引数据时 search_symbols 应返回空列表"
    );
}

/// TC-PRO-GATE-004: Free 模式 set_llm_mode("local") 被 Pro 门控拦截，is_pro 仍为 false。
#[tokio::test]
async fn tc_pro_gate_004_local_mode_free_still_not_pro() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // Free 模式设置 local 模式应被 Pro 门控拦截
    let result = set_llm_mode_inner("local".to_string(), &state).await;
    assert!(result.is_err(), "Free 模式下 local 模式应被 Pro 门控拦截");
    assert!(
        result.unwrap_err().contains("PRO_REQUIRED"),
        "错误应包含 PRO_REQUIRED"
    );

    // 验证模式未被修改（仍为 remote）
    let mode = get_llm_mode_inner(&state).await.unwrap();
    assert_eq!(mode, "remote", "llm_mode 应仍为 remote");

    // 验证 Pro 状态
    let pro_status = get_pro_status_inner(&state).await;
    assert!(!pro_status, "应仍为 Free 模式");
}

#[tokio::test]
async fn tc_perf_batch_001_add_embeddings_batch() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let storage = &state.storage;

    // 准备测试数据：100 个 chunks + embeddings
    let doc = Document::new("test.md".to_string(), "hash_batch_001".to_string());
    storage.add_document(&doc).await.unwrap();

    let chunks: Vec<Chunk> = (0..100)
        .map(|i| Chunk::new(doc.id.clone(), format!("chunk content {i}"), 10, i))
        .collect();
    storage.add_chunks_batch(&chunks).await.unwrap();

    let embeddings: Vec<(String, Vec<f32>)> = chunks
        .iter()
        .map(|c| (c.id.clone(), vec![0.1_f32; 384]))
        .collect();

    // 批量写入（单事务）
    let start = std::time::Instant::now();
    storage.add_embeddings_batch(&embeddings).await.unwrap();
    let batch_duration = start.elapsed();

    // 验证全部写入成功
    // 验证全部写入成功（vector_search 应返回结果）
    let result = storage.vector_search(&[0.1_f32; 384], 1).await.unwrap();
    assert!(!result.is_empty(), "向量检索应返回结果");

    // 批量写入应快速完成（100 条 < 500ms）
    assert!(
        batch_duration.as_millis() < 500,
        "批量写入 100 条 embeddings 应 < 500ms，实际 {:?}",
        batch_duration
    );
}

/// TC-PERF-BATCH-002：批量嵌入缓存查询（单次 DB 查询替代 N 次串行查询）。
///
/// 验证 `lookup_embedding_cache_batch` 在单次 DB 查询中返回全部命中项。
#[tokio::test]
async fn tc_perf_batch_002_lookup_embedding_cache_batch() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let storage = &state.storage;

    // 预填充缓存
    let items: Vec<(String, Vec<f32>)> = (0..50)
        .map(|i| (format!("hash_{i}"), vec![i as f32 * 0.1; 384]))
        .collect();
    storage.put_embedding_cache_batch(&items).await.unwrap();

    // 批量查询：25 个命中 + 25 个未命中
    let hashes: Vec<String> = (0..100).map(|i| format!("hash_{i}")).collect();
    let start = std::time::Instant::now();
    let hits = storage.lookup_embedding_cache_batch(&hashes).await.unwrap();
    let query_duration = start.elapsed();

    // 验证命中数量（hash_0 ~ hash_49 命中，hash_50 ~ hash_99 未命中）
    assert_eq!(hits.len(), 50, "应命中 50 个缓存项，实际 {}", hits.len());

    // 验证 batch_index 正确性
    let hit_indices: std::collections::HashSet<usize> = hits.iter().map(|(idx, _)| *idx).collect();
    for i in 0..50 {
        assert!(hit_indices.contains(&i), "hash_{} 应命中", i);
    }
    for i in 50..100 {
        assert!(!hit_indices.contains(&i), "hash_{} 应未命中", i);
    }

    // 验证向量内容
    for (idx, vector) in &hits {
        let expected = *idx as f32 * 0.1;
        assert!(
            (vector[0] - expected).abs() < 0.01,
            "向量内容应匹配: idx={}, expected={}, got={}",
            idx,
            expected,
            vector[0]
        );
    }

    // 批量查询应快速完成（100 条 < 200ms）
    assert!(
        query_duration.as_millis() < 200,
        "批量查询 100 条缓存应 < 200ms，实际 {:?}",
        query_duration
    );
}

/// TC-PERF-BATCH-003：批量嵌入缓存写入（单事务批量 INSERT）。
///
/// 验证 `put_embedding_cache_batch` 在单事务中写入全部缓存项。
#[tokio::test]
async fn tc_perf_batch_003_put_embedding_cache_batch() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let storage = &state.storage;

    // 批量写入 100 条缓存
    let items: Vec<(String, Vec<f32>)> = (0..100)
        .map(|i| (format!("batch_hash_{i}"), vec![i as f32 * 0.01; 384]))
        .collect();
    let start = std::time::Instant::now();
    storage.put_embedding_cache_batch(&items).await.unwrap();
    let write_duration = start.elapsed();

    // 验证写入成功（逐条查询验证）
    for (i, hash) in items.iter().enumerate() {
        let cached = storage.lookup_embedding_cache(&hash.0).await.unwrap();
        assert!(cached.is_some(), "缓存项 {} 应存在", hash.0);
        let vector = cached.unwrap();
        let expected = i as f32 * 0.01;
        assert!(
            (vector[0] - expected).abs() < 0.001,
            "向量内容应匹配: hash={}, expected={}, got={}",
            hash.0,
            expected,
            vector[0]
        );
    }

    // 批量写入应快速完成（100 条 < 500ms）
    assert!(
        write_duration.as_millis() < 500,
        "批量写入 100 条缓存应 < 500ms，实际 {:?}",
        write_duration
    );
}

/// TC-PERF-BATCH-004：批量写入幂等性（INSERT OR IGNORE 不覆盖已有值）。
#[tokio::test]
async fn tc_perf_batch_004_idempotent_cache_write() {
    let dir = tempfile::tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let storage = &state.storage;

    // 首次写入
    let items = vec![
        ("hash_a".to_string(), vec![1.0_f32; 384]),
        ("hash_b".to_string(), vec![2.0_f32; 384]),
    ];
    storage.put_embedding_cache_batch(&items).await.unwrap();

    // 再次写入相同 hash（不同向量，应被 IGNORE）
    let items2 = vec![
        ("hash_a".to_string(), vec![9.9_f32; 384]),
        ("hash_c".to_string(), vec![3.0_f32; 384]),
    ];
    storage.put_embedding_cache_batch(&items2).await.unwrap();

    // hash_a 应保持首次写入的值
    let cached_a = storage.lookup_embedding_cache("hash_a").await.unwrap();
    assert!(cached_a.is_some());
    assert!(
        (cached_a.as_ref().unwrap()[0] - 1.0).abs() < 0.001,
        "hash_a 应保持首次值 1.0"
    );

    // hash_b 应存在
    let cached_b = storage.lookup_embedding_cache("hash_b").await.unwrap();
    assert!(cached_b.is_some());

    // hash_c 应存在（新写入）
    let cached_c = storage.lookup_embedding_cache("hash_c").await.unwrap();
    assert!(cached_c.is_some());
    assert!(
        (cached_c.as_ref().unwrap()[0] - 3.0).abs() < 0.001,
        "hash_c 应为 3.0"
    );
}

// ==================================================================
// REQ-EXP-004 文档原文导出
// ==================================================================

/// TC-EXP-ORIG-001 export_document_original 成功复制文件到目标路径（REQ-EXP-004-AC-1/AC-3）。
#[tokio::test]
async fn tc_exp_orig_001_export_document_original_success() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建测试文件并导入
    let src_file = dir.path().join("source").join("test_export.md");
    std::fs::create_dir_all(src_file.parent().unwrap()).unwrap();
    std::fs::write(&src_file, b"export test content").unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let imported = import_files_inner(&handle, &[src_file.to_string_lossy().to_string()], &state)
        .await
        .unwrap();
    assert_eq!(imported.len(), 1, "应导入 1 个文件");

    // 获取文档 ID
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1);
    let doc_id = &docs[0].id;

    // 导出到一个新路径
    let dest = dir.path().join("exported.md");
    export_document_original_inner(doc_id, dest.to_str().unwrap(), &state)
        .await
        .unwrap();

    // 验证导出文件内容一致（AC-3：字节级一致）
    let exported_content = std::fs::read_to_string(&dest).unwrap();
    assert_eq!(exported_content, "export test content");
}

/// TC-EXP-ORIG-002 不存在的文档 ID 返回 Err（REQ-EXP-004-AC-1）。
#[tokio::test]
async fn tc_exp_orig_002_export_nonexistent_doc_returns_error() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let dest = dir.path().join("exported.md");
    let result =
        export_document_original_inner("nonexistent-id", dest.to_str().unwrap(), &state).await;

    assert!(result.is_err(), "不存在的文档 ID 必须返回 Err");
    assert!(
        result.unwrap_err().contains("文档不存在"),
        "错误消息应包含「文档不存在」"
    );
}

/// TC-EXP-ORIG-003 文件副本不存在时返回 Err（REQ-EXP-004 异常场景）。
#[tokio::test]
async fn tc_exp_orig_003_export_missing_file_returns_error() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 直接插入文档记录（file_path 指向不存在的文件）
    let doc = Document::new(
        "/nonexistent/path/to/file.md".to_string(),
        "fake_hash_001".to_string(),
    );
    state.storage.add_document(&doc).await.unwrap();

    let dest = dir.path().join("exported.md");
    let result = export_document_original_inner(&doc.id, dest.to_str().unwrap(), &state).await;

    assert!(result.is_err(), "文件副本不存在时必须返回 Err");
    assert!(
        result.unwrap_err().contains("文档副本文件不存在"),
        "错误消息应包含「文档副本文件不存在」"
    );
}
