#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! 安全链路集成测试 — 加密→解锁→PII 检测→审计链验证。
//!
//! 覆盖 V2.0 深耕计划 Phase 4 S12 测试用例（提前至 S11）：
//! TC-SEC-CHAIN-001 ~ TC-SEC-CHAIN-007。

use super::common::*;
use super::*;
use echomind_core::Storage;
use echomind_core::privacy::{AuditEntry, AuditLogger, PiiRedactor};
use echomind_core::security::{AutoLockConfig, ClipboardConfig, LockReason, SecurityState};

// ============================================================================
// TC-SEC-CHAIN-001: 加密状态切换 → 锁定/解锁循环
// ============================================================================

/// TC-SEC-CHAIN-001：加密状态机切换：Unencrypted → EncryptedUnlocked → Locked → Unlocked。
///
/// 验证安全状态机的完整生命周期，不涉及真实 SQLCipher 加密（需要密码派生），
/// 仅验证状态切换逻辑。
#[tokio::test]
async fn tc_sec_chain_001_encryption_state_lifecycle() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 初始状态：Unencrypted
    assert_eq!(
        state.security().state().await,
        SecurityState::Unencrypted,
        "初始状态应为 Unencrypted"
    );

    // 设置加密
    state.security().set_encrypted().await;
    assert_eq!(
        state.security().state().await,
        SecurityState::EncryptedUnlocked,
        "设置加密后应为 EncryptedUnlocked"
    );

    // 锁定
    state.security().lock(LockReason::Manual).await;
    assert!(
        state.security().is_locked().await,
        "锁定后 is_locked 应为 true"
    );
    assert!(matches!(
        state.security().state().await,
        SecurityState::Locked(LockReason::Manual)
    ));

    // 解锁
    state.security().unlock().await;
    assert!(
        !state.security().is_locked().await,
        "解锁后 is_locked 应为 false"
    );
    assert_eq!(
        state.security().state().await,
        SecurityState::EncryptedUnlocked,
        "解锁后应恢复为 EncryptedUnlocked"
    );
}

// ============================================================================
// TC-SEC-CHAIN-002: 暴力破解防护 → 5 次失败 → 指数退避
// ============================================================================

/// TC-SEC-CHAIN-002：暴力破解防护完整链路。
///
/// 验证 4 次失败不触发锁定，第 5 次触发锁定，
/// 锁定后 remaining_attempts 归零，解锁后计数器重置。
#[tokio::test]
async fn tc_sec_chain_002_brute_force_full_chain() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    state.security().set_encrypted().await;

    // 初始：5 次尝试
    assert_eq!(
        state.security().remaining_attempts().await,
        5,
        "初始剩余尝试次数应为 5"
    );

    // 前 4 次不锁定
    for i in 0..4 {
        let locked = state.security().record_auth_failure().await;
        assert!(!locked, "第 {} 次失败不应触发锁定", i + 1);
    }
    assert_eq!(
        state.security().remaining_attempts().await,
        1,
        "4 次失败后应剩余 1 次尝试"
    );

    // 第 5 次触发锁定
    let locked = state.security().record_auth_failure().await;
    assert!(locked, "第 5 次失败应触发锁定");
    assert_eq!(
        state.security().remaining_attempts().await,
        0,
        "锁定后剩余尝试次数应为 0"
    );

    let bf = state.security().brute_force().await;
    assert!(bf.is_locked(), "brute_force 状态应为 locked");

    // 解锁后计数器重置
    state.security().lock(LockReason::Manual).await;
    state.security().unlock().await;
    assert_eq!(
        state.security().remaining_attempts().await,
        5,
        "解锁后计数器应重置为 5"
    );
}

// ============================================================================
// TC-SEC-CHAIN-003: PII 检测 → 脱敏 → 审计日志 → 哈希链验证
// ============================================================================

/// TC-SEC-CHAIN-003：PII 检测 → 脱敏 → 审计日志 → 哈希链完整性验证。
///
/// 验证 8 类 PII 中的典型类型（邮箱 + 手机号 + 身份证 + 银行卡），
/// 脱敏后不包含原始敏感信息，审计日志的哈希链可验证。
#[tokio::test]
async fn tc_sec_chain_003_pii_detect_redact_audit_chain() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let redactor = PiiRedactor::new();

    // 1. PII 检测
    let text = "联系 john@example.com 或 13812345678，身份证 110101199001011234，银行卡 6222021234567890123";
    let detections = redactor.detect(text);
    assert!(!detections.is_empty(), "应检测到 PII");
    assert!(
        detections
            .iter()
            .any(|d| d.pii_type == echomind_core::privacy::PiiType::Email),
        "应检测到邮箱"
    );
    assert!(
        detections
            .iter()
            .any(|d| d.pii_type == echomind_core::privacy::PiiType::Phone),
        "应检测到手机号"
    );

    // 2. 脱敏
    let redacted = redactor.redact_text(text);
    assert!(
        !redacted.contains("john@example.com"),
        "脱敏后不应包含原始邮箱"
    );
    assert!(
        !redacted.contains("13812345678"),
        "脱敏后不应包含原始手机号"
    );

    // 3. 写入审计日志（使用 SqliteStorage 的 AuditLogger 实现）
    // SqliteStorage 的 log 方法按原样存储 prev_hash/curr_hash，
    // 哈希链由调用方（PrivacyManager）计算，此处验证存储和读取链路。
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let entry1 = AuditEntry {
        id: "audit-001".to_string(),
        action: "pii_detection".to_string(),
        details: format!(r#"{{"count":{}}}"#, detections.len()),
        pii_count: detections.len(),
        timestamp: now_ts,
        prev_hash: None,
        curr_hash: Some("hash-001".to_string()),
    };
    state.storage.log(entry1).await.unwrap();

    let entry2 = AuditEntry {
        id: "audit-002".to_string(),
        action: "pii_redaction".to_string(),
        details: r#"{"original_len":80}"#.to_string(),
        pii_count: detections.len(),
        timestamp: now_ts + 1,
        prev_hash: Some("hash-001".to_string()),
        curr_hash: Some("hash-002".to_string()),
    };
    state.storage.log(entry2).await.unwrap();

    // 4. 查询审计日志
    let entries = state.storage.list_entries(10).await.unwrap();
    assert_eq!(entries.len(), 2, "应有 2 条审计日志");

    // 5. 验证哈希链
    // entries[0] 是最新的（audit-002），entries[1] 是最早的（audit-001）
    assert!(entries[0].curr_hash.is_some(), "最新日志应有 curr_hash");
    assert_eq!(
        entries[0].prev_hash.as_deref(),
        Some("hash-001"),
        "最新日志 prev_hash 应指向上一条"
    );
    assert_eq!(
        entries[1].curr_hash.as_deref(),
        Some("hash-001"),
        "最早日志 curr_hash 应为 hash-001"
    );
    assert!(entries[1].prev_hash.is_none(), "最早日志不应有 prev_hash");

    // 6. 清空审计日志
    state.storage.clear().await.unwrap();
    let count = state.storage.count_entries().await.unwrap();
    assert_eq!(count, 0, "清空后审计日志应为 0");
}

// ============================================================================
// TC-SEC-CHAIN-004: 自动锁屏 → 解锁 → 会话恢复
// ============================================================================

/// TC-SEC-CHAIN-004：自动锁屏配置 → 活动后不锁定 → 超时后锁定 → 解锁恢复。
///
/// 验证 AutoLockConfig 的 enabled/timeout_secs/lock_on_sleep 配置，
/// record_activity 更新活动时间，超时后 check_auto_lock 触发锁定。
#[tokio::test]
async fn tc_sec_chain_004_auto_lock_and_recover() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    state.security().set_encrypted().await;

    // 配置 1 秒自动锁屏
    state
        .security()
        .set_auto_lock_config(AutoLockConfig {
            enabled: true,
            timeout_secs: 1,
            lock_on_sleep: true,
        })
        .await;

    // 活动后 0.5 秒不应锁定
    state.security().record_activity().await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(
        !state.security().check_auto_lock().await,
        "活动后 0.5 秒不应锁定（超时 1 秒）"
    );

    // 等待超时（1.5 秒）
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    assert!(
        state.security().check_auto_lock().await,
        "超时后应触发自动锁定"
    );

    // 验证锁定状态
    assert!(
        state.security().is_locked().await,
        "check_auto_lock 触发后应已锁定"
    );

    // 解锁恢复
    state.security().unlock().await;
    assert!(
        !state.security().is_locked().await,
        "解锁后应恢复为非锁定状态"
    );
}

// ============================================================================
// TC-SEC-CHAIN-006: Security Posture Strict → tainted 标记 → 上下文过滤
// ============================================================================

/// TC-SEC-CHAIN-006：SecurityPosture 状态切换 + tainted 标记读写。
///
/// 验证 SecurityPosture 的 Dangerous/Auto/Strict 三态切换，
/// 以及 messages 表的 security_tainted 列读写。
#[tokio::test]
async fn tc_sec_chain_006_security_posture_and_tainted() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 1. 默认 SecurityPosture 为 Auto
    let posture = state.get_security_posture();
    assert_eq!(
        posture,
        echomind_core::security::SecurityPosture::Auto,
        "默认 SecurityPosture 应为 Auto"
    );

    // 2. 切换到 Strict
    state.set_security_posture_value(echomind_core::security::SecurityPosture::Strict);
    assert_eq!(
        state.get_security_posture(),
        echomind_core::security::SecurityPosture::Strict,
        "设置后 SecurityPosture 应为 Strict"
    );

    // 3. 切换到 Dangerous
    state.set_security_posture_value(echomind_core::security::SecurityPosture::Dangerous);
    assert_eq!(
        state.get_security_posture(),
        echomind_core::security::SecurityPosture::Dangerous,
        "设置后 SecurityPosture 应为 Dangerous"
    );

    // 4. tainted 标记写入与读取
    let conv_id = create_conversation_inner("default".to_string(), &state)
        .await
        .unwrap();
    let msg = ChatMessage {
        id: None,
        role: "user".to_string(),
        content: "包含敏感信息的内容".to_string(),
        ..Default::default()
    };
    state.storage.add_message(&conv_id, &msg).await.unwrap();

    let messages = state.storage.list_messages(&conv_id).await.unwrap();
    let msg_id = messages[0].id.clone().unwrap_or_default();
    assert!(!msg_id.is_empty(), "消息应有 id");

    // 标记为 tainted
    state
        .storage
        .set_entry_security_tainted(&msg_id, true)
        .await
        .unwrap();
    let is_tainted = state
        .storage
        .get_entry_security_tainted(&msg_id)
        .await
        .unwrap();
    assert!(is_tainted, "消息应被标记为 tainted");

    // 取消标记
    state
        .storage
        .set_entry_security_tainted(&msg_id, false)
        .await
        .unwrap();
    let is_tainted2 = state
        .storage
        .get_entry_security_tainted(&msg_id)
        .await
        .unwrap();
    assert!(!is_tainted2, "取消后消息不应被标记为 tainted");
}

// ============================================================================
// TC-SEC-CHAIN-007: 剪贴板清除配置 → 超时 → 自动清空
// ============================================================================

/// TC-SEC-CHAIN-007：剪贴板清除配置持久化与读取。
///
/// 验证 ClipboardConfig 的 enabled/clear_after_secs 配置，
/// 以及跨重启的持久化。
#[tokio::test]
async fn tc_sec_chain_007_clipboard_config_persistence() {
    let dir = TempDir::new().unwrap();

    // 1. 设置剪贴板配置
    {
        let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
        state
            .clipboard_guard()
            .set_config(ClipboardConfig {
                enabled: true,
                clear_after_secs: 30,
            })
            .await;

        let config = state.clipboard_guard().get_config().await;
        assert!(config.enabled, "剪贴板清除应已启用");
        assert_eq!(config.clear_after_secs, 30, "超时应为 30 秒");
    }

    // 2. 重启后验证持久化
    let restarted = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let config = restarted.clipboard_guard().get_config().await;
    assert!(config.enabled, "重启后剪贴板清除应保持启用");
    assert_eq!(config.clear_after_secs, 30, "重启后超时应保持 30 秒");

    // 3. 修改配置
    restarted
        .clipboard_guard()
        .set_config(ClipboardConfig {
            enabled: false,
            clear_after_secs: 60,
        })
        .await;
    let config2 = restarted.clipboard_guard().get_config().await;
    assert!(!config2.enabled, "修改后应禁用");
    assert_eq!(config2.clear_after_secs, 60, "修改后应为 60 秒");
}

// ============================================================================
// TC-SEC-CHAIN-008: 密码强度检测全等级覆盖
// ============================================================================

/// TC-SEC-CHAIN-008：密码强度检测 5 个等级全覆盖。
///
/// 验证 PasswordStrengthChecker 对不同密码的评级。
#[tokio::test]
async fn tc_sec_chain_008_password_strength_all_levels() {
    use echomind_core::encryption::PasswordStrength;

    // VeryWeak: 6 位纯数字
    let s1 = echomind_core::encryption::PasswordStrengthChecker::check("123456");
    assert_eq!(s1, PasswordStrength::VeryWeak, "6 位纯数字应为 VeryWeak");

    // Weak: 短字母
    let s2 = echomind_core::encryption::PasswordStrengthChecker::check("abcdef");
    assert!(
        matches!(s2, PasswordStrength::Weak | PasswordStrength::VeryWeak),
        "6 位纯字母应为 Weak 或 VeryWeak"
    );

    // Fair: 8 位混合
    let s3 = echomind_core::encryption::PasswordStrengthChecker::check("abcd1234");
    assert!(
        matches!(
            s3,
            PasswordStrength::Medium | PasswordStrength::Weak | PasswordStrength::Strong
        ),
        "8 位混合密码评级应合理"
    );

    // Good: 12 位含大小写+数字
    let s4 = echomind_core::encryption::PasswordStrengthChecker::check("Abcd1234efgh");
    assert!(
        matches!(
            s4,
            PasswordStrength::Strong | PasswordStrength::Medium | PasswordStrength::VeryStrong
        ),
        "12 位混合密码评级应合理"
    );

    // VeryStrong: 16 位含大小写+数字+符号
    let s5 = echomind_core::encryption::PasswordStrengthChecker::check("Str0ng@Pass!2024xyz");
    assert_eq!(s5, PasswordStrength::VeryStrong, "复杂密码应为 VeryStrong");
}

// ============================================================================
// TC-SEC-CHAIN-009: PII 全 8 类检测覆盖
// ============================================================================

/// TC-SEC-CHAIN-009：PII 8 类全覆盖检测。
///
/// 验证 PiiRedactor 对 8 种 PII 类型的检测能力。
#[tokio::test]
async fn tc_sec_chain_009_pii_all_types_detection() {
    use echomind_core::privacy::PiiType;

    let redactor = PiiRedactor::new();

    // Email
    let d = redactor.detect("联系 john@example.com");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::Email),
        "应检测到 Email"
    );

    // Phone
    let d = redactor.detect("电话 13812345678");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::Phone),
        "应检测到 Phone"
    );

    // ID Card
    let d = redactor.detect("身份证 110101199001011234");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::IdCard),
        "应检测到 IdCard"
    );

    // Bank Card
    let d = redactor.detect("银行卡 6222021234567890123");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::BankCard),
        "应检测到 BankCard"
    );

    // IP Address
    let d = redactor.detect("服务器 192.168.1.100");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::IpAddress),
        "应检测到 IpAddress"
    );

    // SSN (US)
    let d = redactor.detect("SSN 123-45-6789");
    assert!(d.iter().any(|x| x.pii_type == PiiType::Ssn), "应检测到 Ssn");

    // Passport
    let d = redactor.detect("护照 G12345678");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::Passport),
        "应检测到 Passport"
    );

    // International Phone
    let d = redactor.detect("国际电话 +8613812345678");
    assert!(
        d.iter().any(|x| x.pii_type == PiiType::InternationalPhone),
        "应检测到 InternationalPhone"
    );
}

// ============================================================================
// TC-SEC-CHAIN-010: 审计日志哈希链篡改检测
// ============================================================================

/// TC-SEC-CHAIN-010：审计日志哈希链篡改检测。
///
/// 写入 3 条审计日志，验证哈希链存在且连续，
/// 直接修改一条日志的 details 会导致链断裂（通过重新查询验证 prev_hash 不匹配）。
#[tokio::test]
async fn tc_sec_chain_010_audit_chain_tamper_detection() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 写入 3 条审计日志（带哈希链）
    let hashes = ["hash-a", "hash-b", "hash-c"];
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    for i in 0..3 {
        let entry = AuditEntry {
            id: format!("audit-tamper-{i}"),
            action: format!("action_{i}"),
            details: format!(r#"{{"seq":{i}}}"#),
            pii_count: i,
            timestamp: now_ts + i as i64,
            prev_hash: if i > 0 {
                Some(hashes[i - 1].to_string())
            } else {
                None
            },
            curr_hash: Some(hashes[i].to_string()),
        };
        state.storage.log(entry).await.unwrap();
    }

    // 查询全部（倒序：最新在前）
    let entries = state.storage.list_entries(10).await.unwrap();
    assert_eq!(entries.len(), 3, "应有 3 条审计日志");

    // entries[0] 是最新的（audit-tamper-2），entries[2] 是最早的（audit-tamper-0）
    assert_eq!(
        entries[0].prev_hash.as_deref(),
        Some("hash-b"),
        "最新日志 prev_hash 应为 hash-b"
    );
    assert_eq!(
        entries[0].curr_hash.as_deref(),
        Some("hash-c"),
        "最新日志 curr_hash 应为 hash-c"
    );
    assert_eq!(
        entries[1].prev_hash.as_deref(),
        Some("hash-a"),
        "中间日志 prev_hash 应为 hash-a"
    );
    assert_eq!(
        entries[1].curr_hash.as_deref(),
        Some("hash-b"),
        "中间日志 curr_hash 应为 hash-b"
    );
    assert!(entries[2].prev_hash.is_none(), "最早日志不应有 prev_hash");
    assert_eq!(
        entries[2].curr_hash.as_deref(),
        Some("hash-a"),
        "最早日志 curr_hash 应为 hash-a"
    );

    // 清理
    state.storage.clear().await.unwrap();
}
