//! 统一错误分类体系（REQ-ERR-001）。
//!
//! 所有面向前端的错误消息使用 `{PREFIX}: {detail}` 格式，
//! 前端根据前缀选择不同的展示策略（toast kind / paywall / 状态更新）。
//!
//! ## 错误前缀清单
//!
//! | 前缀 | 常量 | 含义 | 前端行为 |
//! |---|---|---|---|
//! | `LIMIT_REACHED` | `ERR_LIMIT_REACHED` | 免费版配额触顶 | 弹出付费墙 |
//! | `PRO_REQUIRED` | `ERR_PRO_REQUIRED` | Pro 功能门禁 | 弹出付费墙 |
//! | `NETWORK` | `ERR_NETWORK` | 网络连接异常 | toast error「网络连接异常」 |
//! | `AUTH` | `ERR_AUTH` | 认证失败（401/403） | toast error「认证失败，请检查 API Key」 |
//! | `PARSE` | `ERR_PARSE` | 文件解析失败 | toast error「文件解析失败」 |
//! | `EMBED` | `ERR_EMBED` | 向量化失败 | toast error「向量化失败」 |
//! | `LLM` | `ERR_LLM` | LLM 服务异常 | toast error「LLM 服务异常」 |
//! | `STORAGE` | `ERR_STORAGE` | 存储异常 | toast error「存储异常」 |
//! | `DISK_FULL` | `ERR_DISK_FULL` | 磁盘空间不足 | toast error「磁盘空间不足，请清理文件」 |
//! | `RATE_LIMIT` | `ERR_RATE_LIMIT` | LLM API 限流 | toast warning「请求过于频繁，请稍后重试」 |
//! | `VALIDATION` | `ERR_VALIDATION` | 输入校验失败 | toast warning（显示原始消息） |
//! | `UNKNOWN` | `ERR_UNKNOWN` | 未知错误 | toast error「未知错误」 |

/// 配额触顶错误前缀（REQ-LIC-002；前端据此弹出付费墙）
pub const ERR_LIMIT_REACHED: &str = "LIMIT_REACHED";
/// PDF 付费门错误前缀（REQ-LIC-002；前端据此弹出付费墙）
pub const ERR_PRO_REQUIRED: &str = "PRO_REQUIRED";
/// 网络连接异常前缀（REQ-ERR-001）
pub const ERR_NETWORK: &str = "NETWORK";
/// 认证失败前缀（REQ-ERR-001）
pub const ERR_AUTH: &str = "AUTH";
/// 文件解析失败前缀（REQ-ERR-001）
pub const ERR_PARSE: &str = "PARSE";
/// 向量化失败前缀（REQ-ERR-001）
pub const ERR_EMBED: &str = "EMBED";
/// LLM 服务异常前缀（REQ-ERR-001）
pub const ERR_LLM: &str = "LLM";
/// 存储异常前缀（REQ-ERR-001）
pub const ERR_STORAGE: &str = "STORAGE";
/// 磁盘空间不足前缀（P1-1：磁盘满弹性设计）。
///
/// 当磁盘可用空间低于阈值时，写入操作返回此前缀错误，
/// 前端展示「磁盘空间不足，请清理文件」提示 + 引导用户清理。
pub const ERR_DISK_FULL: &str = "DISK_FULL";
/// LLM API 限流前缀（P1-2：429 限流分类）。
///
/// 当 LLM API 返回 HTTP 429 时，错误消息使用此前缀，
/// 前端展示「请求过于频繁，请稍后重试」+ wait/switch_model 操作按钮。
pub const ERR_RATE_LIMIT: &str = "RATE_LIMIT";
/// 输入校验失败前缀（REQ-ERR-005）
pub const ERR_VALIDATION: &str = "VALIDATION";
/// 未知错误前缀（REQ-ERR-001）
pub const ERR_UNKNOWN: &str = "UNKNOWN";

/// 查询长度上限（REQ-ERR-005）：超过此长度拒绝执行。
pub const MAX_QUERY_LENGTH: usize = 8000;
/// API Key 长度上限（REQ-ERR-005）：超过此长度拒绝写入。
pub const MAX_API_KEY_LENGTH: usize = 256;

/// 将错误消息格式化为 `{PREFIX}: {detail}` 格式。
///
/// # 参数
/// - `prefix`: 错误前缀常量（如 `ERR_NETWORK`）
/// - `detail`: 错误详情文本
///
/// # 返回
/// 格式化后的错误字符串，如 `"NETWORK: connection refused"`
pub fn prefix_error(prefix: &str, detail: &str) -> String {
    format!("{prefix}: {detail}")
}

/// 分类 LLM 错误并添加前缀（REQ-ERR-001）。
///
/// 根据 OpenAIProvider 返回的错误消息内容，判断错误类型：
/// - 包含 `HTTP 401` / `HTTP 403` → `AUTH:` 前缀
/// - 包含网络相关关键词（连接失败 / 超时 / SSE 流） → `NETWORK:` 前缀
/// - 其他 LLM 错误 → `LLM:` 前缀
///
/// # 参数
/// - `err`: 原始错误消息（来自 OpenAIProvider 或 ChatEngine）
///
/// # 返回
/// 带前缀的错误字符串
pub fn classify_llm_error(err: &str) -> String {
    // 认证失败：HTTP 401/403
    if err.contains("HTTP 401") || err.contains("HTTP 403") {
        return prefix_error(ERR_AUTH, err);
    }
    // 网络错误：连接失败 / 超时 / SSE 流读取失败
    if err.contains("请求发送失败")
        || err.contains("无法连接")
        || err.contains("timeout")
        || err.contains("超时")
        || err.contains("SSE 流")
        || err.contains("connection")
        || err.contains("connect")
        || err.contains("Connection refused")
        || err.contains("dns")
        || err.contains("DNS")
    {
        return prefix_error(ERR_NETWORK, err);
    }
    // 限流：HTTP 429 / rate limit / Too Many Requests
    if err.contains("HTTP 429")
        || err.contains("rate limit")
        || err.contains("Rate limit")
        || err.contains("Too Many")
        || err.contains("too many")
        || err.contains("quota")
        || err.contains("Quota")
    {
        return prefix_error(ERR_RATE_LIMIT, err);
    }
    // 其他 LLM 错误
    prefix_error(ERR_LLM, err)
}

/// 判断错误消息是否已包含错误前缀（`PREFIX:` 格式）。
///
/// 用于避免重复添加前缀（例如错误已在底层被分类，上层不应再次包装）。
///
/// # 参数
/// - `err`: 待检查的错误消息
///
/// # 返回
/// `true` 如果消息以已知的错误前缀开头
pub fn has_error_prefix(err: &str) -> bool {
    let prefixes = [
        ERR_LIMIT_REACHED,
        ERR_PRO_REQUIRED,
        ERR_NETWORK,
        ERR_AUTH,
        ERR_PARSE,
        ERR_EMBED,
        ERR_LLM,
        ERR_STORAGE,
        ERR_DISK_FULL,
        ERR_RATE_LIMIT,
        ERR_VALIDATION,
        ERR_UNKNOWN,
    ];
    for prefix in &prefixes {
        if err.starts_with(&format!("{prefix}:")) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── 常量值验证 ───

    #[test]
    fn test_err_constants_values() {
        assert_eq!(ERR_LIMIT_REACHED, "LIMIT_REACHED");
        assert_eq!(ERR_PRO_REQUIRED, "PRO_REQUIRED");
        assert_eq!(ERR_NETWORK, "NETWORK");
        assert_eq!(ERR_AUTH, "AUTH");
        assert_eq!(ERR_PARSE, "PARSE");
        assert_eq!(ERR_EMBED, "EMBED");
        assert_eq!(ERR_LLM, "LLM");
        assert_eq!(ERR_STORAGE, "STORAGE");
        assert_eq!(ERR_DISK_FULL, "DISK_FULL");
        assert_eq!(ERR_RATE_LIMIT, "RATE_LIMIT");
        assert_eq!(ERR_VALIDATION, "VALIDATION");
        assert_eq!(ERR_UNKNOWN, "UNKNOWN");
    }

    #[test]
    fn test_limit_constants() {
        assert_eq!(MAX_QUERY_LENGTH, 8000);
        assert_eq!(MAX_API_KEY_LENGTH, 256);
    }

    // ─── prefix_error ───

    #[test]
    fn test_prefix_error_format() {
        assert_eq!(prefix_error(ERR_NETWORK, "timeout"), "NETWORK: timeout");
        assert_eq!(prefix_error(ERR_AUTH, "invalid key"), "AUTH: invalid key");
        assert_eq!(
            prefix_error(ERR_VALIDATION, "query too long"),
            "VALIDATION: query too long"
        );
    }

    #[test]
    fn test_prefix_error_empty_detail() {
        assert_eq!(prefix_error(ERR_UNKNOWN, ""), "UNKNOWN: ");
    }

    // ─── classify_llm_error ───

    #[test]
    fn test_classify_auth_401() {
        let result = classify_llm_error("LLM API 错误 (HTTP 401): Unauthorized");
        assert!(result.starts_with("AUTH:"));
        assert!(result.contains("HTTP 401"));
    }

    #[test]
    fn test_classify_auth_403() {
        let result = classify_llm_error("LLM API 错误 (HTTP 403): Forbidden");
        assert!(result.starts_with("AUTH:"));
        assert!(result.contains("HTTP 403"));
    }

    #[test]
    fn test_classify_network_connection_failed() {
        let result = classify_llm_error("LLM 请求发送失败: Connection refused");
        assert!(result.starts_with("NETWORK:"));
    }

    #[test]
    fn test_classify_network_timeout() {
        let result = classify_llm_error("请求超时: timeout (30s)");
        assert!(result.starts_with("NETWORK:"));
    }

    #[test]
    fn test_classify_network_sse_error() {
        let result = classify_llm_error("SSE 流读取失败: connection reset");
        assert!(result.starts_with("NETWORK:"));
    }

    #[test]
    fn test_classify_network_dns_error() {
        let result = classify_llm_error("dns error: failed to lookup address");
        assert!(result.starts_with("NETWORK:"));
    }

    #[test]
    fn test_classify_llm_other_http_error() {
        let result = classify_llm_error("LLM API 错误 (HTTP 500): Internal Server Error");
        assert!(result.starts_with("LLM:"));
        assert!(result.contains("HTTP 500"));
    }

    #[test]
    fn test_classify_llm_other_error() {
        let result = classify_llm_error("JSON 解析失败: invalid response");
        assert!(result.starts_with("LLM:"));
    }

    // ─── has_error_prefix ───

    #[test]
    fn test_has_error_prefix_with_prefix() {
        assert!(has_error_prefix("NETWORK: timeout"));
        assert!(has_error_prefix("AUTH: invalid key"));
        assert!(has_error_prefix("VALIDATION: query too long"));
        assert!(has_error_prefix("LIMIT_REACHED: 50 files"));
        assert!(has_error_prefix("UNKNOWN: unexpected"));
    }

    // ─── classify_llm_error: 429 限流 ───

    #[test]
    fn test_classify_rate_limit_429() {
        let result = classify_llm_error("LLM API 错误 (HTTP 429): Too Many Requests");
        assert!(
            result.starts_with("RATE_LIMIT:"),
            "429 应分类为 RATE_LIMIT，实际: {result}"
        );
        assert!(result.contains("HTTP 429"));
    }

    #[test]
    fn test_classify_rate_limit_text() {
        let result = classify_llm_error("rate limit exceeded");
        assert!(result.starts_with("RATE_LIMIT:"));
    }

    #[test]
    fn test_classify_rate_limit_too_many() {
        let result = classify_llm_error("Too Many Requests");
        assert!(result.starts_with("RATE_LIMIT:"));
    }

    #[test]
    fn test_classify_rate_limit_quota() {
        let result = classify_llm_error("quota exceeded for this API key");
        assert!(result.starts_with("RATE_LIMIT:"));
    }

    #[test]
    fn test_classify_rate_limit_not_confused_with_llm() {
        // 429 不应被分类为 LLM 前缀
        let result = classify_llm_error("LLM API 错误 (HTTP 429): Rate limit");
        assert!(!result.starts_with("LLM:"));
        assert!(result.starts_with("RATE_LIMIT:"));
    }

    #[test]
    fn test_has_error_prefix_disk_full() {
        assert!(has_error_prefix("DISK_FULL: 磁盘空间不足"));
    }

    #[test]
    fn test_has_error_prefix_rate_limit() {
        assert!(has_error_prefix("RATE_LIMIT: 请求过于频繁"));
    }

    #[test]
    fn test_has_error_prefix_without_prefix() {
        assert!(!has_error_prefix("普通错误消息"));
        assert!(!has_error_prefix("HTTP 401: Unauthorized"));
        assert!(!has_error_prefix(""));
    }

    #[test]
    fn test_has_error_prefix_partial_match() {
        // 不应匹配 — 前缀不在开头
        assert!(!has_error_prefix("error: NETWORK: timeout"));
    }
}
