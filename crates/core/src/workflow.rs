//! DAG 工作流引擎（REQ-RAG-030）：用户自定义多步骤 RAG 管线。
//!
//! ## 架构
//!
//! 借鉴 IfAI 的 DAG 工作流引擎：用户定义由节点（`WorkflowNode`）和边
//! （`WorkflowEdge`）组成的有向无环图，引擎负责拓扑排序、并行执行独立节点、
//! 串行执行依赖节点。支持条件分支和聚合节点。
//!
//! ## 节点类型
//!
//! | 类型 | 功能 |
//! |---|---|
//! | `Retrieval` | 知识库检索（使用上游输出作为查询） |
//! | `Generation` | LLM 生成（`{input}` 占位符替换） |
//! | `Condition` | 条件分支（`contains:` / `length>N` / `nonempty` / `true`） |
//! | `Aggregate` | 多输入聚合（`concat` / `summarize` / `best_of`） |
//! | `Output` | 工作流终点 |
//!
//! ## 执行策略
//!
//! 1. 拓扑排序（Kahn's algorithm）确定执行层级
//! 2. 同层无依赖节点并行执行（`join_all`）
//! 3. 有依赖节点等待上游完成
//! 4. `Condition` 节点根据表达式选择下游分支（通过边的 `mapping` 字段匹配）
//! 5. 节点失败时下游标记 `Skipped`（不崩溃）
//!
//! ## Condition 分支机制
//!
//! `Condition` 节点评估表达式后输出 `"true"` 或 `"false"`。
//! 下游边通过 `mapping` 字段指定条件标签：
//! - `Some("true")` — 仅当条件为 true 时激活
//! - `Some("false")` — 仅当条件为 false 时激活
//! - `None` — 无条件激活
//!
//! 未激活的下游节点及其传递闭包标记为 `Skipped`。

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Result, bail};
use echomind_models::{
    NodeStatus, NodeType, RetrievalResult, Workflow, WorkflowEdge, WorkflowNode, WorkflowResult,
};
use futures::StreamExt;
use futures::future::join_all;
use futures::stream::BoxStream;

use crate::{LLMProvider, Retriever};

/// DAG 工作流引擎。
///
/// 泛型参数：
/// - `R` — 检索器实现（`Retriever` trait）
/// - `L` — LLM 提供者实现（`LLMProvider` trait）
///
/// 引擎本身无状态，可安全共享。每次 `run()` 调用独立执行一个工作流定义。
pub struct WorkflowEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
}

impl<R: Retriever, L: LLMProvider> WorkflowEngine<R, L> {
    /// 创建工作流引擎。
    pub fn new(retriever: R, llm: L) -> Self {
        Self { retriever, llm }
    }

    /// 执行工作流。
    ///
    /// # 参数
    /// - `workflow` — 工作流定义（DAG）
    /// - `input` — 初始输入文本（传递给入度为 0 的节点）
    ///
    /// # 返回
    /// `WorkflowResult` 包含各节点状态和最终输出。
    ///
    /// # 错误
    /// - 工作流包含环 → `Err("工作流包含环...")`
    /// - 工作流无 Output 节点 → 最终输出为空字符串（非错误）
    pub async fn run(&self, workflow: &Workflow, input: &str) -> Result<WorkflowResult> {
        let start = std::time::Instant::now();

        // 1. 拓扑排序：将 DAG 分解为并行执行层
        let layers = self.parallel_layers(&workflow.nodes, &workflow.edges)?;

        // 2. 构建邻接表（source → [(target, mapping)]）和入度表
        let adjacency = build_adjacency(&workflow.edges);
        let in_degree = build_in_degree(&workflow.nodes, &workflow.edges);

        // 3. 执行状态跟踪
        let mut outputs: HashMap<String, String> = HashMap::new();
        let mut node_results: HashMap<String, NodeStatus> = HashMap::new();
        // 已跳过的节点集合（条件分支未选中或上游失败的传递闭包）
        let mut skipped_nodes: HashSet<String> = HashSet::new();

        // 4. 逐层执行
        for layer in &layers {
            // 4a. 跳过已标记为 Skipped 的节点
            let executable: Vec<&WorkflowNode> = layer
                .iter()
                .filter_map(|node_id| {
                    if skipped_nodes.contains(node_id) {
                        node_results.insert(node_id.clone(), NodeStatus::Skipped);
                        None
                    } else {
                        workflow.nodes.iter().find(|n| &n.id == node_id)
                    }
                })
                .collect();

            // 4b. 为每个节点准备输入（合并上游输出）
            let node_inputs: Vec<(&WorkflowNode, String)> = executable
                .iter()
                .map(|node| {
                    let node_input = collect_upstream_output(
                        &node.id,
                        &workflow.edges,
                        &outputs,
                        in_degree.get(&node.id).copied().unwrap_or(0),
                        input,
                    );
                    (*node, node_input)
                })
                .collect();

            // 4c. 并行执行同层节点
            let futures: Vec<_> = node_inputs
                .into_iter()
                .map(|(node, node_input)| {
                    let node_id = node.id.clone();
                    async move {
                        let result = self.execute_node(node, &node_input).await;
                        (node_id, result)
                    }
                })
                .collect();

            let results = join_all(futures).await;

            // 4d. 处理执行结果
            for (node_id, result) in results {
                match result {
                    Ok(output) => {
                        // 检查是否为 Condition 节点，处理条件分支
                        let node = workflow.nodes.iter().find(|n| n.id == node_id);
                        if let Some(node) = node
                            && let NodeType::Condition { .. } = &node.node_type
                        {
                            // Condition 输出 "true" 或 "false"
                            let condition_result = output.as_str();
                            // 检查下游边，标记未激活的下游节点
                            for edge in &workflow.edges {
                                if edge.source == node_id {
                                    let should_activate = match &edge.mapping {
                                        Some(label) => label == condition_result,
                                        None => true,
                                    };
                                    if !should_activate {
                                        // 标记 target 及其传递闭包为 Skipped
                                        mark_skip_closure(
                                            &edge.target,
                                            &adjacency,
                                            &mut skipped_nodes,
                                        );
                                    }
                                }
                            }
                        }
                        outputs.insert(node_id.clone(), output.clone());
                        node_results.insert(node_id, NodeStatus::Completed { output });
                    }
                    Err(err) => {
                        // 节点失败 → 下游标记 Skipped
                        mark_skip_closure(&node_id, &adjacency, &mut skipped_nodes);
                        node_results.insert(
                            node_id,
                            NodeStatus::Failed {
                                error: err.to_string(),
                            },
                        );
                    }
                }
            }
        }

        // 5. 收集最终输出（Output 节点的输出）
        let final_output = workflow
            .nodes
            .iter()
            .filter(|n| matches!(n.node_type, NodeType::Output { .. }))
            .filter_map(|n| outputs.get(&n.id).cloned())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(WorkflowResult {
            node_results,
            final_output,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 拓扑排序：将 DAG 分解为并行执行层。
    ///
    /// 同一层内的节点无相互依赖，可并行执行。
    /// 层与层之间有依赖关系，必须按序执行。
    ///
    /// # 算法
    /// Kahn's algorithm（BFS）：
    /// 1. 计算每个节点的入度
    /// 2. 入度为 0 的节点入队（第一层）
    /// 3. 移除第一层节点 → 更新下游入度 → 新的零入度节点为第二层
    /// 4. 重复直到所有节点处理完毕
    /// 5. 若剩余节点入度不为 0 → 存在环 → 返回 Err
    ///
    /// # 参数
    /// - `nodes` — 全部节点
    /// - `edges` — 全部边
    ///
    /// # 返回
    /// `Vec<Vec<String>>` — 每层节点 ID 列表（按拓扑序）
    ///
    /// # 错误
    /// 工作流包含环时返回 `Err`
    pub fn parallel_layers(
        &self,
        nodes: &[WorkflowNode],
        edges: &[WorkflowEdge],
    ) -> Result<Vec<Vec<String>>> {
        // 构建入度表和邻接表
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

        for node in nodes {
            in_degree.insert(node.id.clone(), 0);
            adjacency.insert(node.id.clone(), Vec::new());
        }

        for edge in edges {
            // 仅处理源和目标都存在的边（忽略悬空边）
            if in_degree.contains_key(&edge.source) && in_degree.contains_key(&edge.target) {
                adjacency
                    .get_mut(&edge.source)
                    .unwrap_or(&mut Vec::new())
                    .push(edge.target.clone());
                *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
            }
        }

        let mut layers: Vec<Vec<String>> = Vec::new();
        let mut processed: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        // 初始化：入度为 0 的节点入队
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id.clone());
            }
        }

        while !queue.is_empty() {
            let layer: Vec<String> = queue.drain(..).collect();
            for node_id in &layer {
                processed.insert(node_id.clone());
            }

            // 更新下游入度
            let mut next_queue: VecDeque<String> = VecDeque::new();
            for node_id in &layer {
                if let Some(downstream) = adjacency.get(node_id) {
                    for target in downstream {
                        if let Some(degree) = in_degree.get_mut(target) {
                            *degree = degree.saturating_sub(1);
                            if *degree == 0 && !processed.contains(target) {
                                next_queue.push_back(target.clone());
                            }
                        }
                    }
                }
            }

            layers.push(layer);
            queue = next_queue;
        }

        // 检测环：如果处理的节点数 < 总节点数 → 存在环
        if processed.len() < nodes.len() {
            let cycle_nodes: Vec<String> = nodes
                .iter()
                .filter(|n| !processed.contains(&n.id))
                .map(|n| n.id.clone())
                .collect();
            bail!(
                "工作流包含环，无法拓扑排序。涉及节点: {}",
                cycle_nodes.join(", ")
            );
        }

        Ok(layers)
    }

    /// 评估条件表达式，返回是否选择该分支。
    ///
    /// 支持的表达式格式：
    /// - `"contains:关键词"` — 输入包含关键词（大小写不敏感）
    /// - `"length>N"` — 输入长度大于 N
    /// - `"length<N"` — 输入长度小于 N
    /// - `"nonempty"` — 输入非空（去除首尾空白后）
    /// - `"true"` — 总是返回 true
    ///
    /// 未知表达式格式默认返回 `false`（安全降级）。
    pub fn evaluate_condition(expression: &str, input: &str) -> bool {
        let expr = expression.trim();

        if expr == "true" {
            return true;
        }

        if expr == "nonempty" {
            return !input.trim().is_empty();
        }

        if let Some(keyword) = expr.strip_prefix("contains:") {
            return input
                .to_lowercase()
                .contains(&keyword.trim().to_lowercase());
        }

        if let Some(rest) = expr.strip_prefix("length") {
            if let Some(num_str) = rest.strip_prefix('>')
                && let Ok(n) = num_str.trim().parse::<usize>()
            {
                return input.len() > n;
            }
            if let Some(num_str) = rest.strip_prefix('<')
                && let Ok(n) = num_str.trim().parse::<usize>()
            {
                return input.len() < n;
            }
        }

        // 未知表达式 → 安全降级为 false
        false
    }

    /// 执行单个节点。
    ///
    /// 根据节点类型调用相应的执行逻辑：
    /// - `Retrieval` — 调用 `Retriever.retrieve()`，格式化结果为文本
    /// - `Generation` — 替换 `{input}` 占位符，调用 `LLMProvider.chat_stream()`
    /// - `Condition` — 评估表达式，返回 `"true"` 或 `"false"`
    /// - `Aggregate` — 根据 strategy 合并上游输出
    /// - `Output` — 直接返回输入（标记终点）
    async fn execute_node(&self, node: &WorkflowNode, input: &str) -> Result<String> {
        match &node.node_type {
            NodeType::Retrieval { top_k, .. } => {
                let results = self.retriever.retrieve(input, *top_k).await?;
                Ok(format_retrieval_results(&results))
            }

            NodeType::Generation { system_prompt, .. } => {
                let prompt = system_prompt.replace("{input}", input);
                let stream = self.llm.chat_stream(&prompt, &[], input).await?;
                collect_stream(stream).await
            }

            NodeType::Condition { expression } => {
                let result = Self::evaluate_condition(expression, input);
                Ok(if result { "true" } else { "false" }.to_string())
            }

            NodeType::Aggregate { strategy } => match strategy.as_str() {
                "concat" => {
                    // input 已是合并后的文本（由 collect_upstream_output 拼接）
                    Ok(input.to_string())
                }
                "summarize" => {
                    // 调用 LLM 生成摘要
                    let prompt = format!("请将以下内容合并并生成简洁摘要：\n\n{input}");
                    let stream = self
                        .llm
                        .chat_stream("你是一个摘要生成助手", &[], &prompt)
                        .await?;
                    collect_stream(stream).await
                }
                "best_of" => {
                    // 选择最长的上游输出段（按 \n 分割）
                    let best = input.split('\n').max_by_key(|s| s.len()).unwrap_or(input);
                    Ok(best.to_string())
                }
                _ => {
                    // 未知策略 → 默认 concat
                    Ok(input.to_string())
                }
            },

            NodeType::Output { .. } => {
                // Output 节点不做处理，直接返回输入
                Ok(input.to_string())
            }
        }
    }
}

// ================== 辅助函数 ==================

/// 构建邻接表：source → [(target, mapping)]
fn build_adjacency(edges: &[WorkflowEdge]) -> HashMap<String, Vec<(String, Option<String>)>> {
    let mut adj: HashMap<String, Vec<(String, Option<String>)>> = HashMap::new();
    for edge in edges {
        adj.entry(edge.source.clone())
            .or_default()
            .push((edge.target.clone(), edge.mapping.clone()));
    }
    adj
}

/// 构建入度表：node_id → 入度数
fn build_in_degree(nodes: &[WorkflowNode], edges: &[WorkflowEdge]) -> HashMap<String, usize> {
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for node in nodes {
        in_degree.insert(node.id.clone(), 0);
    }
    for edge in edges {
        if in_degree.contains_key(&edge.target) {
            *in_degree.entry(edge.target.clone()).or_insert(0) += 1;
        }
    }
    in_degree
}

/// 收集节点的上游输出，合并为单个输入字符串。
///
/// - 入度为 0 的节点（起始节点）接收初始 `input`
/// - 入度 > 0 的节点接收所有上游输出的拼接（`\n` 分隔）
fn collect_upstream_output(
    node_id: &str,
    edges: &[WorkflowEdge],
    outputs: &HashMap<String, String>,
    in_degree: usize,
    initial_input: &str,
) -> String {
    if in_degree == 0 {
        return initial_input.to_string();
    }

    let upstream: Vec<&str> = edges
        .iter()
        .filter(|e| e.target == node_id)
        .filter_map(|e| outputs.get(&e.source).map(|s| s.as_str()))
        .collect();

    if upstream.is_empty() {
        // 上游尚未执行或全部失败 → 传递初始输入作为降级
        initial_input.to_string()
    } else {
        upstream.join("\n")
    }
}

/// 标记节点及其所有下游传递闭包为 Skipped。
///
/// 使用 BFS 遍历邻接表，将 `start_node` 及其所有可达下游节点加入 `skipped` 集合。
fn mark_skip_closure(
    start_node: &str,
    adjacency: &HashMap<String, Vec<(String, Option<String>)>>,
    skipped: &mut HashSet<String>,
) {
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(start_node.to_string());

    while let Some(node_id) = queue.pop_front() {
        if skipped.insert(node_id.clone()) {
            // 仅在首次插入时继续遍历下游
            if let Some(downstream) = adjacency.get(&node_id) {
                for (target, _) in downstream {
                    if !skipped.contains(target) {
                        queue.push_back(target.clone());
                    }
                }
            }
        }
    }
}

/// 将检索结果格式化为文本。
///
/// 每个结果包含文档名、分数和内容片段，以 `\n\n` 分隔。
fn format_retrieval_results(results: &[RetrievalResult]) -> String {
    if results.is_empty() {
        return "（未检索到相关内容）".to_string();
    }

    results
        .iter()
        .map(|r| {
            format!(
                "【来源：{}（相关度：{:.2}）】\n{}",
                r.doc_name, r.score, r.chunk.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// 收集 LLM 流式输出为完整字符串。
///
/// 逐 token 消费 `BoxStream`，拼接为完整文本。
/// 流中任何 token 错误都会导致整体返回 `Err`。
async fn collect_stream(mut stream: BoxStream<'_, Result<String>>) -> Result<String> {
    let mut output = String::new();
    while let Some(chunk) = stream.next().await {
        output.push_str(&chunk?);
    }
    Ok(output)
}
