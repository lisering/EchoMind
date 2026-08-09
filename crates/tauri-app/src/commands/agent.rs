//! agent 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 切换 Agentic RAG 多步推理开关（REQ-RAG-022）。
///
/// 写入 settings 表 `rag.agent_enabled`，下次 chat 命令调用时即时生效。
/// 启用后，复杂查询触发 ReAct 多步检索（Thought→Action→Observation 循环），
/// 中间推理步骤通过 `agent_step` 事件推送前端。
/// 默认关闭，与标准 RAG 共存。
#[tauri::command]
pub async fn set_agent_enabled(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_agent_enabled_inner(enabled, state.inner()).await
}

/// Agentic RAG 开关写入逻辑（命令与集成测试复用）。
pub async fn set_agent_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.agent_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换多代理协调模式开关（REQ-RAG-025）。
///
/// 写入 settings 表 `rag.coordinator_enabled`，下次 chat 命令调用时即时生效。
/// 启用后，复杂查询触发四阶段协调流程（Research→Synthesis→Implementation→Verification），
/// 中间阶段进度通过 `coordinator_phase` 事件推送前端。
/// 默认关闭，与标准 RAG 和 Agent 模式共存。
#[tauri::command]
pub async fn set_coordinator_mode(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_coordinator_mode_inner(enabled, state.inner()).await
}

/// 多代理协调模式开关写入逻辑（命令与集成测试复用）。
pub async fn set_coordinator_mode_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.coordinator_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 切换子代理舰队模式开关（REQ-RAG-025 扩展）。
///
/// 写入 settings 表 `rag.sub_agent_enabled`，下次 chat 命令调用时即时生效。
/// 启用后，Coordinator 的 Research 阶段使用独立 AgentEngine ReAct 循环处理
/// 每个子查询，通过 mailbox 消息传递协调。
/// **仅在 `rag.coordinator_enabled` 为 true 时生效。**
/// 默认关闭。
#[tauri::command]
pub async fn set_sub_agent_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_sub_agent_enabled_inner(enabled, state.inner()).await
}

/// 子代理舰队开关写入逻辑（命令与集成测试复用）。
pub async fn set_sub_agent_enabled_inner(enabled: bool, state: &AppState) -> Result<(), String> {
    let value = if enabled { "true" } else { "false" };
    state
        .storage
        .set_setting("rag.sub_agent_enabled", value)
        .await
        .map_err(|e| format!("{e:#}"))
}

/// 发射 Agentic RAG 推理步骤事件（REQ-RAG-022 AC-2）。
///
/// 将 `AgentStepInfo` 转换为 `AgentStepPayload` 并通过 `agent_step` 事件推送前端。
pub(crate) fn emit_agent_step<R: Runtime>(app: &AppHandle<R>, step: &AgentStepInfo) {
    let payload = AgentStepPayload {
        step_type: step.step_type.clone(),
        content: step.content.clone(),
        tool: step.tool.clone(),
        input: step.input.clone(),
        iteration: step.iteration,
    };
    if let Err(err) = app.emit("agent_step", payload) {
        warn!("agent_step 事件发射失败: {err}");
    }
}

/// 发射多代理协调阶段进度事件（REQ-RAG-025）。
///
/// 将 `CoordinatorPhaseInfo` 通过 `coordinator_phase` 事件推送前端，
/// 前端据此显示协调阶段进度（🔍 并行检索 → 🧠 综合分析 → ✍️ 生成答案）。
pub(crate) fn emit_coordinator_phase<R: Runtime>(app: &AppHandle<R>, phase: &CoordinatorPhaseInfo) {
    let payload = serde_json::json!({
        "phase": phase.phase,
        "message": phase.message,
        "sub_query_count": phase.sub_query_count,
    });
    if let Err(err) = app.emit("coordinator_phase", payload) {
        warn!("coordinator_phase 事件发射失败: {err}");
    }
}
