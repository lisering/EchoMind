//! 自进化检索记忆（REQ-PERF-012）：记录哪些检索方法对哪类查询有效，自适应选择最佳策略。
//!
//! ## 核心理念
//!
//! 基于 AutoRAG 2.0 自进化记忆系统：系统持续记录每次检索的效果（命中/未命中、
//! 平均相关度分数），按查询类型分组统计，下次同类查询自动选择历史效果最佳的检索方法。
//!
//! ## 查询类型分类（纯规则，零 LLM）
//!
//! | 类型 | 特征 | 最佳方法（经验） |
//! |---|---|---|
//! | Factual（事实型） | 包含「是什么/定义/who/what/when」 | vector_only |
//! | Analytical（分析型） | 包含「比较/分析/优缺点/compare」 | hybrid_rerank |
//! | Code（代码型） | 包含代码片段、函数名、API | hybrid |
//! | Conversational（对话型） | 简短、上下文依赖、代词引用 | hybrid |
//!
//! ## 检索方法
//!
//! | 方法 | 说明 |
//! |---|---|
//! | vector_only | 纯向量余弦相似度检索 |
//! | hybrid | 向量 + BM25 关键词 RRF 融合 |
//! | hybrid_rerank | hybrid + Cross-Encoder 精排 |
//! | proposition | Proposition 级原子检索 |
//! | colbert | ColBERT 多向量 Late Interaction |
//!
//! ## 自适应策略
//!
//! - 冷启动（无历史数据）→ 使用默认方法（hybrid）
//! - 有历史数据 → 选择 hit_rate 最高的方法
//! - hit_rate 相同时 → 选择 avg_score 最高的方法
//! - 记忆查表 ~1ms（SQLite 单行 SELECT），不增加延迟
//!
//! ## 调研来源
//!
//! - AutoRAG 2.0 — 自进化记忆系统，自动优化检索策略
//! - Self-RAG (arXiv:2310.11511) — 自主检索决策启发

use serde::{Deserialize, Serialize};

// ============================================================
// 查询类型分类器（纯规则，零 LLM）
// ============================================================

/// 查询类型（决定最佳检索方法）。
///
/// 分类基于关键词规则匹配，零 LLM 调用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryType {
    /// 事实型查询：「X 是什么」「定义」「who/what/when」
    Factual,
    /// 分析型查询：「比较」「优缺点」「分析」「compare/analyze」
    Analytical,
    /// 代码型查询：包含函数名、API、代码片段
    Code,
    /// 对话型查询：简短、上下文依赖、代词引用
    Conversational,
}

impl QueryType {
    /// 返回类型字符串标识（用于数据库存储）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Factual => "factual",
            Self::Analytical => "analytical",
            Self::Code => "code",
            Self::Conversational => "conversational",
        }
    }

    /// 从字符串解析查询类型（不区分大小写）。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "factual" => Some(Self::Factual),
            "analytical" => Some(Self::Analytical),
            "code" => Some(Self::Code),
            "conversational" => Some(Self::Conversational),
            _ => None,
        }
    }

    /// 返回所有查询类型列表。
    pub fn all() -> [Self; 4] {
        [
            Self::Factual,
            Self::Analytical,
            Self::Code,
            Self::Conversational,
        ]
    }
}

/// 事实型查询关键词（中英文）。
const FACTUAL_KEYWORDS: &[&str] = &[
    "是什么",
    "什么是",
    "定义",
    "含义",
    "意思",
    "指的是",
    "全称",
    "缩写",
    "who",
    "what",
    "when",
    "where",
    "define",
    "definition",
    "meaning",
];

/// 分析型查询关键词（中英文）。
const ANALYTICAL_KEYWORDS: &[&str] = &[
    "比较",
    "对比",
    "分析",
    "优缺点",
    "区别",
    "差异",
    "优势",
    "劣势",
    "权衡",
    "总结",
    "综述",
    "compare",
    "contrast",
    "analyze",
    "analysis",
    "difference",
    "advantage",
    "disadvantage",
    "pros",
    "cons",
    "tradeoff",
    "trade-off",
    "summary",
    "summarize",
];

/// 代码型查询关键词（中英文）。
const CODE_KEYWORDS: &[&str] = &[
    "代码",
    "函数",
    "方法",
    "接口",
    "api",
    "类",
    "编译",
    "报错",
    "错误",
    "异常",
    "调试",
    "实现",
    "code",
    "function",
    "method",
    "class",
    "compile",
    "error",
    "exception",
    "debug",
    "implement",
    "stack",
    "trace",
    "runtime",
    "thread",
    "async",
    "await",
    "promise",
    "callback",
];

/// 对话型查询关键词（中英文）。
const CONVERSATIONAL_KEYWORDS: &[&str] = &[
    "他",
    "她",
    "它",
    "这个",
    "那个",
    "上面",
    "下面",
    "前面",
    "刚才",
    "继续",
    "还有呢",
    "然后呢",
    "it",
    "this",
    "that",
    "the above",
    "the following",
    "continue",
    "what about",
    "how about",
];

/// 查询类型分类器：根据查询文本特征判断查询类型（纯规则，零 LLM）。
///
/// 分类优先级：Code > Analytical > Factual > Conversational（默认）。
/// 优先级理由：
/// - Code 优先：代码查询特征最明显（函数名/API），且对检索方法敏感度高
/// - Analytical 次之：分析型查询对 reranker 受益最大
/// - Factual：事实型查询向量检索即可
/// - Conversational 为默认：无法明确分类时视为对话型
pub fn classify_query_type(query: &str) -> QueryType {
    let lower = query.to_lowercase();
    let trimmed = query.trim();

    // 代码型：包含代码特征（优先级最高）
    if CODE_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return QueryType::Code;
    }

    // 分析型：包含比较/分析关键词
    if ANALYTICAL_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return QueryType::Analytical;
    }

    // 事实型：包含事实查询关键词
    if FACTUAL_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return QueryType::Factual;
    }

    // 对话型：包含上下文引用关键词
    if CONVERSATIONAL_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return QueryType::Conversational;
    }

    // 短查询（≤ 10 字符）默认为对话型
    if trimmed.chars().count() <= 10 {
        return QueryType::Conversational;
    }

    // 默认：对话型
    QueryType::Conversational
}

// ============================================================
// 检索方法枚举
// ============================================================

/// 检索方法标识（用于记忆表和自适应选择）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalMethod {
    /// 纯向量余弦相似度检索
    VectorOnly,
    /// 向量 + BM25 关键词 RRF 融合
    Hybrid,
    /// Hybrid + Cross-Encoder 精排
    HybridRerank,
    /// Proposition 级原子检索
    Proposition,
    /// ColBERT 多向量 Late Interaction
    Colbert,
}

impl RetrievalMethod {
    /// 返回方法字符串标识（用于数据库存储）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VectorOnly => "vector_only",
            Self::Hybrid => "hybrid",
            Self::HybridRerank => "hybrid_rerank",
            Self::Proposition => "proposition",
            Self::Colbert => "colbert",
        }
    }

    /// 从字符串解析检索方法（不区分大小写）。
    pub fn parse_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "vector_only" => Some(Self::VectorOnly),
            "hybrid" => Some(Self::Hybrid),
            "hybrid_rerank" => Some(Self::HybridRerank),
            "proposition" => Some(Self::Proposition),
            "colbert" => Some(Self::Colbert),
            _ => None,
        }
    }

    /// 返回所有检索方法列表。
    pub fn all() -> [Self; 5] {
        [
            Self::VectorOnly,
            Self::Hybrid,
            Self::HybridRerank,
            Self::Proposition,
            Self::Colbert,
        ]
    }

    /// 返回该查询类型的默认推荐方法（冷启动时使用）。
    ///
    /// 基于 RAG 最佳实践的经验值：
    /// - Factual → VectorOnly（事实查询向量检索即可）
    /// - Analytical → HybridRerank（分析型查询受益于 reranker 精排）
    /// - Code → Hybrid（代码查询需要关键词精确匹配 + 向量语义匹配）
    /// - Conversational → Hybrid（对话型查询需要关键词+向量双通道）
    pub fn default_for(query_type: QueryType) -> Self {
        match query_type {
            QueryType::Factual => Self::VectorOnly,
            QueryType::Analytical => Self::HybridRerank,
            QueryType::Code => Self::Hybrid,
            QueryType::Conversational => Self::Hybrid,
        }
    }
}

// ============================================================
// 记忆记录
// ============================================================

/// 单条检索方法效果记录（对应 retrieval_memory 表一行）。
///
/// 记录某查询类型下某检索方法的累计效果统计。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    /// 查询类型
    pub query_type: QueryType,
    /// 检索方法
    pub method: RetrievalMethod,
    /// 命中次数（检索结果 score > 阈值且用户未中断）
    pub hit_count: u32,
    /// 未命中次数（检索结果为空或 score 过低）
    pub miss_count: u32,
    /// 平均相关度分数（0.0-1.0，检索结果 top-1 score 的滚动均值）
    pub avg_score: f32,
}

impl MemoryRecord {
    /// 创建新记录（初始值全为 0）。
    pub fn new(query_type: QueryType, method: RetrievalMethod) -> Self {
        Self {
            query_type,
            method,
            hit_count: 0,
            miss_count: 0,
            avg_score: 0.0,
        }
    }

    /// 总查询次数（命中 + 未命中）。
    pub fn total(&self) -> u32 {
        self.hit_count + self.miss_count
    }

    /// 命中率（0.0-1.0）。
    ///
    /// 总查询为 0 时返回 0.0（冷启动）。
    pub fn hit_rate(&self) -> f32 {
        if self.total() == 0 {
            0.0
        } else {
            self.hit_count as f32 / self.total() as f32
        }
    }

    /// 记录一次检索效果。
    ///
    /// - `is_hit`: 检索结果是否有效（score > 阈值且非空）
    /// - `score`: 检索结果 top-1 相关度分数（0.0-1.0）
    ///
    /// 更新逻辑：
    /// - hit_count 或 miss_count +1
    /// - avg_score 滚动更新：`new_avg = (old_avg * (n-1) + score) / n`（仅 hit 时更新 score）
    pub fn record(&mut self, is_hit: bool, score: f32) {
        if is_hit {
            let old_total = self.hit_count;
            self.hit_count += 1;
            // 滚动均值：仅 hit 时更新 avg_score
            if old_total == 0 {
                self.avg_score = score;
            } else {
                let n = self.hit_count as f32;
                self.avg_score = (self.avg_score * (n - 1.0) + score) / n;
            }
        } else {
            self.miss_count += 1;
        }
    }
}

// ============================================================
// 自进化检索记忆引擎
// ============================================================

/// 检索效果判定阈值：检索结果 top-1 score > 此值视为命中。
const HIT_SCORE_THRESHOLD: f32 = 0.1;

/// 自进化检索记忆引擎。
///
/// 负责记录检索效果和自适应选择最佳检索方法。
/// 记忆数据通过 `RetrievalMemoryStore` trait 持久化（生产环境为 SQLite）。
///
/// # 线程安全
///
/// 引擎本身无内部可变状态（所有状态由 Storage 持久化），
/// 可安全跨线程共享。
pub struct RetrievalMemoryEngine<S: RetrievalMemoryStore> {
    /// 记忆存储后端
    pub(crate) store: S,
}

impl<S: RetrievalMemoryStore> RetrievalMemoryEngine<S> {
    /// 创建记忆引擎实例。
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// 记录一次检索效果。
    ///
    /// # 参数
    /// - `query`: 用户查询文本（自动分类查询类型）
    /// - `method`: 使用的检索方法
    /// - `results`: 检索结果列表
    ///
    /// # 逻辑
    /// 1. 自动分类查询类型
    /// 2. 判定是否命中（结果非空且 top-1 score > 阈值）
    /// 3. 读取历史记录 → record() 更新 → 写回存储
    pub async fn record_retrieval(
        &self,
        query: &str,
        method: RetrievalMethod,
        results: &[echomind_models::RetrievalResult],
    ) -> anyhow::Result<()> {
        let query_type = classify_query_type(query);

        // 判定命中
        let (is_hit, score) = if results.is_empty() {
            (false, 0.0)
        } else {
            let top_score = results.iter().map(|r| r.score).fold(f32::MIN, f32::max);
            (top_score > HIT_SCORE_THRESHOLD, top_score)
        };

        // 读取现有记录
        let mut record = self
            .store
            .get_memory(query_type, method)
            .await?
            .unwrap_or_else(|| MemoryRecord::new(query_type, method));

        // 更新记录
        record.record(is_hit, score);

        // 写回存储
        self.store.upsert_memory(&record).await?;

        Ok(())
    }

    /// 自适应选择最佳检索方法。
    ///
    /// # 参数
    /// - `query`: 用户查询文本（自动分类查询类型）
    ///
    /// # 返回
    /// 最佳检索方法。
    ///
    /// # 选择策略
    /// 1. 分类查询类型
    /// 2. 读取该类型所有方法的历史记录
    /// 3. 无历史记录 → 返回默认方法（冷启动）
    /// 4. 有历史记录 → 选择 hit_rate 最高的方法
    /// 5. hit_rate 相同 → 选择 avg_score 最高的方法
    /// 6. 所有方法 total=0 → 返回默认方法
    ///
    /// # 延迟
    /// 记忆查表 ~1ms（SQLite 单类型 5 行 SELECT），不增加显著延迟。
    pub async fn select_method(&self, query: &str) -> anyhow::Result<RetrievalMethod> {
        let query_type = classify_query_type(query);

        // 读取该查询类型的所有方法记录
        let records = self.store.list_memories(query_type).await?;

        // 冷启动：无历史记录
        if records.is_empty() {
            return Ok(RetrievalMethod::default_for(query_type));
        }

        // 所有方法都无数据（total=0）→ 默认方法
        if records.iter().all(|r| r.total() == 0) {
            return Ok(RetrievalMethod::default_for(query_type));
        }

        // 选择 hit_rate 最高的方法，hit_rate 相同时选 avg_score 最高的
        let best = records
            .into_iter()
            .filter(|r| r.total() > 0) // 过滤掉无数据的
            .max_by(|a, b| {
                // 先比较 hit_rate：返回 Greater 表示 a 优于 b
                let rate_cmp = a.hit_rate().partial_cmp(&b.hit_rate());
                match rate_cmp {
                    Some(std::cmp::Ordering::Equal) | None => {
                        // hit_rate 相同 → 比较 avg_score
                        a.avg_score
                            .partial_cmp(&b.avg_score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }
                    Some(ord) => ord,
                }
            });

        match best {
            Some(record) => Ok(record.method),
            None => Ok(RetrievalMethod::default_for(query_type)),
        }
    }

    /// 记录用户反馈信号，调整检索方法的 hit_rate 权重。
    ///
    /// # 参数
    /// - `signal`: 用户反馈信号（查询文本 + 检索方法 + 反馈类型）
    ///
    /// # 逻辑
    /// 1. 自动分类查询类型
    /// 2. 读取对应 (query_type, method) 的 MemoryRecord
    /// 3. 根据 FeedbackType.delta() 调整 hit_count / miss_count：
    ///    - 正信号：hit_count += 1（模拟一次"隐性命中"）
    ///    - 负信号：miss_count += 1（模拟一次"隐性未命中"）
    ///    - delta 同时影响 avg_score（正信号微升、负信号微降）
    /// 4. 写回存储
    ///
    /// # 边界保护
    /// - avg_score 不超过 1.0（正信号时 clamp）
    /// - avg_score 不低于 0.0（负信号时 clamp）
    /// - hit_rate 天然由 hit_count / total 限制在 [0, 1]
    pub async fn record_feedback(&self, signal: FeedbackSignal) -> anyhow::Result<()> {
        let query_type = classify_query_type(&signal.query);
        let delta = signal.feedback.delta();

        let mut record = self
            .store
            .get_memory(query_type, signal.method)
            .await?
            .unwrap_or_else(|| MemoryRecord::new(query_type, signal.method));

        if delta > 0.0 {
            // 正信号：增加 hit_count + 微升 avg_score
            record.hit_count += 1;
            record.avg_score = (record.avg_score + delta).min(1.0);
        } else {
            // 负信号：增加 miss_count + 微降 avg_score
            record.miss_count += 1;
            record.avg_score = (record.avg_score + delta).max(0.0);
        }

        self.store.upsert_memory(&record).await?;
        Ok(())
    }

    /// 手动重置所有记忆数据。
    pub async fn reset_all(&self) -> anyhow::Result<()> {
        self.store.clear_all_memories().await
    }

    /// 获取记忆统计信息。
    ///
    /// 返回所有查询类型 × 所有方法的记录列表。
    pub async fn get_stats(&self) -> anyhow::Result<Vec<MemoryRecord>> {
        self.store.list_all_memories().await
    }
}

// ============================================================
// 检索记忆存储端口
// ============================================================

/// 检索记忆存储端口：持久化检索效果记录。
///
/// 生产实现为 `SqliteStorage`（`retrieval_memory` 表），
/// 测试中可用内存 Mock 实现。
pub trait RetrievalMemoryStore: Send + Sync {
    /// 读取指定查询类型 + 方法的记忆记录。
    ///
    /// # 返回
    /// `Some(MemoryRecord)` 表示有历史记录，`None` 表示冷启动（无数据）。
    async fn get_memory(
        &self,
        query_type: QueryType,
        method: RetrievalMethod,
    ) -> anyhow::Result<Option<MemoryRecord>>;

    /// 写入/更新记忆记录（upsert 语义）。
    ///
    /// 以 `(query_type, method)` 为主键，存在则更新，不存在则插入。
    async fn upsert_memory(&self, record: &MemoryRecord) -> anyhow::Result<()>;

    /// 列出指定查询类型的所有方法记忆记录。
    async fn list_memories(&self, query_type: QueryType) -> anyhow::Result<Vec<MemoryRecord>>;

    /// 列出所有查询类型的所有方法记忆记录。
    async fn list_all_memories(&self) -> anyhow::Result<Vec<MemoryRecord>>;

    /// 清空所有记忆数据（手动重置）。
    async fn clear_all_memories(&self) -> anyhow::Result<()>;
}

// ============================================================
// 内存 Mock 存储（测试用）
// ============================================================

/// 内存 Mock 检索记忆存储（测试用）。
///
/// 使用 `Vec<MemoryRecord>` 存储，无持久化。
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
pub struct InMemoryMemoryStore {
    records: std::sync::Mutex<Vec<MemoryRecord>>,
}

#[cfg(test)]
impl InMemoryMemoryStore {
    /// 创建空的内存存储。
    pub fn new() -> Self {
        Self {
            records: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl Default for InMemoryMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
impl RetrievalMemoryStore for InMemoryMemoryStore {
    async fn get_memory(
        &self,
        query_type: QueryType,
        method: RetrievalMethod,
    ) -> anyhow::Result<Option<MemoryRecord>> {
        let records = self.records.lock().unwrap();
        Ok(records
            .iter()
            .find(|r| r.query_type == query_type && r.method == method)
            .cloned())
    }

    async fn upsert_memory(&self, record: &MemoryRecord) -> anyhow::Result<()> {
        let mut records = self.records.lock().unwrap();
        if let Some(existing) = records
            .iter_mut()
            .find(|r| r.query_type == record.query_type && r.method == record.method)
        {
            *existing = record.clone();
        } else {
            records.push(record.clone());
        }
        Ok(())
    }

    async fn list_memories(&self, query_type: QueryType) -> anyhow::Result<Vec<MemoryRecord>> {
        let records = self.records.lock().unwrap();
        Ok(records
            .iter()
            .filter(|r| r.query_type == query_type)
            .cloned()
            .collect())
    }

    async fn list_all_memories(&self) -> anyhow::Result<Vec<MemoryRecord>> {
        let records = self.records.lock().unwrap();
        Ok(records.clone())
    }

    async fn clear_all_memories(&self) -> anyhow::Result<()> {
        let mut records = self.records.lock().unwrap();
        records.clear();
        Ok(())
    }
}

// ============================================================
// 用户反馈信号（隐式采集，零显式标注）
// ============================================================

/// 用户反馈信号类型（借鉴 StoryMoss 持续学习）。
///
/// 通过观察用户行为自动蒸馏检索效果，无需显式标注：
/// - 用户重新提问（相同/相似查询）→ 负信号（上一次回答不满意）
/// - 用户编辑后重发 → 负信号（上一次回答不满意）
/// - 用户接受回答（继续新话题）→ 正信号（上一次回答满意）
/// - 用户点赞 → 强正信号
/// - 用户点踩 → 强负信号
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    /// 用户重新提问（相同/相似查询）→ 负信号
    RetryWithDifferentMethod,
    /// 用户编辑后重发 → 负信号
    EditAndResend,
    /// 用户接受回答（继续新话题）→ 正信号
    Accepted,
    /// 用户点赞 → 强正信号
    ThumbsUp,
    /// 用户点踩 → 强负信号
    ThumbsDown,
}

impl FeedbackType {
    /// 返回该反馈类型对 hit_rate 的调整幅度。
    ///
    /// 正信号提升 hit_rate，负信号降低。
    /// 强信号幅度加倍。
    pub fn delta(&self) -> f32 {
        match self {
            Self::RetryWithDifferentMethod => -0.10,
            Self::EditAndResend => -0.15,
            Self::Accepted => 0.05,
            Self::ThumbsUp => 0.20,
            Self::ThumbsDown => -0.25,
        }
    }

    /// 是否为正信号。
    pub fn is_positive(&self) -> bool {
        self.delta() > 0.0
    }
}

/// 用户反馈信号记录。
///
/// 前端检测到用户行为后构造此信号并上报。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackSignal {
    /// 用户查询文本
    pub query: String,
    /// 使用的检索方法
    pub method: RetrievalMethod,
    /// 反馈类型
    pub feedback: FeedbackType,
    /// 时间戳（Unix 秒）
    pub timestamp: i64,
}

impl FeedbackSignal {
    /// 创建新的反馈信号，自动填充当前时间戳。
    pub fn new(query: String, method: RetrievalMethod, feedback: FeedbackType) -> Self {
        Self {
            query,
            method,
            feedback,
            timestamp: chrono::Utc::now().timestamp(),
        }
    }
}
