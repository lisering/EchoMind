#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! QA 红灯测试：TC-WF-001~009 DAG 工作流引擎（REQ-RAG-030）。
//!
//! 测试策略：
//! - Mock Retriever：返回预置检索结果
//! - Mock LLM：返回固定文本流（支持捕获 system_prompt 验证占位符替换）
//! - 验证拓扑排序、并行执行、条件分支、聚合策略、容错传播

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, bail};
use echomind_models::{
    Chunk, NodeStatus, NodeType, RetrievalResult, Workflow, WorkflowEdge, WorkflowNode,
};
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::workflow::WorkflowEngine;
use crate::{LLMProvider, Retriever};

// ================== Mock 实现 ==================

/// 命中检索 Mock Retriever：返回预置结果。
struct MockRetriever {
    results: Vec<RetrievalResult>,
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

/// 始终返回错误的 Mock Retriever（测试容错传播）。
struct FailingRetriever;

impl Retriever for FailingRetriever {
    async fn retrieve(&self, _query: &str, _top_k: usize) -> Result<Vec<RetrievalResult>> {
        bail!("检索失败（测试用）")
    }
}

/// 简单 Mock LLM：始终返回固定文本。
struct MockLlm {
    response: String,
    call_count: Arc<AtomicUsize>,
}

impl MockLlm {
    fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl LLMProvider for MockLlm {
    async fn chat_stream(
        &self,
        _system_prompt: &str,
        _history: &[echomind_models::ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let resp = self.response.clone();
        Ok(futures::stream::once(async move { Ok(resp) }).boxed())
    }
}

/// 捕获 system_prompt 的 Mock LLM（验证 {input} 占位符替换）。
struct CapturingMockLlm {
    /// 捕获的 system_prompt（最后一次调用）
    captured_prompt: Arc<std::sync::Mutex<String>>,
    response: String,
}

impl CapturingMockLlm {
    fn new(response: &str) -> Self {
        Self {
            captured_prompt: Arc::new(std::sync::Mutex::new(String::new())),
            response: response.to_string(),
        }
    }
}

impl LLMProvider for CapturingMockLlm {
    async fn chat_stream(
        &self,
        system_prompt: &str,
        _history: &[echomind_models::ChatMessage],
        _query: &str,
    ) -> Result<BoxStream<'static, Result<String>>> {
        *self.captured_prompt.lock().unwrap() = system_prompt.to_string();
        let resp = self.response.clone();
        Ok(futures::stream::once(async move { Ok(resp) }).boxed())
    }
}

// ================== 辅助构造函数 ==================

/// 构造测试用检索结果。
fn make_sources() -> Vec<RetrievalResult> {
    vec![RetrievalResult {
        chunk: Chunk::new(
            "doc-1".to_string(),
            "Rust 是一门系统编程语言，强调内存安全和并发。".to_string(),
            10,
            0,
        ),
        score: 0.9,
        doc_name: "rust-guide.md".to_string(),
    }]
}

/// 构造简单线性工作流：Retrieval → Generation → Output
fn make_linear_workflow() -> Workflow {
    let nodes = vec![
        WorkflowNode {
            id: "node_1".to_string(),
            label: "检索".to_string(),
            node_type: NodeType::Retrieval {
                top_k: 5,
                retrieval_mode: "vector".to_string(),
            },
        },
        WorkflowNode {
            id: "node_2".to_string(),
            label: "生成".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "基于以下检索结果回答问题：\n{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "node_3".to_string(),
            label: "输出".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
    ];
    let edges = vec![
        WorkflowEdge {
            source: "node_1".to_string(),
            target: "node_2".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "node_2".to_string(),
            target: "node_3".to_string(),
            mapping: None,
        },
    ];
    Workflow::new(
        "线性工作流".to_string(),
        "检索→生成→输出".to_string(),
        nodes,
        edges,
    )
}

/// 构造并行工作流：Retrieval → {Generation A, Generation B} → Aggregate
fn make_parallel_workflow() -> Workflow {
    let nodes = vec![
        WorkflowNode {
            id: "node_1".to_string(),
            label: "检索".to_string(),
            node_type: NodeType::Retrieval {
                top_k: 5,
                retrieval_mode: "vector".to_string(),
            },
        },
        WorkflowNode {
            id: "node_2".to_string(),
            label: "生成A".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "翻译为英文：{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "node_3".to_string(),
            label: "生成B".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "生成摘要：{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "node_4".to_string(),
            label: "聚合".to_string(),
            node_type: NodeType::Aggregate {
                strategy: "concat".to_string(),
            },
        },
    ];
    let edges = vec![
        WorkflowEdge {
            source: "node_1".to_string(),
            target: "node_2".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "node_1".to_string(),
            target: "node_3".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "node_2".to_string(),
            target: "node_4".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "node_3".to_string(),
            target: "node_4".to_string(),
            mapping: None,
        },
    ];
    Workflow::new(
        "并行工作流".to_string(),
        "检索→{生成A,生成B}→聚合".to_string(),
        nodes,
        edges,
    )
}

// ================== TC-WF-001: 简单线性工作流 ==================

/// TC-WF-001：简单线性工作流（A→B→C）正确执行（REQ-RAG-030 AC-1）。
///
/// 工作流：Retrieval → Generation → Output
/// 验证：3 个节点都 Completed，final_output 非空
#[tokio::test]
async fn tc_workflow_001_linear_execution() {
    let retriever = MockRetriever::new(make_sources());
    let llm = MockLlm::new("Rust 是一门系统编程语言。");

    let engine = WorkflowEngine::new(retriever, llm);
    let workflow = make_linear_workflow();

    let result = engine.run(&workflow, "什么是 Rust？").await.unwrap();

    // AC-1: 3 个节点都 Completed
    assert!(matches!(
        result.node_results.get("node_1"),
        Some(NodeStatus::Completed { .. })
    ));
    assert!(matches!(
        result.node_results.get("node_2"),
        Some(NodeStatus::Completed { .. })
    ));
    assert!(matches!(
        result.node_results.get("node_3"),
        Some(NodeStatus::Completed { .. })
    ));

    // final_output 非空
    assert!(!result.final_output.is_empty(), "最终输出不应为空");
    assert_eq!(result.final_output, "Rust 是一门系统编程语言。");
}

// ================== TC-WF-002: 并行节点执行 ==================

/// TC-WF-002：并行节点（A→{B,C}→D）正确并行执行（REQ-RAG-030 AC-2）。
///
/// 工作流：Retrieval → {Generation A, Generation B} → Aggregate
/// 验证：node_2 和 node_3 在同一层，node_4 聚合两者输出
#[tokio::test]
async fn tc_workflow_002_parallel_execution() {
    let retriever = MockRetriever::new(make_sources());
    let llm = MockLlm::new("处理结果");

    let engine = WorkflowEngine::new(retriever, llm);
    let workflow = make_parallel_workflow();

    let result = engine.run(&workflow, "测试输入").await.unwrap();

    // 所有节点都 Completed
    for i in 1..=4 {
        let node_id = format!("node_{i}");
        assert!(
            matches!(
                result.node_results.get(&node_id),
                Some(NodeStatus::Completed { .. })
            ),
            "节点 {node_id} 应为 Completed"
        );
    }

    // node_4（Aggregate）输出应包含两个 Generation 的输出
    if let Some(NodeStatus::Completed { output }) = result.node_results.get("node_4") {
        // concat 策略：两个 "处理结果" 用 \n 连接
        assert!(output.contains("处理结果"), "聚合输出应包含上游内容");
    } else {
        panic!("node_4 应为 Completed");
    }
}

// ================== TC-WF-003: 拓扑排序正确 ==================

/// TC-WF-003：拓扑排序正确分层（REQ-RAG-030 AC-3）。
///
/// 构造 3 层 DAG，验证 parallel_layers 返回 3 个 Vec
#[tokio::test]
async fn tc_workflow_003_topological_sort() {
    let retriever = MockRetriever::new(make_sources());
    let llm = MockLlm::new("test");

    let engine = WorkflowEngine::new(retriever, llm);

    // 3 层 DAG: A → {B, C} → D
    let nodes = vec![
        WorkflowNode {
            id: "A".to_string(),
            label: "A".to_string(),
            node_type: NodeType::Retrieval {
                top_k: 3,
                retrieval_mode: "vector".to_string(),
            },
        },
        WorkflowNode {
            id: "B".to_string(),
            label: "B".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "B: {input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "C".to_string(),
            label: "C".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "C: {input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "D".to_string(),
            label: "D".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
    ];
    let edges = vec![
        WorkflowEdge {
            source: "A".to_string(),
            target: "B".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "A".to_string(),
            target: "C".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "B".to_string(),
            target: "D".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "C".to_string(),
            target: "D".to_string(),
            mapping: None,
        },
    ];

    let layers = engine.parallel_layers(&nodes, &edges).unwrap();

    // 应为 3 层
    assert_eq!(layers.len(), 3, "3 层 DAG 应返回 3 个执行层");

    // 第一层应包含 A（入度为 0）
    assert!(layers[0].contains(&"A".to_string()), "第一层应包含 A");

    // 第二层应包含 B 和 C
    assert!(
        layers[1].contains(&"B".to_string()) && layers[1].contains(&"C".to_string()),
        "第二层应包含 B 和 C"
    );

    // 第三层应包含 D
    assert!(layers[2].contains(&"D".to_string()), "第三层应包含 D");
}

// ================== TC-WF-004: 环检测 ==================

/// TC-WF-004：检测到环返回错误（REQ-RAG-030 AC-4）。
///
/// edges 构成环 A→B→C→A
/// 验证：parallel_layers 返回 Err
#[tokio::test]
async fn tc_workflow_004_cycle_detection() {
    let retriever = MockRetriever::new(vec![]);
    let llm = MockLlm::new("test");

    let engine = WorkflowEngine::new(retriever, llm);

    let nodes = vec![
        WorkflowNode {
            id: "A".to_string(),
            label: "A".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
        WorkflowNode {
            id: "B".to_string(),
            label: "B".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
        WorkflowNode {
            id: "C".to_string(),
            label: "C".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
    ];
    // 环：A→B→C→A
    let edges = vec![
        WorkflowEdge {
            source: "A".to_string(),
            target: "B".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "B".to_string(),
            target: "C".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "C".to_string(),
            target: "A".to_string(),
            mapping: None,
        },
    ];

    let result = engine.parallel_layers(&nodes, &edges);

    assert!(result.is_err(), "环形 DAG 应返回错误");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("环"),
        "错误信息应包含 '环'，实际: {err_msg}"
    );
}

// ================== TC-WF-005: Condition 节点分支 ==================

/// TC-WF-005：Condition 节点正确分支（REQ-RAG-030 AC-5）。
///
/// expression = "contains:法律"
/// input 包含"法律" → true → 下游 A 执行
/// input 不包含"法律" → false → 下游 B 执行
#[tokio::test]
async fn tc_workflow_005_condition_branch() {
    let _retriever = MockRetriever::new(vec![]);
    let _llm = MockLlm::new("生成结果");

    // 测试 true 分支
    {
        let engine = WorkflowEngine::new(MockRetriever::new(vec![]), MockLlm::new("法律分析结果"));

        let nodes = vec![
            WorkflowNode {
                id: "cond".to_string(),
                label: "条件判断".to_string(),
                node_type: NodeType::Condition {
                    expression: "contains:法律".to_string(),
                },
            },
            WorkflowNode {
                id: "branch_true".to_string(),
                label: "法律路径".to_string(),
                node_type: NodeType::Generation {
                    system_prompt: "法律分析：{input}".to_string(),
                    model: None,
                },
            },
            WorkflowNode {
                id: "branch_false".to_string(),
                label: "技术路径".to_string(),
                node_type: NodeType::Generation {
                    system_prompt: "技术分析：{input}".to_string(),
                    model: None,
                },
            },
        ];
        let edges = vec![
            WorkflowEdge {
                source: "cond".to_string(),
                target: "branch_true".to_string(),
                mapping: Some("true".to_string()),
            },
            WorkflowEdge {
                source: "cond".to_string(),
                target: "branch_false".to_string(),
                mapping: Some("false".to_string()),
            },
        ];
        let workflow = Workflow::new("条件分支".to_string(), "测试".to_string(), nodes, edges);

        let result = engine.run(&workflow, "这份合同的法律风险").await.unwrap();

        // cond 应为 Completed，输出 "true"
        assert!(matches!(
            result.node_results.get("cond"),
            Some(NodeStatus::Completed { output }) if output == "true"
        ));

        // branch_true 应为 Completed
        assert!(
            matches!(
                result.node_results.get("branch_true"),
                Some(NodeStatus::Completed { .. })
            ),
            "条件为 true 时 branch_true 应执行"
        );

        // branch_false 应为 Skipped
        assert!(
            matches!(
                result.node_results.get("branch_false"),
                Some(NodeStatus::Skipped)
            ),
            "条件为 true 时 branch_false 应跳过"
        );
    }

    // 测试 false 分支
    {
        let engine = WorkflowEngine::new(MockRetriever::new(vec![]), MockLlm::new("技术分析结果"));

        let nodes = vec![
            WorkflowNode {
                id: "cond".to_string(),
                label: "条件判断".to_string(),
                node_type: NodeType::Condition {
                    expression: "contains:法律".to_string(),
                },
            },
            WorkflowNode {
                id: "branch_true".to_string(),
                label: "法律路径".to_string(),
                node_type: NodeType::Generation {
                    system_prompt: "法律分析：{input}".to_string(),
                    model: None,
                },
            },
            WorkflowNode {
                id: "branch_false".to_string(),
                label: "技术路径".to_string(),
                node_type: NodeType::Generation {
                    system_prompt: "技术分析：{input}".to_string(),
                    model: None,
                },
            },
        ];
        let edges = vec![
            WorkflowEdge {
                source: "cond".to_string(),
                target: "branch_true".to_string(),
                mapping: Some("true".to_string()),
            },
            WorkflowEdge {
                source: "cond".to_string(),
                target: "branch_false".to_string(),
                mapping: Some("false".to_string()),
            },
        ];
        let workflow = Workflow::new("条件分支".to_string(), "测试".to_string(), nodes, edges);

        let result = engine.run(&workflow, "技术文档").await.unwrap();

        // cond 应为 Completed，输出 "false"
        assert!(matches!(
            result.node_results.get("cond"),
            Some(NodeStatus::Completed { output }) if output == "false"
        ));

        // branch_true 应为 Skipped
        assert!(
            matches!(
                result.node_results.get("branch_true"),
                Some(NodeStatus::Skipped)
            ),
            "条件为 false 时 branch_true 应跳过"
        );

        // branch_false 应为 Completed
        assert!(
            matches!(
                result.node_results.get("branch_false"),
                Some(NodeStatus::Completed { .. })
            ),
            "条件为 false 时 branch_false 应执行"
        );
    }
}

// ================== TC-WF-006: Aggregate concat 策略 ==================

/// TC-WF-006：Aggregate 节点 concat 策略正确合并（REQ-RAG-030 AC-6）。
///
/// 两个上游输出 "结果A" + "结果B" → 合并为 "结果A\n结果B"
#[tokio::test]
async fn tc_workflow_006_aggregate_concat() {
    let retriever = MockRetriever::new(vec![]);
    let llm = MockLlm::new("结果A");

    // 构造：两个 Generation → Aggregate(concat) → Output
    let nodes = vec![
        WorkflowNode {
            id: "gen_a".to_string(),
            label: "生成A".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "生成A：{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "gen_b".to_string(),
            label: "生成B".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "生成B：{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "agg".to_string(),
            label: "聚合".to_string(),
            node_type: NodeType::Aggregate {
                strategy: "concat".to_string(),
            },
        },
        WorkflowNode {
            id: "out".to_string(),
            label: "输出".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
    ];
    let edges = vec![
        WorkflowEdge {
            source: "gen_a".to_string(),
            target: "agg".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "gen_b".to_string(),
            target: "agg".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "agg".to_string(),
            target: "out".to_string(),
            mapping: None,
        },
    ];
    let workflow = Workflow::new("聚合测试".to_string(), "测试".to_string(), nodes, edges);

    let engine = WorkflowEngine::new(retriever, llm);
    let result = engine.run(&workflow, "初始输入").await.unwrap();

    // Aggregate 节点应为 Completed
    if let Some(NodeStatus::Completed { output }) = result.node_results.get("agg") {
        // concat 策略：两个 "结果A" 用 \n 连接
        assert!(output.contains("结果A"), "聚合输出应包含上游内容");
        // 两个上游都是 "结果A"，所以应有 2 行
        assert_eq!(
            output.matches("结果A").count(),
            2,
            "concat 应合并两个上游输出"
        );
    } else {
        panic!("agg 节点应为 Completed");
    }
}

// ================== TC-WF-007: 节点失败下游 Skipped ==================

/// TC-WF-007：节点失败时下游标记 Skipped（REQ-RAG-030 AC-7）。
///
/// node_1（Retrieval）执行失败 → node_2（Generation）标记 Skipped
#[tokio::test]
async fn tc_workflow_007_failure_propagation() {
    let retriever = FailingRetriever;
    let llm = MockLlm::new("不应执行到这里");

    let nodes = vec![
        WorkflowNode {
            id: "node_1".to_string(),
            label: "检索".to_string(),
            node_type: NodeType::Retrieval {
                top_k: 5,
                retrieval_mode: "vector".to_string(),
            },
        },
        WorkflowNode {
            id: "node_2".to_string(),
            label: "生成".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "基于：{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "node_3".to_string(),
            label: "输出".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
    ];
    let edges = vec![
        WorkflowEdge {
            source: "node_1".to_string(),
            target: "node_2".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "node_2".to_string(),
            target: "node_3".to_string(),
            mapping: None,
        },
    ];
    let workflow = Workflow::new("容错测试".to_string(), "测试".to_string(), nodes, edges);

    let engine = WorkflowEngine::new(retriever, llm);
    let result = engine.run(&workflow, "查询").await.unwrap();

    // node_1 应为 Failed
    assert!(
        matches!(
            result.node_results.get("node_1"),
            Some(NodeStatus::Failed { .. })
        ),
        "node_1 应为 Failed"
    );

    // node_2 应为 Skipped（上游失败）
    assert!(
        matches!(result.node_results.get("node_2"), Some(NodeStatus::Skipped)),
        "node_2 应为 Skipped（上游失败）"
    );

    // node_3 应为 Skipped（传递闭包）
    assert!(
        matches!(result.node_results.get("node_3"), Some(NodeStatus::Skipped)),
        "node_3 应为 Skipped（上游失败传递）"
    );

    // final_output 应为空
    assert!(
        result.final_output.is_empty(),
        "最终输出应为空（无 Output 节点完成）"
    );
}

// ================== TC-WF-008: evaluate_condition 表达式 ==================

/// TC-WF-008：evaluate_condition 各种表达式正确评估（REQ-RAG-030 AC-8）。
#[test]
fn tc_workflow_008_evaluate_condition() {
    use crate::workflow::WorkflowEngine;

    // "true" — 总是 true
    assert!(WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("true", "任意内容"));

    // "nonempty" — 非空
    assert!(WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("nonempty", "有内容"));
    assert!(!WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("nonempty", ""));
    assert!(!WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("nonempty", "   "));

    // "contains:关键词" — 包含关键词
    assert!(
        WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition(
            "contains:法律",
            "这份合同的法律风险"
        )
    );
    assert!(
        !WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("contains:法律", "技术文档")
    );
    // 大小写不敏感
    assert!(
        WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition(
            "contains:rust",
            "Rust 编程语言"
        )
    );

    // "length>N" — 长度大于 N
    assert!(WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("length>5", "123456"));
    assert!(!WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("length>5", "12345"));

    // "length<N" — 长度小于 N
    assert!(WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("length<10", "短文本"));
    assert!(
        !WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition(
            "length<5",
            "这是一个很长的文本"
        )
    );

    // 未知表达式 → false（安全降级）
    assert!(
        !WorkflowEngine::<MockRetriever, MockLlm>::evaluate_condition("unknown:foo", "任意内容")
    );
}

// ================== TC-WF-009: {input} 占位符替换 ==================

/// TC-WF-009：Generation 节点 {input} 占位符替换正确（REQ-RAG-030 AC-9）。
///
/// system_prompt = "请翻译以下内容：{input}"
/// input = "Hello" → 验证发送给 LLM 的 system_prompt 包含 "请翻译以下内容：Hello"
#[tokio::test]
async fn tc_workflow_009_input_placeholder() {
    let retriever = MockRetriever::new(make_sources());
    let llm = CapturingMockLlm::new("翻译结果");

    // 构造：Retrieval → Generation({input} 替换) → Output
    let nodes = vec![
        WorkflowNode {
            id: "node_1".to_string(),
            label: "检索".to_string(),
            node_type: NodeType::Retrieval {
                top_k: 5,
                retrieval_mode: "vector".to_string(),
            },
        },
        WorkflowNode {
            id: "node_2".to_string(),
            label: "翻译".to_string(),
            node_type: NodeType::Generation {
                system_prompt: "请翻译以下内容：{input}".to_string(),
                model: None,
            },
        },
        WorkflowNode {
            id: "node_3".to_string(),
            label: "输出".to_string(),
            node_type: NodeType::Output {
                format: "text".to_string(),
            },
        },
    ];
    let edges = vec![
        WorkflowEdge {
            source: "node_1".to_string(),
            target: "node_2".to_string(),
            mapping: None,
        },
        WorkflowEdge {
            source: "node_2".to_string(),
            target: "node_3".to_string(),
            mapping: None,
        },
    ];
    let workflow = Workflow::new("占位符测试".to_string(), "测试".to_string(), nodes, edges);

    let engine = WorkflowEngine::new(retriever, llm);

    // 初始输入
    let result = engine.run(&workflow, "什么是 Rust").await.unwrap();

    // node_2 应为 Completed
    assert!(matches!(
        result.node_results.get("node_2"),
        Some(NodeStatus::Completed { .. })
    ));

    // 验证 system_prompt 中的 {input} 被替换为上游输出
    // 上游是 Retrieval 节点的格式化输出，包含 "Rust"
    // 注意：CapturingMockLlm 的捕获在 engine.run 过程中完成
    // 但我们没有直接访问 llm 的方式。这里改为验证最终输出
    // final_output 应为 "翻译结果"
    assert_eq!(result.final_output, "翻译结果");

    // 另一种验证：直接测试 execute_node 中的占位符替换
    // 使用 CapturingMockLlm 验证 system_prompt
    let _retriever2 = MockRetriever::new(make_sources());
    let _llm2 = CapturingMockLlm::new("结果");
    let _engine2 = WorkflowEngine::new(_retriever2, _llm2);

    // 直接调用 Generation 节点
    let _gen_node = WorkflowNode {
        id: "test_gen".to_string(),
        label: "测试".to_string(),
        node_type: NodeType::Generation {
            system_prompt: "请翻译以下内容：{input}".to_string(),
            model: None,
        },
    };

    // execute_node 是私有方法，无法直接调用。
    // 通过 run() 间接验证：构造单节点工作流
    let single_workflow = Workflow::new(
        "单节点".to_string(),
        "测试".to_string(),
        vec![
            WorkflowNode {
                id: "gen".to_string(),
                label: "翻译".to_string(),
                node_type: NodeType::Generation {
                    system_prompt: "请翻译以下内容：{input}".to_string(),
                    model: None,
                },
            },
            WorkflowNode {
                id: "out".to_string(),
                label: "输出".to_string(),
                node_type: NodeType::Output {
                    format: "text".to_string(),
                },
            },
        ],
        vec![WorkflowEdge {
            source: "gen".to_string(),
            target: "out".to_string(),
            mapping: None,
        }],
    );

    // 使用新的 CapturingMockLlm
    let capturing_llm = CapturingMockLlm::new("翻译完成");
    let captured = capturing_llm.captured_prompt.clone();
    let engine3 = WorkflowEngine::new(MockRetriever::new(vec![]), capturing_llm);

    engine3.run(&single_workflow, "Hello World").await.unwrap();

    // 验证 {input} 被替换
    let captured_prompt = {
        let guard = captured.lock().unwrap();
        guard.clone()
    };
    assert!(
        captured_prompt.contains("请翻译以下内容：Hello World"),
        "system_prompt 中的 {{input}} 应被替换为 'Hello World'，实际: {captured_prompt}"
    );
    assert!(
        !captured_prompt.contains("{input}"),
        "system_prompt 中不应残留 {{input}} 占位符"
    );
}
