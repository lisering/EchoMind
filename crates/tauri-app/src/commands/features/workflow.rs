//! DAG 工作流模板管理（REQ-RAG-030）。
use super::super::*;

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
