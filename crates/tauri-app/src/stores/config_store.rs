//! 配置管理 Store — LLM 配置 + Pro 授权 + LLM 模式。
//!
//! 借鉴 Zed `settings`/`feature_flags` 的 Store 模式：
//! 运行时配置状态封装在独立 Store 中，支持从 settings 表恢复。

use echomind_core::Storage;
use echomind_infra::sqlite_storage::SqliteStorage;
use echomind_models::{LlmConfig, LlmMode};

/// 配置管理 Store（BYOK 配置 + Pro 授权 + LLM 模式）。
///
/// # 职责
/// - BYOK LLM 运行态配置（启动时自 settings 表恢复；update_llm_config 后即时生效）
/// - Pro 授权状态（REQ-LIC-001/002；启动时自 settings 表恢复，activate_pro 后即时生效）
/// - LLM 推理模式（REQ-LLM-003；Remote=BYOK API，Local=mistral.rs 本地推理）
///
/// # 线程安全
/// 使用 `tokio::sync::RwLock` 保护可变运行时状态。
pub struct ConfigStore {
    /// 共享存储引用
    storage: SqliteStorage,
    /// BYOK LLM 运行态配置（启动时自 settings 表恢复；update_llm_config 后即时生效）
    pub llm_config: tokio::sync::RwLock<Option<LlmConfig>>,
    /// Pro 授权状态（REQ-LIC-001/002）
    pub is_pro: tokio::sync::RwLock<bool>,
    /// LLM 推理模式（REQ-LLM-003；Remote=BYOK API，Local=mistral.rs 本地推理）
    pub llm_mode: tokio::sync::RwLock<LlmMode>,
}

impl ConfigStore {
    /// 创建新的配置管理 Store。
    ///
    /// 从 settings 表恢复 BYOK 配置、Pro 授权状态和 LLM 模式。
    pub async fn new(storage: SqliteStorage) -> Self {
        let llm_config = Self::load_llm_config(&storage).await;
        let is_pro = Self::load_is_pro(&storage).await;
        let llm_mode = Self::load_llm_mode(&storage).await;

        Self {
            storage,
            llm_config: tokio::sync::RwLock::new(llm_config),
            is_pro: tokio::sync::RwLock::new(is_pro),
            llm_mode: tokio::sync::RwLock::new(llm_mode),
        }
    }

    /// 获取存储引用（非 Arc 版本，供 AppState 直接使用）。
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }

    /// 从 settings 表恢复 BYOK 配置（REQ-UI-008）。
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

    /// 从 settings 表恢复 Pro 授权状态。
    async fn load_is_pro(storage: &SqliteStorage) -> bool {
        storage
            .get_setting("license.is_pro")
            .await
            .ok()
            .flatten()
            .is_some_and(|v| v == "true")
    }

    /// 从 settings 表恢复 LLM 推理模式。
    async fn load_llm_mode(storage: &SqliteStorage) -> LlmMode {
        match storage.get_setting("llm.mode").await {
            Ok(Some(s)) if s == "local" => LlmMode::Local,
            _ => LlmMode::Remote,
        }
    }

    /// 获取当前 LLM 推理模式（REQ-LLM-003）。
    pub async fn get_llm_mode(&self) -> LlmMode {
        *self.llm_mode.read().await
    }

    /// 切换 LLM 推理模式并持久化到 settings 表（REQ-LLM-003）。
    pub async fn set_llm_mode(&self, mode: LlmMode) -> anyhow::Result<()> {
        let mode_str = match mode {
            LlmMode::Remote => "remote",
            LlmMode::Local => "local",
        };
        self.storage.set_setting("llm.mode", mode_str).await?;
        *self.llm_mode.write().await = mode;
        Ok(())
    }

    /// 从存储创建（仅用于测试 mock）。
    #[cfg(test)]
    pub fn new_mock(
        storage: SqliteStorage,
        llm_config: Option<LlmConfig>,
        is_pro: bool,
        llm_mode: LlmMode,
    ) -> Self {
        Self {
            llm_config: tokio::sync::RwLock::new(llm_config),
            is_pro: tokio::sync::RwLock::new(is_pro),
            llm_mode: tokio::sync::RwLock::new(llm_mode),
            storage,
        }
    }
}
