//! 应用状态组装 — Store 模式（借鉴 Zed `buffer_store`/`git_store` 架构）。
//!
//! 将原巨型 `AppState` 按职责拆分为 5 个独立 Store：
//! - `DocumentStore` — 文档管理 + 导入取消 + 文件监听
//! - `ChatStore` — 会话中断令牌 + 审计取消
//! - `SecurityStore` — SecurityManager + ClipboardGuard
//! - `ModelStore` — embedder + reranker + local_llm + model_manager
//! - `ConfigStore` — llm_config + is_pro + llm_mode
//!
//! `AppState` 保留所有便捷访问方法（委托到各 Store），确保 `commands.rs` 无需大改。
//! 存储（`SqliteStorage`）仍由 `AppState` 直接持有，各 Store 通过 `Arc` 共享引用。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Context;
use echomind_core::Storage;
use echomind_core::auto_dream::DreamResult;
use echomind_core::hooks::HookRegistry;
use echomind_infra::file_watcher::FileWatcherHandle;
use echomind_infra::local_embedder::LocalEmbedder;
use echomind_infra::local_logger::LocalLogger;
use echomind_infra::local_reranker::LocalReranker;
use echomind_infra::sqlite_cache::SqliteCache;
use echomind_infra::sqlite_storage::SqliteStorage;
use echomind_models::LlmMode;
use tokio_util::sync::CancellationToken;
// S70: TraceStore 的 RwLock
use tokio::sync::RwLock;
// Q05: SecurityPosture 原子运行时态
use echomind_core::security::SecurityPosture;

// Pro 模块导入
#[cfg(feature = "pro")]
use echomind_infra::local_llm::LocalLlmEngine;
#[cfg(feature = "pro")]
use echomind_infra::ocrs_engine::OcrsEngine;
#[cfg(feature = "pro")]
use echomind_infra::openai_vision::OpenAIVisionProvider;
#[cfg(feature = "pro")]
use echomind_infra::pdfium_renderer::PdfiumRenderer;

// Store 模式模块在 lib.rs 中声明（pub mod stores）
pub use crate::stores::{
    ChatStore, ConfigStore, DocumentStore, ModelStore, SecurityStore, parse_embedding_model,
};

/// Tauri 全局状态（Store 模式重构后）。
///
/// 原始 15+ 字段拆分为 5 个独立 Store + 共享存储/数据目录。
/// 便捷访问方法委托到各 Store，确保 `commands.rs` 向后兼容。
pub struct AppState {
    /// 存储端口实现（SQLite 持久化，WAL 模式）
    /// 所有 Store 通过 `Arc` 共享此实例
    pub storage: SqliteStorage,
    /// 应用数据目录（文档副本、数据库、模型缓存与密钥文件根目录）
    pub data_dir: PathBuf,
    /// 安全管理 Store（SecurityManager + ClipboardGuard）
    pub security_store: SecurityStore,
    /// 配置管理 Store（llm_config + is_pro + llm_mode）
    pub config_store: ConfigStore,
    /// 模型管理 Store（embedder + reranker + local_llm + model_manager）
    pub model_store: ModelStore,
    /// 文档管理 Store（导入取消 + 文件监听）
    pub document_store: DocumentStore,
    /// 会话管理 Store（流中断令牌 + 审计取消）
    pub chat_store: ChatStore,
    /// Dream Engine 状态（后台空闲整理建议）
    pub dream_engine: DreamEngineState,
    /// 语义缓存（REQ-PERF-001：L0 精确 + L1 语义 + L3 检索结果三级缓存）
    pub cache: SqliteCache,
    /// 步骤级缓存（P2-1 StepCache：Agent/Coordinator 多步推理中间步骤复用）
    pub step_cache: Arc<echomind_core::step_cache::InMemoryStepCache>,
    /// Prompt 压缩比（REQ-PERF-002：1.0=禁用, 2.0=保守, 3.0=平衡, 5.0=激进）
    pub compression_ratio: f32,
    /// Speculative RAG 是否启用（REQ-PERF-011：小模型草稿 → 大模型验证）
    pub speculative_enabled: bool,
    /// 自进化检索记忆是否启用（REQ-PERF-012：记录检索效果，自适应选择最佳策略）
    pub retrieval_memory_enabled: bool,
    /// 知识图谱图遍历检索是否启用（REQ-RAG-027：沿实体关系图边扩展到关联 chunk）
    pub graph_retriever_enabled: bool,
    /// RAG 质量门控是否启用（REQ-RAG-028：检索后评估结果质量，低质量时记录告警）
    pub quality_gate_enabled: bool,
    /// 持久化记忆系统是否启用（REQ-RAG-032：对话记忆提取 + 跨会话注入 + AutoDream 整合）
    pub memory_enabled: bool,
    /// 网页搜索集成是否启用（REQ-RAG-036：本地检索不足时搜索互联网补充 context）
    pub web_search_enabled: bool,
    /// Contextual Retrieval 是否启用（REQ-RAG-041：嵌入时拼接文档名上下文前缀，提升检索精度）
    /// 默认 true：嵌入管线已使用 build_contextual_text() 构建上下文文本。
    /// 关闭后新导入文档的嵌入使用纯 chunk 文本（不含文档名前缀）。
    pub contextual_retrieval_enabled: bool,
    /// Agent 生命周期 Hooks 注册表（REQ-RAG-029：插件式扩展 Agent/Coordinator 行为）
    /// `None` = 未注册 hook，引擎行为与之前完全一致
    pub hook_registry: Option<Arc<HookRegistry>>,
    /// RAG 链路追踪存储（S70：Cherry Studio 借鉴 — span 级耗时追踪）
    pub trace_store: Arc<RwLock<echomind_core::trace::TraceStore>>,
    /// Session Run Coordinator（B06 会话运行协调器）：
    /// 同会话串行 / 跨会话并发 / wake 合并 / interrupt 中断。
    pub session_coordinator: Arc<echomind_core::session_coordinator::SessionCoordinator>,
    /// Burst Buffer — 延迟批量记忆提取（Q02 借鉴 QM createBurstBuffer）。
    ///
    /// 聚合多轮对话后批量 LLM 提取记忆，降低 LLM 调用成本。
    /// 每轮 chat_done 后 push 到 buffer；满足条件（静默窗口 / 最大轮次）时自动 flush。
    pub memory_burst_buffer: Arc<tokio::sync::Mutex<echomind_core::memory_store::BurstBuffer>>,
    /// 后台压缩去重集合（Q03：双阈值压缩）。
    ///
    /// 追踪正在进行后台压缩的会话 ID，防止同一会话重复触发。
    /// Soft 阈值触发时 `try_acquire_background_compaction()` 插入；
    /// 压缩完成后 `release_background_compaction()` 移除。
    pub compaction_pending: Arc<tokio::sync::Mutex<HashSet<String>>>,
    /// 安全态势级别（Q05 借鉴 QM SecurityPosture）。
    ///
    /// 三层安全态势（Dangerous / Auto / Strict）控制内容筛查和工具审批。
    /// 默认 Auto。持久化在 settings 表 `security.posture` 键。
    /// 使用 AtomicU8 存储（0=Dangerous, 1=Auto, 2=Strict）实现无锁读。
    pub security_posture: Arc<std::sync::atomic::AtomicU8>,
    /// Shadow 安全筛查统计收集器（Q06 借鉴 QM security-screen.ts）。
    ///
    /// 收集 shadow 筛查的 agree/disagree 统计，用于验证筛查效果后切换阻断模式。
    pub shadow_screen_collector: Arc<echomind_core::security::ShadowScreenCollector>,
    /// 预算追踪器（Q08 借鉴 QM BudgetTracker）。
    ///
    /// LLM API 费用控制和速率限制，防止过度消耗。
    pub budget_tracker: Arc<echomind_core::budget::BudgetTracker>,
    /// 按 key 序列化并发操作的队列（Q09 借鉴 QM createKeyedQueue）。
    ///
    /// 同 conversation_id 的 persist_exchange 操作串行执行，
    /// 消除 SQLite 并发写入竞态（消息顺序错乱、标题覆盖等）。
    /// 不同 conversation_id 的操作并行执行，不影响跨会话性能。
    pub persist_queue: echomind_core::concurrency::KeyedQueue<String>,
    /// LLM 后端路由器（Q11 借鉴 QM HarnessRouter）。
    ///
    /// 按 conversation_id 追踪上次使用的模式，支持运行时动态切换 LLM 后端
    ///（Remote / Local）。切换时通过 `RouterVerdict::ModeChanged` 通知
    /// 调用方重置会话状态（KV cache 等）。
    pub llm_router: echomind_core::llm_router::LlmRouter,
    /// 日志系统 WorkerGuard（保持非阻塞写入器后台线程存活，REQ-OBS-001）
    pub _log_guard: Option<LocalLogger>,
}

impl AppState {
    /// 初始化：打开数据库、执行崩溃恢复、初始化各 Store。
    pub async fn new(data_dir: PathBuf) -> anyhow::Result<Self> {
        let db_path = data_dir.join("echomind.db");
        let storage = tokio::task::spawn_blocking(move || SqliteStorage::new(&db_path))
            .await
            .context("存储初始化任务失败")?
            .context("初始化 SQLite 存储失败")?;

        let cleaned = storage
            .cleanup_zombies()
            .await
            .context("崩溃恢复清理失败")?;
        if cleaned > 0 {
            eprintln!("崩溃恢复：已将 {cleaned} 条中断的索引任务标记为失败");
        }

        // 健壮下载系统崩溃恢复扫描（REQ-LLM-004 v2）
        // 检测 .partial + .meta.json 文件，记录需要恢复的下载
        let models_llm_dir = data_dir.join("models").join("llm");
        if models_llm_dir.exists() {
            let recovery_dir = models_llm_dir.clone();
            let pending =
                tokio::task::spawn_blocking(move || -> Vec<echomind_models::DownloadManifest> {
                    let mut items = Vec::new();
                    let entries = match std::fs::read_dir(&recovery_dir) {
                        Ok(e) => e,
                        Err(_) => return items,
                    };
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = match path.file_name().and_then(|n| n.to_str()) {
                            Some(n) => n.to_string(),
                            None => continue,
                        };
                        if !name.starts_with('.') || !name.ends_with(".meta.json") {
                            continue;
                        }
                        if let Ok(content) = std::fs::read_to_string(&path)
                            && let Ok(manifest) =
                                serde_json::from_str::<echomind_models::DownloadManifest>(&content)
                        {
                            let partial =
                                recovery_dir.join(format!(".{}.partial", manifest.filename));
                            if partial.exists() {
                                items.push(manifest);
                            } else {
                                let _ = std::fs::remove_file(&path);
                            }
                        }
                    }
                    items
                })
                .await
                .unwrap_or_default();
            if !pending.is_empty() {
                eprintln!("崩溃恢复：检测到 {} 个未完成的模型下载", pending.len());
                for m in &pending {
                    let completed: u64 = m.parts.iter().map(|p| p.completed).sum();
                    eprintln!(
                        "  - {} ({}/{} bytes, {:.1}%)",
                        m.filename,
                        completed,
                        m.total_size,
                        if m.total_size > 0 {
                            completed as f64 / m.total_size as f64 * 100.0
                        } else {
                            0.0
                        }
                    );
                }
            }
        }

        // 初始化各 Store
        let storage_arc = Arc::new(storage);
        let security_store = SecurityStore::new(Arc::clone(&storage_arc));
        let document_store = DocumentStore::new(Arc::clone(&storage_arc));
        let chat_store = ChatStore::new(Arc::clone(&storage_arc));

        // ConfigStore 和 ModelStore 需要 own storage（从 Arc 取回）
        // 由于 SqliteStorage 是 Clone（内部 Arc 连接池），可直接 clone
        let storage_for_config = (*storage_arc).clone();
        let storage_for_models = (*storage_arc).clone();

        let config_store = ConfigStore::new(storage_for_config).await;
        let model_store = ModelStore::new(storage_for_models, data_dir.clone())
            .context("初始化模型管理器失败")?;

        // 从 Arc 取回 storage（Arc 在初始化完成后不再需要）
        // 注意：SqliteStorage 内部是 Arc<r2d2::Pool>，clone 开销极低
        let storage = (*storage_arc).clone();

        // 初始化语义缓存（REQ-PERF-001）：共享 SqliteStorage 连接池
        let cache = SqliteCache::new(storage.pool_clone()).context("初始化语义缓存失败")?;

        // 初始化步骤级缓存（P2-1 StepCache）：内存实现，默认容量 256 条
        let step_cache = Arc::new(echomind_core::step_cache::InMemoryStepCache::default());

        // 初始化 Prompt 压缩比（REQ-PERF-002）：默认 1.0 = 禁用
        let compression_ratio = storage
            .get_setting("compression.ratio")
            .await
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| *v >= 1.0 && *v <= 10.0)
            .unwrap_or(1.0);

        // 初始化 Speculative RAG 开关（REQ-PERF-011）：默认 false
        let speculative_enabled = storage
            .get_setting("rag.speculative_enabled")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true");

        // 初始化自进化检索记忆开关（REQ-PERF-012）：默认 false
        let retrieval_memory_enabled = storage
            .get_setting("rag.retrieval_memory_enabled")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true");

        // 初始化知识图谱图遍历检索开关（REQ-RAG-027）：默认 false
        let graph_retriever_enabled = storage
            .get_setting("rag.graph_retriever_enabled")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true");

        // 初始化 RAG 质量门控开关（REQ-RAG-028）：默认 false
        let quality_gate_enabled = storage
            .get_setting("rag.quality_gate_enabled")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true");

        // 初始化持久化记忆系统开关（REQ-RAG-032）：默认 false
        let memory_enabled = storage
            .get_setting("memory.enabled")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true");

        // 初始化网页搜索开关（REQ-RAG-036）：默认 false（opt-in）
        let web_search_enabled = storage
            .get_setting("rag.web_search_enabled")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true");

        // 初始化 Contextual Retrieval 开关（REQ-RAG-041）：默认 true
        // 嵌入管线已使用 build_contextual_text() 构建上下文文本（文档名前缀），
        // 默认开启以保证已有行为不变。用户可关闭以使用纯 chunk 文本嵌入。
        let contextual_retrieval_enabled = storage
            .get_setting("rag.contextual_retrieval")
            .await
            .ok()
            .flatten()
            .map(|v| v != "false") // 默认 true：仅当显式设为 "false" 时关闭
            .unwrap_or(true);

        // 初始化本地日志系统（REQ-OBS-001）
        let log_dir = data_dir.join("logs");
        let log_level = storage
            .get_setting("log.level")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "info".to_string());
        let log_guard = match LocalLogger::init(log_dir, &log_level) {
            Ok(g) => {
                tracing::info!(module = "state", "EchoMind 日志系统已启动");
                Some(g)
            }
            Err(e) => {
                // 日志初始化失败不阻塞应用启动，降级为 eprintln
                eprintln!("日志系统初始化失败（降级为 stderr）: {e}");
                None
            }
        };

        // 初始化安全态势（Q05 借鉴 QM SecurityPosture）：默认 Auto
        let security_posture_val = storage
            .get_setting("security.posture")
            .await
            .ok()
            .flatten()
            .and_then(|v| SecurityPosture::parse_str(&v))
            .unwrap_or_default();
        let security_posture = Arc::new(std::sync::atomic::AtomicU8::new(
            security_posture_val.as_u8(),
        ));

        // 初始化 LLM 后端路由器（Q11 借鉴 QM HarnessRouter）
        // 根据是否 Pro 选择可用模式集合，并根据当前 llm_mode 设置 fallback
        let is_pro_val = *config_store.is_pro.read().await;
        let llm_mode_val = *config_store.llm_mode.read().await;
        let local_model_val = storage
            .get_setting("llm.local_model")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        let llm_router_init = if is_pro_val {
            echomind_core::llm_router::LlmRouter::new_pro_default()
        } else {
            echomind_core::llm_router::LlmRouter::new_free_default()
        };
        llm_router_init
            .set_fallback(echomind_core::llm_router::LlmChoice::new(
                llm_mode_val,
                local_model_val,
            ))
            .await;

        Ok(Self {
            storage,
            data_dir,
            security_store,
            config_store,
            model_store,
            document_store,
            chat_store,
            dream_engine: DreamEngineState::new(),
            cache,
            step_cache,
            compression_ratio,
            speculative_enabled,
            retrieval_memory_enabled,
            graph_retriever_enabled,
            quality_gate_enabled,
            memory_enabled,
            web_search_enabled,
            contextual_retrieval_enabled,
            hook_registry: None,
            trace_store: Arc::new(RwLock::new(echomind_core::trace::TraceStore::new(50))),
            session_coordinator: Arc::new(
                echomind_core::session_coordinator::SessionCoordinator::new(),
            ),
            memory_burst_buffer: Arc::new(tokio::sync::Mutex::new(
                echomind_core::memory_store::BurstBuffer::new(),
            )),
            compaction_pending: Arc::new(tokio::sync::Mutex::new(HashSet::new())),
            security_posture,
            shadow_screen_collector: Arc::new(echomind_core::security::ShadowScreenCollector::new()),
            budget_tracker: Arc::new(echomind_core::budget::BudgetTracker::new(0.0)), // Default: no limit
            persist_queue: echomind_core::concurrency::KeyedQueue::new(),
            llm_router: llm_router_init,
            _log_guard: log_guard,
        })
    }

    // ---- 便捷访问方法（委托到各 Store，保持 commands.rs 向后兼容）----

    /// 安全管理器引用（向后兼容）。
    pub fn security(&self) -> &Arc<echomind_core::security::SecurityManager> {
        &self.security_store.security
    }

    /// 剪贴板清除管理器引用（向后兼容）。
    pub fn clipboard_guard(&self) -> &Arc<echomind_core::security::ClipboardGuard> {
        &self.security_store.clipboard_guard
    }

    /// LLM 配置读锁引用（向后兼容）。
    pub fn llm_config(&self) -> &tokio::sync::RwLock<Option<echomind_models::LlmConfig>> {
        &self.config_store.llm_config
    }

    /// Pro 授权状态读锁引用（向后兼容）。
    pub fn is_pro(&self) -> &tokio::sync::RwLock<bool> {
        &self.config_store.is_pro
    }

    /// 获取当前 LLM 推理模式（REQ-LLM-003）。
    pub async fn get_llm_mode(&self) -> LlmMode {
        self.config_store.get_llm_mode().await
    }

    /// 切换 LLM 推理模式并持久化（REQ-LLM-003）。
    pub async fn set_llm_mode(&self, mode: LlmMode) -> anyhow::Result<()> {
        self.config_store.set_llm_mode(mode).await
    }

    /// 获取模型文件管理器引用（REQ-LLM-004）。
    pub fn model_manager(&self) -> &echomind_infra::model_manager::ModelManager {
        self.model_store.model_manager()
    }

    /// 获取健壮下载器引用（REQ-LLM-004 v2：断点续传 + 多源容错 + 崩溃恢复）。
    pub fn robust_downloader(&self) -> &echomind_infra::robust_downloader::RobustDownloader {
        self.model_store.robust_downloader()
    }

    // ---- Embedder 委托 ----

    /// 懒加载向量化引擎（委托到 ModelStore）。
    pub async fn embedder(&self) -> anyhow::Result<LocalEmbedder> {
        self.model_store.embedder().await
    }

    /// 检查向量化引擎是否已初始化。
    pub async fn embedder_initialized(&self) -> bool {
        self.model_store.embedder_initialized().await
    }

    /// 切换嵌入模型（REQ-VEC-012）。
    pub async fn set_embedding_model(&self, model_str: &str) -> anyhow::Result<()> {
        self.model_store.set_embedding_model(model_str).await
    }

    /// 带进度回调初始化向量化引擎（REQ-VEC-008）。
    pub async fn init_embedder_with_progress(
        &self,
        progress: echomind_infra::local_embedder::DownloadProgressFn,
    ) -> anyhow::Result<()> {
        self.model_store.init_embedder_with_progress(progress).await
    }

    // ---- Reranker 委托 ----

    /// 懒加载 Cross-Encoder 重排序引擎（REQ-RAG-020）。
    pub async fn reranker(&self) -> anyhow::Result<&LocalReranker> {
        self.model_store.reranker().await
    }

    /// 检查重排序引擎是否已初始化。
    pub fn reranker_initialized(&self) -> bool {
        self.model_store.reranker_initialized()
    }

    // ---- Pro: 本地 LLM 委托 ----

    /// 懒加载本地 LLM 引擎（Pro 门控，REQ-LLM-003）。
    #[cfg(feature = "pro")]
    pub async fn local_llm(&self) -> anyhow::Result<LocalLlmEngine> {
        self.model_store.local_llm().await
    }

    /// 卸载本地 LLM 引擎。
    #[cfg(feature = "pro")]
    pub async fn unload_local_llm(&self) {
        self.model_store.unload_local_llm().await;
    }

    // ---- Pro: PDF/OCR/VLM 委托 ----

    /// 懒加载 PDF 页面渲染引擎（REQ-MM-001）。
    #[cfg(feature = "pro")]
    pub async fn page_renderer(&self) -> anyhow::Result<&PdfiumRenderer> {
        self.model_store.page_renderer().await
    }

    /// 懒加载 OCR 引擎（REQ-MM-002）。
    #[cfg(feature = "pro")]
    pub async fn ocr_engine(&self) -> anyhow::Result<&OcrsEngine> {
        self.model_store.ocr_engine().await
    }

    /// 获取 VLM 图片理解引擎（REQ-MM-003）。
    #[cfg(feature = "pro")]
    pub async fn vision_provider(&self) -> anyhow::Result<Option<OpenAIVisionProvider>> {
        let enabled = self
            .storage
            .get_setting("mm.vlm_enabled")
            .await?
            .is_some_and(|v| v == "true");
        if !enabled {
            return Ok(None);
        }
        let config = self.config_store.llm_config.read().await.clone();
        match config {
            Some(cfg) => match OpenAIVisionProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
                Ok(p) => Ok(Some(p)),
                Err(e) => {
                    eprintln!("VLM 初始化失败，将降级为纯 OCR: {e}");
                    Ok(None)
                }
            },
            None => {
                eprintln!("VLM 已启用但 LLM 配置缺失，将降级为纯 OCR");
                Ok(None)
            }
        }
    }

    // ---- 会话中断委托（ChatStore）----

    /// 获取（或创建）指定会话的中断令牌（REQ-RAG-005）。
    pub async fn abort_token_for(&self, conversation_id: &str) -> CancellationToken {
        self.chat_store.abort_token_for(conversation_id).await
    }

    /// 触发指定会话的中断。
    pub async fn abort_chat(&self, conversation_id: &str) {
        self.chat_store.abort_chat(conversation_id).await;
    }

    /// 清理指定会话的中断令牌。
    pub async fn clear_abort(&self, conversation_id: &str) {
        self.chat_store.clear_abort(conversation_id).await;
    }

    // ---- 审计取消委托（ChatStore）----

    /// 获取（或创建）指定文档的审计取消标志（REQ-AUDIT-005）。
    pub async fn audit_cancel_for(&self, doc_id: &str) -> Arc<std::sync::atomic::AtomicBool> {
        self.chat_store.audit_cancel_for(doc_id).await
    }

    /// 触发指定文档的审计取消。
    pub async fn abort_audit(&self, doc_id: &str) {
        self.chat_store.abort_audit(doc_id).await;
    }

    /// 清理指定文档的审计取消标志。
    pub async fn clear_audit_cancel(&self, doc_id: &str) {
        self.chat_store.clear_audit_cancel(doc_id).await;
    }

    // ---- 导入取消委托（DocumentStore）----

    /// 获取导入取消标志的引用。
    pub fn import_cancel_flag(&self) -> Arc<std::sync::atomic::AtomicBool> {
        self.document_store.import_cancel_flag()
    }

    /// 触发导入取消。
    pub fn abort_import(&self) {
        self.document_store.abort_import();
    }

    /// 重置导入取消标志。
    pub fn reset_import_cancel(&self) {
        self.document_store.reset_import_cancel();
    }

    // ---- 文件监听委托（DocumentStore）----

    /// 注册文件监听器句柄（REQ-SYNC-003）。
    pub async fn register_file_watcher(&self, path: &str, handle: FileWatcherHandle) {
        self.document_store
            .register_file_watcher(path, handle)
            .await;
    }

    /// 注销文件监听器。
    pub async fn unregister_file_watcher(&self, path: &str) {
        self.document_store.unregister_file_watcher(path).await;
    }

    /// 检查指定路径的监听器是否活跃。
    pub async fn is_watcher_active(&self, path: &str) -> bool {
        self.document_store.is_watcher_active(path).await
    }

    // ---- KV cache 目录管理（Phase 4 Session 25）----

    /// 获取 KV cache 文件存储目录（REQ-LLM-009）。
    ///
    /// 返回 `{data_dir}/kv_cache/` 路径。目录可能尚不存在，
    /// 调用方负责按需创建。
    pub fn kv_cache_dir(&self) -> PathBuf {
        self.data_dir.join("kv_cache")
    }

    /// 获取日志文件存储目录（REQ-OBS-001）。
    ///
    /// 返回 `{data_dir}/logs/` 路径。目录可能尚不存在。
    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// 获取自定义嵌入模型存储目录（REQ-VEC-014，Pro 门控）。
    ///
    /// 返回 `{data_dir}/custom_models/` 路径。目录可能尚不存在，
    /// 调用方负责按需创建。每个自定义模型存储在子目录 `{name}/` 中。
    pub fn custom_model_dir(&self) -> PathBuf {
        self.data_dir.join("custom_models")
    }

    // ---- 安全态势管理（Q05 借鉴 QM SecurityPosture）----

    /// 获取当前安全态势（无锁读，AtomicU8 → SecurityPosture）。
    pub fn get_security_posture(&self) -> SecurityPosture {
        SecurityPosture::from_u8(
            self.security_posture
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }

    /// 设置安全态势（无锁写，SecurityPosture → AtomicU8）。
    pub fn set_security_posture_value(&self, posture: SecurityPosture) {
        self.security_posture
            .store(posture.as_u8(), std::sync::atomic::Ordering::Relaxed);
    }
}

// ============================================================================
// DreamEngineState — 后台空闲整理引擎状态
// ============================================================================

/// Dream Engine 运行态：存储最新分析结果与运行标志。
///
/// `trigger_dream` IPC 命令在后台启动分析，结果写入 `results`。
/// `get_dream_suggestions` IPC 命令从 `results` 读取缓存结果返回前端。
/// `running` 标志防止并发执行；`cancel` 支持用户中断分析。
pub struct DreamEngineState {
    /// 最新 Dream 分析结果（`None` 表示尚未运行过）
    results: tokio::sync::RwLock<Option<DreamResult>>,
    /// 是否正在运行分析（防止并发执行）
    running: Arc<AtomicBool>,
    /// 取消信号
    cancel: Arc<AtomicBool>,
}

impl DreamEngineState {
    /// 创建新的 Dream Engine 状态（初始为空、非运行）。
    pub fn new() -> Self {
        Self {
            results: tokio::sync::RwLock::new(None),
            running: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 获取取消信号（用于 DreamEngine::dream()）。
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// 标记为正在运行。返回 `false` 表示已有分析在进行中。
    pub fn try_start(&self) -> bool {
        !self
            .running
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_err()
    }

    /// 标记为已完成（清除 running 标志）。
    pub fn finish(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// 是否正在运行。
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// 触发取消（设置 cancel 标志）。
    pub fn cancel(&self) {
        self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// 重置取消标志（下次运行前调用）。
    pub fn reset_cancel(&self) {
        self.cancel
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// 存储分析结果。
    pub async fn set_results(&self, result: DreamResult) {
        *self.results.write().await = Some(result);
    }

    /// 读取缓存的分析结果（`None` 表示尚未运行过）。
    pub async fn get_results(&self) -> Option<DreamResult> {
        self.results.read().await.clone()
    }
}

impl Default for DreamEngineState {
    fn default() -> Self {
        Self::new()
    }
}
