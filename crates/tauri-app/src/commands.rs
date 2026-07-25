//! Tauri Command 层（契约见 SRS：REQ-ING-001、REQ-RAG-001/005/006、REQ-LLM-002、REQ-UI-008）。
//! 体系三：全部 Result 显式处理，零 unwrap；错误以可读消息返回前端（不泄露敏感信息）。
//! `*_inner` 函数泛型化 Runtime，供 Tauri 命令与 L2 集成测试（MockRuntime）复用。
// 铁律五例外：#[tauri::command] 宏展开的代码内部使用 unreachable!()，
// Clippy 无法区分宏生成代码与手写代码，此处仅豁免 unreachable lint。
// 本文件手写代码严禁使用 unreachable!()，由 code review 把关。
#![allow(clippy::unreachable)]

use std::path::Path;

use echomind_core::Embedder as _;
use echomind_core::NoVlm;
use echomind_core::Storage;
use echomind_core::chat::{ChatEngine, ChatOutcome};
use echomind_core::import::{ImportOutcome, ImportService};
use echomind_core::license::verify_license;
use echomind_core::retriever::{build_contextual_text, VectorRetriever};
use echomind_infra::openai_provider::OpenAIProvider;
use echomind_models::{
    ChatMessage, ChatPhasePayload, Conversation, DocStatus, DocStatusPayload, Document, LlmConfig,
    RetrievalResult, SettingsPayload,
};
use futures::StreamExt;
use futures::stream::BoxStream;
use tauri::{AppHandle, Emitter, Runtime, State};
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

/// RAG 检索默认 top-k（REQ-RAG-003）
/// 8：all-MiniLM-L6-v2 对中文短查询的语义匹配精度有限，top-5 可能遗漏相关片段；
/// top-8 让 LLM 获得更充分的上下文以综合回答（TC-RAG-001c 回归教训）。
const DEFAULT_TOP_K: usize = 8;
/// 会话标题自动提取的最大字符数（REQ-RAG-006）
const TITLE_MAX_CHARS: usize = 24;
/// 新建会话占位标题（REQ-RAG-006）
const PLACEHOLDER_TITLE: &str = "新会话";
/// 单工作区标识（v1.0）
const DEFAULT_WORKSPACE: &str = "default";

/// 从原始路径提取展示用文件名。
fn display_name(raw_path: &str) -> String {
    Path::new(raw_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw_path.to_string())
}

// ---- 事件发射助手（发射失败仅记录日志，不中断主流程）----

fn emit_status<R: Runtime>(app: &AppHandle<R>, status: &str, message: String) {
    emit_status_with_phase(app, status, message, None);
}

/// 发射带子阶段的状态事件（REQ-MM-004 多模态管线进度反馈）。
fn emit_status_with_phase<R: Runtime>(
    app: &AppHandle<R>,
    status: &str,
    message: String,
    sub_phase: Option<&str>,
) {
    let payload = DocStatusPayload {
        status: status.to_string(),
        message,
        sub_phase: sub_phase.map(|s| s.to_string()),
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

/// 发射对话阶段事件（REQ-RAG-001 扩展）：在首个 token 到达前推送进度，消除空白等待。
fn emit_chat_phase<R: Runtime>(app: &AppHandle<R>, phase: &str, message: &str) {
    let payload = ChatPhasePayload {
        phase: phase.to_string(),
        message: message.to_string(),
    };
    if let Err(err) = app.emit("chat_phase", payload) {
        eprintln!("chat_phase 事件发射失败: {err}");
    }
}

// ---- import_files（REQ-ING-001）----

/// 导入文件。返回成功入库的原始文件名列表；格式非法 / 配额触顶 / 未授权 PDF / 路径遍历
/// 等错误经 `Err(可读消息)` 返回。is_pro 从 AppState 读取（REQ-LIC-002 双重拦截）。
#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    import_files_inner(&app, &paths, state.inner()).await
}

/// 导入逻辑（命令与集成测试复用）。
pub async fn import_files_inner<R: Runtime>(
    app: &AppHandle<R>,
    paths: &[String],
    state: &AppState,
) -> Result<Vec<String>, String> {
    let is_pro = *state.is_pro.read().await;
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let mut imported = Vec::new();

    for raw_path in paths {
        let name = display_name(raw_path);
        match service.import_one(raw_path, is_pro).await {
            Ok(ImportOutcome::Imported(doc)) => {
                // 判断是否为 PDF（Pro 版可触发多模态管线，REQ-MM-004）
                let is_pdf = Path::new(&doc.file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("pdf"))
                    .unwrap_or(false);

                let index_result = if is_pdf && is_pro {
                    // Pro 版 PDF：尝试多模态索引（文本 + 渲染 + OCR）
                    index_pdf_multimodal(app, &service, &doc, state, &name).await
                } else {
                    // 非 PDF 或免费版：纯文本索引（现有逻辑）
                    emit_status(app, "indexing", format!("正在索引：{name}"));
                    service
                        .index_document(&doc)
                        .await
                        .map_err(|e| format!("{e:#}"))
                };

                match index_result {
                    Ok(()) => match embed_document_chunks(state, &doc.id, &name).await {
                        Ok(count) => {
                            emit_status(app, "done", format!("索引完成：{name}（{count} 向量）"))
                        }
                        Err(err) => {
                            let _ = state
                                .storage
                                .update_doc_status(
                                    &doc.id,
                                    DocStatus::Failed(format!("向量化失败: {err}")),
                                )
                                .await;
                            emit_status(app, "error", format!("向量化失败：{name}（{err}）"));
                        }
                    },
                    Err(err) => emit_status(app, "error", format!("索引失败：{name}（{err}）")),
                }
                imported.push(name);
            }
            Ok(ImportOutcome::SkippedDuplicate(_)) => {
                emit_status(app, "done", format!("内容已存在，跳过导入：{name}"));
            }
            Err(err) => return Err(err.to_string()),
        }
    }

    Ok(imported)
}

/// Pro 版 PDF 多模态索引（REQ-MM-004）：文本提取 → 图片渲染 → OCR → VLM 增强 → 分块入库。
///
/// VLM 编排（REQ-MM-003 前端接线）：
/// - `mm.vlm_enabled` = true 且 LLM 配置存在 → `index_document_multimodal_with_vlm` + VLM
/// - VLM 禁用或配置缺失 → `index_document_multimodal_with_vlm` + `NoVlm`（管线天然跳过 VLM 阶段）
///
/// 降级策略：OCR 引擎或渲染引擎初始化失败时，回退到纯文本索引（`index_document`），
/// 确保 PDF 导入不因多模态引擎不可用而完全失败。
async fn index_pdf_multimodal<R: Runtime>(
    app: &AppHandle<R>,
    service: &ImportService<echomind_infra::sqlite_storage::SqliteStorage>,
    doc: &echomind_models::Document,
    state: &AppState,
    name: &str,
) -> Result<(), String> {
    // 阶段 1：初始化 OCR 引擎（首次使用需下载模型 ~17MB，可能耗时）
    emit_status_with_phase(app, "indexing", "初始化 OCR 引擎…".to_string(), Some("ocr"));
    let ocr_engine = state.ocr_engine().await;
    let page_renderer = state.page_renderer().await;
    let renderer_ok = page_renderer.is_ok();
    let ocr_ok = ocr_engine.is_ok();

    match (page_renderer, ocr_engine) {
        (Ok(renderer), Ok(ocr)) => {
            // 获取 VLM provider（None 表示禁用或配置缺失，将使用 NoVlm 降级）
            let vlm = state.vision_provider().await.map_err(|e| format!("{e:#}"))?;
            let vlm_on = vlm.is_some();

            // 阶段 2：文本提取 + 图片检测 + 渲染 + OCR + VLM 增强
            let phase_label = if vlm_on { "vlm_enhancing" } else { "text_extracting" };
            emit_status_with_phase(
                app,
                "indexing",
                format!("正在多模态索引：{name}"),
                Some(phase_label),
            );

            match vlm {
                Some(ref vlm_provider) => {
                    // VLM 启用：表格→Markdown、甘特图→Mermaid
                    service
                        .index_document_multimodal_with_vlm(doc, renderer, ocr, vlm_provider)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
                None => {
                    // VLM 禁用或配置缺失：NoVlm 占位，管线天然跳过 VLM 阶段
                    let no_vlm = NoVlm;
                    service
                        .index_document_multimodal_with_vlm(doc, renderer, ocr, &no_vlm)
                        .await
                        .map_err(|e| format!("{e:#}"))
                }
            }
        }
        _ => {
            // 降级：多模态引擎不可用，回退到纯文本索引
            eprintln!(
                "多模态引擎初始化失败（renderer={renderer_ok}, ocr={ocr_ok}），降级为纯文本索引"
            );
            emit_status(app, "indexing", format!("正在索引（纯文本）：{name}"));
            service
                .index_document(doc)
                .await
                .map_err(|e| format!("{e:#}"))
        }
    }
}

/// 嵌入写入链路（REQ-VEC-002/003）：分块读取 → embed_batch → add_embedding。
/// Phase 3 ContextualEmbedding：嵌入文本拼接文档名上下文前缀（`build_contextual_text`），
/// 使向量包含文档上下文信息，提升检索质量（Anthropic Contextual Retrieval）。
/// 失败时由调用方将文档置为 Failed（索引内容保留，可重试）。
async fn embed_document_chunks(state: &AppState, doc_id: &str, doc_name: &str) -> Result<usize, String> {
    let chunks = state
        .storage
        .list_chunks(doc_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if chunks.is_empty() {
        return Ok(0);
    }
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?;
    let texts: Vec<String> = chunks
        .iter()
        .map(|c| build_contextual_text(doc_name, &c.content))
        .collect();
    let vectors = embedder
        .embed_batch(&texts)
        .await
        .map_err(|e| format!("{e:#}"))?;
    for (chunk, vector) in chunks.iter().zip(vectors.iter()) {
        state
            .storage
            .add_embedding(&chunk.id, vector)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(chunks.len())
}

// ---- 文档管理（REQ-UI-006 / REQ-ING-005）----

/// 列出全部文档（知识库面板）。
#[tauri::command]
pub async fn get_documents(state: State<'_, AppState>) -> Result<Vec<Document>, String> {
    state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除文档：级联清理 chunks / embeddings，并删除数据目录中的文件副本（REQ-ING-005）。
#[tauri::command]
pub async fn delete_document(id: String, state: State<'_, AppState>) -> Result<(), String> {
    // 先取副本路径再删记录（删除后无法回查）
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;
    let file_path = docs
        .iter()
        .find(|d| d.id == id)
        .map(|d| d.file_path.clone());

    state
        .storage
        .delete_document(&id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 副本可能已不存在，记录日志但不视为失败
    if let Some(path) = file_path
        && let Err(err) = tokio::fs::remove_file(&path).await
    {
        eprintln!("文档副本清理失败（可忽略）: {path}: {err}");
    }
    Ok(())
}

// ---- chat（REQ-RAG-001/005/006）----

/// 流式对话。事件序列：`chat_sources?` → `chat_token*` → `chat_done`；
/// 失败先推 `chat_error` 再返回 Err；中断时推「已中断」+ `chat_done` 并照常落库。
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

/// 对话编排（命令与集成测试复用）：空库拦截 → 配置检查 → 引擎初始化 → 检索对话 → 落库。
pub async fn chat_inner<R: Runtime>(
    app: &AppHandle<R>,
    query: &str,
    history: &[ChatMessage],
    conversation_id: &str,
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

    // 阶段 1：初始化向量化引擎（首次使用需下载模型，可能耗时较长）
    emit_chat_phase(app, "preparing", "初始化向量化引擎…");
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?;

    // 兜底：会话不存在时幂等创建（正常流程前端先 create_conversation）
    let conversation = Conversation::with_id(
        conversation_id.to_string(),
        DEFAULT_WORKSPACE.to_string(),
        derive_title(query),
    );
    state
        .storage
        .create_conversation(&conversation)
        .await
        .map_err(|e| format!("{e:#}"))?;

    let retriever = VectorRetriever::new(embedder.clone(), state.storage.clone());
    let provider = OpenAIProvider::new(
        llm_config.api_key.clone(),
        llm_config.base_url.clone(),
        llm_config.model.clone(),
    )
    .map_err(|e| format!("{e:#}"))?;
    let engine = ChatEngine::new(retriever, provider);

    // 阶段 2：检索知识库（向量化查询 + top-k 相似度检索）
    emit_chat_phase(app, "retrieving", "检索知识库…");
    let outcome = engine
        .chat(history, query, DEFAULT_TOP_K)
        .await
        .map_err(|e| format!("{e:#}"))?;

    let (sources, stream) = match outcome {
        ChatOutcome::Answered { sources, stream } => {
            emit_chat_sources(app, &sources);
            // 阶段 3：LLM 生成回答（首个 token 到达前的连接与推理延迟）
            emit_chat_phase(app, "generating", "正在生成回答…");
            (Some(sources), stream)
        }
        ChatOutcome::NoContext { stream } => (None, stream),
    };

    let abort_token = state.abort_token_for(conversation_id).await;
    let result = forward_stream(app, stream, abort_token).await?;
    // 正常结束或被中断：均已生成内容照常落库（REQ-RAG-005/006）
    persist_exchange(state, conversation_id, query, &result.content, sources).await?;
    state.clear_abort(conversation_id).await;
    Ok(())
}

/// 流转发结果（REQ-RAG-005：completed=false 表示被中断，content 为已生成部分）。
pub struct ForwardResult {
    pub completed: bool,
    pub content: String,
}

/// 将 token 流逐条转发为 chat_token；abort 令牌触发立即中断（丢弃流即中止 HTTP 连接）。
pub async fn forward_stream<R: Runtime>(
    app: &AppHandle<R>,
    mut stream: BoxStream<'static, anyhow::Result<String>>,
    abort_token: CancellationToken,
) -> Result<ForwardResult, String> {
    let mut content = String::new();
    let mut completed = false;

    loop {
        tokio::select! {
            biased;
            _ = abort_token.cancelled() => break,
            item = stream.next() => {
                match item {
                    None => {
                        completed = true;
                        break;
                    }
                    Some(Ok(token)) => {
                        content.push_str(&token);
                        emit_chat_token(app, token);
                    }
                    Some(Err(err)) => {
                        // 错误经返回值上交，由 chat 包装层统一发射 chat_error（避免重复推送）
                        return Err(format!("{err:#}"));
                    }
                }
            }
        }
    }

    if !completed {
        // 中断语义（REQ-RAG-005 冻结契约）：已生成内容保留并标记
        emit_chat_error(app, "⏹ 生成已中断".to_string());
    }
    emit_chat_done(app);
    Ok(ForwardResult { completed, content })
}

/// 一轮问答落库（REQ-RAG-006）：首轮自动提取标题；空回答（秒中断）跳过 assistant 记录。
pub async fn persist_exchange(
    state: &AppState,
    conversation_id: &str,
    query: &str,
    answer: &str,
    sources: Option<Vec<RetrievalResult>>,
) -> Result<(), String> {
    // 首轮问答后提取标题（仅在尚无历史消息时执行，避免覆盖后续标题）
    let existing = state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    if existing.is_empty() {
        state
            .storage
            .update_conversation_title(conversation_id, &derive_title(query))
            .await
            .map_err(|e| format!("{e:#}"))?;
    }

    let user_msg = ChatMessage {
        role: "user".to_string(),
        content: query.to_string(),
        sources: None,
    };
    state
        .storage
        .add_message(conversation_id, &user_msg)
        .await
        .map_err(|e| format!("{e:#}"))?;

    if !answer.is_empty() {
        let assistant_msg = ChatMessage {
            role: "assistant".to_string(),
            content: answer.to_string(),
            sources,
        };
        state
            .storage
            .add_message(conversation_id, &assistant_msg)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(())
}

/// 标题提取：取问题前 24 个字符，超长追加省略号（REQ-RAG-006）。
fn derive_title(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.chars().count() <= TITLE_MAX_CHARS {
        trimmed.to_string()
    } else {
        format!(
            "{}…",
            trimmed.chars().take(TITLE_MAX_CHARS).collect::<String>()
        )
    }
}

// ---- 会话管理（REQ-RAG-006）----

/// 新建会话，返回唯一 ID（占位标题「新会话」）。
#[tauri::command]
pub async fn create_conversation(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    create_conversation_inner(workspace_id, state.inner()).await
}

/// 新建会话逻辑（命令与集成测试复用）。
pub async fn create_conversation_inner(
    workspace_id: String,
    state: &AppState,
) -> Result<String, String> {
    let conversation = Conversation::new(workspace_id, PLACEHOLDER_TITLE.to_string());
    state
        .storage
        .create_conversation(&conversation)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(conversation.id)
}

/// 列出工作区会话（倒序）。
#[tauri::command]
pub async fn get_conversations(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Conversation>, String> {
    get_conversations_inner(&workspace_id, state.inner()).await
}

/// 会话列表逻辑（命令与集成测试复用）。
pub async fn get_conversations_inner(
    workspace_id: &str,
    state: &AppState,
) -> Result<Vec<Conversation>, String> {
    state
        .storage
        .list_conversations(workspace_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 列出会话消息（正序，含引用块）。
#[tauri::command]
pub async fn get_messages(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    get_messages_inner(&conversation_id, state.inner()).await
}

/// 消息列表逻辑（命令与集成测试复用）。
pub async fn get_messages_inner(
    conversation_id: &str,
    state: &AppState,
) -> Result<Vec<ChatMessage>, String> {
    state
        .storage
        .list_messages(conversation_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除会话（级联清理消息与中断令牌）。
#[tauri::command]
pub async fn delete_conversation(id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_conversation_inner(&id, state.inner()).await
}

/// 删除会话逻辑（命令与集成测试复用）。
pub async fn delete_conversation_inner(id: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .delete_conversation(id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    state.clear_abort(id).await;
    Ok(())
}

// ---- abort_chat（REQ-RAG-005）----

/// 停止生成：触发指定会话的中断令牌，流循环立即 break 并中止网络拉取。
#[tauri::command]
pub async fn abort_chat(conversation_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.abort_chat(&conversation_id).await;
    Ok(())
}

// ---- activate_pro / get_pro_status（REQ-LIC-001/002）----

/// 激活 Pro：验证 License Key 签名，有效则持久化并刷新运行态（REQ-LIC-001）。
#[tauri::command]
pub async fn activate_pro(license_key: String, state: State<'_, AppState>) -> Result<bool, String> {
    activate_pro_inner(&license_key, state.inner()).await
}

/// 激活逻辑（命令与集成测试复用）。
pub async fn activate_pro_inner(license_key: &str, state: &AppState) -> Result<bool, String> {
    match verify_license(license_key) {
        Ok(()) => {
            state
                .storage
                .set_setting("license.is_pro", "true")
                .await
                .map_err(|e| format!("{e:#}"))?;
            *state.is_pro.write().await = true;
            Ok(true)
        }
        Err(e) => Err(format!("License 激活失败: {e:#}")),
    }
}

/// 查询当前授权状态（REQ-LIC-002）。
#[tauri::command]
pub async fn get_pro_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(get_pro_status_inner(state.inner()).await)
}

/// 授权状态查询逻辑（命令与集成测试复用）。
pub async fn get_pro_status_inner(state: &AppState) -> bool {
    *state.is_pro.read().await
}

// ---- test_llm_connection（REQ-LLM-002）----

/// 测试 LLM 连接：极简请求验证 API Key 与 URL 有效性。
#[tauri::command]
pub async fn test_llm_connection(
    api_key: String,
    base_url: String,
    model: String,
) -> Result<String, String> {
    let provider = OpenAIProvider::new(api_key, base_url, model).map_err(|e| format!("{e:#}"))?;
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
/// VLM 开关状态从 settings 表读取（REQ-MM-003 前端接线）。
pub async fn get_settings_inner(state: &AppState) -> Result<SettingsPayload, String> {
    let config = state.llm_config.read().await.clone();
    let vlm_enabled = state
        .storage
        .get_setting("mm.vlm_enabled")
        .await
        .map_err(|e| format!("{e:#}"))?
        .is_some_and(|v| v == "true");
    Ok(match config {
        Some(c) => SettingsPayload {
            has_llm_config: true,
            base_url: c.base_url,
            model: c.model,
            api_key_masked: mask_api_key(&c.api_key),
            vlm_enabled,
        },
        None => SettingsPayload {
            has_llm_config: false,
            base_url: String::new(),
            model: String::new(),
            api_key_masked: String::new(),
            vlm_enabled,
        },
    })
}

// ---- set_vlm_enabled（REQ-MM-003 前端接线）----

/// 切换 VLM 图片理解增强开关（REQ-MM-003）。
/// 写入 settings 表 `mm.vlm_enabled`，vision_provider() 下次调用时即时生效。
#[tauri::command]
pub async fn set_vlm_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_vlm_enabled_inner(enabled, state.inner()).await
}

/// VLM 开关写入逻辑（命令与集成测试复用）。
pub async fn set_vlm_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("mm.vlm_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
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
