//! Tauri Command 层（契约见 SRS：REQ-ING-001、REQ-RAG-001、REQ-LLM-002、REQ-UI-008）。
//! 体系三：全部 Result 显式处理，零 unwrap；错误以可读消息返回前端（不泄露敏感信息）。
//! `*_inner` 函数泛型化 Runtime，供 Tauri 命令与 L2 集成测试（MockRuntime）复用。

use std::path::Path;

use echomind_core::Storage;
use echomind_core::chat::{ChatEngine, ChatOutcome};
use echomind_core::import::{ImportOutcome, ImportService};
use echomind_core::retriever::VectorRetriever;
use echomind_infra::openai_provider::OpenAIProvider;
use echomind_models::{ChatMessage, DocStatusPayload, LlmConfig, RetrievalResult, SettingsPayload};
use futures::StreamExt;
use futures::stream::BoxStream;
use tauri::{AppHandle, Emitter, Runtime, State};

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

// ---- 事件发射助手（发射失败仅记录日志，不中断主流程）----

fn emit_status<R: Runtime>(app: &AppHandle<R>, status: &str, message: String) {
    let payload = DocStatusPayload {
        status: status.to_string(),
        message,
    };
    if let Err(err) = app.emit("doc-status-changed", payload) {
        eprintln!("doc-status-changed 事件发射失败: {err}");
    }
}

fn emit_chat_token<R: Runtime>(app: &AppHandle<R>, token: String) {
    if let Err(err) = app.emit("chat_token", token) {
        eprintln!("chat_token 事件发射失败: {err}");
    }
}

fn emit_chat_sources<R: Runtime>(app: &AppHandle<R>, sources: &[RetrievalResult]) {
    if let Err(err) = app.emit("chat_sources", sources.to_vec()) {
        eprintln!("chat_sources 事件发射失败: {err}");
    }
}

fn emit_chat_error<R: Runtime>(app: &AppHandle<R>, message: String) {
    if let Err(err) = app.emit("chat_error", message) {
        eprintln!("chat_error 事件发射失败: {err}");
    }
}

fn emit_chat_done<R: Runtime>(app: &AppHandle<R>) {
    if let Err(err) = app.emit("chat_done", ()) {
        eprintln!("chat_done 事件发射失败: {err}");
    }
}

// ---- import_files（REQ-ING-001）----

/// 导入文件。返回成功入库的原始文件名列表；格式非法 / 配额触顶 / 未授权 PDF / 路径遍历
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

// ---- chat（REQ-RAG-001）----

/// 流式对话。事件序列：`chat_sources?` → `chat_token*` → `chat_done`；
/// 任何失败先推 `chat_error` 再返回 Err。`conversation_id` 为会话持久化预留（REQ-RAG-006）。
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

/// 对话编排（命令与集成测试复用）：空库拦截 → 配置检查 → 引擎初始化 → 检索对话。
pub async fn chat_inner<R: Runtime>(
    app: &AppHandle<R>,
    query: &str,
    history: &[ChatMessage],
    _conversation_id: &str,
    state: &AppState,
) -> Result<(), String> {
    // 空知识库前置拦截（REQ-RAG-003-AC-2）：明确引导，不触碰向量化引擎与网络
    let doc_count = state
        .storage
        .count_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    if doc_count == 0 {
        return Err("知识库为空：请先通过左下角 + 号导入文档".to_string());
    }

    let llm_config = state
        .llm_config
        .read()
        .await
        .clone()
        .ok_or_else(|| "未配置 LLM：请完成初始配置向导".to_string())?;

    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?;

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
async fn forward_stream<R: Runtime>(
    app: &AppHandle<R>,
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

// ---- test_llm_connection（REQ-LLM-002）----

/// 测试 LLM 连接：极简请求验证 API Key 与 URL 有效性。
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

// ---- update_llm_config / get_settings（REQ-UI-008）----

/// 更新 LLM 配置：写入 settings 加密表并刷新运行态（无需重启）。
#[tauri::command]
pub async fn update_llm_config(
    config: LlmConfig,
    state: State<'_, AppState>,
) -> Result<(), String> {
    update_llm_config_inner(config, state.inner()).await
}

/// 配置写入逻辑（命令与集成测试复用）。
pub async fn update_llm_config_inner(config: LlmConfig, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("llm.api_key", &config.api_key)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.base_url", &config.base_url)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state
        .storage
        .set_setting("llm.model", &config.model)
        .await
        .map_err(|e| format!("{e:#}"))?;
    *state.llm_config.write().await = Some(config);
    Ok(())
}

/// 读取配置状态（Key 仅脱敏返回，严禁回传明文，安全官要求）。
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    get_settings_inner(state.inner()).await
}

/// 配置读取逻辑（命令与集成测试复用）。
pub async fn get_settings_inner(state: &AppState) -> Result<SettingsPayload, String> {
    let config = state.llm_config.read().await.clone();
    Ok(match config {
        Some(c) => SettingsPayload {
            has_llm_config: true,
            base_url: c.base_url,
            model: c.model,
            api_key_masked: mask_api_key(&c.api_key),
        },
        None => SettingsPayload {
            has_llm_config: false,
            base_url: String::new(),
            model: String::new(),
            api_key_masked: String::new(),
        },
    })
}

/// API Key 脱敏：仅保留末四位；不足四位整体遮蔽。
fn mask_api_key(api_key: &str) -> String {
    let char_count = api_key.chars().count();
    if char_count <= 4 {
        return "****".to_string();
    }
    let tail: String = api_key.chars().skip(char_count - 4).collect();
    format!("****{tail}")
}
