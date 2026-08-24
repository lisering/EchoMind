//! 自定义 ONNX 嵌入模型上传/管理（REQ-VEC-014，Pro 门控）。
use super::super::*;

/// 上传自定义 ONNX 嵌入模型（REQ-VEC-014-AC-1，Pro 门控）。
///
/// Pro 用户上传自己的 ONNX 嵌入模型文件（替代预设模型）。
/// 文件复制到 `{data_dir}/custom_models/{name}/` 目录，
/// 然后验证 ONNX 格式有效性 + tokenizer 文件完整性。
///
/// # 参数
/// - `name`: 模型名称（用作目录名，自动清理路径危险字符）
/// - `onnx_path`: 用户选择的 ONNX 模型文件路径
/// - `tokenizer_files`: 用户选择的 tokenizer 文件路径列表
///
/// # 返回
/// 成功返回 `CustomModelInfo`（含模型名称、大小、有效性标志）。
///
/// # 错误
/// - `PRO_REQUIRED`: Free 用户无法使用此功能
/// - `VALIDATION`: 模型名称为空或 ONNX 文件无效
/// - `STORAGE`: 文件复制失败
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn upload_custom_embedding_model(
    name: String,
    onnx_path: String,
    tokenizer_files: Vec<String>,
    state: State<'_, AppState>,
) -> Result<CustomModelInfo, String> {
    upload_custom_embedding_model_inner(name, onnx_path, tokenizer_files, state.inner()).await
}

/// 上传自定义嵌入模型逻辑（命令与集成测试复用，Pro 门控）。
#[cfg(feature = "pro")]
pub async fn upload_custom_embedding_model_inner(
    name: String,
    onnx_path: String,
    tokenizer_files: Vec<String>,
    state: &AppState,
) -> Result<CustomModelInfo, String> {
    // Pro 门控检查
    let is_pro = *state.is_pro().read().await;
    if !is_pro {
        return Err(prefix_error(
            ERR_PRO_REQUIRED,
            "自定义嵌入模型上传是 Pro 版功能",
        ));
    }

    // 参数校验
    let name = name.trim();
    if name.is_empty() {
        return Err(prefix_error(ERR_VALIDATION, "模型名称不能为空"));
    }

    let onnx_path = Path::new(&onnx_path);
    if !onnx_path.exists() {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("ONNX 文件不存在: {}", onnx_path.display()),
        ));
    }

    // 验证 ONNX 文件格式
    echomind_infra::local_embedder::LocalEmbedder::validate_onnx_file(onnx_path)
        .map_err(|e| prefix_error(ERR_VALIDATION, &format!("{e:#}")))?;

    // 转换 tokenizer 文件路径
    let tokenizer_paths: Vec<std::path::PathBuf> = tokenizer_files
        .iter()
        .map(std::path::PathBuf::from)
        .collect();

    // 目标目录
    let custom_dir = state.custom_model_dir();
    let clean_name: String = name
        .replace(['/', '\\'], "_")
        .replace("..", "_")
        .replace('~', "_");
    let dest_dir = custom_dir.join(&clean_name);

    // 复制文件
    let copy_result = tokio::task::spawn_blocking({
        let dest_dir = dest_dir.clone();
        let onnx_path = onnx_path.to_path_buf();
        let tokenizer_paths = tokenizer_paths.clone();
        move || {
            echomind_infra::local_embedder::LocalEmbedder::copy_custom_model_files(
                &dest_dir,
                &onnx_path,
                &tokenizer_paths,
            )
        }
    })
    .await
    .map_err(|e| format!("{ERR_UNKNOWN}: 文件复制任务失败: {e}"))?;

    copy_result.map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;

    // 返回模型信息
    let size = tokio::task::spawn_blocking({
        let dest_dir = dest_dir.clone();
        move || echomind_infra::local_embedder::LocalEmbedder::dir_size(&dest_dir)
    })
    .await
    .unwrap_or(0);

    Ok(CustomModelInfo {
        name: clean_name,
        dim: 0, // 维度在加载时检测
        size_bytes: size,
        is_valid: true,
    })
}

/// Free 版 stub：上传自定义嵌入模型（返回 PRO_REQUIRED 错误）。
#[cfg(not(feature = "pro"))]
#[tauri::command]
pub async fn upload_custom_embedding_model(
    _name: String,
    _onnx_path: String,
    _tokenizer_files: Vec<String>,
    _state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    Err(prefix_error(
        ERR_PRO_REQUIRED,
        "自定义嵌入模型上传是 Pro 版功能",
    ))
}

/// 列出已上传的自定义嵌入模型（REQ-VEC-014-AC-2，Pro 门控）。
///
/// 扫描 `custom_models/` 目录，返回所有已上传的自定义模型列表。
/// 每个模型包含名称、大小和完整性标志。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn list_custom_models(
    state: State<'_, AppState>,
) -> Result<Vec<CustomModelInfo>, String> {
    list_custom_models_inner(state.inner()).await
}

/// 列出自定义模型逻辑（命令与集成测试复用，Pro 门控）。
#[cfg(feature = "pro")]
pub async fn list_custom_models_inner(state: &AppState) -> Result<Vec<CustomModelInfo>, String> {
    let is_pro = *state.is_pro().read().await;
    if !is_pro {
        return Err(prefix_error(
            ERR_PRO_REQUIRED,
            "自定义嵌入模型管理是 Pro 版功能",
        ));
    }
    let custom_dir = state.custom_model_dir();
    let models = tokio::task::spawn_blocking(move || {
        echomind_infra::local_embedder::LocalEmbedder::list_custom_models(&custom_dir)
    })
    .await
    .map_err(|e| format!("{ERR_UNKNOWN}: 列出模型任务失败: {e}"))?;
    Ok(models)
}

/// Free 版 stub：列出自定义模型（返回 PRO_REQUIRED 错误）。
#[cfg(not(feature = "pro"))]
#[tauri::command]
pub async fn list_custom_models(
    _state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    Err(prefix_error(
        ERR_PRO_REQUIRED,
        "自定义嵌入模型管理是 Pro 版功能",
    ))
}

/// 删除自定义嵌入模型（REQ-VEC-014-AC-4，Pro 门控）。
///
/// 删除 `custom_models/{name}/` 目录及其所有文件。
/// 如果当前正在使用该模型，会清除 embedder 缓存（下次使用时重新初始化）。
///
/// # 参数
/// - `name`: 要删除的模型名称
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn delete_custom_model(name: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_custom_model_inner(name, state.inner()).await
}

/// 删除自定义模型逻辑（命令与集成测试复用，Pro 门控）。
#[cfg(feature = "pro")]
pub async fn delete_custom_model_inner(name: String, state: &AppState) -> Result<(), String> {
    let is_pro = *state.is_pro().read().await;
    if !is_pro {
        return Err(prefix_error(
            ERR_PRO_REQUIRED,
            "自定义嵌入模型管理是 Pro 版功能",
        ));
    }
    let custom_dir = state.custom_model_dir();
    let name_for_task = name.clone();
    let freed = tokio::task::spawn_blocking(move || {
        echomind_infra::local_embedder::LocalEmbedder::delete_custom_model(
            &custom_dir,
            &name_for_task,
        )
    })
    .await
    .map_err(|e| format!("{ERR_UNKNOWN}: 删除模型任务失败: {e}"))?;

    if freed == 0 {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("自定义模型 '{name}' 不存在"),
        ));
    }

    // 检查当前是否正在使用被删除的模型，如果是则清除 embedder 缓存
    let current_model = state
        .storage
        .get_setting("vec.embedding_model")
        .await
        .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;
    if let Some(ref model_str) = current_model
        && model_str == &format!("custom:{name}")
    {
        // 重置为默认模型
        state
            .storage
            .set_setting("vec.embedding_model", "all-MiniLM-L6-v2")
            .await
            .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;
        // 清除 embedder 缓存
        state
            .set_embedding_model("all-MiniLM-L6-v2")
            .await
            .map_err(|e| format!("{ERR_STORAGE}: {e:#}"))?;
    }

    Ok(())
}

/// Free 版 stub：删除自定义模型（返回 PRO_REQUIRED 错误）。
#[cfg(not(feature = "pro"))]
#[tauri::command]
pub async fn delete_custom_model(_name: String, _state: State<'_, AppState>) -> Result<(), String> {
    Err(prefix_error(
        ERR_PRO_REQUIRED,
        "自定义嵌入模型管理是 Pro 版功能",
    ))
}
