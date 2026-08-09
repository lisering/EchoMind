//! # Prompt Caching 自动放置策略
//!
//! 借鉴 OpenCode v1.18.15 的 `packages/llm/src/cache-policy.ts`。
//!
//! `Auto` 策略自动在三个位置放置缓存断点：
//! 1. 最后一条系统消息（静态前缀末尾）
//! 2. 最新用户消息
//! 3. 工具定义末尾（Agent 模式时）
//!
//! EchoMind 适配：由于 `SegmentedPrompt` 是纯字符串结构，缓存策略通过标注
//! 哪些段应该被缓存来实现。`OpenAIProvider` 根据这些标注在 messages 数组中
//! 正确放置 system 消息，使前缀匹配 API 端的 prompt caching 机制。

use serde::{Deserialize, Serialize};

// ============================================================
// 缓存策略类型
// ============================================================

/// 缓存策略枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum CachePolicy {
    /// 自动放置（默认）：系统消息末尾 + 最新用户消息
    #[default]
    Auto,
    /// 禁用缓存
    None,
    /// 自定义放置位置
    Custom(CachePolicyObject),
}

/// 缓存策略配置对象
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachePolicyObject {
    /// 在最后一条系统消息处放置断点
    pub system: Option<bool>,
    /// 在用户消息处放置断点
    pub messages: Option<CacheMessagePolicy>,
}

/// 消息缓存策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CacheMessagePolicy {
    /// 最新用户消息
    LatestUserMessage,
    /// 所有用户消息
    AllUserMessages,
    /// 不在消息处放置
    None,
}

// Default trait 由 #[derive(Default)] + #[default] 属性实现
// impl Default for CachePolicy 已由 derive 宏自动生成

// ============================================================
// 缓存断点标记
// ============================================================

/// 缓存断点位置（标注哪些段应该被 API 端缓存）
#[derive(Debug, Clone, PartialEq)]
pub enum CacheBreakpoint {
    /// 在此段末尾放置缓存断点
    After,
    /// 不放置缓存断点
    None,
}

/// 带缓存标注的分段提示词
///
/// 在 `SegmentedPrompt` 基础上增加缓存断点标注，
/// 供 `OpenAIProvider` 在构建 messages 数组时决定缓存放置位置。
#[derive(Debug, Clone)]
pub struct CachedSegmentedPrompt {
    /// 静态前缀文本（跨请求不变）
    pub static_prefix: String,
    /// 静态前缀的缓存断点（Auto 策略下为 After）
    pub static_prefix_cache: CacheBreakpoint,
    /// 动态上下文文本（每次请求不同）
    pub dynamic_context: String,
    /// 动态上下文的缓存断点（通常为 None）
    pub dynamic_context_cache: CacheBreakpoint,
}

impl CachedSegmentedPrompt {
    /// 将两段拼接为单个字符串（向后兼容）
    pub fn to_combined_string(&self) -> String {
        format!("{}\n\n{}", self.static_prefix, self.dynamic_context)
    }
}

// ============================================================
// 策略解析
// ============================================================

/// 解析缓存策略为具体配置
fn resolve(policy: &CachePolicy) -> CachePolicyObject {
    match policy {
        CachePolicy::Auto => CachePolicyObject {
            system: Some(true),
            messages: Some(CacheMessagePolicy::LatestUserMessage),
        },
        CachePolicy::None => CachePolicyObject {
            system: None,
            messages: None,
        },
        CachePolicy::Custom(obj) => obj.clone(),
    }
}

/// 应用缓存策略到分段提示词
///
/// 根据 `CachePolicy` 在 `SegmentedPrompt` 上标注缓存断点位置。
///
/// # Auto 策略行为
/// - 静态前缀末尾放置 `After` 断点（系统消息末尾）
/// - 动态上下文不放置断点（每次不同）
/// - 最新用户消息断点由 `OpenAIProvider` 在构建 messages 数组时处理
pub fn apply_cache_policy(
    static_prefix: &str,
    dynamic_context: &str,
    policy: &CachePolicy,
) -> CachedSegmentedPrompt {
    let resolved = resolve(policy);

    let static_cache = if resolved.system == Some(true) {
        CacheBreakpoint::After
    } else {
        CacheBreakpoint::None
    };

    // 动态上下文通常不缓存（每次请求不同）
    let dynamic_cache = CacheBreakpoint::None;

    CachedSegmentedPrompt {
        static_prefix: static_prefix.to_string(),
        static_prefix_cache: static_cache,
        dynamic_context: dynamic_context.to_string(),
        dynamic_context_cache: dynamic_cache,
    }
}

// ============================================================
// 辅助函数
// ============================================================

/// 检查缓存策略是否在系统消息处放置断点
pub fn caches_system_messages(policy: &CachePolicy) -> bool {
    resolve(policy).system == Some(true)
}

/// 检查缓存策略是否在最新用户消息处放置断点
pub fn caches_latest_user_message(policy: &CachePolicy) -> bool {
    resolve(policy).messages == Some(CacheMessagePolicy::LatestUserMessage)
}

/// 检查缓存策略是否在所有用户消息处放置断点
pub fn caches_all_user_messages(policy: &CachePolicy) -> bool {
    resolve(policy).messages == Some(CacheMessagePolicy::AllUserMessages)
}

// 测试在 lib.rs 中注册（crates/prompt/src/cache_policy_tests.rs）
