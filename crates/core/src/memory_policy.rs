//! Memory Policy + Compactor + DemotionHook 三层抽象（B-07 借鉴 Rig Memory 体系）。
//!
//! ## 核心设计
//!
//! 将对话历史管理拆分为三个标准化抽象层：
//!
//! 1. **`MemoryPolicy`** — 截断/保留策略（`apply(messages) -> (kept, demoted)`）
//!    - `SlidingWindowPolicy`: 保留最近 N 条
//!    - `TokenBudgetPolicy`: 按 token 预算截断
//!
//! 2. **`Compactor`** — 被截断的消息 → 摘要 artifact（重新插入 history）
//!    - `Compactor` trait: `compact(demoted_messages) -> summary`
//!    - 与现有 `CompactionEngine` 整合
//!
//! 3. **`DemotionHook`** — 被截断的消息流入长期存储（不丢弃）
//!    - `DemotionHook` trait: `on_demote(demoted_messages)`
//!    - 与 `MemoryStore` 整合：被截断消息提取关键事实后存入记忆系统
//!
//! ## 与现有系统的对比
//!
//! | 维度 | 现有（SessionStrip） | 三层抽象 |
//! |---|---|---|
//! | 截断策略 | 硬编码按索引/保留数 | `MemoryPolicy` trait（可替换） |
//! | 被截断消息 | 直接丢弃 | `Compactor` 生成摘要 + `DemotionHook` 流入记忆 |
//! | 摘要 | 可选插入 system 消息 | 标准化 `Compactor` trait |

use echomind_models::ChatMessage;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

// ============================================================
// MemoryPolicy trait
// ============================================================

/// 对话历史截断/保留策略端口。
///
/// 实现者决定哪些消息保留、哪些被降级（demoted）。
/// 被降级的消息可被 `Compactor` 生成摘要、被 `DemotionHook` 流入长期存储。
///
/// # 对象安全
///
/// 使用手动 `Pin<Box<Future>>` 返回类型，保证对象安全。
pub trait MemoryPolicy: Send + Sync {
    /// 应用截断策略。
    ///
    /// # 参数
    /// - `messages`: 全部对话历史
    ///
    /// # 返回
    /// `(kept_messages, demoted_messages)` — 保留的 + 被降级的
    fn apply<'a>(
        &'a self,
        messages: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = MemoryPolicyResult> + Send + 'a>>;
}

/// MemoryPolicy 应用结果。
#[derive(Debug, Clone)]
pub struct MemoryPolicyResult {
    /// 保留的消息（在上下文窗口内）
    pub kept: Vec<ChatMessage>,
    /// 被降级的消息（超出上下文窗口，需要 compact 或 demote）
    pub demoted: Vec<ChatMessage>,
}

impl MemoryPolicyResult {
    /// 创建全部保留的结果（无截断）。
    pub fn all_kept(messages: Vec<ChatMessage>) -> Self {
        Self {
            kept: messages,
            demoted: Vec::new(),
        }
    }

    /// 是否有被降级的消息。
    pub fn has_demoted(&self) -> bool {
        !self.demoted.is_empty()
    }
}

// ============================================================
// SlidingWindowPolicy — 滑动窗口策略
// ============================================================

/// 滑动窗口截断策略：保留最近 N 条消息。
///
/// 最简单的截断策略：保留最后 `window_size` 条消息，
/// 之前的消息全部降级。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingWindowPolicy {
    /// 窗口大小（保留最近 N 条）
    pub window_size: usize,
}

impl SlidingWindowPolicy {
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }
}

impl MemoryPolicy for SlidingWindowPolicy {
    fn apply<'a>(
        &'a self,
        messages: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = MemoryPolicyResult> + Send + 'a>> {
        let window_size = self.window_size;
        Box::pin(async move {
            if messages.len() <= window_size {
                return MemoryPolicyResult::all_kept(messages.to_vec());
            }
            let split_point = messages.len() - window_size;
            let demoted = messages[..split_point].to_vec();
            let kept = messages[split_point..].to_vec();
            MemoryPolicyResult { kept, demoted }
        })
    }
}

// ============================================================
// TokenBudgetPolicy — Token 预算策略
// ============================================================

/// Token 预算截断策略：按 token 估算截断。
///
/// 保留最近的消息直到 token 估算不超过 `budget`，
/// 之前的消息全部降级。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBudgetPolicy {
    /// Token 预算上限
    pub budget: usize,
}

impl TokenBudgetPolicy {
    pub fn new(budget: usize) -> Self {
        Self { budget }
    }

    /// 估算消息的 token 数（简化版：字符数 / 4）。
    fn estimate_tokens(msg: &ChatMessage) -> usize {
        msg.content.len() / 4
    }
}

impl MemoryPolicy for TokenBudgetPolicy {
    fn apply<'a>(
        &'a self,
        messages: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = MemoryPolicyResult> + Send + 'a>> {
        let budget = self.budget;
        Box::pin(async move {
            let mut token_count = 0;
            // split_point = 0 表示不截断（全部保留）
            let mut split_point = 0;

            // 从末尾向前累计 token
            for (i, msg) in messages.iter().enumerate().rev() {
                let msg_tokens = Self::estimate_tokens(msg);
                if token_count + msg_tokens > budget {
                    // 当前消息加不入预算，split 在它之后
                    split_point = i + 1;
                    break;
                }
                token_count += msg_tokens;
            }

            if split_point == 0 || split_point >= messages.len() {
                return MemoryPolicyResult::all_kept(messages.to_vec());
            }

            let demoted = messages[..split_point].to_vec();
            let kept = messages[split_point..].to_vec();
            MemoryPolicyResult { kept, demoted }
        })
    }
}

// ============================================================
// Compactor trait
// ============================================================

/// 摘要压缩器端口：被截断的消息 → 摘要 artifact。
///
/// 实现者将 `demoted` 消息列表压缩为一条摘要消息，
/// 重新插入到 `kept` 消息列表的开头（rolling summary 模式）。
///
/// # 与现有 `CompactionEngine` 的关系
///
/// `CompactionEngine` 可实现此 trait 成为标准化的 `Compactor`。
pub trait Compactor: Send + Sync {
    /// 压缩被降级的消息为摘要。
    ///
    /// # 参数
    /// - `demoted`: 被降级的消息列表
    ///
    /// # 返回
    /// 摘要文本（如为空则不插入摘要消息）
    fn compact<'a>(
        &'a self,
        demoted: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<Option<String>>> + Send + 'a>>;
}

/// 零依赖文本摘要压缩器（不调用 LLM，仅截取前 N 字符）。
///
/// 用于测试或 LLM 不可用时的降级方案。
pub struct TextTruncationCompactor {
    /// 摘要最大字符数
    pub max_chars: usize,
}

impl TextTruncationCompactor {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }
}

impl Default for TextTruncationCompactor {
    fn default() -> Self {
        Self::new(500)
    }
}

impl Compactor for TextTruncationCompactor {
    fn compact<'a>(
        &'a self,
        demoted: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<Option<String>>> + Send + 'a>>
    {
        let max_chars = self.max_chars;
        Box::pin(async move {
            if demoted.is_empty() {
                return Ok(None);
            }
            let mut summary = String::new();
            for msg in demoted {
                let truncated = if msg.content.len() > max_chars / demoted.len() {
                    &msg.content[..max_chars / demoted.len()]
                } else {
                    &msg.content
                };
                summary.push_str(&format!("[{}] {}\n", msg.role, truncated));
            }
            Ok(Some(format!("以下是之前对话的摘要：\n{summary}")))
        })
    }
}

// ============================================================
// DemotionHook trait
// ============================================================

/// 降级 Hook 端口：被截断的消息流入长期存储。
///
/// 实现者将 `demoted` 消息存入记忆系统（如 `MemoryStore`），
/// 确保被截断的对话不丢失关键信息。
///
/// # 与现有 `MemoryStore` 的关系
///
/// `MemoryStore` 可实现此 trait，在 `on_demote` 中提取关键事实。
pub trait DemotionHook: Send + Sync {
    /// 被降级的消息流入长期存储。
    ///
    /// # 参数
    /// - `demoted`: 被降级的消息列表
    fn on_demote<'a>(
        &'a self,
        demoted: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>>;
}

/// 无操作 DemotionHook（用于测试或禁用 demotion）。
pub struct NoOpDemotionHook;

impl DemotionHook for NoOpDemotionHook {
    fn on_demote<'a>(
        &'a self,
        _demoted: &'a [ChatMessage],
    ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }
}

// ============================================================
// MemoryPipeline — 三层管线编排
// ============================================================

/// 三层记忆管线：Policy → Compactor → DemotionHook。
///
/// 编排三个抽象层：
/// 1. `MemoryPolicy` 决定截断
/// 2. `Compactor` 生成摘要（可选）
/// 3. `DemotionHook` 流入长期存储
pub struct MemoryPipeline {
    policy: Box<dyn MemoryPolicy>,
    compactor: Option<Box<dyn Compactor>>,
    demotion_hook: Option<Box<dyn DemotionHook>>,
}

impl MemoryPipeline {
    /// 创建记忆管线。
    pub fn new(policy: Box<dyn MemoryPolicy>) -> Self {
        Self {
            policy,
            compactor: None,
            demotion_hook: None,
        }
    }

    /// 设置 Compactor。
    pub fn with_compactor(mut self, compactor: Box<dyn Compactor>) -> Self {
        self.compactor = Some(compactor);
        self
    }

    /// 设置 DemotionHook。
    pub fn with_demotion_hook(mut self, hook: Box<dyn DemotionHook>) -> Self {
        self.demotion_hook = Some(hook);
        self
    }

    /// 执行三层管线。
    ///
    /// 1. Policy 截断 → (kept, demoted)
    /// 2. Compactor 生成摘要 → 插入 kept 开头
    /// 3. DemotionHook 流入长期存储
    ///
    /// # 返回
    /// 处理后的消息列表（kept + 可选摘要）
    pub async fn process(&self, messages: Vec<ChatMessage>) -> anyhow::Result<Vec<ChatMessage>> {
        let result = self.policy.apply(&messages).await;

        // 拆分结果
        let kept = result.kept;
        let demoted = result.demoted;
        let has_demoted = !demoted.is_empty();

        let mut kept = kept;
        if has_demoted {
            if let Some(ref compactor) = self.compactor {
                if let Some(summary) = compactor.compact(&demoted).await? {
                    kept.insert(
                        0,
                        ChatMessage {
                            id: None,
                            role: "system".into(),
                            content: summary,
                            sources: None,
                            reasoning: None,
                            turn_group: None,
                            version: None,
                        },
                    );
                }
            }

            // DemotionHook 流入长期存储
            if let Some(ref hook) = self.demotion_hook {
                hook.on_demote(&demoted).await?;
            }
        }

        Ok(kept)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use echomind_models::ChatMessage;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn make_msg(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            id: None,
            role: role.into(),
            content: content.into(),
            sources: None,
            reasoning: None,
            turn_group: None,
            version: None,
        }
    }

    fn make_msgs(n: usize) -> Vec<ChatMessage> {
        (0..n)
            .map(|i| make_msg("user", &format!("message {i}")))
            .collect()
    }

    // ─── SlidingWindowPolicy ───

    #[tokio::test]
    async fn tc_mp_001_sliding_window_no_truncation() {
        let policy = SlidingWindowPolicy::new(10);
        let messages = make_msgs(5);
        let result = policy.apply(&messages).await;
        assert!(!result.has_demoted());
        assert_eq!(result.kept.len(), 5);
    }

    #[tokio::test]
    async fn tc_mp_002_sliding_window_truncation() {
        let policy = SlidingWindowPolicy::new(3);
        let messages = make_msgs(5);
        let result = policy.apply(&messages).await;
        assert!(result.has_demoted());
        assert_eq!(result.kept.len(), 3);
        assert_eq!(result.demoted.len(), 2);
        // 保留的是最后 3 条
        assert_eq!(result.kept[0].content, "message 2");
        assert_eq!(result.kept[2].content, "message 4");
    }

    #[tokio::test]
    async fn tc_mp_003_sliding_window_exact_boundary() {
        let policy = SlidingWindowPolicy::new(3);
        let messages = make_msgs(3);
        let result = policy.apply(&messages).await;
        assert!(!result.has_demoted());
        assert_eq!(result.kept.len(), 3);
    }

    // ─── TokenBudgetPolicy ───

    #[tokio::test]
    async fn tc_mp_004_token_budget_no_truncation() {
        let policy = TokenBudgetPolicy::new(1000);
        let messages = make_msgs(5);
        let result = policy.apply(&messages).await;
        assert!(!result.has_demoted());
    }

    #[tokio::test]
    async fn tc_mp_005_token_budget_truncation() {
        // 每条消息 "message X" ≈ 10 字符 / 4 = 2 tokens
        // budget=10 → 保留约 5 条
        let policy = TokenBudgetPolicy::new(10);
        let messages = make_msgs(10);
        let result = policy.apply(&messages).await;
        assert!(result.has_demoted());
        assert!(result.kept.len() < 10);
    }

    // ─── TextTruncationCompactor ───

    #[tokio::test]
    async fn tc_mp_006_compactor_empty_input() {
        let compactor = TextTruncationCompactor::new(500);
        let result = compactor.compact(&[]).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn tc_mp_007_compactor_generates_summary() {
        let compactor = TextTruncationCompactor::new(500);
        let demoted = vec![
            make_msg("user", "hello world"),
            make_msg("assistant", "hi there"),
        ];
        let result = compactor.compact(&demoted).await.unwrap();
        assert!(result.is_some());
        let summary = result.unwrap();
        assert!(summary.contains("摘要"));
        assert!(summary.contains("hello world"));
        assert!(summary.contains("hi there"));
    }

    // ─── NoOpDemotionHook ───

    #[tokio::test]
    async fn tc_mp_008_noop_demotion_hook() {
        let hook = NoOpDemotionHook;
        let demoted = make_msgs(3);
        hook.on_demote(&demoted).await.unwrap();
    }

    // ─── 计数 DemotionHook（用于验证调用） ───

    struct CountingDemotionHook {
        call_count: Arc<AtomicUsize>,
    }

    impl CountingDemotionHook {
        fn new() -> (Self, Arc<AtomicUsize>) {
            let count = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    call_count: count.clone(),
                },
                count,
            )
        }
    }

    impl DemotionHook for CountingDemotionHook {
        fn on_demote<'a>(
            &'a self,
            demoted: &'a [ChatMessage],
        ) -> Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>> {
            let count = self.call_count.clone();
            let n = demoted.len();
            Box::pin(async move {
                count.fetch_add(n, Ordering::SeqCst);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn tc_mp_009_demotion_hook_called() {
        let (hook, count) = CountingDemotionHook::new();
        let demoted = make_msgs(3);
        hook.on_demote(&demoted).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    // ─── MemoryPipeline 三层编排 ───

    #[tokio::test]
    async fn tc_mp_010_pipeline_no_truncation() {
        let pipeline = MemoryPipeline::new(Box::new(SlidingWindowPolicy::new(10)));
        let messages = make_msgs(5);
        let result = pipeline.process(messages).await.unwrap();
        assert_eq!(result.len(), 5);
    }

    #[tokio::test]
    async fn tc_mp_011_pipeline_with_compactor() {
        let pipeline = MemoryPipeline::new(Box::new(SlidingWindowPolicy::new(3)))
            .with_compactor(Box::new(TextTruncationCompactor::new(500)));
        let messages = make_msgs(5);
        let result = pipeline.process(messages).await.unwrap();
        // kept(3) + summary(1) = 4
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].role, "system");
        assert!(result[0].content.contains("摘要"));
    }

    #[tokio::test]
    async fn tc_mp_012_pipeline_with_demotion_hook() {
        let (hook, count) = CountingDemotionHook::new();
        let pipeline = MemoryPipeline::new(Box::new(SlidingWindowPolicy::new(3)))
            .with_demotion_hook(Box::new(hook));
        let messages = make_msgs(5);
        pipeline.process(messages).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 2); // 2 条被降级
    }

    #[tokio::test]
    async fn tc_mp_013_pipeline_full_three_layers() {
        let (hook, count) = CountingDemotionHook::new();
        let pipeline = MemoryPipeline::new(Box::new(SlidingWindowPolicy::new(3)))
            .with_compactor(Box::new(TextTruncationCompactor::new(500)))
            .with_demotion_hook(Box::new(hook));
        let messages = make_msgs(5);
        let result = pipeline.process(messages).await.unwrap();
        // kept(3) + summary(1) = 4
        assert_eq!(result.len(), 4);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn tc_mp_014_pipeline_empty_messages() {
        let pipeline = MemoryPipeline::new(Box::new(SlidingWindowPolicy::new(5)));
        let result = pipeline.process(vec![]).await.unwrap();
        assert!(result.is_empty());
    }

    // ─── MemoryPolicyResult ───

    #[test]
    fn tc_mp_015_result_has_demoted() {
        let result = MemoryPolicyResult {
            kept: vec![make_msg("user", "recent")],
            demoted: vec![make_msg("user", "old")],
        };
        assert!(result.has_demoted());
    }

    #[test]
    fn tc_mp_016_result_no_demoted() {
        let result = MemoryPolicyResult::all_kept(vec![make_msg("user", "msg")]);
        assert!(!result.has_demoted());
    }
}
