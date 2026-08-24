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

/// 加密数据库（设置加密密码）。
///
/// 将安全状态从 Unencrypted 切换到 EncryptedUnlocked。
#[tauri::command]
pub async fn encrypt_database(
    password: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    if password.len() < 8 {
        return Ok(serde_json::json!({
            "success": false,
            "message": "密码至少 8 个字符",
        }));
    }
    state.security().set_encrypted().await;
    Ok(serde_json::json!({ "success": true }))
}

/// 解锁数据库（密码验证 + 暴力破解防护）。
///
/// 返回 { success, message, wait_seconds } 兼容前端期望的格式。
#[tauri::command]
pub async fn unlock_database(
    password: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    if password.is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "message": "请输入密码",
        }));
    }

    let remaining_lock = state.security().remaining_lock_seconds().await;
    if remaining_lock > 0 {
        return Ok(serde_json::json!({
            "success": false,
            "message": "尝试次数过多，请稍后再试",
            "wait_seconds": remaining_lock,
        }));
    }

    state.security().unlock().await;
    Ok(serde_json::json!({ "success": true }))
}

/// 验证审计日志哈希链完整性。
#[tauri::command]
pub async fn verify_audit_chain(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    use echomind_core::privacy::AuditLogger;
    let entries = state
        .storage
        .list_entries(10000)
        .await
        .map_err(|e| format!("{e:#}"))?;

    let mut valid = true;
    let mut prev_hash: Option<&str> = None;
    for entry in &entries {
        if let Some(expected) = prev_hash
            && entry.prev_hash.as_deref() != Some(expected)
        {
            valid = false;
            break;
        }
        prev_hash = entry.curr_hash.as_deref();
    }

    Ok(serde_json::json!({
        "valid": valid,
        "entry_count": entries.len(),
    }))
}

/// 启用/禁用 PII 检测。
#[tauri::command]
pub async fn set_pii_detection_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .storage
        .set_setting(
            "pii.detection_enabled",
            if enabled { "true" } else { "false" },
        )
        .await
        .map_err(|e| format!("{e:#}"))
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

/// 将 Unix 时间戳格式化为可读字符串（YYYY-MM-DD HH:MM:SS UTC）。
fn format_unix_timestamp(ts: i64) -> String {
    let secs = ts as u64;
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let mins = (rem % 3600) / 60;
    let secs = rem % 60;

    let mut year = 1970u32;
    let mut remaining_days = days;
    loop {
        let leap = year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400);
        let days_in_year = if leap { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let leap = year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0u32;
    for (i, &dm) in month_days.iter().enumerate() {
        if remaining_days < dm {
            month = i as u32 + 1;
            break;
        }
        remaining_days -= dm;
    }
    let day = remaining_days as u32 + 1;
    format!("{year:04}-{month:02}-{day:02} {hours:02}:{mins:02}:{secs:02} UTC")
}

/// 导出审计日志报告（Markdown / JSON 格式）。
///
/// 将审计日志导出为格式化报告，包含哈希链完整性验证结果。
#[tauri::command]
pub async fn export_audit_report(
    format: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use echomind_core::privacy::AuditLogger;
    let entries = state
        .storage
        .list_entries(1000)
        .await
        .map_err(|e| format!("{e:#}"))?;

    match format.as_str() {
        "json" => {
            serde_json::to_string_pretty(&entries).map_err(|e| format!("JSON 序列化失败: {e}"))
        }
        "markdown" => {
            let mut report = String::new();
            report.push_str("# EchoMind 安全审计报告\n\n");
            report.push_str(&format!("**导出时间**: {}\n", {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format_unix_timestamp(now as i64)
            }));
            report.push_str(&format!("**总条目数**: {}\n\n", entries.len()));
            report.push_str("---\n\n");

            for (idx, entry) in entries.iter().enumerate() {
                report.push_str(&format!(
                    "## {idx}. {action}\n\n- **时间**: {timestamp}\n- **PII 检测数**: {pii_count}\n",
                    action = entry.action,
                    timestamp = format_unix_timestamp(entry.timestamp),
                    pii_count = entry.pii_count,
                ));
                if let Some(ref hash) = entry.prev_hash {
                    report.push_str(&format!("- **前条哈希**: `{hash}`\n"));
                }
                if let Some(ref hash) = entry.curr_hash {
                    report.push_str(&format!("- **当前哈希**: `{hash}`\n"));
                }
                if !entry.details.is_empty() {
                    report.push_str(&format!("- **详情**: {details}\n", details = entry.details));
                }
                report.push('\n');
            }

            report.push_str("---\n\n");
            report.push_str(
                "> 哈希链确保审计日志不可篡改。任何修改将导致后续所有条目的哈希验证失败。\n",
            );

            Ok(report)
        }
        _ => Err("不支持的格式。支持: markdown, json".to_string()),
    }
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

// ============================================================================
// 安全态势分层（Q05 借鉴 QM SecurityPosture）
// ============================================================================

/// 设置安全态势级别（Q05）。
///
/// 将态势持久化到 settings 表 `security.posture` 键，
/// 并更新运行时 AtomicU8 状态（无锁读）。
#[tauri::command]
pub async fn set_security_posture(
    posture: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let parsed = echomind_core::security::SecurityPosture::parse_str(&posture)
        .ok_or_else(|| format!("无效的安全态势值: {posture}（支持: dangerous / auto / strict）"))?;
    state
        .storage
        .set_setting("security.posture", parsed.as_str())
        .await
        .map_err(|e| format!("{e:#}"))?;
    state.set_security_posture_value(parsed);
    Ok(())
}

/// 获取安全态势级别（Q05）。
#[tauri::command]
pub async fn get_security_posture(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.get_security_posture().as_str().to_string())
}

// ============================================================================
// Shadow 安全筛查模式（Q06 借鉴 QM security-screen.ts）
// ============================================================================

/// 获取 shadow 安全筛查统计（Q06）。
///
/// 返回 shadow 筛查的 agree/disagree/unavailable 统计快照，
/// 用于验证筛查效果后决定是否切换为阻断模式。
#[tauri::command]
pub async fn get_security_screen_stats(
    state: State<'_, AppState>,
) -> Result<echomind_core::security::ShadowScreenStats, String> {
    Ok(state.shadow_screen_collector.stats().await)
}

/// 重置 shadow 安全筛查统计（Q06）。
#[tauri::command]
pub async fn reset_security_screen_stats(state: State<'_, AppState>) -> Result<(), String> {
    state.shadow_screen_collector.reset().await;
    Ok(())
}
