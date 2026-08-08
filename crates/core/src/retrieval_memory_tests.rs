#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 自进化检索记忆 TDD 测试（TC-MEM-001~006）。
//!
//! 测试覆盖：
//! - TC-MEM-001: 记录 (query_type, method, hit_rate) 三元组
//! - TC-MEM-002: 查询类型分类（事实型/分析型/代码型/对话型）
//! - TC-MEM-003: 根据历史效果选择最佳检索方法
//! - TC-MEM-004: 冷启动使用默认方法
//! - TC-MEM-005: 效果记录持久化（内存 Mock 模拟 SQLite）
//! - TC-MEM-006: 可手动重置记忆

use echomind_models::{Chunk, RetrievalResult};

use crate::retrieval_memory::{
    InMemoryMemoryStore, MemoryRecord, QueryType, RetrievalMemoryEngine, RetrievalMemoryStore,
    RetrievalMethod, classify_query_type,
};

// ============================================================
// 辅助函数
// ============================================================

/// 构造 N 条检索结果（按 score 降序）。
fn make_results(n: usize, base_score: f32) -> Vec<RetrievalResult> {
    (0..n)
        .map(|i| RetrievalResult {
            chunk: Chunk::new(
                format!("doc-{i}"),
                format!("这是第 {i} 条文档片段。"),
                10,
                i,
            ),
            score: base_score - i as f32 * 0.05,
            doc_name: format!("doc_{i}.md"),
        })
        .collect()
}

/// 构造空检索结果。
fn empty_results() -> Vec<RetrievalResult> {
    vec![]
}

// ============================================================
// TC-MEM-001: 记录 (query_type, method, hit_rate) 三元组
// ============================================================

/// TC-MEM-001：记录检索效果后，能正确读取 (query_type, method, hit_rate) 三元组。
///
/// 验证：
/// 1. 调用 record_retrieval 后，存储中存在对应记录
/// 2. hit_count / miss_count 正确累加
/// 3. avg_score 滚动更新正确
#[tokio::test]
async fn tc_mem_001_record_query_type_method_hit_rate() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    // 记录一次命中
    let query = "什么是 Rust 语言？"; // Factual
    let results = make_results(3, 0.85);
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &results)
        .await
        .unwrap();

    // 读取记录
    let record = engine
        .store
        .get_memory(QueryType::Factual, RetrievalMethod::VectorOnly)
        .await
        .unwrap()
        .expect("记录应存在");

    assert_eq!(record.query_type, QueryType::Factual);
    assert_eq!(record.method, RetrievalMethod::VectorOnly);
    assert_eq!(record.hit_count, 1);
    assert_eq!(record.miss_count, 0);
    assert!(
        (record.avg_score - 0.85).abs() < 0.01,
        "avg_score 应为 0.85，实际 {}",
        record.avg_score
    );
    assert!((record.hit_rate() - 1.0).abs() < 0.01, "hit_rate 应为 1.0");

    // 再记录一次未命中
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &empty_results())
        .await
        .unwrap();

    let record = engine
        .store
        .get_memory(QueryType::Factual, RetrievalMethod::VectorOnly)
        .await
        .unwrap()
        .expect("记录应存在");

    assert_eq!(record.hit_count, 1);
    assert_eq!(record.miss_count, 1);
    assert!((record.hit_rate() - 0.5).abs() < 0.01, "hit_rate 应为 0.5");
}

// ============================================================
// TC-MEM-002: 查询类型分类
// ============================================================

/// TC-MEM-002：查询类型分类器正确分类四种类型。
#[test]
fn tc_mem_002_query_type_classification() {
    // 事实型
    assert_eq!(
        classify_query_type("什么是 Rust 语言？"),
        QueryType::Factual,
        "「什么是」应分类为 Factual"
    );
    assert_eq!(
        classify_query_type("What is the definition of RAG?"),
        QueryType::Factual,
        "What is/definition 应分类为 Factual"
    );
    assert_eq!(
        classify_query_type("HTTP 的全称是什么？"),
        QueryType::Factual,
        "「全称」应分类为 Factual"
    );

    // 分析型
    assert_eq!(
        classify_query_type("比较 Rust 和 Go 的优缺点"),
        QueryType::Analytical,
        "「比较/优缺点」应分类为 Analytical"
    );
    assert_eq!(
        classify_query_type("Compare REST and GraphQL approaches"),
        QueryType::Analytical,
        "Compare 应分类为 Analytical"
    );
    assert_eq!(
        classify_query_type("分析两种架构的 tradeoff"),
        QueryType::Analytical,
        "「分析」+ tradeoff 应分类为 Analytical"
    );

    // 代码型
    assert_eq!(
        classify_query_type("这个函数的 API 接口怎么调用？"),
        QueryType::Code,
        "「函数/API/接口」应分类为 Code"
    );
    assert_eq!(
        classify_query_type("How to implement async error handling?"),
        QueryType::Code,
        "implement/async/error 应分类为 Code"
    );
    assert_eq!(
        classify_query_type("编译报错 undefined reference"),
        QueryType::Code,
        "「编译/报错」应分类为 Code"
    );

    // 对话型
    assert_eq!(
        classify_query_type("继续说说这个"),
        QueryType::Conversational,
        "「继续」+ 短查询应分类为 Conversational"
    );
    assert_eq!(
        classify_query_type("它怎么样？"),
        QueryType::Conversational,
        "「它」应分类为 Conversational"
    );

    // 默认（无明显特征的长查询）
    assert_eq!(
        classify_query_type("我想了解一些关于这个主题的更多细节"),
        QueryType::Conversational,
        "无明显特征的查询应默认为 Conversational"
    );
}

// ============================================================
// TC-MEM-003: 根据历史效果选择最佳检索方法
// ============================================================

/// TC-MEM-003：有历史数据时，选择 hit_rate 最高的方法。
#[tokio::test]
async fn tc_mem_003_select_best_method_by_hit_rate() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    // 为 Analytical 类型记录三种方法的效果
    let query = "比较 Rust 和 Go 的优缺点"; // Analytical

    // VectorOnly: 1 hit / 3 total = 33% hit rate
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &make_results(1, 0.5))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &empty_results())
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &empty_results())
        .await
        .unwrap();

    // Hybrid: 2 hit / 3 total = 67% hit rate
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &make_results(2, 0.7))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &make_results(1, 0.6))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &empty_results())
        .await
        .unwrap();

    // HybridRerank: 3 hit / 3 total = 100% hit rate
    engine
        .record_retrieval(query, RetrievalMethod::HybridRerank, &make_results(3, 0.9))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::HybridRerank, &make_results(2, 0.85))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::HybridRerank, &make_results(1, 0.8))
        .await
        .unwrap();

    // 选择 → 应选 HybridRerank（100% hit rate）
    let selected = engine.select_method(query).await.unwrap();
    assert_eq!(
        selected,
        RetrievalMethod::HybridRerank,
        "应选择 hit_rate 最高的 HybridRerank"
    );
}

/// TC-MEM-003b：hit_rate 相同时，选择 avg_score 最高的方法。
#[tokio::test]
async fn tc_mem_003b_tie_break_by_avg_score() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是机器学习？"; // Factual

    // VectorOnly: 2/2 = 100%, avg_score = 0.60
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &make_results(1, 0.60))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &make_results(1, 0.60))
        .await
        .unwrap();

    // Hybrid: 2/2 = 100%, avg_score = 0.90
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &make_results(1, 0.90))
        .await
        .unwrap();
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &make_results(1, 0.90))
        .await
        .unwrap();

    // 选择 → hit_rate 相同（100%），应选 avg_score 更高的 Hybrid
    let selected = engine.select_method(query).await.unwrap();
    assert_eq!(
        selected,
        RetrievalMethod::Hybrid,
        "hit_rate 相同时应选 avg_score 更高的 Hybrid"
    );
}

// ============================================================
// TC-MEM-004: 冷启动使用默认方法
// ============================================================

/// TC-MEM-004：无历史数据时（冷启动），返回默认方法。
#[tokio::test]
async fn tc_mem_004_cold_start_uses_default_method() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    // Factual → 默认 VectorOnly
    let method = engine.select_method("什么是 Rust 语言？").await.unwrap();
    assert_eq!(
        method,
        RetrievalMethod::VectorOnly,
        "Factual 冷启动应使用默认方法 VectorOnly"
    );

    // Analytical → 默认 HybridRerank
    let method = engine.select_method("比较两种架构的优缺点").await.unwrap();
    assert_eq!(
        method,
        RetrievalMethod::HybridRerank,
        "Analytical 冷启动应使用默认方法 HybridRerank"
    );

    // Code → 默认 Hybrid
    let method = engine
        .select_method("这个函数的 API 怎么调用？")
        .await
        .unwrap();
    assert_eq!(
        method,
        RetrievalMethod::Hybrid,
        "Code 冷启动应使用默认方法 Hybrid"
    );

    // Conversational → 默认 Hybrid
    let method = engine.select_method("继续说说").await.unwrap();
    assert_eq!(
        method,
        RetrievalMethod::Hybrid,
        "Conversational 冷启动应使用默认方法 Hybrid"
    );
}

// ============================================================
// TC-MEM-005: 效果记录持久化
// ============================================================

/// TC-MEM-005：效果记录持久化（内存 Mock 模拟 SQLite）。
///
/// 验证：
/// 1. 记录写入后可重新读取
/// 2. 多次记录后统计数据正确
/// 3. upsert 语义（同主键更新而非插入）
#[tokio::test]
async fn tc_mem_005_persistence() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual
    let method = RetrievalMethod::VectorOnly;

    // 第一次记录（命中）
    engine
        .record_retrieval(query, method, &make_results(3, 0.85))
        .await
        .unwrap();

    // 验证持久化：读取应返回正确数据
    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("第一次记录后应存在");
    assert_eq!(record.hit_count, 1);
    assert_eq!(record.miss_count, 0);

    // 第二次记录（命中，更高 score）
    engine
        .record_retrieval(query, method, &make_results(2, 0.95))
        .await
        .unwrap();

    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("第二次记录后应存在");
    assert_eq!(record.hit_count, 2, "hit_count 应累加到 2");
    assert_eq!(record.miss_count, 0);
    // avg_score 应为 (0.85 + 0.95) / 2 = 0.90
    assert!(
        (record.avg_score - 0.90).abs() < 0.01,
        "avg_score 应为 0.90，实际 {}",
        record.avg_score
    );

    // 第三次记录（未命中）
    engine
        .record_retrieval(query, method, &empty_results())
        .await
        .unwrap();

    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("第三次记录后应存在");
    assert_eq!(record.hit_count, 2, "hit_count 不应变");
    assert_eq!(record.miss_count, 1, "miss_count 应为 1");
    // avg_score 不应因 miss 而改变
    assert!(
        (record.avg_score - 0.90).abs() < 0.01,
        "miss 不应影响 avg_score"
    );

    // 验证 upsert 语义：不应产生重复行
    let all = engine.store.list_all_memories().await.unwrap();
    let count = all
        .iter()
        .filter(|r| r.query_type == QueryType::Factual && r.method == method)
        .count();
    assert_eq!(count, 1, "同一 (query_type, method) 应只有一行（upsert）");
}

// ============================================================
// TC-MEM-006: 可手动重置记忆
// ============================================================

/// TC-MEM-006：手动重置后所有记忆数据清空。
#[tokio::test]
async fn tc_mem_006_manual_reset() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    // 记录一些数据
    engine
        .record_retrieval(
            "什么是 Rust？",
            RetrievalMethod::VectorOnly,
            &make_results(2, 0.8),
        )
        .await
        .unwrap();
    engine
        .record_retrieval(
            "比较两种方案",
            RetrievalMethod::HybridRerank,
            &make_results(3, 0.9),
        )
        .await
        .unwrap();

    // 验证数据存在
    let stats = engine.get_stats().await.unwrap();
    assert!(!stats.is_empty(), "重置前应有数据");

    // 重置
    engine.reset_all().await.unwrap();

    // 验证数据已清空
    let stats = engine.get_stats().await.unwrap();
    assert!(stats.is_empty(), "重置后应无数据");

    // 重置后冷启动 → 默认方法
    let method = engine.select_method("什么是 Rust？").await.unwrap();
    assert_eq!(
        method,
        RetrievalMethod::VectorOnly,
        "重置后应回到冷启动默认方法"
    );
}

// ============================================================
// 补充测试：MemoryRecord 单元测试
// ============================================================

/// MemoryRecord::record() 正确更新 hit_count / miss_count / avg_score。
#[test]
fn tc_mem_record_updates_correctly() {
    let mut record = MemoryRecord::new(QueryType::Code, RetrievalMethod::Hybrid);

    // 初始状态
    assert_eq!(record.hit_count, 0);
    assert_eq!(record.miss_count, 0);
    assert!(record.avg_score.abs() < 0.01);
    assert!(record.hit_rate().abs() < 0.01);

    // 第一次命中
    record.record(true, 0.8);
    assert_eq!(record.hit_count, 1);
    assert_eq!(record.miss_count, 0);
    assert!((record.avg_score - 0.8).abs() < 0.01);
    assert!((record.hit_rate() - 1.0).abs() < 0.01);

    // 第二次命中（更高 score）
    record.record(true, 0.9);
    assert_eq!(record.hit_count, 2);
    assert!(
        (record.avg_score - 0.85).abs() < 0.01,
        "avg 应为 (0.8+0.9)/2=0.85"
    );

    // 第一次未命中
    record.record(false, 0.0);
    assert_eq!(record.hit_count, 2);
    assert_eq!(record.miss_count, 1);
    assert!(
        (record.avg_score - 0.85).abs() < 0.01,
        "miss 不应影响 avg_score"
    );
    assert!(
        (record.hit_rate() - (2.0 / 3.0)).abs() < 0.01,
        "hit_rate 应为 2/3"
    );
}

/// QueryType 和 RetrievalMethod 的 as_str/from_str 往返转换。
#[test]
fn tc_mem_enum_roundtrip() {
    // QueryType
    for qt in QueryType::all() {
        let s = qt.as_str();
        assert_eq!(QueryType::parse_str(s), Some(qt), "QueryType 往返失败: {s}");
        assert_eq!(
            QueryType::parse_str(&s.to_uppercase()),
            Some(qt),
            "QueryType 大小写不敏感失败: {}",
            s.to_uppercase()
        );
    }
    assert_eq!(QueryType::parse_str("unknown"), None);

    // RetrievalMethod
    for m in RetrievalMethod::all() {
        let s = m.as_str();
        assert_eq!(
            RetrievalMethod::parse_str(s),
            Some(m),
            "RetrievalMethod 往返失败: {s}"
        );
    }
    assert_eq!(RetrievalMethod::parse_str("unknown"), None);
}

/// default_for 为每种查询类型返回合理的默认方法。
#[test]
fn tc_mem_default_for_each_type() {
    assert_eq!(
        RetrievalMethod::default_for(QueryType::Factual),
        RetrievalMethod::VectorOnly
    );
    assert_eq!(
        RetrievalMethod::default_for(QueryType::Analytical),
        RetrievalMethod::HybridRerank
    );
    assert_eq!(
        RetrievalMethod::default_for(QueryType::Code),
        RetrievalMethod::Hybrid
    );
    assert_eq!(
        RetrievalMethod::default_for(QueryType::Conversational),
        RetrievalMethod::Hybrid
    );
}

// ============================================================
// 用户反馈信号测试（TC-FEEDBACK-001~007）
// ============================================================

use crate::retrieval_memory::{FeedbackSignal, FeedbackType};

/// TC-FEEDBACK-001：正信号（Accepted）提高 hit_rate。
///
/// 验证：
/// 1. 先 record_retrieval(hit, 0.5) → hit_rate = 1.0
/// 2. 再 record_feedback(Accepted) → hit_count += 1, hit_rate 仍 = 1.0（上限）
/// 3. hit_count 增加了 1
#[tokio::test]
async fn tc_feedback_001_positive_signal_increases_hit_count() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual
    let method = RetrievalMethod::VectorOnly;

    // 记录一次命中
    engine
        .record_retrieval(query, method, &make_results(3, 0.5))
        .await
        .unwrap();

    // 验证初始状态
    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("记录应存在");
    assert_eq!(record.hit_count, 1);
    assert!((record.hit_rate() - 1.0).abs() < 0.01);

    // 记录正反馈
    let signal = FeedbackSignal::new(query.to_string(), method, FeedbackType::Accepted);
    engine.record_feedback(signal).await.unwrap();

    // 验证 hit_count 增加了 1，hit_rate 仍为 1.0（上限）
    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("记录应存在");
    assert_eq!(record.hit_count, 2, "正信号应使 hit_count +1");
    assert_eq!(record.miss_count, 0);
    assert!(
        (record.hit_rate() - 1.0).abs() < 0.01,
        "hit_rate 应为 1.0（上限）"
    );
    // avg_score 应微升（0.5 + 0.05 = 0.55）
    assert!(
        (record.avg_score - 0.55).abs() < 0.01,
        "avg_score 应为 0.55，实际 {}",
        record.avg_score
    );
}

/// TC-FEEDBACK-002：负信号（EditAndResend）降低 hit_rate。
///
/// 验证：
/// 1. 先 record_retrieval(hit, 0.5) → hit_rate = 1.0
/// 2. 再 record_feedback(EditAndResend) → miss_count += 1
/// 3. hit_rate = 1/2 = 0.5
#[tokio::test]
async fn tc_feedback_002_negative_signal_decreases_hit_rate() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual
    let method = RetrievalMethod::VectorOnly;

    // 记录一次命中
    engine
        .record_retrieval(query, method, &make_results(3, 0.5))
        .await
        .unwrap();

    // 记录负反馈
    let signal = FeedbackSignal::new(query.to_string(), method, FeedbackType::EditAndResend);
    engine.record_feedback(signal).await.unwrap();

    // 验证 miss_count += 1, hit_rate = 1/2 = 0.5
    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("记录应存在");
    assert_eq!(record.hit_count, 1, "hit_count 不应变");
    assert_eq!(record.miss_count, 1, "负信号应使 miss_count +1");
    assert!((record.hit_rate() - 0.5).abs() < 0.01, "hit_rate 应为 0.5");
    // avg_score 应微降（0.5 - 0.15 = 0.35）
    assert!(
        (record.avg_score - 0.35).abs() < 0.01,
        "avg_score 应为 0.35，实际 {}",
        record.avg_score
    );
}

/// TC-FEEDBACK-003：强正信号（ThumbsUp）幅度大于普通正信号（Accepted）。
///
/// 验证：
/// 1. 两条方法都初始 record_retrieval(hit, 0.5)
/// 2. 方法 A 收到 Accepted（delta=0.05），方法 B 收到 ThumbsUp（delta=0.20）
/// 3. B 的 avg_score > A 的 avg_score
#[tokio::test]
async fn tc_feedback_003_thumbs_up_stronger_than_accepted() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual

    // 方法 A: VectorOnly + Accepted
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &make_results(1, 0.5))
        .await
        .unwrap();
    engine
        .record_feedback(FeedbackSignal::new(
            query.to_string(),
            RetrievalMethod::VectorOnly,
            FeedbackType::Accepted,
        ))
        .await
        .unwrap();

    // 方法 B: Hybrid + ThumbsUp
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &make_results(1, 0.5))
        .await
        .unwrap();
    engine
        .record_feedback(FeedbackSignal::new(
            query.to_string(),
            RetrievalMethod::Hybrid,
            FeedbackType::ThumbsUp,
        ))
        .await
        .unwrap();

    let record_a = engine
        .store
        .get_memory(QueryType::Factual, RetrievalMethod::VectorOnly)
        .await
        .unwrap()
        .expect("记录 A 应存在");
    let record_b = engine
        .store
        .get_memory(QueryType::Factual, RetrievalMethod::Hybrid)
        .await
        .unwrap()
        .expect("记录 B 应存在");

    // B 的 avg_score (0.5 + 0.20 = 0.70) > A 的 avg_score (0.5 + 0.05 = 0.55)
    assert!(
        record_b.avg_score > record_a.avg_score,
        "ThumbsUp (avg={}) 应比 Accepted (avg={}) 更高",
        record_b.avg_score,
        record_a.avg_score
    );
}

/// TC-FEEDBACK-004：强负信号（ThumbsDown）幅度大于普通负信号（EditAndResend）。
///
/// 验证：
/// 1. 两条方法都初始 record_retrieval(hit, 0.5)
/// 2. 方法 A 收到 EditAndResend（delta=-0.15），方法 B 收到 ThumbsDown（delta=-0.25）
/// 3. B 的 avg_score 降幅更大
#[tokio::test]
async fn tc_feedback_004_thumbs_down_stronger_than_edit_resend() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual

    // 方法 A: VectorOnly + EditAndResend
    engine
        .record_retrieval(query, RetrievalMethod::VectorOnly, &make_results(1, 0.5))
        .await
        .unwrap();
    engine
        .record_feedback(FeedbackSignal::new(
            query.to_string(),
            RetrievalMethod::VectorOnly,
            FeedbackType::EditAndResend,
        ))
        .await
        .unwrap();

    // 方法 B: Hybrid + ThumbsDown
    engine
        .record_retrieval(query, RetrievalMethod::Hybrid, &make_results(1, 0.5))
        .await
        .unwrap();
    engine
        .record_feedback(FeedbackSignal::new(
            query.to_string(),
            RetrievalMethod::Hybrid,
            FeedbackType::ThumbsDown,
        ))
        .await
        .unwrap();

    let record_a = engine
        .store
        .get_memory(QueryType::Factual, RetrievalMethod::VectorOnly)
        .await
        .unwrap()
        .expect("记录 A 应存在");
    let record_b = engine
        .store
        .get_memory(QueryType::Factual, RetrievalMethod::Hybrid)
        .await
        .unwrap()
        .expect("记录 B 应存在");

    // A 的 avg_score = 0.5 - 0.15 = 0.35
    // B 的 avg_score = 0.5 - 0.25 = 0.25
    assert!(
        record_b.avg_score < record_a.avg_score,
        "ThumbsDown (avg={}) 应比 EditAndResend (avg={}) 降幅更大",
        record_b.avg_score,
        record_a.avg_score
    );
}

/// TC-FEEDBACK-005：FeedbackType::delta() 返回值正确。
#[test]
fn tc_feedback_005_delta_values_correct() {
    assert!(
        (FeedbackType::RetryWithDifferentMethod.delta() - (-0.10)).abs() < 0.001,
        "RetryWithDifferentMethod delta 应为 -0.10"
    );
    assert!(
        (FeedbackType::EditAndResend.delta() - (-0.15)).abs() < 0.001,
        "EditAndResend delta 应为 -0.15"
    );
    assert!(
        (FeedbackType::Accepted.delta() - 0.05).abs() < 0.001,
        "Accepted delta 应为 0.05"
    );
    assert!(
        (FeedbackType::ThumbsUp.delta() - 0.20).abs() < 0.001,
        "ThumbsUp delta 应为 0.20"
    );
    assert!(
        (FeedbackType::ThumbsDown.delta() - (-0.25)).abs() < 0.001,
        "ThumbsDown delta 应为 -0.25"
    );

    // is_positive 测试
    assert!(!FeedbackType::RetryWithDifferentMethod.is_positive());
    assert!(!FeedbackType::EditAndResend.is_positive());
    assert!(FeedbackType::Accepted.is_positive());
    assert!(FeedbackType::ThumbsUp.is_positive());
    assert!(!FeedbackType::ThumbsDown.is_positive());
}

/// TC-FEEDBACK-006：avg_score 不超过 1.0（clamp 上限）。
///
/// 初始 avg_score = 0.95 → 收到 ThumbsUp(delta=0.20) → avg_score = 1.0（clamped）
#[tokio::test]
async fn tc_feedback_006_avg_score_clamped_to_max() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual
    let method = RetrievalMethod::VectorOnly;

    // 记录一次高分命中
    engine
        .record_retrieval(query, method, &make_results(1, 0.95))
        .await
        .unwrap();

    // 记录强正反馈（delta=0.20）
    engine
        .record_feedback(FeedbackSignal::new(
            query.to_string(),
            method,
            FeedbackType::ThumbsUp,
        ))
        .await
        .unwrap();

    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("记录应存在");
    // 0.95 + 0.20 = 1.15 → clamp 到 1.0
    assert!(
        (record.avg_score - 1.0).abs() < 0.001,
        "avg_score 应 clamp 到 1.0，实际 {}",
        record.avg_score
    );
}

/// TC-FEEDBACK-007：avg_score 不低于 0.0（clamp 下限）。
///
/// 初始 avg_score = 0.15 → 收到 ThumbsDown(delta=-0.25) → avg_score = 0.0（clamped）
#[tokio::test]
async fn tc_feedback_007_avg_score_clamped_to_min() {
    let store = InMemoryMemoryStore::new();
    let engine = RetrievalMemoryEngine::new(store);

    let query = "什么是 Rust 语言？"; // Factual
    let method = RetrievalMethod::VectorOnly;

    // 记录一次低分命中（score > 0.1 阈值才算命中）
    engine
        .record_retrieval(query, method, &make_results(1, 0.15))
        .await
        .unwrap();

    // 记录强负反馈（delta=-0.25）
    engine
        .record_feedback(FeedbackSignal::new(
            query.to_string(),
            method,
            FeedbackType::ThumbsDown,
        ))
        .await
        .unwrap();

    let record = engine
        .store
        .get_memory(QueryType::Factual, method)
        .await
        .unwrap()
        .expect("记录应存在");
    // 0.15 - 0.25 = -0.10 → clamp 到 0.0
    assert!(
        (record.avg_score - 0.0).abs() < 0.001,
        "avg_score 应 clamp 到 0.0，实际 {}",
        record.avg_score
    );
    // miss_count 应 +1（初始 hit_count=1, miss_count=0 → 负反馈后 miss_count=1）
    assert_eq!(record.miss_count, 1, "负信号应使 miss_count +1");
}
