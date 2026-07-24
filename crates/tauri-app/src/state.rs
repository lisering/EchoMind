//! 应用状态组装（Phase 6：会话级流中断令牌，REQ-RAG-005）。
//! 存储为 crates/infra 的 SQLite 适配器（REQ-DB-001）。

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use echomind_core::Storage;
use echomind_infra::local_embedder::LocalEmbedder;
use echomind_infra::sqlite_storage::SqliteStorage;
use echomind_models::LlmConfig;
use tokio_util::sync::CancellationToken;

/// Tauri 全局状态。
pub struct AppState {
    /// 存储端口实现（SQLite 持久化，WAL 模式）
    pub storage: SqliteStorage,
    /// 应用数据目录（文档副本、数据库、模型缓存与密钥文件根目录）
    pub data_dir: PathBuf,
    /// BYOK LLM 运行态配置（启动时自 settings 表恢复；update_llm_config 后即时生效）
    pub llm_config: tokio::sync::RwLock<Option<LlmConfig>>,
    /// Pro 授权状态（REQ-LIC-001/002；启动时自 settings 表恢复，activate_pro 后即时生效）
    pub is_pro: tokio::sync::RwLock<bool>,
    /// 向量化引擎（首次使用时初始化：模型下载可能耗时，严禁阻塞启动）
    embedder: tokio::sync::OnceCell<LocalEmbedder>,
    /// 每个会话的流中断令牌（REQ-RAG-005；会话结束清理）
    abort_tokens: tokio::sync::Mutex<HashMap<String, CancellationToken>>,
}

impl AppState {
    /// 初始化：打开数据库、执行崩溃恢复、自 settings 表恢复 BYOK 配置。
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

        let llm_config = Self::load_llm_config(&storage).await;
        let is_pro = Self::load_is_pro(&storage).await;

        Ok(Self {
            storage,
            data_dir,
            llm_config: tokio::sync::RwLock::new(llm_config),
            is_pro: tokio::sync::RwLock::new(is_pro),
            embedder: tokio::sync::OnceCell::new(),
            abort_tokens: tokio::sync::Mutex::new(HashMap::new()),
        })
    }

    /// 从 settings 表恢复 BYOK 配置（REQ-UI-008；任一项缺失视为未配置，触发配置向导）。
    async fn load_llm_config(storage: &SqliteStorage) -> Option<LlmConfig> {
        let api_key = storage.get_setting("llm.api_key").await.ok()??;
        let base_url = storage.get_setting("llm.base_url").await.ok()??;
        let model = storage.get_setting("llm.model").await.ok()??;
        Some(LlmConfig {
            api_key,
            base_url,
            model,
        })
    }

    /// 从 settings 表恢复 Pro 授权状态（REQ-LIC-001；值为 "true" 时视为已激活）。
    async fn load_is_pro(storage: &SqliteStorage) -> bool {
        storage
            .get_setting("license.is_pro")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true")
    }

    /// 懒加载向量化引擎（首次调用时初始化，模型下载在阻塞线程池执行）。
    pub async fn embedder(&self) -> anyhow::Result<&LocalEmbedder> {
        self.embedder
            .get_or_try_init(|| async { LocalEmbedder::new(self.data_dir.join("models")).await })
            .await
    }

    /// 获取（或创建）指定会话的中断令牌（REQ-RAG-005）。
    pub async fn abort_token_for(&self, conversation_id: &str) -> CancellationToken {
        self.abort_tokens
            .lock()
            .await
            .entry(conversation_id.to_string())
            .or_default()
            .clone()
    }

    /// 触发指定会话的中断（abort_chat 命令行为）。
    pub async fn abort_chat(&self, conversation_id: &str) {
        let token = self.abort_tokens.lock().await.get(conversation_id).cloned();
        if let Some(token) = token {
            token.cancel();
        }
    }

    /// 清理指定会话的中断令牌（对话结束或会话删除后调用）。
    pub async fn clear_abort(&self, conversation_id: &str) {
        self.abort_tokens.lock().await.remove(conversation_id);
    }
}
