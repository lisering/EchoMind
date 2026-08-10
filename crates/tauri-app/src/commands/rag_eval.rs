//! RAG 评估指标 IPC 命令子模块（REQ-RAG-045）。
//!
//! 提供 RAG 评估指标的 IPC 接口，支持单样本和批量评估。
//! LLM 指标使用已配置的 LLM Provider（Remote 或 Local）。

use super::*;
use echomind_core::rag_eval::RagEvaluator;
use echomind_models::{RagEvalReport, RagEvalSample, RagEvalSettings};

/// 评估单个 RAG 响应（REQ-RAG-045）。
///
/// 输入一个 RAG 评估样本（query + answer + contexts + 可选 ground truth），
/// 返回各指标分数。LLM 指标使用已配置的 LLM Provider。
#[tauri::command]
pub async fn evaluate_rag_response(
    sample: RagEvalSample,
    state: State<'_, AppState>,
) -> Result<Vec<echomind_models::RagEvalMetric>, String> {
    evaluate_rag_response_inner(sample, state.inner()).await
}

/// 评估逻辑（命令与集成测试复用）。
pub async fn evaluate_rag_response_inner(
    sample: RagEvalSample,
    state: &AppState,
) -> Result<Vec<echomind_models::RagEvalMetric>, String> {
    let settings = load_eval_settings(state).await;
    let provider = get_llm_provider(state).await?;
    let evaluator = RagEvaluator::with_settings(settings);
    evaluator
        .evaluate(&provider, &sample)
        .await
        .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))
}

/// 批量评估多个 RAG 响应（REQ-RAG-045）。
///
/// 输入多个评估样本，返回聚合报告（各指标平均值 + 每样本明细）。
#[tauri::command]
pub async fn evaluate_rag_batch(
    samples: Vec<RagEvalSample>,
    state: State<'_, AppState>,
) -> Result<RagEvalReport, String> {
    evaluate_rag_batch_inner(samples, state.inner()).await
}

/// 批量评估逻辑（命令与集成测试复用）。
pub async fn evaluate_rag_batch_inner(
    samples: Vec<RagEvalSample>,
    state: &AppState,
) -> Result<RagEvalReport, String> {
    let settings = load_eval_settings(state).await;
    let provider = get_llm_provider(state).await?;
    let evaluator = RagEvaluator::with_settings(settings);
    evaluator
        .evaluate_batch(&provider, &samples)
        .await
        .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))
}

/// 获取 RAG 评估设置（REQ-RAG-045）。
#[tauri::command]
pub async fn get_rag_eval_settings(state: State<'_, AppState>) -> Result<RagEvalSettings, String> {
    let settings = load_eval_settings(state.inner()).await;
    Ok(settings)
}

/// 设置 RAG 评估设置（REQ-RAG-045）。
#[tauri::command]
pub async fn set_rag_eval_settings(
    settings: RagEvalSettings,
    state: State<'_, AppState>,
) -> Result<(), String> {
    set_rag_eval_settings_inner(settings, state.inner()).await
}

/// 设置评估配置的内部实现。
pub async fn set_rag_eval_settings_inner(
    settings: RagEvalSettings,
    state: &AppState,
) -> Result<(), String> {
    let json =
        serde_json::to_string(&settings).map_err(|e| prefix_error(ERR_PARSE, &format!("{e:#}")))?;
    state
        .storage
        .set_setting("rag.eval_settings", &json)
        .await
        .map_err(|e| prefix_error(ERR_STORAGE, &format!("{e:#}")))
}

// ============================================================
// 辅助函数
// ============================================================

/// 从 settings 表加载评估设置，默认值回退。
async fn load_eval_settings(state: &AppState) -> RagEvalSettings {
    match state.storage.get_setting("rag.eval_settings").await {
        Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
        _ => RagEvalSettings::default(),
    }
}

/// 从 AppState 获取 LLM Provider（Remote 或 Local）。
async fn get_llm_provider(state: &AppState) -> Result<LlmProvider, String> {
    let llm_config = state
        .llm_config()
        .read()
        .await
        .clone()
        .ok_or_else(|| prefix_error(ERR_VALIDATION, "未配置 LLM，请完成初始配置向导"))?;

    let llm_mode = state.get_llm_mode().await;
    if llm_mode == LlmMode::Local {
        #[cfg(feature = "pro")]
        {
            let engine = state
                .local_llm()
                .await
                .map_err(|e| prefix_error(ERR_LLM, &format!("本地推理引擎不可用: {e:#}")))?;
            Ok(LlmProvider::Local(engine))
        }
        #[cfg(not(feature = "pro"))]
        {
            let _ = llm_config;
            Err(prefix_error(ERR_PRO_REQUIRED, "本地推理是 Pro 版功能"))
        }
    } else {
        let p = OpenAIProvider::new(
            llm_config.api_key.clone(),
            llm_config.base_url.clone(),
            llm_config.model.clone(),
        )
        .map_err(|e| prefix_error(ERR_LLM, &format!("{e:#}")))?;
        Ok(LlmProvider::Remote(p))
    }
}
