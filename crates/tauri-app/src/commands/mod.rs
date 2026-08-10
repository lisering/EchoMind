//! Tauri Command 层（契约见 SRS：REQ-ING-001、REQ-RAG-001/005/006、REQ-LLM-002、REQ-UI-008）。
//! 体系三：全部 Result 显式处理，零 unwrap；错误以可读消息返回前端（不泄露敏感信息）。
//! `*_inner` 函数泛型化 Runtime，供 Tauri 命令与 L2 集成测试（MockRuntime）复用。
// 铁律五例外：#[tauri::command] 宏展开的代码内部使用 unreachable!()，
// Clippy 无法区分宏生成代码与手写代码，此处仅豁免 unreachable lint。
// 本文件手写代码严禁使用 unreachable!()，由 code review 把关。
#![allow(clippy::unreachable)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use md5::{Digest, Md5};
use tracing::{debug, info, warn};

use echomind_compact::CompactionEngine;
use echomind_core::DomainClassifier as _;
use echomind_core::Embedder as _;
use echomind_core::ResponseCache as _;
use echomind_core::Retriever as _;
use echomind_core::Storage;
use echomind_core::agent::{AgentEngine, AgentStepInfo};
use echomind_core::auto_dream::DreamEngine;
use echomind_core::cache::query_hash;
use echomind_core::chat::{ChatEngine, ChatOutcome};
use echomind_core::coordinator::{CoordinatorEngine, CoordinatorPhaseInfo};
use echomind_core::domain::EmbeddingDomainClassifier;
use echomind_core::errors::ERR_PRO_REQUIRED;
use echomind_core::errors::{
    ERR_EMBED, ERR_LLM, ERR_PARSE, ERR_STORAGE, ERR_UNKNOWN, ERR_VALIDATION, MAX_API_KEY_LENGTH,
    MAX_QUERY_LENGTH, classify_llm_error, has_error_prefix, prefix_error,
};
use echomind_core::export::{export_conversation_to_markdown, sanitize_filename};
use echomind_core::hybrid_retriever::HybridRetriever;
use echomind_core::import::{ImportOutcome, ImportService};
use echomind_core::license::verify_license;
use echomind_core::retriever::build_contextual_text;
use echomind_core::session_strip::SessionStripper;
use echomind_core::step_cache::StepCache as _;
use echomind_infra::duckduckgo_provider::DuckDuckGoProvider;
use echomind_infra::hyde_rewriter::HydeRewriter;
use echomind_infra::openai_provider::OpenAIProvider;
use echomind_infra::sqlite_storage::SqliteStorage;
use echomind_models::{
    AgentStepPayload, CacheSettingsPayload, CacheStats, ChatMessage, ChatPhasePayload,
    Conversation, ConversationCost, ConversationTree, ConversationTreeNode, DocStatus,
    DocStatusPayload, Document, EmbeddingProgressPayload, EntityRelation, ExecutionResult,
    GraphCommunity, GraphPath, GraphStats, GraphTriple, ImportProgressPayload, LlmConfig, LlmMode,
    LlmSamplingParams, MemoryEntry, MemoryTier, MessageSearchResult, ModelInfo, PaginatedResult,
    PendingInput, PromptTemplate, RetrievalResult, SearchResult, SessionTodo, SettingsPayload,
    StripConfig, StripPreview, StripResult, TodoStatus, TokenUsage, TurnActiveVersion, Workflow,
    WorkflowResult,
};
use futures::FutureExt;
use futures::StreamExt;
use futures::stream::BoxStream;
use tauri::{AppHandle, Emitter, Runtime, State};
use tokio_util::sync::CancellationToken;

// Pro 模块导入（仅在 --features pro 时编译）
#[cfg(feature = "pro")]
use echomind_core::NoVlm;
#[cfg(feature = "pro")]
use echomind_core::audit::{AuditEngine, AuditOutcome, AuditReport, ContradictionPair, Severity};
#[cfg(feature = "pro")]
use echomind_infra::local_llm::LocalLlmEngine;
#[cfg(feature = "pro")]
use echomind_infra::symbol_engine::SymbolEngine;
#[cfg(feature = "pro")]
use echomind_models::KvCacheStatus;
// REQ-VEC-014: 自定义 ONNX 嵌入模型上传（Pro 门控）
#[cfg(feature = "pro")]
use echomind_infra::local_embedder::CustomModelInfo;

use crate::state::AppState;

// ============================================================================
// 子模块声明（S65 SRP 重构：7706 行 → 12 功能域子模块）
// ============================================================================
mod agent;
mod audit;
mod chat;
mod conversation;
mod document;
mod features;
mod graph;
mod import;
mod license;
mod local_llm;
mod performance;
mod rag_eval;
mod security;
mod settings;
mod sync;
mod trace;

// ============================================================================
// pub use 重导出（保持 lib.rs generate_handler! 不变）
// ============================================================================
pub use agent::*;
pub use audit::*;
pub use chat::*;
pub use conversation::*;
pub use document::*;
pub use features::*;
pub use graph::*;
pub use import::*;
pub use license::*;
pub use local_llm::*;
pub use performance::*;
pub use rag_eval::*;
pub use security::*;
pub use settings::*;
pub use sync::*;
pub use trace::*;

// ============================================================================
// 共享类型 / 常量 / 辅助函数（各子模块通过 super::* 访问）
// ============================================================================

/// LLM Provider 运行时切换包装器（REQ-LLM-003）。
///
/// 根据 `llm_mode` 选择远程 API 或本地推理引擎。
/// 实现 `LLMProvider` trait，可直接传入 `ChatEngine`。
enum LlmProvider {
    /// 远程 BYOK API（OpenAI 兼容）
    Remote(OpenAIProvider),
    /// 本地推理（mistral.rs，Pro 功能）
    #[cfg(feature = "pro")]
    Local(LocalLlmEngine),
}

impl echomind_core::LLMProvider for LlmProvider {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        match self {
            LlmProvider::Remote(p) => p.chat_stream(system_prompt, history, query).await,
            #[cfg(feature = "pro")]
            LlmProvider::Local(e) => e.chat_stream(system_prompt, history, query).await,
        }
    }

    /// 转发分段式流式对话到内部 Provider（Prompt Caching 优化）。
    ///
    /// 必须覆盖此方法：trait 默认实现会将两段拼接后调用 `chat_stream`，
    /// 导致 `OpenAIProvider::chat_stream_segmented` 的双 system 消息优化被绕过。
    /// 显式转发确保 `OpenAIProvider` 能发射两条独立 system 消息，命中 API 端 prompt caching。
    async fn chat_stream_segmented(
        &self,
        static_prefix: &str,
        dynamic_context: &str,
        history: &[ChatMessage],
        query: &str,
    ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
        match self {
            LlmProvider::Remote(p) => {
                p.chat_stream_segmented(static_prefix, dynamic_context, history, query)
                    .await
            }
            #[cfg(feature = "pro")]
            LlmProvider::Local(e) => {
                e.chat_stream_segmented(static_prefix, dynamic_context, history, query)
                    .await
            }
        }
    }

    // Q10 辅助模型方法转发（借鉴 QM HarnessModelUtilities）

    async fn generate_title(&self, transcript: &str) -> anyhow::Result<Option<String>> {
        match self {
            LlmProvider::Remote(p) => p.generate_title(transcript).await,
            #[cfg(feature = "pro")]
            LlmProvider::Local(e) => e.generate_title(transcript).await,
        }
    }

    async fn one_shot(&self, system: &str, prompt: &str) -> anyhow::Result<Option<String>> {
        match self {
            LlmProvider::Remote(p) => p.one_shot(system, prompt).await,
            #[cfg(feature = "pro")]
            LlmProvider::Local(e) => e.one_shot(system, prompt).await,
        }
    }

    async fn judge(&self, system: &str, prompt: &str) -> anyhow::Result<Option<String>> {
        match self {
            LlmProvider::Remote(p) => p.judge(system, prompt).await,
            #[cfg(feature = "pro")]
            LlmProvider::Local(e) => e.judge(system, prompt).await,
        }
    }

    fn context_token_budget(&self) -> Option<usize> {
        match self {
            LlmProvider::Remote(p) => p.context_token_budget(),
            #[cfg(feature = "pro")]
            LlmProvider::Local(e) => e.context_token_budget(),
        }
    }
}

impl LlmProvider {
    /// 返回推理接收端共享句柄（Arc），可在 provider 被 move 进引擎前克隆。
    pub fn reasoning_rx_handle(
        &self,
    ) -> Option<
        std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>>,
    > {
        match self {
            LlmProvider::Remote(p) => Some(p.reasoning_rx_handle()),
            #[cfg(feature = "pro")]
            LlmProvider::Local(_) => None,
        }
    }
}

/// 启动推理内容转发任务：将 provider 的 reasoning_content 增量逐条
/// 发射为 `chat_reasoning` 事件（前端追加到思考面板展开内容）。
///
/// 接收句柄（Arc）需在 provider 被 move 进引擎前获取；任务内部轮询等待
/// `chat_stream` 建立 channel（引擎内部调用）。前置 LLM 调用（上下文压缩/
/// HyDE 改写）与最终回答流可能各自建立 channel，任务循环消费直到全部结束。
///
/// `collector`（可选）：若提供，每条 reasoning 增量同步追加到累积器，
/// 供 `forward_stream` 返回后落库持久化（历史消息重现思考过程）。
fn spawn_reasoning_forwarder<R: Runtime>(
    app: &AppHandle<R>,
    handle: Option<
        std::sync::Arc<tokio::sync::Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<String>>>>,
    >,
    collector: Option<std::sync::Arc<std::sync::Mutex<String>>>,
) {
    let app = app.clone();
    tokio::spawn(async move {
        let Some(handle) = handle else { return };
        loop {
            // 等待一个新 receiver（50ms 轮询，60s 上限——流结束且无新调用时退出）
            let rx = match tokio::time::timeout(std::time::Duration::from_secs(60), async {
                loop {
                    if let Some(rx) = handle.lock().await.take() {
                        return rx;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            })
            .await
            {
                Ok(rx) => rx,
                Err(_) => return,
            };
            // 消费该流的推理增量，直到流结束（channel 关闭）
            let mut rx = rx;
            while let Some(text) = rx.recv().await {
                if let Err(e) = app.emit("chat_reasoning", text.clone()) {
                    warn!("chat_reasoning 事件发射失败: {e}");
                }
                // 落库累积：同步追加（unbounded channel 无背压，stream 结束即已全部入队）
                if let Some(c) = &collector
                    && let Ok(mut acc) = c.lock()
                {
                    acc.push_str(&text);
                }
            }
            // 继续等待下一个推理流（Agent 等场景可能有多次 LLM 调用）
        }
    });
}

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

/// 向量化引擎初始化超时（秒）。
///
/// 首次使用需下载 ONNX 模型（~30MB），网络良好时 10~30 秒完成。
/// 超时阈值设为 180 秒以容忍慢速网络；超时后前端收到 `EMBED:` 错误并提示用户检查网络。
/// 此前无超时保护，当 HuggingFace 不可达时 chat 命令永久阻塞，前端卡死在「初始化向量化引擎」。
///
/// 测试可通过环境变量 `ECHOMIND_EMBEDDER_TIMEOUT` 覆盖（如设为 1 用于集成测试）。
fn embedder_init_timeout() -> u64 {
    std::env::var("ECHOMIND_EMBEDDER_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(180)
}

/// 从原始路径提取展示用文件名。
fn display_name(raw_path: &str) -> String {
    Path::new(raw_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw_path.to_string())
}

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
        warn!("doc-status-changed 事件发射失败: {err}");
    }
}

/// 发射导入进度事件（REQ-ING-006）：推送已完成数 / 总数 / 当前文件名。
fn emit_import_progress<R: Runtime>(
    app: &AppHandle<R>,
    completed: usize,
    total: usize,
    current_file: &str,
    cancelled: bool,
) {
    let payload = ImportProgressPayload {
        completed,
        total,
        current_file: current_file.to_string(),
        cancelled,
    };
    if let Err(err) = app.emit("import-progress", payload) {
        warn!("import-progress 事件发射失败: {err}");
    }
}

fn emit_chat_token<R: Runtime>(app: &AppHandle<R>, token: String) {
    if let Err(err) = app.emit("chat_token", token) {
        warn!("chat_token 事件发射失败: {err}");
    }
}

fn emit_chat_sources<R: Runtime>(app: &AppHandle<R>, sources: &[RetrievalResult]) {
    if let Err(err) = app.emit("chat_sources", sources.to_vec()) {
        warn!("chat_sources 事件发射失败: {err}");
    }
}

fn emit_chat_error<R: Runtime>(app: &AppHandle<R>, message: String) {
    if let Err(err) = app.emit("chat_error", message) {
        warn!("chat_error 事件发射失败: {err}");
    }
}

fn emit_chat_done<R: Runtime>(app: &AppHandle<R>, usage: Option<TokenUsage>) {
    if let Err(err) = app.emit("chat_done", usage) {
        warn!("chat_done 事件发射失败: {err}");
    }
}

/// 将缓存答案按字符块切分，模拟流式输出（前端打字效果与正常回答一致）。
fn split_cached_answer(answer: &str, chunk_size: usize) -> Vec<String> {
    let chars: Vec<char> = answer.chars().collect();
    if chunk_size == 0 {
        return vec![answer.to_string()];
    }
    chars
        .chunks(chunk_size)
        .map(|c| c.iter().collect())
        .collect()
}

/// 正常回答完成后写入查询缓存（REQ-PERF-001）：L0 精确 + L1 语义双写，
/// 下次相同/语义相似问题直接命中缓存秒回。写入失败仅告警，不影响主流程。
async fn write_query_cache(
    state: &AppState,
    query: &str,
    answer: &str,
    sources: &Option<Vec<RetrievalResult>>,
    conversation_id: &str,
) {
    if answer.is_empty() {
        return;
    }
    let sources_json = sources
        .as_ref()
        .and_then(|s| serde_json::to_string(s).ok())
        .unwrap_or_default();
    let qhash = query_hash(query);
    if let Err(e) = state
        .cache
        .insert_exact(&qhash, query, answer, &sources_json, Some(conversation_id))
        .await
    {
        warn!("写入精确缓存失败: {e:#}");
    }
    // 语义缓存需要查询嵌入（embedder 已就绪）
    if let Ok(emb) = state.embedder().await
        && let Ok(vec) = emb.embed(query).await
        && let Err(e) = state
            .cache
            .insert_semantic(query, &vec, answer, &sources_json, Some(conversation_id))
            .await
    {
        warn!("写入语义缓存失败: {e:#}");
    }
}

/// 发射对话阶段事件（REQ-RAG-001 扩展）：在首个 token 到达前推送进度，消除空白等待。
fn emit_chat_phase<R: Runtime>(app: &AppHandle<R>, phase: &str, message: &str) {
    let payload = ChatPhasePayload {
        phase: phase.to_string(),
        message: message.to_string(),
    };
    if let Err(err) = app.emit("chat_phase", payload) {
        warn!("chat_phase 事件发射失败: {err}");
    }
}

/// 文件大小警告阈值（REQ-ING-013 AC-1）：超过此大小的文件导入前弹警告确认。
/// 已提升至 2GB 以支持超大文档（导入链路为流式/零拷贝路径，内存与文件大小无关）。
const FILE_SIZE_WARN_THRESHOLD: u64 = 2 * 1024 * 1024 * 1024; // 2 GB

/// 文件大小硬上限（REQ-ING-013 AC-2）：超过此大小直接拒绝导入。
/// 安全网上限 10GB：索引/嵌入为分批处理，超大文件亦可完成。
const FILE_SIZE_HARD_LIMIT: u64 = 10 * 1024 * 1024 * 1024; // 10 GB

/// 嵌入微批次大小（GB 级文档加速）：每批处理 64 条 chunk，
/// 平衡 ONNX 批量推理效率与进度反馈粒度。
/// 64 条 × 256 tokens ≈ 16K tokens/批，单批推理约 200ms（8 核 CPU）。
const EMBED_BATCH_SIZE: usize = 64;

/// 流转发结果（REQ-RAG-005：completed=false 表示被中断，content 为已生成部分）。
pub struct ForwardResult {
    pub completed: bool,
    pub content: String,
    /// 本次对话的 token 用量（远程 API 模式下携带，本地推理模式为 None）
    pub token_usage: Option<TokenUsage>,
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

/// 自动重试上限（REQ-VEC-005-AC-2）。
const MAX_RETRY_ATTEMPTS: usize = 3;

/// 安全状态信息（返回给前端的安全指示器数据）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SecurityStatus {
    /// 当前安全状态标识：unencrypted / encrypted_unlocked / locked
    pub state: String,
    /// 状态颜色（#ef4444 / #22c55e / #f59e0b）
    pub color: String,
    /// 是否已锁定
    pub is_locked: bool,
    /// 剩余尝试次数（暴力破解防护）
    pub remaining_attempts: u32,
    /// 剩余锁定秒数（频率限制）
    pub remaining_lock_seconds: u32,
}

/// JSON 序列化的监听文件夹条目（存储在 `sync.watched_folders` settings 键中）。
///
/// 仅存储最小必要信息，`WatchedFolderInfo` 的 `sync_status` / `last_synced_at`
/// 在 `get_watched_folders` 命令中根据运行态补充。
#[derive(serde::Serialize, serde::Deserialize)]
struct WatchedFolderEntry {
    path: String,
    last_synced_at: Option<i64>,
}

/// 检索记忆统计条目（返回给前端）。
#[derive(serde::Serialize)]
pub struct RetrievalMemoryStatEntry {
    /// 查询类型
    pub query_type: String,
    /// 检索方法
    pub method: String,
    /// 命中次数
    pub hit_count: u32,
    /// 未命中次数
    pub miss_count: u32,
    /// 命中率（0.0-1.0）
    pub hit_rate: f32,
    /// 平均相关度分数
    pub avg_score: f32,
}

/// 工作流存储键前缀。
const WORKFLOW_KEY_PREFIX: &str = "workflow.";

/// 工作流索引键。
const WORKFLOW_INDEX_KEY: &str = "workflow.index";

/// 模板存储键前缀。
const PROMPT_TEMPLATE_KEY_PREFIX: &str = "prompt_template.";

/// 模板索引键。
const PROMPT_TEMPLATE_INDEX_KEY: &str = "prompt_template.index";

/// 分支创建结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct BranchResult {
    /// 新版本号
    pub new_version: i32,
    /// 所属 turn_group
    pub turn_group: String,
}
