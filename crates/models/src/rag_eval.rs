//! RAG 评估指标系统数据契约。
//!
//! 对应 SRS：REQ-RAG-045（RAGAS 风格）。

use serde::{Deserialize, Serialize};

/// RAG 评估指标类型（REQ-RAG-045）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RagMetricType {
    Faithfulness,
    AnswerRelevance,
    ContextPrecision,
    ContextRecall,
    HitRate,
    MRR,
    NDCG,
    ContextSimilarity,
    KeywordOverlap,
}

impl RagMetricType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Faithfulness => "faithfulness",
            Self::AnswerRelevance => "answer_relevance",
            Self::ContextPrecision => "context_precision",
            Self::ContextRecall => "context_recall",
            Self::HitRate => "hit_rate",
            Self::MRR => "mrr",
            Self::NDCG => "ndcg",
            Self::ContextSimilarity => "context_similarity",
            Self::KeywordOverlap => "keyword_overlap",
        }
    }

    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "faithfulness" => Some(Self::Faithfulness),
            "answer_relevance" => Some(Self::AnswerRelevance),
            "context_precision" => Some(Self::ContextPrecision),
            "context_recall" => Some(Self::ContextRecall),
            "hit_rate" => Some(Self::HitRate),
            "mrr" => Some(Self::MRR),
            "ndcg" => Some(Self::NDCG),
            "context_similarity" => Some(Self::ContextSimilarity),
            "keyword_overlap" => Some(Self::KeywordOverlap),
            _ => None,
        }
    }

    pub fn needs_llm(&self) -> bool {
        matches!(
            self,
            Self::Faithfulness
                | Self::AnswerRelevance
                | Self::ContextPrecision
                | Self::ContextRecall
        )
    }

    pub fn needs_ground_truth(&self) -> bool {
        matches!(self, Self::ContextRecall)
    }

    pub fn needs_embedding(&self) -> bool {
        matches!(self, Self::ContextSimilarity)
    }
}

/// RAG 评估样本（单次问答评估输入，REQ-RAG-045）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalSample {
    pub query: String,
    pub answer: String,
    pub contexts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub ground_truth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relevance_scores: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub relevant_indices: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub query_embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub context_embeddings: Option<Vec<Vec<f32>>>,
}

impl RagEvalSample {
    pub fn new(query: String, answer: String, contexts: Vec<String>) -> Self {
        Self {
            query,
            answer,
            contexts,
            ground_truth: None,
            relevance_scores: None,
            relevant_indices: None,
            query_embedding: None,
            context_embeddings: None,
        }
    }

    pub fn with_ground_truth(mut self, truth: String) -> Self {
        self.ground_truth = Some(truth);
        self
    }

    pub fn with_relevance_scores(mut self, scores: Vec<f32>) -> Self {
        self.relevance_scores = Some(scores);
        self
    }

    pub fn with_relevant_indices(mut self, indices: Vec<usize>) -> Self {
        self.relevant_indices = Some(indices);
        self
    }
}

/// RAG 评估指标结果（REQ-RAG-045）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalMetric {
    pub metric_type: RagMetricType,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub details: Option<String>,
}

impl RagEvalMetric {
    pub fn new(metric_type: RagMetricType, score: f32) -> Self {
        Self {
            metric_type,
            score: score.clamp(0.0, 1.0),
            details: None,
        }
    }

    pub fn with_details(metric_type: RagMetricType, score: f32, details: String) -> Self {
        Self {
            metric_type,
            score: score.clamp(0.0, 1.0),
            details: Some(details),
        }
    }
}

/// RAG 评估报告（REQ-RAG-045）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalReport {
    pub sample_count: usize,
    pub aggregate_metrics: Vec<RagEvalMetric>,
    pub per_sample_metrics: Vec<Vec<RagEvalMetric>>,
}

impl RagEvalReport {
    pub fn empty() -> Self {
        Self {
            sample_count: 0,
            aggregate_metrics: Vec::new(),
            per_sample_metrics: Vec::new(),
        }
    }

    pub fn from_samples(per_sample: Vec<Vec<RagEvalMetric>>) -> Self {
        let sample_count = per_sample.len();
        if sample_count == 0 {
            return Self::empty();
        }

        let mut type_scores: std::collections::HashMap<String, Vec<f32>> =
            std::collections::HashMap::new();
        for metrics in &per_sample {
            for m in metrics {
                let name = m.metric_type.as_str().to_string();
                type_scores.entry(name).or_default().push(m.score);
            }
        }

        let aggregate: Vec<RagEvalMetric> = type_scores
            .iter()
            .filter_map(|(name, scores)| {
                RagMetricType::parse_str(name).map(|mt| {
                    let avg = scores.iter().sum::<f32>() / scores.len() as f32;
                    RagEvalMetric::new(mt, avg)
                })
            })
            .collect();

        Self {
            sample_count,
            aggregate_metrics: aggregate,
            per_sample_metrics: per_sample,
        }
    }

    pub fn get_metric(&self, metric_type: &RagMetricType) -> Option<f32> {
        self.aggregate_metrics
            .iter()
            .find(|m| &m.metric_type == metric_type)
            .map(|m| m.score)
    }
}

/// RAG 评估设置（REQ-RAG-045）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalSettings {
    pub enable_faithfulness: bool,
    pub enable_answer_relevance: bool,
    pub enable_context_precision: bool,
    pub enable_context_recall: bool,
    pub enable_retrieval_metrics: bool,
    pub enable_embedding_metrics: bool,
    pub enable_keyword_overlap: bool,
}

impl Default for RagEvalSettings {
    fn default() -> Self {
        Self {
            enable_faithfulness: true,
            enable_answer_relevance: true,
            enable_context_precision: true,
            enable_context_recall: false,
            enable_retrieval_metrics: true,
            enable_embedding_metrics: false,
            enable_keyword_overlap: true,
        }
    }
}

/// RAG 评估数据集中的单个评估样本（REQ-RAG-048）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalDatasetSample {
    pub query: String,
    pub ground_truth: String,
    #[serde(default)]
    pub relevant_doc_ids: Vec<String>,
    #[serde(default)]
    pub relevant_chunk_ids: Vec<String>,
}

impl RagEvalDatasetSample {
    pub fn new(query: String, ground_truth: String) -> Self {
        Self {
            query,
            ground_truth,
            relevant_doc_ids: Vec::new(),
            relevant_chunk_ids: Vec::new(),
        }
    }

    pub fn with_doc_ids(mut self, doc_ids: Vec<String>) -> Self {
        self.relevant_doc_ids = doc_ids;
        self
    }

    pub fn with_chunk_ids(mut self, chunk_ids: Vec<String>) -> Self {
        self.relevant_chunk_ids = chunk_ids;
        self
    }
}

/// RAG 评估数据集（REQ-RAG-048）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagEvalDataset {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub samples: Vec<RagEvalDatasetSample>,
}

impl RagEvalDataset {
    pub fn new(name: String, samples: Vec<RagEvalDatasetSample>) -> Self {
        Self {
            name,
            description: String::new(),
            samples,
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}
