//! 应用状态组装（Phase 4：LLM 配置加载与 Embedder 懒加载，REQ-RAG-001 / REQ-LLM-001）。
//! 存储为 crates/infra 的 SQLite 适配器（REQ-DB-001）。

use std::path::{Path, PathBuf};

use anyhow::Context;
use echomind_core::Storage;
use echomind_infra::local_embedder::LocalEmbedder;
use echomind_infra::sqlite_storage::SqliteStorage;
use echomind_models::LlmConfig;

/// Tauri 全局状态。
pub struct AppState {
    /// 存储端口实现（SQLite 持久化，WAL 模式）
    pub storage: SqliteStorage,
    /// 应用数据目录（文档副本、数据库与模型缓存根目录）
    pub data_dir: PathBuf,
    /// BYOK LLM 配置（`data_dir/llm_config.json`；未配置时 chat 返回引导错误）
    pub llm_config: Option<LlmConfig>,
    /// 向量化引擎（首次使用时初始化：模型下载可能耗时，严禁阻塞启动）
    embedder: tokio::sync::OnceCell<LocalEmbedder>,
}

impl AppState {
    /// 初始化：打开数据库、执行崩溃恢复、加载 BYOK 配置。
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

        let llm_config = Self::load_llm_config(&data_dir);

        Ok(Self {
            storage,
            data_dir,
            llm_config,
            embedder: tokio::sync::OnceCell::new(),
        })
    }

    /// 读取 BYOK 配置（REQ-RAG-007 前置；设置界面交付前经 llm_config.json 手配）。
    fn load_llm_config(data_dir: &Path) -> Option<LlmConfig> {
        let path = data_dir.join("llm_config.json");
        let raw = std::fs::read_to_string(&path).ok()?;
        match serde_json::from_str::<LlmConfig>(&raw) {
            Ok(config) => Some(config),
            Err(err) => {
                eprintln!("llm_config.json 解析失败，按未配置处理: {err}");
                None
            }
        }
    }

    /// 懒加载向量化引擎（首次调用时初始化，模型下载在阻塞线程池执行）。
    pub async fn embedder(&self) -> anyhow::Result<&LocalEmbedder> {
        self.embedder
            .get_or_try_init(|| async { LocalEmbedder::new(self.data_dir.join("models")).await })
            .await
    }
}
