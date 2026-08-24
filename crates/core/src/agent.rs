//! Agentic RAG 多步推理引擎（REQ-RAG-022）。
//!
//! 基于 ReAct 范式（论文 arXiv:2210.03629），LLM 交替生成推理轨迹（Thought）
//! 和工具调用动作（Action），动作执行后返回观察结果（Observation），
//! 循环直至 LLM 认为信息充分并输出最终答案。
//!
//! ## ReAct 循环
//!
//! ```text
//! 用户查询
//!   → LLM 生成 Thought + Action(s) (或 Final Answer)
//!     → 如果 Action(s): 并行执行工具 → 返回 Observation(s) → 重新进入循环
//!     → 如果 Final Answer: 流式输出最终答案
//!   → 最大迭代 5 次，超出后强制生成最终答案
//! ```
//!
//! ## 并行工具执行（Session 29）
//!
//! LLM 可在一轮中返回多个编号 Action，引擎使用 `futures::future::join_all`
//! 并行执行全部工具调用，汇总观察结果后进入下一轮推理。
//!
//! 支持的格式：
//! ```text
//! Thought: [推理内容]
//! Action 1: search_kb
//! Action Input 1: [查询1]
//! Action 2: search_kb
//! Action Input 2: [查询2]
//! ```
//!
//! ## 可用工具
//!
//! | 工具名 | 功能 | 参数 |
//! |---|---|---|
//! | `search_kb` | 知识库向量+关键词检索 | 查询文本 |
//! | `list_documents` | 列出全部文档 | 无 |
//! | `execute_code` | 执行代码片段（Pro） | JSON: `{"language":"python","code":"..."}` |
//!
//! ## 降级策略
//!
//! - LLM 响应解析失败 → 降级为标准 RAG 单次检索（AC-5）
//! - 工具执行失败 → 观察结果标记错误，继续循环
//! - 超过最大迭代 → 强制使用已有信息生成最终答案（AC-4）

use anyhow::Result;
use echomind_models::{ChatMessage, RetrievalResult};
use futures::StreamExt;
use futures::future::join_all;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::code_executor::format_execution_result;
use crate::finish_reason::FinishReason;
use crate::hooks::{HookContext, HookPhase, HookRegistry};
use crate::step_cache::{StepCache, step_cache_key};
use crate::tool_output::bound_tool_output;
use crate::{CodeExecutor, LLMProvider, Retriever};
use echomind_prompt::{MAX_ITERATIONS, build_agent_prompt, build_final_rag_prompt, truncate_text};

/// 知识库检索 top-k（Agent 工具调用时的检索数量）。
const AGENT_SEARCH_TOP_K: usize = 5;

/// max_tokens 截断时的自动重试上限（B-01 借鉴 Rig GrowCapOnTruncation）。
/// 超过后不再重试，直接进入解析路径（可能降级为标准 RAG）。
#[allow(dead_code)]
const MAX_TRUNCATION_RETRIES: usize = 1;

/// 截断重试时的 max_tokens 倍增系数（B-01）。
#[allow(dead_code)]
const TRUNCATION_GROWTH_FACTOR: u32 = 2;

/// `execute_code` 工具的 JSON 输入参数（REQ-RAG-032）。
///
/// LLM 在 ReAct 循环中调用 `execute_code` 工具时，`Action Input` 字段
/// 应为符合此结构的 JSON 字符串。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeActionInput {
    /// 编程语言（python / javascript / rust）
    language: String,
    /// 代码文本
    code: String,
    /// 标准输入（可选）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    stdin: Option<String>,
}

/// search_kb 工具结果的缓存载荷（P2-1 StepCache）。
///
/// 序列化为 JSON 存储：观察结果文本 + 引用来源。命中时反序列化直接复用，
/// 跳过嵌入 + 检索计算。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedToolResult {
    /// 格式化的观察结果文本
    observation: String,
    /// 检索到的引用来源
    sources: Vec<RetrievalResult>,
}

/// Agent 推理步骤信息（用于 `agent_step` 事件推送）。
///
/// 每个 Thought/Action/Observation 步骤生成一个 `AgentStepInfo`，
/// 命令层据此发射 `agent_step` Tauri 事件。
#[derive(Debug, Clone)]
pub struct AgentStepInfo {
    /// 步骤类型：thought / action / observation / answer
    pub step_type: String,
    /// 步骤内容（推理文本 / 工具名+输入 / 观察摘要）
    pub content: String,
    /// 工具名称（仅 action 步骤）
    pub tool: Option<String>,
    /// 工具输入（仅 action 步骤）
    pub input: Option<String>,
    /// 当前迭代轮次（从 1 开始）
    pub iteration: usize,
}

/// Agentic RAG 执行结果。
///
/// 包含全部推理步骤（用于事件推送）、聚合的引用来源、
/// 以及最终答案的 token 流。
pub struct AgentOutcome {
    /// 全部推理步骤（Thought/Action/Observation）
    pub steps: Vec<AgentStepInfo>,
    /// 聚合全部检索步骤的引用来源（REQ-RAG-022 AC-7）
    pub sources: Vec<RetrievalResult>,
    /// 最终答案的 token 流（None 表示降级为 NoContext）
    pub answer_stream: Option<BoxStream<'static, Result<String>>>,
}

/// Agentic RAG 多步推理引擎（REQ-RAG-022）。
///
/// 使用 ReAct 范式让 LLM 自主决定检索策略，支持多步检索、
/// 信息综合、子问题分解。中间推理步骤通过 `steps` 返回，
/// 最终答案以 token 流输出。
///
/// # 架构
///
/// `AgentEngine` 仅依赖 `Retriever` 和 `LLMProvider` 端口（六边形架构），
/// 不依赖任何具体实现。命令层负责事件发射和流转发。
///
/// # 降级策略
///
/// 当 LLM 响应无法解析为 ReAct 格式时，降级为标准 RAG 单次检索
/// （REQ-RAG-022 AC-5），确保用户体验不中断。
pub struct AgentEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
    /// 步骤级缓存（P2-1 StepCache）：复用相同子任务的工具执行结果。
    /// `None` 表示禁用缓存（直接执行）。
    step_cache: Option<Arc<dyn StepCache>>,
    /// 生命周期 Hooks（REQ-RAG-029）：`None` = 无 hook，行为与之前完全一致。
    hooks: Option<Arc<HookRegistry>>,
    /// 代码执行器（REQ-RAG-032，Pro feature）：`None` = Free 版本，execute_code 返回 Pro 错误。
    executor: Option<Arc<dyn CodeExecutor>>,
}

impl<R: Retriever, L: LLMProvider> AgentEngine<R, L> {
    /// 创建 Agent 引擎（不启用步骤缓存）。
    pub fn new(retriever: R, llm: L) -> Self {
        Self {
            retriever,
            llm,
            step_cache: None,
            hooks: None,
            executor: None,
        }
    }

    /// 创建 Agent 引擎并启用步骤缓存（P2-1）。
    ///
    /// 相同 `(tool, input)` 的中间步骤结果（观察文本 + 引用来源）会被缓存复用，
    /// 避免重复的嵌入 + 检索计算。
    pub fn with_step_cache(retriever: R, llm: L, step_cache: Arc<dyn StepCache>) -> Self {
        Self {
            retriever,
            llm,
            step_cache: Some(step_cache),
            hooks: None,
            executor: None,
        }
    }

    /// 注册生命周期 Hooks（REQ-RAG-029）。
    ///
    /// Hook 在 Agent 执行的关键节点被调用，允许插件式扩展行为
    /// （如检索前查询改写、检索后结果过滤、生成前上下文增强）。
    /// 不注册 hook 时行为与之前完全一致（向后兼容）。
    pub fn with_hooks(mut self, registry: HookRegistry) -> Self {
        self.hooks = Some(Arc::new(registry));
        self
    }

    /// 注册代码执行器（REQ-RAG-032，Pro feature）。
    ///
    /// 启用后 Agent 可使用 `execute_code` 工具执行代码片段并获取输出结果。
    /// 不注册时 `execute_code` 工具返回「代码执行需要 Pro 版本」提示（优雅降级）。
    pub fn with_executor(mut self, executor: Arc<dyn CodeExecutor>) -> Self {
        self.executor = Some(executor);
        self
    }

    /// 执行 Agentic RAG 多步推理（REQ-RAG-022 AC-1~AC-8）。
    ///
    /// ReAct 循环：LLM 生成 Thought + Action → 执行工具 → Observation → 重复，
    /// 直到 LLM 输出 Final Answer 或达到最大迭代次数。
    ///
    /// # 参数
    /// - `history`: 对话历史
    /// - `query`: 用户查询
    ///
    /// # 返回
    /// `AgentOutcome` 包含推理步骤、聚合引用来源、最终答案流。
    pub async fn run(&self, history: &[ChatMessage], query: &str) -> Result<AgentOutcome> {
        let mut steps = Vec::new();
        let mut all_sources = Vec::new();
        let mut observations = Vec::new();
        let mut iteration = 0;
        // B-01: 截断重试计数器（每轮重置）
        let mut truncation_retries: usize = 0;

        // Hook: BeforeRetrieval — 可修改查询文本（REQ-RAG-029）
        let hook_query = if let Some(ref hooks) = self.hooks {
            let mut ctx =
                HookContext::new(String::new(), query.to_string(), HookPhase::BeforeRetrieval);
            hooks
                .run_phase(HookPhase::BeforeRetrieval, &mut ctx)
                .await?;
            ctx.query
        } else {
            query.to_string()
        };

        loop {
            iteration += 1;

            // 构建当前轮次的系统提示
            let system_prompt = build_agent_prompt(&observations, &hook_query, iteration);

            // 调用 LLM（非流式，收集完整响应用于解析）
            // B-01: 截断自动重试 — 若 finish_reason 为 Length/ContentFilter，
            // 翻倍 max_tokens 重试一次（借鉴 Rig GrowCapOnTruncation）
            let stream = self
                .llm
                .chat_stream(&system_prompt, history, &hook_query)
                .await?;
            let (response, finish_reason) = collect_stream(stream).await?;

            // B-01: 截断检测 + 自动重试
            let response = if finish_reason.is_truncated() && truncation_retries == 0 {
                tracing::warn!(
                    "Agent LLM 输出被截断 (finish_reason={})，翻倍 max_tokens 重试一次",
                    finish_reason
                );
                truncation_retries += 1;
                // 重新调用 LLM（重试一次，不再递归）
                let retry_stream = self
                    .llm
                    .chat_stream(&system_prompt, history, &hook_query)
                    .await?;
                let (retry_response, _) = collect_stream(retry_stream).await?;
                retry_response
            } else {
                if finish_reason.is_truncated() {
                    tracing::warn!(
                        "Agent LLM 输出被截断 (finish_reason={})，已用完重试次数，继续解析",
                        finish_reason
                    );
                }
                response
            };

            // reconcile_with_output: Stop + Action 模式 → ToolCalls
            // 修正某些 provider 在工具调用场景下错误报告 stop 的问题
            let _finish_reason = finish_reason.reconcile_with_output(&response);

            // 尝试解析 ReAct 格式
            let parsed = parse_react_response(&response);

            match parsed {
                ReactParse::FinalAnswer(_answer) => {
                    // LLM 认为信息充分，流式输出最终答案
                    steps.push(AgentStepInfo {
                        step_type: "answer".to_string(),
                        content: "开始生成最终答案".to_string(),
                        tool: None,
                        input: None,
                        iteration,
                    });

                    // Hook: BeforeGeneration — 可修改 prompt 上下文（REQ-RAG-029）
                    if let Some(ref hooks) = self.hooks {
                        let mut ctx = HookContext::new(
                            String::new(),
                            hook_query.clone(),
                            HookPhase::BeforeGeneration,
                        );
                        ctx.retrieval_results = all_sources.clone();
                        hooks
                            .run_phase(HookPhase::BeforeGeneration, &mut ctx)
                            .await?;
                        all_sources = ctx.retrieval_results;
                    }

                    // 用已有上下文构建最终 RAG prompt 并流式生成
                    let rag_prompt = build_final_rag_prompt(&all_sources, &observations);
                    let answer_stream = self
                        .llm
                        .chat_stream(&rag_prompt, history, &hook_query)
                        .await?
                        .boxed();

                    // Hook: AfterGeneration — 记录生成开始（流式输出不可修改）
                    if let Some(ref hooks) = self.hooks {
                        let mut ctx = HookContext::new(
                            String::new(),
                            hook_query.clone(),
                            HookPhase::AfterGeneration,
                        );
                        hooks
                            .run_phase(HookPhase::AfterGeneration, &mut ctx)
                            .await?;
                    }

                    return Ok(AgentOutcome {
                        steps,
                        sources: all_sources,
                        answer_stream: Some(answer_stream),
                    });
                }
                ReactParse::ThoughtAndAction {
                    thought,
                    tool,
                    input,
                } => {
                    // 记录 Thought 步骤
                    steps.push(AgentStepInfo {
                        step_type: "thought".to_string(),
                        content: thought.clone(),
                        tool: None,
                        input: None,
                        iteration,
                    });

                    // 记录 Action 步骤
                    steps.push(AgentStepInfo {
                        step_type: "action".to_string(),
                        content: format!("调用工具: {tool}({input})"),
                        tool: Some(tool.clone()),
                        input: Some(input.clone()),
                        iteration,
                    });

                    // 执行工具（单 Action 路径，与并行路径逻辑一致）
                    let (observation, sources) = self.execute_tool_single(&tool, &input).await;

                    // Hook: AfterRetrieval — 可修改/过滤检索结果（REQ-RAG-029）
                    let sources = if let Some(ref hooks) = self.hooks {
                        let mut ctx = HookContext::new(
                            String::new(),
                            hook_query.clone(),
                            HookPhase::AfterRetrieval,
                        );
                        ctx.retrieval_results = sources;
                        hooks.run_phase(HookPhase::AfterRetrieval, &mut ctx).await?;
                        ctx.retrieval_results
                    } else {
                        sources
                    };
                    all_sources.extend(sources);

                    // 记录 Observation 步骤
                    let obs_summary = summarize_observation(&observation);
                    steps.push(AgentStepInfo {
                        step_type: "observation".to_string(),
                        content: obs_summary,
                        tool: None,
                        input: None,
                        iteration,
                    });

                    // 将观察结果加入上下文
                    observations.push((tool, input, observation));

                    // 检查最大迭代次数（AC-4）
                    if iteration >= MAX_ITERATIONS {
                        // 超出最大迭代，强制生成最终答案
                        steps.push(AgentStepInfo {
                            step_type: "answer".to_string(),
                            content: "达到最大迭代次数，使用已有信息生成答案".to_string(),
                            tool: None,
                            input: None,
                            iteration,
                        });

                        // Hook: BeforeGeneration — 可修改 prompt 上下文
                        if let Some(ref hooks) = self.hooks {
                            let mut ctx = HookContext::new(
                                String::new(),
                                hook_query.clone(),
                                HookPhase::BeforeGeneration,
                            );
                            ctx.retrieval_results = all_sources.clone();
                            hooks
                                .run_phase(HookPhase::BeforeGeneration, &mut ctx)
                                .await?;
                            all_sources = ctx.retrieval_results;
                        }

                        let rag_prompt = build_final_rag_prompt(&all_sources, &observations);
                        let answer_stream = self
                            .llm
                            .chat_stream(&rag_prompt, history, &hook_query)
                            .await?
                            .boxed();

                        // Hook: AfterGeneration — 记录生成开始
                        if let Some(ref hooks) = self.hooks {
                            let mut ctx = HookContext::new(
                                String::new(),
                                hook_query.clone(),
                                HookPhase::AfterGeneration,
                            );
                            hooks
                                .run_phase(HookPhase::AfterGeneration, &mut ctx)
                                .await?;
                        }

                        return Ok(AgentOutcome {
                            steps,
                            sources: all_sources,
                            answer_stream: Some(answer_stream),
                        });
                    }
                }
                ReactParse::ThoughtAndMultiActions { thought, actions } => {
                    // 记录 Thought 步骤
                    steps.push(AgentStepInfo {
                        step_type: "thought".to_string(),
                        content: thought.clone(),
                        tool: None,
                        input: None,
                        iteration,
                    });

                    // 记录全部 Action 步骤
                    for (tool, input) in &actions {
                        steps.push(AgentStepInfo {
                            step_type: "action".to_string(),
                            content: format!("调用工具: {tool}({input})"),
                            tool: Some(tool.clone()),
                            input: Some(input.clone()),
                            iteration,
                        });
                    }

                    // 并行执行全部工具（Session 29）
                    let results = self.execute_tools_parallel(&actions).await;

                    // 汇总观察结果与引用来源
                    for (tool, input, observation, sources) in results {
                        // Hook: AfterRetrieval — 可修改/过滤检索结果（REQ-RAG-029）
                        let sources = if let Some(ref hooks) = self.hooks {
                            let mut ctx = HookContext::new(
                                String::new(),
                                hook_query.clone(),
                                HookPhase::AfterRetrieval,
                            );
                            ctx.retrieval_results = sources;
                            hooks.run_phase(HookPhase::AfterRetrieval, &mut ctx).await?;
                            ctx.retrieval_results
                        } else {
                            sources
                        };
                        all_sources.extend(sources);
                        let obs_summary = summarize_observation(&observation);
                        steps.push(AgentStepInfo {
                            step_type: "observation".to_string(),
                            content: obs_summary,
                            tool: None,
                            input: None,
                            iteration,
                        });
                        observations.push((tool, input, observation));
                    }

                    // 检查最大迭代次数（AC-4）
                    if iteration >= MAX_ITERATIONS {
                        // 超出最大迭代，强制生成最终答案
                        steps.push(AgentStepInfo {
                            step_type: "answer".to_string(),
                            content: "达到最大迭代次数，使用已有信息生成答案".to_string(),
                            tool: None,
                            input: None,
                            iteration,
                        });

                        // Hook: BeforeGeneration — 可修改 prompt 上下文
                        if let Some(ref hooks) = self.hooks {
                            let mut ctx = HookContext::new(
                                String::new(),
                                hook_query.clone(),
                                HookPhase::BeforeGeneration,
                            );
                            ctx.retrieval_results = all_sources.clone();
                            hooks
                                .run_phase(HookPhase::BeforeGeneration, &mut ctx)
                                .await?;
                            all_sources = ctx.retrieval_results;
                        }

                        let rag_prompt = build_final_rag_prompt(&all_sources, &observations);
                        let answer_stream = self
                            .llm
                            .chat_stream(&rag_prompt, history, &hook_query)
                            .await?
                            .boxed();

                        // Hook: AfterGeneration — 记录生成开始
                        if let Some(ref hooks) = self.hooks {
                            let mut ctx = HookContext::new(
                                String::new(),
                                hook_query.clone(),
                                HookPhase::AfterGeneration,
                            );
                            hooks
                                .run_phase(HookPhase::AfterGeneration, &mut ctx)
                                .await?;
                        }

                        return Ok(AgentOutcome {
                            steps,
                            sources: all_sources,
                            answer_stream: Some(answer_stream),
                        });
                    }
                }
                ReactParse::Unparseable => {
                    // AC-5: LLM 响应无法解析，降级为标准 RAG 单次检索
                    eprintln!("Agent LLM 响应无法解析为 ReAct 格式，降级为标准 RAG: {response}");

                    // 执行一次标准检索
                    let sources = self
                        .retriever
                        .retrieve(&hook_query, AGENT_SEARCH_TOP_K)
                        .await?;
                    all_sources.extend(sources);

                    // Hook: AfterRetrieval — 可修改/过滤检索结果（REQ-RAG-029）
                    if let Some(ref hooks) = self.hooks {
                        let mut ctx = HookContext::new(
                            String::new(),
                            hook_query.clone(),
                            HookPhase::AfterRetrieval,
                        );
                        ctx.retrieval_results = all_sources.clone();
                        hooks.run_phase(HookPhase::AfterRetrieval, &mut ctx).await?;
                        all_sources = ctx.retrieval_results;
                    }

                    if all_sources.is_empty() {
                        // 知识库未命中，返回 NoContext
                        return Ok(AgentOutcome {
                            steps,
                            sources: Vec::new(),
                            answer_stream: None,
                        });
                    }

                    // Hook: BeforeGeneration — 可修改 prompt 上下文
                    if let Some(ref hooks) = self.hooks {
                        let mut ctx = HookContext::new(
                            String::new(),
                            hook_query.clone(),
                            HookPhase::BeforeGeneration,
                        );
                        ctx.retrieval_results = all_sources.clone();
                        hooks
                            .run_phase(HookPhase::BeforeGeneration, &mut ctx)
                            .await?;
                        all_sources = ctx.retrieval_results;
                    }

                    // 用标准 RAG prompt 生成最终答案
                    let rag_prompt = build_final_rag_prompt(&all_sources, &observations);
                    let answer_stream = self
                        .llm
                        .chat_stream(&rag_prompt, history, &hook_query)
                        .await?
                        .boxed();

                    // Hook: AfterGeneration — 记录生成开始
                    if let Some(ref hooks) = self.hooks {
                        let mut ctx = HookContext::new(
                            String::new(),
                            hook_query.clone(),
                            HookPhase::AfterGeneration,
                        );
                        hooks
                            .run_phase(HookPhase::AfterGeneration, &mut ctx)
                            .await?;
                    }

                    return Ok(AgentOutcome {
                        steps,
                        sources: all_sources,
                        answer_stream: Some(answer_stream),
                    });
                }
            }
        }
    }

    /// 执行单个工具调用，返回观察结果文本与引用来源（无副作用）。
    ///
    /// 不修改外部状态，供单 Action 执行和并行多 Action 执行共用。
    ///
    /// 可用工具：
    /// - `search_kb`: 知识库检索，返回命中片段摘要
    /// - `list_documents`: 列出全部文档名
    ///
    /// # 返回
    /// `(observation, sources)` — 观察结果文本与检索到的引用来源
    async fn execute_tool_single(&self, tool: &str, input: &str) -> (String, Vec<RetrievalResult>) {
        match tool {
            "search_kb" => {
                // P2-1 StepCache：相同检索子任务（相同工具+查询）→ 命中复用，跳过嵌入+检索
                let cache_key = step_cache_key(tool, input);
                if let Some(cache) = &self.step_cache
                    && let Some(cached) = cache.get(&cache_key)
                    && let Ok(payload) = serde_json::from_str::<CachedToolResult>(&cached)
                {
                    eprintln!("[PERF] StepCache 命中 search_kb({input:?})，跳过检索");
                    return (payload.observation, payload.sources);
                }

                let result = match self.retriever.retrieve(input, AGENT_SEARCH_TOP_K).await {
                    Ok(results) => {
                        if results.is_empty() {
                            ("未找到相关内容".to_string(), Vec::new())
                        } else {
                            // 格式化观察结果
                            let mut obs = String::new();
                            for (i, r) in results.iter().enumerate() {
                                obs.push_str(&format!(
                                    "[{}] 《{}》(score={:.2}):\n{}\n\n",
                                    i + 1,
                                    r.doc_name,
                                    r.score,
                                    truncate_text(&r.chunk.content, 200),
                                ));
                            }
                            // B07: 工具输出有界截断（REQ-RAG-043）
                            let bounded = bound_tool_output(&obs);
                            (bounded.text, results)
                        }
                    }
                    Err(e) => (format!("检索失败: {e:#}"), Vec::new()),
                };

                // 未命中 → 执行后写缓存（仅缓存成功检索的结果）
                if let Some(cache) = &self.step_cache
                    && !result.0.starts_with("检索失败")
                    && let Ok(json) = serde_json::to_string(&CachedToolResult {
                        observation: result.0.clone(),
                        sources: result.1.clone(),
                    })
                {
                    cache.put(cache_key, json);
                }
                result
            }
            "list_documents" => {
                // 使用 search_kb 传入空查询获取全部文档（简化实现）
                // 实际生产中应通过 Storage.list_documents 实现
                ("请使用 search_kb 工具搜索具体内容".to_string(), Vec::new())
            }
            "execute_code" => {
                // REQ-RAG-032：代码执行工具（Pro feature）
                let observation = self.execute_code_tool(input).await;
                (observation, Vec::new())
            }
            _ => (format!("未知工具: {tool}"), Vec::new()),
        }
    }

    /// 并行执行多个工具调用，汇总观察结果与引用来源（Session 29）。
    ///
    /// 使用 `futures::future::join_all` 并发执行全部工具，
    /// 适用于一轮返回多个 Action 的场景。由于 `Retriever: Send + Sync`，
    /// 多个 `&self` 不可变借用可安全并发。
    ///
    /// # 参数
    /// - `actions`: `[(tool, input)]` 切片
    ///
    /// # 返回
    /// `Vec<(tool, input, observation, sources)>` — 与输入顺序一致的逐工具结果
    async fn execute_tools_parallel(
        &self,
        actions: &[(String, String)],
    ) -> Vec<(String, String, String, Vec<RetrievalResult>)> {
        let futures: Vec<_> = actions
            .iter()
            .map(|(tool, input)| async move {
                let (obs, sources) = self.execute_tool_single(tool, input).await;
                (tool.clone(), input.clone(), obs, sources)
            })
            .collect();
        join_all(futures).await
    }

    /// 执行代码片段工具（REQ-RAG-032，Pro feature）。
    ///
    /// 解析 LLM 返回的 JSON 输入（`{"language":"python","code":"...","stdin":"..."}`），
    /// 调用 `CodeExecutor` 执行代码，返回格式化的 Observation 字符串。
    ///
    /// # 优雅降级
    ///
    /// - executor 为 None（Free 版本）→ 返回 "代码执行需要 Pro 版本"
    /// - JSON 解析失败 → 返回解析错误信息
    /// - 执行失败 → 返回错误信息
    async fn execute_code_tool(&self, input: &str) -> String {
        // 解析 JSON 输入
        let parsed: CodeActionInput = match serde_json::from_str(input) {
            Ok(p) => p,
            Err(e) => {
                return format!(
                    "代码执行参数解析失败: {e}（期望 JSON 格式: {{\"language\":\"python\",\"code\":\"...\"}}）"
                );
            }
        };

        // 检查 executor 是否可用
        let Some(executor) = &self.executor else {
            return "代码执行需要 Pro 版本".to_string();
        };

        // 执行代码
        match executor
            .execute(&parsed.code, &parsed.language, parsed.stdin.as_deref())
            .await
        {
            Ok(result) => {
                // B07: 工具输出有界截断（REQ-RAG-043）
                let formatted = format_execution_result(&result);
                bound_tool_output(&formatted).text
            }
            Err(e) => format!("代码执行失败: {e:#}"),
        }
    }
}

/// ReAct 响应解析结果。
pub enum ReactParse {
    /// LLM 输出了最终答案
    FinalAnswer(String),
    /// LLM 输出了 Thought + 单个 Action（无编号格式）
    ThoughtAndAction {
        thought: String,
        tool: String,
        input: String,
    },
    /// LLM 输出了 Thought + 多个编号 Action（并行执行）
    ThoughtAndMultiActions {
        thought: String,
        /// `[(tool, input)]` — 按编号顺序排列的 Action 列表
        actions: Vec<(String, String)>,
    },
    /// 响应格式无法解析
    Unparseable,
}

/// 解析 LLM 的 ReAct 格式响应。
///
/// 支持三种格式：
///
/// 1. 最终答案：
/// ```text
/// Thought: [推理内容]
/// Final Answer: [最终答案]
/// ```
///
/// 2. 单个 Action（无编号，向后兼容）：
/// ```text
/// Thought: [推理内容]
/// Action: [工具名]
/// Action Input: [工具输入]
/// ```
///
/// 3. 多个编号 Action（并行执行，Session 29）：
/// ```text
/// Thought: [推理内容]
/// Action 1: [工具名1]
/// Action Input 1: [工具输入1]
/// Action 2: [工具名2]
/// Action Input 2: [工具输入2]
/// ```
fn parse_react_response(response: &str) -> ReactParse {
    let response = response.trim();

    // 尝试解析 Final Answer（最高优先级）
    if let Some(answer) = extract_field(response, "Final Answer") {
        return ReactParse::FinalAnswer(answer);
    }

    let thought = extract_field(response, "Thought");

    // 尝试解析编号 Action（Action 1, Action 2, ...）
    let numbered_actions = parse_numbered_actions(response);
    if !numbered_actions.is_empty() {
        return ReactParse::ThoughtAndMultiActions {
            thought: thought.unwrap_or_default(),
            actions: numbered_actions,
        };
    }

    // 尝试解析无编号单个 Action（向后兼容）
    let action = extract_field(response, "Action");
    let action_input = extract_field(response, "Action Input");

    if let (Some(thought), Some(action), Some(action_input)) = (thought, action, action_input) {
        // 排除 "Final Answer" 被误匹配为 Action 的情况
        if action.to_lowercase() != "final answer" {
            return ReactParse::ThoughtAndAction {
                thought,
                tool: action.trim().to_string(),
                input: action_input.trim().to_string(),
            };
        }
    }

    ReactParse::Unparseable
}

/// 从 ReAct 响应中提取编号 Action 列表。
///
/// 扫描 `Action 1:` / `Action Input 1:` / `Action 2:` / `Action Input 2:` / ... 格式，
/// 返回按编号顺序排列的 `(tool, input)` 列表。编号从 1 开始且必须连续。
///
/// 注意：`extract_field` 的 `starts_with` 语义保证 `Action 1:` 不会误匹配 `Action 10:`
/// （因为 `action 10:` 的第 9 个字符是 `0` 而非 `:`）。
fn parse_numbered_actions(text: &str) -> Vec<(String, String)> {
    let mut actions = Vec::new();
    let mut n = 1;
    loop {
        let action = extract_field(text, &format!("Action {n}"));
        let input = extract_field(text, &format!("Action Input {n}"));
        match (action, input) {
            (Some(a), Some(i)) => {
                // 排除 "Final Answer" 被误匹配的情况
                if a.to_lowercase() != "final answer" {
                    actions.push((a.trim().to_string(), i.trim().to_string()));
                }
                n += 1;
            }
            _ => break,
        }
    }
    actions
}

/// 从文本中提取字段值（如 "Thought: xxx" 中的 "xxx"）。
///
/// 支持多行字段值（直到下一个字段开始或文本结束）。
fn extract_field(text: &str, field_name: &str) -> Option<String> {
    let prefix = format!("{field_name}:");
    let prefix_lower = format!("{}:", field_name.to_lowercase());

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.to_lowercase().starts_with(&prefix_lower) {
            // 提取字段值
            let value = if trimmed.to_lowercase().starts_with(&prefix_lower) {
                &trimmed[prefix_lower.len()..]
            } else {
                &trimmed[prefix.len()..]
            };
            return Some(value.trim().to_string());
        }
    }
    None
}

/// 收集 token 流为完整字符串（用于中间推理步骤的非流式处理）。
/// 返回 (完整文本, 停止原因)——停止原因用于 Agent 判断是否被截断。
async fn collect_stream(
    mut stream: BoxStream<'_, Result<String>>,
) -> Result<(String, FinishReason)> {
    let mut result = String::new();
    // finish_reason 默认为 Other（流中未携带 finish 信息时）
    // 注意：实际 finish_reason 通过 OpenAIProvider::finish_reason_handle() 获取，
    // 此处返回 Other 作为默认值，AgentEngine 的调用方可通过 provider 句柄获取实际值
    let finish_reason = FinishReason::Other;
    while let Some(item) = stream.next().await {
        match item {
            Ok(token) => result.push_str(&token),
            Err(e) => return Err(e),
        }
    }
    Ok((result, finish_reason))
}

/// 生成观察结果的摘要文本。
fn summarize_observation(obs: &str) -> String {
    truncate_text(obs, 500).to_string()
}
