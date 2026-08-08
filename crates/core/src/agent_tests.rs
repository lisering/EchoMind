#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-AGENT-001~008 Agentic RAG 多步推理（REQ-RAG-022）。
//!
//! 测试策略：
//! - Mock Retriever：返回预置检索结果
//! - Mock LLM：返回 ReAct 格式响应（Thought + Action / Final Answer）
//! - 验证 ReAct 循环、步骤事件、降级策略、最大迭代限制

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use echomind_models::{ChatMessage, Chunk, RetrievalResult};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::agent::{AgentEngine, AgentStepInfo};
use crate::{LLMProvider, Retriever};

// ================== Mock 实现 ==================

/// 命中检索 Mock Retriever：返回预置结果。
struct MockRetriever {
    results: Vec<RetrievalResult>,
    /// 检索调用计数
    call_count: Arc<AtomicUsize>,
}

impl MockRetriever {
    fn new(results: Vec<RetrievalResult>) -> Self {
        Self {
            results,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Retriever for MockRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.results.clone())
    }
}

/// 空检索 Mock Retriever：始终返回空结果。
struct EmptyRetriever;

impl Retriever for EmptyRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        Ok(vec![])
    }
}

/// ReAct Mock LLM：按预设序列返回 ReAct 格式响应。
///
/// 第一次调用返回 Thought + Action（触发检索），
/// 第二次调用返回 Final Answer（触发最终答案流）。
struct ReactMockLlm {
    /// chat_stream 调用计数
    call_count: Arc<AtomicUsize>,
    /// 第一次调用的响应（Thought + Action）
    action_response: String,
    /// 第二次调用的响应（Final Answer 触发后的最终答案流）
    final_answer_tokens: Vec<String>,
}

impl ReactMockLlm {
    fn new(action_response: String, final_answer_tokens: Vec<String>) -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
            action_response,
            final_answer_tokens,
        }
    }
}

impl LLMProvider for ReactMockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);

        if count == 0 {
            // 第一次调用：返回 Action 响应
            let resp = self.action_response.clone();
            Ok(futures::stream::once(async move { Ok(resp) }).boxed())
        } else if count == 1 {
            // 第二次调用：返回 Final Answer 触发最终答案生成
            Ok(futures::stream::once(async move {
                Ok("Thought: 信息已充分\nFinal Answer: 开始生成答案".to_string())
            })
            .boxed())
        } else {
            // 第三次及后续调用：返回最终答案 token 流
            let tokens = self.final_answer_tokens.clone();
            Ok(futures::stream::iter(tokens.into_iter().map(Ok).collect::<Vec<_>>()).boxed())
        }
    }
}

/// 不可解析 Mock LLM：返回非 ReAct 格式响应（测试降级策略）。
struct UnparseableLlm {
    call_count: Arc<AtomicUsize>,
}

impl UnparseableLlm {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl LLMProvider for UnparseableLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        if count == 0 {
            // 返回非 ReAct 格式的响应
            Ok(futures::stream::once(async move {
                Ok(
                    "这是一个无法解析的普通回答，不包含 Thought/Action/Final Answer 格式。"
                        .to_string(),
                )
            })
            .boxed())
        } else {
            // 降级后的最终答案流
            Ok(futures::stream::iter(vec![
                Ok("降".to_string()),
                Ok("级".to_string()),
                Ok("回".to_string()),
                Ok("答".to_string()),
            ])
            .boxed())
        }
    }
}

/// 构造测试用检索结果。
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

// ================== TC-AGENT-001: 多步检索 ==================

/// TC-AGENT-001：启用 Agentic 模式后触发多步检索（REQ-RAG-022 AC-1）。
///
/// LLM 先输出 Thought + Action(search_kb)，触发检索，
/// 再输出 Final Answer，完成多步推理。
#[tokio::test]
async fn tc_agent_001_multi_step_retrieval() {
    let retriever = MockRetriever::new(make_sources());
    let retriever_calls = retriever.call_count.clone();

    let llm = ReactMockLlm::new(
        "Thought: 我需要搜索知识库中关于 Rust 的内容\n\
         Action: search_kb\n\
         Action Input: Rust 编程语言"
            .to_string(),
        vec![
            "Rust".to_string(),
            " 是".to_string(),
            " 系统编程语言".to_string(),
        ],
    );
    let llm_calls = llm.call_count.clone();

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "什么是 Rust？").await.unwrap();

    // AC-1: 触发了检索（Action 执行了 search_kb）
    assert!(
        retriever_calls.load(Ordering::SeqCst) >= 1,
        "Agent 应至少调用一次 search_kb 工具"
    );

    // LLM 被调用至少 2 次（第一次 Action，第二次 Final Answer）
    assert!(
        llm_calls.load(Ordering::SeqCst) >= 2,
        "Agent 应至少调用 LLM 两次（Action + Final Answer）"
    );

    // 步骤中应包含 thought 和 action 类型
    let has_thought = outcome.steps.iter().any(|s| s.step_type == "thought");
    let has_action = outcome.steps.iter().any(|s| s.step_type == "action");
    assert!(has_thought, "步骤中应包含 thought 类型");
    assert!(has_action, "步骤中应包含 action 类型");
}

// ================== TC-AGENT-002: 步骤事件 ==================

/// TC-AGENT-002：中间推理步骤包含 thought/action/observation/answer 类型（REQ-RAG-022 AC-2）。
#[tokio::test]
async fn tc_agent_002_step_types() {
    let retriever = MockRetriever::new(make_sources());
    let llm = ReactMockLlm::new(
        "Thought: 需要搜索\nAction: search_kb\nAction Input: 查询".to_string(),
        vec!["答案".to_string()],
    );

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试问题").await.unwrap();

    // 验证步骤类型
    let step_types: Vec<&str> = outcome.steps.iter().map(|s| s.step_type.as_str()).collect();

    assert!(step_types.contains(&"thought"), "步骤应包含 thought 类型");
    assert!(step_types.contains(&"action"), "步骤应包含 action 类型");
    assert!(
        step_types.contains(&"observation"),
        "步骤应包含 observation 类型"
    );
    assert!(step_types.contains(&"answer"), "步骤应包含 answer 类型");

    // 验证 action 步骤包含 tool 和 input
    let action_step = outcome
        .steps
        .iter()
        .find(|s| s.step_type == "action")
        .expect("应存在 action 步骤");
    assert!(action_step.tool.is_some(), "action 步骤应包含 tool 字段");
    assert!(action_step.input.is_some(), "action 步骤应包含 input 字段");

    // 验证迭代轮次
    for step in &outcome.steps {
        assert!(step.iteration >= 1, "迭代轮次应从 1 开始");
    }
}

// ================== TC-AGENT-003: 最终答案流式输出 ==================

/// TC-AGENT-003：最终答案以 token 流输出（REQ-RAG-022 AC-3）。
#[tokio::test]
async fn tc_agent_003_final_answer_streamed() {
    let retriever = MockRetriever::new(make_sources());
    let expected_tokens = vec![
        "Rust".to_string(),
        " 是".to_string(),
        " 系统编程语言".to_string(),
    ];
    let llm = ReactMockLlm::new(
        "Thought: 搜索\nAction: search_kb\nAction Input: Rust".to_string(),
        expected_tokens.clone(),
    );

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "什么是 Rust？").await.unwrap();

    // AC-3: 最终答案应为流式输出
    assert!(outcome.answer_stream.is_some(), "应返回最终答案流");

    // 收集流中的 token
    let mut stream = outcome.answer_stream.unwrap();
    let mut tokens = Vec::new();
    while let Some(Ok(token)) = stream.next().await {
        tokens.push(token);
    }

    assert!(!tokens.is_empty(), "最终答案流应包含 token");
    let answer = tokens.join("");
    assert!(answer.contains("Rust"), "最终答案应包含 'Rust'");
}

// ================== TC-AGENT-004: 最大迭代限制 ==================

/// TC-AGENT-004：超过最大迭代次数后强制生成最终答案（REQ-RAG-022 AC-4）。
///
/// LLM 始终返回 Action（不返回 Final Answer），验证 5 次迭代后强制结束。
#[tokio::test]
async fn tc_agent_004_max_iterations() {
    let retriever = MockRetriever::new(make_sources());

    /// 始终返回 Action 的 Mock LLM（永不给出 Final Answer）。
    struct AlwaysActionLlm {
        call_count: Arc<AtomicUsize>,
    }

    impl LLMProvider for AlwaysActionLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(futures::stream::once(async move {
                Ok("Thought: 继续搜索\nAction: search_kb\nAction Input: 更多信息".to_string())
            })
            .boxed())
        }
    }

    let llm = AlwaysActionLlm {
        call_count: Arc::new(AtomicUsize::new(0)),
    };
    let llm_calls = llm.call_count.clone();

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试问题").await.unwrap();

    // AC-4: 应在 5 次迭代后强制结束
    // 每次迭代调用 LLM 一次（Action），第 6 次调用生成最终答案
    assert!(
        llm_calls.load(Ordering::SeqCst) <= 7,
        "LLM 调用次数不应超过 7 次（5 次 Action + 1 次强制答案 + 容差）"
    );

    // 应有最终答案流
    assert!(
        outcome.answer_stream.is_some(),
        "超过最大迭代后应强制生成最终答案"
    );

    // 步骤中应包含 "达到最大迭代" 的信息
    let has_max_iter_msg = outcome.steps.iter().any(|s| s.content.contains("最大迭代"));
    assert!(has_max_iter_msg, "应包含达到最大迭代次数的提示信息");
}

// ================== TC-AGENT-005: 降级策略 ==================

/// TC-AGENT-005：LLM 响应无法解析时降级为标准 RAG（REQ-RAG-022 AC-5）。
#[tokio::test]
async fn tc_agent_005_degrade_to_standard_rag() {
    let retriever = MockRetriever::new(make_sources());
    let llm = UnparseableLlm::new();

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试问题").await.unwrap();

    // AC-5: 降级后仍应返回答案流
    assert!(
        outcome.answer_stream.is_some(),
        "降级后应仍返回标准 RAG 答案流"
    );

    // 降级后应仍有引用来源（通过标准检索获取）
    assert!(
        !outcome.sources.is_empty(),
        "降级后应通过标准检索获取引用来源"
    );
}

/// TC-AGENT-005b：空知识库降级为 NoContext。
#[tokio::test]
async fn tc_agent_005b_empty_kb_no_context() {
    let retriever = EmptyRetriever;
    let llm = UnparseableLlm::new();

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试问题").await.unwrap();

    // 空知识库 + 降级 → answer_stream 为 None
    assert!(
        outcome.answer_stream.is_none(),
        "空知识库降级时应返回 None（NoContext）"
    );
    assert!(outcome.sources.is_empty(), "空知识库不应有引用来源");
}

// ================== TC-AGENT-006: 持久化开关（旁证） ==================

/// TC-AGENT-006：AgentEngine 可被正确构造和销毁（REQ-RAG-022 AC-6 旁证）。
///
/// 持久化逻辑在命令层（commands.rs），此处验证引擎本身的构造正确性。
#[tokio::test]
async fn tc_agent_006_engine_construction() {
    let retriever = MockRetriever::new(make_sources());
    let llm = ReactMockLlm::new(
        "Thought: 搜索\nAction: search_kb\nAction Input: 测试".to_string(),
        vec!["答案".to_string()],
    );

    let engine = AgentEngine::new(retriever, llm);

    // 引擎应能正常执行
    let outcome = engine.run(&[], "测试").await;
    assert!(outcome.is_ok(), "引擎应正常执行不报错");
}

// ================== TC-AGENT-007: 引用来源聚合 ==================

/// TC-AGENT-007：引用来源聚合全部检索步骤的命中结果（REQ-RAG-022 AC-7）。
#[tokio::test]
async fn tc_agent_007_aggregated_sources() {
    let retriever = MockRetriever::new(make_sources());
    let llm = ReactMockLlm::new(
        "Thought: 搜索\nAction: search_kb\nAction Input: Rust".to_string(),
        vec!["最终答案".to_string()],
    );

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "什么是 Rust？").await.unwrap();

    // AC-7: 引用来源应包含检索步骤的命中结果
    assert!(!outcome.sources.is_empty(), "引用来源不应为空");
    assert!(
        outcome
            .sources
            .iter()
            .any(|s| s.doc_name == "rust-guide.md"),
        "引用来源应包含检索命中的文档"
    );
}

// ================== TC-AGENT-008: 步骤迭代轮次 ==================

/// TC-AGENT-008：步骤中的迭代轮次正确递增（REQ-RAG-022 AC-4 旁证）。
#[tokio::test]
async fn tc_agent_008_iteration_tracking() {
    let retriever = MockRetriever::new(make_sources());
    let llm = ReactMockLlm::new(
        "Thought: 搜索\nAction: search_kb\nAction Input: 测试".to_string(),
        vec!["答案".to_string()],
    );

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试").await.unwrap();

    // 第一轮的步骤应标记为 iteration=1
    let first_steps: Vec<&AgentStepInfo> =
        outcome.steps.iter().filter(|s| s.iteration == 1).collect();
    assert!(!first_steps.is_empty(), "应存在 iteration=1 的步骤");

    // 所有步骤的 iteration 应 >= 1
    for step in &outcome.steps {
        assert!(step.iteration >= 1, "所有步骤的迭代轮次应 >= 1");
    }
}

// ================== TC-AGENT-009: 多 Action 并行执行 ==================

/// TC-AGENT-009：LLM 返回编号 Action 时触发并行执行（Session 29）。
///
/// LLM 第一次调用返回两个编号 Action（search_kb × 2），
/// 第二次调用返回 Final Answer。
/// 验证：retriever 被调用 2 次（两个 Action 并行执行），
/// 步骤中同一 iteration 包含多个 action 和 observation。
#[tokio::test]
async fn tc_agent_009_multi_action_parallel() {
    let retriever = MockRetriever::new(make_sources());
    let retriever_calls = retriever.call_count.clone();

    /// 多 Action Mock LLM：第一次返回编号 Action，第二次返回 Final Answer。
    struct MultiActionLlm {
        call_count: Arc<AtomicUsize>,
    }

    impl LLMProvider for MultiActionLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            match count {
                0 => Ok(futures::stream::once(async move {
                    Ok("Thought: 需要同时搜索 Rust 和 Python\n\
                         Action 1: search_kb\n\
                         Action Input 1: Rust 编程语言\n\
                         Action 2: search_kb\n\
                         Action Input 2: Python 编程语言"
                        .to_string())
                })
                .boxed()),
                1 => Ok(futures::stream::once(async move {
                    Ok("Thought: 信息已充分\nFinal Answer: 开始生成答案".to_string())
                })
                .boxed()),
                _ => Ok(futures::stream::iter(vec![
                    Ok("Rust".to_string()),
                    Ok(" 和 ".to_string()),
                    Ok("Python".to_string()),
                ])
                .boxed()),
            }
        }
    }

    let llm = MultiActionLlm {
        call_count: Arc::new(AtomicUsize::new(0)),
    };

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "Rust 和 Python 是什么？").await.unwrap();

    // 并行执行：retriever 应被调用恰好 2 次（两个 search_kb Action）
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        2,
        "两个编号 Action 应触发 2 次 retriever 调用"
    );

    // 同一 iteration 中应包含 2 个 action 步骤
    let iter1_actions: Vec<&AgentStepInfo> = outcome
        .steps
        .iter()
        .filter(|s| s.iteration == 1 && s.step_type == "action")
        .collect();
    assert_eq!(
        iter1_actions.len(),
        2,
        "第 1 轮应包含 2 个 action 步骤（并行执行）"
    );

    // 同一 iteration 中应包含 2 个 observation 步骤
    let iter1_obs: Vec<&AgentStepInfo> = outcome
        .steps
        .iter()
        .filter(|s| s.iteration == 1 && s.step_type == "observation")
        .collect();
    assert_eq!(
        iter1_obs.len(),
        2,
        "第 1 轮应包含 2 个 observation 步骤（并行结果汇总）"
    );

    // 两个 action 的输入应不同
    assert!(
        iter1_actions[0].input.as_deref() != iter1_actions[1].input.as_deref(),
        "两个并行 Action 的输入应不同"
    );

    // 应有最终答案流
    assert!(outcome.answer_stream.is_some(), "应返回最终答案流");
}

// ================== TC-AGENT-010: 并行来源聚合 ==================

/// TC-AGENT-010：并行执行的多个 Action 各自返回引用来源，全部聚合到 outcome.sources（Session 29）。
///
/// 使用查询感知 Mock Retriever：不同查询返回不同文档。
/// 验证：outcome.sources 包含两个不同查询的检索结果。
#[tokio::test]
async fn tc_agent_010_parallel_sources_aggregation() {
    /// 查询感知 Mock Retriever：根据查询关键词返回不同结果。
    struct QueryAwareRetriever {
        call_count: Arc<AtomicUsize>,
    }

    impl Retriever for QueryAwareRetriever {
        async fn retrieve(&self, query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            if query.contains("Rust") {
                Ok(vec![RetrievalResult {
                    chunk: Chunk::new(
                        "doc-rust".to_string(),
                        "Rust 是系统编程语言".to_string(),
                        10,
                        0,
                    ),
                    score: 0.95,
                    doc_name: "rust-guide.md".to_string(),
                }])
            } else if query.contains("Python") {
                Ok(vec![RetrievalResult {
                    chunk: Chunk::new(
                        "doc-python".to_string(),
                        "Python 是脚本语言".to_string(),
                        10,
                        0,
                    ),
                    score: 0.92,
                    doc_name: "python-intro.md".to_string(),
                }])
            } else {
                Ok(vec![])
            }
        }
    }

    let retriever = QueryAwareRetriever {
        call_count: Arc::new(AtomicUsize::new(0)),
    };
    let retriever_calls = retriever.call_count.clone();

    /// 多 Action Mock LLM（复用 TC-AGENT-009 模式）。
    struct MultiActionLlm {
        call_count: Arc<AtomicUsize>,
    }

    impl LLMProvider for MultiActionLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            match count {
                0 => Ok(futures::stream::once(async move {
                    Ok("Thought: 需要分别搜索\n\
                         Action 1: search_kb\n\
                         Action Input 1: Rust\n\
                         Action 2: search_kb\n\
                         Action Input 2: Python"
                        .to_string())
                })
                .boxed()),
                1 => Ok(futures::stream::once(async move {
                    Ok("Thought: 信息已充分\nFinal Answer: 完成".to_string())
                })
                .boxed()),
                _ => Ok(futures::stream::iter(vec![Ok("答案".to_string())]).boxed()),
            }
        }
    }

    let llm = MultiActionLlm {
        call_count: Arc::new(AtomicUsize::new(0)),
    };

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "Rust 和 Python").await.unwrap();

    // 两个并行 Action 各执行一次检索
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        2,
        "应触发 2 次检索（两个并行 Action）"
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
    assert!(
        outcome.sources.len() >= 2,
        "聚合来源应至少包含 2 条结果（两个并行 Action 各 1 条）"
    );
}

// ================== TC-AGENT-011: 单个编号 Action 兼容 ==================

/// TC-AGENT-011：LLM 返回单个编号 Action（Action 1）时正确解析为 MultiActions（Session 29）。
///
/// 验证：即使只有一个编号 Action，也能正确解析并执行。
#[tokio::test]
async fn tc_agent_011_single_numbered_action() {
    let retriever = MockRetriever::new(make_sources());
    let retriever_calls = retriever.call_count.clone();

    /// 单编号 Action Mock LLM。
    struct SingleNumberedLlm {
        call_count: Arc<AtomicUsize>,
    }

    impl LLMProvider for SingleNumberedLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            match count {
                0 => Ok(futures::stream::once(async move {
                    Ok("Thought: 需要搜索\n\
                         Action 1: search_kb\n\
                         Action Input 1: 测试查询"
                        .to_string())
                })
                .boxed()),
                1 => Ok(futures::stream::once(async move {
                    Ok("Thought: 完成\nFinal Answer: 答案".to_string())
                })
                .boxed()),
                _ => Ok(futures::stream::iter(vec![Ok("答案".to_string())]).boxed()),
            }
        }
    }

    let llm = SingleNumberedLlm {
        call_count: Arc::new(AtomicUsize::new(0)),
    };

    let engine = AgentEngine::new(retriever, llm);
    let outcome = engine.run(&[], "测试").await.unwrap();

    // 单个编号 Action 也应触发 1 次检索
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        1,
        "单个编号 Action 应触发 1 次 retriever 调用"
    );

    // 第 1 轮应有 1 个 action 步骤
    let iter1_actions: Vec<&AgentStepInfo> = outcome
        .steps
        .iter()
        .filter(|s| s.iteration == 1 && s.step_type == "action")
        .collect();
    assert_eq!(iter1_actions.len(), 1, "第 1 轮应包含 1 个 action 步骤");

    // 应有最终答案
    assert!(outcome.answer_stream.is_some(), "应返回最终答案流");
    assert!(!outcome.sources.is_empty(), "应包含引用来源");
}

// ================== TC-AGENT-012: StepCache 步骤缓存复用 ==================

/// TC-AGENT-012：启用 StepCache 后，相同 search_kb 子任务复用缓存结果（P2-1）。
///
/// 引擎运行两次（相同工具 + 相同查询），第二次的 search_kb 命中步骤缓存，
/// retriever 不再被调用（跳过嵌入 + 检索计算），观察结果与来源与首次一致。
#[tokio::test]
async fn tc_agent_012_step_cache_reuses_tool_result() {
    use crate::step_cache::{InMemoryStepCache, StepCache as _};
    use std::sync::Arc;

    let retriever = MockRetriever::new(make_sources());
    let retriever_calls = retriever.call_count.clone();

    /// 循环 Mock LLM：每轮调用返回相同 ReAct 序列（Action → Final Answer 触发 → 答案流）。
    struct CyclicReactLlm {
        call_count: Arc<AtomicUsize>,
    }

    impl LLMProvider for CyclicReactLlm {
        async fn chat_stream(
            &self,
            _system_prompt: &str,
            _history: &[ChatMessage],
            _query: &str,
        ) -> Result<BoxStream<'static, Result<String>>> {
            let count = self.call_count.fetch_add(1, Ordering::SeqCst);
            match count % 3 {
                0 => Ok(futures::stream::once(async move {
                    Ok("Thought: 需要搜索 Rust\n\
                         Action: search_kb\n\
                         Action Input: Rust 编程语言"
                        .to_string())
                })
                .boxed()),
                1 => Ok(futures::stream::once(async move {
                    Ok("Thought: 信息已充分\nFinal Answer: 开始生成答案".to_string())
                })
                .boxed()),
                _ => Ok(futures::stream::iter(vec![
                    Ok("Rust".to_string()),
                    Ok(" 是".to_string()),
                    Ok(" 系统语言".to_string()),
                ])
                .boxed()),
            }
        }
    }

    let llm = CyclicReactLlm {
        call_count: Arc::new(AtomicUsize::new(0)),
    };
    let cache = Arc::new(InMemoryStepCache::default());
    let cache_for_stats = Arc::clone(&cache);
    let engine = AgentEngine::with_step_cache(retriever, llm, cache);

    // 第一次运行：search_kb 未命中 → 执行检索并写缓存
    let outcome1 = engine.run(&[], "什么是 Rust？").await.unwrap();
    let calls_after_first = retriever_calls.load(Ordering::SeqCst);
    assert_eq!(calls_after_first, 1, "首次运行应触发 1 次检索");
    assert!(!outcome1.sources.is_empty(), "首次运行应包含引用来源");

    // 第二次运行：相同 search_kb 输入 → 命中步骤缓存，retriever 不再被调用
    let outcome2 = engine.run(&[], "什么是 Rust？").await.unwrap();
    assert_eq!(
        retriever_calls.load(Ordering::SeqCst),
        calls_after_first,
        "第二次运行的相同 search_kb 应命中 StepCache，不再调用 retriever"
    );
    assert!(!outcome2.sources.is_empty(), "缓存命中时仍应返回引用来源");

    // 缓存统计：至少 1 次命中、2 次写入（两轮运行各写一次相同键）
    let stats = cache_for_stats.stats();
    assert!(
        stats.hits >= 1,
        "应至少有 1 次缓存命中，实际 {}",
        stats.hits
    );
    assert!(stats.entries >= 1, "缓存中应保留条目");
}
