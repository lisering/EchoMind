/**
 * EchoMind RAG 评估指标模块（REQ-RAG-045 前端 UI）。
 *
 * 提供 RAG 响应质量评估的可视化面板，展示 RAGAS 风格指标分数：
 * - Faithfulness（答案忠实度）
 * - Answer Relevance（答案相关性）
 * - Context Precision（上下文精度）
 * - Context Recall（上下文召回，需要 GT）
 * - Hit Rate / MRR / NDCG（检索指标）
 * - Keyword Overlap / Context Similarity（纯 Rust 指标）
 *
 * 使用方式：
 * 1. 在聊天消息上添加「评估」按钮，调用 showRagEvalModal()
 * 2. 在性能设置面板中调用 renderRagEvalSettings() 渲染指标开关
 */

import { $ } from './utils.js';
import { ragEvalApi } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';

/** 指标中文名称映射 */
const METRIC_LABELS = {
  faithfulness: '答案忠实度',
  answer_relevance: '答案相关性',
  context_precision: '上下文精度',
  context_recall: '上下文召回',
  hit_rate: '命中率',
  mrr: '平均倒数排名',
  ndcg: 'NDCG 排序质量',
  context_similarity: '上下文相似度',
  keyword_overlap: '关键词重叠率',
};

/** 指标颜色（分数越高越绿） */
function scoreColor(score) {
  if (score >= 0.8) return '#10b981'; // emerald-500
  if (score >= 0.6) return '#f59e0b'; // amber-500
  if (score >= 0.4) return '#f97316'; // orange-500
  return '#ef4444'; // red-500
}

/**
 * 显示 RAG 评估结果模态框。
 *
 * @param {string} query - 用户问题
 * @param {string} answer - LLM 生成的答案
 * @param {Array<string>} contexts - 检索到的上下文片段
 * @param {Array<object>} [sources] - RetrievalResult 数组（用于提取 relevance_scores）
 * @param {string} [groundTruth] - 参考答案（可选，用于 Context Recall）
 */
export async function showRagEvalModal(query, answer, contexts, sources, groundTruth) {
  if (!query || !answer) {
    toast('缺少查询或答案，无法评估', 'error');
    return;
  }

  // 构建评估样本
  const contextTexts = contexts || (sources || []).map(s => s.chunk?.content || '');
  const sample = {
    query,
    answer,
    contexts: contextTexts,
  };

  // 从 sources 提取相关性分数（用于 NDCG）
  if (sources && sources.length > 0) {
    sample.relevance_scores = sources.map(s => s.score || 0);
    sample.relevant_indices = sources
      .map((s, i) => (s.score || 0) > 0.5 ? i : -1)
      .filter(i => i >= 0);
  }

  // 可选 ground truth
  if (groundTruth) {
    sample.ground_truth = groundTruth;
  }

  // 显示加载中模态框
  const modal = createEvalModal('正在评估 RAG 响应质量…');
  document.body.appendChild(modal);

  try {
    const metrics = await ragEvalApi.evaluate(sample);
    renderEvalResults(modal, metrics);
  } catch (err) {
    modal.querySelector('#ragEvalContent').innerHTML =
      `<p class="text-red-400 text-sm">${String(err).replace(/^.*?:\s*/, '')}</p>`;
  }
}

/**
 * 批量评估当前会话的所有问答对。
 *
 * @param {Array<{query: string, answer: string, contexts: Array<string>}>} samples
 */
export async function evaluateBatch(samples) {
  if (!samples || samples.length === 0) {
    toast('没有可评估的问答对', 'error');
    return;
  }

  const modal = createEvalModal(`正在批量评估 ${samples.length} 个问答对…`);
  document.body.appendChild(modal);

  try {
    const report = await ragEvalApi.evaluateBatch(samples);
    renderBatchReport(modal, report);
  } catch (err) {
    modal.querySelector('#ragEvalContent').innerHTML =
      `<p class="text-red-400 text-sm">${String(err).replace(/^.*?:\s*/, '')}</p>`;
  }
}

/**
 * 在性能设置面板中渲染 RAG 评估指标开关。
 * @param {HTMLElement} container - 目标容器
 */
export async function renderRagEvalSettings(container) {
  if (!container) return;

  let settings = null;
  try {
    settings = await ragEvalApi.getSettings();
  } catch {
    // 静默失败
  }

  if (!settings) {
    settings = {
      enable_faithfulness: true,
      enable_answer_relevance: true,
      enable_context_precision: true,
      enable_context_recall: false,
      enable_retrieval_metrics: true,
      enable_embedding_metrics: false,
      enable_keyword_overlap: true,
    };
  }

  container.innerHTML = `
    <div class="bg-surface-2 border border-border-default rounded-xl px-4 py-3 space-y-3">
      <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-2">RAG 评估指标</h4>
      <p class="text-xs text-text-quaternary mb-2">RAGAS 风格指标，评估 RAG 响应质量</p>

      ${evalToggleRow('evalFaithfulness', '答案忠实度 (Faithfulness)', 'LLM 验证答案是否忠实于上下文', settings.enable_faithfulness)}
      ${evalToggleRow('evalAnswerRelevance', '答案相关性 (Relevance)', 'LLM 评估答案是否切题', settings.enable_answer_relevance)}
      ${evalToggleRow('evalContextPrecision', '上下文精度 (Precision)', 'LLM 判断检索上下文是否相关', settings.enable_context_precision)}
      ${evalToggleRow('evalContextRecall', '上下文召回 (Recall)', '需要参考答案', settings.enable_context_recall)}
      ${evalToggleRow('evalRetrievalMetrics', '检索指标 (Hit/MRR/NDCG)', '纯 Rust 计算，无需 LLM', settings.enable_retrieval_metrics)}
      ${evalToggleRow('evalKeywordOverlap', '关键词重叠率', '纯 Rust 计算，无需 LLM', settings.enable_keyword_overlap)}
      ${evalToggleRow('evalEmbeddingMetrics', '嵌入相似度', '需要嵌入向量', settings.enable_embedding_metrics)}
    </div>
  `;

  initEvalSettingsHandlers(settings);
}

// ============================================================
// 内部函数
// ============================================================

/** 创建评估模态框骨架 */
function createEvalModal(loadingText) {
  const modal = document.createElement('div');
  modal.id = 'ragEvalModal';
  modal.className = 'fixed inset-0 z-[9999] flex items-center justify-center bg-black/50';
  modal.innerHTML = `
    <div class="bg-surface-1 border border-border-default rounded-2xl w-[520px] max-h-[80vh] overflow-y-auto shadow-2xl">
      <div class="flex items-center justify-between px-5 py-3 border-b border-border-default">
        <h3 class="text-base font-medium text-text-primary">RAG 评估指标</h3>
        <button id="ragEvalClose" class="text-text-tertiary hover:text-text-primary transition-colors leading-none" aria-label="close"><svg class="icon-md" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div id="ragEvalContent" class="px-5 py-4">
        <div class="flex items-center justify-center py-8">
          <div class="animate-spin w-6 h-6 border-2 border-accent border-t-transparent rounded-full"></div>
          <span class="ml-3 text-sm text-text-tertiary">${loadingText}</span>
        </div>
      </div>
    </div>
  `;
  modal.querySelector('#ragEvalClose').onclick = () => modal.remove();
  modal.onclick = (e) => { if (e.target === modal) modal.remove(); };
  return modal;
}

/** 渲染单次评估结果 */
function renderEvalResults(modal, metrics) {
  const content = modal.querySelector('#ragEvalContent');
  if (!metrics || metrics.length === 0) {
    content.innerHTML = '<p class="text-text-tertiary text-sm">未生成任何指标（可能缺少必要数据）</p>';
    return;
  }

  content.innerHTML = `
    <div class="space-y-3">
      ${metrics.map(m => metricBar(m)).join('')}
    </div>
  `;
}

/** 渲染批量评估报告 */
function renderBatchReport(modal, report) {
  const content = modal.querySelector('#ragEvalContent');
  if (!report || report.sample_count === 0) {
    content.innerHTML = '<p class="text-text-tertiary text-sm">无评估数据</p>';
    return;
  }

  const agg = report.aggregate_metrics || [];
  content.innerHTML = `
    <div class="space-y-4">
      <div class="flex items-center gap-2 text-sm text-text-secondary">
        <span>评估样本数: <strong>${report.sample_count}</strong></span>
      </div>
      <div class="space-y-3">
        ${agg.map(m => metricBar(m)).join('')}
      </div>
      ${report.per_sample_metrics && report.per_sample_metrics.length > 0 ? `
        <details class="mt-4">
          <summary class="text-sm text-text-tertiary cursor-pointer hover:text-text-secondary">展开每样本明细</summary>
          <div class="mt-2 space-y-2 max-h-40 overflow-y-auto">
            ${report.per_sample_metrics.map((metrics, i) => `
              <div class="text-xs text-text-quaternary">
                样本 #${i + 1}: ${metrics.map(m => `${METRIC_LABELS[m.metric_type] || m.metric_type}=${m.score.toFixed(2)}`).join(', ')}
              </div>
            `).join('')}
          </div>
        </details>
      ` : ''}
    </div>
  `;
}

/** 生成单个指标条 */
function metricBar(m) {
  const label = METRIC_LABELS[m.metric_type] || m.metric_type;
  const pct = Math.round((m.score || 0) * 100);
  const color = scoreColor(m.score || 0);
  return `
    <div>
      <div class="flex items-center justify-between mb-1">
        <span class="text-sm text-text-secondary">${label}</span>
        <span class="text-sm font-medium" style="color: ${color}">${pct}%</span>
      </div>
      <div class="w-full h-2 bg-surface-3 rounded-full overflow-hidden">
        <div class="h-full rounded-full transition-all duration-500" style="width: ${pct}%; background-color: ${color}"></div>
      </div>
      ${m.details ? `<p class="text-xs text-text-quaternary mt-1">${m.details}</p>` : ''}
    </div>
  `;
}

/** 生成评估设置 toggle 行 */
function evalToggleRow(id, label, desc, enabled) {
  return `
    <div class="flex items-center justify-between py-1">
      <div>
        <p class="text-sm text-text-secondary">${label}</p>
        <p class="text-xs text-text-quaternary">${desc}</p>
      </div>
      <div id="${id}" class="w-10 h-5 rounded-full cursor-pointer transition-colors shrink-0 ${enabled ? 'bg-accent' : 'bg-slate-600'}" role="switch" aria-checked="${enabled}">
        <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform ${enabled ? 'translate-x-5' : ''}"></span>
      </div>
    </div>
  `;
}

/** 初始化评估设置的事件处理器 */
function initEvalSettingsHandlers(settings) {
  const toggles = [
    { id: 'evalFaithfulness', key: 'enable_faithfulness' },
    { id: 'evalAnswerRelevance', key: 'enable_answer_relevance' },
    { id: 'evalContextPrecision', key: 'enable_context_precision' },
    { id: 'evalContextRecall', key: 'enable_context_recall' },
    { id: 'evalRetrievalMetrics', key: 'enable_retrieval_metrics' },
    { id: 'evalKeywordOverlap', key: 'enable_keyword_overlap' },
    { id: 'evalEmbeddingMetrics', key: 'enable_embedding_metrics' },
  ];

  for (const { id, key } of toggles) {
    const toggle = $(id);
    if (toggle) {
      toggle.onclick = async () => {
        const enabled = toggle.getAttribute('aria-checked') === 'true';
        const newEnabled = !enabled;
        updateToggleVisual(toggle, newEnabled);
        settings[key] = newEnabled;
        try {
          await ragEvalApi.setSettings(settings);
          toastSuccess(newEnabled ? '已启用' : '已禁用');
        } catch (err) {
          updateToggleVisual(toggle, !newEnabled);
          settings[key] = !newEnabled;
          toastError(err);
        }
      };
    }
  }
}

/** 更新 toggle 视觉状态 */
function updateToggleVisual(toggle, enabled) {
  const knob = toggle.querySelector('span');
  if (enabled) {
    toggle.classList.remove('bg-slate-600');
    toggle.classList.add('bg-accent');
    toggle.setAttribute('aria-checked', 'true');
    if (knob) knob.classList.add('translate-x-5');
  } else {
    toggle.classList.add('bg-slate-600');
    toggle.classList.remove('bg-accent');
    toggle.setAttribute('aria-checked', 'false');
    if (knob) knob.classList.remove('translate-x-5');
  }
}
