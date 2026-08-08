//! security 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 获取安全状态。
#[tauri::command]
pub async fn get_security_status(state: State<'_, AppState>) -> Result<SecurityStatus, String> {
    let sec = state.security();
    let current_state = sec.state().await;
    let is_locked = sec.is_locked().await;
    let remaining_attempts = sec.remaining_attempts().await;
    let remaining_lock_seconds = sec.remaining_lock_seconds().await;

    Ok(SecurityStatus {
        state: current_state.icon_id().to_string(),
        color: current_state.color().to_string(),
        is_locked,
        remaining_attempts,
        remaining_lock_seconds,
    })
}

/// 设置自动锁屏配置。
#[tauri::command]
pub async fn set_auto_lock_config(
    enabled: bool,
    timeout_secs: u64,
    lock_on_sleep: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .security()
        .set_auto_lock_config(echomind_core::security::AutoLockConfig {
            enabled,
            timeout_secs,
            lock_on_sleep,
        })
        .await;
    Ok(())
}

/// 手动锁定应用。
#[tauri::command]
pub async fn lock_app(state: State<'_, AppState>) -> Result<(), String> {
    state
        .security()
        .lock(echomind_core::security::LockReason::Manual)
        .await;
    Ok(())
}

/// 解锁应用（密码验证通过后调用）。
#[tauri::command]
pub async fn unlock_app(state: State<'_, AppState>) -> Result<(), String> {
    state.security().unlock().await;
    Ok(())
}

/// 记录用户活动（重置自动锁屏计时器）。
#[tauri::command]
pub async fn record_activity(state: State<'_, AppState>) -> Result<(), String> {
    state.security().record_activity().await;
    Ok(())
}

/// 检测文本中的 PII 并返回脱敏结果。
#[tauri::command]
pub async fn detect_pii(text: String) -> Result<serde_json::Value, String> {
    let redactor = echomind_core::privacy::PiiRedactor::new();
    let detections = redactor.detect(&text);
    let redacted = redactor.redact_text(&text);
    Ok(serde_json::json!({
        "detections": detections,
        "redacted": redacted,
        "pii_count": detections.len(),
    }))
}

/// 设置紧急销毁密码。
#[tauri::command]
pub async fn set_panic_wipe_password(
    password: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.security().set_panic_wipe_password(&password).await;
    Ok(())
}

/// 清除紧急销毁密码。
#[tauri::command]
pub async fn clear_panic_wipe_password(state: State<'_, AppState>) -> Result<(), String> {
    state.security().clear_panic_wipe_password().await;
    Ok(())
}

/// 查询紧急销毁是否已启用。
#[tauri::command]
pub async fn is_panic_wipe_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.security().is_panic_wipe_enabled().await)
}

/// 设置剪贴板自动清除配置。
#[tauri::command]
pub async fn set_clipboard_config(
    enabled: bool,
    clear_after_secs: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .clipboard_guard()
        .set_config(echomind_core::security::ClipboardConfig {
            enabled,
            clear_after_secs,
        })
        .await;
    Ok(())
}

/// 查询审计日志。
#[tauri::command]
pub async fn get_audit_logs(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_core::privacy::AuditEntry>, String> {
    use echomind_core::privacy::AuditLogger;
    let entries = state
        .storage
        .list_entries(limit.unwrap_or(100))
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(entries)
}

/// 清空审计日志。
#[tauri::command]
pub async fn clear_audit_logs(state: State<'_, AppState>) -> Result<(), String> {
    use echomind_core::privacy::AuditLogger;
    state.storage.clear().await.map_err(|e| format!("{e:#}"))
}

/// 检查密码强度。
#[tauri::command]
pub async fn check_password_strength(password: String) -> Result<serde_json::Value, String> {
    let strength = echomind_core::encryption::PasswordStrengthChecker::check(&password);
    let suggestions = echomind_core::encryption::PasswordStrengthChecker::suggestions(&password);
    Ok(serde_json::json!({
        "level": strength.i18n_key(),
        "percentage": strength.percentage(),
        "color": strength.color(),
        "suggestions": suggestions,
    }))
}
