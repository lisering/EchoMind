//! workspace 域 IPC 命令子模块（REQ-WS-001/003 多知识库管理）。
use super::*;

/// 创建知识库（REQ-WS-001 AC-2）。
///
/// 用户输入名称后创建新知识库（工作空间），返回新工作空间 ID。
#[tauri::command]
pub async fn create_workspace(name: String, state: State<'_, AppState>) -> Result<String, String> {
    create_workspace_inner(name, state.inner()).await
}

/// 创建知识库逻辑（命令与集成测试复用）。
pub async fn create_workspace_inner(name: String, state: &AppState) -> Result<String, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("知识库名称不能为空".to_string());
    }
    if name.len() > 100 {
        return Err("知识库名称过长（上限 100 字符）".to_string());
    }
    let ws = echomind_models::Workspace::new(name);
    state
        .storage
        .create_workspace(&ws)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(ws.id)
}

/// 列出全部知识库（REQ-WS-001 AC-1）。
#[tauri::command]
pub async fn list_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::Workspace>, String> {
    list_workspaces_inner(state.inner()).await
}

/// 列出全部知识库逻辑（命令与集成测试复用）。
pub async fn list_workspaces_inner(
    state: &AppState,
) -> Result<Vec<echomind_models::Workspace>, String> {
    state
        .storage
        .list_workspaces()
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换当前知识库（REQ-WS-001 AC-3/AC-5）。
///
/// 持久化当前选择到 settings 表，重启后恢复。
#[tauri::command]
pub async fn switch_workspace(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    switch_workspace_inner(workspace_id, state.inner()).await
}

/// 切换知识库逻辑（命令与集成测试复用）。
pub async fn switch_workspace_inner(workspace_id: String, state: &AppState) -> Result<(), String> {
    // 验证工作空间存在
    let workspaces = state
        .storage
        .list_workspaces()
        .await
        .map_err(|e| format!("{e:#}"))?;
    if !workspaces.iter().any(|ws| ws.id == workspace_id) {
        return Err(format!("工作空间不存在: {workspace_id}"));
    }
    // 持久化当前选择
    state
        .storage
        .set_setting("workspace.current", &workspace_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 获取当前知识库 ID（REQ-WS-001 AC-5 重启恢复）。
#[tauri::command]
pub async fn get_current_workspace(state: State<'_, AppState>) -> Result<String, String> {
    get_current_workspace_inner(state.inner()).await
}

/// 获取当前知识库 ID 逻辑（命令与集成测试复用）。
pub async fn get_current_workspace_inner(state: &AppState) -> Result<String, String> {
    let current = state
        .storage
        .get_setting("workspace.current")
        .await
        .map_err(|e| format!("{e:#}"))?;
    match current {
        Some(id) => {
            // 验证工作空间仍存在
            let workspaces = state
                .storage
                .list_workspaces()
                .await
                .map_err(|e| format!("{e:#}"))?;
            if workspaces.iter().any(|ws| ws.id == id) {
                Ok(id)
            } else {
                // 工作空间已被删除，回退到 default
                Ok(DEFAULT_WORKSPACE.to_string())
            }
        }
        None => Ok(DEFAULT_WORKSPACE.to_string()),
    }
}

/// 重命名知识库（REQ-WS-003 AC-1/AC-2）。
#[tauri::command]
pub async fn rename_workspace(
    id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    rename_workspace_inner(id, name, state.inner()).await
}

/// 重命名知识库逻辑（命令与集成测试复用）。
pub async fn rename_workspace_inner(
    id: String,
    name: String,
    state: &AppState,
) -> Result<(), String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("知识库名称不能为空".to_string());
    }
    if name.len() > 100 {
        return Err("知识库名称过长（上限 100 字符）".to_string());
    }
    state
        .storage
        .rename_workspace(&id, &name)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 删除知识库（REQ-WS-003 AC-4 级联清理）。
///
/// 删除前检查是否为最后一个知识库（AC-5 禁删）。
#[tauri::command]
pub async fn delete_workspace(id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_workspace_inner(id, state.inner()).await
}

/// 删除知识库逻辑（命令与集成测试复用）。
pub async fn delete_workspace_inner(id: String, state: &AppState) -> Result<(), String> {
    // AC-5：不允许删除最后一个知识库
    let workspaces = state
        .storage
        .list_workspaces()
        .await
        .map_err(|e| format!("{e:#}"))?;
    if workspaces.len() <= 1 {
        return Err("至少保留一个知识库".to_string());
    }
    // 验证工作空间存在
    if !workspaces.iter().any(|ws| ws.id == id) {
        return Err(format!("工作空间不存在: {id}"));
    }
    // 不允许删除默认工作空间
    if id == DEFAULT_WORKSPACE {
        return Err("默认知识库不可删除".to_string());
    }
    state
        .storage
        .delete_workspace(&id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 如果删除的是当前工作空间，回退到 default
    let current = state
        .storage
        .get_setting("workspace.current")
        .await
        .map_err(|e| format!("{e:#}"))?;
    if current.as_deref() == Some(&id) {
        state
            .storage
            .set_setting("workspace.current", DEFAULT_WORKSPACE)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    Ok(())
}

/// 获取知识库数据量预览（REQ-WS-003 AC-3 删除确认对话框）。
#[tauri::command]
pub async fn get_workspace_stats(
    id: String,
    state: State<'_, AppState>,
) -> Result<echomind_models::WorkspaceStats, String> {
    get_workspace_stats_inner(id, state.inner()).await
}

/// 获取知识库数据量预览逻辑（命令与集成测试复用）。
pub async fn get_workspace_stats_inner(
    id: String,
    state: &AppState,
) -> Result<echomind_models::WorkspaceStats, String> {
    state
        .storage
        .get_workspace_stats(&id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 获取知识库配额用量（REQ-WS-002 AC-2 配额显示同步更新）。
///
/// 返回 `(当前文档数, 上限)`。Pro 版上限为 0 表示不受限。
#[tauri::command]
pub async fn get_workspace_quota(state: State<'_, AppState>) -> Result<(usize, usize), String> {
    get_workspace_quota_inner(state.inner()).await
}

/// 配额查询逻辑（命令与集成测试复用）。
pub async fn get_workspace_quota_inner(state: &AppState) -> Result<(usize, usize), String> {
    let is_pro = *state.is_pro().read().await;
    let workspace_id = get_current_workspace_inner(state)
        .await
        .unwrap_or_else(|_| DEFAULT_WORKSPACE.to_string());
    let count = state
        .storage
        .count_documents_in_workspace(&workspace_id)
        .await
        .map_err(|e| format!("{e:#}"))?;
    // Pro 版不受配额限制，返回 0 表示无上限
    let limit = if is_pro {
        0
    } else {
        echomind_core::import::FREE_TIER_MAX_FILES
    };
    Ok((count, limit))
}

/// 迁移文档到目标知识库（REQ-WS-004 跨知识库迁移）。
///
/// 仅更新 `documents.workspace_id`，chunks / 向量通过外键关联自动归属。
/// 不重新解析或嵌入。目标库配额不足时拒绝。
#[tauri::command]
pub async fn migrate_document(
    doc_id: String,
    target_workspace_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    migrate_document_inner(doc_id, target_workspace_id, state.inner()).await
}

/// 迁移文档逻辑（命令与集成测试复用）。
pub async fn migrate_document_inner(
    doc_id: String,
    target_workspace_id: String,
    state: &AppState,
) -> Result<(), String> {
    // 验证目标工作空间存在
    let workspaces = state
        .storage
        .list_workspaces()
        .await
        .map_err(|e| format!("{e:#}"))?;
    if !workspaces.iter().any(|ws| ws.id == target_workspace_id) {
        return Err(format!("目标知识库不存在: {target_workspace_id}"));
    }

    // 免费版配额检查（REQ-WS-004 AC-5）
    let is_pro = *state.is_pro().read().await;
    if !is_pro {
        let count = state
            .storage
            .count_documents_in_workspace(&target_workspace_id)
            .await
            .map_err(|e| format!("{e:#}"))?;
        if count >= echomind_core::import::FREE_TIER_MAX_FILES {
            return Err(format!(
                "LIMIT_REACHED: 目标知识库已达免费版上限（{} 个文件）",
                echomind_core::import::FREE_TIER_MAX_FILES
            ));
        }
    }

    // 执行迁移
    state
        .storage
        .migrate_document(&doc_id, &target_workspace_id)
        .await
        .map_err(|e| format!("{e:#}"))?;

    Ok(())
}
