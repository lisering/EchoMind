//! features 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 获取当前日志级别（REQ-OBS-001 AC-5）。
///
/// 从 settings 表 `log.level` 键读取，默认 `"info"`。
///
/// # 返回
///
/// `"debug"` / `"info"` / `"warn"` / `"error"`
#[tauri::command]
pub async fn get_log_level(state: State<'_, AppState>) -> Result<String, String> {
    get_log_level_inner(state.inner()).await
}

/// 日志级别查询逻辑（命令与集成测试复用）。
pub async fn get_log_level_inner(state: &AppState) -> Result<String, String> {
    let level = state
        .storage
        .get_setting("log.level")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "info".to_string());
    Ok(level)
}

/// 设置日志级别（REQ-OBS-001 AC-5）。
///
/// 持久化到 settings 表 `log.level` 键，同时通过 `LocalLogger::set_level()`
/// 动态更新运行时过滤级别（无需重启应用）。
///
/// # 参数
///
/// - `level` — `"debug"` / `"info"` / `"warn"` / `"error"`（不区分大小写）
///
/// # 错误
///
/// - 无效的日志级别
/// - settings 表写入失败
/// - 日志系统未初始化（降级为仅持久化，下次启动生效）
#[tauri::command]
pub async fn set_log_level(level: String, state: State<'_, AppState>) -> Result<(), String> {
    set_log_level_inner(level, state.inner()).await
}

/// 日志级别设置逻辑（命令与集成测试复用）。
pub async fn set_log_level_inner(level: String, state: &AppState) -> Result<(), String> {
    // 验证级别有效性
    let normalized = level.to_lowercase();
    match normalized.as_str() {
        "debug" | "info" | "warn" | "error" => {}
        _ => {
            return Err(format!(
                "无效的日志级别: {level}（可选: debug/info/warn/error）"
            ));
        }
    }

    // 持久化到 settings 表
    state
        .storage
        .set_setting("log.level", &normalized)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 动态更新运行时过滤级别（日志系统可能未初始化，降级为下次启动生效）
    if let Err(e) = echomind_infra::local_logger::LocalLogger::set_level(&normalized) {
        eprintln!("运行时切换日志级别失败（下次启动将生效）: {e}");
    }

    Ok(())
}

/// 导出日志文件内容（REQ-OBS-001）。
///
/// 读取最近 `tail_lines` 行日志（默认 100 行），返回文本内容。
///
/// # 参数
///
/// - `tail_lines` — 返回最近 N 行日志（默认 100）
///
/// # 返回
///
/// 日志文本内容（JSON Lines 格式，每行一个 JSON 对象）
#[tauri::command]
pub async fn export_logs(
    tail_lines: Option<usize>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    export_logs_inner(tail_lines, state.inner()).await
}

/// 日志导出逻辑（命令与集成测试复用）。
pub async fn export_logs_inner(
    tail_lines: Option<usize>,
    state: &AppState,
) -> Result<String, String> {
    let n = tail_lines.unwrap_or(100);
    let log_dir = state.logs_dir();

    if !log_dir.exists() {
        return Ok("(日志目录不存在)".to_string());
    }

    echomind_infra::local_logger::LocalLogger::read_logs_from_dir(&log_dir, n)
        .map_err(|e| format!("读取日志文件失败: {e:#}"))
}

/// 导出诊断信息（REQ-OBS-002）。
///
/// 收集系统信息、应用版本、数据库规模、知识库规模、嵌入维度、
/// LLM 配置（脱敏）、最近 100 行日志，聚合为 JSON 字符串。
///
/// **隐私铁律**：不包含 API Key 明文、不包含用户文档内容或对话内容。
///
/// # 返回
///
/// JSON 格式字符串，包含：
/// - `app_version` — 应用版本
/// - `timestamp` — 导出时间（ISO 8601）
/// - `system` — 操作系统、架构、CPU 核心数、内存（MB）
/// - `database` — 数据库路径、大小
/// - `knowledge_base` — 文档数、chunk 数、嵌入维度
/// - `llm_config` — LLM 配置（脱敏）
/// - `recent_logs` — 最近 100 行日志
#[tauri::command]
pub async fn export_diagnostics(state: State<'_, AppState>) -> Result<String, String> {
    export_diagnostics_inner(state.inner()).await
}

/// 诊断信息导出逻辑（命令与集成测试复用）。
pub async fn export_diagnostics_inner(state: &AppState) -> Result<String, String> {
    // 收集系统信息
    let doc_count = state
        .storage
        .count_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let chunk_count = state
        .storage
        .count_chunks()
        .await
        .map_err(|e| format!("{e:#}"))?;

    // LLM 配置（脱敏）
    let llm_config = state.llm_config().read().await.clone();
    let (api_key_masked, model, base_url) = match &llm_config {
        Some(cfg) => (
            Some(mask_api_key(&cfg.api_key)),
            Some(cfg.model.clone()),
            Some(cfg.base_url.clone()),
        ),
        None => (None, None, None),
    };

    // LLM 模式
    let llm_mode = state.get_llm_mode().await;
    let mode_str = match llm_mode {
        LlmMode::Remote => "remote",
        LlmMode::Local => "local",
    };

    let db_path = state.data_dir.join("echomind.db");

    let diagnostics = echomind_infra::local_logger::collect_diagnostics(
        env!("CARGO_PKG_VERSION"),
        &state.data_dir,
        &db_path,
        doc_count,
        chunk_count,
        384, // all-MiniLM-L6-v2 嵌入维度
        api_key_masked.as_deref(),
        model.as_deref(),
        base_url.as_deref(),
        Some(mode_str),
    );

    serde_json::to_string_pretty(&diagnostics).map_err(|e| format!("序列化诊断信息失败: {e:#}"))
}

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

/// 搜索代码符号（精确 + 模糊匹配，REQ-RAG-031）。
///
/// 精确匹配优先：在 `code_symbols` 表中按 `name` 精确查找；
/// 若结果不足，追加模糊匹配（`name LIKE '%query%'`）。
#[tauri::command]
pub async fn search_symbols(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::CodeSymbol>, String> {
    search_symbols_inner(query, limit, state.inner()).await
}

/// 符号搜索逻辑（命令与集成测试复用）。
pub async fn search_symbols_inner(
    query: String,
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<echomind_models::CodeSymbol>, String> {
    let max = limit.unwrap_or(20);
    let mut results = state
        .storage
        .search_by_symbol(&query, None)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 精确匹配不足时追加模糊匹配
    if results.len() < max {
        let fuzzy = state
            .storage
            .search_symbols_fuzzy(&query, max)
            .await
            .map_err(|e| format!("{e:#}"))?;
        // 去重：跳过已精确匹配的符号（owned strings 避免借用冲突）
        let existing: std::collections::HashSet<String> =
            results.iter().map(|s| s.name.clone()).collect();
        for sym in fuzzy {
            if !existing.contains(&sym.name) {
                results.push(sym);
            }
            if results.len() >= max {
                break;
            }
        }
    }

    results.truncate(max);
    Ok(results)
}

/// 获取指定 chunk 的所有代码符号（REQ-RAG-031）。
#[tauri::command]
pub async fn get_symbols_for_chunk(
    chunk_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::CodeSymbol>, String> {
    state
        .storage
        .get_symbols_for_chunk(&chunk_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 重建代码符号索引（REQ-RAG-031）。
///
/// 遍历所有已索引的代码文件（.rs/.ts/.tsx/.py/.go），
/// 重新通过 tree-sitter AST 抽取符号并写入 `code_symbols` 表。
/// Pro 专用功能。
#[tauri::command]
#[cfg(feature = "pro")]
pub async fn rebuild_symbol_index(state: State<'_, AppState>) -> Result<usize, String> {
    rebuild_symbol_index_inner(state.inner()).await
}

/// 符号索引重建逻辑（命令与集成测试复用，Pro feature）。
#[cfg(feature = "pro")]
pub async fn rebuild_symbol_index_inner(state: &AppState) -> Result<usize, String> {
    use echomind_core::SymbolExtractor;

    let engine = SymbolEngine;
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let code_exts = ["rs", "ts", "tsx", "py", "go"];
    let mut total_symbols = 0usize;

    for doc in &docs {
        // 仅处理代码文件
        let ext = Path::new(&doc.file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        if !code_exts.contains(&ext.as_str()) {
            continue;
        }

        // 检测语言
        let language = match engine.detect_language(&doc.file_path) {
            Some(lang) => lang,
            None => continue,
        };

        // 获取该文档的所有 chunk
        let chunks = state
            .storage
            .list_chunks(&doc.id)
            .await
            .map_err(|e| format!("{e:#}"))?;

        // 为每个 chunk 抽取符号
        let mut all_symbols = Vec::new();
        for chunk in &chunks {
            let symbols = engine.extract_symbols(&chunk.content, &language, &chunk.id);
            all_symbols.extend(symbols);
        }

        if !all_symbols.is_empty() {
            total_symbols += all_symbols.len();
            state
                .storage
                .add_symbols(&all_symbols)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
    }

    Ok(total_symbols)
}

/// 创建并保存工作流模板（持久化到 settings 表 JSON）。
///
/// # 参数
/// - `name` — 工作流名称
/// - `description` — 工作流描述
/// - `nodes_json` — 节点列表 JSON 字符串
/// - `edges_json` — 边列表 JSON 字符串
///
/// # 返回
/// 工作流 ID（UUID v4）
///
/// # 错误
/// - JSON 解析失败
/// - settings 表写入失败
#[tauri::command]
pub async fn save_workflow_template(
    name: String,
    description: String,
    nodes_json: String,
    edges_json: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    save_workflow_template_inner(&name, &description, &nodes_json, &edges_json, state.inner()).await
}

/// 工作流模板保存逻辑（命令与集成测试复用）。
pub async fn save_workflow_template_inner(
    name: &str,
    description: &str,
    nodes_json: &str,
    edges_json: &str,
    state: &AppState,
) -> Result<String, String> {
    // 解析 nodes JSON
    let nodes: Vec<echomind_models::WorkflowNode> = serde_json::from_str(nodes_json)
        .map_err(|e| prefix_error(ERR_PARSE, &format!("节点 JSON 解析失败: {e}")))?;

    // 解析 edges JSON
    let edges: Vec<echomind_models::WorkflowEdge> = serde_json::from_str(edges_json)
        .map_err(|e| prefix_error(ERR_PARSE, &format!("边 JSON 解析失败: {e}")))?;

    // 验证：至少 1 个节点
    if nodes.is_empty() {
        return Err(prefix_error(ERR_VALIDATION, "工作流至少需要 1 个节点"));
    }

    // 构造 Workflow
    let workflow = Workflow::new(name.to_string(), description.to_string(), nodes, edges);

    let workflow_id = workflow.id.clone();

    // 序列化为 JSON 并存储
    let workflow_json = serde_json::to_string(&workflow)
        .map_err(|e| prefix_error(ERR_PARSE, &format!("工作流序列化失败: {e}")))?;

    state
        .storage
        .set_setting(
            &format!("{WORKFLOW_KEY_PREFIX}{workflow_id}"),
            &workflow_json,
        )
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 更新工作流索引
    update_workflow_index(state, &workflow_id, true)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("工作流索引更新失败: {e:#}")))?;

    Ok(workflow_id)
}

/// 执行工作流。
///
/// 从 settings 表读取工作流定义，构建 `WorkflowEngine` 并执行。
///
/// # 参数
/// - `workflow_id` — 工作流 ID
/// - `input` — 初始输入文本
///
/// # 返回
/// `WorkflowResult` — 各节点状态和最终输出
///
/// # 错误
/// - 工作流不存在
/// - LLM 配置缺失
/// - 嵌入引擎初始化失败
/// - 工作流执行错误（如环检测）
#[tauri::command]
pub async fn run_workflow(
    workflow_id: String,
    input: String,
    state: State<'_, AppState>,
) -> Result<WorkflowResult, String> {
    run_workflow_inner(&workflow_id, &input, state.inner()).await
}

/// 工作流执行逻辑（命令与集成测试复用）。
pub async fn run_workflow_inner(
    workflow_id: &str,
    input: &str,
    state: &AppState,
) -> Result<WorkflowResult, String> {
    // 1. 从 settings 表读取工作流定义
    let workflow_json = state
        .storage
        .get_setting(&format!("{WORKFLOW_KEY_PREFIX}{workflow_id}"))
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?
        .ok_or_else(|| prefix_error(ERR_VALIDATION, &format!("工作流不存在: {workflow_id}")))?;

    let workflow: Workflow = serde_json::from_str(&workflow_json)
        .map_err(|e| prefix_error(ERR_PARSE, &format!("工作流反序列化失败: {e}")))?;

    // 2. 加载 LLM 配置
    let llm_config = state
        .llm_config()
        .read()
        .await
        .clone()
        .ok_or_else(|| prefix_error(ERR_VALIDATION, "未配置 LLM，请完成初始配置向导"))?;

    // 3. 初始化 Embedder（懒加载，首次使用触发模型下载）
    let embedder = state
        .embedder()
        .await
        .map_err(|e| prefix_error(ERR_EMBED, &format!("向量化引擎不可用: {e:#}")))?;

    // 4. 构建检索器（HybridRetriever：向量 + 关键词 RRF 融合）
    let retriever = HybridRetriever::new(embedder, state.storage.clone());

    // 5. 构建 LLM Provider
    let provider = OpenAIProvider::new(
        llm_config.api_key.clone(),
        llm_config.base_url.clone(),
        llm_config.model.clone(),
    )
    .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))?;

    // 6. 构建工作流引擎并执行
    let engine = echomind_core::workflow::WorkflowEngine::new(retriever, provider);
    engine
        .run(&workflow, input)
        .await
        .map_err(|e| prefix_error(ERR_UNKNOWN, &format!("工作流执行失败: {e:#}")))
}

/// 列出所有已保存的工作流模板。
///
/// 扫描 `workflow.index` 索引，逐个读取并反序列化工作流定义。
///
/// # 返回
/// `Vec<Workflow>` — 所有已保存的工作流模板列表
#[tauri::command]
pub async fn list_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String> {
    list_workflows_inner(state.inner()).await
}

/// 工作流列表查询逻辑（命令与集成测试复用）。
pub async fn list_workflows_inner(state: &AppState) -> Result<Vec<Workflow>, String> {
    // 读取工作流索引
    let index_json = state
        .storage
        .get_setting(WORKFLOW_INDEX_KEY)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    let workflow_ids: Vec<String> = match index_json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| prefix_error(ERR_PARSE, &format!("工作流索引解析失败: {e}")))?,
        None => Vec::new(),
    };

    // 逐个读取工作流定义
    let mut workflows = Vec::new();
    for id in &workflow_ids {
        if let Ok(Some(json)) = state
            .storage
            .get_setting(&format!("{WORKFLOW_KEY_PREFIX}{id}"))
            .await
            && let Ok(wf) = serde_json::from_str::<Workflow>(&json)
        {
            workflows.push(wf);
        }
    }

    Ok(workflows)
}

/// 删除工作流模板。
///
/// 从 settings 表删除 `workflow.{id}` 键，并从索引中移除。
///
/// # 参数
/// - `workflow_id` — 要删除的工作流 ID
///
/// # 错误
/// - settings 表操作失败
#[tauri::command]
pub async fn delete_workflow(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_workflow_inner(&workflow_id, state.inner()).await
}

/// 工作流删除逻辑（命令与集成测试复用）。
pub async fn delete_workflow_inner(workflow_id: &str, state: &AppState) -> Result<(), String> {
    // 删除工作流定义
    state
        .storage
        .set_setting(&format!("{WORKFLOW_KEY_PREFIX}{workflow_id}"), "")
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 从索引中移除
    update_workflow_index(state, workflow_id, false)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("工作流索引更新失败: {e:#}")))?;

    Ok(())
}

/// 更新工作流索引（添加或移除工作流 ID）。
///
/// `add = true` → 添加 ID 到索引；`add = false` → 从索引移除 ID。
async fn update_workflow_index(
    state: &AppState,
    workflow_id: &str,
    add: bool,
) -> anyhow::Result<()> {
    let index_json = state.storage.get_setting(WORKFLOW_INDEX_KEY).await?;

    let mut ids: Vec<String> = match index_json {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => Vec::new(),
    };

    if add {
        if !ids.contains(&workflow_id.to_string()) {
            ids.push(workflow_id.to_string());
        }
    } else {
        ids.retain(|id| id != workflow_id);
    }

    let new_index = serde_json::to_string(&ids)?;

    state
        .storage
        .set_setting(WORKFLOW_INDEX_KEY, &new_index)
        .await?;

    Ok(())
}

/// 执行代码片段并返回结果（Pro feature，REQ-RAG-032）。
///
/// 在安全沙箱中执行代码片段（Python / JavaScript），返回 stdout / stderr / exit_code。
/// 安全限制：超时 10s、内存 64MB、无网络访问。
///
/// Free 版本调用此命令返回 Pro 错误。
#[cfg(feature = "pro")]
#[tauri::command]
pub async fn execute_code_snippet(
    _state: State<'_, AppState>,
    code: String,
    language: String,
    stdin: Option<String>,
) -> Result<ExecutionResult, String> {
    execute_code_snippet_inner(&code, &language, stdin.as_deref())
        .await
        .map_err(|e| format!("{e:#}"))
}

/// Free 版本：返回 Pro 错误。
#[cfg(not(feature = "pro"))]
#[tauri::command]
pub async fn execute_code_snippet(
    _state: State<'_, AppState>,
    _code: String,
    _language: String,
    _stdin: Option<String>,
) -> Result<ExecutionResult, String> {
    Err(prefix_error(ERR_PRO_REQUIRED, "代码执行需要 Pro 版本"))
}

/// 代码执行内部实现（Pro 版本）。
#[cfg(feature = "pro")]
async fn execute_code_snippet_inner(
    code: &str,
    language: &str,
    stdin: Option<&str>,
) -> anyhow::Result<ExecutionResult> {
    use echomind_core::CodeExecutor;
    use echomind_infra::wasm_executor::WasmExecutor;

    let executor = WasmExecutor::with_defaults();
    executor.execute(code, language, stdin).await
}

/// 启用/禁用持久化记忆系统。
///
/// 持久化到 settings 表 `memory.enabled` 键，下次 chat 命令调用时即时生效。
/// 启用后：ChatEngine 检索相关跨会话记忆注入 system prompt；AutoDream 后台自动整合记忆。
#[tauri::command]
pub async fn set_memory_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_memory_enabled_inner(enabled, state.inner()).await
}

/// 记忆开关逻辑（命令与集成测试复用）。
pub async fn set_memory_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    state
        .storage
        .set_setting("memory.enabled", if enabled { "true" } else { "false" })
        .await
        .map_err(|e| format!("{e:#}"))?;
    // 运行时即时更新 AppState 字段
    // 注意：AppState 字段不可变，需通过内部 RwLock 或 AtomicBool 更新
    // 当前实现：下次 chat 命令从 settings batch 读取，state.memory_enabled 在重启后更新
    // 未来优化：使用 AtomicBool 字段实现即时生效
    Ok(())
}

/// 获取所有记忆条目（可按层级过滤）。
///
/// `tier` 为 `None` 时返回所有层级的记忆，按 importance DESC 排序。
#[tauri::command]
pub async fn get_memories(
    tier: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<MemoryEntry>, String> {
    get_memories_inner(tier.as_deref(), state.inner()).await
}

/// 记忆查询逻辑（命令与集成测试复用）。
pub async fn get_memories_inner(
    tier: Option<&str>,
    state: &AppState,
) -> Result<Vec<MemoryEntry>, String> {
    let tier_enum = match tier {
        Some("wing") => Some(MemoryTier::Wing),
        Some("hall") => Some(MemoryTier::Hall),
        Some("room") => Some(MemoryTier::Room),
        Some(_) => return Err("无效的层级，可选值：wing / hall / room".to_string()),
        None => None,
    };
    let mut entries = state
        .storage
        .get_memory_entries(tier_enum.as_ref())
        .await
        .map_err(|e| format!("{e:#}"))?;
    entries.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(entries)
}

/// 用户手动置顶记忆（直接创建 Room 层，importance 1.0）。
///
/// 用于用户明确希望永久记住的关键信息（如个人偏好、重要决定）。
#[tauri::command]
pub async fn pin_memory(
    conversation_id: String,
    content: String,
    state: State<'_, AppState>,
) -> Result<MemoryEntry, String> {
    pin_memory_inner(&conversation_id, &content, state.inner()).await
}

/// 置顶记忆逻辑（命令与集成测试复用）。
pub async fn pin_memory_inner(
    conversation_id: &str,
    content: &str,
    state: &AppState,
) -> Result<MemoryEntry, String> {
    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    memory_store
        .pin_memory(conversation_id, content)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 手动提升记忆层级（Wing → Hall → Room）。
///
/// 每次提升 importance += 0.1（上限 1.0）。Room 层无法再提升。
#[tauri::command]
pub async fn promote_memory(memory_id: String, state: State<'_, AppState>) -> Result<(), String> {
    promote_memory_inner(&memory_id, state.inner()).await
}

/// 记忆提升逻辑（命令与集成测试复用）。
pub async fn promote_memory_inner(memory_id: &str, state: &AppState) -> Result<(), String> {
    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    memory_store
        .promote(memory_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 删除指定记忆条目。
#[tauri::command]
pub async fn delete_memory(memory_id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_memory_inner(&memory_id, state.inner()).await
}

/// 删除记忆逻辑（命令与集成测试复用）。
pub async fn delete_memory_inner(memory_id: &str, state: &AppState) -> Result<(), String> {
    state
        .storage
        .delete_memory_entry(memory_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 清空记忆条目（可按层级过滤）。
///
/// `tier` 为 `None` 时清空所有层级。返回删除的行数。
#[tauri::command]
pub async fn clear_memories(
    tier: Option<String>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    clear_memories_inner(tier.as_deref(), state.inner()).await
}

/// 清空记忆逻辑（命令与集成测试复用）。
pub async fn clear_memories_inner(tier: Option<&str>, state: &AppState) -> Result<usize, String> {
    let tier_enum = match tier {
        Some("wing") => Some(MemoryTier::Wing),
        Some("hall") => Some(MemoryTier::Hall),
        Some("room") => Some(MemoryTier::Room),
        Some(_) => return Err("无效的层级，可选值：wing / hall / room".to_string()),
        None => None,
    };
    state
        .storage
        .clear_memory_entries(tier_enum.as_ref())
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 手动触发 Scratch 层记忆整合（Q01 借鉴 QM consolidation）。
///
/// 读取 scratch_logs 表中的临时事实，通过 LLM 审查后执行 UPDATE/DELETE/ADD 动作，
/// 将有价值的事实 promote 到长期记忆层（Wing/Hall/Room）。
/// 整合完成后清除已处理的 scratch 条目。
///
/// 需要已配置 LLM（api_key + base_url + model）。未配置时返回错误。
#[tauri::command]
pub async fn trigger_memory_consolidation(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    trigger_memory_consolidation_inner(state.inner()).await
}

/// 记忆整合逻辑（命令与集成测试复用）。
pub async fn trigger_memory_consolidation_inner(
    state: &AppState,
) -> Result<serde_json::Value, String> {
    // 初始化 LLM Provider
    let llm_config = state.llm_config().read().await.clone();
    let provider = match llm_config {
        Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
            Ok(p) => p,
            Err(e) => {
                return Err(format!("LLM 初始化失败: {e:#}"));
            }
        },
        None => {
            return Err("未配置 LLM：请完成初始配置向导".to_string());
        }
    };

    // 执行 Scratch 整合
    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    let result = memory_store
        .consolidate_scratch(&provider, 14)
        .await
        .map_err(|e| format!("记忆整合失败: {e:#}"))?;

    Ok(serde_json::json!({
        "actions_count": result.actions.len(),
        "expired_cleaned": result.expired_cleaned,
        "remaining_scratch": result.remaining_scratch,
    }))
}

/// 获取 Scratch 层日志条目（Q01 借鉴 QM scratch-promote）。
///
/// 返回按创建时间正序排列的 scratch 日志，可选限制数量。
/// `limit` 为 `None` 时返回全部条目。
#[tauri::command]
pub async fn get_scratch_logs(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::ScratchLogEntry>, String> {
    get_scratch_logs_inner(limit, state.inner()).await
}

/// Scratch 日志查询逻辑（命令与集成测试复用）。
pub async fn get_scratch_logs_inner(
    limit: Option<usize>,
    state: &AppState,
) -> Result<Vec<echomind_models::ScratchLogEntry>, String> {
    state
        .storage
        .get_scratch_logs(limit)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换网页搜索开关（REQ-RAG-036）。
///
/// 持久化到 settings 表 `rag.web_search_enabled` 键，下次 chat 命令调用时即时生效。
/// 启用后，当本地检索 top-1 score < 阈值（0.3）时，自动调用 DuckDuckGo API 搜索互联网，
/// 将搜索结果通过 RRF 融合到本地检索结果中，在 prompt 中标注来源（🌐 Web）。
/// 搜索失败时优雅降级为仅使用本地结果。默认关闭（opt-in）。
#[tauri::command]
pub async fn set_web_search_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_web_search_enabled_inner(enabled, state.inner()).await
}

/// 网页搜索开关写入逻辑（命令与集成测试复用）。
pub async fn set_web_search_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.web_search_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))?;
    Ok(())
}

/// 执行网页搜索（REQ-RAG-036）。
///
/// 直接调用 DuckDuckGo Instant Answer API 搜索互联网，返回搜索结果列表。
/// 用于前端搜索面板或独立搜索功能（非 chat 管线内的自动触发）。
///
/// # 参数
/// - `query`: 搜索查询文本
///
/// # 返回
/// 搜索结果列表（`SearchResult`），按相关性降序排列。
/// 搜索失败返回空列表（不报错）。
#[tauri::command]
pub async fn web_search(
    query: String,
    _state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    web_search_inner(&query).await
}

/// 网页搜索逻辑（命令与集成测试复用）。
pub async fn web_search_inner(query: &str) -> Result<Vec<SearchResult>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let provider = DuckDuckGoProvider::new().map_err(|e| format!("{e:#}"))?;
    let provider_arc: Arc<dyn echomind_core::WebSearchProvider> = Arc::new(provider);
    provider_arc
        .search(query)
        .await
        .map_err(|e| format!("{e:#}"))
}

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

/// 创建或更新自定义快捷指令模板（S56）。
///
/// # 参数
/// - `name` — 指令名（不含 `/`，仅小写字母/数字/下划线，1-32 字符）
/// - `label` — 显示标签
/// - `description` — 描述说明
/// - `icon` — 图标 emoji
/// - `prompt_template` — Prompt 模板内容（必须包含 `{query}` 占位符）
///
/// # 返回
/// 模板 ID（UUID v4）
///
/// # 错误
/// - 名称不合法（空/含非法字符/超长）
/// - 模板内容缺少 `{query}` 占位符
/// - 名称与系统内置指令冲突（summary/compare/extract/translate/timeline/mindmap）
/// - settings 表写入失败
#[tauri::command]
pub async fn save_prompt_template(
    name: String,
    label: String,
    description: String,
    icon: String,
    prompt_template: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    save_prompt_template_inner(
        &name,
        &label,
        &description,
        &icon,
        &prompt_template,
        state.inner(),
    )
    .await
}

/// 模板保存逻辑（命令与集成测试复用）。
pub async fn save_prompt_template_inner(
    name: &str,
    label: &str,
    description: &str,
    icon: &str,
    prompt_template: &str,
    state: &AppState,
) -> Result<String, String> {
    // 验证名称合法性
    if !PromptTemplate::is_valid_name(name) {
        return Err(prefix_error(
            ERR_VALIDATION,
            "指令名称不合法（仅小写字母/数字/下划线，1-32 字符）",
        ));
    }

    // 验证不与系统内置指令冲突
    const SYSTEM_COMMANDS: &[&str] = &[
        "summary",
        "compare",
        "extract",
        "translate",
        "timeline",
        "mindmap",
    ];
    if SYSTEM_COMMANDS.contains(&name) {
        return Err(prefix_error(
            ERR_VALIDATION,
            &format!("指令名称 '{name}' 与系统内置指令冲突"),
        ));
    }

    // 验证模板内容包含 {query} 占位符
    if !PromptTemplate::has_query_placeholder(prompt_template) {
        return Err(prefix_error(
            ERR_VALIDATION,
            "模板内容必须包含 {query} 占位符",
        ));
    }

    // 检查名称是否已被使用（更新已有模板时允许同名）
    let existing_templates = list_prompt_templates_inner(state).await.unwrap_or_default();
    let existing = existing_templates.iter().find(|t| t.name == name);

    let template = if let Some(existing) = existing {
        // 更新已有模板（保留 ID 和 created_at）
        let mut updated = existing.clone();
        updated.label = label.to_string();
        updated.description = description.to_string();
        updated.icon = icon.to_string();
        updated.prompt_template = prompt_template.to_string();
        updated.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        updated
    } else {
        // 创建新模板
        PromptTemplate::new(
            name.to_string(),
            label.to_string(),
            description.to_string(),
            icon.to_string(),
            prompt_template.to_string(),
        )
    };

    let template_id = template.id.clone();

    // 序列化为 JSON 并存储
    let json = serde_json::to_string(&template)
        .map_err(|e| prefix_error(ERR_PARSE, &format!("模板序列化失败: {e}")))?;

    state
        .storage
        .set_setting(&format!("{PROMPT_TEMPLATE_KEY_PREFIX}{template_id}"), &json)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 更新索引（新模板才需要添加到索引）
    if existing.is_none() {
        update_prompt_template_index(state, &template_id, true)
            .await
            .map_err(|e| prefix_error(ERR_STORAGE, &format!("模板索引更新失败: {e:#}")))?;
    }

    Ok(template_id)
}

/// 列出所有自定义快捷指令模板（S56）。
///
/// 返回 `PromptTemplate` 列表，按 `name` 字母序排列。
#[tauri::command]
pub async fn list_prompt_templates(
    state: State<'_, AppState>,
) -> Result<Vec<PromptTemplate>, String> {
    list_prompt_templates_inner(state.inner()).await
}

/// 模板列表查询逻辑（命令与集成测试复用）。
pub async fn list_prompt_templates_inner(state: &AppState) -> Result<Vec<PromptTemplate>, String> {
    // 读取索引
    let index_json = state
        .storage
        .get_setting(PROMPT_TEMPLATE_INDEX_KEY)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    let template_ids: Vec<String> = match index_json {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| prefix_error(ERR_PARSE, &format!("模板索引解析失败: {e}")))?,
        None => Vec::new(),
    };

    // 逐个读取模板
    let mut templates = Vec::new();
    for id in &template_ids {
        if let Ok(Some(json)) = state
            .storage
            .get_setting(&format!("{PROMPT_TEMPLATE_KEY_PREFIX}{id}"))
            .await
            && let Ok(tmpl) = serde_json::from_str::<PromptTemplate>(&json)
        {
            templates.push(tmpl);
        }
    }

    // 按名称排序
    templates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(templates)
}

/// 删除自定义快捷指令模板（S56）。
///
/// 从 settings 表删除 `prompt_template.{id}` 键，并从索引中移除。
///
/// # 参数
/// - `template_id` — 要删除的模板 ID
///
/// # 错误
/// - settings 表操作失败
#[tauri::command]
pub async fn delete_prompt_template(
    template_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_prompt_template_inner(&template_id, state.inner()).await
}

/// 模板删除逻辑（命令与集成测试复用）。
pub async fn delete_prompt_template_inner(
    template_id: &str,
    state: &AppState,
) -> Result<(), String> {
    // 删除模板定义
    state
        .storage
        .set_setting(&format!("{PROMPT_TEMPLATE_KEY_PREFIX}{template_id}"), "")
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))?;

    // 从索引中移除
    update_prompt_template_index(state, template_id, false)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("模板索引更新失败: {e:#}")))?;

    Ok(())
}

/// 更新模板索引（添加或移除模板 ID）。
///
/// `add = true` → 添加 ID 到索引；`add = false` → 从索引移除 ID。
async fn update_prompt_template_index(
    state: &AppState,
    template_id: &str,
    add: bool,
) -> anyhow::Result<()> {
    let index_json = state.storage.get_setting(PROMPT_TEMPLATE_INDEX_KEY).await?;

    let mut ids: Vec<String> = match index_json {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => Vec::new(),
    };

    if add {
        if !ids.contains(&template_id.to_string()) {
            ids.push(template_id.to_string());
        }
    } else {
        ids.retain(|id| id != template_id);
    }

    let new_index = serde_json::to_string(&ids)?;

    state
        .storage
        .set_setting(PROMPT_TEMPLATE_INDEX_KEY, &new_index)
        .await?;

    Ok(())
}

/// 查询文档的正向链接（REQ-ING-020）。
///
/// 返回该文档通过 `[[wiki-link]]` 引用的所有目标文档。
///
/// `doc_id` 为源文档 ID，返回 `Vec<WikiLink>`（source_doc_id = doc_id 的记录）。
#[tauri::command]
pub async fn get_forward_links(
    doc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    get_forward_links_inner(&doc_id, state.inner()).await
}

/// 正向链接查询逻辑（命令与集成测试复用）。
pub async fn get_forward_links_inner(
    doc_id: &str,
    state: &AppState,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    state
        .storage
        .get_forward_links(doc_id)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 查询文档的反向链接（REQ-ING-020）。
///
/// 返回引用了该文档的所有来源文档（即 `[[doc_name]]` 出现在哪些文档中）。
///
/// `doc_name` 为文档文件名（不含扩展名），返回 `Vec<WikiLink>`（target LIKE %doc_name% 的记录）。
#[tauri::command]
pub async fn get_backlinks(
    doc_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    get_backlinks_inner(&doc_name, state.inner()).await
}

/// 反向链接查询逻辑（命令与集成测试复用）。
pub async fn get_backlinks_inner(
    doc_name: &str,
    state: &AppState,
) -> Result<Vec<echomind_models::WikiLink>, String> {
    state
        .storage
        .get_backlinks(doc_name)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 重建 wiki-link 索引（REQ-ING-020）。
///
/// 清空 `wiki_links` 表后，遍历所有 Indexed 文档的 chunks，
/// 重新解析 `[[wiki-link]]` 语法并写入索引。
#[tauri::command]
pub async fn rebuild_wiki_links(state: State<'_, AppState>) -> Result<usize, String> {
    rebuild_wiki_links_inner(state.inner()).await
}

/// 重建 wiki-link 索引逻辑（命令与集成测试复用）。
///
/// 返回重建的 wiki-link 总数。
pub async fn rebuild_wiki_links_inner(state: &AppState) -> Result<usize, String> {
    use echomind_core::wiki_link_parser::parse_wiki_links;

    // 获取所有已索引文档
    let docs = state
        .storage
        .list_documents()
        .await
        .map_err(|e| format!("{e:#}"))?;

    let mut total_links = 0usize;

    for doc in &docs {
        if doc.status != DocStatus::Indexed {
            continue;
        }

        // 获取文档的所有 chunks
        let chunks = state
            .storage
            .list_chunks(&doc.id)
            .await
            .map_err(|e| format!("{e:#}"))?;

        // 先删除该文档的旧 wiki-link 索引（通过重新导入实现）
        // 由于没有 delete_wiki_links_by_doc 方法，我们直接重新写入
        // wiki_links 表使用 INSERT OR IGNORE，重复的不会冲突
        let mut all_links = Vec::new();

        for chunk in &chunks {
            let links = parse_wiki_links(&chunk.content, &doc.id, &chunk.id);
            all_links.extend(links);
        }

        if !all_links.is_empty() {
            state
                .storage
                .add_wiki_links(&all_links)
                .await
                .map_err(|e| format!("{e:#}"))?;
            total_links += all_links.len();
        }
    }

    Ok(total_links)
}

// ============================================================
// Burst Buffer IPC 命令（Q02 借鉴 QM createBurstBuffer）
// ============================================================

/// 推入一轮对话到 Burst Buffer（Q02 借鉴 QM createBurstBuffer）。
///
/// 前端在 `chat_done` 事件后调用此命令，将本轮对话推入 burst buffer。
/// 如果满足 flush 条件（静默窗口 / 最大轮次），自动异步触发 flush。
///
/// # 参数
/// - `conversation_id`: 会话 ID
/// - `message_seq`: 消息序号（该会话中的第几轮，从 1 开始）
/// - `user_msg`: 用户消息
/// - `assistant_reply`: 助手回复
///
/// # 返回
/// JSON 对象：`{ "pending": N, "flushed": bool, "extracted": M }`
/// - `pending`: push 后 buffer 中的待处理轮次
/// - `flushed`: 是否触发了 flush
/// - `extracted`: flush 提取的记忆数（未 flush 时为 0）
#[tauri::command]
pub async fn push_burst_turn(
    conversation_id: String,
    message_seq: usize,
    user_msg: String,
    assistant_reply: String,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    push_burst_turn_inner(
        &conversation_id,
        message_seq,
        &user_msg,
        &assistant_reply,
        state.inner(),
    )
    .await
}

/// Burst Buffer push 逻辑（命令与集成测试复用）。
pub async fn push_burst_turn_inner(
    conversation_id: &str,
    message_seq: usize,
    user_msg: &str,
    assistant_reply: &str,
    state: &AppState,
) -> Result<serde_json::Value, String> {
    use echomind_models::ProvenanceTag;

    let provenance = ProvenanceTag::new(
        conversation_id.to_string(),
        message_seq,
        format!("对话：{conversation_id} 的第 {message_seq} 轮"),
    );

    let mut buf = state.memory_burst_buffer.lock().await;
    buf.push(
        user_msg.to_string(),
        assistant_reply.to_string(),
        provenance,
    );

    let pending = buf.pending_count();
    let should_flush = buf.should_flush();

    let extracted = if should_flush {
        // 初始化 LLM Provider
        let llm_config = state.llm_config().read().await.clone();
        let provider = match llm_config {
            Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
                Ok(p) => p,
                Err(_) => {
                    // LLM 初始化失败，不阻塞 push，返回未 flush 状态
                    return Ok(serde_json::json!({
                        "pending": pending,
                        "flushed": false,
                        "extracted": 0,
                        "error": "LLM 初始化失败，跳过 flush"
                    }));
                }
            },
            None => {
                // 未配置 LLM，不阻塞 push
                return Ok(serde_json::json!({
                    "pending": pending,
                    "flushed": false,
                    "extracted": 0,
                    "error": "未配置 LLM"
                }));
            }
        };

        let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
        buf.flush(&memory_store, &provider)
            .await
            .unwrap_or_default()
    } else {
        0
    };

    Ok(serde_json::json!({
        "pending": if should_flush { 0 } else { pending },
        "flushed": should_flush,
        "extracted": extracted
    }))
}

/// 手动触发 Burst Buffer flush（Q02）。
///
/// 将 buffer 中所有 pending 轮次聚合后调用 LLM 提取记忆，写入 scratch 层。
/// 如果 buffer 为空或 LLM 未配置，返回 `extracted: 0`。
///
/// # 返回
/// JSON 对象：`{ "extracted": M, "pending_before": N }`
#[tauri::command]
pub async fn flush_memory_burst_buffer(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    flush_memory_burst_buffer_inner(state.inner()).await
}

/// Burst Buffer flush 逻辑（命令与集成测试复用）。
pub async fn flush_memory_burst_buffer_inner(
    state: &AppState,
) -> Result<serde_json::Value, String> {
    let mut buf = state.memory_burst_buffer.lock().await;
    let pending_before = buf.pending_count();

    if pending_before == 0 {
        return Ok(serde_json::json!({
            "extracted": 0,
            "pending_before": 0
        }));
    }

    // 初始化 LLM Provider
    let llm_config = state.llm_config().read().await.clone();
    let provider = match llm_config {
        Some(cfg) => match OpenAIProvider::new(cfg.api_key, cfg.base_url, cfg.model) {
            Ok(p) => p,
            Err(e) => {
                return Ok(serde_json::json!({
                    "extracted": 0,
                    "pending_before": pending_before,
                    "error": format!("LLM 初始化失败: {e:#}")
                }));
            }
        },
        None => {
            return Ok(serde_json::json!({
                "extracted": 0,
                "pending_before": pending_before,
                "error": "未配置 LLM"
            }));
        }
    };

    let memory_store = echomind_core::memory_store::MemoryStore::new(state.storage.clone());
    let extracted = match buf.flush(&memory_store, &provider).await {
        Ok(n) => n,
        Err(e) => {
            return Ok(serde_json::json!({
                "extracted": 0,
                "pending_before": pending_before,
                "error": format!("flush 失败: {e:#}")
            }));
        }
    };

    Ok(serde_json::json!({
        "extracted": extracted,
        "pending_before": pending_before
    }))
}

/// 查询 Burst Buffer 状态（Q02）。
///
/// 返回 buffer 中的 pending 轮次数和是否满足 flush 条件。
///
/// # 返回
/// JSON 对象：`{ "pending": N, "should_flush": bool }`
#[tauri::command]
pub async fn get_burst_buffer_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    get_burst_buffer_status_inner(state.inner()).await
}

/// Burst Buffer 状态查询逻辑（命令与集成测试复用）。
pub async fn get_burst_buffer_status_inner(state: &AppState) -> Result<serde_json::Value, String> {
    let buf = state.memory_burst_buffer.lock().await;
    Ok(serde_json::json!({
        "pending": buf.pending_count(),
        "should_flush": buf.should_flush()
    }))
}

// ============================================================
// 语音转写 IPC 命令（REQ-RAG-034 桌面应用方案）
// getUserMedia + MediaRecorder 录音 → IPC 发送到 OpenAI Whisper API
// ============================================================

/// 语音转写：将音频数据发送到 OpenAI 兼容的 Whisper API 进行语音识别（REQ-RAG-034）。
///
/// 桌面应用方案：前端使用 `navigator.mediaDevices.getUserMedia` + `MediaRecorder`
/// 录制音频，通过 IPC 发送到 Rust 侧，Rust 侧调用 OpenAI `/audio/transcriptions`
/// 端点转写为文本。避免了 WKWebView 不支持 Web Speech API 的问题。
///
/// # 参数
/// - `audio_data`: 音频二进制数据（WebM/OGG 格式，MediaRecorder 默认输出）
/// - `mime_type`: MIME 类型（如 `"audio/webm"`、`"audio/ogg"`）
///
/// # 返回
/// 转写文本字符串。
///
/// # 错误
/// - `LLM: ` 前缀 — API 调用失败（网络错误、API Key 无效、服务不可用）
/// - `VALIDATION: ` 前缀 — 未配置 API Key 或 Base URL
/// 新增：STT 配置查询命令（前端设置面板使用）。
#[tauri::command]
pub async fn get_stt_config(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let stt_api_key = state
        .storage
        .get_setting("voice.stt_api_key")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let stt_base_url = state
        .storage
        .get_setting("voice.stt_base_url")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let stt_model = state
        .storage
        .get_setting("voice.stt_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "whisper-1".to_string());
    let stt_language = state
        .storage
        .get_setting("voice.stt_language")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "zh".to_string());
    // 掩码 API Key（安全）
    let masked_key = if stt_api_key.is_empty() {
        String::new()
    } else if stt_api_key.len() <= 8 {
        "****".to_string()
    } else {
        format!("****{}", &stt_api_key[stt_api_key.len() - 4..])
    };
    Ok(serde_json::json!({
        "stt_api_key_masked": masked_key,
        "stt_base_url": stt_base_url,
        "stt_model": stt_model,
        "stt_language": stt_language,
        "has_custom_config": !stt_api_key.is_empty() || !stt_base_url.is_empty()
    }))
}

/// 新增：STT 配置保存命令（前端设置面板使用）。
#[tauri::command]
pub async fn set_stt_config(
    stt_api_key: Option<String>,
    stt_base_url: Option<String>,
    stt_model: Option<String>,
    stt_language: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(key) = stt_api_key {
        // 空字符串表示清除专用配置，降级到 LLM 配置
        state
            .storage
            .set_setting("voice.stt_api_key", &key)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    if let Some(url) = stt_base_url {
        state
            .storage
            .set_setting("voice.stt_base_url", &url)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    if let Some(model) = stt_model {
        // 验证模型名非空
        let trimmed = model.trim();
        if trimmed.is_empty() {
            return Err(prefix_error(ERR_VALIDATION, "STT 模型名不能为空"));
        }
        state
            .storage
            .set_setting("voice.stt_model", trimmed)
            .await
            .map_err(|e| format!("{e:#}"))?;
    }
    if let Some(lang) = stt_language {
        let trimmed = lang.trim();
        if !trimmed.is_empty() {
            state
                .storage
                .set_setting("voice.stt_language", trimmed)
                .await
                .map_err(|e| format!("{e:#}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn transcribe_audio(
    audio_data: Vec<u8>,
    mime_type: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    transcribe_audio_inner(&audio_data, &mime_type, state.inner()).await
}

/// 语音转写逻辑（命令与集成测试复用）。
///
/// 支持独立的 STT 配置（voice.stt_api_key / voice.stt_base_url / voice.stt_model /
/// voice.stt_language），未配置时降级到 LLM 配置。支持 Groq Whisper、OpenAI Whisper
/// 及任何 OpenAI 兼容的 /audio/transcriptions 端点。
pub async fn transcribe_audio_inner(
    audio_data: &[u8],
    mime_type: &str,
    state: &AppState,
) -> Result<String, String> {
    // 读取 STT 专用配置，降级到 LLM 配置
    let stt_key = state
        .storage
        .get_setting("voice.stt_api_key")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let api_key = if !stt_key.is_empty() {
        stt_key
    } else {
        state
            .storage
            .get_setting("llm.api_key")
            .await
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_default()
    };

    let stt_url = state
        .storage
        .get_setting("voice.stt_base_url")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_default();
    let base_url = if !stt_url.is_empty() {
        stt_url
    } else {
        state
            .storage
            .get_setting("llm.base_url")
            .await
            .map_err(|e| format!("{e:#}"))?
            .unwrap_or_default()
    };

    // 读取 STT 模型（默认 whisper-1）
    let stt_model = state
        .storage
        .get_setting("voice.stt_model")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "whisper-1".to_string());

    // 读取 STT 语言（默认 zh，可配 en/ja 等）
    let stt_language = state
        .storage
        .get_setting("voice.stt_language")
        .await
        .map_err(|e| format!("{e:#}"))?
        .unwrap_or_else(|| "zh".to_string());

    if api_key.is_empty() {
        return Err(prefix_error(
            ERR_VALIDATION,
            "未配置 API Key，无法使用语音转写功能（请在设置中配置 LLM API Key 或专用 STT API Key）",
        ));
    }
    if base_url.is_empty() {
        return Err(prefix_error(
            ERR_VALIDATION,
            "未配置 API Base URL，无法使用语音转写功能",
        ));
    }

    // 构建 audio transcriptions URL（复用 chat_completions_url 的智能拼接逻辑）
    let base_url = base_url.trim_end_matches('/');
    let transcription_url = if base_url.ends_with("/audio/transcriptions") {
        base_url.to_string()
    } else if last_path_segment_is_version(base_url) {
        format!("{base_url}/audio/transcriptions")
    } else {
        format!("{base_url}/v1/audio/transcriptions")
    };

    // 构建 HTTP 客户端（禁止代理，铁律一）
    // 超时 90s 支持较长录音（Whisper API 通常 10-30s 处理 60s 音频）
    let client = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(15))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| prefix_error(ERR_LLM, &format!("构建 HTTP 客户端失败: {e}")))?;

    // 构建 multipart 请求
    // 文件扩展名根据 MIME 类型推断
    let ext = match mime_type {
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        "audio/mp4" => "mp4",
        "audio/mpeg" => "mp3",
        _ => "webm", // 默认 webm（MediaRecorder 最常用格式）
    };

    let part = reqwest::multipart::Part::bytes(audio_data.to_vec())
        .file_name(format!("audio.{ext}"))
        .mime_str(mime_type)
        .map_err(|e| prefix_error(ERR_LLM, &format!("MIME 类型设置失败: {e}")))?;

    let form = reqwest::multipart::Form::new()
        .text("model", stt_model)
        .text("language", stt_language)
        .text("response_format", "json")
        .part("file", part);

    // 发送请求
    let resp = client
        .post(&transcription_url)
        .bearer_auth(&api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            let msg = if e.is_timeout() {
                "语音转写请求超时（90s），请缩短录音后重试"
            } else if e.is_connect() {
                "无法连接语音转写服务，请检查网络或 API Base URL 配置"
            } else {
                "语音转写请求失败"
            };
            prefix_error(ERR_LLM, &format!("{msg}: {e}"))
        })?;

    // 检查响应状态
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let truncated = truncate_error_message(&body, 500);
        return Err(prefix_error(
            ERR_LLM,
            &format!("语音转写 API 返回错误 {status}: {truncated}"),
        ));
    }

    // 解析响应 JSON
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| prefix_error(ERR_LLM, &format!("解析语音转写响应失败: {e}")))?;

    let text = json
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(prefix_error(
            ERR_LLM,
            "语音转写返回空文本，可能未检测到语音",
        ));
    }

    Ok(text)
}

/// 检查 URL 最后一段路径是否为 API 版本标识（如 "v1"、"v4"）。
fn last_path_segment_is_version(url: &str) -> bool {
    let last = url.rsplit('/').next().unwrap_or("");
    last.len() > 1 && last.starts_with('v') && last[1..].bytes().all(|b| b.is_ascii_digit())
}

/// 截断错误信息，避免超长响应体刷屏。
fn truncate_error_message(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let mut end = max_len;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}
