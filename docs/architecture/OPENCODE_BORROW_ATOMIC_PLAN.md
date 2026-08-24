# OpenCode 借鉴吸收原子计划

> **来源**：OpenCode v1.18.15（github.com/anomalyco/opencode）全面源码研究
> **创建**：2026-08-07
> **状态**：📋 规划中
> **关联**：`CONTEXT.md`（OpenCode 会话运行时概念体系）、`DESIGN.md`（LLM 库设计文档）

## 借鉴清单总览

| # | 借鉴点 | 优先级 | Session | 状态 |
|---|---|---|---|---|
| B01 | System Context Registry | 🔴 高 | S1 | ✅ |
| B02 | Context Epoch + Durable Baseline | 🔴 高 | S2 | 📋 |
| B03 | 结构化压缩模板 | 🔴 高 | S3 | 📋 |
| B04 | Prompt Caching 自动放置策略 | 🔴 高 | S4 | 📋 |
| B05 | Durable Prompt Admission | 🟡 中 | S5 | 📋 |
| B06 | Session Run Coordinator | 🟡 中 | S6 | 📋 |
| B07 | Tool Output Bounding | 🟡 中 | S7 | ✅ |
| B08 | Session Todo 持久化 | 🟡 中 | S8 | ✅ |
| B09 | Skill 系统发现 | 🟡 中 | S9 | ✅ Done |
| B10 | 多协议 LLM 适配 | 🟢 低 | S10 | ✅ Done |
| B11 | Permission 细粒度控制 | 🟢 低 | S11 | ✅ Done |
| B12 | Snapshot/Revert | 🟢 低 | S12 | ⛔ Deprecated |
| B13 | Plugin 系统 V2 | 🟢 低 | S13 | ⛔ Deprecated |
| B14 | Effect 服务分层思路 | 🟢 低 | S14 | ✅ 仅参考 |

---

## 第一部分：高优先级（🔴）— 架构级质变

### S1: System Context Registry（系统上下文注册表）

#### 借鉴来源

OpenCode `packages/core/src/system-context/index.ts` + `registry.ts` + `builtins.ts`

将系统提示拆分为多个独立的、类型化的 Context Source，每个 Source 有自己的 `load()` / `baseline()` / `update()` / `removed()` 方法。在 Safe Provider-Turn Boundary 处懒加载比较，只生成差异消息。

#### EchoMind 现状

`crates/prompt/src/lib.rs` 的 `build_rag_prompt_segmented()` 一次性构建完整系统提示（静态前缀 + 动态上下文），每次 `chat_inner` 调用都重新构建全量上下文。没有增量更新机制，没有上下文变更检测。

#### 设计方案

**新增 crate：`crates/context`**

```
crates/context/
  src/
    lib.rs          — SystemContext trait + Source trait + Registry
    source.rs       — 内置 Source 实现
    snapshot.rs     — ContextSnapshot 比较与持久化
    tests.rs        — TDD 测试
```

**核心 trait 定义**（`lib.rs`）：

```rust
/// 稳定的命名空间标识（如 "system/role"、"kb/summary"、"conv/history"）
pub type SourceKey = String;

/// 一个独立的上下文源：观察当前值 → 比较快照 → 渲染基线/更新/移除文本
pub trait ContextSource: Send + Sync {
    /// 稳定标识
    fn key(&self) -> &SourceKey;
    
    /// 观察当前值（可能返回 Unavailable 表示临时不可用）
    async fn load(&self) -> Result<SourceValue>;
    
    /// 首次渲染：完整基线文本
    fn baseline(&self, value: &SourceValue) -> String;
    
    /// 变更差异渲染：前值 → 当前值 → 差异消息
    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String;
    
    /// 移除渲染（Source 被注销时）
    fn removed(&self, previous: &SourceValue) -> Option<String> { None }
}

/// 上下文源的值（JSON 可序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceValue {
    Text(String),
    Json(serde_json::Value),
    Unavailable,
}

/// 上下文快照：持久化的比较状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    /// key → (value, 可选的移除消息)
    pub sources: HashMap<SourceKey, (SourceValue, Option<String>)>,
}

/// 三种协调结果
pub enum ReconcileResult {
    /// 无变化
    Unchanged,
    /// 有更新：生成 Mid-Conversation System Message
    Updated { text: String, snapshot: ContextSnapshot },
    /// 需要替换基线（压缩后或上下文不可用后恢复）
    ReplacementReady { baseline: String, snapshot: ContextSnapshot },
    /// 替换被阻塞（上下文不可用，保留旧值）
    ReplacementBlocked,
}

/// 系统上下文：多个 Source 的组合
pub struct SystemContext {
    sources: Vec<Box<dyn ContextSource>>,
}

impl SystemContext {
    /// 初始化：首次渲染完整基线
    pub async fn initialize(&self) -> Result<(String, ContextSnapshot)>;
    
    /// 协调：比较当前值与快照，决定下一步动作
    pub async fn reconcile(&self, snapshot: &ContextSnapshot) -> Result<ReconcileResult>;
    
    /// 替换：压缩后重新生成基线
    pub async fn replace(&self, snapshot: &ContextSnapshot) -> Result<ReconcileResult>;
}
```

**内置 Source 实现**（`source.rs`）：

```rust
/// 角色描述源 — 静态不变
pub struct RoleSource {
    role_text: String,
}

impl ContextSource for RoleSource {
    fn key(&self) -> &SourceKey { &"system/role".to_string() }
    async fn load(&self) -> Result<SourceValue> {
        Ok(SourceValue::Text(self.role_text.clone()))
    }
    fn baseline(&self, value: &SourceValue) -> String {
        match value { SourceValue::Text(t) => t.clone(), _ => String::new() }
    }
    fn update(&self, _: &SourceValue, _: &SourceValue) -> String {
        String::new() // 角色不变，不产生更新
    }
}

/// 知识库摘要源 — 导入新文档时变化
pub struct KnowledgeBaseSource {
    storage: Arc<dyn Storage>,
}

impl ContextSource for KnowledgeBaseSource {
    fn key(&self) -> &SourceKey { &"kb/summary".to_string() }
    async fn load(&self) -> Result<SourceValue> {
        let count = self.storage.count_documents().await?;
        let domains = self.storage.get_domain_distribution().await?;
        Ok(SourceValue::Json(serde_json::json!({
            "doc_count": count,
            "domains": domains,
        })))
    }
    fn baseline(&self, value: &SourceValue) -> String {
        // 渲染知识库概况
        format!("当前知识库包含 {} 篇文档。", /* ... */)
    }
    fn update(&self, previous: &SourceValue, current: &SourceValue) -> String {
        // 渲染变化：新增/删除了哪些文档
        format!("[知识库更新] 文档数量从 {} 变为 {}", /* ... */)
    }
}

/// 对话历史源 — 每轮对话变化
pub struct ConversationHistorySource {
    storage: Arc<dyn Storage>,
}

/// 检索记忆源 — 记忆更新时变化
pub struct RetrievalMemorySource {
    storage: Arc<dyn Storage>,
}

/// Agent 模式源 — 用户切换 Agent 时变化
pub struct AgentModeSource {
    config_store: ConfigStore,
}
```

**集成到 ChatEngine**：

```rust
// crates/core/src/chat.rs — ChatEngine 新增字段
pub struct ChatEngine<R: Retriever, L: LLMProvider> {
    // ... 现有字段 ...
    /// 系统上下文注册表（None = 传统模式，向后兼容）
    context_registry: Option<Arc<SystemContext>>,
}

impl<R: Retriever, L: LLMProvider> ChatEngine<R, L> {
    pub fn with_context_registry(mut self, registry: Arc<SystemContext>) -> Self {
        self.context_registry = Some(registry);
        self
    }
    
    pub async fn chat(&self, query: &str, history: &[ChatMessage]) -> Result<ChatOutcome> {
        if let Some(registry) = &self.context_registry {
            // 新路径：协调上下文
            let snapshot = self.load_context_snapshot().await?;
            let result = registry.reconcile(&snapshot).await?;
            match result {
                ReconcileResult::Unchanged => {
                    // 复用基线，仅追加检索片段
                }
                ReconcileResult::Updated { text, snapshot } => {
                    // 在历史中插入 Mid-Conversation System Message
                    self.persist_context_update(text, snapshot).await?;
                }
                ReconcileResult::ReplacementReady { baseline, snapshot } => {
                    // 新纪元：重新设置基线
                    self.set_baseline(baseline, snapshot).await?;
                }
                ReconcileResult::ReplacementBlocked => {
                    // 保留旧基线
                }
            }
        }
        // ... 现有检索 + LLM 调用逻辑 ...
    }
}
```

#### 数据库变更

```sql
-- 新增 context_epochs 表
CREATE TABLE IF NOT EXISTS context_epochs (
    conversation_id TEXT NOT NULL,
    baseline_text TEXT NOT NULL,
    baseline_seq INTEGER NOT NULL,
    snapshot TEXT NOT NULL,  -- JSON 序列化的 ContextSnapshot
    created_at INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

-- 新增 context_updates 表（Mid-Conversation System Messages）
CREATE TABLE IF NOT EXISTS context_updates (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    update_text TEXT NOT NULL,
    snapshot TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
```

#### TDD 测试计划

| 测试 ID | 描述 |
|---|---|
| TC-CTX-001 | 单个 Source 的 baseline 渲染正确 |
| TC-CTX-002 | Source 值不变时 reconcile 返回 Unchanged |
| TC-CTX-003 | Source 值变化时 reconcile 返回 Updated + 正确差异文本 |
| TC-CTX-004 | Source 不可用时 reconcile 返回 ReplacementBlocked |
| TC-CTX-005 | 多 Source 组合时 baseline 按注册顺序拼接 |
| TC-CTX-006 | 压缩后 replace 生成新基线 |
| TC-CTX-007 | 快照持久化后跨会话恢复正确 |
| TC-CTX-008 | Source 移除时调用 removed() 渲染移除消息 |

#### 验收标准

- [ ] `crates/context` crate 编译通过（Free + Pro）
- [ ] 8 个 TDD 测试全绿
- [ ] ChatEngine 集成后传统模式行为不变（向后兼容）
- [ ] Clippy 零警告 + fmt 通过
- [ ] `docs/api/code-contracts.md` 新增 SystemContext 契约

---

### S2: Context Epoch + Durable Baseline（上下文纪元 + 持久化基线）

#### 借鉴来源

OpenCode `packages/core/src/session/context-epoch.ts`

每个会话有一个不可变的 Baseline System Context，持久化到数据库。跨进程重启时复用同一基线。只有压缩、会话移动或上下文不可用时才开启新纪元。

#### EchoMind 现状

每次 `chat_inner` 调用都通过 `build_rag_prompt_segmented()` 重新构建系统提示，没有持久化基线概念。Prompt Caching 仅依赖 API 端的前缀匹配，客户端不管理基线。

#### 设计方案

**新增模块：`crates/context/src/epoch.rs`**

```rust
/// 上下文纪元管理器
pub struct ContextEpochManager {
    storage: Arc<dyn Storage>,
}

impl ContextEpochManager {
    /// 首次初始化：生成基线 + 快照
    pub async fn initialize(
        &self,
        conversation_id: &str,
        context: &SystemContext,
    ) -> Result<PreparedBaseline>;
    
    /// 准备：加载已有基线或创建新的
    /// - 有存储的基线 + 无压缩 → reconcile（增量更新）
    /// - 有存储的基线 + 有压缩 → replace（新纪元）
    /// - 无存储的基线 → initialize
    pub async fn prepare(
        &self,
        conversation_id: &str,
        context: &SystemContext,
    ) -> Result<PreparedBaseline>;
    
    /// 推进快照：上下文变更后更新快照
    pub async fn advance(
        &self,
        conversation_id: &str,
        snapshot: ContextSnapshot,
    ) -> Result<()>;
}

/// 准备好的基线
pub struct PreparedBaseline {
    /// 基线文本（不可变，作为 Prompt Caching 前缀）
    pub baseline_text: String,
    /// 基线序列号
    pub baseline_seq: i64,
}
```

**集成到 `chat_inner`**：

```rust
// crates/tauri-app/src/commands/chat.rs

async fn chat_inner(state: &AppState, req: ChatRequest) -> Result<()> {
    // ... 现有初始化 ...
    
    // S2: 上下文纪元管理
    if let Some(registry) = &state.context_registry {
        let epoch_mgr = ContextEpochManager::new(state.storage.clone().into());
        let prepared = epoch_mgr.prepare(&req.conversation_id, registry).await?;
        
        // 使用 prepared.baseline_text 作为静态前缀
        // 检索片段作为动态上下文追加
        let prompt = SegmentedPrompt {
            static_prefix: prepared.baseline_text,
            dynamic_context: build_dynamic_context(&sources),
        };
        
        // LLM 调用使用分段 prompt
        // ...
    } else {
        // 传统路径（向后兼容）
        let prompt = build_rag_prompt_segmented(&sources);
    }
}
```

**压缩集成**：

```rust
// crates/compact/src/lib.rs — CompactionEngine 压缩后触发新纪元

impl<'a, L: LLMProvider> CompactionEngine<'a, L> {
    pub async fn compact(&self, history: &[ChatMessage], token_limit: usize) -> Result<CompactionResult> {
        // ... 现有压缩逻辑 ...
        
        // 压缩完成后，标记需要新纪元
        // 下次 chat_inner 调用时，ContextEpochManager.prepare() 检测到压缩
        // → 调用 SystemContext.replace() 生成新基线
    }
}
```

#### 数据库变更

复用 S1 的 `context_epochs` 表，新增压缩标记：

```sql
-- messages 表新增 compaction 标记（已有 type 字段可复用）
-- compaction 消息的 seq > baseline_seq 时触发 replace
```

#### TDD 测试计划

| 测试 ID | 描述 |
|---|---|
| TC-EPOCH-001 | 首次调用 initialize 生成基线 + 快照 |
| TC-EPOCH-002 | 二次调用 prepare 复用已有基线 |
| TC-EPOCH-003 | 压缩后 prepare 触发 replace 新基线 |
| TC-EPOCH-004 | 上下文不可用时 prepare 返回旧基线 |
| TC-EPOCH-005 | 跨进程重启后基线一致 |
| TC-EPOCH-006 | 基线文本跨请求完全不变（Prompt Caching 命中） |

#### 验收标准

- [ ] `ContextEpochManager` 编译通过
- [ ] 6 个 TDD 测试全绿
- [ ] `chat_inner` 集成后传统模式行为不变
- [ ] 基线文本在连续对话中保持不变（手动验证 API 请求日志）
- [ ] Clippy 零警告 + fmt 通过

---

### S3: 结构化压缩模板（Structured Compaction Template）

#### 借鉴来源

OpenCode `packages/core/src/session/compaction.ts` — `SUMMARY_TEMPLATE`

使用结构化 Markdown 模板（Objective / Important Details / Work State / Next Move / Relevant Files）进行 LLM 压缩，保留关键工作状态信息。

#### EchoMind 现状

`crates/compact/src/lib.rs` 的 `SUMMARY_SYSTEM_PROMPT` 使用自由文本指令："保留关键信息、用户意图、已确定的结论和重要上下文"。LLM 输出格式不受控，信息保留质量不稳定。

#### 设计方案

**替换压缩系统提示词**：

```rust
// crates/compact/src/lib.rs

/// 结构化压缩模板（借鉴 OpenCode SessionCompaction）
const STRUCTURED_SUMMARY_TEMPLATE: &str = r#"请按以下 Markdown 结构输出对话历史摘要，保持章节顺序不变。不要包含 <template> 标签。

<template>
## 目标
- [一到两句话描述用户试图完成的任务]

## 重要细节
- [约束/偏好、决策及原因、重要事实/假设、继续所需的确切上下文，或"(无)"]

## 工作状态
### 已完成
- [已完成的工作、已验证的事实、或所做的更改；否则"(无)"]

### 进行中
- [当前工作、部分更改、或调查状态；否则"(无)"]

### 阻塞
- [障碍、失败命令、或未知问题；否则"(无)"]

## 下一步
1. [立即要做的具体操作，或"(无)"]
2. [已知的下一步操作，或"(无)"]

## 相关文件
- [文件或文档路径：为何重要，或"(无)"]
</template>

规则：
- 保留每个章节，即使为空。
- 使用简洁的要点，不要写成段落。
- 保留确切的文件路径、符号、命令、错误字符串、URL 和标识符。
- 不要提及摘要过程或上下文被压缩。"#;

/// 降级截断时插入的提示消息前缀（保持不变）
const FALLBACK_NOTICE: &str = "[早期对话历史已截断] ";
```

**增强 `compact()` 方法**：

```rust
impl<'a, L: LLMProvider> CompactionEngine<'a, L> {
    pub async fn compact(
        &self,
        history: &[ChatMessage],
        token_limit: usize,
    ) -> anyhow::Result<CompactionResult> {
        // ... 现有 token 计算和分割逻辑不变 ...
        
        // 使用结构化模板生成摘要
        let summary = self.generate_structured_summary(&old_messages).await;
        
        match summary {
            Ok(text) => {
                // 结构化摘要成功
                let summary_msg = ChatMessage::system(&text);
                Ok(CompactionResult {
                    history: vec![summary_msg].into_iter().chain(recent_messages).collect(),
                    info: Some(CompactionInfo {
                        original_tokens: total_tokens,
                        compacted_tokens: /* ... */,
                        message_count: old_messages.len(),
                    }),
                })
            }
            Err(_) => {
                // 降级为截断（保持不变）
                // ...
            }
        }
    }
    
    /// 使用结构化模板调用 LLM 生成摘要
    async fn generate_structured_summary(&self, messages: &[ChatMessage]) -> anyhow::Result<String> {
        let conversation_text = messages.iter()
            .map(|m| format!("[{}]: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");
        
        let user_prompt = format!("请将以下对话历史压缩为结构化摘要：\n\n{}", conversation_text);
        
        // 使用结构化模板作为系统提示
        let stream = self.llm.chat_stream(
            STRUCTURED_SUMMARY_TEMPLATE,
            &[ChatMessage::user(&user_prompt)],
        ).await?;
        
        // 收集完整响应
        let result = collect_stream_to_string(stream).await?;
        Ok(result)
    }
}
```

**`IncrementalCompactor` 同步更新**：

```rust
// crates/compact/src/token_budget.rs

/// 增量压缩也使用结构化模板
const INCREMENTAL_TEMPLATE: &str = r#"请按以下结构更新对话历史摘要。保持章节顺序，合并新信息。

<template>
## 目标
- [更新后的任务描述]

## 重要细节
- [新增的约束/偏好/决策/事实]

## 工作状态
### 已完成
- [新完成的工作]
### 进行中
- [当前进行的工作]
### 阻塞
- [新的障碍]

## 下一步
1. [更新后的下一步]
2. [后续步骤]

## 相关文件
- [新涉及的文件]
</template>

规则：
- 保留每个章节。
- 合并旧摘要和新对话的信息，不要丢失旧信息。
- 使用简洁的要点。"#;
```

#### TDD 测试计划

| 测试 ID | 描述 |
|---|---|
| TC-COMP-TPL-001 | 结构化模板包含全部 6 个章节标题 |
| TC-COMP-TPL-002 | LLM 输出遵循模板结构（模拟 LLM 返回） |
| TC-COMP-TPL-003 | 摘要中保留文件路径和命令字符串 |
| TC-COMP-TPL-004 | 空章节输出"(无)" |
| TC-COMP-TPL-005 | 增量压缩合并旧摘要和新对话 |
| TC-COMP-TPL-006 | 降级路径（LLM 失败）行为不变 |

#### 验收标准

- [ ] `STRUCTURED_SUMMARY_TEMPLATE` 替换 `SUMMARY_SYSTEM_PROMPT`
- [ ] 6 个 TDD 测试全绿
- [ ] 手动验证：多轮对话后压缩摘要包含结构化章节
- [ ] 向后兼容：`compact()` 签名不变
- [ ] Clippy 零警告 + fmt 通过

---

### S4: Prompt Caching 自动放置策略

#### 借鉴来源

OpenCode `packages/llm/src/cache-policy.ts`

`auto` 策略自动在三个位置放置缓存断点：最后一条工具定义、最后一条系统消息、最新的用户消息。手动 `cache` 放置被保留。

#### EchoMind 现状

`crates/prompt/src/lib.rs` 的 `SegmentedPrompt` 已有静态前缀/动态上下文分离，但没有自动放置缓存断点的策略。`OpenAIProvider` 通过两条 system 消息实现前缀缓存，但放置策略不够精细。

#### 设计方案

**新增模块：`crates/prompt/src/cache_policy.rs`**

```rust
/// 缓存策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CachePolicy {
    /// 自动放置（默认）：工具末尾 + 系统消息末尾 + 最新用户消息
    Auto,
    /// 禁用缓存
    None,
    /// 自定义放置位置
    Custom(CachePolicyObject),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePolicyObject {
    /// 在最后一条工具定义处放置断点
    pub tools: Option<bool>,
    /// 在最后一条系统消息处放置断点
    pub system: Option<bool>,
    /// 在用户消息处放置断点
    pub messages: Option<CacheMessagePolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheMessagePolicy {
    /// 最新用户消息
    LatestUserMessage,
    /// 所有用户消息
    AllUserMessages,
    /// 不在消息处放置
    None,
}

/// 缓存提示标记（附加到消息上）
#[derive(Debug, Clone)]
pub struct CacheHint {
    /// 缓存类型
    pub cache_type: CacheType,
    /// TTL（秒），仅某些协议支持
    pub ttl_seconds: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum CacheType {
    /// 临时缓存
    Ephemeral,
}

/// 应用缓存策略：在指定位置插入 CacheHint
pub fn apply_cache_policy(
    system: &mut Vec<SystemPart>,
    messages: &mut Vec<ChatMessage>,
    tools: &mut Vec<ToolDefinition>,
    policy: &CachePolicy,
) {
    let resolved = resolve(policy);
    
    // 在最后一条工具定义处放置
    if resolved.tools == Some(true) && !tools.is_empty() {
        let last = tools.len() - 1;
        tools[last].cache = Some(make_hint(None));
    }
    
    // 在最后一条系统消息处放置
    if resolved.system == Some(true) && !system.is_empty() {
        let last = system.len() - 1;
        system[last].cache = Some(make_hint(None));
    }
    
    // 在用户消息处放置
    match resolved.messages {
        Some(CacheMessagePolicy::LatestUserMessage) => {
            // 找到最后一条用户消息
            for msg in messages.iter_mut().rev() {
                if msg.role == "user" {
                    msg.cache = Some(make_hint(None));
                    break;
                }
            }
        }
        Some(CacheMessagePolicy::AllUserMessages) => {
            for msg in messages.iter_mut() {
                if msg.role == "user" {
                    msg.cache = Some(make_hint(None));
                }
            }
        }
        _ => {}
    }
}

fn resolve(policy: &CachePolicy) -> CachePolicyObject {
    match policy {
        CachePolicy::Auto => CachePolicyObject {
            tools: Some(true),
            system: Some(true),
            messages: Some(CacheMessagePolicy::LatestUserMessage),
        },
        CachePolicy::None => CachePolicyObject {
            tools: None,
            system: None,
            messages: None,
        },
        CachePolicy::Custom(obj) => obj.clone(),
    }
}
```

**集成到 `SegmentedPrompt`**：

```rust
// crates/prompt/src/lib.rs

impl SegmentedPrompt {
    /// 构建带缓存策略的分段提示词
    pub fn with_cache_policy(
        sources: &[RetrievalResult],
        cache_policy: &CachePolicy,
    ) -> SegmentedPromptWithCache {
        let mut prompt = build_rag_prompt_segmented(sources);
        
        // 应用缓存策略
        // 静态前缀 → 系统消息末尾放置断点
        // 动态上下文 → 不放置（每次不同）
        
        SegmentedPromptWithCache {
            static_prefix: prompt.static_prefix,
            dynamic_context: prompt.dynamic_context,
            cache_hints: vec![
                CacheHintPlacement::StaticPrefixEnd,
            ],
        }
    }
}
```

**OpenAIProvider 增强**：

```rust
// crates/infra/src/openai_provider.rs

impl LLMProvider for OpenAIProvider {
    async fn chat_stream_segmented(
        &self,
        static_prefix: &str,
        dynamic_context: &str,
        history: &[ChatMessage],
    ) -> Result<BoxStream<'static, Result<String>>> {
        // 检查是否支持显式缓存（OpenAI 隐式缓存，Anthropic 显式缓存）
        // OpenAI: 前缀匹配自动缓存，无需显式标记
        // Anthropic: 需要 cache_control 字段
        
        // 对于 OpenAI 兼容 API：
        // 1. 静态前缀作为第一条 system 消息（可被缓存）
        // 2. 动态上下文作为第二条 system 消息（不可缓存）
        // 3. 确保历史消息顺序不变（前缀对齐）
        
        let mut messages = vec![];
        messages.push(json!({"role": "system", "content": static_prefix}));
        messages.push(json!({"role": "system", "content": dynamic_context}));
        for msg in history {
            messages.push(json!({"role": msg.role, "content": msg.content}));
        }
        
        // ... 发送请求 ...
    }
}
```

#### TDD 测试计划

| 测试 ID | 描述 |
|---|---|
| TC-CACHE-POL-001 | Auto 策略解析为 tools+system+latest_user |
| TC-CACHE-POL-002 | None 策略不放置任何断点 |
| TC-CACHE-POL-003 | Custom 策略按指定位置放置 |
| TC-CACHE-POL-004 | 空工具列表时不放置工具断点 |
| TC-CACHE-POL-005 | 空系统消息时不放置系统断点 |
| TC-CACHE-POL-006 | 最新用户消息正确定位 |
| TC-CACHE-POL-007 | 手动 cache 标记不被覆盖 |

#### 验收标准

- [ ] `cache_policy.rs` 模块编译通过
- [ ] 7 个 TDD 测试全绿
- [ ] `SegmentedPrompt` 集成后向后兼容
- [ ] Clippy 零警告 + fmt 通过
- [ ] `docs/api/settings-reference.md` 新增 `cache.policy` 设置键

---

## 第二部分：中优先级（🟡）— 体验增强

### S5: Durable Prompt Admission（持久化提示接纳）

#### 借鉴来源

OpenCode `packages/core/src/session/input.ts` — Prompt Admission / Promotion

用户消息先持久化为"已接纳"状态，在安全边界点"提升"为模型可见的历史消息。支持三种投递模式。

#### EchoMind 现状

用户消息直接持久化为 `messages` 表，没有"已接纳但未执行"的中间状态。流式生成中途无法处理新消息。

#### 设计方案

**新增数据表**：

```sql
CREATE TABLE IF NOT EXISTS pending_inputs (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    content TEXT NOT NULL,
    delivery TEXT NOT NULL DEFAULT 'steer',  -- steer | queue
    created_at INTEGER NOT NULL,
    promoted_seq INTEGER,  -- NULL = 未提升, 非 NULL = 已提升为消息
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
```

**新增 Storage trait 方法**：

```rust
// crates/core/src/lib.rs — Storage trait 新增

/// 接纳用户输入（持久化但不加入消息历史）
async fn admit_input(&self, conversation_id: &str, content: &str, delivery: &str) -> Result<String>;
/// 提升待处理输入为正式消息
async fn promote_input(&self, input_id: &str) -> Result<()>;
/// 获取待处理输入
async fn get_pending_inputs(&self, conversation_id: &str) -> Result<Vec<PendingInput>>;
```

**集成到 chat_inner**：

```rust
// crates/tauri-app/src/commands/chat.rs

async fn chat_inner(state: &AppState, req: ChatRequest) -> Result<()> {
    // 接纳用户输入
    let input_id = state.storage.admit_input(
        &req.conversation_id,
        &req.message,
        "steer",
    ).await?;
    
    // 在安全边界点提升为正式消息
    state.storage.promote_input(&input_id).await?;
    
    // 继续正常 RAG 流程
    // ...
    
    // 流式生成完成后检查是否有排队消息
    let pending = state.storage.get_pending_inputs(&req.conversation_id).await?;
    if !pending.is_empty() {
        // 自动处理下一条排队消息
        // ...
    }
}
```

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-ADMIT-001 | 接纳输入后 pending_inputs 表有记录 |
| TC-ADMIT-002 | 提升后 promoted_seq 非 NULL |
| TC-ADMIT-003 | steer 模式优先于 queue 模式 |
| TC-ADMIT-004 | 多条排队消息按 FIFO 顺序提升 |
| TC-ADMIT-005 | 删除会话时级联删除 pending_inputs |

---

### S6: Session Run Coordinator（会话运行协调器）

#### 借鉴来源

OpenCode `packages/core/src/session/run-coordinator.ts`

每个 Session 串行执行，不同 Session 并发执行。支持 `wake`（合并唤醒）和 `interrupt`（中断）。

#### EchoMind 现状

`AppState.abort_tokens` 使用 `CancellationToken` 管理中断，但没有协调器模式。不同会话的并发执行由 Tokio runtime 隐式管理。

#### 设计方案

**新增模块：`crates/core/src/session_coordinator.rs`**

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

/// 会话运行协调器
pub struct SessionCoordinator {
    /// 活跃会话：conversation_id → 协调条目
    active: Arc<Mutex<HashMap<String, CoordEntry>>>,
}

struct CoordEntry {
    /// 完成通知器
    done: Arc<Notify>,
    /// 是否有待处理的唤醒
    pending_wake: bool,
    /// 是否正在停止
    stopping: bool,
}

impl SessionCoordinator {
    pub fn new() -> Self {
        Self { active: Arc::new(Mutex::new(HashMap::new())) }
    }
    
    /// 启动或加入执行
    pub async fn run<F, Fut>(&self, session_id: &str, f: F) -> Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut active = self.active.lock().await;
        
        if let Some(entry) = active.get(session_id) {
            if entry.stopping {
                // 等待停止完成后重试
                drop(active);
                // ... 递归重试 ...
            } else {
                // 加入已有执行
                let done = entry.done.clone();
                drop(active);
                done.notified().await;
                return Ok(());
            }
        }
        
        // 启动新执行
        let entry = CoordEntry {
            done: Arc::new(Notify::new()),
            pending_wake: false,
            stopping: false,
        };
        active.insert(session_id.to_string(), entry);
        drop(active);
        
        // 执行 drain
        let result = f().await;
        
        // 检查是否有待处理唤醒
        let mut active = self.active.lock().await;
        if let Some(entry) = active.get(session_id) {
            if entry.pending_wake && !entry.stopping {
                entry.pending_wake = false;
                drop(active);
                // 递归执行
                return self.run(session_id, f).await;
            }
        }
        active.remove(session_id);
        
        result
    }
    
    /// 注册后续工作（合并唤醒）
    pub async fn wake(&self, session_id: &str) {
        let mut active = self.active.lock().await;
        if let Some(entry) = active.get_mut(session_id) {
            entry.pending_wake = true;
        }
    }
    
    /// 中断执行
    pub async fn interrupt(&self, session_id: &str) {
        let mut active = self.active.lock().await;
        if let Some(entry) = active.get_mut(session_id) {
            entry.stopping = true;
        }
    }
    
    /// 获取活跃会话列表
    pub async fn active_sessions(&self) -> Vec<String> {
        self.active.lock().await.keys().cloned().collect()
    }
}
```

**集成到 AppState**：

```rust
// crates/tauri-app/src/state.rs
pub struct AppState {
    // ... 现有字段 ...
    pub session_coordinator: Arc<SessionCoordinator>,
}
```

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-COORD-001 | 同一会话的两次 run 串行执行 |
| TC-COORD-002 | 不同会话的 run 并发执行 |
| TC-COORD-003 | wake 合并多次唤醒为一次 |
| TC-COORD-004 | interrupt 标记停止状态 |
| TC-COORD-005 | 执行完成后从活跃列表移除 |

---

### S7: Tool Output Bounding（工具输出有界截断）

#### 借鉴来源

OpenCode `packages/core/src/tool-output-store.ts` + `CONTEXT.md` Model Tool Output 概念

超大工具输出进行有界截断：保留开头和结尾，完整内容存入临时文件。

#### EchoMind 现状

`crates/core/src/code_executor.rs` 的代码执行结果没有大小限制。Agent 的 `search_kb` 工具输出也没有截断。

#### 设计方案

**新增模块：`crates/core/src/tool_output.rs`**

```rust
/// 工具输出上限配置
pub const MAX_TOOL_OUTPUT_LINES: usize = 500;
pub const MAX_TOOL_OUTPUT_BYTES: usize = 64 * 1024; // 64KB

/// 有界截断：保留开头和结尾
pub fn bound_tool_output(output: &str) -> BoundedOutput {
    let lines: Vec<&str> = output.lines().collect();
    let total_bytes = output.len();
    
    if lines.len() <= MAX_TOOL_OUTPUT_LINES && total_bytes <= MAX_TOOL_OUTPUT_BYTES {
        return BoundedOutput {
            text: output.to_string(),
            truncated: false,
            managed_path: None,
        };
    }
    
    // 保留前 N 行 + 后 N 行
    let head_lines = MAX_TOOL_OUTPUT_LINES / 2;
    let tail_lines = MAX_TOOL_OUTPUT_LINES / 2;
    
    let head = lines[..head_lines.min(lines.len())].join("\n");
    let tail = lines[lines.len().saturating_sub(tail_lines)..].join("\n");
    
    let truncated_text = format!(
        "{}\n\n[... 输出已截断，共 {} 行 {} 字节 ...]\n\n{}",
        head, lines.len(), total_bytes, tail
    );
    
    BoundedOutput {
        text: truncated_text,
        truncated: true,
        managed_path: None, // 完整内容可写入临时文件
    }
}

pub struct BoundedOutput {
    pub text: String,
    pub truncated: bool,
    pub managed_path: Option<String>,
}
```

**集成到 AgentEngine**：

```rust
// crates/core/src/agent.rs — execute_tool 方法

let observation = match tool_name {
    "search_kb" => {
        let results = retriever.retrieve(query, 5).await?;
        let text = format_search_results(&results);
        // 应用有界截断
        let bounded = bound_tool_output(&text);
        if bounded.truncated {
            format!("{}\n[注意：输出已截断]", bounded.text)
        } else {
            bounded.text
        }
    }
    "execute_code" => {
        let result = executor.execute(code, lang, stdin).await?;
        let text = format_execution_result(&result);
        let bounded = bound_tool_output(&text);
        bounded.text
    }
    // ...
};
```

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-BOUND-001 | 短输出不截断 |
| TC-BOUND-002 | 超行数限制截断保留首尾 |
| TC-BOUND-003 | 超字节限制截断保留首尾 |
| TC-BOUND-004 | 截断标记包含原始行数和字节数 |
| TC-BOUND-005 | 空输出安全处理 |

---

### S8: Session Todo 持久化

#### 借鉴来源

OpenCode `packages/core/src/session/todo.ts` + `todowrite.ts` 工具

将 Agent 的 Todo 列表持久化到 SQLite，支持跨会话恢复。

#### EchoMind 现状

AgentEngine（`agent.rs`）的 ReAct 循环没有持久化中间步骤的 Todo。

#### 设计方案

**新增数据表**：

```sql
CREATE TABLE IF NOT EXISTS session_todos (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    content TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | in_progress | completed
    priority TEXT NOT NULL DEFAULT 'medium', -- low | medium | high
    position INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);
```

**新增模型**（`crates/models/src/lib.rs`）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTodo {
    pub id: String,
    pub conversation_id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoStatus { Pending, InProgress, Completed }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TodoPriority { Low, Medium, High }
```

**Storage trait 新增方法 + SqliteStorage 实现**

**AgentEngine 集成**：

```rust
// crates/core/src/agent.rs

impl<R, L> AgentEngine<R, L> {
    pub async fn run(&self, query: &str, history: &[ChatMessage]) -> Result<AgentOutcome> {
        // ... 现有 ReAct 循环 ...
        
        // 每轮推理后更新 Todo
        if let Some(todos) = parse_todos_from_thought(&thought) {
            self.storage.update_todos(&conversation_id, &todos).await?;
        }
    }
}
```

**前端展示**：在 `chat-render.js` 中渲染 Todo 进度面板。

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-TODO-001 | 创建 Todo 持久化 |
| TC-TODO-002 | 更新状态 pending→in_progress→completed |
| TC-TODO-003 | position 排序正确 |
| TC-TODO-004 | 删除会话级联删除 Todo |
| TC-TODO-005 | 跨会话恢复 Todo 列表 |

---

### S9: Skill 系统发现

#### 借鉴来源

OpenCode `packages/core/src/skill.ts` — Markdown 文件驱动的技能发现

从目录发现 `SKILL.md` 文件，解析 frontmatter，通过 permission 过滤可用技能。

#### EchoMind 现状

`slash-commands.js` 有系统指令和自定义模板，但没有 Markdown 文件驱动的技能发现。

#### 设计方案

**新增模块：`crates/core/src/skill.rs`**

```rust
/// 技能信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    pub description: Option<String>,
    pub slash: bool, // 是否注册为 /slash 命令
    pub content: String, // Markdown 正文
    pub source_path: Option<String>,
}

/// 技能发现器
pub struct SkillDiscovery {
    storage: Arc<dyn Storage>,
}

impl SkillDiscovery {
    /// 从知识库文档中发现技能
    /// 查找文档内容以 "---\nskill:" frontmatter 开头的文档
    pub async fn discover_from_kb(&self) -> Result<Vec<Skill>> {
        let docs = self.storage.list_documents().await?;
        let mut skills = Vec::new();
        
        for doc in docs {
            // 读取文档内容
            let content = self.storage.get_document_content(&doc.id).await?;
            if let Some(skill) = self.parse_skill(&content, &doc.file_path) {
                skills.push(skill);
            }
        }
        
        Ok(skills)
    }
    
    /// 从本地目录发现 SKILL.md 文件
    pub async fn discover_from_dir(&self, dir: &Path) -> Result<Vec<Skill>> {
        let mut skills = Vec::new();
        for entry in fs::read_dir(dir).await? {
            let path = entry?.path();
            if path.file_name() == Some(OsStr::new("SKILL.md")) 
                || path.extension() == Some(OsStr::new("md")) {
                let content = fs::read_to_string(&path).await?;
                if let Some(skill) = self.parse_skill(&content, &path.to_string_lossy()) {
                    skills.push(skill);
                }
            }
        }
        Ok(skills)
    }
    
    fn parse_skill(&self, content: &str, source: &str) -> Option<Skill> {
        // 解析 YAML frontmatter
        if !content.starts_with("---\n") { return None; }
        let end = content[4..].find("\n---")?;
        let frontmatter = &content[4..4+end];
        let body = &content[4+end+4..];
        
        // 解析 name / description / slash
        let name = extract_yaml_field(frontmatter, "name")?;
        let description = extract_yaml_field(frontmatter, "description");
        let slash = extract_yaml_field(frontmatter, "slash")
            .map(|v| v == "true")
            .unwrap_or(false);
        
        Some(Skill {
            name,
            description,
            slash,
            content: body.trim().to_string(),
            source_path: Some(source.to_string()),
        })
    }
}
```

**IPC 命令**：

```rust
#[tauri::command]
async fn list_skills(state: State<'_, AppState>) -> Result<Vec<Skill>> {
    let discovery = SkillDiscovery::new(state.storage.clone().into());
    discovery.discover_from_kb().await
}

#[tauri::command]
async fn get_skill_content(name: String, state: State<'_, AppState>) -> Result<String> {
    // 返回技能正文内容
}
```

**前端集成**：在 `slash-commands.js` 中加载技能列表，`@skill_name` 调用时注入技能内容。

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-SKILL-001 | 解析带 frontmatter 的 SKILL.md |
| TC-SKILL-002 | 无 frontmatter 的文件被跳过 |
| TC-SKILL-003 | slash=true 注册为 / 命令 |
| TC-SKILL-004 | 技能内容通过 @name 调用注入 |
| TC-SKILL-005 | 从知识库和目录双源发现 |

---

## 第三部分：低优先级（🟢）— 未来扩展

### S10: 多协议 LLM 适配

#### 借鉴来源

OpenCode `packages/llm/` — Anthropic Messages / OpenAI Chat / OpenAI Responses / Gemini / Bedrock Converse 五种协议适配器。

#### 设计方案

**新增 trait 扩展**：

```rust
// crates/core/src/lib.rs — 扩展 LLMProvider trait

/// LLM 协议类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmProtocol {
    OpenAIChat,         // OpenAI Chat Completions（现有）
    OpenAIResponses,    // OpenAI Responses API（新）
    AnthropicMessages,  // Anthropic Messages API（原生 Prompt Caching）
    Gemini,             // Google Gemini
}

/// 协议感知的 LLM Provider
pub trait ProtocolAwareProvider: LLMProvider {
    fn protocol(&self) -> LlmProtocol;
    
    /// Anthropic 原生缓存控制
    async fn chat_stream_with_cache(
        &self,
        system: &str,
        messages: &[ChatMessage],
        cache_breakpoints: &[usize], // 消息索引位置
    ) -> Result<BoxStream<'static, Result<String>>>;
}
```

**Anthropic 适配器**（`crates/infra/src/anthropic_provider.rs`）：

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl LLMProvider for AnthropicProvider {
    async fn chat_stream(&self, system: &str, messages: &[ChatMessage]) 
        -> Result<BoxStream<'static, Result<String>>> 
    {
        // Anthropic Messages API
        // 支持 cache_control 字段实现显式 Prompt Caching
        // system 消息中可插入 {"type": "text", "text": "...", "cache_control": {"type": "ephemeral"}}
    }
}
```

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-ANTHROPIC-001 | Anthropic Messages API 请求格式正确 |
| TC-ANTHROPIC-002 | cache_control 字段正确放置 |
| TC-ANTHROPIC-003 | SSE 流解析正确 |
| TC-GEMINI-001 | Gemini API 请求格式正确 |

---

### S11: Permission 细粒度控制

#### 借鉴来源

OpenCode `packages/core/src/permission.ts` — Wildcard 匹配的 RBAC

#### 设计方案

```rust
// crates/core/src/permission.rs（扩展现有 security.rs）

/// 权限规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub action: String,      // "agent.search_kb" | "agent.execute_code" | "*"
    pub resource: String,    // "*" | "python" | "bash"
    pub effect: PermissionEffect, // Allow | Deny | Ask
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PermissionEffect { Allow, Deny, Ask }

/// 权限规则集
pub type Ruleset = Vec<PermissionRule>;

/// 评估权限
pub fn evaluate(action: &str, resource: &str, ruleset: &Ruleset) -> PermissionEffect {
    // 从后向前查找第一个匹配的规则
    for rule in ruleset.iter().rev() {
        if wildcard_match(action, &rule.action) && wildcard_match(resource, &rule.resource) {
            return rule.effect.clone();
        }
    }
    PermissionEffect::Deny // 默认拒绝
}

/// 通配符匹配
fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" { return true; }
    // 简单的 * 通配符匹配
    if pattern.ends_with('*') {
        return value.starts_with(&pattern[..pattern.len()-1]);
    }
    pattern == value
}
```

**Agent 工具调用集成**：

```rust
// crates/core/src/agent.rs

let effect = evaluate("agent.execute_code", &language, &agent_permissions);
match effect {
    PermissionEffect::Allow => { /* 执行 */ }
    PermissionEffect::Deny => { 
        observation = "权限被拒绝：无法执行代码".to_string();
    }
    PermissionEffect::Ask => {
        // 发送 permission_request 事件，前端弹出确认对话框
        // 等待用户响应
    }
}
```

#### TDD 测试

| 测试 ID | 描述 |
|---|---|
| TC-PERM-001 | 通配符 * 匹配所有 |
| TC-PERM-002 | 前缀通配符 agent.* 匹配 |
| TC-PERM-003 | 后到前规则优先（last-writer-wins） |
| TC-PERM-004 | 默认拒绝 |
| TC-PERM-005 | Ask 模式触发用户确认 |

---

### S12: Snapshot/Revert（文件快照回滚）— ⛔ Deprecated

#### 废弃理由

EchoMind 是 Tauri 桌面应用，无 OpenCode 的 Web IDE 热重载场景。Tauri 窗口状态已由 `window.{x,y,width,height,maximized}` settings 键持久化，无需快照/回滚机制。文档版本管理可通过导入时 MD5 去重 + 历史记录实现，不需要内容寻址快照系统。

#### 借鉴来源

OpenCode `packages/core/src/snapshot.ts` + `session/revert.ts`

内容寻址的文件系统快照，支持选择性回滚到特定消息点。

#### 设计方案

此功能对 EchoMind 的 RAG 知识库应用适用场景有限（不直接编辑代码文件），但如果未来增加"文档版本管理"功能可参考：

- 导入文档时捕获快照
- 支持回滚到导入前的状态
- 支持比较两个快照的差异

**评估**：低优先级，仅在用户明确需要文档版本管理时实施。

---

### S13: Plugin 系统 V2 — ⛔ Deprecated

#### 废弃理由

EchoMind 当前架构为单体桌面应用，六边形架构的 ports & adapters 已提供足够的扩展性。插件系统会增加安全攻击面（铁律安全要求），且无用户需求驱动。如需扩展功能，直接在 workspace 内新增 crate 即可，无需运行时插件加载。

#### 借鉴来源

OpenCode `packages/plugin/` — Effect 和 Promise 双版本 SDK

#### 设计方案

如果 EchoMind 未来开放插件生态：

```rust
// crates/core/src/plugin.rs

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    
    /// 注册时调用
    async fn register(&self, registry: &mut PluginRegistry) -> Result<()>;
}

pub struct PluginRegistry {
    tools: HashMap<String, Box<dyn ToolHandler>>,
    agents: HashMap<String, AgentDefinition>,
    commands: HashMap<String, Box<dyn CommandHandler>>,
}
```

**评估**：当前阶段不需要，等用户基数达到一定规模后再考虑。

---

### S14: Effect 服务分层思路

#### 借鉴来源

OpenCode 全面的 Effect 框架使用模式（Service / Layer / Interface 三层分离）

#### 设计方案

EchoMind 使用 Rust 的 trait 系统，不需要 Effect 框架。但可以借鉴其**服务分层**思路：

- **Interface**：trait 定义（已有，如 `Storage` / `LLMProvider` / `Retriever`）
- **Service**：具体实现（已有，如 `SqliteStorage` / `OpenAIProvider`）
- **Layer**：依赖组装层（部分已有，如 `AppState` 组装各组件）

**评估**：EchoMind 的六边形架构已足够优秀，此点仅作为设计参考，不需要代码变更。

---

## 实施路线图

### 阶段一：高优先级（S1-S4）

| Session | 功能 | 预计工时 | 依赖 |
|---|---|---|---|
| S1 | System Context Registry | 1-2 天 | 无 |
| S2 | Context Epoch + Durable Baseline | 1 天 | S1 |
| S3 | 结构化压缩模板 | 0.5 天 | 无 |
| S4 | Prompt Caching 自动放置 | 0.5 天 | S1 |

**阶段一验收**：连续 5 轮对话中，基线文本保持不变，Prompt Caching 命中率 > 80%。

### 阶段二：中优先级（S5-S9）

| Session | 功能 | 预计工时 | 依赖 |
|---|---|---|---|
| S5 | Durable Prompt Admission | 1 天 | 无 |
| S6 | Session Run Coordinator | 1 天 | 无 |
| S7 | Tool Output Bounding | 0.5 天 | 无 |
| S8 | Session Todo 持久化 | 1 天 | 无 |
| S9 | Skill 系统发现 | 1 天 | 无 |

### 阶段三：低优先级（S10-S14）

| Session | 功能 | 预计工时 | 依赖 |
|---|---|---|---|
| S10 | 多协议 LLM 适配 | 2-3 天 | 无 |
| S11 | Permission 细粒度 | 1 天 | S8 |
| S12 | Snapshot/Revert | 2 天 | 无 |
| S13 | Plugin 系统 | 3-5 天 | S11 |
| S14 | Effect 服务分层 | 0（仅参考） | — |

---

## 铁律检查清单

- [ ] 铁律一：网络代理（如需下载依赖）
- [ ] 铁律二：TDD 驱动（先写红灯测试，再实现变绿）
- [ ] 铁律三：代码注释（每个公开 API 有 `///` 文档注释）
- [ ] 铁律四：真实数据测试（非空断言）
- [ ] 铁律五：生产级代码质量（零 unwrap/expect/panic）
- [ ] 铁律六：最新版本依赖
- [ ] 铁律七：文档实时同步
- [ ] 铁律九：Rust 生态优先
- [ ] 铁律十一：GitHub 发布同步
- [ ] 铁律十二：Session 自动管理
- [ ] 铁律十三：Tailwind 样式规范（前端变更适用）
