#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 真实数据端到端集成测试 — 关闭 QA 体系性漏洞 V5（TD-7）。
//!
//! 与 full_pipeline.rs（mock embedder）互补：本文件用**真实 ONNX 嵌入** +
//! **真实 LLM API** 驱动 导入→分块→嵌入→检索→生成 全链路。
//!
//! # 启用条件（全部满足才实际执行，否则静默跳过）
//!
//! | 环境变量 | 说明 |
//! |---|---|
//! | `ECHOMIND_E2E_REAL_DATA=1` | 总开关 |
//! | `ECHOMIND_LLM_API_KEY` | LLM API Key（如 DeepSeek） |
//! | `ECHOMIND_LLM_BASE_URL` | OpenAI 兼容端点（默认 https://api.deepseek.com） |
//! | `ECHOMIND_LLM_MODEL` | 模型名（默认 deepseek-chat） |
//! | `ECHOMIND_REAL_MODEL_DIR` | 含 bge-small-en-v1.5/ 的模型目录（symlink 进 TempDir） |
//! | `ECHOMIND_TEST_DOC` | 测试文档路径（默认 ~/freesoft/lisp-rs/README_zh.md） |
//!
//! # 本地运行
//!
//! ```bash
//! mkdir -p /tmp/echomind-realdata/models
//! ln -s "$HOME/Library/Application Support/com.echomind.app/models/"* \
//!       /tmp/echomind-realdata/models/
//! ECHOMIND_E2E_REAL_DATA=1 \
//! ECHOMIND_REAL_MODEL_DIR=/tmp/echomind-realdata/models \
//! ECHOMIND_LLM_API_KEY=sk-xxx \
//! cargo test -p echomind-tauri-app --test real_data -- --nocapture
//! ```
//!
//! CI：`.github/workflows/real-data-e2e.yml`（手动触发，secrets 注入）。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::common::make_valid_license;
use crate::*;

/// 收集启用条件，全部满足返回 Some(配置)，否则 None（跳过）。
fn real_data_guard() -> Option<(String, String, String, PathBuf, PathBuf)> {
    if std::env::var("ECHOMIND_E2E_REAL_DATA").as_deref() != Ok("1") {
        return None;
    }
    let api_key = std::env::var("ECHOMIND_LLM_API_KEY").ok()?;
    let base_url = std::env::var("ECHOMIND_LLM_BASE_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com".to_string());
    let model = std::env::var("ECHOMIND_LLM_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());

    // 模型目录解析：显式 env 优先（CI 传缓存目录——模型缺失时由 fastembed
    // 首次 embed 自动下载到该目录并被 actions/cache 持久化）；本地回退到
    // macOS 用户数据目录（开发机已有模型，免下载）。
    let model_dir = std::env::var("ECHOMIND_REAL_MODEL_DIR")
        .map(|p| {
            let p = PathBuf::from(p);
            std::fs::create_dir_all(&p).expect("创建模型缓存目录");
            p
        })
        .ok()
        .or_else(|| {
            let p = dirs_home().join("Library/Application Support/com.echomind.app/models");
            p.join("bge-small-en-v1.5").exists().then_some(p)
        })?;

    let doc = std::env::var("ECHOMIND_TEST_DOC")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs_home().join("freesoft/lisp-rs/README_zh.md"));
    if !doc.exists() {
        return None;
    }

    Some((api_key, base_url, model, model_dir, doc))
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}

/// RD-RUST-001：真实导入 → 真实嵌入 → 真实检索 → 真实 LLM 回答全链路。
///
/// 断言：
/// 1. 导入后文档状态达到 Indexed（真实 ONNX 嵌入完成）
/// 2. chat_inner 产出非空 assistant 回答并持久化
/// 3. 回答内容与文档主题相关（包含 README 中的关键词）
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_data_001_full_pipeline_with_real_embeddings_and_llm() {
    let Some((api_key, base_url, model, model_dir, doc_path)) = real_data_guard() else {
        eprintln!(
            "[real-data] 跳过：未满足启用条件（设置 ECHOMIND_E2E_REAL_DATA=1 及 \
             ECHOMIND_LLM_* / ECHOMIND_REAL_MODEL_DIR 环境变量）"
        );
        return;
    };
    eprintln!(
        "[real-data] 启用：model={model}, base_url={base_url}, doc={}",
        doc_path.display()
    );

    // ── 环境准备：TempDir + symlink 模型目录（只读共享，避免重复下载 ~100MB）──
    let dir = TempDir::new().unwrap();
    let models_link = dir.path().join("models");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&model_dir, &models_link).unwrap();
    #[cfg(not(unix))]
    std::fs::create_dir_all(&models_link).unwrap();

    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // Pro 激活（导入配额与格式门控按 Pro 放行）
    let license = make_valid_license();
    activate_pro_inner(&license, &state).await.unwrap();

    // ── 1. 配置真实 LLM ──
    update_llm_config_inner(
        echomind_models::LlmConfig {
            api_key,
            base_url,
            model,
        },
        &state,
    )
    .await
    .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    // ── 2. 真实导入（解析 + 分块 + 实体抽取，嵌入在索引阶段异步完成）──
    let canon = doc_path.canonicalize().unwrap().to_string_lossy().into_owned();
    let imported = import_files_inner(&handle, &[canon], &state).await.unwrap();
    assert!(!imported.is_empty(), "真实文档导入应成功入库");
    let doc_name = imported[0].clone();
    eprintln!("[real-data] 导入成功: {doc_name}");

    // ── 3. 轮询等待真实嵌入完成（Indexed），上限 120s ──
    let start = Instant::now();
    loop {
        // get_documents 无 _inner 变体（薄命令），直接走 storage
        let docs = state.storage.list_documents().await.unwrap();
        let doc = docs.first().expect("导入后至少存在一个文档");
        match &doc.status {
            echomind_models::DocStatus::Indexed => {
                eprintln!(
                    "[real-data] 嵌入完成（{:?}），chunks={}",
                    start.elapsed(),
                    state.storage.list_chunks(&doc.id).await.unwrap().len()
                );
                break;
            }
            echomind_models::DocStatus::Failed(reason) => {
                panic!("真实嵌入失败: {reason}");
            }
            _ => {}
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "120s 内未完成真实嵌入（ONNX 推理超时）"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // ── 4. 真实 RAG 对话 ──
    let conv_id = create_conversation_inner("default".to_string(), &state).await.unwrap();
    let question = "这个项目是做什么的？请简要概括它的核心特性。";
    chat_inner(&handle, question, &[], &conv_id, None, None, &state)
        .await
        .expect("真实 RAG 对话不应失败");

    // ── 5. 从持久化层断言回答 ──
    let messages = get_messages_inner(&conv_id, &state).await.unwrap();
    let assistant = messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .expect("chat_inner 后必须存在 assistant 消息（persist_exchange）");

    eprintln!(
        "[real-data] 回答长度={} 字符，预览: {}",
        assistant.content.len(),
        assistant.content.chars().take(120).collect::<String>()
    );

    assert!(
        assistant.content.chars().count() >= 20,
        "真实 LLM 回答不应为空或过短"
    );
    assert!(
        !assistant.content.contains("PRO_REQUIRED") && !assistant.content.contains("EMBED:"),
        "回答不得包含错误前缀（说明链路走了降级）"
    );
    // 相关性：README_zh 的核心词（lisp/解释器/Rust 等至少命中一个，宽松匹配）
    let lower = assistant.content.to_lowercase();
    assert!(
        ["lisp", "解释", "rust", "语言", "知识"].iter().any(|k| lower.contains(k)),
        "回答应与文档主题相关，实际: {}",
        assistant.content
    );
}
