//! 嵌入模型对比评估数据契约。
//!
//! 对应 SRS：REQ-VEC-018。

use serde::{Deserialize, Serialize};

use crate::rag_eval::{RagEvalMetric, RagMetricType};

/// 嵌入模型对比评估请求（REQ-VEC-018）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedComparisonRequest {
    pub model_names: Vec<String>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    pub dataset_json: Option<String>,
}

fn default_top_k() -> usize {
    5
}

/// 嵌入模型评估指标分数（REQ-VEC-018）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedMetricScores {
    pub hit_rate: f32,
    pub mrr: f32,
    pub ndcg: f32,
}

impl EmbedMetricScores {
    pub fn zero() -> Self {
        Self {
            hit_rate: 0.0,
            mrr: 0.0,
            ndcg: 0.0,
        }
    }

    pub fn from_metrics_avg(metrics: &[Vec<RagEvalMetric>]) -> Self {
        if metrics.is_empty() {
            return Self::zero();
        }

        let mut hit_sum = 0.0f32;
        let mut mrr_sum = 0.0f32;
        let mut ndcg_sum = 0.0f32;
        let mut hit_count = 0usize;
        let mut mrr_count = 0usize;
        let mut ndcg_count = 0usize;

        for sample_metrics in metrics {
            for m in sample_metrics {
                match m.metric_type {
                    RagMetricType::HitRate => {
                        hit_sum += m.score;
                        hit_count += 1;
                    }
                    RagMetricType::MRR => {
                        mrr_sum += m.score;
                        mrr_count += 1;
                    }
                    RagMetricType::NDCG => {
                        ndcg_sum += m.score;
                        ndcg_count += 1;
                    }
                    _ => {}
                }
            }
        }

        Self {
            hit_rate: if hit_count > 0 {
                hit_sum / hit_count as f32
            } else {
                0.0
            },
            mrr: if mrr_count > 0 {
                mrr_sum / mrr_count as f32
            } else {
                0.0
            },
            ndcg: if ndcg_count > 0 {
                ndcg_sum / ndcg_count as f32
            } else {
                0.0
            },
        }
    }
}

/// 单个嵌入模型的对比评估结果（REQ-VEC-018）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedComparisonResult {
    pub model_name: String,
    pub dim: usize,
    pub metrics: EmbedMetricScores,
    pub sample_count: usize,
}
