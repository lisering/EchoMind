//! 类型安全 Settings 注册表（借鉴 Zed `settings_macros`/`RegisterSetting` 模式）。
//!
//! 将所有 settings key 集中定义，提供类型安全访问，消除字符串拼写错误风险。
//!
//! # 设计
//! - `SettingKey` 枚举 — 所有合法 settings key 的类型安全表示
//! - `SettingDefinition` — key + 类型 + 默认值 + 描述
//! - `SettingsRegistry` — 注册表 trait，提供 get/set/ schema 生成
//!
//! # 借鉴 Zed
//! Zed 使用 `#[derive(RegisterSetting)]` 宏自动注册设置项。
//! EchoMind 不使用宏（避免 proc-macro crate 复杂度），改为手动注册 + 枚举保证类型安全。

use serde::{Deserialize, Serialize};
use serde_json::json;

/// 设置值类型。
#[derive(Debug, Clone, PartialEq)]
pub enum SettingType {
    /// 字符串值
    String(String),
    /// 布尔值
    Bool(bool),
    /// 整数值
    Int(i64),
    /// JSON 对象（如采样参数）
    Json(serde_json::Value),
}

impl SettingType {
    /// 序列化为存储字符串（settings 表存储为 TEXT）。
    pub fn to_storage_string(&self) -> String {
        match self {
            SettingType::String(s) => s.clone(),
            SettingType::Bool(b) => b.to_string(),
            SettingType::Int(i) => i.to_string(),
            SettingType::Json(v) => v.to_string(),
        }
    }

    /// 从存储字符串解析为指定类型。
    pub fn from_storage_string(s: &str, kind: SettingKind) -> Self {
        match kind {
            SettingKind::String => SettingType::String(s.to_string()),
            SettingKind::Bool => SettingType::Bool(s == "true"),
            SettingKind::Int => SettingType::Int(s.parse().unwrap_or(0)),
            SettingKind::Json => {
                if let Ok(v) = serde_json::from_str(s) {
                    SettingType::Json(v)
                } else {
                    SettingType::String(s.to_string())
                }
            }
        }
    }
}

/// 设置类型种类（用于 schema 生成和解析）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingKind {
    String,
    Bool,
    Int,
    Json,
}

/// 设置项定义（key + 类型 + 默认值 + 描述）。
#[derive(Debug, Clone)]
pub struct SettingDefinition {
    /// 设置键名（如 "llm.mode"）
    pub key: &'static str,
    /// 值类型
    pub kind: SettingKind,
    /// 默认值
    pub default: &'static str,
    /// 人类可读描述
    pub description: &'static str,
    /// 设置分组（如 "llm" / "rag" / "vec" / "security"）
    pub group: &'static str,
}

/// 所有合法 settings key 的类型安全枚举。
///
/// 使用枚举而非字符串，编译期保证 key 合法性。
/// 新增 key 时只需在枚举中添加一个变体 + 在 `definitions()` 中注册。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SettingKey {
    // LLM 配置
    LlmApiKey,
    LlmBaseUrl,
    LlmModel,
    LlmMode,
    LlmLocalModel,
    LlmPagedAttn,
    LlmBlockSize,
    LlmGpuMemoryCtx,
    LlmSampling,
    // License
    LicenseIsPro,
    // 向量化
    VecEmbeddingModel,
    // RAG
    RagHybridSearch,
    RagRerankEnabled,
    RagHydeEnabled,
    RagAgentEnabled,
    RagContextTokenLimit,
    // 多模态
    MmVlmEnabled,
    // 安全
    SecurityAutoLockTimeout,
    SecurityPiiDetection,
    SecurityClipboardClearTimeout,
    // UI
    UiLocale,
}

impl SettingKey {
    /// 转换为存储键名字符串。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LlmApiKey => "llm.api_key",
            Self::LlmBaseUrl => "llm.base_url",
            Self::LlmModel => "llm.model",
            Self::LlmMode => "llm.mode",
            Self::LlmLocalModel => "llm.local_model",
            Self::LlmPagedAttn => "llm.paged_attn",
            Self::LlmBlockSize => "llm.block_size",
            Self::LlmGpuMemoryCtx => "llm.gpu_memory_ctx",
            Self::LlmSampling => "llm.sampling",
            Self::LicenseIsPro => "license.is_pro",
            Self::VecEmbeddingModel => "vec.embedding_model",
            Self::RagHybridSearch => "rag.hybrid_search",
            Self::RagRerankEnabled => "rag.rerank_enabled",
            Self::RagHydeEnabled => "rag.hyde_enabled",
            Self::RagAgentEnabled => "rag.agent_enabled",
            Self::RagContextTokenLimit => "rag.context_token_limit",
            Self::MmVlmEnabled => "mm.vlm_enabled",
            Self::SecurityAutoLockTimeout => "security.auto_lock_timeout",
            Self::SecurityPiiDetection => "security.pii_detection",
            Self::SecurityClipboardClearTimeout => "security.clipboard_clear_timeout",
            Self::UiLocale => "ui.locale",
        }
    }

    /// 从字符串解析为 SettingKey（找不到返回 None）。
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "llm.api_key" => Some(Self::LlmApiKey),
            "llm.base_url" => Some(Self::LlmBaseUrl),
            "llm.model" => Some(Self::LlmModel),
            "llm.mode" => Some(Self::LlmMode),
            "llm.local_model" => Some(Self::LlmLocalModel),
            "llm.paged_attn" => Some(Self::LlmPagedAttn),
            "llm.block_size" => Some(Self::LlmBlockSize),
            "llm.gpu_memory_ctx" => Some(Self::LlmGpuMemoryCtx),
            "llm.sampling" => Some(Self::LlmSampling),
            "license.is_pro" => Some(Self::LicenseIsPro),
            "vec.embedding_model" => Some(Self::VecEmbeddingModel),
            "rag.hybrid_search" => Some(Self::RagHybridSearch),
            "rag.rerank_enabled" => Some(Self::RagRerankEnabled),
            "rag.hyde_enabled" => Some(Self::RagHydeEnabled),
            "rag.agent_enabled" => Some(Self::RagAgentEnabled),
            "rag.context_token_limit" => Some(Self::RagContextTokenLimit),
            "mm.vlm_enabled" => Some(Self::MmVlmEnabled),
            "security.auto_lock_timeout" => Some(Self::SecurityAutoLockTimeout),
            "security.pii_detection" => Some(Self::SecurityPiiDetection),
            "security.clipboard_clear_timeout" => Some(Self::SecurityClipboardClearTimeout),
            "ui.locale" => Some(Self::UiLocale),
            _ => None,
        }
    }

    /// 获取所有已注册设置项的定义。
    pub fn definitions() -> &'static [SettingDefinition] {
        DEFINITIONS
    }

    /// 获取此 key 的定义。
    pub fn definition(&self) -> &'static SettingDefinition {
        DEFINITIONS
            .iter()
            .find(|d| d.key == self.as_str())
            .unwrap_or_else(|| {
                const FALLBACK: SettingDefinition = SettingDefinition {
                    key: "",
                    kind: SettingKind::String,
                    default: "",
                    description: "",
                    group: "",
                };
                &FALLBACK
            })
    }

    /// 生成 JSON Schema（供前端动态渲染设置 UI）。
    pub fn schema_json() -> serde_json::Value {
        let items: Vec<serde_json::Value> = Self::definitions()
            .iter()
            .map(|d| {
                json!({
                    "key": d.key,
                    "type": format!("{:?}", d.kind).to_lowercase(),
                    "default": d.default,
                    "description": d.description,
                    "group": d.group,
                })
            })
            .collect();
        json!({ "settings": items })
    }
}

/// 所有设置项的静态定义表。
///
/// 新增设置时在此添加对应定义。
/// `SettingKey::from_str()` 和 `definitions()` 引用此表保证一致性。
static DEFINITIONS: &[SettingDefinition] = &[
    SettingDefinition {
        key: "llm.api_key",
        kind: SettingKind::String,
        default: "",
        description: "LLM API 密钥（BYOK 模式）",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.base_url",
        kind: SettingKind::String,
        default: "",
        description: "LLM API 基础 URL",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.model",
        kind: SettingKind::String,
        default: "",
        description: "LLM 模型名称",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.mode",
        kind: SettingKind::String,
        default: "remote",
        description: "LLM 推理模式（remote=BYOK API, local=本地推理）",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.local_model",
        kind: SettingKind::String,
        default: "",
        description: "本地 LLM 模型文件名",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.paged_attn",
        kind: SettingKind::Bool,
        default: "false",
        description: "是否启用 PagedAttention（仅 GPU 模式）",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.block_size",
        kind: SettingKind::Int,
        default: "32",
        description: "PagedAttention 块大小",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.gpu_memory_ctx",
        kind: SettingKind::Int,
        default: "4096",
        description: "GPU 内存上下文大小",
        group: "llm",
    },
    SettingDefinition {
        key: "llm.sampling",
        kind: SettingKind::Json,
        default: "{}",
        description: "采样参数 JSON（temperature/top_p/top_k/max_tokens/penalty）",
        group: "llm",
    },
    SettingDefinition {
        key: "license.is_pro",
        kind: SettingKind::Bool,
        default: "false",
        description: "Pro 授权状态",
        group: "license",
    },
    SettingDefinition {
        key: "vec.embedding_model",
        kind: SettingKind::String,
        default: "all-MiniLM-L6-v2",
        description: "嵌入模型标识（all-MiniLM-L6-v2 / bge-small-zh-v1.5 / e5-small-v2 / bge-base-en-v1.5；切换后需重建全部 embeddings）",
        group: "vec",
    },
    SettingDefinition {
        key: "rag.hybrid_search",
        kind: SettingKind::Bool,
        default: "true",
        description: "是否启用混合检索（向量+关键词 RRF 融合）",
        group: "rag",
    },
    SettingDefinition {
        key: "rag.rerank_enabled",
        kind: SettingKind::Bool,
        default: "true",
        description: "是否启用 Cross-Encoder 层次化重排序（过检索 5× 候选 → 精排）",
        group: "rag",
    },
    SettingDefinition {
        key: "rag.hyde_enabled",
        kind: SettingKind::Bool,
        default: "false",
        description: "是否启用 HyDE 查询改写",
        group: "rag",
    },
    SettingDefinition {
        key: "rag.agent_enabled",
        kind: SettingKind::Bool,
        default: "false",
        description: "是否启用 Agentic RAG 多步推理",
        group: "rag",
    },
    SettingDefinition {
        key: "rag.context_token_limit",
        kind: SettingKind::Int,
        default: "4096",
        description: "RAG 上下文 token 上限",
        group: "rag",
    },
    SettingDefinition {
        key: "mm.vlm_enabled",
        kind: SettingKind::Bool,
        default: "false",
        description: "是否启用 VLM 图片理解增强",
        group: "mm",
    },
    SettingDefinition {
        key: "security.auto_lock_timeout",
        kind: SettingKind::Int,
        default: "0",
        description: "自动锁屏超时（秒，0=禁用）",
        group: "security",
    },
    SettingDefinition {
        key: "security.pii_detection",
        kind: SettingKind::Bool,
        default: "false",
        description: "是否启用 PII 检测",
        group: "security",
    },
    SettingDefinition {
        key: "security.clipboard_clear_timeout",
        kind: SettingKind::Int,
        default: "30",
        description: "剪贴板自动清除超时（秒）",
        group: "security",
    },
    SettingDefinition {
        key: "ui.locale",
        kind: SettingKind::String,
        default: "zh-CN",
        description: "界面语言",
        group: "ui",
    },
];

/// 类型安全 settings 读写 trait。
///
/// 借鉴 Zed `SettingsStore` — 所有 settings 读写通过此 trait，
/// 调用方使用 `SettingKey` 枚举而非字符串，编译期保证 key 合法。
pub trait SettingsRegistry {
    /// 类型安全读取设置项。
    fn get_setting_typed(
        &self,
        key: SettingKey,
    ) -> impl std::future::Future<Output = anyhow::Result<Option<String>>>;

    /// 类型安全写入设置项。
    fn set_setting_typed(
        &self,
        key: SettingKey,
        value: &str,
    ) -> impl std::future::Future<Output = anyhow::Result<()>>;

    /// 获取设置项 schema JSON（供前端动态渲染设置 UI）。
    fn settings_schema() -> serde_json::Value {
        SettingKey::schema_json()
    }
}

/// 为 AppState 实现 SettingsRegistry。
///
/// 通过 `SettingKey` 枚举保证编译期类型安全，消除字符串拼写错误。
impl SettingsRegistry for crate::state::AppState {
    async fn get_setting_typed(&self, key: SettingKey) -> anyhow::Result<Option<String>> {
        // 委托到现有 storage.get_setting
        use echomind_core::Storage;
        self.storage.get_setting(key.as_str()).await
    }

    async fn set_setting_typed(&self, key: SettingKey, value: &str) -> anyhow::Result<()> {
        use echomind_core::Storage;
        self.storage.set_setting(key.as_str(), value).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_key_roundtrip() {
        // 确保所有 key 都能 from_str → as_str 往返
        for def in SettingKey::definitions() {
            let key = SettingKey::from_str(def.key);
            assert!(key.is_some(), "from_str 失败: {}", def.key);
            assert_eq!(key.map(|k| k.as_str()), Some(def.key));
        }
    }

    #[test]
    fn test_setting_key_unknown_returns_none() {
        assert!(SettingKey::from_str("nonexistent.key").is_none());
        assert!(SettingKey::from_str("").is_none());
    }

    #[test]
    fn test_schema_json() {
        let schema = SettingKey::schema_json();
        let items = schema["settings"].as_array();
        assert!(items.is_some(), "schema settings should be array");
        let items = items.cloned().unwrap_or_default();
        assert!(!items.is_empty(), "schema should have settings");
        // 验证每个 item 都有必需字段
        for item in &items {
            assert!(item["key"].is_string());
            assert!(item["type"].is_string());
            assert!(item["default"].is_string());
            assert!(item["description"].is_string());
            assert!(item["group"].is_string());
        }
    }

    #[test]
    fn test_setting_type_to_storage_string() {
        assert_eq!(
            SettingType::String("hello".into()).to_storage_string(),
            "hello"
        );
        assert_eq!(SettingType::Bool(true).to_storage_string(), "true");
        assert_eq!(SettingType::Bool(false).to_storage_string(), "false");
        assert_eq!(SettingType::Int(42).to_storage_string(), "42");
    }

    #[test]
    fn test_setting_type_from_storage_string() {
        assert_eq!(
            SettingType::from_storage_string("hello", SettingKind::String),
            SettingType::String("hello".into())
        );
        assert_eq!(
            SettingType::from_storage_string("true", SettingKind::Bool),
            SettingType::Bool(true)
        );
        assert_eq!(
            SettingType::from_storage_string("42", SettingKind::Int),
            SettingType::Int(42)
        );
    }

    #[test]
    fn test_all_keys_have_definitions() {
        // 确保枚举变体数 == 定义数
        let enum_count = SettingKey::definitions().len();
        // 手动列举所有变体验证数量匹配
        let keys = [
            SettingKey::LlmApiKey,
            SettingKey::LlmBaseUrl,
            SettingKey::LlmModel,
            SettingKey::LlmMode,
            SettingKey::LlmLocalModel,
            SettingKey::LlmPagedAttn,
            SettingKey::LlmBlockSize,
            SettingKey::LlmGpuMemoryCtx,
            SettingKey::LlmSampling,
            SettingKey::LicenseIsPro,
            SettingKey::VecEmbeddingModel,
            SettingKey::RagHybridSearch,
            SettingKey::RagRerankEnabled,
            SettingKey::RagHydeEnabled,
            SettingKey::RagAgentEnabled,
            SettingKey::RagContextTokenLimit,
            SettingKey::MmVlmEnabled,
            SettingKey::SecurityAutoLockTimeout,
            SettingKey::SecurityPiiDetection,
            SettingKey::SecurityClipboardClearTimeout,
            SettingKey::UiLocale,
        ];
        assert_eq!(keys.len(), enum_count);
        for key in &keys {
            // 每个枚举变体都能找到对应定义
            let def = key.definition();
            assert_eq!(def.key, key.as_str());
        }
    }
}
