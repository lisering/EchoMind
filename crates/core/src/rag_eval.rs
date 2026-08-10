//! RAG 评估指标系统（REQ-RAG-045，RAGAS 风格）。
//!
//! 借鉴 RAGAS（Retrieval-Augmented Generation Assessment）框架，
//! 实现纯 Rust + LLM-as-Judge 双层评估指标体系。
//!
//! # 架构设计
//!
//! ## 纯 Rust 检索指标（零 LLM 调用，零新增依赖）
//!
//! 1. **Hit Rate** — 相关文档是否在 top-k 检索结果中
//! 2. **MRR (Mean Reciprocal Rank)** — 第一个相关文档的倒数排名
//! 3. **NDCG (Normalized Discounted Cumulative Gain)** — 排序质量
//! 4. **Context Embedding Similarity** — 查询与上下文的平均余弦相似度
//! 5. **Keyword Overlap** — 查询与上下文的 token 重叠率
//!
//! ## LLM-as-Judge 生成指标（使用 `LLMProvider::one_shot`）
//!
//! 1. **Faithfulness** — 答案是否忠实于检索上下文（无幻觉）
//!    - 算法：LLM 将答案分解为声明 → 逐条验证是否可从上下文推导 → faithful/total
//! 2. **Answer Relevance** — 答案是否切题回答了用户问题
//!    - 算法：LLM 直接评分 0-10 → 归一化到 [0, 1]
//! 3. **Context Precision** — 检索上下文是否与问题相关
//!    - 算法：LLM 逐条判断每个上下文是否相关 → 加权精度
//! 4. **Context Recall** — 上下文是否包含回答所需信息（需要 ground truth）
//!    - 算法：LLM 将 GT 分解为声明 → 逐条验证是否可从上下文推导 → covered/total
//!
//! # 零新增依赖
//!
//! 复用现有 `LLMProvider` trait（`one_shot` 方法）+ `echomind_models` 数据模型。
//! 余弦相似度复用 `crates/core/src/cache.rs::cosine_similarity()`。

use anyhow::Result;
use echomind_models::{
    RagEvalMetric, RagEvalReport, RagEvalSample, RagEvalSettings, RagMetricType,
};

use crate::LLMProvider;

// ============================================================
// 纯 Rust 检索指标（零 LLM 调用）
// ============================================================

/// 计算命中率（Hit Rate）。
///
/// 检查相关文档是否出现在检索结果中。
///
/// # 参数
/// - `relevance`：布尔数组，表示每个检索结果是否相关
///
/// # 返回
/// - 1.0：至少一个相关文档在检索结果中
/// - 0.0：没有相关文档
pub fn hit_rate(relevance: &[bool]) -> f32 {
    if relevance.is_empty() {
        return 0.0;
    }
    if relevance.iter().any(|&r| r) {
        1.0
    } else {
        0.0
    }
}

/// 计算平均倒数排名（Mean Reciprocal Rank, MRR）。
///
/// 第一个相关文档的排名倒数。
///
/// # 参数
/// - `relevance`：布尔数组，表示每个检索结果是否相关
///
/// # 返回
/// - 1.0：第一个结果就是相关文档
/// - 1/n：第 n 个结果是第一个相关文档
/// - 0.0：没有相关文档
pub fn mrr(relevance: &[bool]) -> f32 {
    for (i, &is_relevant) in relevance.iter().enumerate() {
        if is_relevant {
            return 1.0 / (i + 1) as f32;
        }
    }
    0.0
}

/// 计算归一化折扣累积增益（NDCG）。
///
/// 评估排序质量，相关度高的文档排在前面得更高分。
///
/// # 参数
/// - `relevance_scores`：每个检索结果的相关性分数 [0.0, 1.0]
///
/// # 返回
/// - [0.0, 1.0]，1.0 = 完美排序
pub fn ndcg(relevance_scores: &[f32]) -> f32 {
    if relevance_scores.is_empty() {
        return 0.0;
    }

    // DCG: Σ rel_i / log2(i + 2)
    let dcg: f32 = relevance_scores
        .iter()
        .enumerate()
        .map(|(i, &score)| score / (i as f32 + 2.0).log2())
        .sum();

    // IDCG: 理想排序（降序排列）的 DCG
    let mut sorted = relevance_scores.to_vec();
    sorted.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    let idcg: f32 = sorted
        .iter()
        .enumerate()
        .map(|(i, &score)| score / (i as f32 + 2.0).log2())
        .sum();

    if idcg > 0.0 { dcg / idcg } else { 0.0 }
}

/// 计算查询与上下文的关键词重叠率。
///
/// 使用简单分词（按空格 + 中文字符），计算查询 token 在上下文中出现的比例。
///
/// # 参数
/// - `query`：用户查询
/// - `contexts`：检索到的上下文列表
///
/// # 返回
/// - [0.0, 1.0]，1.0 = 查询所有 token 都在某个上下文中出现
pub fn keyword_overlap(query: &str, contexts: &[String]) -> f32 {
    let query_tokens = tokenize(query);
    if query_tokens.is_empty() {
        return 0.0;
    }

    let mut matched = 0usize;
    for token in &query_tokens {
        let lower = token.to_lowercase();
        if contexts
            .iter()
            .any(|ctx| ctx.to_lowercase().contains(&lower))
        {
            matched += 1;
        }
    }

    matched as f32 / query_tokens.len() as f32
}

/// 计算查询嵌入与上下文嵌入的平均余弦相似度。
///
/// # 参数
/// - `query_embedding`：查询的嵌入向量
/// - `context_embeddings`：上下文的嵌入向量列表
///
/// # 返回
/// - [0.0, 1.0]，1.0 = 完全相似
pub fn context_embedding_similarity(
    query_embedding: &[f32],
    context_embeddings: &[Vec<f32>],
) -> f32 {
    if context_embeddings.is_empty() || query_embedding.is_empty() {
        return 0.0;
    }

    let sims: Vec<f32> = context_embeddings
        .iter()
        .map(|ctx_emb| cosine_similarity(query_embedding, ctx_emb))
        .collect();

    let sum: f32 = sims.iter().sum();
    // 余弦相似度 [-1, 1] → 归一化到 [0, 1]
    let avg = sum / context_embeddings.len() as f32;
    (avg + 1.0) / 2.0
}

// ============================================================
// LLM-as-Judge 生成指标
// ============================================================

/// 评估答案忠实度（Faithfulness）。
///
/// 使用 LLM 将答案分解为声明，逐条验证是否可从上下文推导。
///
/// # 算法
/// 1. LLM 从答案中提取原子声明列表
/// 2. 对每条声明，LLM 判断是否可从上下文推导
/// 3. Score = 可推导声明数 / 总声明数
///
/// # 参数
/// - `llm`：LLM Provider
/// - `answer`：待评估的答案
/// - `contexts`：检索到的上下文
///
/// # 返回
/// - `Ok(score)`：评估分数 [0.0, 1.0]
/// - `Err(...)`：LLM 调用失败
pub async fn faithfulness<L: LLMProvider>(
    llm: &L,
    answer: &str,
    contexts: &[String],
) -> Result<f32> {
    if answer.is_empty() || contexts.is_empty() {
        return Ok(0.0);
    }

    let context_text = contexts.join("\n\n---\n\n");

    let system = "You are a RAG evaluation assistant. Your task is to evaluate the faithfulness of an answer to the given context. Faithfulness measures whether all claims in the answer can be inferred from the context.\n\nRespond in this exact format:\nCLAIMS: <number of total claims>\nSUPPORTED: <number of claims supported by context>\nREASON: <brief explanation>";

    let prompt = format!(
        "Context:\n{context_text}\n\nAnswer:\n{answer}\n\nAnalyze the answer and identify all factual claims. For each claim, determine if it is supported by the context above. Report the total number of claims and how many are supported."
    );

    let response = llm.one_shot(system, &prompt).await?;
    let response = match response {
        Some(text) => text,
        None => return Ok(0.0), // Provider 不支持 one_shot
    };

    parse_faithfulness_response(&response)
}

/// 评估答案相关性（Answer Relevance）。
///
/// 使用 LLM 直接评估答案是否切题回答了问题。
///
/// # 算法
/// LLM 评分 0-10 → 归一化到 [0, 1]
///
/// # 参数
/// - `llm`：LLM Provider
/// - `query`：用户问题
/// - `answer`：待评估的答案
pub async fn answer_relevance<L: LLMProvider>(llm: &L, query: &str, answer: &str) -> Result<f32> {
    if query.is_empty() || answer.is_empty() {
        return Ok(0.0);
    }

    let system = "You are a RAG evaluation assistant. Your task is to evaluate how relevant the answer is to the question. Score from 0 to 10 where 0 means completely irrelevant and 10 means perfectly relevant.\n\nRespond in this exact format:\nSCORE: <0-10>\nREASON: <brief explanation>";

    let prompt = format!(
        "Question:\n{query}\n\nAnswer:\n{answer}\n\nEvaluate the relevance of the answer to the question."
    );

    let response = llm.one_shot(system, &prompt).await?;
    let response = match response {
        Some(text) => text,
        None => return Ok(0.0),
    };

    parse_score_response(&response, 10.0)
}

/// 评估上下文精度（Context Precision）。
///
/// 使用 LLM 逐条判断每个上下文是否与问题相关。
///
/// # 算法
/// 对每个上下文，LLM 判断 yes/no → 加权精度（rank 越高权重越大）
/// Score = (1/total) * Σ(precision_at_k * relevance_k)
///
/// # 参数
/// - `llm`：LLM Provider
/// - `query`：用户问题
/// - `contexts`：检索到的上下文列表
pub async fn context_precision<L: LLMProvider>(
    llm: &L,
    query: &str,
    contexts: &[String],
) -> Result<f32> {
    if query.is_empty() || contexts.is_empty() {
        return Ok(0.0);
    }

    let system = "You are a RAG evaluation assistant. For each context passage, determine if it is relevant to answering the question. Respond with a JSON array of 0/1 values, where 1 means relevant and 0 means not relevant.\n\nExample: [1, 0, 1]\n\nRespond ONLY with the JSON array, no other text.";

    let contexts_json: Vec<&str> = contexts.iter().map(|s| s.as_str()).collect();
    let prompt = format!(
        "Question: {query}\n\nContexts (in order):\n{}\n\nFor each context, output 1 if relevant to the question, 0 if not. Respond as a JSON array.",
        contexts_json
            .iter()
            .enumerate()
            .map(|(i, c)| format!("[{i}] {c}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let response = llm.one_shot(system, &prompt).await?;
    let response = match response {
        Some(text) => text,
        None => return Ok(0.0),
    };

    let relevance = parse_relevance_array(&response, contexts.len());
    if relevance.is_empty() {
        return Ok(0.0);
    }

    // 加权精度：precision_at_k
    let total = contexts.len() as f32;
    let mut weighted_sum = 0.0f32;
    let mut relevant_count = 0usize;

    for (i, &is_relevant) in relevance.iter().enumerate() {
        if is_relevant {
            relevant_count += 1;
            let precision_at_k = relevant_count as f32 / (i + 1) as f32;
            weighted_sum += precision_at_k;
        }
    }

    if relevant_count == 0 {
        Ok(0.0)
    } else {
        Ok(weighted_sum / total)
    }
}

/// 评估上下文召回（Context Recall）。
///
/// 使用 LLM 将 ground truth 分解为声明，逐条验证是否可从上下文推导。
///
/// # 算法
/// 1. LLM 从 GT 中提取声明
/// 2. 对每条声明，LLM 判断是否可从上下文推导
/// 3. Score = 可推导声明数 / 总声明数
///
/// # 参数
/// - `llm`：LLM Provider
/// - `ground_truth`：参考答案
/// - `contexts`：检索到的上下文
pub async fn context_recall<L: LLMProvider>(
    llm: &L,
    ground_truth: &str,
    contexts: &[String],
) -> Result<f32> {
    if ground_truth.is_empty() || contexts.is_empty() {
        return Ok(0.0);
    }

    let context_text = contexts.join("\n\n---\n\n");

    let system = "You are a RAG evaluation assistant. Your task is to evaluate context recall. Given a ground truth answer and retrieved context, determine what fraction of the ground truth statements can be attributed to the context.\n\nRespond in this exact format:\nSTATEMENTS: <total number of statements in ground truth>\nCOVERED: <number of statements attributable to context>\nREASON: <brief explanation>";

    let prompt = format!(
        "Context:\n{context_text}\n\nGround Truth Answer:\n{ground_truth}\n\nAnalyze the ground truth answer and identify all factual statements. For each statement, determine if it can be attributed to (found in or inferred from) the context above."
    );

    let response = llm.one_shot(system, &prompt).await?;
    let response = match response {
        Some(text) => text,
        None => return Ok(0.0),
    };

    parse_faithfulness_response(&response) // Same parsing logic
}

// ============================================================
// 主评估器
// ============================================================

/// RAG 评估器（REQ-RAG-045）。
///
/// 根据设置选择性计算指标，支持单样本和批量评估。
/// LLM 指标失败时优雅降级（跳过该指标，不影响其他指标）。
pub struct RagEvaluator {
    /// 评估设置
    settings: RagEvalSettings,
}

impl RagEvaluator {
    /// 使用默认设置创建评估器。
    pub fn new() -> Self {
        Self {
            settings: RagEvalSettings::default(),
        }
    }

    /// 使用指定设置创建评估器。
    pub fn with_settings(settings: RagEvalSettings) -> Self {
        Self { settings }
    }

    /// 获取设置引用。
    pub fn settings(&self) -> &RagEvalSettings {
        &self.settings
    }

    /// 评估单个样本。
    ///
    /// 根据设置选择性地计算各指标。LLM 指标失败时跳过（不返回 Err）。
    /// 纯 Rust 指标始终计算（如果数据可用）。
    pub async fn evaluate<L: LLMProvider>(
        &self,
        llm: &L,
        sample: &RagEvalSample,
    ) -> Result<Vec<RagEvalMetric>> {
        let mut metrics = Vec::new();

        // --- 纯 Rust 检索指标 ---

        if self.settings.enable_retrieval_metrics {
            // Hit Rate + MRR（需要 relevant_indices）
            if let Some(ref indices) = sample.relevant_indices {
                let relevance: Vec<bool> = (0..sample.contexts.len())
                    .map(|i| indices.contains(&i))
                    .collect();
                metrics.push(RagEvalMetric::new(
                    RagMetricType::HitRate,
                    hit_rate(&relevance),
                ));
                metrics.push(RagEvalMetric::new(RagMetricType::MRR, mrr(&relevance)));
            }

            // NDCG（需要 relevance_scores 或用检索分数）
            if let Some(ref scores) = sample.relevance_scores {
                metrics.push(RagEvalMetric::new(RagMetricType::NDCG, ndcg(scores)));
            }
        }

        // Keyword Overlap
        if self.settings.enable_keyword_overlap {
            let overlap = keyword_overlap(&sample.query, &sample.contexts);
            metrics.push(RagEvalMetric::new(RagMetricType::KeywordOverlap, overlap));
        }

        // Context Similarity（需要嵌入向量）
        if self.settings.enable_embedding_metrics
            && let (Some(q_emb), Some(c_embs)) = (
                sample.query_embedding.as_ref(),
                sample.context_embeddings.as_ref(),
            )
        {
            let sim = context_embedding_similarity(q_emb, c_embs);
            metrics.push(RagEvalMetric::new(RagMetricType::ContextSimilarity, sim));
        }

        // --- LLM-as-Judge 指标 ---

        if self.settings.enable_faithfulness {
            match faithfulness(llm, &sample.answer, &sample.contexts).await {
                Ok(score) => metrics.push(RagEvalMetric::new(RagMetricType::Faithfulness, score)),
                Err(e) => tracing::warn!("Faithfulness 评估失败: {e}"),
            }
        }

        if self.settings.enable_answer_relevance {
            match answer_relevance(llm, &sample.query, &sample.answer).await {
                Ok(score) => {
                    metrics.push(RagEvalMetric::new(RagMetricType::AnswerRelevance, score))
                }
                Err(e) => tracing::warn!("Answer Relevance 评估失败: {e}"),
            }
        }

        if self.settings.enable_context_precision {
            match context_precision(llm, &sample.query, &sample.contexts).await {
                Ok(score) => {
                    metrics.push(RagEvalMetric::new(RagMetricType::ContextPrecision, score))
                }
                Err(e) => tracing::warn!("Context Precision 评估失败: {e}"),
            }
        }

        if self.settings.enable_context_recall
            && let Some(ref gt) = sample.ground_truth
        {
            match context_recall(llm, gt, &sample.contexts).await {
                Ok(score) => metrics.push(RagEvalMetric::new(RagMetricType::ContextRecall, score)),
                Err(e) => tracing::warn!("Context Recall 评估失败: {e}"),
            }
        }

        Ok(metrics)
    }

    /// 批量评估多个样本，返回聚合报告。
    ///
    /// 逐个评估每个样本，LLM 指标失败时跳过。
    /// 返回的 `RagEvalReport` 包含每个样本的指标和整体平均值。
    pub async fn evaluate_batch<L: LLMProvider>(
        &self,
        llm: &L,
        samples: &[RagEvalSample],
    ) -> Result<RagEvalReport> {
        if samples.is_empty() {
            return Ok(RagEvalReport::empty());
        }

        let mut per_sample = Vec::with_capacity(samples.len());
        for sample in samples {
            let metrics = self.evaluate(llm, sample).await?;
            per_sample.push(metrics);
        }

        Ok(RagEvalReport::from_samples(per_sample))
    }
}

impl Default for RagEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 简单分词：按空格分割 + 中文逐字。
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.split_whitespace() {
        // 检查是否包含中文字符
        let has_cjk = word.chars().any(|c| {
            ('\u{4E00}'..='\u{9FFF}').contains(&c) || ('\u{3400}'..='\u{4DBF}').contains(&c)
        });

        if has_cjk {
            // 中文逐字提取（连续非CJK部分作为一个 token）
            let mut current_latin = String::new();
            for ch in word.chars() {
                if ('\u{4E00}'..='\u{9FFF}').contains(&ch)
                    || ('\u{3400}'..='\u{4DBF}').contains(&ch)
                {
                    if !current_latin.is_empty() {
                        tokens.push(current_latin.clone());
                        current_latin.clear();
                    }
                    tokens.push(ch.to_string());
                } else if ch.is_alphanumeric() {
                    current_latin.push(ch);
                } else if !current_latin.is_empty() {
                    tokens.push(current_latin.clone());
                    current_latin.clear();
                }
            }
            if !current_latin.is_empty() {
                tokens.push(current_latin);
            }
        } else {
            // 纯拉丁：去除标点
            let cleaned: String = word
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !cleaned.is_empty() {
                tokens.push(cleaned);
            }
        }
    }
    tokens
}

/// 余弦相似度（复用 cache.rs 的实现，避免重复代码）。
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// 解析 Faithfulness / Context Recall 响应。
///
/// 预期格式：`CLAIMS: N\nSUPPORTED: M\nREASON: ...`
/// 或 `STATEMENTS: N\nCOVERED: M\nREASON: ...`
fn parse_faithfulness_response(response: &str) -> Result<f32> {
    let response_upper = response.to_uppercase();
    let total = extract_number_after(&response_upper, "CLAIMS:")
        .or_else(|| extract_number_after(&response_upper, "STATEMENTS:"));
    let supported = extract_number_after(&response_upper, "SUPPORTED:")
        .or_else(|| extract_number_after(&response_upper, "COVERED:"));

    match (total, supported) {
        (Some(t), Some(s)) if t > 0 => {
            let score = s as f32 / t as f32;
            Ok(score.clamp(0.0, 1.0))
        }
        _ => {
            // 尝试从纯数字解析
            let numbers: Vec<usize> = response
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<usize>().ok())
                .collect();
            if numbers.len() >= 2 && numbers[0] > 0 {
                let score = numbers[1] as f32 / numbers[0] as f32;
                Ok(score.clamp(0.0, 1.0))
            } else {
                Ok(0.5) // 无法解析时返回中性分数
            }
        }
    }
}

/// 解析评分响应（如 SCORE: 8 → 0.8）。
fn parse_score_response(response: &str, max_score: f32) -> Result<f32> {
    let response_upper = response.to_uppercase();
    if let Some(score) = extract_number_after(&response_upper, "SCORE:") {
        let normalized = score as f32 / max_score;
        return Ok(normalized.clamp(0.0, 1.0));
    }

    // 尝试提取第一个数字
    let numbers: Vec<usize> = response
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<usize>().ok())
        .collect();
    if let Some(&first) = numbers.first() {
        let normalized = first as f32 / max_score;
        return Ok(normalized.clamp(0.0, 1.0));
    }

    Ok(0.5)
}

/// 解析相关性数组（如 [1, 0, 1] → [true, false, true]）。
fn parse_relevance_array(response: &str, expected_len: usize) -> Vec<bool> {
    // 尝试 JSON 解析
    let trimmed = response.trim();
    if trimmed.starts_with('[') {
        // 简单解析 JSON 数组
        let inner = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
        let parts: Vec<bool> = inner
            .split(',')
            .map(|s| {
                let s = s.trim();
                s == "1" || s.eq_ignore_ascii_case("true")
            })
            .take(expected_len)
            .collect();
        if !parts.is_empty() {
            return parts;
        }
    }

    // 尝试提取数字序列
    let nums: Vec<bool> = response
        .chars()
        .filter_map(|c| {
            if c == '1' {
                Some(true)
            } else if c == '0' {
                Some(false)
            } else {
                None
            }
        })
        .take(expected_len)
        .collect();

    if nums.len() == expected_len {
        nums
    } else if nums.is_empty() {
        // 默认全部相关（保守估计）
        vec![true; expected_len]
    } else {
        nums
    }
}

/// 从文本中提取关键词后的数字。
fn extract_number_after(text: &str, keyword: &str) -> Option<usize> {
    if let Some(pos) = text.find(keyword) {
        let after = &text[pos + keyword.len()..];
        let num_str: String = after
            .trim()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if !num_str.is_empty() {
            return num_str.parse::<usize>().ok();
        }
    }
    None
}
