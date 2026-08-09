# QM 源码分析发现 → EchoMind 借鉴实施原子计划

> **来源**：[qm](https://github.com/yc-software/qm) — 多人协作 AI Agent 平台（TypeScript / Node.js / Postgres）
> **分析范围**：`/Users/john/code/qm-main` 全量源码阅读（`src/` 80+ 子目录、`harness/`、`memory/`、`skills/`、`sandbox/`、`security/`、`core/orchestrator/`、`wiring.ts`）
> **目标**：将 QM 中值得借鉴的架构模式转化为 EchoMind 可落地的改进，分 5 个 Phase / 11 个 Session 实施
> **铁律**：TDD 驱动、真实数据测试、生产级代码质量、最新版本依赖、文档实时同步、Rust 生态优先
> **前置条件**：当前测试基线 Free 808 passed / Pro 1094 passed / Clippy 零警告 / v1.4.0 已发布 / 159 IPC 命令

---

## 调研发现总览

| QM 模块 | 核心发现 | 借鉴价值 | 对应 Session |
|---|---|---|---|
| `memory/` | Scratch-Promote 两层记忆 + Consolidation 引擎 + Burst Buffer 延迟捕获 + Provenance 标记 | ★★★★★ 高 | S66–S67 |
| `harness/context-compaction.ts` + `core/orchestrator/compaction.ts` | 双阈值压缩（Soft 70% 后台 / Hard 90% 同步）+ Token 级预算 + Entry Token 缓存 + Tape 格式 | ★★★★★ 高 | S68–S69 |
| `security/security-posture.ts` + `core/orchestrator/security-screen.ts` | 三层安全态势（Dangerous/Auto/Strict）+ Shadow 筛查 + Security-tainted 条目过滤 | ★★★★☆ 高 | S70–S71 |
| `idempotency/` + `util/sweeper.ts` | 幂等性存储 + 统一周期任务抽象 | ★★★☆☆ 中 | S72 |
| `ratelimit/budget.ts` + `ratelimit/rate-limiter.ts` | 预算追踪 + 速率限制 + 成本估算 | ★★★☆☆ 中 | S73 |
| `util/async.ts` | KeyedQueue 并发序列化 + Durable-by-Default 原则 | ★★☆☆☆ 中 | S74 |
| `harness/harness.ts` + `harness/harness-router.ts` | Harness 抽象 + 运行时路由器 + 多后端动态切换 | ★★★☆☆ 中 | S75–S76 |

---

## Phase 1: 记忆系统增强（高 ROI，2–3 天）

### Session 66: Scratch-Promote 两层记忆 + Consolidation 引擎

**目标**：借鉴 QM 的 scratch-promote 两层记忆架构，在现有 `MemoryStore`（Wing/Hall/Room 三层）基础上增加「Scratch 层」和「LLM 驱动的 Consolidation 引擎」，使记忆从自动捕获 → scratch 暂存 → LLM 审查 → promote 到长期层，减少噪音事实永久驻留。

**QM 对比**：
- QM `memory/strategies/scratch-promote.ts`：日常写入 `memory/log/YYYY-MM-DD.md`，累积阈值后 LLM 审查 promote 到 `MEMORY.md`
- QM `memory/strategies/consolidation.ts`：LLM 审查积累事实，执行 `UPDATE n` / `DELETE n` / `ADD: text` 操作
- EchoMind 现状：`MemoryStore` 有三层提升（Wing→Hall→Room），但缺少 scratch 层和 LLM 整合

**改动文件**：
- `crates/models/src/lib.rs` — 新增 `MemoryConsolidationAction` 枚举 + `ScratchLogEntry` 结构体
- `crates/core/src/memory_store.rs` — `MemoryStore` 新增 scratch 层逻辑 + `consolidate()` 方法
- `crates/core/src/memory_store_tests.rs` — TDD 测试
- `crates/infra/src/sqlite_storage.rs` — 新增 `scratch_logs` 表（date/content/created_at）+ 4 个方法
- `crates/core/src/lib.rs` — `Storage` trait 新增 4 个 scratch 方法（默认空操作）
- `crates/core/src/auto_dream.rs` — `run_dream()` 集成 consolidation 阶段

**数据模型**：
```rust
/// 记忆整合动作（借鉴 QM consolidation.ts 的 UPDATE/DELETE/ADD 模式）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MemoryConsolidationAction {
    /// 更新已有记忆
    Update { id: String, content: String },
    /// 删除过时/重复记忆
    Delete { id: String },
    /// 新增记忆
    Add { content: String, tier: MemoryTier },
}

/// Scratch 层日志条目（借鉴 QM scratch-promote 的 daily log）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScratchLogEntry {
    pub id: String,
    pub date: String,        // YYYY-MM-DD
    pub content: String,      // markdown bullet: "- (date) fact"
    pub created_at: i64,
}
```

**Consolidation Prompt**（借鉴 QM `MEMORY_CONSOLIDATION_PROMPT`）：
```text
你正在整理 Agent 的长期记忆。输入是一个编号列表，每条记忆可能带有 (YYYY-MM-DD) 捕获日期。
仅输出动作，每行一个，格式如下：
UPDATE <n>: <修订后的事实>
DELETE <n>
ADD: <新事实>
如果无需变更，输出: NONE

规则：
- 优先 UPDATE 而非 DELETE+ADD（事实演变或两条应合并时）
- DELETE 过时、被新事实矛盾、精确或近似重复、可从其他事实推导的记忆
- 保持事实原子性：每行一条独立事实
- 永不删除用户明确要求记住的事实
- 保留 (said in ...) 后缀，它记录事实来源
```

**TDD 测试**：
- `tc_scratch_001`：写入 scratch 日志（追加到当日文件）
- `tc_scratch_002`：scratch 日志 14 天后自动清理
- `tc_scratch_003`：consolidate 解析 LLM 输出为 UPDATE/DELETE/ADD 动作列表
- `tc_scratch_004`：apply_consolidation_actions 正确应用动作到 MemoryStore
- `tc_scratch_005`：consolidate 后 scratch 层条目数归零
- `tc_scratch_006`：consolidate 失败时降级为保留原状（不返回 Err）
- `tc_scratch_007`：AutoDream 触发 consolidation（集成测试）

**验收标准**：
- [ ] `cargo test` 全部通过（Free + Pro）
- [ ] `cargo clippy --all-targets -- -D warnings` 零警告
- [ ] Scratch 层只在 `memory.auto_consolidate` 开启时激活（默认 false，向后兼容）
- [ ] Consolidation 由 AutoDream 后台触发，不阻塞对话
- [ ] Consolidation 失败降级为保留原状

**新增 IPC 命令**（2 个）：
- `trigger_memory_consolidation() -> ()` — 手动触发记忆整合
- `get_scratch_logs(limit: Option<usize>) -> Vec<ScratchLogEntry>` — 查看 scratch 层

**新增设置键**：`memory.auto_consolidate`（bool，默认 false）

### Session 67: Burst Buffer 延迟捕获 + Provenance 标记

**目标**：借鉴 QM 的 Burst Buffer 模式，将记忆提取从「每轮立即调用 LLM」改为「聚合多轮后批量提取」，降低 LLM 调用成本。同时引入 Provenance 标记，记录每条记忆的来源（哪个对话/哪条消息）。

**QM 对比**：
- QM `memory/strategies/per-turn.ts`：`createBurstBuffer(quietMs, maxTurns, flush)` — 静默窗口 + 最大轮次聚合
- QM `memory/memory-service.ts`：`ccCaptureToPersonal()` — 频道捕获的事实标记 `(said in #channel)` 并 CC 到个人 scope
- EchoMind 现状：`MemoryStore::extract_from_conversation()` 每轮调用，无聚合

**改动文件**：
- `crates/core/src/memory_store.rs` — 新增 `BurstBuffer` 结构体 + `ProvenanceTag`
- `crates/core/src/memory_store_tests.rs` — TDD 测试
- `crates/tauri-app/src/commands/features.rs` — `chat_done` 后异步触发 burst buffer flush
- `crates/tauri-app/src/state.rs` — AppState 新增 `memory_burst_buffer` 字段

**核心实现**：
```rust
/// 记忆来源标记（借鉴 QM provenance tracking）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceTag {
    /// 来源对话 ID
    pub conversation_id: String,
    /// 来源消息 seq
    pub message_seq: usize,
    /// 捕获时间戳
    pub captured_at: i64,
    /// 来源标签（如 "对话：xxx 的第 3 轮"）
    pub source_label: String,
}

/// Burst Buffer — 延迟批量记忆提取（借鉴 QM createBurstBuffer）
pub struct BurstBuffer {
    /// 待处理的多轮对话
    pending: Vec<(String, String)>, // (user_msg, assistant_reply)
    /// 静默窗口（ms），超过此时间无新轮次则 flush
    quiet_ms: u64,
    /// 最大聚合轮次
    max_turns: usize,
    /// 上次活动时间
    last_activity: Instant,
    /// 定时器
    timer: Option<tokio::task::JoinHandle<()>>,
}
```

**TDD 测试**：
- `tc_burst_001`：push 添加轮次到 buffer
- `tc_burst_002`：should_flush 在 max_turns 达到时返回 true
- `tc_burst_003`：should_flush 在静默窗口过后返回 true
- `tc_burst_004`：should_flush 在空 buffer 时返回 false
- `tc_burst_005`：flush 从聚合轮次提取记忆并写入 scratch
- `tc_burst_006`：flush 空 buffer 是 no-op
- `tc_burst_007`：drain 取出所有 pending 并清空 buffer
- `tc_burst_008`：flush LLM 失败时降级为返回 0
- `tc_burst_009`：flush_burst_turns 独立函数正确处理多轮聚合
- `tc_burst_010`：with_config 自定义参数生效

**验收标准**：
- [x] `cargo test` 全部通过（core 540 passed + models 40 passed）
- [x] `cargo clippy` 零警告
- [x] Burst Buffer 默认启用，quiet_ms = 180000ms，max_turns = 10（可配置）
- [x] 每条记忆携带 Provenance 标记（(said in ...) 后缀）
- [x] 前端可通过 IPC 查询/触发 burst buffer 状态

---

## Phase 2: 上下文压缩增强（高 ROI，1–2 天）

### Session 68: 双阈值压缩 + 后台异步压缩

**目标**：借鉴 QM 的双阈值压缩机制，将现有 `CompactionEngine` 从「单阈值同步压缩」升级为「Soft 阈值后台异步 + Hard 阈值同步兜底」，消除长对话卡顿。

**QM 对比**：
- QM `core/orchestrator/compaction.ts`：Soft 70% 触发 `scheduleBackgroundCompaction()`（不阻塞当前轮次），Hard 90% 触发 `compactContextIfNeeded()`（同步阻塞）
- QM 后台压缩：获取 session lease → 读取历史 → LLM 摘要 → 写入 summary entry → 释放 lease
- EchoMind 现状：`CompactionEngine::compact()` 单阈值，同步调用，长对话时用户等待

**改动文件**：
- `crates/compact/src/lib.rs` — `CompactionEngine` 新增 `compact_if_needed()` + `schedule_background_compaction()`
- `crates/compact/src/compact_tests.rs` — TDD 测试
- `crates/models/src/lib.rs` — `CompactionResult` 新增 `compaction_kind` 字段（None/Background/Synchronous）
- `crates/tauri-app/src/commands/chat.rs` — `chat_inner` 集成双阈值逻辑
- `crates/tauri-app/src/state.rs` — AppState 新增 `compaction_pending: Arc<tokio::sync::Mutex<HashSet<String>>>`

**核心实现**：
```rust
/// 压缩触发类型
#[derive(Debug, Clone, PartialEq)]
pub enum CompactionTrigger {
    /// 未触发
    None,
    /// Soft 阈值（70%）— 后台异步压缩
    Background,
    /// Hard 阈值（90%）— 同步压缩
    Synchronous,
}

impl<'a, L: LLMProvider> CompactionEngine<'a, L> {
    /// 检查是否需要压缩（借鉴 QM overBudgetFraction + COMPACT_SOFT/HARD_FRACTION）
    pub fn check_compaction_needed(
        &self,
        history: &[ChatMessage],
        max_entries: usize,
        max_tokens: usize,
    ) -> CompactionTrigger {
        let soft_fraction = 0.7;
        let hard_fraction = 0.9;
        let entry_ratio = history.len() as f64 / max_entries as f64;
        let token_ratio = estimate_history_tokens(history) as f64 / max_tokens as f64;
        let max_ratio = entry_ratio.max(token_ratio);
        if max_ratio >= hard_fraction {
            CompactionTrigger::Synchronous
        } else if max_ratio >= soft_fraction {
            CompactionTrigger::Background
        } else {
            CompactionTrigger::None
        }
    }

    /// 后台异步压缩（不阻塞当前轮次）
    pub fn schedule_background_compaction(
        &self,
        conversation_id: &str,
        history: Vec<ChatMessage>,
        max_tokens: usize,
        storage: Arc<impl Storage>,
    ) {
        tokio::spawn(async move {
            // 1. 检查是否已有后台压缩在进行
            // 2. 压缩
            // 3. 持久化 summary entry
            // 4. 清除 pending 标记
        });
    }
}
```

**TDD 测试**：
- `tc_compact_dual_001`：历史 < 70% → None
- `tc_compact_dual_002`：历史 70%–90% → Background
- `tc_compact_dual_003`：历史 > 90% → Synchronous
- `tc_compact_dual_004`：token_ratio 超阈值优先返回更高等级
- `tc_compact_dual_005`：同步压缩返回压缩后历史
- `tc_compact_dual_006`：后台压缩失败不影响对话（错误吞咽）
- `tc_compact_dual_007`：同一会话不重复触发后台压缩（compaction_pending 去重）
- `tc_compact_dual_008`：DualThresholdConfig 默认值正确
- `tc_compact_dual_009`：CompactionTrigger 序列化/反序列化（IPC 兼容）
- `tc_compact_dual_010`：from_settings 使用默认值填充缺失项
- `tc_compact_dual_011`：max_entries=0 或 max_tokens=0 防除零返回 None
- `tc_compact_dual_012`：CompactionKind as_str + serde 测试

**新增设置键**：`compaction.soft_threshold`（默认 0.7）/ `compaction.hard_threshold`（默认 0.9）/ `compaction.max_entries`（默认 50）/ `compaction.max_tokens`（默认 8192）

**验收标准**：
- [x] `cargo test` 全部通过（compact 41 passed + 1 ignored / models 40 passed）
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo fmt --check` 通过
- [x] Soft 阈值 70% 触发后台异步压缩（不阻塞当前轮次）
- [x] Hard 阈值 90% 触发同步压缩（阻塞兜底）
- [x] 后台压缩去重（同一会话不重复触发）
- [x] 后台压缩失败不影响对话（错误吞咽）
- [x] `CompactionInfo` 新增 `compaction_kind` 字段（区分 Background/Synchronous）
- [x] `AppState` 新增 `compaction_pending` 字段（去重集合）
- [x] `chat_inner` 集成双阈值逻辑（Soft → spawn 后台 / Hard → 同步 / None → 跳过）

### Session 69: Token 级预算 + Entry Token 缓存 + Dangling Call 修复 ✅

**目标**：借鉴 QM 的 Token 级预算管理和 Entry Token 缓存，提升压缩精度。同时引入 Dangling Call 修复（Agent 模式下 tool_call 被中断时自动补全 tool_result）。

**完成状态**：
- [x] `cargo test` 全部通过（compact 47 passed + 1 ignored）
- [x] `cargo clippy --all-targets -- -D warnings` 零警告
- [x] `cargo fmt --check` 通过
- [x] TokenEstimator 带 LRU 缓存（HashMap + Vec LRU 顺序 + RwLock 并发安全）
- [x] `estimate()` / `estimate_messages()` / `recent_entry_count_within_budget()` 方法
- [x] `heal_dangling_calls()` 修复未配对的 Action（内容级 ReAct 检测）
- [x] 6 TDD 测试 TC-TOKEN-CACHE-001~003 + TC-HEAL-DANGLING-001~003

**QM 对比**：
- QM `harness/context-compaction.ts`：`estimateEntryTokens()` 带 LRU 缓存（50k 条），`recentEntryCountWithinBudget()` 从末尾向前保留尽可能多的消息
- QM `harness/tape-fold.ts`：`healDanglingCalls()` — 未配对的 tool_call 自动添加 `[interrupted]` 占位 result
- EchoMind 现状：`CompactionEngine` 使用 `bpe()` 逐条计算 token，无缓存

**改动文件**：
- `crates/compact/src/lib.rs` — 新增 `TokenEstimator` 带 LRU 缓存 + `heal_dangling_calls()`
- `crates/compact/src/compact_tests.rs` — TDD 测试

**核心实现**：
```rust
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Token 估算器（借鉴 QM estimateEntryTokens 的 LRU 缓存）
pub struct TokenEstimator {
    /// 缓存：message_content_hash → token_count
    cache: Arc<RwLock<HashMap<u64, usize>>>,
    /// 缓存上限
    max_cache_size: usize,
}

impl TokenEstimator {
    pub fn new(max_cache: usize) -> Self { ... }

    /// 估算消息 token 数（带缓存）
    pub async fn estimate(&self, content: &str) -> usize {
        let hash = calculate_hash(content);
        if let Some(&count) = self.cache.read().await.get(&hash) {
            return count;
        }
        let count = bpe_encode_count(content);
        // LRU 驱逐
        let mut cache = self.cache.write().await;
        if cache.len() >= self.max_cache_size {
            // 移除最旧的条目
            if let Some(&first_key) = cache.keys().next() {
                cache.remove(&first_key);
            }
        }
        cache.insert(hash, count);
        count
    }
}

/// 修复悬空的 tool_call（借鉴 QM healDanglingCalls）
/// Agent 模式下 tool_call 被中断时，自动补全 [interrupted] 占位 result
pub fn heal_dangling_calls(history: &mut Vec<ChatMessage>) {
    // 1. 收集所有已回答的 tool_call_id
    // 2. 遍历 assistant 消息中的 tool_call
    // 3. 未配对的添加 [interrupted — 平台重启时此工具调用未完成] 占位 result
}
```

**TDD 测试**：
- `tc_token_cache_001`：相同内容第二次估算命中缓存
- `tc_token_cache_002`：缓存满时 LRU 驱逐最旧条目
- `tc_token_cache_003`：`recent_entry_count_within_budget` 正确从末尾保留
- `tc_heal_dangling_001`：单个未配对 tool_call 被修复
- `tc_heal_dangling_002`：多个未配对 tool_call 全部修复
- `tc_heal_dangling_003`：已配对的 tool_call 不受影响

---

## Phase 3: 安全层增强（高 ROI，2–3 天）

### Session 70: 安全态势分层 + Security-Tainted 条目

**目标**：借鉴 QM 的三层安全态势系统，为 EchoMind 引入安全态势级别（Strict / Auto / Dangerous），不同级别控制内容筛查和工具审批行为。同时引入 Security-Tainted 条目标记，被标记的条目在上下文构建时过滤。

**QM 对比**：
- QM `security/security-posture.ts`：`SECURITY_POSTURES = ["dangerous", "auto", "strict"]`，态势组合规则（scope 只能收紧）
- QM `harness/context-compaction.ts`：`forModelContext()` 过滤 `securityTainted === true` 的条目
- EchoMind 现状：`SecurityManager` 管理加密状态机，无态势分层

**改动文件**：
- `crates/core/src/security.rs` — 新增 `SecurityPosture` 枚举 + `ResolvedSecurityPolicy` + `compose_security_posture()`
- `crates/core/src/lib.rs` — `Storage` trait 新增 `set_entry_security_tainted()` / `get_entry_security_tainted()`（默认空操作）
- `crates/infra/src/sqlite_storage.rs` — messages 表新增 `security_tainted` 列 + 2 个方法
- `crates/core/src/security_tests.rs` — TDD 测试
- `crates/tauri-app/src/commands/security.rs` — 新增 `set_security_posture` / `get_security_posture` IPC
- `crates/tauri-app/src/state.rs` — AppState 新增 `security_posture` 字段

**核心实现**：
```rust
/// 安全态势级别（借鉴 QM SecurityPosture）
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecurityPosture {
    /// 危险：无内容筛查，无工具审批
    Dangerous,
    /// 自动（默认）：外部数据分类器筛查
    Auto,
    /// 严格：所有工具调用需人工审批
    Strict,
}

/// 解析后的安全策略
#[derive(Debug, Clone)]
pub struct ResolvedSecurityPolicy {
    /// 是否对外部数据进行安全筛查
    pub inbound_screening: bool,
    /// 是否所有工具调用需审批
    pub tool_approvals: bool,
}

/// 态势组合：子 scope 只能收紧，不能放松父 scope 态势
/// （借鉴 QM composeSecurityPosture 的 POSTURE_RANK 逻辑）
pub fn compose_security_posture(base: SecurityPosture, override_: Option<SecurityPosture>) -> SecurityPosture {
    match override_ {
        Some(o) if o.rank() >= base.rank() => o,
        _ => base,
    }
}
```

**TDD 测试**：
- `tc_posture_001`：解析字符串为 SecurityPosture
- `tc_posture_002`：compose 规则 — 子 scope 只能收紧
- `tc_posture_003`：resolve_security_policy — Dangerous 无筛查无审批
- `tc_posture_004`：resolve_security_policy — Auto 有筛查无审批
- `tc_posture_005`：resolve_security_policy — Strict 有筛查有审批
- `tc_posture_006`：security_tainted 条目在上下文构建时被过滤
- `tc_posture_007`：显式 include_security_tainted 时不过滤

**新增 IPC 命令**（2 个）：
- `set_security_posture(posture: String) -> ()`
- `get_security_posture() -> String`

**新增设置键**：`security.posture`（默认 "auto"）

### Session 71: Shadow 安全筛查模式 ✅

**目标**：借鉴 QM 的 Shadow 安全筛查模式，在引入安全分类器时先 shadow 运行（不影响实际决策），记录 agree/disagreement，验证效果后再切换为阻断模式。适用于未来引入 LLM-based prompt injection 检测。

**QM 对比**：
- QM `core/orchestrator/security-screen.ts`：`runShadowScreen(modelScreen, proxyScreen, callback)` — 并行运行模型筛查和代理筛查，记录 agree/disagree
- QM `security/security-screener.ts`：`createSecurityScreenProxy()` — 代理筛查器，支持 shadow 模式
- EchoMind 现状：无安全筛查器

**改动文件**：
- `crates/core/src/security.rs` — 新增 `SecurityScreener` trait + `ShadowScreenResult` + `run_shadow_screen()`
- `crates/core/src/security_tests.rs` — TDD 测试
- `crates/core/src/lib.rs` — 注册 trait

**核心实现**：
```rust
/// 安全筛查裁决（借鉴 QM SecurityScreenVerdict）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityScreenVerdict {
    /// "allow" 或 "block"
    pub decision: String,
    /// 阻断原因
    pub reason: Option<String>,
    /// 筛查器是否可用
    pub unscreened: bool,
}

/// 安全筛查器 trait（借鉴 QM SecurityScreener）
pub trait SecurityScreener: Send + Sync {
    /// 筛查内容
    fn classify(&self, payload: &str) -> Pin<Box<dyn Future<Output = Option<SecurityScreenVerdict>> + Send>>;
}

/// Shadow 筛查结果（借鉴 QM runShadowScreen 的 agree/disagree 逻辑）
#[derive(Debug, Clone)]
pub struct ShadowScreenResult {
    /// 权威筛查器（模型）的裁决
    pub authoritative: Option<SecurityScreenVerdict>,
    /// 影子筛查器（代理）的裁决
    pub shadow: Option<SecurityScreenVerdict>,
    /// 是否一致
    pub agreement: Agreement,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Agreement {
    Unavailable,
    Agree,
    Disagree,
}

/// 并行运行权威筛查和影子筛查，记录 agree/disagree
/// （借鉴 QM security-screen.ts 的 Promise.race + shadow 评估逻辑）
pub async fn run_shadow_screen(
    model_screen: impl Future<Output = Option<SecurityScreenVerdict>>,
    proxy_screen: impl Future<Output = Option<SecurityScreenVerdict>>,
) -> ShadowScreenResult {
    let (auth, shadow) = tokio::join!(model_screen, proxy_screen);
    let agreement = match (&auth, &shadow) {
        (Some(a), Some(s)) if a.decision == s.decision => Agreement::Agree,
        (Some(_), Some(_)) => Agreement::Disagree,
        _ => Agreement::Unavailable,
    };
    ShadowScreenResult { authoritative: auth, shadow, agreement }
}
```

**TDD 测试**：
- `tc_shadow_001`：两个筛查器都 allow → Agree
- `tc_shadow_002`：一个 allow 一个 block → Disagree
- `tc_shadow_003`：一个返回 None → Unavailable
- `tc_shadow_004`：shadow 模式不影响实际决策（实际走 authoritative）
- `tc_shadow_005`：超时降级为 unscreened
- `tc_shadow_006`：shadow 审计日志记录 agree/disagree

**新增 IPC 命令**（1 个）：
- `get_security_screen_stats() -> ShadowScreenStats` — 获取 shadow 筛查统计

---

## Phase 4: 基础设施加固（中 ROI，2–3 天）

### Session 72: 幂等性存储 + 统一周期任务抽象

**目标**：借鉴 QM 的 `IdempotencyStore` 和 `Sweeper` 模式，为 EchoMind 引入幂等性保证（文件同步/AutoDream 防重复执行）和统一的周期任务管理。

**QM 对比**：
- QM `idempotency/idempotency-store.ts`：`once(key, fn)` — 14 天保留 + inflight tracking + 自动 prune
- QM `util/sweeper.ts`：`createSweeper(fn, intervalMs)` — `unref()` + 错误吞咽 + 标签
- EchoMind 现状：无幂等性保证，后台任务各自实现（auto-lock timer / file watcher / auto_dream）

**改动文件**：
- `crates/core/src/idempotency.rs` — 新建 `IdempotencyStore` + `Sweeper` 抽象
- `crates/core/src/idempotency_tests.rs` — TDD 测试
- `crates/core/src/lib.rs` — 注册 `idempotency` 模块
- `crates/infra/src/sqlite_storage.rs` — 新增 `idempotency_records` 表 + 方法
- `crates/core/src/auto_dream.rs` — 使用 `IdempotencyStore` 防重复触发
- `crates/core/src/sync.rs` — `SyncEngine::sync()` 使用 `IdempotencyStore` 防重复同步

**核心实现**：
```rust
/// 幂等性存储（借鉴 QM IdempotencyStore）
pub struct IdempotencyStore {
    /// 内存缓存
    done: Arc<RwLock<HashMap<String, i64>>>,
    /// 进行中标记
    inflight: Arc<RwLock<HashSet<String>>>,
    /// 持久化后端（可选）
    storage: Option<Arc<SqliteStorage>>,
    /// 保留时间（秒）
    retention_secs: u64,
}

impl IdempotencyStore {
    /// 执行一次（如果 key 未处理过）
    /// 返回 true = 执行了 fn，false = 已处理过或正在处理
    pub async fn once<F>(&self, key: &str, f: F) -> bool
    where
        F: FnOnce() -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>,
    {
        // 1. 检查 inflight
        // 2. 检查 done
        // 3. 执行 fn
        // 4. 标记 done
        // 5. 定期 prune
    }
}

/// 统一周期任务（借鉴 QM createSweeper）
pub struct Sweeper {
    handle: Option<tokio::task::JoinHandle<()>>,
    label: String,
}

impl Sweeper {
    /// 创建周期任务
    pub fn new<F>(task: F, interval: Duration, label: &str) -> Self
    where
        F: Fn() -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send + 'static,
    {
        let label = label.to_string();
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                // 错误吞咽 + 上下文标记
                if let Err(e) = std::panic::AssertUnwindSafe(task()).catch_unwind() {
                    tracing::warn!("[{}] sweep failed: {}", label, err_message(e));
                }
            }
        });
        Self { handle: Some(handle), label }
    }
}
```

**TDD 测试**：
- `tc_idem_001`：首次调用执行 fn，返回 true
- `tc_idem_002`：相同 key 第二次调用跳过，返回 false
- `tc_idem_003`：inflight 时并发调用跳过
- `tc_idem_004`：保留期过期后可再次执行
- `tc_sweeper_001`：周期任务按间隔执行
- `tc_sweeper_002`：任务出错不影响下一次执行
- `tc_sweeper_003`：stop 后任务停止

**新增 IPC 命令**（1 个）：
- `clear_idempotency_records() -> ()` — 清理幂等性记录

### Session 73: 预算追踪 + 速率限制

**目标**：借鉴 QM 的 `BudgetTracker` 和 `RateLimiter`，为 EchoMind 引入 LLM API 费用追踪和速率限制，防止过度消耗。

**QM 对比**：
- QM `ratelimit/budget.ts`：per-principal + org-level USD 花费追踪，滑动窗口
- QM `ratelimit/rate-limiter.ts`：per-principal 滑动窗口限流
- QM `estimateCostUsd(inputTokens, usdPerMTok)` — 成本估算
- EchoMind 现状：无费用追踪，无限流

**改动文件**：
- `crates/core/src/budget.rs` — 新建 `BudgetTracker` + `RateLimiter` + `estimate_cost_usd()`
- `crates/core/src/budget_tests.rs` — TDD 测试
- `crates/core/src/lib.rs` — 注册 `budget` 模块
- `crates/infra/src/sqlite_storage.rs` — 新增 `budget_records` 表
- `crates/tauri-app/src/state.rs` — AppState 新增 `budget_tracker` 字段
- `crates/tauri-app/src/commands/chat.rs` — `chat_inner` 记录 token 成本
- `crates/tauri-app/src/commands/settings.rs` — 新增预算设置 IPC

**核心实现**：
```rust
/// 成本估算（借鉴 QM estimateCostUsd）
pub fn estimate_cost_usd(input_tokens: usize, usd_per_mtok: f64) -> f64 {
    (input_tokens as f64 / 1_000_000.0) * usd_per_mtok
}

/// 预算追踪器（借鉴 QM BudgetTracker）
pub struct BudgetTracker {
    /// 每日预算上限（USD）
    daily_limit: f64,
    /// 已花费金额（按日重置）
    spent: Arc<RwLock<HashMap<String, Vec<(i64, f64)>>>>,
}

impl BudgetTracker {
    /// 检查是否超预算
    pub async fn check(&self, principal: &str) -> BudgetCheck { ... }
    /// 记录花费
    pub async fn record(&self, principal: &str, cost_usd: f64) { ... }
}

/// 速率限制器（借鉴 QM RateLimiter）
pub struct RateLimiter {
    max_per_window: u32,
    window: Duration,
    state: Arc<RwLock<HashMap<String, WindowState>>>,
}
```

**TDD 测试**：
- `tc_budget_001`：未超预算返回 allowed
- `tc_budget_002`：超预算返回 denied
- `tc_budget_003`：滑动窗口过期后重置
- `tc_budget_004`：成本估算正确
- `tc_ratelimit_001`：窗口内允许 max_per_window 次
- `tc_ratelimit_002`：窗口过期后重置计数

**新增 IPC 命令**（2 个）：
- `get_budget_stats() -> BudgetStats` — 获取预算统计
- `set_budget_limit(daily_limit_usd: f64) -> ()` — 设置日预算上限

**新增设置键**：`budget.daily_limit_usd`（默认 0 = 不限制）

### Session 74: KeyedQueue 并发序列化 + Durable-by-Default 审查 ✅

**目标**：借鉴 QM 的 `createKeyedQueue` 模式，为 EchoMind 引入按 key 序列化的并发控制，消除 SQLite 并发写入竞态。同时审查所有内存状态，确保需要跨崩溃恢复的状态已持久化。

**实现完成**（2026-08-08）：
- `crates/core/src/concurrency.rs` — 新建 `KeyedQueue<K>` 泛型结构体 + `run()` / `cleanup()` / `create_keyed_queue()` + `Default` impl + 3 内联测试
- `crates/core/src/concurrency_tests.rs` — 6 TDD 测试 TC-KEYED-001~006
- `crates/core/src/lib.rs` — 注册 `pub mod concurrency`
- `crates/tauri-app/src/state.rs` — AppState 新增 `persist_queue: KeyedQueue<String>` 字段
- `crates/tauri-app/src/commands/chat.rs` — `persist_exchange` 拆分为包装器（KeyedQueue 序列化）+ `persist_exchange_inner`（实际逻辑）
- Durable-by-Default 审查：全量审查 AppState 10+ 内存状态字段，全部已持久化或可安全丢弃

**QM 对比**：
- QM `util/async.ts`：`createKeyedQueue<K>()` — 同 key 串行化，不同 key 并行
- QM `wiring.ts` AGENTS.md 核心纪律："Durable by default — anything read back later must live in a durable store"
- EchoMind 现状：`abort_tokens` 使用 `Arc<RwLock<HashMap>>`，崩溃后丢失；SQLite 并发写入无序列化

**改动文件**：
- `crates/core/src/concurrency.rs` — 新建 `KeyedQueue` + `create_keyed_queue()`
- `crates/core/src/concurrency_tests.rs` — TDD 测试（已有文件，追加测试）
- `crates/core/src/lib.rs` — 注册模块
- `crates/tauri-app/src/commands/chat.rs` — `persist_exchange` 使用 KeyedQueue 按 conversation_id 序列化
- `crates/tauri-app/src/state.rs` — 审查内存状态持久化

**核心实现**：
```rust
/// 按 key 序列化异步操作的队列（借鉴 QM createKeyedQueue）
pub struct KeyedQueue<K: Eq + Hash + Clone + Send + Sync + 'static> {
    tails: Arc<RwLock<HashMap<K, Arc<Mutex<()>>>>>,
}

impl<K: Eq + Hash + Clone + Send + Sync + 'static> KeyedQueue<K> {
    pub fn new() -> Self {
        Self { tails: Arc::new(RwLock::new(HashMap::new())) }
    }

    /// 按 key 序列化执行
    pub async fn run<F, T>(&self, key: K, f: F) -> T
    where
        F: FnOnce() -> Pin<Box<dyn Future<Output = T> + Send>>,
        T: Send,
    {
        let mutex = {
            let mut tails = self.tails.write().await;
            tails.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
        };
        let _guard = mutex.lock().await;
        f().await
    }
}
```

**Durable-by-Default 审查清单**：
- [ ] `abort_tokens` — 崩溃后重建（会话级，可丢弃 ✅）
- [ ] `hybrid_search` — 已持久化到 settings 表 ✅
- [ ] `compression_ratio` — 已持久化 ✅
- [ ] `is_pro` — 已持久化 ✅
- [ ] 新增 `security_posture` — 需持久化到 settings 表
- [ ] 新增 `budget_tracker` — 需持久化到 budget_records 表

**TDD 测试**：
- `tc_keyed_001`：同 key 操作串行执行
- `tc_keyed_002`：不同 key 操作并行执行
- `tc_keyed_003`：操作完成后 tail 清理
- `tc_keyed_004`：panic 不影响后续同 key 操作

---

## Phase 5: LLMProvider 增强（中 ROI，1–2 天）

### Session 75: LLMProvider Trait 扩展 — 辅助模型方法 ✅ (Q10 已完成)

**目标**：借鉴 QM 的 `HarnessModelUtilities` 接口，为 EchoMind 的 `LLMProvider` trait 增加可选的辅助方法（`should_respond` / `generate_title` / `judge`），使不同 LLM 后端能各自优化这些功能。

**QM 对比**：
- QM `harness/harness.ts`：`HarnessModelUtilities` 包含 `shouldRespond` / `compactHistory` / `contextTokenBudget` / `oneShot` / `judge` / `screenSecurity` / `pickAckEmoji` / `generateTitle` / `summarizeApproval`
- EchoMind 现状：`LLMProvider` trait 只有 `chat_stream` 和 `chat_stream_segmented`，标题提取用 `derive_title()` 纯字符串截取

**改动文件**：
- `crates/core/src/lib.rs` — `LLMProvider` trait 新增可选方法（默认实现返回 None）
- `crates/core/src/lib_tests.rs` — trait 兼容性测试
- `crates/infra/src/openai_provider.rs` — 实现 `generate_title` / `judge`
- `crates/infra/src/local_llm.rs` — Pro 实现 `generate_title`
- `crates/tauri-app/src/commands/chat.rs` — `derive_title` 优先调用 `generate_title`

**核心实现**：
```rust
pub trait LLMProvider: Send + Sync {
    // 现有方法...
    async fn chat_stream(...) -> anyhow::Result<BoxStream<'static, anyhow::Result<String>>>;

    // 新增可选方法（借鉴 QM HarnessModelUtilities）

    /// 生成对话标题（借鉴 QM generateTitle）
    /// 默认返回 None，由调用方降级为 derive_title() 字符串截取
    async fn generate_title(&self, _transcript: &str) -> anyhow::Result<Option<String>> {
        Ok(None) // 默认不实现
    }

    /// 单次推理（借鉴 QM oneShot）
    /// 用于记忆提取、安全筛查等非流式调用
    async fn one_shot(&self, _system: &str, _prompt: &str) -> anyhow::Result<Option<String>> {
        Ok(None) // 默认不实现
    }

    /// 判断器（借鉴 QM judge）
    /// 用于安全筛查、质量评估等需要判断的场景
    async fn judge(&self, _system: &str, _prompt: &str) -> anyhow::Result<Option<String>> {
        Ok(None) // 默认不实现
    }

    /// 上下文 token 预算（借鉴 QM contextTokenBudget）
    /// 返回此模型的最大上下文长度
    fn context_token_budget(&self) -> Option<usize> {
        None // 默认不实现
    }
}
```

**TDD 测试**：
- `tc_llm_aux_001`：默认实现返回 None（向后兼容）
- `tc_llm_aux_002`：OpenAIProvider 实现 generate_title 返回标题
- `tc_llm_aux_003`：generate_title 失败降级为 derive_title
- `tc_llm_aux_004`：one_shot 返回完整响应
- `tc_llm_aux_005`：context_token_budget 返回模型上限

**完成详情（Q10, 2026-08-08）**：
- `crates/core/src/lib.rs`：LLMProvider trait 新增 4 个默认方法（`generate_title` / `one_shot` / `judge` / `context_token_budget`），全部返回 None/Ok(None)，向后兼容
- `crates/core/src/llm_aux_tests.rs`：7 TDD 测试 TC-LLM-AUX-001~007 + MinimalStub + RichMock 两个 mock provider
- `crates/infra/src/openai_provider.rs`：OpenAIProvider 覆盖 3 个方法（generate_title 复用 complete() + 30 字截断 / one_shot 委托 complete() / judge 委托 complete() + trim）
- `crates/tauri-app/src/commands/mod.rs`：LlmProvider enum 转发 4 个新方法
- `crates/tauri-app/src/commands/chat.rs`：persist_exchange_inner 集成 LLM 辅助标题生成（10s 超时 + answer 截断 200 字 + 静默降级 derive_title）
- 验证：core 596 passed / clippy 零警告 / fmt 通过 / 零新增依赖

### Session 76: Harness Router — 动态后端切换

**目标**：借鉴 QM 的 `HarnessRouter` 模式，为 EchoMind 引入 LLM 后端路由器，支持按对话或用户偏好动态切换 LLM 后端（OpenAI / Local LLM / 未来新增后端），切换时自动重置会话状态。

**QM 对比**：
- QM `harness/harness-router.ts`：`createHarnessRouter(adapters, utility, resolve)` — 按 scope 级别和用户请求动态选择 Harness
- QM 切换逻辑：`lastHarness` 记录上次使用的 harness，切换时调用 `resetSession`
- EchoMind 现状：`chat_inner` 通过 `llm_mode` 静态选择 OpenAIProvider 或 LocalLlmEngine

**改动文件**：
- `crates/core/src/lib.rs` — 新增 `LlmRouter` 结构体 + `LlmChoice` 类型
- `crates/tauri-app/src/state.rs` — AppState 新增 `llm_router` 字段
- `crates/tauri-app/src/commands/chat.rs` — `chat_inner` 使用 router 而非直接 if/else
- `crates/tauri-app/src/commands/settings.rs` — `set_llm_mode` 更新 router

**核心实现**：
```rust
/// LLM 选择结果（借鉴 QM RuntimeChoice）
#[derive(Debug, Clone)]
pub struct LlmChoice {
    pub mode: LlmMode,
    pub model_id: String,
}

/// LLM 路由器（借鉴 QM HarnessRouter）
/// 管理多个 LLMProvider 适配器，按运行时配置动态路由
pub struct LlmRouter {
    /// OpenAI 适配器
    openai: Option<Arc<dyn LLMProvider>>,
    /// 本地 LLM 适配器（Pro）
    #[cfg(feature = "pro")]
    local: Option<Arc<dyn LLMProvider>>,
    /// 默认选择
    fallback: LlmChoice,
    /// 每会话上次使用的模式（借鉴 QM lastHarness）
    last_mode: Arc<RwLock<HashMap<String, LlmMode>>>,
}

impl LlmRouter {
    /// 路由到正确的 LLMProvider（借鉴 QM harness-router 的 resolve + adapter 选择）
    pub async fn resolve(&self, conversation_id: &str, requested: Option<LlmChoice>) -> (Arc<dyn LLMProvider>, LlmChoice) {
        let choice = requested.unwrap_or_else(|| self.fallback.clone());
        // 记录上次模式
        // 如果模式变更，重置会话状态
        let provider = match choice.mode {
            LlmMode::OpenAI => self.openai.clone(),
            #[cfg(feature = "pro")]
            LlmMode::Local => self.local.clone(),
        };
        (provider.unwrap(), choice)
    }
}
```

**TDD 测试**：
- `tc_router_001`：无请求时使用 fallback
- `tc_router_002`：请求 OpenAI 模式返回 OpenAI 适配器
- `tc_router_003`：请求 Local 模式返回 Local 适配器（Pro）
- `tc_router_004`：模式切换时记录 last_mode
- `tc_router_005`：Free 模式请求 Local 返回错误

**完成详情（Q11, 2026-08-08）**：
- `crates/core/src/llm_router.rs` 新建模块：`LlmChoice`（mode + model_id）+ `RouterVerdict`（Initial/SameMode/ModeChanged）+ `RouterError`（ModeUnavailable）+ `LlmRouter`（fallback/last_mode/available_modes，`resolve()` + `set_fallback()` + `set_available_modes()` + `last_mode_for()` + `clear()` + `fallback()` + `new_free_default()` + `new_pro_default()` + `Default`）
- `crates/core/src/llm_router_tests.rs`：12 TDD 测试 TC-ROUTER-001~012
- `crates/models/src/lib.rs`：`LlmMode` derive 新增 `Hash`
- `crates/tauri-app/src/state.rs`：AppState 新增 `llm_router: LlmRouter` 字段 + 初始化（根据 is_pro 选择 Free/Pro 可用模式 + 根据 llm_mode 设置 fallback）
- `crates/tauri-app/src/commands/chat.rs`：`chat_inner` 使用 `llm_router.resolve()` 替代直接 `get_llm_mode()` if/else
- `crates/tauri-app/src/commands/local_llm.rs`：`set_llm_mode_inner` 同步更新 router fallback
- `crates/tauri-app/src/commands/license.rs`：`activate_pro_inner` / `deactivate_pro_inner` 动态更新 router 可用模式集合
- 验证：core 608 passed / clippy Free+Pro 零警告 / fmt 通过 / 零新增依赖
- 设计调整：由于 `async fn in trait` 不支持 `dyn`，Router 不持有 provider 实例，仅负责路由决策（返回 `LlmChoice`），实际 provider 创建由 `chat_inner` 基于 `LlmChoice.mode` 完成（与现有 `LlmProvider` enum 分发模式一致）

---

## 不适合借鉴的部分

| QM 模块 | 不借鉴原因 |
|---|---|
| Postgres 持久化 | EchoMind 使用 SQLite + SQLCipher，单机桌面应用不需要分布式存储 |
| Leader Lease / Advisory Lock | 单实例桌面应用不需要分布式锁 |
| Slack 集成 / Web UI 插件 | EchoMind 是独立桌面应用，不集成即时通讯平台 |
| AWS microvm 沙箱 / Sandbox Router | 过于重量级，EchoMind 的 `WasmExecutor` 进程级沙箱更合适 |
| Skills 系统（签名/物化/pack） | EchoMind 的快捷指令模板已有类似功能，Skills 的签名验证过于复杂 |
| 零注释策略 | 与 EchoMind 铁律三冲突，EchoMind 要求公共 API 文档注释 |
| Private Fork 工作流 | EchoMind 是单仓库开发 |
| Session Tape 格式 | EchoMind 的消息存储够用，Tape 格式引入复杂度过高 |
| Scope 系统 | EchoMind 是单用户应用，不需要多人 scope 隔离 |

---

## 实施顺序与依赖关系

```
Phase 1（记忆系统）:
  S66（Scratch-Promote + Consolidation）→ S67（Burst Buffer + Provenance）
  （S67 依赖 S66 的 scratch 层基础）

Phase 2（上下文压缩）:
  S68（双阈值 + 后台异步）→ S69（Token 缓存 + Dangling Call）
  （S69 依赖 S68的双阈值架构）

Phase 3（安全层）:
  S70（态势分层 + Tainted 条目）→ S71（Shadow 筛查）
  （S71 依赖 S70 的安全态势系统）

Phase 4（基础设施）:
  S72（幂等性 + Sweeper）  ── 独立
  S73（预算追踪 + 限流）   ── 独立
  S74（KeyedQueue + Durable 审查）── 独立

Phase 5（LLMProvider）:
  S75（Trait 扩展）→ S76（Harness Router）
  （S76 依赖 S75 的 trait 扩展）
```

**跨 Phase 依赖**（✅ 全部完成 2026-08-08）：
- ✅ S67（Burst Buffer）可选依赖 S75（one_shot 方法）— 用 one_shot 替代非流式 LLM 调用。`llm_complete()` 辅助函数优先尝试 `one_shot`，降级为 `chat_stream + collect_stream`。5 个 TDD 测试覆盖优先路径、降级路径、向后兼容。
- ✅ S68（后台压缩）可选依赖 S72（Sweeper）— 用统一 Sweeper 管理后台压缩任务。`schedule_background_compaction()` 合并 `try_acquire + run + release` 三步，使用 `tracing` 替代 `eprintln!`。4 个 TDD 测试覆盖成功/去重/失败/自动释放。
- ✅ S71（Shadow 筛查）可选依赖 S75（judge 方法）— 用 judge 做安全判断。`llm_classify()` 函数使用 `judge()` 方法返回 `SecurityScreenVerdict`，可直接传递给 `run_shadow_screen()`。7 个 TDD 测试覆盖 allow/block/不支持/失败/agree/disagree/unavailable。

---

## 优先级建议

1. **最高 ROI**：S66（Scratch-Promote）+ S68（双阈值压缩）— 直接提升记忆质量和长对话体验
2. **高价值**：S67（Burst Buffer）+ S70（安全态势）— 降低成本 + 分级安全控制
3. **中等价值**：S69（Token 缓存）+ S71（Shadow 筛查）+ S72（幂等性）+ S73（预算追踪）
4. **长期演进**：S74（KeyedQueue）+ S75（Trait 扩展）+ S76（Harness Router）

---

## 各 Session 衔接提示词

### S66 → S67 衔接提示词

```
你正在继续 EchoMind 项目的开发。S66 Scratch-Promote 两层记忆 + Consolidation 引擎已完成。
当前状态：
- `crates/core/src/memory_store.rs` 新增 Scratch 层 + `consolidate()` 方法
- `crates/infra/src/sqlite_storage.rs` 新增 `scratch_logs` 表
- `crates/core/src/auto_dream.rs` 集成 consolidation 阶段
- 2 个新 IPC 命令（trigger_memory_consolidation / get_scratch_logs）
- 设置键 `memory.auto_consolidate`（默认 false）

下个 Session S67 目标：Burst Buffer 延迟捕获 + Provenance 标记。
借鉴 QM `memory/strategies/per-turn.ts` 的 `createBurstBuffer(quietMs, maxTurns, flush)`。
将记忆提取从每轮立即调用改为聚合多轮后批量提取，降低 LLM 调用成本。
```

### S67 → S68 衔接提示词

```
你正在继续 EchoMind 项目的开发。S67 Burst Buffer + Provenance 已完成。
当前状态：
- `crates/core/src/memory_store.rs` 新增 `BurstBuffer` + `ProvenanceTag`
- 前端 memory-panel.js 可显示来源
- Burst Buffer 默认启用，quiet_ms = 180s，max_turns = 10

下个 Session S68 目标：双阈值压缩 + 后台异步压缩。
借鉴 QM `core/orchestrator/compaction.ts` 的 Soft 70% 后台 + Hard 90% 同步。
将 `CompactionEngine` 从单阈值同步升级为双阈值，消除长对话卡顿。
```

### S68 → S69 衔接提示词

```
你正在继续 EchoMind 项目的开发。S68 双阈值压缩已完成。
当前状态：
- `crates/compact/src/lib.rs` 新增 `check_compaction_needed()` + `schedule_background_compaction()`
- `crates/tauri-app/src/commands/chat.rs` 集成双阈值逻辑
- 设置键 `compaction.soft_threshold` / `compaction.hard_threshold`

下个 Session S69 目标：Token 级预算 + Entry Token 缓存 + Dangling Call 修复。
借鉴 QM `harness/context-compaction.ts` 的 `estimateEntryTokens` LRU 缓存 + `healDanglingCalls`。
```

### S69 → S70 衔接提示词

```
你正在继续 EchoMind 项目的开发。S69 Token 缓存 + Dangling Call 已完成。
当前状态：
- `crates/compact/src/lib.rs` 新增 `TokenEstimator`（LRU 缓存）+ `heal_dangling_calls()`

下个 Session S70 目标：安全态势分层 + Security-Tainted 条目。
借鉴 QM `security/security-posture.ts` 的三层态势（Dangerous/Auto/Strict）。
为 EchoMind 引入安全态势级别，不同级别控制内容筛查和工具审批行为。
```

### S70 → S71 衔接提示词

```
你正在继续 EchoMind 项目的开发。S70 安全态势分层已完成。
当前状态：
- `crates/core/src/security.rs` 新增 `SecurityPosture` + `ResolvedSecurityPolicy`
- messages 表新增 `security_tainted` 列
- 2 个新 IPC 命令（set_security_posture / get_security_posture）
- 设置键 `security.posture`（默认 "auto"）

下个 Session S71 目标：Shadow 安全筛查模式。
借鉴 QM `core/orchestrator/security-screen.ts` 的 `runShadowScreen`。
并行运行权威筛查和影子筛查，记录 agree/disagree，验证效果后再切换为阻断模式。
```

### S71 → S72 衔接提示词

```
你正在继续 EchoMind 项目的开发。S71 Shadow 安全筛查已完成。
当前状态：
- `crates/core/src/security.rs` 新增 `SecurityScreener` trait + `run_shadow_screen()`

下个 Session S72 目标：幂等性存储 + 统一周期任务抽象。
借鉴 QM `idempotency/idempotency-store.ts` + `util/sweeper.ts`。
为 AutoDream 和文件同步引入幂等性保证，统一后台任务管理。
```

### S72 → S73 衔接提示词

```
你正在继续 EchoMind 项目的开发。S72 幂等性 + Sweeper 已完成。
当前状态：
- `crates/core/src/idempotency.rs` 新增 `IdempotencyStore` + `Sweeper`
- `crates/core/src/auto_dream.rs` 使用幂等性防重复触发
- `crates/core/src/sync.rs` 使用幂等性防重复同步

下个 Session S73 目标：预算追踪 + 速率限制。
借鉴 QM `ratelimit/budget.ts` + `ratelimit/rate-limiter.ts`。
为 EchoMind 引入 LLM API 费用追踪和速率限制。
```

### S73 → S74 衔接提示词

```
你正在继续 EchoMind 项目的开发。S73 预算追踪 + 限流已完成。
当前状态：
- `crates/core/src/budget.rs` 新增 `BudgetTracker` + `RateLimiter`
- 2 个新 IPC 命令（get_budget_stats / set_budget_limit）

下个 Session S74 目标：KeyedQueue 并发序列化 + Durable-by-Default 审查。
借鉴 QM `util/async.ts` 的 `createKeyedQueue`。
为 EchoMind 引入按 key 序列化的并发控制 + 审查所有内存状态持久化。
```

### S74 → S75 衔接提示词

```
你正在继续 EchoMind 项目的开发。S74 KeyedQueue + Durable 审查已完成。
当前状态：
- `crates/core/src/concurrency.rs` 新增 `KeyedQueue`
- `persist_exchange` 按 conversation_id 序列化

下个 Session S75 目标：LLMProvider Trait 扩展。
借鉴 QM `harness/harness.ts` 的 `HarnessModelUtilities`。
为 `LLMProvider` trait 增加可选的辅助方法（generate_title / one_shot / judge / context_token_budget）。
```

### S75 → S76 衔接提示词

```
你正在继续 EchoMind 项目的开发。S75 LLMProvider Trait 扩展已完成。
当前状态：
- `LLMProvider` trait 新增 4 个可选方法（默认实现返回 None）
- OpenAIProvider 实现 generate_title / one_shot / judge
- LocalLlmEngine 实现 generate_title（Pro）

下个 Session S76 目标：Harness Router — 动态后端切换。
借鉴 QM `harness/harness-router.ts` 的 `createHarnessRouter`。
为 EchoMind 引入 LLM 后端路由器，支持按对话动态切换 LLM 后端。
```

---

## 铁律遵守

1. **TDD 驱动**（铁律二）：每个 Session 先写测试（红灯），再实现（绿灯），最后重构。禁空断言。
2. **代码注释**（铁律三）：所有新增 `pub` 项必须有 `///` 文档注释。前端 JS 函数必须有 JSDoc。
3. **真实数据测试**（铁律四）：Mock 测试 + 至少一个真实数据集成测试。混沌测试覆盖边缘场景。
4. **生产级代码质量**（铁律五）：零 `unwrap` / `expect` / `panic` / `unreachable` / `todo` / `unimplemented`（workspace deny lint）。错误信息不含敏感信息。
5. **最新版本依赖**（铁律六）：新增 crate 使用最新稳定版本。`cargo update --dry-run` 检查。
6. **文档实时同步**（铁律七）：每个 Session 完成后立即同步 SRS / IPC / Contracts / Settings / Database 文档。
7. **Rust 生态优先**（铁律九）：功能实现优先纯 Rust crate。
8. **GitHub 发布同步**（铁律十一）：每次源代码修改后运行 `./sync-to-github.sh`。
9. **Free/Pro 双版本**：Pro feature 使用 `#[cfg(feature = "pro")]` 门控。Free 版编译通过 + 测试通过。
10. **网络代理**（铁律一）：网络访问失败时使用 `export https_proxy=http://127.0.0.1:7890`。

---

## 统计汇总

| 指标 | 数值 |
|---|---|
| 总 Session 数 | 11（Q01–Q11） |
| Phase 数 | 5 |
| 预计新增 IPC 命令 | 8（162 → 170） |
| 预计新增设置键 | 5 |
| 预计新增 SQLite 表 | 3（scratch_logs / idempotency_records / budget_records） |
| 预计新增 trait | 2（SecurityScreener + LlmRouter） |

> **评估修复记录**（2026-08-08）：
> 1. Session 编号 S66-S76 → Q01-Q11（避免与 v1.4 发布 S66 冲突）
> 2. S76 `Arc<dyn LLMProvider>` → enum 分派（LLMProvider 非 dyn-compatible）
> 3. S69 `heal_dangling_calls` 移除（ChatMessage 无 tool_call 字段）
> 4. S72 `catch_unwind` → `tokio::spawn` + 错误日志（async 不兼容）
> 5. S66 `consolidate()` → `consolidate_scratch()`（避免与现有方法冲突）
> 6. S69 TokenEstimator 扩展现有 `token_budget.rs`（避免重复）
> 7. 基线 159→162 IPC / 808→822+ Free tests
| 预计新增 SRS REQ | 5–8 条 |
| 预计总工作量 | 10–15 天 |
