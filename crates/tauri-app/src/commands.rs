//! Tauri Command 层（契约见 SRS：REQ-ING-001、REQ-RAG-001、REQ-LLM-002，TPM 冻结）。
//! 体系三：全部 Result 显式处理，零 unwrap；错误以可读消息返回前端（不泄露敏感信息）。

use std::path::Path;

use echomind_core::chat::{ChatEngine, ChatOutcome};
use echomind_core::import::{ImportOutcome, ImportService};
use echomind_core::retriever::VectorRetriever;
use echomind_infra::openai_provider::OpenAIProvider;
use echomind_models::{ChatMessage, DocStatusPayload, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// RAG 检索默认 top-k（REQ-RAG-003）
const DEFAULT_TOP_K: usize = 5;

/// 从原始路径提取展示用文件名。
fn display_name(raw_path: &str) -> String {
    Path::new(raw_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw_path.to_string())
}

/// 发射文档状态事件（契约：`doc-status-changed` / DocStatusPayload）。
/// 发射失败仅记录日志，不中断主流程。
fn emit_status(app: &AppHandle, status: &str, message: String) {
    let payload = DocStatusPayload {
        status: status.to_string(),
        message,
    };
    if let Err(err) = app.emit("doc-status-changed", payload) {
        eprintln!("doc-status-changed 事件发射失败: {err}");
    }
}

/// 发射对话事件（契约：`chat_token` / `chat_sources` / `chat_error` / `chat_done`）。
fn emit_chat_token(app: &AppHandle, token: String) {
    if let Err(err) = app.emit("chat_token", token) {
        eprintln!("chat_token 事件发射失败: {err}");
    }
}

fn emit_chat_sources(app: &AppHandle, sources: &[RetrievalResult]) {
    if let Err(err) = app.emit("chat_sources", sources.to_vec()) {
        eprintln!("chat_sources 事件发射失败: {err}");
    }
}

fn emit_chat_error(app: &AppHandle, message: String) {
    if let Err(err) = app.emit("chat_error", message) {
        eprintln!("chat_error 事件发射失败: {err}");
    }
}

fn emit_chat_done(app: &AppHandle) {
    if let Err(err) = app.emit("chat_done", ()) {
        eprintln!("chat_done 事件发射失败: {err}");
    }
}

/// 导入文件（REQ-ING-001）。
/// 返回成功入库的原始文件名列表；格式非法 / 配额触顶 / 未授权 PDF / 路径遍历
/// 等错误经 `Err(可读消息)` 返回。
#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    paths: Vec<String>,
    is_pro: bool,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let mut imported = Vec::new();

    for raw_path in &paths {
        let name = display_name(raw_path);
        match service.import_one(raw_path, is_pro).await {
            Ok(ImportOutcome::Imported(doc)) => {
                emit_status(&app, "indexing", format!("正在索引：{name}"));
                match service.index_document(&doc).await {
                    Ok(()) => emit_status(&app, "done", format!("索引完成：{name}")),
                    Err(err) => emit_status(&app, "error", format!("索引失败：{name}（{err}）")),
                }
                imported.push(name);
            }
            Ok(ImportOutcome::SkippedDuplicate(_)) => {
                emit_status(&app, "done", format!("内容已存在，跳过导入：{name}"));
            }
            Err(err) => return Err(err.to_string()),
        }
    }

    Ok(imported)
}

/// 流式对话（REQ-RAG-001）。
/// 事件序列：`chat_sources?` → `chat_token*` → `chat_done`；任何失败先推 `chat_error` 再返回 Err。
/// `conversation_id` 为会话持久化预留（REQ-RAG-006，Phase 5 启用）。
#[tauri::command]
pub async fn chat(
    app: AppHandle,
    query: String,
    history: Vec<ChatMessage>,
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let result = chat_inner(&app, &query, &history, &conversation_id, state.inner()).await;
    if let Err(err) = &result {
        emit_chat_error(&app, err.clone());
    }
    result
}

async fn chat_inner(
    app: &AppHandle,
    query: &str,
    history: &[ChatMessage],
    _conversation_id: &str,
    state: &AppState,
) -> Result<(), String> {
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?;
    let llm_config = state.llm_config.as_ref().ok_or_else(|| {
        "未配置 LLM：请在应用数据目录创建 llm_config.json（api_key / base_url / model）".to_string()
    })?;

    let retriever = VectorRetriever::new(embedder.clone(), state.storage.clone());
    let provider = OpenAIProvider::new(
        llm_config.api_key.clone(),
        llm_config.base_url.clone(),
        llm_config.model.clone(),
    );
    let engine = ChatEngine::new(retriever, provider);

    let outcome = engine
        .chat(history, query, DEFAULT_TOP_K)
        .await
        .map_err(|e| format!("{e:#}"))?;

    match outcome {
        ChatOutcome::Answered { sources, stream } => {
            emit_chat_sources(app, &sources);
            forward_stream(app, stream).await
        }
        ChatOutcome::NoContext { stream } => forward_stream(app, stream).await,
    }
}

/// 将 token 流逐条转发为 chat_token 事件，结束时发射 chat_done。
async fn forward_stream(
    app: &AppHandle,
    mut stream: BoxStream<'static, anyhow::Result<String>>,
) -> Result<(), String> {
    while let Some(item) = stream.next().await {
        match item {
            Ok(token) => emit_chat_token(app, token),
            Err(err) => return Err(format!("{err:#}")),
        }
    }
    emit_chat_done(app);
    Ok(())
}

/// 测试 LLM 连接（REQ-LLM-002）：极简请求验证 API Key 与 URL 有效性。
#[tauri::command]
pub async fn test_llm_connection(
    api_key: String,
    base_url: String,
    model: String,
) -> Result<String, String> {
    let provider = OpenAIProvider::new(api_key, base_url, model);
    provider
        .test_connection()
        .await
        .map_err(|e| format!("{e:#}"))
}
