//! Tauri Command 层（契约见 SRS REQ-ING-001，TPM 于 2026-07-24 冻结）。
//! 体系三：全部 Result 显式处理，零 unwrap；错误以可读消息返回前端（不泄露敏感信息）。

use std::path::Path;

use echomind_core::import::{ImportOutcome, ImportService};
use echomind_models::DocStatusPayload;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// 从原始路径提取展示用文件名。
fn display_name(raw_path: &str) -> String {
    Path::new(raw_path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw_path.to_string())
}

/// 发射文档状态事件（契约：`doc-status-changed` / DocStatusPayload）。
/// 发射失败仅记录日志，不中断导入主流程。
fn emit_status(app: &AppHandle, status: &str, message: String) {
    let payload = DocStatusPayload {
        status: status.to_string(),
        message,
    };
    if let Err(err) = app.emit("doc-status-changed", payload) {
        eprintln!("doc-status-changed 事件发射失败: {err}");
    }
}

/// 导入文件（REQ-ING-001）。
/// 返回成功入库的原始文件名列表；格式非法 / 配额触顶 / 未授权 PDF / 路径遍历
/// 等错误经 `Err(可读消息)` 返回。
#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    paths: Vec<String>,
    is_pro: bool,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let service = ImportService::new(state.storage.clone(), state.data_dir.clone());
    let mut imported = Vec::new();

    for raw_path in &paths {
        let name = display_name(raw_path);
        match service.import_one(raw_path, is_pro).await {
            Ok(ImportOutcome::Imported(doc)) => {
                emit_status(&app, "indexing", format!("正在索引：{name}"));
                match service.index_document(&doc).await {
                    Ok(()) => emit_status(&app, "done", format!("索引完成：{name}")),
                    Err(err) => emit_status(&app, "error", format!("索引失败：{name}（{err}）")),
                }
                imported.push(name);
            }
            Ok(ImportOutcome::SkippedDuplicate(_)) => {
                emit_status(&app, "done", format!("内容已存在，跳过导入：{name}"));
            }
            Err(err) => return Err(err.to_string()),
        }
    }

    Ok(imported)
}
