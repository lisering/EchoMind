//! AutoDream 后台空闲整理引擎命令（REQ-RAG-029）。
use super::super::*;

/// 触发后台 Dream 分析（重复文档检测 + 跨文档矛盾发现 + 整理建议生成）。
///
/// 在后台异步执行全库扫描，分析进度通过 `dream_progress` 事件推送前端。
/// 分析完成后通过 `dream_done` 事件推送结果摘要，前端可调用 `get_dream_suggestions` 获取详情。
///
/// # 事件序列
/// - `dream_progress` { phase: "scanning" | "analyzing" | "done" | "error", message }
/// - `dream_done` { total_suggestions, total_documents, elapsed_ms }
///
/// # 并发控制
/// 如果已有分析在进行中，返回错误提示。
#[tauri::command]
pub async fn trigger_dream(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    trigger_dream_inner(&app, state.inner()).await
}

/// Dream 分析逻辑（命令与集成测试复用）。
///
/// # 流程
/// 1. 并发检查（防止重复执行）
/// 2. 初始化 Embedder + LLM Provider
/// 3. 构造 DreamEngine 并执行三阶段分析
/// 4. 结果写入 `dream_engine` 状态
/// 5. 发射 `dream_done` 事件
pub async fn trigger_dream_inner<R: Runtime>(
    app: &AppHandle<R>,
    state: &AppState,
) -> Result<(), String> {
    // 并发检查
    if state.dream_engine.is_running() {
        return Err("Dream 分析正在进行中，请等待完成".to_string());
    }
    if !state.dream_engine.try_start() {
        return Err("Dream 分析正在进行中，请等待完成".to_string());
    }

    // 重置取消标志
    state.dream_engine.reset_cancel();

    // 发射进度事件
    emit_dream_progress(app, "scanning", "正在初始化分析引擎…");

    // 初始化 Embedder
    let embedder = match state.embedder().await {
        Ok(e) => e.clone(),
        Err(e) => {
            state.dream_engine.finish();
            let msg = format!("向量化引擎不可用: {e:#}");
            emit_dream_progress(app, "error", &msg);
            return Err(msg);
        }
    };

    // 初始化 LLM Provider
    let llm_config = state.llm_config().read().await.clone();
    let provider = match llm_config {
        Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
            Ok(p) => p,
            Err(e) => {
                state.dream_engine.finish();
                let msg = format!("LLM 初始化失败: {e:#}");
                emit_dream_progress(app, "error", &msg);
                return Err(msg);
            }
        },
        None => {
            state.dream_engine.finish();
            let msg = "未配置 LLM：请完成初始配置向导".to_string();
            emit_dream_progress(app, "error", &msg);
            return Err(msg);
        }
    };

    // 构造 DreamEngine
    let engine = DreamEngine::new(
        embedder,
        state.storage.clone(),
        provider,
        echomind_core::idempotency::IdempotencyStore::new(),
    );
    let cancel = state.dream_engine.cancel_flag();

    emit_dream_progress(app, "analyzing", "正在分析知识库…");

    // 执行分析
    match engine.dream(cancel).await {
        Ok(result) => {
            let summary = serde_json::json!({
                "total_suggestions": result.total_suggestions,
                "total_documents": result.total_documents,
                "elapsed_ms": result.elapsed_ms,
            });
            state.dream_engine.set_results(result).await;
            state.dream_engine.finish();
            emit_dream_progress(app, "done", "分析完成");
            if let Err(err) = app.emit("dream_done", summary) {
                eprintln!("dream_done 事件发射失败: {err}");
            }
            Ok(())
        }
        Err(e) => {
            state.dream_engine.finish();
            let msg = format!("Dream 分析失败: {e:#}");
            emit_dream_progress(app, "error", &msg);
            Err(msg)
        }
    }
}

/// 获取 Dream 分析建议（返回缓存的分析结果）。
///
/// 返回最近一次 `trigger_dream` 的完整结果。
/// 如果尚未运行过分析，返回空结果（total_suggestions=0）。
#[tauri::command]
pub async fn get_dream_suggestions(
    state: State<'_, AppState>,
) -> Result<echomind_core::auto_dream::DreamResult, String> {
    get_dream_suggestions_inner(state.inner()).await
}

/// Dream 建议查询逻辑（命令与集成测试复用）。
pub async fn get_dream_suggestions_inner(
    state: &AppState,
) -> Result<echomind_core::auto_dream::DreamResult, String> {
    match state.dream_engine.get_results().await {
        Some(result) => Ok(result),
        None => Ok(echomind_core::auto_dream::DreamResult {
            suggestions: vec![],
            total_documents: 0,
            total_suggestions: 0,
            elapsed_ms: 0,
        }),
    }
}

/// 取消正在进行的 Dream 分析。
///
/// 设置取消标志，DreamEngine 在下一个检查点中断并返回部分结果。
#[tauri::command]
pub async fn abort_dream(state: State<'_, AppState>) -> Result<(), String> {
    state.dream_engine.cancel();
    Ok(())
}

/// 发射 Dream 进度事件。
fn emit_dream_progress<R: Runtime>(app: &AppHandle<R>, phase: &str, message: &str) {
    let payload = serde_json::json!({
        "phase": phase,
        "message": message,
    });
    if let Err(err) = app.emit("dream_progress", payload) {
        eprintln!("dream_progress 事件发射失败: {err}");
    }
}
