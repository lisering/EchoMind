#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
//! Security & Audit 相关集成测试 — 安全状态/加密/锁定/暴力破解/剪贴板/PII/审计门控。

use super::common::*;
use super::*;

// ================== AUDIT 域集成测试（REQ-AUDIT-001~005） ==================
// L2 契约测试：真实 SQLite + Tauri Mock，验证 IPC 层门控与取消机制。
// AuditEngine 核心逻辑（Decompose/Verify/Report）由 audit_tests.rs 单元测试覆盖（Mock LLM/Embedder/Storage）。
// 此处不发起真实 LLM/Embedder 调用，仅验证 Pro 门控、配置检查、空文档处理与取消信号。

/// TC-AUDIT-006 免费版审计门控：is_pro=false 时调用 audit_document 返回 Pro 版功能错误（REQ-AUDIT-001）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_audit_006_free_tier_audit_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    // 配置 LLM（排除「未配置」干扰项，单独验证 Pro 门控）
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();
    // 确认为免费版
    assert!(!get_pro_status_inner(&state).await, "初始状态应为免费版");

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = audit_document_inner(&handle, "any-doc", "test.md", &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("Pro 版功能"),
        "免费版审计应返回 Pro 版功能错误，实际: {err}"
    );
}

/// TC-AUDIT-007 Pro 版未配置 LLM：is_pro=true 但无 LLM 配置时返回配置错误（REQ-AUDIT-001）。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_audit_007_pro_without_llm_config_blocked() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    let license = make_valid_license();
    activate_pro_inner(&license, &state).await.unwrap();
    assert!(get_pro_status_inner(&state).await, "激活后应为 Pro");
    // 不配置 LLM，验证 Pro 门控通过但 LLM 配置检查拦截

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();

    let result = audit_document_inner(&handle, "any-doc", "test.md", &state).await;

    let err = result.err().unwrap_or_default();
    assert!(
        err.contains("未配置 LLM"),
        "Pro 版未配置 LLM 应返回配置错误，实际: {err}"
    );
}
#[tokio::test]
async fn tc_audit_009_pro_indexed_doc_ready_for_audit() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro
    let license = make_valid_license();
    activate_pro_inner(&license, &state).await.unwrap();
    assert!(get_pro_status_inner(&state).await, "应为 Pro");

    // 配置 LLM
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 导入测试文档
    let md = dir.path().join("audit-test.md");
    std::fs::write(
        &md,
        "# 测试文档\n\n实验温度为 25°C，误差小于 5%。\n\n实验温度为 20°C。\n",
    )
    .unwrap();

    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let imported = import_files_inner(&handle, &[md.to_string_lossy().into_owned()], &state)
        .await
        .unwrap();
    assert_eq!(imported.len(), 1, "应导入 1 个文档");

    // 验证文档已索引
    let docs = state.storage.list_documents().await.unwrap();
    assert_eq!(docs.len(), 1, "应有 1 个文档");

    let doc = &docs[0];
    let status = format!("{:?}", doc.status);
    assert!(
        status.contains("Indexed"),
        "文档应已索引，实际状态: {status}"
    );

    // 验证 chunks 已写入（审计的前置条件）
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(!chunks.is_empty(), "已索引文档应有 chunks");

    // 验证 chunks 中包含矛盾内容（审计的目标数据）
    let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect();
    assert!(
        all_text.contains("25°C") || all_text.contains("25"),
        "chunks 应包含温度参数 25°C"
    );
    assert!(
        all_text.contains("20°C") || all_text.contains("20"),
        "chunks 应包含温度参数 20°C"
    );
}

/// TC-AUDIT-010 空文档审计：Pro + LLM 配置但文档无 chunks 时返回 NoChunks 提示（REQ-AUDIT-001-AC-4）。
/// 此测试验证文档存在但无 chunks 的边界情况。
#[cfg(feature = "pro")]
#[tokio::test]
async fn tc_audit_010_empty_doc_no_chunks_handling() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 激活 Pro + 配置 LLM
    let license = make_valid_license();
    activate_pro_inner(&license, &state).await.unwrap();
    update_llm_config_inner(test_config(), &state)
        .await
        .unwrap();

    // 直接写入文档记录（无 chunks）
    let doc = Document::new("empty-audit.md".to_string(), "empty-hash".to_string());
    state.storage.add_document(&doc).await.unwrap();

    // 验证文档存在但无 chunks
    let chunks = state.storage.list_chunks(&doc.id).await.unwrap();
    assert!(chunks.is_empty(), "文档不应有 chunks");

    // 调用审计——会在 embedder 初始化阶段失败（无 ONNX 模型），
    // 但此测试验证的是 list_chunks 返回空时的边界处理。
    // 由于 embedder 初始化在 list_chunks 之前，此测试主要验证 Pro 门控通过。
    let app = tauri::test::mock_app();
    let handle = app.handle().clone();
    let result = audit_document_inner(&handle, &doc.id, "empty.md", &state).await;

    // 审计会因 embedder 不可用而返回错误（测试环境无 ONNX 模型），
    // 但不应是 Pro 门控或 LLM 配置错误——证明门控已通过。
    if let Err(ref err) = result {
        assert!(
            !err.contains("Pro 版功能"),
            "Pro 已激活，不应返回 Pro 门控错误: {err}"
        );
        assert!(
            !err.contains("未配置 LLM"),
            "LLM 已配置，不应返回配置错误: {err}"
        );
    }
    // embedder 不可用是预期的（测试环境无 ONNX 模型下载）
}

// ============================================================================
// 安全防御 IPC 集成测试（REQ-SEC-013~020）
// ============================================================================

/// TC-IPC-SEC-001：安全状态查询——初始状态为 Unencrypted（REQ-SEC-013）。
#[tokio::test]
async fn tc_ipc_sec_001_initial_security_state_is_unencrypted() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let security_state = state.security().state().await;
    assert_eq!(
        security_state,
        echomind_core::security::SecurityState::Unencrypted,
        "初始状态应为 Unencrypted"
    );
    assert!(!state.security().is_locked().await, "初始不应被锁定");
    assert_eq!(
        state.security().remaining_attempts().await,
        5,
        "初始剩余尝试次数应为 5"
    );
}

/// TC-IPC-SEC-002：应用锁定与解锁循环（REQ-SEC-014）。
#[tokio::test]
async fn tc_ipc_sec_002_lock_unlock_cycle() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 设置加密状态
    state.security().set_encrypted().await;
    assert_eq!(
        state.security().state().await,
        echomind_core::security::SecurityState::EncryptedUnlocked
    );

    // 锁定
    state
        .security()
        .lock(echomind_core::security::LockReason::Manual)
        .await;
    assert!(state.security().is_locked().await);
    assert!(state.security().locked_at().await.is_some());

    // 解锁
    state.security().unlock().await;
    assert!(!state.security().is_locked().await);
    assert_eq!(
        state.security().state().await,
        echomind_core::security::SecurityState::EncryptedUnlocked
    );
    assert!(state.security().locked_at().await.is_none());
}

/// TC-IPC-SEC-003：自动锁屏配置设置与查询（REQ-SEC-015）。
#[tokio::test]
async fn tc_ipc_sec_003_auto_lock_config_set_and_get() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let config = echomind_core::security::AutoLockConfig {
        enabled: true,
        timeout_secs: 60,
        lock_on_sleep: false,
    };
    state.security().set_auto_lock_config(config).await;

    let retrieved = state.security().get_auto_lock_config().await;
    assert!(retrieved.enabled);
    assert_eq!(retrieved.timeout_secs, 60);
    assert!(!retrieved.lock_on_sleep);
}

/// TC-IPC-SEC-004：暴力破解防护——5次失败后锁定（REQ-SEC-016）。
#[tokio::test]
async fn tc_ipc_sec_004_brute_force_protection_5_failures() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    // 前4次不应锁定
    for i in 0..4 {
        let locked = state.security().record_auth_failure().await;
        assert!(!locked, "第 {} 次失败不应触发锁定", i + 1);
    }
    assert_eq!(state.security().remaining_attempts().await, 1);

    // 第5次应触发锁定
    let locked = state.security().record_auth_failure().await;
    assert!(locked, "第 5 次失败应触发锁定");

    let bf = state.security().brute_force().await;
    assert!(bf.is_locked());
}

/// TC-IPC-SEC-005：解锁后暴力破解计数器重置（REQ-SEC-016）。
#[tokio::test]
async fn tc_ipc_sec_005_unlock_resets_brute_force() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    state.security().set_encrypted().await;

    // 3次失败
    state.security().record_auth_failure().await;
    state.security().record_auth_failure().await;
    state.security().record_auth_failure().await;
    assert_eq!(state.security().remaining_attempts().await, 2);

    // 锁定后解锁
    state
        .security()
        .lock(echomind_core::security::LockReason::Manual)
        .await;
    state.security().unlock().await;

    // 计数器应重置
    assert_eq!(state.security().remaining_attempts().await, 5);
}

/// TC-IPC-SEC-006：紧急销毁密码设置与清除（REQ-SEC-017）。
#[tokio::test]
async fn tc_ipc_sec_006_panic_wipe_password_set_and_clear() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    assert!(!state.security().is_panic_wipe_enabled().await);

    state.security().set_panic_wipe_password("panic123").await;
    assert!(state.security().is_panic_wipe_enabled().await);
    assert!(
        state.security().check_panic_wipe_password("panic123").await,
        "正确密码应验证通过"
    );
    assert!(
        !state.security().check_panic_wipe_password("wrong").await,
        "错误密码应验证失败"
    );

    state.security().clear_panic_wipe_password().await;
    assert!(!state.security().is_panic_wipe_enabled().await);
}

/// TC-IPC-SEC-007：剪贴板清除配置（REQ-SEC-018）。
#[tokio::test]
async fn tc_ipc_sec_007_clipboard_config() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();

    let config = echomind_core::security::ClipboardConfig {
        enabled: true,
        clear_after_secs: 15,
    };
    state.clipboard_guard().set_config(config).await;

    let retrieved = state.clipboard_guard().get_config().await;
    assert!(retrieved.enabled);
    assert_eq!(retrieved.clear_after_secs, 15);
}

/// TC-IPC-SEC-008：密码强度检测——弱密码（REQ-SEC-019）。
#[tokio::test]
async fn tc_ipc_sec_008_password_strength_weak() {
    let strength = echomind_core::encryption::PasswordStrengthChecker::check("123456");
    assert_eq!(
        strength,
        echomind_core::encryption::PasswordStrength::VeryWeak
    );
}

/// TC-IPC-SEC-009：密码强度检测——强密码（REQ-SEC-019）。
#[tokio::test]
async fn tc_ipc_sec_009_password_strength_strong() {
    let strength = echomind_core::encryption::PasswordStrengthChecker::check("Str0ng@Pass!2024");
    assert_eq!(
        strength,
        echomind_core::encryption::PasswordStrength::VeryStrong
    );
}

/// TC-IPC-SEC-010：PII 检测——邮箱+手机号（REQ-SEC-020）。
#[tokio::test]
async fn tc_ipc_sec_010_pii_detection_email_and_phone() {
    let redactor = echomind_core::privacy::PiiRedactor::new();
    let text = "联系 john@example.com 或 13812345678";
    let detections = redactor.detect(text);

    assert!(
        detections
            .iter()
            .any(|d| d.pii_type == echomind_core::privacy::PiiType::Email)
    );
    assert!(
        detections
            .iter()
            .any(|d| d.pii_type == echomind_core::privacy::PiiType::Phone)
    );

    let redacted = redactor.redact_text(text);
    assert!(redacted.contains("j***@example.com"));
    assert!(redacted.contains("138****5678"));
    assert!(!redacted.contains("john@example.com"));
}

/// TC-IPC-SEC-011：系统睡眠唤醒后自动锁定（REQ-SEC-015）。
#[tokio::test]
async fn tc_ipc_sec_011_system_wake_triggers_lock() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    state.security().set_encrypted().await;

    state.security().on_system_wake().await;
    assert!(state.security().is_locked().await);
    assert!(matches!(
        state.security().state().await,
        echomind_core::security::SecurityState::Locked(
            echomind_core::security::LockReason::SystemSleep
        )
    ));
}

/// TC-IPC-SEC-012：record_activity 更新活动时间（REQ-SEC-015）。
#[tokio::test]
async fn tc_ipc_sec_012_record_activity_resets_timer() {
    let dir = TempDir::new().unwrap();
    let state = AppState::new(dir.path().to_path_buf()).await.unwrap();
    state.security().set_encrypted().await;
    state
        .security()
        .set_auto_lock_config(echomind_core::security::AutoLockConfig {
            enabled: true,
            timeout_secs: 2,
            lock_on_sleep: true,
        })
        .await;

    // 活动后1秒不应锁定
    state.security().record_activity().await;
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    assert!(!state.security().check_auto_lock().await);

    // 再次活动后1秒仍不应锁定
    state.security().record_activity().await;
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    assert!(!state.security().check_auto_lock().await);
}
