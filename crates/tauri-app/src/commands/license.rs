//! license 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 激活 Pro：验证 License Key 签名，有效则持久化并刷新运行态（REQ-LIC-001）。
#[tauri::command]
pub async fn activate_pro(license_key: String, state: State<'_, AppState>) -> Result<bool, String> {
    activate_pro_inner(&license_key, state.inner()).await
}

/// 激活逻辑（命令与集成测试复用）。
pub async fn activate_pro_inner(license_key: &str, state: &AppState) -> Result<bool, String> {
    match verify_license(license_key) {
        Ok(()) => {
            state
                .storage
                .set_setting("license.is_pro", "true")
                .await
                .map_err(|e| format!("{e:#}"))?;
            *state.is_pro().write().await = true;
            // Q11：Pro 激活后，Local 模式可用
            let mut modes = std::collections::HashSet::new();
            modes.insert(echomind_models::LlmMode::Remote);
            modes.insert(echomind_models::LlmMode::Local);
            state.llm_router.set_available_modes(modes).await;
            Ok(true)
        }
        Err(e) => Err(format!("License 激活失败: {e:#}")),
    }
}

/// 查询当前授权状态（REQ-LIC-002）。
#[tauri::command]
pub async fn get_pro_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(get_pro_status_inner(state.inner()).await)
}

/// 授权状态查询逻辑（命令与集成测试复用）。
pub async fn get_pro_status_inner(state: &AppState) -> bool {
    *state.is_pro().read().await
}

/// 停用 Pro 授权（REQ-LIC-004）：清除 settings 表 license.is_pro，刷新运行态。
/// 停用后回落为免费版规则；已入库文件保留可读，但新导入按免费版拦截（50 文件上限 + PDF 付费门）。
#[tauri::command]
pub async fn deactivate_pro(state: State<'_, AppState>) -> Result<(), String> {
    deactivate_pro_inner(state.inner()).await
}

/// 停用逻辑（命令与集成测试复用）。
/// 将 settings 表中 `license.is_pro` 置为 `"false"`，并将运行态 `is_pro` 置为 `false`。
pub async fn deactivate_pro_inner(state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("license.is_pro", "false")
        .await
        .map_err(|e| format!("{e:#}"))?;
    *state.is_pro().write().await = false;
    // Q11：Pro 停用后，Local 模式不可用
    let mut modes = std::collections::HashSet::new();
    modes.insert(echomind_models::LlmMode::Remote);
    state.llm_router.set_available_modes(modes).await;
    Ok(())
}
