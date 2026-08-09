//! sync 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 添加监听文件夹（REQ-SYNC-001）。
///
/// 验证路径 → 首次全量同步 → 启动文件监听器 → 持久化到 settings 表。
/// 同步进度通过 `sync_progress` 事件推送前端。
#[tauri::command]
pub async fn add_watched_folder<R: Runtime>(
    app: AppHandle<R>,
    path: String,
    state: State<'_, AppState>,
) -> Result<echomind_models::SyncResult, String> {
    add_watched_folder_inner(&app, &path, state.inner()).await
}

/// `add_watched_folder` 的逻辑实现（命令与集成测试复用）。
///
/// # 流程
/// 1. 验证文件夹路径存在（REQ-SYNC-001 AC-6）
/// 2. 执行首次全量同步（REQ-SYNC-002）
/// 3. 启动文件监听器（REQ-SYNC-003）
/// 4. 持久化监听文件夹到 settings 表（REQ-SYNC-001 AC-5）
///
/// # 参数
/// - `app`: Tauri AppHandle（用于发射 `sync_progress` 事件）
/// - `path`: 监听文件夹路径
/// - `state`: 应用状态
pub async fn add_watched_folder_inner<R: Runtime>(
    app: &AppHandle<R>,
    path: &str,
    state: &AppState,
) -> Result<echomind_models::SyncResult, String> {
    // AC-6: 验证文件夹路径
    let canonical = echomind_core::sync::validate_folder_path(path)
        .map_err(|e| format!("文件夹路径无效: {e:#}"))?;
    let canonical_str = canonical.to_string_lossy().into_owned();

    // 读取 is_pro 状态
    let is_pro = *state.is_pro().read().await;

    // 创建同步进度回调（通过 Tauri 事件推送前端）
    let app_for_progress = app.clone();
    let progress: echomind_core::sync::SyncProgressFn =
        std::sync::Arc::new(move |payload: echomind_models::SyncProgressPayload| {
            let _ = app_for_progress.emit("sync_progress", &payload);
        });

    // 创建 SyncService 并执行首次同步
    let sync_service =
        echomind_core::sync::SyncService::new(state.storage.clone(), state.data_dir.clone());
    let result = sync_service
        .sync_folder(&canonical_str, is_pro, Some(progress))
        .await
        .map_err(|e| format!("首次同步失败: {e:#}"))?;

    // 启动文件监听器（REQ-SYNC-003）
    let storage_clone = state.storage.clone();
    let data_dir_clone = state.data_dir.clone();
    let app_clone = app.clone();
    let watch_path = canonical_str.clone();

    let handle = echomind_infra::file_watcher::FileWatcher::start(&canonical_str, move || {
        // 回调在 debouncer 线程中同步调用，需要 spawn 异步任务
        let storage = storage_clone.clone();
        let data_dir = data_dir_clone.clone();
        let app_handle = app_clone.clone();
        let folder = watch_path.clone();

        tauri::async_runtime::spawn(async move {
            // 每次同步时重新读取 is_pro（可能已激活）
            let pro = storage
                .get_setting("license.is_pro")
                .await
                .ok()
                .flatten()
                .is_some_and(|v| v == "true");

            let progress: echomind_core::sync::SyncProgressFn =
                std::sync::Arc::new(move |payload: echomind_models::SyncProgressPayload| {
                    let _ = app_handle.emit("sync_progress", &payload);
                });

            let svc = echomind_core::sync::SyncService::new(storage, data_dir);
            if let Err(e) = svc.sync_folder(&folder, pro, Some(progress)).await {
                warn!("文件监听同步失败: {e:#}");
            }
        });
        Ok(())
    })
    .map_err(|e| format!("启动文件监听器失败: {e:#}"))?;

    // 注册监听器句柄
    state.register_file_watcher(&canonical_str, handle).await;

    // 持久化到 settings 表（sync.watched_folders JSON 数组）
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut folders = load_watched_folders(&state.storage).await;
    // 更新或添加条目
    if let Some(entry) = folders.iter_mut().find(|f| f.path == canonical_str) {
        entry.last_synced_at = Some(now);
    } else {
        folders.push(WatchedFolderEntry {
            path: canonical_str.clone(),
            last_synced_at: Some(now),
        });
    }
    persist_watched_folders(&state.storage, &folders)
        .await
        .map_err(|e| format!("持久化监听文件夹失败: {e:#}"))?;

    Ok(result)
}

/// 移除监听文件夹（REQ-SYNC-001 AC-4）。
///
/// 仅停止监听，不删除已导入的文档（级联清理仅由文件删除事件触发）。
#[tauri::command]
pub async fn remove_watched_folder(path: String, state: State<'_, AppState>) -> Result<(), String> {
    remove_watched_folder_inner(&path, state.inner()).await
}

/// `remove_watched_folder` 的逻辑实现（命令与集成测试复用）。
pub async fn remove_watched_folder_inner(path: &str, state: &AppState) -> Result<(), String> {
    // 规范化路径（便于匹配）
    let canonical = match std::path::Path::new(path).canonicalize() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => path.to_string(),
    };

    // 停止文件监听器（Drop handle → 停止监听）
    state.unregister_file_watcher(&canonical).await;

    // 从 settings 表移除
    let mut folders = load_watched_folders(&state.storage).await;
    folders.retain(|f| f.path != canonical);
    persist_watched_folders(&state.storage, &folders)
        .await
        .map_err(|e| format!("移除监听文件夹失败: {e:#}"))?;

    Ok(())
}

/// 获取监听文件夹列表（REQ-SYNC-001 AC-3）。
#[tauri::command]
pub async fn get_watched_folders(
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::WatchedFolderInfo>, String> {
    get_watched_folders_inner(state.inner()).await
}

/// `get_watched_folders` 的逻辑实现（命令与集成测试复用）。
pub async fn get_watched_folders_inner(
    state: &AppState,
) -> Result<Vec<echomind_models::WatchedFolderInfo>, String> {
    let folders = load_watched_folders(&state.storage).await;

    let mut infos = Vec::with_capacity(folders.len());
    for f in folders {
        let name = std::path::Path::new(&f.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.path.clone());
        let sync_status = if state.is_watcher_active(&f.path).await {
            "idle".to_string()
        } else {
            "stopped".to_string()
        };
        infos.push(echomind_models::WatchedFolderInfo {
            path: f.path,
            name,
            sync_status,
            last_synced_at: f.last_synced_at,
        });
    }

    Ok(infos)
}

/// 从 settings 表读取监听文件夹列表（内部辅助函数）。
async fn load_watched_folders(storage: &SqliteStorage) -> Vec<WatchedFolderEntry> {
    match storage.get_setting("sync.watched_folders").await {
        Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_else(|e| {
            warn!("解析监听文件夹 JSON 失败: {e}");
            Vec::new()
        }),
        _ => Vec::new(),
    }
}

/// 将监听文件夹列表持久化到 settings 表（内部辅助函数）。
async fn persist_watched_folders(
    storage: &SqliteStorage,
    folders: &[WatchedFolderEntry],
) -> anyhow::Result<()> {
    let json = serde_json::to_string(folders)?;
    storage.set_setting("sync.watched_folders", &json).await?;
    Ok(())
}

/// 打开应用数据目录（REQ-ERR-004-AC-3）。
///
/// 返回数据目录路径字符串，前端通过 `tauri_plugin_opener` 在系统文件管理器中打开。
#[tauri::command]
pub async fn open_data_dir(state: State<'_, AppState>) -> Result<String, String> {
    open_data_dir_inner(state.inner()).await
}

/// `open_data_dir` 逻辑（命令与集成测试复用）。
///
/// 验证数据目录存在并返回路径字符串。
/// 前端收到路径后通过 `window.__TAURI__.opener.openPath(path)` 在系统文件管理器中打开。
pub async fn open_data_dir_inner(state: &AppState) -> Result<String, String> {
    let dir = &state.data_dir;
    if !dir.exists() {
        return Err(format!("数据目录不存在: {}", dir.display()));
    }
    Ok(dir.to_string_lossy().to_string())
}
