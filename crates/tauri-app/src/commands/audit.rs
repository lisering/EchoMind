//! audit 域 IPC 命令子模块（从 commands.rs 拆分，纯重构）。
use super::*;

/// 取消审计：触发指定文档的审计取消标志，审计循环检测后 break 并输出部分报告。
#[tauri::command]
pub async fn abort_audit(doc_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.abort_audit(&doc_id).await;
    Ok(())
}

#[cfg(not(feature = "pro"))]
#[tauri::command]
pub async fn audit_document(
    _app: AppHandle,
    _doc_id: String,
    _doc_name: String,
    _state: State<'_, AppState>,
) -> Result<(), String> {
    Err("文档审计是 Pro 版功能，请使用 Pro 版本".to_string())
}

#[cfg(feature = "pro")]
#[tauri::command]
pub async fn audit_document(
    app: AppHandle,
    doc_id: String,
    doc_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let result = audit_document_inner(&app, &doc_id, &doc_name, state.inner()).await;
    if let Err(err) = &result {
        emit_chat_error(&app, err.clone());
    }
    result
}

/// 审计逻辑（命令与集成测试复用）：
/// Pro 拦截 → 配置检查 → 引擎初始化 → 全量审计 → 报告流式推送。
#[cfg(feature = "pro")]
pub async fn audit_document_inner<R: Runtime>(
    app: &AppHandle<R>,
    doc_id: &str,
    doc_name: &str,
    state: &AppState,
) -> Result<(), String> {
    // Pro 前置拦截（REQ-AUDIT-001：Pro 版功能）
    let is_pro = *state.is_pro().read().await;
    if !is_pro {
        return Err("文档审计是 Pro 版功能，请先激活 License".to_string());
    }

    let llm_config = state
        .llm_config()
        .read()
        .await
        .clone()
        .ok_or_else(|| "未配置 LLM：请完成初始配置向导".to_string())?;

    // 阶段 1：初始化向量化引擎（SIFiD 预筛需要 embedding）
    emit_audit_phase(app, "extracting", "初始化向量化引擎…");
    let embedder = state
        .embedder()
        .await
        .map_err(|e| format!("向量化引擎不可用: {e:#}"))?
        .clone();

    let provider = OpenAIProvider::new(
        llm_config.api_key.clone(),
        llm_config.base_url.clone(),
        llm_config.model.clone(),
    )
    .map_err(|e| format!("{e:#}"))?;

    let engine = AuditEngine::new(embedder, state.storage.clone(), provider);
    let cancel = state.audit_cancel_for(doc_id).await;

    // 阶段 2：执行审计（Decompose → Verify → Report）
    emit_audit_phase(app, "comparing", "正在比对声明…");
    let outcome = engine
        .audit(doc_id, doc_name, cancel)
        .await
        .map_err(|e| format!("{e:#}"))?;

    // 阶段 3：生成报告并流式推送
    emit_audit_phase(app, "reporting", "正在生成报告…");
    let (report, cancelled) = match outcome {
        AuditOutcome::Completed { report } => (report, false),
        AuditOutcome::NoChunks => {
            emit_chat_token(app, "文档无内容（0 个分块），无法执行审计。".to_string());
            emit_chat_done(app, None);
            state.clear_audit_cancel(doc_id).await;
            return Ok(());
        }
        AuditOutcome::Cancelled { partial_report } => {
            emit_chat_error(app, "⏹ 审计已中断".to_string());
            (partial_report, true)
        }
    };

    // 将报告转为 Markdown 并流式推送（复用 chat_token 渲染链路）
    let markdown = report_to_markdown(&report);
    emit_chat_token(app, markdown);
    emit_chat_done(app, None);

    state.clear_audit_cancel(doc_id).await;
    let _ = cancelled; // cancelled 标记保留用于未来扩展（如日志记录）
    Ok(())
}

/// 发射审计阶段事件（REQ-AUDIT-005）：在报告生成前推送进度。
#[cfg(feature = "pro")]
fn emit_audit_phase<R: Runtime>(app: &AppHandle<R>, phase: &str, message: &str) {
    let payload = ChatPhasePayload {
        phase: phase.to_string(),
        message: message.to_string(),
    };
    if let Err(err) = app.emit("audit_phase", payload) {
        warn!("audit_phase 事件发射失败: {err}");
    }
}

/// 将审计报告转换为 Markdown 格式（REQ-AUDIT-004）。
#[cfg(feature = "pro")]
fn report_to_markdown(report: &AuditReport) -> String {
    let mut md = String::new();

    // 摘要段
    md.push_str(&format!(
        "## 📋 文档审计报告\n\n\
         **文档**：《{}》  \n\
         **分块数**：{}  \n\
         **提取声明**：{} 条  \n\
         **发现矛盾**：{} 处  \n\
         **耗时**：{} ms\n\n",
        report.doc_name,
        report.total_chunks,
        report.total_claims,
        report
            .contradictions
            .iter()
            .filter(|c| c.verdict == echomind_core::audit::Verdict::Contradiction)
            .count(),
        report.elapsed_ms,
    ));

    // 矛盾清单
    let contradictions: Vec<&ContradictionPair> = report
        .contradictions
        .iter()
        .filter(|c| c.verdict == echomind_core::audit::Verdict::Contradiction)
        .collect();

    if contradictions.is_empty() {
        md.push_str("✅ **未发现明显矛盾**\n\n");
    } else {
        md.push_str("### 🔍 矛盾清单\n\n");
        for (i, pair) in contradictions.iter().enumerate() {
            let severity_icon = match pair.severity {
                Severity::High => "🔴",
                Severity::Medium => "🟡",
                Severity::Low => "🟢",
            };
            md.push_str(&format!(
                "#### {} {}. {} 矛盾\n\n\
                 **声明 A**（片段 {}）：{}  \n\
                 **声明 B**（片段 {}）：{}  \n\
                 **说明**：{}\n\n",
                severity_icon,
                i + 1,
                pair.pair_id,
                pair.claim_a.sequence,
                pair.claim_a.text,
                pair.claim_b.sequence,
                pair.claim_b.text,
                pair.explanation,
            ));
        }
    }

    // 免责声明
    md.push_str("---\n\n> ⚠️ 本报告由 AI 辅助生成，矛盾判定可能存在遗漏或误报，请人工核实。");

    md
}
