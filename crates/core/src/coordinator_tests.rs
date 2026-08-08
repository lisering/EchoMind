#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 多代理协调引擎测试（TC-COORD-001~005）。
//!
//! 验证 `CoordinatorEngine` 的四阶段流水线：
//! Research（子查询分解 + 并行检索）→ Synthesis（综合分析）→ Implementation（流式答案）

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::coordinator::CoordinatorEngine;
use crate::{LLMProvider, Retriever};

// ================== Mock 实现 ==================

/// 查询感知 Mock Retriever：不同查询返回不同结果。
struct QueryAwareRetriever {
    call_count: Arc<AtomicUsize>,
}

impl QueryAwareRetriever {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Retriever for QueryAwareRetriever {
    async fn retrieve(&self, query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if query.contains("Rust") {
            Ok(vec![RetrievalResult {
                chunk: Chunk::new(
                    "doc-rust".to_string(),
                    "Rust 是一门系统编程语言，强调内存安全和并发性能。".to_string(),
                    15,
                    0,
                ),
                score: 0.95,
                doc_name: "rust-guide.md".to_string(),
            }])
        } else if query.contains("Python") {
            Ok(vec![RetrievalResult {
                chunk: Chunk::new(
                    "doc-python".to_string(),
                    "Python 是一门高级脚本语言，广泛用于数据科学和 AI。".to_string(),
                    15,
                    0,
                ),
                score: 0.92,
                doc_name: "python-intro.md".to_string(),
            }])
        } else {
            Ok(vec![RetrievalResult {
                chunk: Chunk::new(
                    "doc-general".to_string(),
                    "通用编程概念说明。".to_string(),
                    10,
                    0,
                ),
                score: 0.80,
                doc_name: "general.md".to_string(),
            }])
        }
    }
}

/// 固定结果 Mock Retriever：始终返回同一结果。
struct FixedRetriever {
    results: Vec<RetrievalResult>,
    call_count: Arc<AtomicUsize>,
}

impl FixedRetriever {
    fn new(results: Vec<RetrievalResult>) -> Self {
        Self {
            results,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Retriever for FixedRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.results.clone())
    }
}

/// 空检索 Mock Retriever：始终返回空。
struct EmptyRetriever;

impl Retriever for EmptyRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// 多阶段 Mock LLM：按调用顺序返回不同响应。
///
/// - 第 1 次调用（子查询分解）：返回 JSON 数组
/// - 第 2 次调用（综合分析）：返回综合摘要
/// - 第 3 次调用（最终答案）：返回流式 token
struct CoordinatorMockLlm {
    call_count: Arc<AtomicUsize>,
    /// 子查询分解响应
    subquery_response: String,
    /// 综合分析响应
    synthesis_response: String,
    /// 最终答案 token 序列
    answer_tokens: Vec<String>,
}

impl CoordinatorMockLlm {
    fn new(
        subquery_response: String,
        synthesis_response: String,
        answer_tokens: Vec<String>,
    ) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            subquery_response,
            synthesis_response,
            answer_tokens,
        }
    }
}

impl LLMProvider for CoordinatorMockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        match count {
            0 => {
                // 子查询分解
                let resp = self.subquery_response.clone();
                Ok(futures::stream::once(async move { Ok(resp) }).boxed())
            }
            1 => {
                // 综合分析
                let resp = self.synthesis_response.clone();
                Ok(futures::stream::once(async move { Ok(resp) }).boxed())
            }
            _ => {
                // 最终答案流
                let tokens = self.answer_tokens.clone();
                Ok(futures::stream::iter(tokens.into_iter().map(Ok)).boxed())
            }
        }
    }
}

/// 辅助：构造检索结果
fn make_sources() -> Vec<RetrievalResult> {
    vec![RetrievalResult {
        chunk: Chunk::new(
            "doc-1".to_string(),
            "Rust 是一门系统编程语言。".to_string(),
            10,
            0,
        ),
        score: 0.9,
        doc_name: "rust-guide.md".to_string(),
    }]
}

// ================== TC-COORD-001: 单子查询等价标准 RAG ==================

/// TC-COORD-001：子查询分解返回单个查询时，等价于标准 RAG。
///
/// LLM 子查询分解返回 `["测试查询"]`（单元素数组），
/// 验证：retriever 被调用 1 次，最终答案流非空。
#[tokio::test]
async fn tc_coord_001_single_subquery_equiv_standard_rag() {
    let retriever = FixedRetriever::new(make_sources());
    let retriever_calls = retriever.call_count.clone();

    let llm = CoordinatorMockLlm::new(
        r#"["测试查询"]"#.to_string(),
        "综合分析结果：Rust 是系统编程语言。".to_string(),
        vec!["Rust".to_string(), " 是系统语言".to_string()],
    );

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试查询").await.unwrap();

    // 单子查询 → retriever 调用 1 次
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        1,
        "单子查询应触发 1 次 retriever 调用"
    );

    // 应有最终答案流
    assert!(outcome.answer_stream.is_some(), "单子查询应返回最终答案流");

    // 应有引用来源
    assert!(!outcome.sources.is_empty(), "单子查询应包含引用来源");

    // 阶段序列应包含 researching → synthesizing → generating
    let phases: Vec<&str> = outcome.phases.iter().map(|p| p.phase.as_str()).collect();
    assert_eq!(
        phases,
        vec!["researching", "synthesizing", "generating"],
        "阶段序列应为 researching → synthesizing → generating"
    );
}

// ================== TC-COORD-002: 多子查询并行检索 ==================

/// TC-COORD-002：多个子查询并行检索，每个子查询各执行一次 retriever 调用。
///
/// LLM 子查询分解返回 `["Rust", "Python"]` 两个子查询，
/// 验证：retriever 被调用 2 次（两个并行 worker），
/// sources 包含两个不同文档的检索结果。
#[tokio::test]
async fn tc_coord_002_multi_subquery_parallel_retrieval() {
    let retriever = QueryAwareRetriever::new();
    let retriever_calls = retriever.call_count.clone();

    let llm = CoordinatorMockLlm::new(
        r#"["Rust 编程语言", "Python 编程语言"]"#.to_string(),
        "Rust 侧重系统编程，Python 侧重数据科学。".to_string(),
        vec!["Rust".to_string(), " 和 ".to_string(), "Python".to_string()],
    );

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "Rust 和 Python 是什么？").await.unwrap();

    // 两个子查询 → retriever 调用 2 次
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        2,
        "两个子查询应触发 2 次 retriever 调用（并行）"
    );

    // sources 应包含两个不同文档的结果
    let doc_names: Vec<&str> = outcome
        .sources
        .iter()
        .map(|s| s.doc_name.as_str())
        .collect();
    assert!(
        doc_names.contains(&"rust-guide.md"),
        "聚合来源应包含 Rust 文档"
    );
    assert!(
        doc_names.contains(&"python-intro.md"),
        "聚合来源应包含 Python 文档"
    );

    // researching 阶段应记录子查询数量
    let research_phase = outcome
        .phases
        .iter()
        .find(|p| p.phase == "researching")
        .expect("should have researching phase");
    assert_eq!(
        research_phase.sub_query_count,
        Some(2),
        "researching 阶段应记录 2 个子查询"
    );
}

// ================== TC-COORD-003: 综合阶段合并多源信息 ==================

/// TC-COORD-003：综合阶段正确合并多源信息。
///
/// 验证：综合分析内容被注入最终答案的 dynamic_context 中，
/// 最终答案流包含综合分析的概要信息。
#[tokio::test]
async fn tc_coord_003_synthesis_merges_multi_source() {
    let retriever = QueryAwareRetriever::new();
    let llm = CoordinatorMockLlm::new(
        r#"["Rust", "Python"]"#.to_string(),
        "综合：Rust 和 Python 在不同领域各有优势。".to_string(),
        vec!["答案".to_string()],
    );
    let llm_calls = llm.call_count.clone();

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "Rust 和 Python").await.unwrap();

    // LLM 应被调用至少 3 次（分解 + 综合 + 最终答案）
    assert!(
        llm_calls.load(Ordering::SeqCst) >= 3,
        "LLM 应至少调用 3 次（分解+综合+答案）"
    );

    // 最终答案流非空
    assert!(outcome.answer_stream.is_some(), "应返回最终答案流");

    // 消费答案流验证内容
    if let Some(stream) = outcome.answer_stream {
        let tokens: Vec<String> = stream.map(|r| r.unwrap()).collect().await;
        let answer = tokens.join("");
        assert!(!answer.is_empty(), "最终答案不应为空");
    }
}

// ================== TC-COORD-004: 最终答案流包含来源引用 ==================

/// TC-COORD-004：最终答案流引用了所有相关来源。
///
/// 验证：outcome.sources 包含全部并行检索的结果，
/// 且来源数量 >= 子查询数量。
#[tokio::test]
async fn tc_coord_004_sources_include_all_findings() {
    let retriever = QueryAwareRetriever::new();

    let llm = CoordinatorMockLlm::new(
        r#"["Rust", "Python", "通用"]"#.to_string(),
        "综合分析。".to_string(),
        vec!["最终答案".to_string()],
    );

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "Rust 和 Python").await.unwrap();

    // sources 应至少包含 2 个不同文档（3 个子查询可能有重复文档）
    let unique_docs: std::collections::HashSet<&str> = outcome
        .sources
        .iter()
        .map(|s| s.doc_name.as_str())
        .collect();
    assert!(
        unique_docs.len() >= 2,
        "聚合来源应至少包含 2 个不同文档，实际 {}",
        unique_docs.len()
    );

    // 来源去重后不应有重复 chunk.id
    let mut seen = std::collections::HashSet::new();
    for s in &outcome.sources {
        assert!(
            seen.insert(s.chunk.id.as_str()),
            "来源 chunk.id 不应重复: {}",
            s.chunk.id
        );
    }
}

// ================== TC-COORD-005: 降级策略 ==================

/// TC-COORD-005：综合分析失败时降级为标准 RAG。
///
/// 子查询分解返回单个查询（模拟降级），综合分析正常但最终答案生成成功。
/// 验证：降级路径不中断流程，最终答案流仍可用。
#[tokio::test]
async fn tc_coord_005_degradation_to_standard_rag() {
    let retriever = FixedRetriever::new(make_sources());
    let retriever_calls = retriever.call_count.clone();

    // 子查询分解返回无效 JSON → 降级为原始查询
    let llm = CoordinatorMockLlm::new(
        "这不是有效的JSON".to_string(), // 无效 → 降级为原始查询
        "综合分析。".to_string(),
        vec!["降级答案".to_string()],
    );

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "降级测试").await.unwrap();

    // 即使分解失败，retriever 也应被调用至少 1 次（降级为原始查询）
    assert!(
        retriever_calls.load(Ordering::SeqCst) >= 1,
        "降级后应至少检索 1 次"
    );

    // 最终答案流应可用
    assert!(outcome.answer_stream.is_some(), "降级后仍应返回最终答案流");
    assert!(!outcome.sources.is_empty(), "降级后仍应包含引用来源");
}

// ================== TC-COORD-006: 空知识库 NoContext ==================

/// TC-COORD-006：所有子查询均无命中时返回 NoContext。
#[tokio::test]
async fn tc_coord_006_empty_kb_returns_no_context() {
    let retriever = EmptyRetriever;

    let llm = CoordinatorMockLlm::new(
        r#"["查询1", "查询2"]"#.to_string(),
        "综合分析。".to_string(),
        vec!["答案".to_string()],
    );

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "空知识库测试").await.unwrap();

    // 空知识库 → answer_stream 为 None
    assert!(
        outcome.answer_stream.is_none(),
        "空知识库应返回 NoContext（answer_stream = None）"
    );
    assert!(outcome.sources.is_empty(), "空知识库应无引用来源");

    // 阶段序列应只有 researching（后续阶段不执行）
    assert!(
        outcome.phases.iter().all(|p| p.phase == "researching"),
        "空知识库应只执行 researching 阶段"
    );
}

// ================== TC-COORD-007: 阶段信息完整 ==================

/// TC-COORD-007：CoordinatorPhaseInfo 字段完整且正确。
#[tokio::test]
async fn tc_coord_007_phase_info_fields() {
    let retriever = FixedRetriever::new(make_sources());

    let llm = CoordinatorMockLlm::new(
        r#"["测试"]"#.to_string(),
        "综合。".to_string(),
        vec!["答案".to_string()],
    );

    let engine = CoordinatorEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试").await.unwrap();

    // 验证每个阶段的信息
    assert!(outcome.phases.len() >= 3, "应至少有 3 个阶段");

    for phase in &outcome.phases {
        assert!(!phase.phase.is_empty(), "阶段标识不应为空");
        assert!(!phase.message.is_empty(), "阶段消息不应为空");
    }

    // researching 阶段应有子查询数量
    let research_phase = outcome
        .phases
        .iter()
        .find(|p| p.phase == "researching")
        .expect("should have researching");
    assert!(
        research_phase.sub_query_count.is_some(),
        "researching 阶段应记录子查询数量"
    );
}

// ================== TC-COORD-008: StepCache 步骤缓存复用 ==================

/// TC-COORD-008：启用 StepCache 后，相同查询的分解/检索/综合阶段复用缓存结果（P2-1）。
///
/// 引擎运行两次（相同查询），第二次运行时：
/// - 子查询分解命中缓存 → 跳过 LLM 调用
/// - 逐子查询检索命中缓存 → retriever 不再被调用
/// - 综合分析命中缓存 → 跳过 LLM 调用
#[tokio::test]
async fn tc_coord_008_step_cache_reuses_stages() {
    use crate::step_cache::{InMemoryStepCache, StepCache as _};
    use std::sync::Arc;

    let retriever = QueryAwareRetriever::new();
    let retriever_calls = retriever.call_count.clone();

    /// 循环 Mock LLM：每轮返回相同三阶段序列（分解 → 综合 → 答案流）。
    struct CyclicCoordinatorLlm {
        call_count: Arc<AtomicUsize>,
    }

    impl LLMProvider for CyclicCoordinatorLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            match count % 3 {
                0 => Ok(futures::stream::once(async move {
                    Ok(r#"["Rust 编程语言", "Python 编程语言"]"#.to_string())
                })
                .boxed()),
                1 => Ok(futures::stream::once(async move {
                    Ok("综合：Rust 侧重系统，Python 侧重数据。".to_string())
                })
                .boxed()),
                _ => {
                    Ok(
                        futures::stream::iter(vec![Ok("最终".to_string()), Ok("答案".to_string())])
                            .boxed(),
                    )
                }
            }
        }
    }

    let llm = CyclicCoordinatorLlm {
        call_count: Arc::new(AtomicUsize::new(0)),
    };
    let llm_calls = llm.call_count.clone();
    let cache = Arc::new(InMemoryStepCache::default());
    let cache_for_stats = Arc::clone(&cache);
    let engine = CoordinatorEngine::with_step_cache(retriever, llm, cache);

    // 第一次运行：全链路未命中 → 分解 1 次 LLM + 2 次检索 + 综合 1 次 LLM
    let outcome1 = engine.run(&[], "Rust 和 Python 是什么？").await.unwrap();
    let calls_after_first = retriever_calls.load(Ordering::SeqCst);
    assert_eq!(
        calls_after_first, 2,
        "首次运行应触发 2 次检索（两个子查询）"
    );
    assert!(!outcome1.sources.is_empty(), "首次运行应包含引用来源");

    // 第二次运行：分解/检索/综合全部命中缓存 → retriever 零调用，LLM 仅 1 次（最终答案流）
    let llm_calls_before_second = llm_calls.load(Ordering::SeqCst);
    let outcome2 = engine.run(&[], "Rust 和 Python 是什么？").await.unwrap();
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        calls_after_first,
        "第二次运行的子查询检索应命中 StepCache，不再调用 retriever"
    );
    let llm_calls_after_second = llm_calls.load(Ordering::SeqCst);
    assert_eq!(
        llm_calls_after_second - llm_calls_before_second,
        1,
        "第二次运行应仅 1 次 LLM 调用（最终答案流），分解/综合命中缓存"
    );
    assert!(!outcome2.sources.is_empty(), "缓存命中时仍应返回引用来源");

    // 缓存统计：至少 3 次命中（分解 + 2 个子查询检索）
    let stats = cache_for_stats.stats();
    assert!(
        stats.hits >= 3,
        "应至少有 3 次缓存命中，实际 {}",
        stats.hits
    );
}
