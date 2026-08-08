//! 多协议 LLM 适配（B10 LLM Protocol Types，REQ-ARCH-012）。
//!
//! 借鉴 OpenCode `packages/llm/`：定义 LLM 协议类型枚举和协议感知 Provider trait。
//!
//! ## 设计
//!
//! - **协议枚举**：`LlmProtocol` 定义 4 种 LLM 协议类型
//! - **字符串映射**：`as_str()` / `from_str()` 用于序列化和设置面板
//! - **协议感知 trait**：`ProtocolAwareProvider` 扩展 `LLMProvider`，提供协议类型查询
//! - **纯数据类型**：本模块仅定义类型，不涉及具体 API 调用实现

use crate::LLMProvider;
use serde::{Deserialize, Serialize};

/// LLM 协议类型（支持 4 种主流 LLM API 协议）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmProtocol {
    /// OpenAI Chat Completions API（现有默认协议）
    OpenAIChat,
    /// OpenAI Responses API（新版 OpenAI 协议）
    OpenAIResponses,
    /// Anthropic Messages API（原生 Prompt Caching 支持）
    AnthropicMessages,
    /// Google Gemini API
    Gemini,
}

impl LlmProtocol {
    /// 返回协议的字符串标识（用于序列化和设置面板）。
    ///
    /// # 示例
    ///
    /// ```
    /// use echomind_core::llm_protocol::LlmProtocol;
    ///
    /// assert_eq!(LlmProtocol::OpenAIChat.as_str(), "openai-chat");
    /// assert_eq!(LlmProtocol::AnthropicMessages.as_str(), "anthropic-messages");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            LlmProtocol::OpenAIChat => "openai-chat",
            LlmProtocol::OpenAIResponses => "openai-responses",
            LlmProtocol::AnthropicMessages => "anthropic-messages",
            LlmProtocol::Gemini => "gemini",
        }
    }

    /// 从字符串解析协议类型（用于反序列化和设置面板）。
    ///
    /// # 示例
    ///
    /// ```
    /// use echomind_core::llm_protocol::LlmProtocol;
    ///
    /// assert_eq!(LlmProtocol::parse_str("openai-chat"), Some(LlmProtocol::OpenAIChat));
    /// assert_eq!(LlmProtocol::parse_str("anthropic-messages"), Some(LlmProtocol::AnthropicMessages));
    /// assert_eq!(LlmProtocol::parse_str("unknown"), None);
    /// ```
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "openai-chat" => Some(LlmProtocol::OpenAIChat),
            "openai-responses" => Some(LlmProtocol::OpenAIResponses),
            "anthropic-messages" => Some(LlmProtocol::AnthropicMessages),
            "gemini" => Some(LlmProtocol::Gemini),
            _ => None,
        }
    }

    /// 返回所有协议类型的列表（用于设置面板下拉选项）。
    pub fn all() -> &'static [LlmProtocol] {
        &[
            LlmProtocol::OpenAIChat,
            LlmProtocol::OpenAIResponses,
            LlmProtocol::AnthropicMessages,
            LlmProtocol::Gemini,
        ]
    }
}

/// 协议感知的 LLM Provider（扩展 `LLMProvider` trait）。
///
/// 实现此 trait 的 Provider 可以声明自己使用的协议类型，
/// 供上层根据协议类型选择不同的调用策略（如 Anthropic 的 Prompt Caching）。
pub trait ProtocolAwareProvider: LLMProvider {
    /// 返回当前 Provider 使用的协议类型。
    fn protocol(&self) -> LlmProtocol;
}

// ============================================================================
// TDD 测试（TC-PROTO-001~004，对应 REQ-ARCH-012 AC-1~AC-4）
// ============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    /// TC-PROTO-001：`LlmProtocol` 枚举包含 4 种协议变体（AC-1）。
    #[test]
    fn tc_proto_001_four_protocol_variants() {
        let all = LlmProtocol::all();
        assert_eq!(all.len(), 4, "应有 4 种协议变体");
        assert!(all.contains(&LlmProtocol::OpenAIChat));
        assert!(all.contains(&LlmProtocol::OpenAIResponses));
        assert!(all.contains(&LlmProtocol::AnthropicMessages));
        assert!(all.contains(&LlmProtocol::Gemini));
    }

    /// TC-PROTO-002：`parse_str()` 从字符串正确解析协议类型（AC-2）。
    #[test]
    fn tc_proto_002_parse_str_parses_correctly() {
        assert_eq!(
            LlmProtocol::parse_str("openai-chat"),
            Some(LlmProtocol::OpenAIChat)
        );
        assert_eq!(
            LlmProtocol::parse_str("openai-responses"),
            Some(LlmProtocol::OpenAIResponses)
        );
        assert_eq!(
            LlmProtocol::parse_str("anthropic-messages"),
            Some(LlmProtocol::AnthropicMessages)
        );
        assert_eq!(LlmProtocol::parse_str("gemini"), Some(LlmProtocol::Gemini));
    }

    /// TC-PROTO-003：`as_str()` 返回协议字符串标识（AC-3）。
    #[test]
    fn tc_proto_003_as_str_returns_identifier() {
        assert_eq!(LlmProtocol::OpenAIChat.as_str(), "openai-chat");
        assert_eq!(LlmProtocol::OpenAIResponses.as_str(), "openai-responses");
        assert_eq!(
            LlmProtocol::AnthropicMessages.as_str(),
            "anthropic-messages"
        );
        assert_eq!(LlmProtocol::Gemini.as_str(), "gemini");
    }

    /// TC-PROTO-004：未知协议字符串返回 `None`（AC-4）。
    #[test]
    fn tc_proto_004_unknown_returns_none() {
        assert_eq!(LlmProtocol::parse_str("unknown"), None);
        assert_eq!(LlmProtocol::parse_str(""), None);
        assert_eq!(LlmProtocol::parse_str("claude"), None);
        assert_eq!(LlmProtocol::parse_str("OPENAI"), None); // 大小写敏感
    }

    /// 额外测试：`as_str()` 和 `parse_str()` 往返一致性。
    #[test]
    fn tc_proto_extra_roundtrip() {
        for proto in LlmProtocol::all() {
            let s = proto.as_str();
            assert_eq!(LlmProtocol::parse_str(s), Some(*proto), "往返不一致: {s}");
        }
    }
}
