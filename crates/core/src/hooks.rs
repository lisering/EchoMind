//! Agent 生命周期 Hooks 系统（借鉴 Bamboo-agent 生命周期 hooks，REQ-RAG-029）。
//!
//! 在 `AgentEngine` 和 `CoordinatorEngine` 的关键执行节点插入可扩展的 hook 点，
//! 允许插件式注入逻辑（如检索前自动查询改写、生成后事实核查），无需修改核心引擎代码。
//!
//! ## Hook 阶段
//!
//! | 阶段 | 时机 | 典型用途 |
//! |---|---|---|
//! | `BeforeRetrieval` | 检索前 | 查询改写、查询扩展、检索模式切换 |
//! | `AfterRetrieval` | 检索后 | 结果过滤、质量评估、结果增强 |
//! | `BeforeGeneration` | LLM 生成前 | Prompt 增强、上下文注入 |
//! | `AfterGeneration` | LLM 生成后 | 事实核查、输出过滤、格式校验（记录用） |
//!
//! ## 对象安全设计
//!
//! 与 `Reranker` / `QueryRewriter` / `PromptCompressor` 端口相同，使用手动
//! `Pin<Box<Future>>` 返回类型以保证对象安全（dyn-compatible）。
//! `AgentHook` 以 `Box<dyn AgentHook>` 形式存储在 `HookRegistry` 中，
//! 实现运行时插件式注册——用户可在不修改引擎源码的情况下扩展 Agent 行为。
//!
//! ## 向后兼容
//!
//! `AgentEngine` / `CoordinatorEngine` 的 `hooks` 字段为 `Option<Arc<HookRegistry>>`，
//! `None` 时行为与之前完全一致（零开销）。

use echomind_models::RetrievalResult;
use std::pin::Pin;

/// Hook 执行阶段。
///
/// 标识 hook 在 Agent/Coordinator 生命周期中的执行时机。
/// 每个 `AgentHook` 实现绑定到一个阶段，`HookRegistry::run_phase` 仅执行
/// 匹配阶段的 hook。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookPhase {
    /// 检索前：可修改查询文本（`HookContext::query`）。
    BeforeRetrieval,
    /// 检索后：可修改/过滤检索结果（`HookContext::retrieval_results`）。
    AfterRetrieval,
    /// LLM 生成前：可修改 prompt 上下文（查询 + 检索结果）。
    BeforeGeneration,
    /// LLM 生成后：可读取生成输出（`HookContext::output`，只读）。
    ///
    /// 注意：由于 LLM 输出为流式（`BoxStream`），此阶段在流开始前执行，
    /// `output` 字段为 `None`。hook 仅用于记录生成开始事件或做准备工作。
    AfterGeneration,
}

impl HookPhase {
    /// 返回阶段的字符串标识（用于日志和调试）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BeforeRetrieval => "before_retrieval",
            Self::AfterRetrieval => "after_retrieval",
            Self::BeforeGeneration => "before_generation",
            Self::AfterGeneration => "after_generation",
        }
    }
}

/// Hook 执行上下文（传递给 hook 的可变上下文）。
///
/// 不同阶段使用不同字段：
/// - `BeforeRetrieval`: `query` 可被修改
/// - `AfterRetrieval`: `retrieval_results` 可被修改/过滤
/// - `BeforeGeneration`: `query` + `retrieval_results` 可被修改
/// - `AfterGeneration`: `output` 可被读取（流式输出场景下为 `None`）
///
/// `metadata` 字段为自由 JSON 值，允许 hook 间传递任意数据
/// （如前一个 hook 标记的「已改写查询」标志，供后续 hook 参考）。
#[derive(Debug, Clone)]
pub struct HookContext {
    /// 会话 ID（可选，空字符串表示无关联会话）。
    pub conversation_id: String,
    /// 当前查询文本（`BeforeRetrieval` 阶段可被修改）。
    pub query: String,
    /// 当前 Hook 阶段。
    pub current_phase: HookPhase,
    /// 检索结果（`AfterRetrieval` / `BeforeGeneration` 阶段可被修改/过滤）。
    pub retrieval_results: Vec<RetrievalResult>,
    /// LLM 生成输出（`AfterGeneration` 阶段有值，只读）。
    /// 流式输出场景下为 `None`（hook 在流开始前执行）。
    pub output: Option<String>,
    /// 自由元数据（hook 间传递任意数据）。
    pub metadata: serde_json::Value,
}

impl HookContext {
    /// 创建新的 Hook 上下文。
    ///
    /// # 参数
    /// - `conversation_id`: 会话 ID（空字符串表示无关联会话）
    /// - `query`: 当前查询文本
    /// - `phase`: 当前 Hook 阶段
    pub fn new(conversation_id: String, query: String, phase: HookPhase) -> Self {
        Self {
            conversation_id,
            query,
            current_phase: phase,
            retrieval_results: Vec::new(),
            output: None,
            metadata: serde_json::Value::Null,
        }
    }
}

/// Agent 生命周期 Hook trait：插件式扩展点（REQ-RAG-029）。
///
/// 每个 hook 绑定到一个 `HookPhase`，在对应的生命周期节点被 `HookRegistry` 调用。
/// hook 可修改 `HookContext` 中的字段（如改写查询、过滤检索结果），
/// 从而影响后续引擎行为，无需修改 `AgentEngine` / `CoordinatorEngine` 源码。
///
/// # 对象安全
///
/// 使用手动 `Pin<Box<Future>>` 返回类型（与 `Reranker` / `QueryRewriter` 一致），
/// 保证对象安全（dyn-compatible），允许 `Box<dyn AgentHook>` 存储。
///
/// # 错误处理
///
/// `execute` 返回 `Err` 时中断后续 hook 执行，错误冒泡到引擎。
/// 引擎将错误向上传播给调用方（`chat_inner`），由命令层决定降级策略。
///
/// # 典型实现
///
/// - **查询改写 hook**（`BeforeRetrieval`）：复用 `QueryRewriter` 改写查询
/// - **结果过滤 hook**（`AfterRetrieval`）：过滤低分检索结果
/// - **Prompt 增强 hook**（`BeforeGeneration`）：注入额外上下文
/// - **事实核查 hook**（`AfterGeneration`）：记录生成开始事件
pub trait AgentHook: Send + Sync {
    /// Hook 名称（用于日志和调试）。
    fn name(&self) -> &str;

    /// Hook 执行阶段。
    fn phase(&self) -> HookPhase;

    /// 执行 hook，可修改上下文。
    ///
    /// 返回 `Err` 时中断后续 hook 执行（错误冒泡到引擎）。
    fn execute<'a>(
        &'a self,
        ctx: &'a mut HookContext,
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>>;

    /// 是否启用（默认 `true`）。
    ///
    /// 禁用的 hook 不会被 `HookRegistry::run_phase` 调用，
    /// 实现运行时开关——无需从注册表中移除 hook 即可临时禁用。
    fn enabled(&self) -> bool {
        true
    }
}

/// Hook 注册表：管理多个 hook，按阶段执行。
///
/// # 执行顺序
///
/// hook 按注册顺序执行。同一阶段可注册多个 hook，它们将依次执行，
/// 前一个 hook 修改的 `HookContext` 会传递给下一个 hook。
///
/// # 错误传播
///
/// 任一 hook 返回 `Err` 时中断后续 hook 执行，错误冒泡到引擎。
///
/// # 线程安全
///
/// `HookRegistry` 内部使用 `Vec<Box<dyn AgentHook>>`，
/// 所有 `AgentHook` 实现必须是 `Send + Sync`，因此 `HookRegistry` 本身是 `Send + Sync`。
pub struct HookRegistry {
    hooks: Vec<Box<dyn AgentHook>>,
}

impl HookRegistry {
    /// 创建空的注册表。
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// 注册一个 hook。
    ///
    /// hook 按注册顺序在对应阶段执行。
    pub fn register(&mut self, hook: Box<dyn AgentHook>) {
        self.hooks.push(hook);
    }

    /// 执行指定阶段的所有已启用 hook（按注册顺序）。
    ///
    /// 仅执行 `hook.phase() == phase && hook.enabled()` 的 hook。
    /// 前一个 hook 修改的 `HookContext` 会传递给下一个 hook。
    ///
    /// # 错误处理
    ///
    /// 任一 hook 返回 `Err` 时中断后续 hook 执行，错误冒泡到调用方。
    ///
    /// # 参数
    /// - `phase`: 要执行的 Hook 阶段
    /// - `ctx`: 可变 Hook 上下文（hook 可修改其中的字段）
    pub async fn run_phase(&self, phase: HookPhase, ctx: &mut HookContext) -> anyhow::Result<()> {
        for hook in &self.hooks {
            if hook.enabled() && hook.phase() == phase {
                hook.execute(ctx).await?;
            }
        }
        Ok(())
    }

    /// 已注册的 hook 数量。
    pub fn count(&self) -> usize {
        self.hooks.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HookRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistry")
            .field("hook_count", &self.hooks.len())
            .field(
                "hook_names",
                &self.hooks.iter().map(|h| h.name()).collect::<Vec<_>>(),
            )
            .finish()
    }
}
