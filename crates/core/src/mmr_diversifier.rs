//! MMR（Maximal Marginal Relevance）多样性重排（借鉴 OpenMontage corpus.py）。
//!
//! ## 背景
//!
//! 纯 Top-K 余弦相似度检索常返回冗余结果：同一文档的相邻 chunk 内容高度重叠，
//! 占据 Top-K 名额却提供重复信息。MMR 在相关性与多样性之间取得平衡，
//! 确保检索结果覆盖更多独立信息源。
//!
//! ## 算法
//!
//! ```text
//! score(c) = (1 - λ) * sim(c, query) - λ * max(sim(c, already_picked))
//! ```
//!
//! - `λ`（diversity）：多样性参数，0 = 纯相关度，1 = 纯多样性，默认 0.3
//! - `sim(c, query)`：候选 c 与查询的相关度（即 `RetrievalResult.score`）
//! - `max(sim(c, already_picked))`：候选 c 与已选结果的最大相似度
//!
//! ## 借鉴来源
//!
//! OpenMontage `lib/corpus.py` → `find_similar_set()` + `diversify()`
//! Carbonell & Goldstein 1998: "Use of MMR, Term Diversity for Reactive Document Retrieval"

use std::collections::HashMap;

use echomind_models::RetrievalResult;

/// MMR 配置。
///
/// # 参数
/// - `diversity`：多样性参数 λ（0.0 = 纯相关度, 1.0 = 纯多样性, 0.3 = 平衡默认值）
/// - `candidate_pool`：候选池大小上限（从原始结果中取前 N 个作为候选池，默认 30）
#[derive(Debug, Clone)]
pub struct MmrConfig {
    /// 多样性参数 λ
    pub diversity: f32,
    /// 候选池大小上限
    pub candidate_pool: usize,
}

impl Default for MmrConfig {
    fn default() -> Self {
        Self {
            diversity: 0.3,
            candidate_pool: 30,
        }
    }
}

impl MmrConfig {
    /// 创建纯相关度配置（diversity=0，等价于原始排序）。
    pub fn pure_relevance() -> Self {
        Self {
            diversity: 0.0,
            candidate_pool: 30,
        }
    }

    /// 创建纯多样性配置（diversity=1，最大化结果间差异）。
    pub fn pure_diversity() -> Self {
        Self {
            diversity: 1.0,
            candidate_pool: 30,
        }
    }

    /// 创建平衡配置（diversity=0.3，默认推荐）。
    pub fn balanced() -> Self {
        Self::default()
    }

    /// 设置多样性参数。
    #[must_use]
    pub fn with_diversity(mut self, diversity: f32) -> Self {
        self.diversity = diversity;
        self
    }

    /// 设置候选池大小。
    #[must_use]
    pub fn with_candidate_pool(mut self, pool: usize) -> Self {
        self.candidate_pool = pool;
        self
    }
}

// --- Token-based cosine similarity ---

/// 将文本分词为词频 HashMap（小写化 + 非字母数字分割）。
///
/// 纯 Rust 实现，零依赖。用于计算 chunk 间的文本相似度。
fn tokenize(text: &str) -> HashMap<String, f32> {
    let mut map: HashMap<String, f32> = HashMap::new();
    for word in text.split(|c: char| !c.is_alphanumeric()) {
        let w = word.trim().to_lowercase();
        if w.len() >= 2 {
            *map.entry(w).or_insert(0.0) += 1.0;
        }
    }
    map
}

/// 计算两个词频向量的余弦相似度。
///
/// `cos(a, b) = (a · b) / (||a|| * ||b||)`
fn cosine_sim_tokens(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // dot product — iterate smaller map for efficiency
    let mut dot = 0.0f32;
    let (smaller, larger) = if a.len() < b.len() { (a, b) } else { (b, a) };
    for (key, &val) in smaller {
        if let Some(&other) = larger.get(key) {
            dot += val * other;
        }
    }

    if dot == 0.0 {
        return 0.0;
    }

    // norms
    let norm_a: f32 = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let norm_b: f32 = b.values().map(|v| v * v).sum::<f32>().sqrt();

    if norm_a < 1e-8 || norm_b < 1e-8 {
        return 0.0;
    }

    dot / (norm_a * norm_b)
}

/// MMR 多样性重排：在相关度与多样性之间取得平衡。
///
/// # 参数
/// - `results`：原始检索结果（按相关度降序排列）
/// - `config`：MMR 配置
/// - `top_k`：返回结果数量上限
///
/// # 返回
/// 重排后的 `RetrievalResult` 列表（长度 ≤ `top_k`）。
///
/// # 算法
///
/// 1. 从候选池中选取相关度最高的结果作为第一个 picked
/// 2. 对每个剩余候选，计算 MMR 分数：
///    `mmr(c) = (1-λ) * sim(c, query) - λ * max(sim(c, picked))`
/// 3. 选 MMR 分数最高的候选加入 picked
/// 4. 重复直到 picked 达到 top_k 或候选池耗尽
pub fn mmr_diversify(
    results: Vec<RetrievalResult>,
    config: &MmrConfig,
    top_k: usize,
) -> Vec<RetrievalResult> {
    if results.is_empty() || top_k == 0 {
        return Vec::new();
    }

    // 限制候选池大小
    let pool_size = config.candidate_pool.min(results.len());
    let candidates: Vec<RetrievalResult> = results.into_iter().take(pool_size).collect();

    if candidates.len() <= 1 || top_k == 1 {
        return candidates.into_iter().take(top_k).collect();
    }

    // 预计算所有候选的 token 频率向量
    let token_maps: Vec<HashMap<String, f32>> = candidates
        .iter()
        .map(|r| tokenize(&r.chunk.content))
        .collect();

    let mut picked: Vec<usize> = Vec::with_capacity(top_k);
    let mut remaining: Vec<usize> = (0..candidates.len()).collect();

    // 第一个：选相关度最高的（candidates 已按 score 降序，即 index 0）
    picked.push(0);
    remaining.retain(|&i| i != 0);

    while !remaining.is_empty() && picked.len() < top_k {
        let mut best_idx = remaining[0];
        let mut best_score = f32::MIN;

        for &i in &remaining {
            // sim(c, query) = 原始检索分数
            let sim_query = candidates[i].score;

            // max(sim(c, already_picked))
            let mut max_sim_picked = 0.0f32;
            for &j in &picked {
                let sim = cosine_sim_tokens(&token_maps[i], &token_maps[j]);
                if sim > max_sim_picked {
                    max_sim_picked = sim;
                }
            }

            // MMR score = (1-λ) * sim(c, query) - λ * max(sim(c, picked))
            let mmr = (1.0 - config.diversity) * sim_query - config.diversity * max_sim_picked;

            if mmr > best_score {
                best_score = mmr;
                best_idx = i;
            }
        }

        picked.push(best_idx);
        remaining.retain(|&i| i != best_idx);
    }

    picked.into_iter().map(|i| candidates[i].clone()).collect()
}

/// 相邻去冗余：确保结果列表中相邻两项不来自同一文档的相邻 chunk。
///
/// 借鉴 OpenMontage `diversify()` 的贪心策略：给定候选列表，贪心选取 `n` 个
/// 互相不来自同一文档相邻位置的 chunk。保留相关度排序的同时减少视觉冗余。
///
/// # 参数
/// - `results`：原始检索结果（按相关度降序）
/// - `n`：期望保留的结果数量
///
/// # 返回
/// 去冗余后的结果列表（长度 ≤ `n`）。
///
/// # 规则
/// - 同文档同 sequence → 视为相同，跳过
/// - 同文档相邻 sequence（差 1）→ 视为冗余，跳过
/// - 不同文档或同文档非相邻 → 保留
pub fn dedup_adjacent(results: Vec<RetrievalResult>, n: usize) -> Vec<RetrievalResult> {
    if results.is_empty() || n == 0 {
        return Vec::new();
    }

    let mut picked: Vec<RetrievalResult> = Vec::with_capacity(n);
    let mut skipped: Vec<RetrievalResult> = Vec::new();

    for result in results {
        if picked.len() >= n {
            break;
        }

        // 检查是否与已选结果中的任意一个同文档且相邻
        let is_redundant = picked.iter().any(|p| {
            p.chunk.doc_id == result.chunk.doc_id
                && (p.chunk.sequence as i64 - result.chunk.sequence as i64).abs() <= 1
        });

        if !is_redundant {
            picked.push(result);
        } else {
            skipped.push(result);
        }
    }

    // 如果去冗余后不足 n 个，用被跳过的结果补齐（保留原始排序）
    if picked.len() < n {
        let picked_ids: std::collections::HashSet<String> =
            picked.iter().map(|p| p.chunk.id.clone()).collect();
        for result in skipped {
            if picked.len() >= n {
                break;
            }
            if !picked_ids.contains(&result.chunk.id) {
                picked.push(result);
            }
        }
    }

    picked
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use echomind_models::{Chunk, RetrievalResult};

    fn make_result(
        id: &str,
        doc_id: &str,
        content: &str,
        score: f32,
        seq: usize,
    ) -> RetrievalResult {
        RetrievalResult {
            chunk: Chunk {
                id: id.to_string(),
                doc_id: doc_id.to_string(),
                content: content.to_string(),
                token_count: content.len() / 4,
                sequence: seq,
            },
            score,
            doc_name: String::new(),
        }
    }

    /// TC-MMR-001: 空输入返回空列表
    #[test]
    fn test_mmr_empty_input() {
        let results: Vec<RetrievalResult> = vec![];
        let config = MmrConfig::default();
        let diversified = mmr_diversify(results, &config, 5);
        assert!(diversified.is_empty());
    }

    /// TC-MMR-002: 单个结果直接返回
    #[test]
    fn test_mmr_single_result() {
        let results = vec![make_result("a", "d1", "Rust memory safety", 0.9, 0)];
        let config = MmrConfig::default();
        let diversified = mmr_diversify(results, &config, 5);
        assert_eq!(diversified.len(), 1);
        assert_eq!(diversified[0].chunk.id, "a");
    }

    /// TC-MMR-003: 纯相关度（diversity=0）保持原始排序
    #[test]
    fn test_mmr_pure_relevance() {
        let results = vec![
            make_result("a", "d1", "Rust memory safety ownership", 0.9, 0),
            make_result("b", "d1", "Rust memory safety borrowing", 0.85, 1),
            make_result("c", "d2", "Python garbage collection", 0.7, 0),
        ];
        let config = MmrConfig::pure_relevance();
        let diversified = mmr_diversify(results, &config, 3);
        assert_eq!(diversified.len(), 3);
        // diversity=0 → 纯相关度排序 → 原始顺序
        assert_eq!(diversified[0].chunk.id, "a");
        assert_eq!(diversified[1].chunk.id, "b");
        assert_eq!(diversified[2].chunk.id, "c");
    }

    /// TC-MMR-004: 默认多样性（0.3）将不同内容的结果排在前面
    #[test]
    fn test_mmr_balanced_diversifies() {
        let results = vec![
            make_result("a", "d1", "Rust memory safety ownership borrowing", 0.9, 0),
            make_result("b", "d1", "Rust memory safety borrowing rules", 0.88, 1),
            make_result("c", "d2", "Python garbage collection GC", 0.75, 0),
        ];
        let config = MmrConfig::balanced();
        let diversified = mmr_diversify(results, &config, 3);
        assert_eq!(diversified.len(), 3);
        // 第一个仍是最相关的
        assert_eq!(diversified[0].chunk.id, "a");
        // 第二个应该选 "c"（与 "a" 内容不同），而非 "b"（与 "a" 高度相似）
        assert_eq!(diversified[1].chunk.id, "c");
        // 第三个选 "b"
        assert_eq!(diversified[2].chunk.id, "b");
    }

    /// TC-MMR-005: top_k=0 返回空列表
    #[test]
    fn test_mmr_top_k_zero() {
        let results = vec![make_result("a", "d1", "content", 0.9, 0)];
        let config = MmrConfig::default();
        let diversified = mmr_diversify(results, &config, 0);
        assert!(diversified.is_empty());
    }

    /// TC-MMR-006: top_k 小于结果数量时截断
    #[test]
    fn test_mmr_top_k_truncation() {
        let results = vec![
            make_result("a", "d1", "apple banana", 0.9, 0),
            make_result("b", "d2", "cherry date", 0.8, 0),
            make_result("c", "d3", "elderberry fig", 0.7, 0),
        ];
        let config = MmrConfig::default();
        let diversified = mmr_diversify(results, &config, 2);
        assert_eq!(diversified.len(), 2);
    }

    /// TC-MMR-007: 候选池限制生效
    #[test]
    fn test_mmr_candidate_pool_limit() {
        let results: Vec<RetrievalResult> = (0..10)
            .map(|i| {
                make_result(
                    &format!("r{i}"),
                    "d1",
                    &format!("content number {i}"),
                    0.9 - i as f32 * 0.05,
                    i as usize,
                )
            })
            .collect();
        let config = MmrConfig::default().with_candidate_pool(5);
        let diversified = mmr_diversify(results, &config, 10);
        // 候选池只有 5 个，所以最多返回 5 个
        assert_eq!(diversified.len(), 5);
    }

    /// TC-MMR-008: 纯多样性（diversity=1）最大化结果间差异
    #[test]
    fn test_mmr_pure_diversity() {
        let results = vec![
            make_result(
                "a",
                "d1",
                "Rust memory safety ownership borrowing lifetimes",
                0.9,
                0,
            ),
            make_result(
                "b",
                "d1",
                "Rust memory safety borrowing rules lifetimes",
                0.88,
                1,
            ),
            make_result("c", "d2", "Python garbage collection GC marksweep", 0.75, 0),
        ];
        let config = MmrConfig::pure_diversity();
        let diversified = mmr_diversify(results, &config, 3);
        assert_eq!(diversified.len(), 3);
        // 第一个仍是最相关的（picked 为空时 MMR = sim_query）
        assert_eq!(diversified[0].chunk.id, "a");
        // 纯多样性 → 第二个选与 "a" 最不相似的 → "c"（Python vs Rust）
        assert_eq!(diversified[1].chunk.id, "c");
    }

    /// TC-MMR-009: dedup_adjacent 去除同文档相邻 chunk
    #[test]
    fn test_dedup_adjacent_removes_neighbors() {
        let results = vec![
            make_result("a", "d1", "intro paragraph", 0.9, 0),
            make_result("b", "d1", "next paragraph", 0.85, 1), // 同文档 seq=1 → 与 a 相邻
            make_result("c", "d2", "different doc", 0.8, 0),
        ];
        let deduped = dedup_adjacent(results, 3);
        // "a" 被选 → "b" 被跳过（同文档相邻 seq diff=1）→ "c" 被选 → 不足 3，补 "b"
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].chunk.id, "a");
        assert_eq!(deduped[1].chunk.id, "c");
        assert_eq!(deduped[2].chunk.id, "b");
    }

    /// TC-MMR-010: dedup_adjacent 空输入
    #[test]
    fn test_dedup_adjacent_empty() {
        let deduped = dedup_adjacent(vec![], 5);
        assert!(deduped.is_empty());
    }

    /// TC-MMR-011: dedup_adjacent n=0 返回空
    #[test]
    fn test_dedup_adjacent_n_zero() {
        let results = vec![make_result("a", "d1", "content", 0.9, 0)];
        let deduped = dedup_adjacent(results, 0);
        assert!(deduped.is_empty());
    }

    /// TC-MMR-012: dedup_adjacent 同文档非相邻 chunk 保留
    #[test]
    fn test_dedup_adjacent_non_neighbor_kept() {
        let results = vec![
            make_result("a", "d1", "section one", 0.9, 0),
            make_result("b", "d1", "section five", 0.85, 5), // 同文档但 seq diff=5 → 非相邻
            make_result("c", "d1", "section two", 0.8, 2),   // 同文档 seq diff=2 与 a → 非相邻
        ];
        let deduped = dedup_adjacent(results, 3);
        // "a" 选 → "b" seq=5 与 a seq=0 diff=5 → 非相邻 → 选 → "c" seq=2 与 a diff=2 → 非相邻，与 b diff=3 → 非相邻 → 选
        assert_eq!(deduped.len(), 3);
    }

    /// TC-MMR-013: tokenize 基本功能
    #[test]
    fn test_tokenize_basic() {
        let tokens = tokenize("Hello World hello");
        assert_eq!(tokens.get("hello"), Some(&2.0));
        assert_eq!(tokens.get("world"), Some(&1.0));
    }

    /// TC-MMR-014: cosine_sim_tokens 相同文本相似度为 1.0
    #[test]
    fn test_cosine_sim_identical() {
        let a = tokenize("rust memory safety");
        let b = tokenize("rust memory safety");
        let sim = cosine_sim_tokens(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    /// TC-MMR-015: cosine_sim_tokens 完全不同文本相似度为 0.0
    #[test]
    fn test_cosine_sim_disjoint() {
        let a = tokenize("rust memory");
        let b = tokenize("python garbage");
        let sim = cosine_sim_tokens(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    /// TC-MMR-016: MmrConfig builder 方法链
    #[test]
    fn test_mmr_config_builder() {
        let config = MmrConfig::default()
            .with_diversity(0.5)
            .with_candidate_pool(100);
        assert!((config.diversity - 0.5).abs() < 0.001);
        assert_eq!(config.candidate_pool, 100);
    }
}
