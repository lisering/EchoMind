//! # 隐私保护模块
//!
//! 提供 PII（个人身份信息）检测和脱敏功能，以及操作审计日志。
//!
//! ## PII 脱敏
//!
//! [`PiiRedactor`] 使用正则表达式检测文本中的敏感信息：
//! - 邮箱地址、中国手机号、身份证号、银行卡号
//! - IP 地址、国际手机号、美国社会安全号、中国护照号
//!
//! 检测到的 PII 会被替换为脱敏形式（如 `j***@example.com`）。
//!
//! ## 审计日志
//!
//! [`AuditLogger`] trait 定义审计日志的持久化接口。
//! [`InMemoryAuditLogger`] 提供内存实现（用于测试）。

// 预编译正则表达式使用 unwrap() 初始化：所有 pattern 均为编译时已知的有效正则，
// unwrap() 不会触发 panic，但 clippy 无法静态验证这一点。
#![allow(clippy::unwrap_used)]

use anyhow::Result;
use std::sync::Arc;
use std::sync::LazyLock;
use tracing::debug;

// ============================================================================
// 预编译正则表达式（LazyLock 保证线程安全的延迟初始化，仅编译一次）
// ============================================================================

/// 邮箱正则：标准 email 格式
static EMAIL_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());

/// 中国手机号正则：1 开头的 11 位数字
static PHONE_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"1[3-9]\d{9}").unwrap());

/// 身份证号正则：18 位（前 17 位数字 + 末位数字或 X）
static ID_CARD_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\d{17}[\dXx]").unwrap());

/// 银行卡号正则：16-19 位连续数字
static BANK_CARD_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\d{16,19}").unwrap());

/// IPv4 地址正则
static IP_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap());

/// 国际手机号正则（E.164 格式：+国家码 + 号码，总长 8-15 位）
static INTL_PHONE_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\+\d{7,15}").unwrap());

/// 美国社会安全号正则（SSN：XXX-XX-XXXX）
static SSN_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());

/// 中国护照号正则（E/G + 8位数字）
static PASSPORT_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b[EGeg]\d{8}\b").unwrap());

/// Luhn 校验算法
///
/// 用于验证银行卡号/信用卡号的有效性。
/// 注意：仅对 16 位数字（信用卡长度）执行校验，
/// 17-19 位数字（可能是中国银联借记卡）不遵循 Luhn，跳过校验。
#[must_use]
fn luhn_check(number: &str) -> bool {
    let digits: Vec<u32> = number.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.len() < 13 || digits.len() > 19 {
        return false;
    }
    // 仅对 16 位数字执行 Luhn 校验（信用卡标准长度）
    if digits.len() != 16 {
        return true;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

/// PII 类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PiiType {
    /// 邮箱地址
    Email,
    /// 手机号码
    Phone,
    /// 身份证号码
    IdCard,
    /// 银行卡号
    BankCard,
    /// IP 地址
    IpAddress,
    /// 国际手机号（E.164 格式：+XX...）
    InternationalPhone,
    /// 美国社会安全号（SSN：XXX-XX-XXXX）
    Ssn,
    /// 护照号码（中国：E/G + 8位数字）
    Passport,
}

/// PII 检测结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct PiiDetection {
    /// PII 类型
    pub pii_type: PiiType,
    /// 匹配的原始文本
    pub matched: String,
    /// 脱敏后的文本
    pub redacted: String,
    /// 在原文中的起始位置
    pub start: usize,
    /// 在原文中的结束位置
    pub end: usize,
}

/// PII 脱敏器
///
/// 使用正则表达式检测和脱敏文本中的个人身份信息。
pub struct PiiRedactor {
    /// 是否启用脱敏（默认 true）
    enabled: bool,
}

impl PiiRedactor {
    /// 创建新的 PII 脱敏器
    #[must_use]
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// 创建禁用的脱敏器（不做任何处理）
    #[must_use]
    pub fn disabled() -> Self {
        Self { enabled: false }
    }

    /// 设置启用/禁用状态
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 检测文本中的所有 PII
    #[must_use]
    pub fn detect(&self, text: &str) -> Vec<PiiDetection> {
        if !self.enabled {
            return Vec::new();
        }

        let mut detections = Vec::new();

        // 邮箱
        for mat in EMAIL_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            let redacted = Self::redact_email(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::Email,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 中国手机号
        for mat in PHONE_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            let redacted = Self::redact_phone(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::Phone,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 身份证号（18位）
        for mat in ID_CARD_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            let redacted = Self::redact_id_card(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::IdCard,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 银行卡号（16-19位数字，需通过 Luhn 校验）
        for mat in BANK_CARD_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            if !luhn_check(&matched) {
                continue;
            }
            let redacted = Self::redact_bank_card(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::BankCard,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // IP 地址
        for mat in IP_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            let redacted = Self::redact_ip(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::IpAddress,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 国际手机号（E.164 格式）
        for mat in INTL_PHONE_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            let redacted = Self::redact_intl_phone(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::InternationalPhone,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 美国社会安全号（SSN）
        for mat in SSN_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            // SSN 排除规则：area 不能为 000、666、9xx
            let area = &matched[..3];
            if area == "000" || area == "666" || area.starts_with('9') {
                continue;
            }
            let redacted = Self::redact_ssn(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::Ssn,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 中国护照号
        for mat in PASSPORT_REGEX.find_iter(text) {
            let matched = mat.as_str().to_string();
            let redacted = Self::redact_passport(&matched);
            detections.push(PiiDetection {
                pii_type: PiiType::Passport,
                matched,
                redacted,
                start: mat.start(),
                end: mat.end(),
            });
        }

        // 按位置排序
        detections.sort_by_key(|d| d.start);
        detections
    }

    /// 脱敏文本中的所有 PII（返回脱敏后的文本）
    #[must_use]
    pub fn redact_text(&self, text: &str) -> String {
        if !self.enabled {
            return text.to_string();
        }

        let detections = self.detect(text);
        if detections.is_empty() {
            return text.to_string();
        }

        let mut result = String::with_capacity(text.len());
        let mut last_end = 0;

        for det in &detections {
            if det.start < last_end {
                continue;
            }
            result.push_str(&text[last_end..det.start]);
            result.push_str(&det.redacted);
            last_end = det.end;
        }
        result.push_str(&text[last_end..]);

        result
    }

    /// 脱敏邮箱：j***@example.com
    fn redact_email(email: &str) -> String {
        if let Some(at_pos) = email.find('@') {
            let (local, domain) = email.split_at(at_pos);
            if local.len() > 1 {
                format!("{}***{}", &local[..1], domain)
            } else {
                format!("***{domain}")
            }
        } else {
            "***".to_string()
        }
    }

    /// 脱敏手机号：138****1234
    fn redact_phone(phone: &str) -> String {
        if phone.len() >= 7 {
            format!("{}****{}", &phone[..3], &phone[phone.len() - 4..])
        } else {
            "****".to_string()
        }
    }

    /// 脱敏身份证：110***********1234
    fn redact_id_card(id: &str) -> String {
        if id.len() >= 6 {
            format!("{}**************{}", &id[..3], &id[id.len() - 4..])
        } else {
            "****".to_string()
        }
    }

    /// 脱敏银行卡：6222****1234
    fn redact_bank_card(card: &str) -> String {
        if card.len() >= 8 {
            format!("{}****{}", &card[..4], &card[card.len() - 4..])
        } else {
            "****".to_string()
        }
    }

    /// 脱敏 IP 地址：192.168.*.*
    fn redact_ip(ip: &str) -> String {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() == 4 {
            format!("{}.{}.**.**", parts[0], parts[1])
        } else {
            "***".to_string()
        }
    }

    /// 脱敏国际手机号：+86***5678
    fn redact_intl_phone(phone: &str) -> String {
        if phone.len() >= 6 {
            format!("{}****{}", &phone[..3], &phone[phone.len() - 4..])
        } else {
            "****".to_string()
        }
    }

    /// 脱敏 SSN：XXX-**-XXXX
    fn redact_ssn(ssn: &str) -> String {
        if ssn.len() >= 11 {
            format!("{}-**-{}", &ssn[..3], &ssn[7..])
        } else {
            "****".to_string()
        }
    }

    /// 脱敏护照号：E****5678
    fn redact_passport(passport: &str) -> String {
        if passport.len() >= 5 {
            format!("{}****{}", &passport[..1], &passport[passport.len() - 4..])
        } else {
            "****".to_string()
        }
    }
}

impl Default for PiiRedactor {
    fn default() -> Self {
        Self::new()
    }
}

/// 审计日志条目
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    /// 日志唯一标识
    pub id: String,
    /// 操作类型（如 "search", "chat", "import", "delete"）
    pub action: String,
    /// 操作详情（JSON 格式）
    pub details: String,
    /// 检测到的 PII 数量
    pub pii_count: usize,
    /// 时间戳
    pub timestamp: i64,
    /// 前一条日志的哈希值（哈希链）
    ///
    /// 每条日志附加前一条的哈希链，
    /// 删除任意一条会导致后续验证失败。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
    /// 当前日志的哈希值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub curr_hash: Option<String>,
}

/// 审计日志 trait
///
/// 定义审计日志的持久化接口。使用 boxed future 模式以支持 `dyn AuditLogger`。
pub trait AuditLogger: Send + Sync {
    /// 记录审计日志
    fn log<'a>(
        &'a self,
        entry: AuditEntry,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>>;

    /// 查询审计日志
    fn list_entries<'a>(
        &'a self,
        limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AuditEntry>>> + Send + 'a>>;

    /// 清空审计日志
    fn clear<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>>;

    /// 轮转审计日志：删除超过指定天数的旧日志
    fn purge_old_entries<'a>(
        &'a self,
        max_age_days: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize>> + Send + 'a>>;

    /// 查询审计日志总数
    fn count_entries<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize>> + Send + 'a>>;
}

/// 简单的内存审计日志（用于测试）
pub struct InMemoryAuditLogger {
    entries: tokio::sync::Mutex<Vec<AuditEntry>>,
}

impl InMemoryAuditLogger {
    /// 创建新的内存审计日志
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Default for InMemoryAuditLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLogger for InMemoryAuditLogger {
    fn log<'a>(
        &'a self,
        entry: AuditEntry,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.entries.lock().await.push(entry);
            Ok(())
        })
    }

    fn list_entries<'a>(
        &'a self,
        limit: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<AuditEntry>>> + Send + 'a>>
    {
        Box::pin(async move {
            let guard = self.entries.lock().await;
            Ok(guard.iter().rev().take(limit).cloned().collect())
        })
    }

    fn clear<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.entries.lock().await.clear();
            Ok(())
        })
    }

    fn purge_old_entries<'a>(
        &'a self,
        max_age_days: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize>> + Send + 'a>> {
        Box::pin(async move {
            let now = chrono::Utc::now().timestamp();
            let cutoff = now - (max_age_days as i64) * 86_400;
            let mut guard = self.entries.lock().await;
            let before = guard.len();
            guard.retain(|e| e.timestamp >= cutoff);
            Ok(before - guard.len())
        })
    }

    fn count_entries<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<usize>> + Send + 'a>> {
        Box::pin(async move { Ok(self.entries.lock().await.len()) })
    }
}

/// 隐私管理器
///
/// 组合 PII 脱敏器和审计日志，提供统一的隐私保护接口。
pub struct PrivacyManager {
    /// PII 脱敏器
    redactor: PiiRedactor,
    /// 审计日志（可选）
    audit_logger: Option<Arc<dyn AuditLogger>>,
}

impl PrivacyManager {
    /// 创建新的隐私管理器
    #[must_use]
    pub fn new(redactor: PiiRedactor) -> Self {
        Self {
            redactor,
            audit_logger: None,
        }
    }

    /// 设置审计日志
    #[must_use]
    pub fn with_audit_logger(mut self, logger: Arc<dyn AuditLogger>) -> Self {
        self.audit_logger = Some(logger);
        self
    }

    /// 处理用户输入：脱敏 PII 并记录审计日志
    ///
    /// # 参数
    /// - `text`: 用户输入文本
    /// - `action`: 操作类型（如 "search", "chat"）
    ///
    /// # 返回
    /// 脱敏后的文本
    pub async fn process_input(&self, text: &str, action: &str) -> Result<String> {
        let detections = self.redactor.detect(text);
        let redacted = self.redactor.redact_text(text);

        if let Some(logger) = &self.audit_logger {
            let entry = AuditEntry {
                id: uuid::Uuid::new_v4().to_string(),
                action: action.to_string(),
                details: format!(
                    "{{\"pii_detected\": {}, \"pii_types\": {:?}}}",
                    detections.len(),
                    detections.iter().map(|d| &d.pii_type).collect::<Vec<_>>()
                ),
                pii_count: detections.len(),
                timestamp: chrono::Utc::now().timestamp(),
                prev_hash: None,
                curr_hash: None,
            };
            logger.log(entry).await?;
        }

        if !detections.is_empty() {
            debug!(
                pii_count = detections.len(),
                action = %action,
                "PII 检测完成，已脱敏"
            );
        }

        Ok(redacted)
    }

    /// 获取 PII 脱敏器引用
    #[must_use]
    pub fn redactor(&self) -> &PiiRedactor {
        &self.redactor
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    // ============ 邮箱检测 ============

    #[test]
    fn test_detect_email_finds_and_redacts_correctly() {
        let redactor = PiiRedactor::new();
        let text = "Contact me at john@example.com for details.";
        let detections = redactor.detect(text);

        assert_eq!(detections.len(), 1, "应检测到 1 个邮箱");
        assert_eq!(detections[0].pii_type, PiiType::Email);
        assert_eq!(detections[0].matched, "john@example.com");
        assert_eq!(detections[0].redacted, "j***@example.com");
    }

    #[test]
    fn test_detect_multiple_emails_in_one_text() {
        let redactor = PiiRedactor::new();
        let text = "Emails: alice@test.com and bob@example.org";
        let detections = redactor.detect(text);
        let emails: Vec<_> = detections
            .iter()
            .filter(|d| d.pii_type == PiiType::Email)
            .collect();
        assert_eq!(emails.len(), 2);
    }

    #[test]
    fn test_redact_email_short_local_part() {
        assert_eq!(PiiRedactor::redact_email("a@b.com"), "***@b.com");
    }

    // ============ 手机号检测 ============

    #[test]
    fn test_detect_chinese_phone_number() {
        let redactor = PiiRedactor::new();
        let text = "My phone is 13812345678.";
        let detections = redactor.detect(text);
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].pii_type, PiiType::Phone);
        assert_eq!(detections[0].matched, "13812345678");
        assert_eq!(detections[0].redacted, "138****5678");
    }

    // ============ 身份证检测 ============

    #[test]
    fn test_detect_id_card_18_digits() {
        let redactor = PiiRedactor::new();
        let text = "ID: 110101199001011234";
        let detections = redactor.detect(text);
        assert!(detections.iter().any(|d| d.pii_type == PiiType::IdCard));
    }

    // ============ 银行卡检测 ============

    #[test]
    fn test_detect_bank_card_passes_luhn_check() {
        let redactor = PiiRedactor::new();
        let text = "Card: 4111111111111111";
        let detections = redactor.detect(text);
        assert!(detections.iter().any(|d| d.pii_type == PiiType::BankCard));
    }

    #[test]
    fn test_detect_bank_card_fails_luhn_check() {
        let redactor = PiiRedactor::new();
        let text = "Number: 1234567890123456";
        let detections = redactor.detect(text);
        assert!(!detections.iter().any(|d| d.pii_type == PiiType::BankCard));
    }

    // ============ IP 地址检测 ============

    #[test]
    fn test_detect_ip_address() {
        let redactor = PiiRedactor::new();
        let text = "Server at 192.168.1.100 is down.";
        let detections = redactor.detect(text);
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].pii_type, PiiType::IpAddress);
        assert_eq!(detections[0].redacted, "192.168.**.**");
    }

    // ============ 国际手机号检测 ============

    #[test]
    fn test_detect_international_phone() {
        let redactor = PiiRedactor::new();
        let text = "Call me at +8613812345678.";
        let detections = redactor.detect(text);
        assert!(
            detections
                .iter()
                .any(|d| d.pii_type == PiiType::InternationalPhone)
        );
    }

    // ============ SSN 检测 ============

    #[test]
    fn test_detect_valid_ssn() {
        let redactor = PiiRedactor::new();
        let text = "SSN: 123-45-6789";
        let detections = redactor.detect(text);
        assert!(detections.iter().any(|d| d.pii_type == PiiType::Ssn));
    }

    #[test]
    fn test_detect_ssn_excludes_area_000() {
        let redactor = PiiRedactor::new();
        let text = "SSN: 000-12-3456";
        let detections = redactor.detect(text);
        assert!(!detections.iter().any(|d| d.pii_type == PiiType::Ssn));
    }

    // ============ 护照号检测 ============

    #[test]
    fn test_detect_passport_e_prefix() {
        let redactor = PiiRedactor::new();
        let text = "Passport: E12345678";
        let detections = redactor.detect(text);
        assert!(detections.iter().any(|d| d.pii_type == PiiType::Passport));
    }

    // ============ 综合脱敏 ============

    #[test]
    fn test_redact_text_with_multiple_pii_types() {
        let redactor = PiiRedactor::new();
        let text = "Email: john@test.com, Phone: 13812345678";
        let redacted = redactor.redact_text(text);

        assert!(redacted.contains("j***@test.com"));
        assert!(redacted.contains("138****5678"));
        assert!(!redacted.contains("john@test.com"));
        assert!(!redacted.contains("13812345678"));
    }

    #[test]
    fn test_redact_text_no_pii_returns_original() {
        let redactor = PiiRedactor::new();
        let text = "This is a normal text without PII.";
        let redacted = redactor.redact_text(text);
        assert_eq!(redacted, text);
    }

    #[test]
    fn test_disabled_redactor_detects_nothing() {
        let redactor = PiiRedactor::disabled();
        let text = "Email: john@test.com";
        let detections = redactor.detect(text);
        assert!(detections.is_empty());
    }

    // ============ PrivacyManager ============

    #[tokio::test]
    async fn test_privacy_manager_redacts_and_logs_audit() {
        let redactor = PiiRedactor::new();
        let logger = Arc::new(InMemoryAuditLogger::new());
        let manager = PrivacyManager::new(redactor).with_audit_logger(logger.clone());

        let input = "My email is john@test.com and phone is 13812345678";
        let result = manager.process_input(input, "chat").await.unwrap();

        assert!(result.contains("j***@test.com"));
        assert!(result.contains("138****5678"));

        let entries = logger.list_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, "chat");
        assert!(entries[0].pii_count > 0);
    }

    #[tokio::test]
    async fn test_privacy_manager_no_pii_still_logs() {
        let redactor = PiiRedactor::new();
        let logger = Arc::new(InMemoryAuditLogger::new());
        let manager = PrivacyManager::new(redactor).with_audit_logger(logger.clone());

        let input = "What is Rust programming language?";
        let result = manager.process_input(input, "search").await.unwrap();

        assert_eq!(result, input);

        let entries = logger.list_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].pii_count, 0);
    }

    #[tokio::test]
    async fn test_privacy_manager_without_audit_logger_works() {
        let redactor = PiiRedactor::new();
        let manager = PrivacyManager::new(redactor);

        let input = "Email: john@test.com";
        let result = manager.process_input(input, "chat").await.unwrap();
        assert!(result.contains("j***@test.com"));
    }

    // ============ InMemoryAuditLogger ============

    #[tokio::test]
    async fn test_audit_clear_removes_all_entries() {
        let logger = InMemoryAuditLogger::new();

        for i in 0..3 {
            logger
                .log(AuditEntry {
                    id: format!("e{i}"),
                    action: "test".to_string(),
                    details: "{}".to_string(),
                    pii_count: 0,
                    timestamp: chrono::Utc::now().timestamp(),
                    prev_hash: None,
                    curr_hash: None,
                })
                .await
                .unwrap();
        }

        assert_eq!(logger.count_entries().await.unwrap(), 3);

        logger.clear().await.unwrap();
        assert_eq!(logger.count_entries().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_audit_purge_old_entries_deletes_expired() {
        let logger = InMemoryAuditLogger::new();
        let now = chrono::Utc::now().timestamp();

        logger
            .log(AuditEntry {
                id: "old".to_string(),
                action: "search".to_string(),
                details: "{}".to_string(),
                pii_count: 0,
                timestamp: now - 10 * 86_400,
                prev_hash: None,
                curr_hash: None,
            })
            .await
            .unwrap();
        logger
            .log(AuditEntry {
                id: "new".to_string(),
                action: "import".to_string(),
                details: "{}".to_string(),
                pii_count: 0,
                timestamp: now,
                prev_hash: None,
                curr_hash: None,
            })
            .await
            .unwrap();

        let deleted = logger.purge_old_entries(7).await.unwrap();
        assert_eq!(deleted, 1);

        let entries = logger.list_entries(10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries.iter().all(|e| e.id != "old"));
    }
}
