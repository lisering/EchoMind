//! LLM/设置领域数据契约。
//!
//! 对应 SRS：REQ-UI-008（SettingsPayload）、REQ-LLM-003（LlmSamplingParams）、
//! REQ-RAG-014/015（RagParams/GenerationParams）。

use serde::{Deserialize, Serialize};

/// BYOK LLM 配置（REQ-UI-008：经 SQLite settings 表 AES-256-GCM 加密持久化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

/// RAG 检索参数（REQ-RAG-014）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RagParams {
    pub top_k: usize,
    pub score_threshold: f32,
    pub chunk_expansion_enabled: bool,
    pub chunk_expansion_window: usize,
}

impl Default for RagParams {
    fn default() -> Self {
        Self {
            top_k: 8,
            score_threshold: 0.0,
            chunk_expansion_enabled: true,
            chunk_expansion_window: 1,
        }
    }
}

impl RagParams {
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            top_k: self.top_k.clamp(1, 20),
            score_threshold: self.score_threshold.clamp(0.0, 1.0),
            chunk_expansion_enabled: self.chunk_expansion_enabled,
            chunk_expansion_window: self.chunk_expansion_window.clamp(0, 3),
        }
    }
}

/// LLM 生成参数（REQ-RAG-015）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationParams {
    pub temperature: f32,
    pub max_tokens: usize,
    pub top_p: f32,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            max_tokens: 4096,
            top_p: 1.0,
        }
    }
}

impl GenerationParams {
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            temperature: self.temperature.clamp(0.0, 2.0),
            max_tokens: self.max_tokens.clamp(256, 8192),
            top_p: self.top_p.clamp(0.0, 1.0),
        }
    }
}

/// get_settings 返回载荷（REQ-UI-008）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPayload {
    pub has_llm_config: bool,
    pub base_url: String,
    pub model: String,
    pub api_key_masked: String,
    pub vlm_enabled: bool,
    pub hybrid_search: bool,
    pub rerank_enabled: bool,
    pub hyde_enabled: bool,
    pub agent_enabled: bool,
    pub embedding_model: String,
    #[serde(default = "crate::default_context_token_limit")]
    pub context_token_limit: usize,
    #[serde(default)]
    pub llm_mode: String,
    #[serde(default)]
    pub local_model: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub llm_sampling: Option<LlmSamplingParams>,
    #[serde(default)]
    pub token_budget: u64,
    #[serde(default = "crate::default_cache_enabled")]
    pub cache_enabled: bool,
    #[serde(default = "crate::default_cache_ttl")]
    pub cache_ttl_secs: u64,
    #[serde(default = "crate::default_cache_semantic_threshold")]
    pub cache_semantic_threshold: f32,
    #[serde(default)]
    pub cache_privacy_mode: bool,
    #[serde(default)]
    pub quality_gate_enabled: bool,
    #[serde(default)]
    pub sub_agent_enabled: bool,
    #[serde(default)]
    pub progressive_injection: bool,
    #[serde(default)]
    pub speculative_enabled: bool,
    #[serde(default)]
    pub graph_retriever_enabled: bool,
    #[serde(default = "crate::default_true")]
    pub contextual_retrieval: bool,
}

/// LLM 推理模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LlmMode {
    #[default]
    Remote,
}

/// 本地模型元信息（REQ-LLM-004）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
    pub architecture: String,
    pub param_size: String,
    pub quantization: String,
}

/// 模型下载进度事件载荷（REQ-LLM-004 AC-2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgressPayload {
    pub filename: String,
    pub downloaded: u64,
    pub total: u64,
    pub speed: u64,
}

/// 模型加载状态事件载荷（REQ-LLM-003 AC-4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadStatusPayload {
    pub model: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

/// LLM 采样参数（REQ-LLM-003 扩展）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LlmSamplingParams {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub presence_penalty: Option<f32>,
}

/// Token 用量统计（对话 token 消耗追踪）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 对话成本统计（累计 token 用量追踪）。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConversationCost {
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_tokens: u64,
    pub exchange_count: u32,
    pub token_budget: u64,
}

/// KV cache 持久化状态（REQ-LLM-009）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvCacheStatus {
    pub enabled: bool,
    pub cache_dir: String,
    pub file_count: usize,
    pub total_size_bytes: u64,
}

/// 预算使用统计（QM 借鉴）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStats {
    pub daily_limit: f64,
    pub spent_today: f64,
    pub remaining: f64,
}

/// 速率限制统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStats {
    pub max_per_window: u32,
    pub current_count: u32,
    pub window_seconds: u64,
}

/// 分页查询结果（列表懒加载，REQ-RAG-007/REQ-UI-009）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: usize,
}
