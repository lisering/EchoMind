# Claude Code 逆向发现 → EchoMind 借鉴实施原子计划

> **来源**：对 Claude Code v2.1.88 泄露源码的逆向分析 + claurst/clawrust/ccx-rs 等重写项目的架构研究
> **目标**：将值得借鉴的发现转化为 EchoMind 可落地的改进，分 3 个 Phase / 8 个 Session 实施
> **铁律**：TDD 驱动、真实数据测试、生产级代码质量、最新版本依赖、文档实时同步

---

## Phase 1: 立即改进（高 ROI，1-2 天）

### Session 26: 系统提示词缓存架构重构

**目标**：将 `build_rag_prompt()` 从单一字符串重构为「静态前缀 + 动态边界标记 + 动态后缀」三段式架构，使 API 端可命中 prompt caching。

**改动文件**：
- `crates/core/src/chat.rs` — 重构 `build_rag_prompt()` 为 `build_rag_prompt_segmented()`
- `crates/core/src/chat_tests.rs` — 新增分段架构测试
- `crates/infra/src/openai_provider.rs` — `chat_stream` 适配分段提示词

### Session 27: Token 用量追踪

**目标**：ChatEngine 每次对话返回 token 用量，前端显示消耗。

**改动文件**：
- `crates/models/src/lib.rs` — 新增 `TokenUsage` / `ConversationCost` 模型
- `crates/core/src/chat.rs` — `ChatOutcome::Answered` 增加 `token_usage` 字段
- `crates/infra/src/openai_provider.rs` — SSE 流解析 usage 字段
- `crates/tauri-app/src/commands.rs` — `chat_done` 事件携带 token 用量
- `ui/index.html` — 聊天底部显示 token/cost

---

## Phase 2: 核心增强（1-2 周）

### Session 28: 上下文压缩引擎

**目标**：新增 `compact.rs`，当历史超过阈值时将旧消息压缩为摘要（而非丢弃），替代当前 `truncate_history` 的纯截断策略。

**改动文件**：
- `crates/core/src/compact.rs` — 新增 `CompactionEngine`
- `crates/core/src/chat.rs` — `chat()` 调用压缩引擎替代 `truncate_history`
- `crates/core/src/lib.rs` — 注册 `compact` 模块
- `crates/models/src/lib.rs` — 新增 `CompactionResult` 模型
- `crates/tauri-app/src/commands.rs` — `chat_inner` 适配压缩流程

### Session 29: 并行工具执行

**目标**：`AgentEngine` 支持一轮返回多个 Action，并行执行后汇总观察结果。

**改动文件**：
- `crates/core/src/agent.rs` — `parse_react_response` 支持多 Action 解析 + `execute_tools_parallel`
- `crates/core/src/agent_tests.rs` — 并行执行测试

### Session 30: 成本追踪 UI + 设置面板

**目标**：前端显示累计 token/成本，设置面板增加 token 预算配置。

**改动文件**：
- `ui/index.html` — 成本追踪 UI + token 预算设置
- `crates/tauri-app/src/commands.rs` — 新增 `get_conversation_cost` IPC 命令
- `crates/models/src/lib.rs` — `SettingsPayload` 增加 `token_budget` 字段

---

## Phase 3: 架构演进（1 个月）

### Session 31: autoDream 知识库自动维护引擎

**目标**：后台空闲时自动检测重复文档、发现文档间矛盾、生成整理建议。

**改动文件**：
- `crates/core/src/auto_dream.rs` — 新增 `DreamEngine`
- `crates/core/src/lib.rs` — 注册 `auto_dream` 模块
- `crates/tauri-app/src/commands.rs` — 新增 `trigger_dream` / `get_dream_suggestions` IPC
- `crates/tauri-app/src/state.rs` — AppState 增加 `dream_engine` 字段

### Session 32: Crate 拆分（echomind-prompt + echomind-compact）

**目标**：将 prompt 构建和上下文压缩从 core 拆分为独立 crate。

**改动文件**：
- `Cargo.toml` — workspace 新增 `crates/prompt` / `crates/compact`
- `crates/prompt/` — 新建 crate
- `crates/compact/` — 新建 crate
- `crates/core/src/lib.rs` — 移除已拆分模块
- 全 workspace 依赖路径更新

### Session 33: 多代理协调模式（Coordinator）

**目标**：支持 Research → Synthesis → Implementation → Verification 四阶段多代理编排。

**改动文件**：
- `crates/core/src/coordinator.rs` — 新增 `CoordinatorEngine`
- `crates/core/src/agent.rs` — 抽取共享的 LLM 调用逻辑
- `crates/tauri-app/src/commands.rs` — 新增 `set_coordinator_mode` IPC
- `ui/index.html` — 协调模式 UI

---

## 各 Session 衔接提示词

以下提示词可直接粘贴到新对话中使用，每个提示词自包含完整上下文。

---

### 📌 Session 26 提示词：系统提示词缓存架构重构

```
## 任务：系统提示词缓存架构重构（借鉴 Claude Code SYSTEM_PROMPT_DYNAMIC_BOUNDARY）

### 背景

对 Claude Code v2.1.88 逆向分析发现，其系统提示词不是单一字符串，而是使用 `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` 标记分割为：
- **静态段**（角色描述、输出格式要求等不变内容）→ 跨会话可被 API prompt caching 命中
- **动态段**（检索结果等每次变化的内容）→ 放在边界标记之后

当前 EchoMind 的 `build_rag_prompt()`（crates/core/src/chat.rs:85-103）每次将角色指令 + 检索片段 + 图表引导 + 语言引导拼接为一个字符串。静态指令和动态检索结果混在一起，无法被 API 端缓存。

### 当前代码状态

`crates/core/src/chat.rs` 的 `build_rag_prompt()` 结构：
```rust
pub(crate) fn build_rag_prompt(sources: &[RetrievalResult]) -> String {
    let mut prompt = String::from("你是「灵犀」本地知识库助手…引用片段处以 [1][2] 形式标注来源编号。\n\n");
    // 动态：检索片段（每次不同）
    for (i, src) in sources.iter().enumerate() { ... }
    // 静态：图表引导（VIZ_GUIDANCE）
    prompt.push_str(VIZ_GUIDANCE);
    // 静态：语言引导（LANG_GUIDANCE）
    prompt.push_str(LANG_GUIDANCE);
    prompt
}
```

`crates/infra/src/openai_provider.rs` 的 `chat_stream()` 直接将整个 system_prompt 作为单条 system 消息发送：
```rust
let mut messages = vec![serde_json::json!({"role": "system", "content": system_prompt})];
```

### 需要实现的改动

1. **`crates/core/src/chat.rs`**：
   - 新增 `pub const SYSTEM_PROMPT_DYNAMIC_BOUNDARY: &str = "\n\n---\n\n";`
   - 新增 `PromptSegment` 枚举：`Static(String)` / `Dynamic(String)`
   - 新增 `pub struct SegmentedPrompt { pub static_prefix: String, pub dynamic_suffix: String }`
   - 新增 `pub(crate) fn build_segmented_prompt(sources: &[RetrievalResult]) -> SegmentedPrompt`
     - static_prefix = 角色指令 + VIZ_GUIDANCE + LANG_GUIDANCE（不变部分放前面）
     - dynamic_suffix = 编号检索片段（每次变化部分）
   - 保留原 `build_rag_prompt()` 但内部委托 `build_segmented_prompt` + 拼接，保持向后兼容
   - `ChatEngine::chat()` 改用 `build_segmented_prompt`

2. **`crates/core/src/lib.rs`**：在 `LLMProvider` trait 新增方法：
   ```rust
   async fn chat_stream_segmented(
       &self,
       prompt: &SegmentedPrompt,
       history: &[ChatMessage],
       query: &str,
   ) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>> {
       // 默认实现：拼接为单字符串，委托 chat_stream（向后兼容）
       let combined = format!("{}{}{}", prompt.static_prefix, SYSTEM_PROMPT_DYNAMIC_BOUNDARY, prompt.dynamic_suffix);
       self.chat_stream(&combined, history, query).await
   }
   ```

3. **`crates/infra/src/openai_provider.rs`**：
   - 实现 `chat_stream_segmented`：将 static_prefix 作为第一条 system 消息，dynamic_suffix 作为第二条 system 消息（或合并到 user 消息前缀），利用 API prompt caching

4. **`crates/core/src/chat_tests.rs`**：新增测试：
   - `tc_prompt_cache_001`：验证 `SegmentedPrompt` 静态前缀不包含检索片段内容
   - `tc_prompt_cache_002`：验证动态后缀包含编号检索片段
   - `tc_prompt_cache_003`：验证多次调用静态前缀内容相同（缓存命中前提）
   - `tc_prompt_cache_004`：验证 `build_rag_prompt` 向后兼容（拼接结果与分段一致）

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 通过
- 现有 `chat_tests.rs` 测试不受影响（向后兼容）

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-023（系统提示词缓存架构）
- `docs/api/code-contracts.md`：新增 `SegmentedPrompt` / `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` 契约
```

---

### 📌 Session 27 提示词：Token 用量追踪

```
## 任务：Token 用量追踪（借鉴 Claude Code cost-tracker.ts）

### 背景

Claude Code 有完整的 token 预算管理和成本追踪系统（cost-tracker.ts, 11KB），实时追踪每次 API 调用的 token 消耗和费用。EchoMind 当前完全没有 token 计数和成本追踪，用户无法知道每次对话消耗了多少 token。

### 当前代码状态

- `ChatOutcome::Answered` 只有 `sources` 和 `stream`，没有 token 用量
- `OpenAIProvider::chat_stream()` 发送 SSE 请求后只解析 token 增量，忽略 SSE 最后的 `usage` 字段
- `forward_stream()` 只收集 `content`，不收集 usage
- `chat_done` 事件无 payload（空 tuple `()`）
- 前端无 token/cost 显示

OpenAI SSE 流的最后一个 chunk（`[DONE]` 之前）通常包含：
```json
{"choices": [], "usage": {"prompt_tokens": 1234, "completion_tokens": 567, "total_tokens": 1801}}
```

### 需要实现的改动

1. **`crates/models/src/lib.rs`**：新增模型
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, Default)]
   pub struct TokenUsage {
       pub prompt_tokens: usize,
       pub completion_tokens: usize,
       pub total_tokens: usize,
   }
   
   #[derive(Debug, Clone, Serialize, Deserialize, Default)]
   pub struct ChatDonePayload {
       pub token_usage: Option<TokenUsage>,
       pub model: Option<String>,
   }
   ```

2. **`crates/core/src/stream_parse.rs`**：
   - `StreamItem` 枚举新增 `Usage(TokenUsage)` 变体
   - `parse_openai_payload()` 解析 `usage` 字段，返回 `StreamItem::Usage`

3. **`crates/infra/src/openai_provider.rs`**：
   - `chat_stream()` 的 SSE 解析链路增加 `Usage` 变体处理
   - 流末尾的 usage 通过流传递（或通过单独通道返回）

4. **`crates/core/src/chat.rs`**：
   - `ChatOutcome::Answered` 增加 `token_usage: Option<TokenUsage>` 字段
   - `ChatEngine::chat()` 在流结束后填充 token_usage

5. **`crates/tauri-app/src/commands.rs`**：
   - `forward_stream()` 返回值增加 `token_usage: Option<TokenUsage>`
   - `emit_chat_done()` 改为 `emit_chat_done(app, payload: ChatDonePayload)`
   - `chat_inner` 在流结束后发射携带 token_usage 的 chat_done 事件

6. **`crates/tauri-app/tests/integration.rs`**：新增测试：
   - `tc_token_001`：Mock LLM 返回 usage，验证 forward_stream 提取
   - `tc_token_002`：验证 chat_done 事件携带 TokenUsage

7. **`ui/index.html`**：
   - 监听 `chat_done` 事件，提取 `token_usage`
   - 在消息底部显示「📊 1,234 → 567 tokens」

### 验收标准
- `cargo test` 全部通过（Free + Pro）
- `cargo clippy --all-targets -- -D warnings` 零警告
- Mock LLM 测试验证 usage 解析
- 前端显示 token 消耗

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-OBS-004（Token 用量追踪）
- `docs/api/ipc-commands.md`：`chat_done` 事件增加 `ChatDonePayload` 载荷说明
- `docs/api/code-contracts.md`：新增 `TokenUsage` / `ChatDonePayload` 契约
```

---

### 📌 Session 28 提示词：上下文压缩引擎

```
## 任务：上下文压缩引擎（借鉴 Claude Code auto-compact 机制）

### 背景

Claude Code 当消息超过阈值时，将较旧的上下文压缩为摘要（而非丢弃），保留系统提示词和最近对话。压缩后旧消息变成一条 "compacted summary" 消息注入上下文。

EchoMind 当前 `truncate_history()`（crates/core/src/chat.rs:129-193）直接丢弃中间消息——保留首轮 2 条 + 从末尾往前保留尽可能多的消息。信息永久丢失。对于律师/研究员连续提问数十轮的场景，这会导致关键上下文丢失。

### 当前代码状态

`crates/core/src/chat.rs` 的 `truncate_history()`：
- 计算 token 数 → 超限时保留前 2 条 + 末尾 N 条 → 中间消息被截断丢弃
- 返回 `TruncationResult { history, info: Option<HistoryTruncationPayload> }`
- `commands.rs` 的 `chat_inner` 调用 `truncate_history(history, context_token_limit)`

`LLMProvider` trait 有 `chat_stream(system_prompt, history, query)` 方法。
`OpenAIProvider` 有 `complete(system_prompt, user_message)` 非流式方法（HyDE 用）。

### 需要实现的改动

1. **`crates/core/src/compact.rs`**（新文件）：
   ```rust
   pub struct CompactionConfig {
       pub token_threshold: usize,      // 触发压缩的 token 阈值（默认 3072）
       pub keep_recent_turns: usize,     // 保留最近 N 轮原始消息（默认 6）
       pub keep_first_turn: bool,        // 是否保留首轮（默认 true）
       pub max_summary_tokens: usize,    // 摘要最大 token（默认 512）
   }
   
   pub struct CompactionResult {
       pub compacted_history: Vec<ChatMessage>,
       pub summary: Option<String>,      // 压缩摘要（None 表示无需压缩）
       pub compacted_count: usize,       // 被压缩的消息数
       pub original_tokens: usize,
       pub compacted_tokens: usize,
   }
   
   pub async fn compact_history<L: LLMProvider>(
       history: &[ChatMessage],
       config: &CompactionConfig,
       llm: &L,
   ) -> anyhow::Result<CompactionResult>
   ```
   
   压缩策略：
   - 若总 token ≤ threshold：返回原始历史（无需压缩）
   - 保留首轮（前 2 条）+ 最近 `keep_recent_turns * 2` 条
   - 中间消息发送给 LLM 压缩为摘要：system prompt = "请将以下对话历史压缩为简洁摘要，保留关键事实、结论和引用编号"
   - 摘要作为一条 `role: "system"` 消息插入到保留的首轮和最近消息之间

2. **`crates/core/src/chat.rs`**：
   - `ChatEngine::chat()` 签名增加 `compaction_config: Option<&CompactionConfig>` 参数
   - 若提供 config，调用 `compact_history` 替代 `truncate_history`
   - 新增 `ChatOutcome` 变体 `Compacted { sources, stream, compaction_info }`

3. **`crates/core/src/lib.rs`**：`pub mod compact;`

4. **`crates/models/src/lib.rs`**：
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CompactionInfoPayload {
       pub compacted_count: usize,
       pub original_tokens: usize,
       pub compacted_tokens: usize,
       pub summary_preview: String,  // 摘要前 100 字符预览
   }
   ```

5. **`crates/tauri-app/src/commands.rs`**：
   - `chat_inner` 在 `truncate_history` 调用点增加压缩分支
   - 读取设置 `rag.compaction_enabled`（默认 false，向后兼容）
   - 若启用压缩：调用 `compact_history`，发射 `chat_context_compacted` 事件
   - 若未启用：保持现有 `truncate_history` 行为

6. **`crates/core/src/compact_tests.rs`**（新文件）：TDD 测试
   - `tc_compact_001`：未超阈值时返回原始历史
   - `tc_compact_002`：超阈值时触发 LLM 压缩（Mock LLM 返回摘要）
   - `tc_compact_003`：保留首轮 + 最近 N 轮原始消息
   - `tc_compact_004`：摘要消息插入正确位置
   - `tc_compact_005`：压缩后总 token 低于原始

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 默认不启用压缩（`rag.compaction_enabled` 默认 false），现有行为不受影响
- 启用压缩后，Mock LLM 验证摘要生成和注入

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-024（上下文压缩）
- `docs/api/code-contracts.md`：新增 `CompactionEngine` / `CompactionConfig` / `CompactionResult` 契约
- `docs/api/settings-reference.md`：新增 `rag.compaction_enabled` / `rag.compaction_threshold` 设置键
```

---

### 📌 Session 29 提示词：并行工具执行

```
## 任务：AgentEngine 并行工具执行（借鉴 Claude Code Coordinator 并行ism）

### 背景

Claude Code 的 Coordinator Mode 明确教导 LLM「Parallelism is your superpower. Workers are async. Launch independent workers concurrently whenever possible」。ccx-rs 也实现了「所有工具在一个 turn 中并行执行」。

EchoMind 当前 `AgentEngine`（crates/core/src/agent.rs）的 ReAct 循环中，每次迭代只解析一个 Action，执行一个工具，等待结果后才进入下一轮。当 Agent 需要同时搜索多个子问题时，串行执行浪费大量等待时间。

### 当前代码状态

`crates/core/src/agent.rs`：
- `parse_react_response()` 返回 `ReactParse::ThoughtAndAction { thought, tool, input }` — 单个 Action
- `execute_tool()` 一次执行一个工具
- `run()` 循环：解析 → 执行 → 观察 → 重复

### 需要实现的改动

1. **`crates/core/src/agent.rs`**：
   - `ReactParse` 新增变体：
     ```rust
     ReactParse::ThoughtAndActions {
         thought: String,
         actions: Vec<(String, String)>,  // Vec<(tool, input)>
     }
     ```
   - `parse_react_response()` 增强：支持解析多个 `Action:` / `Action Input:` 对
     - 格式：`Thought: ...\nAction: tool1\nAction Input: input1\nAction: tool2\nAction Input: input2`
     - 向后兼容：单个 Action 仍返回 `ThoughtAndAction`
   - 新增方法：
     ```rust
     async fn execute_tools_parallel(
         &self,
         actions: &[(String, String)],
         all_sources: &mut Vec<RetrievalResult>,
     ) -> Vec<String>  // 每个工具的观察结果
     ```
     使用 `tokio::join!` 或 `futures::future::join_all` 并行执行多个 `execute_tool`
   - `run()` 循环适配：`ThoughtAndActions` 分支调用 `execute_tools_parallel`，将所有观察结果加入 `observations`

2. **`crates/core/src/agent_tests.rs`**：新增测试
   - `tc_agent_parallel_001`：Mock LLM 返回多 Action，验证两个工具都被调用
   - `tc_agent_parallel_002`：验证并行执行结果都加入 observations
   - `tc_agent_parallel_003`：单个 Action 仍走原路径（向后兼容）
   - `tc_agent_parallel_004`：一个工具失败不影响其他工具（容错）

3. **`build_agent_prompt()`** 更新：在 ReAct 格式说明中增加多 Action 示例：
   ```
   你可以在一轮中发起多个并行工具调用：
   Thought: 我需要同时搜索两个关键词
   Action: search_kb
   Action Input: 关键词1
   Action: search_kb
   Action Input: 关键词2
   ```

### 验收标准
- `cargo test -p echomind-core` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 单 Action 路径完全不受影响（向后兼容）
- 多 Action 路径验证并行执行和结果聚合

### 文档同步
- `docs/SRS_v1.0.md`：REQ-RAG-022 扩展说明（并行工具执行）
- `docs/api/code-contracts.md`：`AgentEngine` 新增 `execute_tools_parallel` 方法签名
```

---

### 📌 Session 30 提示词：成本追踪 UI + 设置面板

```
## 任务：成本追踪 UI + Token 预算设置（依赖 Session 27 完成）

### 前置条件

Session 27（Token 用量追踪）已完成：`ChatDonePayload` 携带 `TokenUsage`，`chat_done` 事件已包含 token 信息。

### 背景

Claude Code 有完整的 token 预算管理系统：每会话 token 硬上限、`cost-tracker.ts` 实时追踪、超限自动触发压缩。EchoMind 需要前端显示累计成本，并允许用户设置 token 预算。

### 需要实现的改动

1. **`crates/models/src/lib.rs`**：
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, Default)]
   pub struct ConversationCost {
       pub total_prompt_tokens: usize,
       pub total_completion_tokens: usize,
       pub total_tokens: usize,
       pub exchange_count: usize,
       pub estimated_cost_usd: Option<f64>,
   }
   
   // SettingsPayload 新增字段
   pub token_budget: Option<usize>,  // 每会话 token 上限
   ```

2. **`crates/infra/src/sqlite_storage.rs`**：
   - 新增方法 `get_conversation_cost(conversation_id) -> ConversationCost`
   - 从 messages 表累计 token 用量（需在 add_message 时存储 token_usage）
   - messages 表新增 `prompt_tokens` / `completion_tokens` 列（migration）

3. **`crates/tauri-app/src/commands.rs`**：
   - 新增 `#[tauri::command] pub async fn get_conversation_cost(conversation_id, state) -> Result<ConversationCost, String>`
   - 新增 `#[tauri::command] pub async fn set_token_budget(budget: usize, state) -> Result<(), String>`
   - `persist_exchange()` 增加参数 `token_usage: Option<TokenUsage>`，写入 messages 表

4. **`crates/tauri-app/src/lib.rs`**：注册新命令

5. **`ui/index.html`**：
   - 聊天区底部状态栏：显示当前会话累计 token「📊 12,345 tokens | 5 轮对话」
   - 每条助手消息底部：显示该轮 token「1,234 → 567」
   - 设置面板 → 高级设置：Token 预算输入框 + 预算耗尽行为选择（警告/停止/自动压缩）
   - 预算耗尽时前端 toast 警告

6. **`crates/tauri-app/tests/integration.rs`**：新增测试
   - `tc_cost_001`：`get_conversation_cost` 返回正确累计
   - `tc_cost_002`：`set_token_budget` 持久化到 settings 表

### 验收标准
- `cargo test` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 前端正确显示 token 消耗
- 设置面板可配置 token 预算

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-OBS-005（Token 预算管理）
- `docs/api/ipc-commands.md`：新增 `get_conversation_cost` / `set_token_budget` 命令
- `docs/api/settings-reference.md`：新增 `rag.token_budget` 设置键
- `docs/api/frontend-modules.md`：新增前端 token 显示函数
```

---

### 📌 Session 31 提示词：autoDream 知识库自动维护引擎

```
## 任务：autoDream 知识库自动维护引擎（借鉴 Claude Code autoDream 系统）

### 背景

Claude Code 的 autoDream 是后台记忆整合引擎，三重触发门（24h + 5 会话 + 锁），四阶段流程：
1. Orient：扫描现有记忆
2. Gather：收集新信息
3. Consolidate：写入/更新记忆，消除矛盾
4. Prune：保持索引精简

EchoMind 可将此模式应用于知识库维护——空闲时自动检测重复文档、发现文档间矛盾、生成整理建议。EchoMind 已有 `audit.rs`（文档一致性审计引擎），可复用其 `ContradictionPair` 逻辑。

### 当前代码状态

- `crates/core/src/audit.rs`：`AuditEngine` 全量扫描式矛盾检测，Pro feature
- `crates/infra/src/sqlite_storage.rs`：有 `vector_search` / `keyword_search` / `list_documents` / `list_chunks`
- `crates/core/src/retriever.rs`：`VectorRetriever` 有 `embedder` + `storage`
- `AppState` 有 `storage` / `embedder()` / `data_dir`

### 需要实现的改动

1. **`crates/core/src/auto_dream.rs`**（新文件）：
   ```rust
   pub struct DreamConfig {
       pub idle_threshold_secs: u64,       // 闲置触发阈值（默认 86400 = 24h）
       pub min_new_docs_since_last: usize, // 最少新增文档数（默认 5）
       pub max dream_tokens: usize,        // 梦境摘要最大 token
   }
   
   pub struct DreamSuggestion {
       pub kind: DreamSuggestionKind,  // Duplicate / Contradiction / Stale / Orphan
       pub doc_ids: Vec<String>,
       pub description: String,
       pub confidence: f32,
   }
   
   pub enum DreamSuggestionKind {
       DuplicateDocuments,    // 嵌入余弦相似度 > 0.95 的文档对
       ContentContradiction,  // 复用 audit.rs 的矛盾检测
       StaleDocument,         // 长时间未更新的文档
       OrphanChunk,           // 无对应文档的孤立分块
   }
   
   pub struct DreamResult {
       pub suggestions: Vec<DreamSuggestion>,
       pub scanned_docs: usize,
       pub duration_ms: u64,
   }
   
   pub async fn run_dream<E: Embedder, S: Storage>(
       embedder: &E,
       storage: &S,
       config: &DreamConfig,
   ) -> anyhow::Result<DreamResult>
   ```
   
   四阶段流程：
   - **Orient**：`storage.list_documents()` + `storage.count_documents()`，读取文档元数据
   - **Gather**：对每对文档计算嵌入余弦相似度（复用 `vector_search`），检测重复
   - **Consolidate**：复用 `audit.rs` 的 `AuditEngine` 检测内容矛盾
   - **Prune**：检测孤立分块、过期文档，生成整理建议

2. **`crates/core/src/lib.rs`**：`pub mod auto_dream;`

3. **`crates/tauri-app/src/commands.rs`**：
   - 新增 `#[tauri::command] pub async fn trigger_dream(state) -> Result<DreamResult, String>`
   - 新增 `#[tauri::command] pub async fn get_dream_suggestions(state) -> Result<Vec<DreamSuggestion>, String>`
   - 新增 `#[tauri::command] pub async fn dismiss_dream_suggestion(id, state) -> Result<(), String>`

4. **`crates/tauri-app/src/state.rs`**：
   - AppState 增加 `dream_suggestions: Arc<RwLock<Vec<DreamSuggestion>>>`
   - 增加 `last_dream_at: Arc<RwLock<Option<Instant>>>`

5. **`crates/tauri-app/src/lib.rs`**：注册 3 个新命令

6. **`ui/index.html`**：
   - 侧栏知识库区域：空闲时显示「💤 知识库整理建议 (N)」badge
   - 点击展开建议列表：每条建议显示类型/文档名/描述/「忽略」按钮
   - 设置面板：自动整理开关 + 整理触发条件配置

7. **`crates/core/src/auto_dream_tests.rs`**（新文件）：TDD 测试
   - `tc_dream_001`：无文档时返回空建议
   - `tc_dream_002`：检测到高相似度文档对（Mock embedder 返回相同向量）
   - `tc_dream_003`：检测到孤立分块
   - `tc_dream_004`：建议置信度计算正确
   - `tc_dream_005`：DreamResult 包含扫描耗时

### 验收标准
- `cargo test` 全部通过（Free + Pro）
- `cargo clippy --all-targets --features pro -- -D warnings` 零警告
- 不自动删除任何文档（仅生成建议）
- Pro feature gating（`#[cfg(feature = "pro")]`），Free 版 stub 返回空

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-VEC-014（知识库自动维护）
- `docs/api/ipc-commands.md`：新增 `trigger_dream` / `get_dream_suggestions` / `dismiss_dream_suggestion` 命令
- `docs/api/code-contracts.md`：新增 `DreamEngine` / `DreamSuggestion` 契约
- `docs/api/settings-reference.md`：新增 `dream.enabled` / `dream.idle_threshold` 设置键
- `docs/architecture/adr/`：新增 ADR-012（autoDream 知识库自动维护引擎）
```

---

### 📌 Session 32 提示词：Crate 拆分

```
## 任务：Crate 拆分 — echomind-prompt + echomind-compact

### 背景

clawrust 将系统拆分为 13 个 crate，其中 `claude-prompt`（提示词构建）和 `claude-compact`（上下文压缩）是独立 crate。随着 EchoMind 功能增长（compact、dream、更多工具），core crate 越来越臃肿。将 prompt 构建和上下文压缩拆分为独立 crate 可以改善编译速度和模块边界。

### 前置条件

Session 26（系统提示词缓存架构）和 Session 28（上下文压缩引擎）已完成。

### 需要实现的改动

1. **`Cargo.toml`**（workspace root）：
   ```toml
   members = [
       "crates/models",
       "crates/prompt",     # 新增
       "crates/compact",    # 新增
       "crates/core",
       "crates/infra",
       "crates/tauri-app",
       "crates/license-issuer",
   ]
   ```

2. **`crates/prompt/`**（新 crate）：
   - `Cargo.toml`：依赖 `echomind-models`
   - `src/lib.rs`：
     - 迁移 `SegmentedPrompt` / `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` / `build_segmented_prompt` / `build_rag_prompt` / `VIZ_GUIDANCE` / `LANG_GUIDANCE`
     - 迁移 `build_agent_prompt` / `build_final_rag_prompt`（from agent.rs）
     - 公开 API：`pub fn build_segmented_prompt(...)` / `pub fn build_rag_prompt(...)` / `pub fn build_agent_prompt(...)`

3. **`crates/compact/`**（新 crate）：
   - `Cargo.toml`：依赖 `echomind-models` / `echomind-prompt` / `echomind-core`（仅 trait）
   - `src/lib.rs`：
     - 迁移 `CompactionEngine` / `CompactionConfig` / `CompactionResult` / `compact_history`
     - 迁移 `compact_tests.rs`

4. **`crates/core/src/chat.rs`**：
   - 移除 `build_rag_prompt` / `build_segmented_prompt` / `SegmentedPrompt` / `VIZ_GUIDANCE` / `LANG_GUIDANCE`
   - 改为 `use echomind_prompt::{build_segmented_prompt, SegmentedPrompt};`
   - `ChatEngine` 保留但 prompt 构建委托给 `echomind-prompt`

5. **`crates/core/src/agent.rs`**：
   - 移除 `build_agent_prompt` / `build_final_rag_prompt`
   - 改为 `use echomind_prompt::{build_agent_prompt, build_final_rag_prompt};`

6. **`crates/core/src/lib.rs`**：
   - 移除 `pub mod compact;`
   - `Cargo.toml` 新增 `echomind-prompt` / `echomind-compact` 依赖

7. **全 workspace**：
   - `crates/infra/Cargo.toml`：新增 `echomind-prompt` 依赖（OpenAIProvider 需要 SegmentedPrompt）
   - `crates/tauri-app/Cargo.toml`：新增 `echomind-prompt` / `echomind-compact` 依赖
   - `crates/tauri-app/src/commands.rs`：更新导入路径

### 验收标准
- `cargo build` 全 workspace 编译通过
- `cargo test` 全部通过（Free + Pro）
- `cargo clippy --all-targets -- -D warnings` 零警告
- `cargo fmt --check` 通过
- 编译速度改善（core crate 变小）

### 文档同步
- `AGENTS.md`：更新 Architecture 表（6 crate → 8 crate）
- `CLAUDE.md`：更新 crate 列表
- `docs/api/code-contracts.md`：更新 crate 依赖方向图
```

---

### 📌 Session 33 提示词：多代理协调模式

```
## 任务：多代理协调模式（借鉴 Claude Code Coordinator Mode）

### 背景

Claude Code 的 Coordinator Mode 将 Claude Code 从单代理转变为协调器，四阶段流程：
1. Research（Workers 并行）：调研代码库，发现文件，理解问题
2. Synthesis（Coordinator）：阅读发现，理解问题，制定规格
3. Implementation（Workers）：按规格做针对性修改
4. Verification（Workers）：验证变更有效

EchoMind 当前 `AgentEngine` 是单代理串行 ReAct 循环。对于复杂研究查询（如「对比这三份合同的法律风险」），可以并行检索多份文档，再综合分析。

### 前置条件

Session 29（并行工具执行）已完成。

### 需要实现的改动

1. **`crates/core/src/coordinator.rs`**（新文件）：
   ```rust
   pub struct CoordinatorConfig {
       pub max_workers: usize,           // 最大并行 worker 数（默认 3）
       pub research_top_k: usize,        // 每个 worker 检索 top-k（默认 5）
       pub synthesis_max_tokens: usize,  // 综合阶段最大 token
   }
   
   pub struct CoordinatorOutcome {
       pub research_results: Vec<ResearchFinding>,  // 各 worker 的研究发现
       pub synthesis: String,                       // Coordinator 综合分析
       pub final_answer_stream: BoxStream<'static, Result<String>>,
       pub sources: Vec<RetrievalResult>,
   }
   
   pub struct CoordinatorEngine<R: Retriever, L: LLMProvider> {
       retriever: R,
       llm: L,
       config: CoordinatorConfig,
   }
   ```
   
   四阶段实现：
   - **Research**：将用户查询分解为子查询（通过 LLM 或规则），每个子查询并行调用 `retriever.retrieve()`，收集研究发现
   - **Synthesis**：将所有研究发现发送给 LLM，生成综合分析（非流式 `complete()`）
   - **Implementation**：基于综合分析构建最终 RAG prompt，流式生成答案
   - **Verification**：验证答案是否引用了所有相关来源

2. **`crates/core/src/lib.rs`**：`pub mod coordinator;`

3. **`crates/tauri-app/src/commands.rs`**：
   - `chat_inner` 增加 Coordinator 模式分支
   - 读取设置 `rag.coordinator_enabled`（默认 false）
   - 启用时：`CoordinatorEngine::new(retriever, provider).run(history, query)`
   - 发射 `coordinator_phase` 事件（researching/synthesizing/generating）

4. **`crates/tauri-app/src/lib.rs`**：注册 `set_coordinator_mode` 命令

5. **`ui/index.html`**：
   - 设置面板：协调模式开关
   - 聊天区：显示协调阶段进度（「🔍 并行检索 3 个子问题…」→「🧠 综合分析…」→「✍️ 生成答案…」）

6. **`crates/core/src/coordinator_tests.rs`**（新文件）：TDD 测试
   - `tc_coord_001`：单子查询时等价于标准 RAG
   - `tc_coord_002`：多子查询并行检索（Mock Retriever 验证并行调用）
   - `tc_coord_003`：综合阶段正确合并多源信息
   - `tc_coord_004`：最终答案流包含所有来源引用
   - `tc_coord_005`：降级策略（综合失败 → 标准单轮 RAG）

### 验收标准
- `cargo test` 全部通过
- `cargo clippy --all-targets -- -D warnings` 零警告
- 默认关闭协调模式（`rag.coordinator_enabled` 默认 false）
- 启用后验证并行检索和综合分析

### 文档同步
- `docs/SRS_v1.0.md`：新增 REQ-RAG-025（多代理协调模式）
- `docs/api/ipc-commands.md`：新增 `set_coordinator_mode` 命令 + `coordinator_phase` 事件
- `docs/api/code-contracts.md`：新增 `CoordinatorEngine` / `CoordinatorConfig` / `CoordinatorOutcome` 契约
- `docs/api/settings-reference.md`：新增 `rag.coordinator_enabled` 设置键
- `docs/architecture/adr/`：新增 ADR-013（多代理协调模式）
```

### ✅ Session 33 完成详情（2026-08-01）

**实现内容**：
- `crates/core/src/coordinator.rs` — CoordinatorEngine 多代理协调引擎（Pro 无门控，全量可用）
- `crates/core/src/coordinator_tests.rs` — 7 个 TDD 测试（TC-COORD-001~007）
- `crates/core/src/lib.rs` — 注册 coordinator + coordinator_tests 模块
- `crates/tauri-app/src/commands.rs` — 新增 set_coordinator_mode IPC 命令 + _inner + emit_coordinator_phase 助手 + chat_inner 协调模式分支
- `crates/tauri-app/src/lib.rs` — 注册 set_coordinator_mode 命令
- `ui/index.html` — 设置面板协调模式开关 + onCoordinatorToggle 函数
- `ui/locales/en.json` + `ui/locales/zh-CN.json` — 5 个 i18n 键
- `crates/tauri-app/tests/integration.rs` — 2 个集成测试（TC-COORD-001~002）+ 4 个 Token/Cost 集成测试（TC-TOKEN-001~002 + TC-COST-001~002）

**新增 IPC 命令**：`set_coordinator_mode(enabled: bool) -> ()`
**新增设置键**：`rag.coordinator_enabled`（bool，默认 false）
**新增事件**：`coordinator_phase`（phase/message/sub_query_count）

**关键设计**：
- 四阶段流水线：Research（子查询分解 + 并行检索）→ Synthesis（LLM 综合分析）→ Implementation（分段式 RAG prompt 流式生成）→ Verification（来源去重验证）
- 子查询分解：通过 LLM `chat_stream` 发送 JSON 数组分解指令，解析失败降级为原始查询
- 并行检索：`futures::future::join_all` 并发执行全部子查询检索
- 综合分析注入：综合摘要作为前缀注入 `dynamic_context`，利用 prompt caching
- 来源去重：按 `chunk.id` 去重，防止重复引用
- 降级策略：子查询分解失败→单次检索；综合分析失败→标准 RAG；全部失败→NoContext

**验证结果**：
- cargo test --features pro: 761 passed, 0 failed, 20 ignored（core 283 + infra 298 + models 12 + tauri-app 9 + integration 142 + license-issuer 10 + doc-tests 7）
- cargo clippy --all-targets -- -D warnings（Free）：零警告
- cargo clippy --all-targets --features pro -- -D warnings（Pro）：零警告
- cargo fmt --check：通过

---

## 实施注意事项

### 铁律遵守

1. **TDD 驱动**：每个 Session 先写测试（红灯），再实现（绿灯），最后重构
2. **代码注释**：所有新增 pub 项必须有 `///` 文档注释
3. **真实数据测试**：Mock 测试 + 至少一个真实数据集成测试
4. **生产级代码质量**：零 clippy 警告，零 unwrap/expect/panic
5. **最新版本依赖**：新增 crate 使用最新稳定版本
6. **文档实时同步**：每个 Session 完成后立即同步 SRS / IPC / Contracts / Settings 文档
7. **Free/Pro 双版本**：Pro feature 使用 `#[cfg(feature = "pro")]` 门控

### Session 依赖关系

```
Session 26 (Prompt 缓存) ──────────────────────┐
                                                ├──→ Session 32 (Crate 拆分)
Session 27 (Token 追踪) → Session 30 (成本 UI)  │
                                                │
Session 28 (上下文压缩) ────────────────────────┘
                                                
Session 29 (并行工具) → Session 33 (Coordinator)

Session 31 (autoDream) — 独立，无前置依赖
```

### 优先级建议

1. **最高 ROI**：Session 26（Prompt 缓存）+ Session 27（Token 追踪）— 立即可做，直接降低成本
2. **高价值**：Session 28（上下文压缩）— 直接提升长对话质量
3. **中等价值**：Session 29（并行工具）+ Session 30（成本 UI）
4. **长期演进**：Session 31（autoDream）+ Session 32（Crate 拆分）+ Session 33（Coordinator）

