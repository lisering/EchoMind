//! 多代理协调引擎（REQ-RAG-025）。
//!
//! 借鉴 Claude Code 的 Coordinator Mode，将复杂查询分解为子查询，
//! 并行检索后综合分析，再流式生成最终答案。
//!
//! ## 四阶段流程
//!
//! 1. **Research**：将用户查询分解为子查询（通过 LLM 或规则），每个子查询并行检索
//! 2. **Synthesis**：将所有研究发现发送给 LLM，生成综合分析（非流式 `complete()`）
//! 3. **Implementation**：基于综合分析构建最终 RAG prompt，流式生成答案
//! 4. **Verification**：验证答案引用了所有相关来源
//!
//! ## 降级策略
//!
//! - 子查询分解失败 → 使用原始查询做单次检索（等价于标准 RAG）
//! - 综合分析失败 → 使用标准 RAG 单轮检索
//! - 所有阶段失败 → 返回 NoContext

use anyhow::Result;
use echomind_models::{ChatMessage, RetrievalResult};
use futures::StreamExt;
use futures::future::join_all;
use futures::stream::BoxStream;
use std::sync::Arc;

use crate::hooks::{HookContext, HookPhase, HookRegistry};
use crate::step_cache::{StepCache, step_cache_key};
use crate::{LLMProvider, Retriever};
use echomind_prompt::{build_rag_prompt_segmented, truncate_text};

/// 每个 worker 的检索 top-k（默认值）。
const DEFAULT_RESEARCH_TOP_K: usize = 5;

/// 子查询分解系统提示词。
const SUBQUERY_SYSTEM_PROMPT: &str = "你是一个查询分析器。请将用户的问题分解为 1-3 个独立的子查询，\
    每个子查询用于在知识库中检索不同方面的信息。\n\
    按以下 JSON 数组格式输出，不要添加其他说明：\n\
    [\"子查询1\", \"子查询2\", \"子查询3\"]\n\
    如果问题简单到不需要分解，返回只包含原始问题的数组。";

/// 综合分析系统提示词。
const SYNTHESIS_SYSTEM_PROMPT: &str = "你是知识综合分析器。请根据以下多个信息源，\
    生成一份综合分析报告。报告中应指出各信息源之间的关联、矛盾和互补关系。\
    直接输出分析内容，不要添加额外说明。";

/// Coordinator 阶段信息（用于 `coordinator_phase` 事件推送）。
#[derive(Debug, Clone)]
pub struct CoordinatorPhaseInfo {
    /// 阶段标识：researching / synthesizing / generating
    pub phase: String,
    /// 可读消息（展示给用户的进度文案）
    pub message: String,
    /// 子查询数量（仅 researching 阶段有值）
    pub sub_query_count: Option<usize>,
}

/// Coordinator 执行结果。
pub struct CoordinatorOutcome {
    /// 全部阶段信息（用于事件推送）
    pub phases: Vec<CoordinatorPhaseInfo>,
    /// 聚合全部检索步骤的引用来源
    pub sources: Vec<RetrievalResult>,
    /// 最终答案的 token 流（None 表示降级为 NoContext）
    pub answer_stream: Option<BoxStream<'static, Result<String>>>,
}

/// 多代理协调引擎配置。
#[derive(Debug, Clone)]
pub struct CoordinatorConfig {
    /// 最大并行 worker 数（默认 3）
    pub max_workers: usize,
    /// 每个 worker 检索 top-k（默认 5）
    pub research_top_k: usize,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_workers: 3,
            research_top_k: DEFAULT_RESEARCH_TOP_K,
        }
    }
}

/// 多代理协调引擎（REQ-RAG-025）。
///
/// 将复杂查询分解为子查询，并行检索后综合分析，再流式生成最终答案。
/// 适用于「对比这三份合同的法律风险」等需要多维度检索的复杂研究查询。
///
/// # 架构
///
/// `CoordinatorEngine` 仅依赖 `Retriever` 和 `LLMProvider` 端口（六边形架构），
/// 不依赖任何具体实现。命令层负责事件发射和流转发。
///
/// # 降级策略
///
/// - 子查询分解失败 → 使用原始查询做单次检索（等价于标准 RAG）
/// - 综合分析失败 → 使用标准 RAG 单轮检索
/// - 所有阶段失败 → 返回 NoContext
pub struct CoordinatorEngine<R: Retriever, L: LLMProvider> {
    retriever: R,
    llm: L,
    config: CoordinatorConfig,
    /// 步骤级缓存（P2-1 StepCache）：复用子查询分解 / 检索 / 综合分析结果。
    /// `None` 表示禁用缓存（直接执行）。
    step_cache: Option<Arc<dyn StepCache>>,
    /// 生命周期 Hooks（REQ-RAG-029）：`None` = 无 hook，行为与之前完全一致。
    hooks: Option<Arc<HookRegistry>>,
    /// 子代理超时秒数（None = 不启用子代理，使用标准并行检索；
    /// Some(secs) = 启用子代理舰队，每个子查询由独立 SubAgent 处理）。
    sub_agent_timeout: Option<u64>,
}

impl<R: Retriever, L: LLMProvider> CoordinatorEngine<R, L> {
    /// 创建 Coordinator 引擎（不启用步骤缓存）。
    pub fn new(retriever: R, llm: L) -> Self {
        Self {
            retriever,
            llm,
            config: CoordinatorConfig::default(),
            step_cache: None,
            hooks: None,
            sub_agent_timeout: None,
        }
    }

    /// 使用指定配置创建 Coordinator 引擎。
    pub fn with_config(retriever: R, llm: L, config: CoordinatorConfig) -> Self {
        Self {
            retriever,
            llm,
            config,
            step_cache: None,
            hooks: None,
            sub_agent_timeout: None,
        }
    }

    /// 创建 Coordinator 引擎并启用步骤缓存（P2-1）。
    ///
    /// 缓存三个昂贵阶段的结果：子查询分解（LLM 调用）、逐子查询检索（嵌入+检索）、
    /// 综合分析（LLM 调用）。相同输入直接复用，跳过重复计算。
    pub fn with_step_cache(retriever: R, llm: L, step_cache: Arc<dyn StepCache>) -> Self {
        Self {
            retriever,
            llm,
            config: CoordinatorConfig::default(),
            step_cache: Some(step_cache),
            hooks: None,
            sub_agent_timeout: None,
        }
    }

    /// 注册生命周期 Hooks（REQ-RAG-029）。
    ///
    /// Hook 在 Coordinator 执行的关键节点被调用，允许插件式扩展行为
    /// （如检索前查询改写、检索后结果过滤、生成前上下文增强）。
    /// 不注册 hook 时行为与之前完全一致（向后兼容）。
    pub fn with_hooks(mut self, registry: HookRegistry) -> Self {
        self.hooks = Some(Arc::new(registry));
        self
    }

    /// 启用子代理舰队模式（REQ-RAG-025 扩展）。
    ///
    /// 启用后，Research 阶段的每个子查询由独立的 `SubAgent` 处理，
    /// 通过 mailbox 消息传递与主代理协调，支持「分工→并行→汇总」模式。
    ///
    /// # 参数
    /// - `secs`: 每个子代理的超时秒数（建议 60-180s）
    ///
    /// # 向后兼容
    ///
    /// 不调用此方法时，`sub_agent_timeout` 为 `None`，Research 阶段使用
    /// 原有的并行检索行为（`join_all` + `retrieve()`），完全一致。
    pub fn with_sub_agent_timeout(mut self, secs: u64) -> Self {
        self.sub_agent_timeout = Some(secs);
        self
    }

    /// 执行多代理协调查询（四阶段流水线）。
    ///
    /// # 参数
    /// - `history`: 对话历史
    /// - `query`: 用户查询
    ///
    /// # 返回
    /// `CoordinatorOutcome` 包含阶段信息、聚合引用来源、最终答案流。
    pub async fn run(&self, history: &[ChatMessage], query: &str) -> Result<CoordinatorOutcome> {
        let mut phases = Vec::new();
        let mut all_sources = Vec::new();

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

        // ---- Phase 1: Research（子查询分解 + 并行检索）----
        phases.push(CoordinatorPhaseInfo {
            phase: "researching".to_string(),
            message: "正在分解查询并并行检索…".to_string(),
            sub_query_count: None,
        });

        let sub_queries = match self.decompose_query(&hook_query).await {
            Ok(queries) if !queries.is_empty() => queries,
            _ => vec![hook_query.clone()],
        };

        let sub_count = sub_queries.len();
        // 更新 researching 阶段的子查询数量
        if !phases.is_empty() {
            phases[0].sub_query_count = Some(sub_count);
        }

        // 并行检索所有子查询
        let research_findings = self.research_parallel(&sub_queries).await?;

        for (_, sources) in &research_findings {
            all_sources.extend(sources.clone());
        }

        // 去重 sources（按 chunk.id）
        all_sources = dedup_sources(all_sources);

        // Hook: AfterRetrieval — 可修改/过滤检索结果（REQ-RAG-029）
        if let Some(ref hooks) = self.hooks {
            let mut ctx =
                HookContext::new(String::new(), hook_query.clone(), HookPhase::AfterRetrieval);
            ctx.retrieval_results = all_sources.clone();
            hooks.run_phase(HookPhase::AfterRetrieval, &mut ctx).await?;
            all_sources = ctx.retrieval_results;
        }

        if all_sources.is_empty() {
            // 知识库未命中，返回 NoContext
            return Ok(CoordinatorOutcome {
                phases,
                sources: Vec::new(),
                answer_stream: None,
            });
        }

        // ---- Phase 2: Synthesis（综合分析）----
        phases.push(CoordinatorPhaseInfo {
            phase: "synthesizing".to_string(),
            message: "正在综合分析多源信息…".to_string(),
            sub_query_count: Some(sub_count),
        });

        let synthesis = self.generate_synthesis(&research_findings).await;

        // ---- Phase 3: Implementation（流式生成最终答案）----
        phases.push(CoordinatorPhaseInfo {
            phase: "generating".to_string(),
            message: "正在生成最终答案…".to_string(),
            sub_query_count: Some(sub_count),
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

        // 构建最终 RAG prompt（使用分段式架构以利用 prompt caching）
        let segmented = build_rag_prompt_segmented(&all_sources);

        // 如果综合分析成功，将综合摘要注入 dynamic_context 前面
        let (static_prefix, dynamic_context) = match &synthesis {
            Ok(syn) => (
                segmented.static_prefix.clone(),
                format!(
                    "[综合分析]\n{syn}\n\n{dynamic}",
                    dynamic = segmented.dynamic_context
                ),
            ),
            Err(_) => (segmented.static_prefix, segmented.dynamic_context),
        };

        let answer_stream = self
            .llm
            .chat_stream_segmented(&static_prefix, &dynamic_context, history, &hook_query)
            .await?
            .boxed();

        // Hook: AfterGeneration — 记录生成开始（流式输出不可修改，REQ-RAG-029）
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

        Ok(CoordinatorOutcome {
            phases,
            sources: all_sources,
            answer_stream: Some(answer_stream),
        })
    }

    /// Phase 1a: 使用 LLM 将用户查询分解为子查询。
    ///
    /// 通过 `chat_stream` 发送分解指令给 LLM，收集完整响应后解析 JSON 数组。
    /// 解析失败时返回只包含原始查询的数组（降级）。
    ///
    /// P2-1 StepCache：相同查询的分解结果直接复用，跳过 LLM 调用。
    async fn decompose_query(&self, query: &str) -> Result<Vec<String>> {
        let cache_key = step_cache_key("decompose", query);
        if let Some(cache) = &self.step_cache
            && let Some(cached) = cache.get(&cache_key)
            && let Ok(queries) = serde_json::from_str::<Vec<String>>(&cached)
        {
            eprintln!("[PERF] StepCache 命中 decompose({query:?})，跳过 LLM 分解");
            return Ok(queries);
        }

        let stream = self
            .llm
            .chat_stream(SUBQUERY_SYSTEM_PROMPT, &[], query)
            .await?;
        let response = collect_stream(stream).await?;

        // 尝试解析 JSON 数组
        let trimmed = response.trim();

        // 尝试直接解析 JSON
        let sub_queries = if let Ok(queries) = serde_json::from_str::<Vec<String>>(trimmed) {
            let filtered: Vec<String> = queries
                .into_iter()
                .filter(|q| !q.trim().is_empty())
                .take(self.config.max_workers)
                .collect();
            if filtered.is_empty() {
                // 降级：返回只包含原始查询的数组
                vec![query.to_string()]
            } else {
                filtered
            }
        } else {
            // 降级：返回只包含原始查询的数组
            vec![query.to_string()]
        };

        // 未命中 → 写缓存供下次复用
        if let Some(cache) = &self.step_cache
            && let Ok(json) = serde_json::to_string(&sub_queries)
        {
            cache.put(cache_key, json);
        }

        Ok(sub_queries)
    }

    /// Phase 1b: 并行执行所有子查询的检索。
    ///
    /// 启用子代理模式时（`sub_agent_timeout.is_some()`），每个子查询由独立的
    /// `SubAgent`（`AgentEngine` ReAct 循环）处理；禁用时退回原有行为
    /// （直接 `retrieve` + StepCache）。
    async fn research_parallel(
        &self,
        sub_queries: &[String],
    ) -> Result<Vec<(String, Vec<RetrievalResult>)>> {
        // 子代理舰队模式：每个子查询由独立 AgentEngine 处理
        if self.sub_agent_timeout.is_some() {
            return self.research_with_sub_agents(sub_queries).await;
        }

        // 原有逻辑：并行 retrieve + StepCache
        let futures: Vec<_> = sub_queries
            .iter()
            .map(|sq| {
                let cache_key = step_cache_key("search_kb", sq);
                async move {
                    // P2-1 StepCache：命中 → 直接复用上次检索结果
                    if let Some(cache) = &self.step_cache
                        && let Some(cached) = cache.get(&cache_key)
                        && let Ok(sources) = serde_json::from_str::<Vec<RetrievalResult>>(&cached)
                    {
                        eprintln!("[PERF] StepCache 命中 search_kb({sq:?})，跳过检索");
                        return (sq.clone(), sources);
                    }

                    let sources = self
                        .retriever
                        .retrieve(sq, self.config.research_top_k)
                        .await
                        .unwrap_or_default();

                    // 未命中 → 写缓存供下次复用
                    if let Some(cache) = &self.step_cache
                        && let Ok(json) = serde_json::to_string(&sources)
                    {
                        cache.put(cache_key, json);
                    }

                    (sq.clone(), sources)
                }
            })
            .collect();

        let results = join_all(futures).await;
        Ok(results)
    }

    /// Phase 1b (子代理模式): 使用子代理舰队并行执行子查询。
    ///
    /// 每个子查询由独立的 `SubAgent` 处理，通过 mailbox 消息传递与主代理协调。
    /// 子代理共享 `&self.retriever` 和 `&self.llm` 的不可变引用
    /// （`Retriever: Send + Sync` / `LLMProvider: Send + Sync` 保证并发安全）。
    ///
    /// # 容错
    ///
    /// 单个子代理失败（`Failed` / `TimedOut`）不影响其他子代理，
    /// 其 `sources` 为空 `Vec`，`output` 包含错误信息。
    async fn research_with_sub_agents(
        &self,
        sub_queries: &[String],
    ) -> Result<Vec<(String, Vec<RetrievalResult>)>> {
        let timeout = self.sub_agent_timeout;
        let max_agents = self.config.max_workers;
        let mut fleet = crate::sub_agent::SubAgentFleet::with_max_agents(max_agents);

        // 构建 agent_name → sub_query 映射，用于结果匹配
        let mut name_to_query: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let tasks: Vec<(String, String)> = sub_queries
            .iter()
            .enumerate()
            .map(|(i, sq)| {
                let name = format!("worker_{}", i + 1);
                name_to_query.insert(name.clone(), sq.clone());
                (name, sq.clone())
            })
            .collect();

        let agents = fleet.dispatch(tasks)?;

        // 并发执行所有子代理（使用 join_all，共享 &self 不可变引用）
        let agent_futures: Vec<_> = agents
            .into_iter()
            .map(|agent| agent.run(&self.retriever, &self.llm, timeout))
            .collect();

        join_all(agent_futures).await;

        let results = fleet.join_all().await;

        // 将 SubAgentResult 转换为 (sub_query, sources) 格式
        Ok(results
            .into_iter()
            .map(|r| {
                let sq = name_to_query
                    .get(&r.agent_name)
                    .cloned()
                    .unwrap_or_else(|| r.agent_name.clone());
                (sq, r.sources)
            })
            .collect())
    }

    /// Phase 2: 生成综合分析。
    ///
    /// 将所有研究发现拼接为文本，通过 `chat_stream` 发送给 LLM，
    /// 收集流式输出为完整综合分析字符串。
    ///
    /// P2-1 StepCache：相同研究发现（子查询 + 命中片段）→ 复用综合分析，跳过 LLM 调用。
    async fn generate_synthesis(
        &self,
        findings: &[(String, Vec<RetrievalResult>)],
    ) -> Result<String> {
        let mut research_text = String::new();
        for (i, (sq, sources)) in findings.iter().enumerate() {
            research_text.push_str(&format!("--- 子查询 {i}：{sq} ---\n"));
            for (j, src) in sources.iter().enumerate() {
                research_text.push_str(&format!(
                    "  来源 {j}：《{}》(score={:.2}):\n{}\n\n",
                    src.doc_name,
                    src.score,
                    truncate_text(&src.chunk.content, 300),
                ));
            }
        }

        // P2-1 StepCache：以研究发现文本为键，命中 → 直接复用上次综合分析
        let cache_key = step_cache_key("synthesis", &research_text);
        if let Some(cache) = &self.step_cache
            && let Some(cached) = cache.get(&cache_key)
        {
            eprintln!("[PERF] StepCache 命中 synthesis，跳过 LLM 综合分析");
            return Ok(cached);
        }

        let stream = self
            .llm
            .chat_stream(
                SYNTHESIS_SYSTEM_PROMPT,
                &[],
                &format!("请综合分析以下多源信息：\n\n{research_text}"),
            )
            .await?;

        let synthesis = collect_stream(stream).await?;
        let trimmed = synthesis.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("综合分析为空"));
        }

        // 未命中 → 写缓存供下次复用
        if let Some(cache) = &self.step_cache {
            cache.put(cache_key, trimmed.to_string());
        }

        Ok(trimmed.to_string())
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 收集 token 流为完整字符串（用于中间步骤的非流式处理）。
async fn collect_stream(mut stream: BoxStream<'_, Result<String>>) -> Result<String> {
    let mut result = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(token) => result.push_str(&token),
            Err(e) => return Err(e),
        }
    }
    Ok(result)
}

/// 按 chunk.id 去重引用来源（保留首次出现）。
fn dedup_sources(sources: Vec<RetrievalResult>) -> Vec<RetrievalResult> {
    let mut seen = std::collections::HashSet::new();
    sources
        .into_iter()
        .filter(|s| seen.insert(s.chunk.id.clone()))
        .collect()
}
