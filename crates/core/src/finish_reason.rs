//! FinishReason 归一化枚举（借鉴 Rig `FinishReason`，REQ-RAG-022 增强）。
//!
//! ## 核心设计
//!
//! 归一化所有 provider 的停止原因为统一枚举，使上层逻辑能程序化判断
//! 「LLM 自然结束」vs「被 max_tokens 截断」vs「工具调用结束」等场景。
//!
//! ## reconcile_with_output()
//!
//! 当 provider 报告 `Stop` 但输出中包含 tool_call 模式时，自动升级为 `ToolCalls`。
//! 这解决某些 provider 在工具调用场景下错误报告 `stop` 的问题。
//!
//! ## truncated_output()
//!
//! 判定输出是否被截断（`Length` | `ContentFilter`），驱动 AgentEngine
//! 自动翻倍 `max_tokens` 重试逻辑。

use serde::{Deserialize, Serialize};

/// LLM 响应的归一化停止原因。
///
/// 统一映射各 provider 的 `finish_reason` 字段：
///
/// | Provider | 原始值 | 归一化 |
/// |---|---|---|
/// | OpenAI | `stop` | `Stop` |
/// | OpenAI | `length` | `Length` |
/// | OpenAI | `tool_calls` / `function_call` | `ToolCalls` |
/// | OpenAI | `content_filter` | `ContentFilter` |
/// | 其他 | 任意非识别值 | `Other` |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    /// LLM 自然停止生成（完整回答）
    Stop,
    /// 达到 max_tokens 上限，输出被截断
    Length,
    /// LLM 发起工具调用（function call）
    ToolCalls,
    /// 内容被安全过滤器拦截
    ContentFilter,
    /// 其他未识别的停止原因
    Other,
}

impl FinishReason {
    /// 从 provider 原始字符串解析为归一化枚举。
    ///
    /// 支持的输入（大小写不敏感）：
    /// - `"stop"` → `Stop`
    /// - `"length"` → `Length`
    /// - `"tool_calls"` / `"function_call"` → `ToolCalls`
    /// - `"content_filter"` → `ContentFilter`
    /// - 其他 / 空 → `Other`
    pub fn from_provider_str(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "stop" => Self::Stop,
            "length" => Self::Length,
            "tool_calls" | "function_call" => Self::ToolCalls,
            "content_filter" => Self::ContentFilter,
            _ => Self::Other,
        }
    }

    /// 根据实际输出内容修正停止原因（借鉴 Rig `reconcile_with_output`）。
    ///
    /// 当 provider 报告 `Stop` 但输出文本包含 ReAct Action 模式
    /// （`Action:` / `Action N:`）时，说明 LLM 实际发起了工具调用，
    /// provider 错误报告了 `stop`。此方法将其升级为 `ToolCalls`。
    ///
    /// # 参数
    /// - `output`: LLM 生成的完整文本输出
    ///
    /// # 返回
    /// 修正后的 `FinishReason`：
    /// - `Stop` + 输出含 Action 模式 → `ToolCalls`
    /// - 其他情况 → 原始值不变
    pub fn reconcile_with_output(self, output: &str) -> Self {
        match self {
            Self::Stop if has_action_pattern(output) => Self::ToolCalls,
            _ => self,
        }
    }

    /// 判断输出是否被截断（借鉴 Rig `truncated_output`）。
    ///
    /// `Length`（max_tokens 截断）和 `ContentFilter`（内容过滤截断）
    /// 都表示输出不完整，上层可据此触发重试逻辑。
    ///
    /// # 返回
    /// - `true` — 输出被截断，可能需要增大 `max_tokens` 重试
    /// - `false` — 输出完整或为工具调用（不需要重试）
    pub fn is_truncated(self) -> bool {
        matches!(self, Self::Length | Self::ContentFilter)
    }

    /// 返回字符串标识（用于日志和序列化）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Length => "length",
            Self::ToolCalls => "tool_calls",
            Self::ContentFilter => "content_filter",
            Self::Other => "other",
        }
    }
}

impl Default for FinishReason {
    fn default() -> Self {
        Self::Other
    }
}

impl std::fmt::Display for FinishReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 检测输出文本是否包含 ReAct Action 模式。
///
/// 匹配 `Action:` 或 `Action N:` 格式（N 为数字），大小写不敏感。
fn has_action_pattern(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim().to_lowercase();
        trimmed.starts_with("action:")
            || trimmed.starts_with("action ") && {
                let after = &trimmed[7..]; // "action ".len()
                after.bytes().next().is_some_and(|b| b.is_ascii_digit())
            }
    })
}
