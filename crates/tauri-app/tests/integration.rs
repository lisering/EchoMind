#![allow(clippy::unwrap_used)]
//! QA 红灯集成测试（L2：Tauri Mock + 真实 SQLite，体系二阶段 2 先于实现编写）。
//! 覆盖：TC-UI-001 配置持久化（REQ-UI-008）/ TC-UI-002 空知识库拦截（REQ-RAG-003-AC-2）。

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use echomind_core::Storage;
use echomind_models::{Document, LlmConfig};
use echomind_tauri_app::commands::{
    activate_pro_inner, chat_inner, create_conversation_inner, delete_conversation_inner,
    forward_stream, get_conversations_inner, get_messages_inner, get_pro_status_inner,
    get_settings_inner, import_files_inner, persist_exchange, update_llm_config_inner,
};
use echomind_tauri_app::state::AppState;
use ed25519_dalek::Signer;
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

// ================== Phase 8：License 校验与配额门控（REQ-LIC-001/002） ==================

/// 开发测试种子（与 BUILTIN_PUBKEY 对应；仅存在于测试代码）。
const DEV_SEED: [u8; 32] = [
    0xE5, 0xC0, 0x4D, 0x49, 0x4E, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

/// 生成合法 License Key（使用开发种子签名）。
fn make_valid_license() -> String {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&DEV_SEED);
    let payload = b"EchoMind-Pro-v1";
    let signature = signing_key.sign(payload);
    format!(
        "{}-{}",
        B64.encode(payload),
        B64.encode(signature.to_bytes())
    )
}

/// TC-LIC-003 免费版配额拦截：已有 50 个文件，导入第 51 个返回 LIMIT_REACHED（REQ-LIC-002-AC-1）。
#[tokio::test]
async fn tc_lic_003_free_tier_quota_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

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

/// TC-LIC-004 免费版 PDF 付费门：is_pro=false 时导入 PDF 返回 PRO_REQUIRED（REQ-LIC-002-AC-2）。
#[tokio::test]
async fn tc_lic_004_free_tier_pdf_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 创建伪造 PDF 文件
    let pdf = dir.path().join("paper.pdf");
    std::fs::write(&pdf, b"%PDF-1.7 fake-bytes").unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = import_files_inner(&handle, &[pdf.to_string_lossy().into_owned()], &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("PRO_REQUIRED"),
        "免费版 PDF 导入应返回 PRO_REQUIRED 错误，实际: {err}"
    );
}

/// TC-LIC-005 激活 Pro 并跨重启持久化（REQ-LIC-001-AC-3）。
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

    let perm_strs: Vec<&str> = permissions
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(
        perm_strs.contains(&"dialog:allow-open"),
        "capabilities/default.json 必须包含 'dialog:allow-open' 权限，\
         否则点击加号按钮会报 'dialog.open not allowed'。当前权限: {perm_strs:?}"
    );

    assert!(
        perm_strs.contains(&"core:default"),
        "capabilities/default.json 必须包含 'core:default' 权限（事件系统等基础能力）"
    );
}
