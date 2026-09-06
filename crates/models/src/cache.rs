//! 缓存领域数据契约。
//!
//! 对应 SRS：REQ-PERF-001 语义缓存金字塔。

use serde::{Deserialize, Serialize};

/// 缓存级别（REQ-PERF-001 语义缓存金字塔）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheLevel {
    Exact,
    Semantic,
    Retrieval,
}

/// 缓存命中结果（REQ-PERF-001）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHit {
    pub level: CacheLevel,
    pub answer_text: Option<String>,
    pub sources_json: Option<String>,
    pub retrieval_results_json: Option<String>,
}

/// 缓存统计信息（REQ-PERF-001）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub enabled: bool,
    pub exact_hits: u32,
    pub semantic_hits: u32,
    pub retrieval_hits: u32,
    pub total_queries: u32,
    pub cache_size_entries: u32,
    pub estimated_token_saved: u64,
}

/// 缓存设置载荷（REQ-PERF-001）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettingsPayload {
    pub enabled: bool,
    pub ttl_secs: u64,
    pub semantic_threshold: f32,
    pub privacy_mode: bool,
}

impl Default for CacheSettingsPayload {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl_secs: 86400,
            semantic_threshold: 0.92,
            privacy_mode: false,
        }
    }
}
