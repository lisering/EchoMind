//! LLM API 类型化错误（B-02 借鉴 Rig `ProviderResponseError`）。
//!
//! ## 核心设计
//!
//! 保留 provider 返回的原始 HTTP status code 和响应体 JSON，使调用方能
//! 程序化精确区分错误类型（429 限流 / 401 认证失败 / 500 服务端错误），
//! 替代之前的字符串前缀匹配方案。
//!
//! ## 错误分类
//!
//! | HTTP Status | 分类 | 前端行为 |
//! |---|---|---|
//! | 401 / 403 | `Auth` | toast error「API 密钥无效」 |
//! | 429 | `RateLimit` | toast warning「请求过于频繁」+ 重试 |
//! | 5xx | `ServerError` | toast error「LLM 服务异常」+ 重试 |
//! | 其他 4xx | `ClientError` | toast error 显示原始消息 |
//! | 网络错误 | `Network` | toast error「网络连接异常」 |

use serde::{Deserialize, Serialize};

/// LLM API 错误分类（按 HTTP status code 自动分类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmErrorKind {
    /// 认证失败（HTTP 401/403）：API Key 无效或无权限
    Auth,
    /// 限流（HTTP 429）：请求频率超出限制
    RateLimit,
    /// 服务端错误（HTTP 5xx）：LLM 服务内部异常
    ServerError,
    /// 客户端错误（其他 HTTP 4xx）：请求参数错误等
    ClientError,
    /// 网络错误：连接超时、DNS 解析失败等
    Network,
    /// 未知错误
    Unknown,
}

impl LlmErrorKind {
    /// 从 HTTP status code 分类。
    pub fn from_status(status: u16) -> Self {
        match status {
            401 | 403 => Self::Auth,
            429 => Self::RateLimit,
            500..=599 => Self::ServerError,
            400..=499 => Self::ClientError,
            _ => Self::Unknown,
        }
    }

    /// 是否可重试（429 限流 + 5xx 服务端错误 + 网络错误）。
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::RateLimit | Self::ServerError | Self::Network)
    }

    /// 返回对应的错误前缀常量（与 `errors.rs` 中的前缀体系兼容）。
    pub fn error_prefix(self) -> &'static str {
        match self {
            Self::Auth => crate::errors::ERR_AUTH,
            Self::RateLimit => crate::errors::ERR_RATE_LIMIT,
            Self::ServerError | Self::Network => crate::errors::ERR_LLM,
            Self::ClientError | Self::Unknown => crate::errors::ERR_LLM,
        }
    }

    /// 返回字符串标识。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::RateLimit => "rate_limit",
            Self::ServerError => "server_error",
            Self::ClientError => "client_error",
            Self::Network => "network",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for LlmErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 类型化 LLM API 错误（保留原始 HTTP status + body）。
///
/// 替代之前的 `bail!("LLM API 错误 (HTTP {}): {}", status, text)` 格式化字符串，
/// 使调用方能程序化访问 `status` / `kind` / `body` 字段做精确路由。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmApiError {
    /// HTTP status code（网络错误时为 0）
    pub status: u16,
    /// 错误分类（从 status 派生）
    pub kind: LlmErrorKind,
    /// 原始响应体文本（截断至 500 字符，保持 UTF-8 字符边界）
    pub body: String,
    /// Provider 名称（如 "openai" / "deepseek" / "ollama"）
    pub provider: String,
}

impl LlmApiError {
    /// 创建新的 LLM API 错误。
    ///
    /// # 参数
    /// - `status`: HTTP status code（网络错误传 0）
    /// - `body`: 原始响应体文本（将自动截断至 500 字符）
    /// - `provider`: Provider 名称
    pub fn new(status: u16, body: &str, provider: &str) -> Self {
        Self {
            status,
            kind: LlmErrorKind::from_status(status),
            body: truncate_body(body, 500),
            provider: provider.to_string(),
        }
    }

    /// 创建网络错误（无 HTTP status）。
    pub fn network_error(detail: &str, provider: &str) -> Self {
        Self {
            status: 0,
            kind: LlmErrorKind::Network,
            body: detail.to_string(),
            provider: provider.to_string(),
        }
    }

    /// 转换为带前缀的错误字符串（与 `errors.rs` 前缀体系兼容）。
    ///
    /// 格式：`{PREFIX}: LLM API 错误 (HTTP {status}): {body}`
    pub fn to_prefixed_string(&self) -> String {
        if self.status == 0 {
            format!(
                "{}: {} 服务异常: {}",
                self.kind.error_prefix(),
                self.provider,
                self.body
            )
        } else {
            format!(
                "{}: {} API 错误 (HTTP {}): {}",
                self.kind.error_prefix(),
                self.provider,
                self.status,
                self.body
            )
        }
    }

    /// 是否可重试。
    pub fn is_retryable(&self) -> bool {
        self.kind.is_retryable()
    }
}

impl std::fmt::Display for LlmApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_prefixed_string())
    }
}

impl std::error::Error for LlmApiError {}

/// 截断错误响应体（保持字符边界，防超长错误刷屏）。
fn truncate_body(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ─── LlmErrorKind ───

    #[test]
    fn tc_llm_err_001_kind_from_status_401() {
        assert_eq!(LlmErrorKind::from_status(401), LlmErrorKind::Auth);
    }

    #[test]
    fn tc_llm_err_002_kind_from_status_403() {
        assert_eq!(LlmErrorKind::from_status(403), LlmErrorKind::Auth);
    }

    #[test]
    fn tc_llm_err_003_kind_from_status_429() {
        assert_eq!(LlmErrorKind::from_status(429), LlmErrorKind::RateLimit);
    }

    #[test]
    fn tc_llm_err_004_kind_from_status_500() {
        assert_eq!(LlmErrorKind::from_status(500), LlmErrorKind::ServerError);
    }

    #[test]
    fn tc_llm_err_005_kind_from_status_503() {
        assert_eq!(LlmErrorKind::from_status(503), LlmErrorKind::ServerError);
    }

    #[test]
    fn tc_llm_err_006_kind_from_status_400() {
        assert_eq!(LlmErrorKind::from_status(400), LlmErrorKind::ClientError);
    }

    #[test]
    fn tc_llm_err_007_kind_from_status_404() {
        assert_eq!(LlmErrorKind::from_status(404), LlmErrorKind::ClientError);
    }

    #[test]
    fn tc_llm_err_008_kind_from_status_200() {
        assert_eq!(LlmErrorKind::from_status(200), LlmErrorKind::Unknown);
    }

    #[test]
    fn tc_llm_err_009_kind_is_retryable() {
        assert!(LlmErrorKind::RateLimit.is_retryable());
        assert!(LlmErrorKind::ServerError.is_retryable());
        assert!(LlmErrorKind::Network.is_retryable());
        assert!(!LlmErrorKind::Auth.is_retryable());
        assert!(!LlmErrorKind::ClientError.is_retryable());
    }

    #[test]
    fn tc_llm_err_010_kind_error_prefix() {
        assert_eq!(LlmErrorKind::Auth.error_prefix(), "AUTH");
        assert_eq!(LlmErrorKind::RateLimit.error_prefix(), "RATE_LIMIT");
    }

    // ─── LlmApiError ───

    #[test]
    fn tc_llm_err_011_new_429() {
        let err = LlmApiError::new(429, "Too Many Requests", "deepseek");
        assert_eq!(err.status, 429);
        assert_eq!(err.kind, LlmErrorKind::RateLimit);
        assert_eq!(err.provider, "deepseek");
        assert!(err.is_retryable());
    }

    #[test]
    fn tc_llm_err_012_new_401() {
        let err = LlmApiError::new(401, "Unauthorized", "openai");
        assert_eq!(err.status, 401);
        assert_eq!(err.kind, LlmErrorKind::Auth);
        assert!(!err.is_retryable());
    }

    #[test]
    fn tc_llm_err_013_new_500() {
        let err = LlmApiError::new(500, "Internal Server Error", "ollama");
        assert_eq!(err.status, 500);
        assert_eq!(err.kind, LlmErrorKind::ServerError);
        assert!(err.is_retryable());
    }

    #[test]
    fn tc_llm_err_014_network_error() {
        let err = LlmApiError::network_error("connection refused", "openai");
        assert_eq!(err.status, 0);
        assert_eq!(err.kind, LlmErrorKind::Network);
        assert!(err.is_retryable());
    }

    #[test]
    fn tc_llm_err_015_to_prefixed_string_429() {
        let err = LlmApiError::new(429, "Too Many Requests", "deepseek");
        let s = err.to_prefixed_string();
        assert!(s.starts_with("RATE_LIMIT:"));
        assert!(s.contains("HTTP 429"));
        assert!(s.contains("Too Many Requests"));
    }

    #[test]
    fn tc_llm_err_016_to_prefixed_string_401() {
        let err = LlmApiError::new(401, "Unauthorized", "openai");
        let s = err.to_prefixed_string();
        assert!(s.starts_with("AUTH:"));
        assert!(s.contains("HTTP 401"));
    }

    #[test]
    fn tc_llm_err_017_to_prefixed_string_network() {
        let err = LlmApiError::network_error("timeout", "openai");
        let s = err.to_prefixed_string();
        assert!(s.starts_with("LLM:"));
        assert!(s.contains("timeout"));
        // 网络错误不含 HTTP 状态码
        assert!(!s.contains("HTTP"));
    }

    #[test]
    fn tc_llm_err_018_body_truncated() {
        let long_body = "a".repeat(1000);
        let err = LlmApiError::new(500, &long_body, "test");
        assert_eq!(err.body.len(), 500);
    }

    #[test]
    fn tc_llm_err_019_body_unicode_boundary() {
        // 中文字符 3 字节，截断到字符边界
        let body = "你好世界".repeat(100);
        let err = LlmApiError::new(500, &body, "test");
        assert_eq!(err.body.len() % 3, 0);
    }

    #[test]
    fn tc_llm_err_020_serde_roundtrip() {
        let err = LlmApiError::new(429, "rate limited", "deepseek");
        let json = serde_json::to_string(&err).unwrap();
        let back: LlmApiError = serde_json::from_str(&json).unwrap();
        assert_eq!(back.status, 429);
        assert_eq!(back.kind, LlmErrorKind::RateLimit);
        assert_eq!(back.body, "rate limited");
        assert_eq!(back.provider, "deepseek");
    }

    #[test]
    fn tc_llm_err_021_display() {
        let err = LlmApiError::new(401, "bad key", "openai");
        let s = format!("{err}");
        assert!(s.contains("AUTH:"));
        assert!(s.contains("HTTP 401"));
    }

    #[test]
    fn tc_llm_err_022_error_trait() {
        let err = LlmApiError::new(500, "server error", "test");
        // 验证实现了 std::error::Error trait
        let _: &dyn std::error::Error = &err;
    }
}
