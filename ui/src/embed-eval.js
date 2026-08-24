/**
 * EchoMind 嵌入模型对比评估面板（REQ-VEC-018 前端 UI）。
 *
 * 提供嵌入模型对比评估的可视化面板：
 * - 选择 2-3 个嵌入模型进行对比
 * - 使用内置示例数据集或用户自定义数据集
 * - 评估结果表格展示 Hit Rate / MRR / NDCG
 * - SVG 柱状图可视化对比
 * - 评估过程进度展示（逐模型嵌入 + 检索）
 *
 * 使用方式：
 * 1. 设置面板「高级」区域点击「嵌入模型评估」按钮打开面板
 * 2. 勾选要对比的模型 → 选择数据集 → 点击「开始评估」
 * 3. 评估完成后查看表格 + 柱状图对比结果
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { pushPanel, removePanel } from './panel-stack.js';

/** 可用嵌入模型列表 */
const EMBED_MODELS = [
  { id: 'all-MiniLM-L6-v2', label: 'AllMiniLM-L6-v2', dim: 384, desc: '英文通用 384维' },
  { id: 'bge-small-en-v1.5', label: 'BgeSmall-En-v1.5', dim: 384, desc: '英文优化 384维' },
  { id: 'bge-small-zh-v1.5', label: 'BgeSmall-Zh-v1.5', dim: 512, desc: '中文优化 512维' },
  { id: 'e5-small-v2', label: 'E5-Small-v2', dim: 384, desc: '多语言 384维' },
  { id: 'bge-base-en-v1.5', label: 'BgeBase-En-v1.5', dim: 768, desc: '英文强检索 768维' },
  { id: 'bge-m3', label: 'Bge-M3', dim: 1024, desc: '多语言 1024维' },
];

/** 面板 DOM 引用 */
let _overlay = null;

/** 进度事件监听器取消函数 */
let _unlistenProgress = null;

/** 面板 ID 常量 */
const PANEL_ID = 'embed-eval-panel';

/**
 * 打开嵌入模型评估面板。
 */
export async function openEmbedEvalPanel() {
  if (_overlay) {
    _overlay.focus();
    return;
  }

  _overlay = createPanel();
  document.body.appendChild(_overlay);
  pushPanel({ id: PANEL_ID, close: closeEmbedEvalPanel, element: _overlay });

  // 监听进度事件
  setupProgressListener();

  // 加载当前知识库 chunks 数量
  try {
    const stats = await invoke('get_kb_stats');
    const chunkCount = stats?.total_chunks || 0;
    const infoEl = _overlay.querySelector('#embedEvalChunkInfo');
    if (infoEl) {
      infoEl.textContent = chunkCount > 0
        ? t('embed_eval.kb_chunks_available', { count: chunkCount })
        : t('embed_eval.no_chunks');
    }
  } catch (_) {
    // 静默失败
  }
}

/**
 * 关闭面板。
 */
export function closeEmbedEvalPanel() {
  if (_unlistenProgress) {
    _unlistenProgress();
    _unlistenProgress = null;
  }
  if (_overlay) {
    removePanel(PANEL_ID);
    _overlay.remove();
    _overlay = null;
  }
}

/**
 * 创建面板 DOM。
 */
function createPanel() {
  const overlay = document.createElement('div');
  overlay.id = 'embedEvalOverlay';
  overlay.className = 'fixed inset-0 z-[9999] flex items-center justify-center bg-black/50';
  overlay.innerHTML = `
    <div class="bg-surface-1 border border-border-default rounded-2xl w-[680px] max-h-[85vh] overflow-y-auto shadow-2xl">
      <div class="flex items-center justify-between px-5 py-3 border-b border-border-default">
        <h3 class="text-base font-medium text-text-primary">${t('embed_eval.title')}</h3>
        <button id="embedEvalClose" class="text-text-tertiary hover:text-text-primary transition-colors leading-none" aria-label="close">
          <svg class="icon-md" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>
      <div class="px-5 py-4 space-y-4">
        <!-- 知识库信息 -->
        <div class="flex items-center gap-2 text-sm text-text-secondary">
          <svg class="icon-sm text-accent" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
          <span id="embedEvalChunkInfo" class="text-text-tertiary">${t('embed_eval.loading')}</span>
        </div>

        <!-- 模型选择 -->
        <div>
          <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-2">${t('embed_eval.select_models')}</h4>
          <div class="grid grid-cols-2 gap-2" id="embedEvalModelList">
            ${EMBED_MODELS.map(m => `
              <label class="flex items-center gap-2 px-3 py-2 bg-surface-2 border border-border-default rounded-lg cursor-pointer hover:border-accent transition-colors">
                <input type="checkbox" value="${m.id}" class="embed-model-cb accent-accent" />
                <div class="flex-1 min-w-0">
                  <div class="text-sm text-text-primary truncate">${m.label}</div>
                  <div class="text-xs text-text-quaternary">${m.desc}</div>
                </div>
              </label>
            `).join('')}
          </div>
          <p class="text-xs text-text-quaternary mt-1">${t('embed_eval.select_hint')}</p>
        </div>

        <!-- 数据集选择 -->
        <div>
          <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-2">${t('embed_eval.dataset')}</h4>
          <div class="flex items-center gap-3">
            <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
              <input type="radio" name="embedDataset" value="builtin" checked class="accent-accent" />
              ${t('embed_eval.builtin_dataset')}
            </label>
            <label class="flex items-center gap-2 text-sm text-text-secondary cursor-pointer">
              <input type="radio" name="embedDataset" value="custom" class="accent-accent" />
              ${t('embed_eval.custom_dataset')}
            </label>
          </div>
          <textarea id="embedEvalDatasetJson" class="hidden w-full h-24 mt-2 px-3 py-2 bg-surface-2 border border-border-default rounded-lg text-xs text-text-secondary font-mono resize-none" placeholder='{"name":"custom","samples":[{"query":"...","ground_truth":"...","relevant_chunk_ids":["chunk-1"]}]}'></textarea>
        </div>

        <!-- Top-K 设置 -->
        <div class="flex items-center gap-3">
          <label class="text-sm text-text-secondary">${t('embed_eval.top_k')}</label>
          <input type="number" id="embedEvalTopK" value="5" min="1" max="20" class="w-16 px-2 py-1 bg-surface-2 border border-border-default rounded-lg text-sm text-text-primary" />
        </div>

        <!-- 开始评估按钮 -->
        <button id="embedEvalStart" class="w-full py-2.5 bg-accent text-white rounded-lg text-sm font-medium hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed">
          ${t('embed_eval.start')}
        </button>

        <!-- 进度区域 -->
        <div id="embedEvalProgress" class="hidden space-y-2">
          <div class="flex items-center gap-2 text-sm text-text-secondary">
            <div class="animate-spin w-4 h-4 border-2 border-accent border-t-transparent rounded-full"></div>
            <span id="embedEvalProgressText" class="text-text-tertiary"></span>
          </div>
          <div class="w-full h-1.5 bg-surface-3 rounded-full overflow-hidden">
            <div id="embedEvalProgressBar" class="h-full bg-accent rounded-full transition-all duration-300" style="width: 0%"></div>
          </div>
        </div>

        <!-- 结果区域 -->
        <div id="embedEvalResults" class="hidden space-y-4"></div>
      </div>
    </div>
  `;

  // 事件绑定
  overlay.querySelector('#embedEvalClose').onclick = closeEmbedEvalPanel;
  overlay.onclick = (e) => { if (e.target === overlay) closeEmbedEvalPanel(); };

  // 数据集切换
  overlay.querySelectorAll('input[name="embedDataset"]').forEach(radio => {
    radio.onchange = () => {
      const textarea = overlay.querySelector('#embedEvalDatasetJson');
      if (radio.value === 'custom') {
        textarea.classList.remove('hidden');
      } else {
        textarea.classList.add('hidden');
      }
    };
  });

  // 开始评估
  overlay.querySelector('#embedEvalStart').onclick = startEvaluation;

  return overlay;
}

/**
 * 开始评估。
 */
async function startEvaluation() {
  if (!_overlay) return;

  const selectedModels = Array.from(_overlay.querySelectorAll('.embed-model-cb:checked'))
    .map(cb => cb.value);

  if (selectedModels.length < 2) {
    toast(t('embed_eval.need_at_least_2'), 'error');
    return;
  }

  const useCustomDataset = _overlay.querySelector('input[name="embedDataset"]:checked')?.value === 'custom';
  const datasetJson = useCustomDataset
    ? _overlay.querySelector('#embedEvalDatasetJson').value.trim() || null
    : null;

  const topK = parseInt(_overlay.querySelector('#embedEvalTopK').value, 10) || 5;

  // 验证自定义数据集 JSON
  if (datasetJson) {
    try {
      JSON.parse(datasetJson);
    } catch {
      toast(t('embed_eval.invalid_json'), 'error');
      return;
    }
  }

  // 显示进度
  const progressEl = _overlay.querySelector('#embedEvalProgress');
  const resultsEl = _overlay.querySelector('#embedEvalResults');
  const startBtn = _overlay.querySelector('#embedEvalStart');
  progressEl.classList.remove('hidden');
  resultsEl.classList.add('hidden');
  startBtn.disabled = true;

  updateProgress(t('embed_eval.starting'), 0);

  try {
    const request = {
      model_names: selectedModels,
      top_k: topK,
      dataset_json: datasetJson,
    };

    const results = await invoke('run_embed_comparison', { request });

    renderResults(resultsEl, results);
    resultsEl.classList.remove('hidden');
    toastSuccess(t('embed_eval.completed'));
  } catch (err) {
    toastError(err);
    resultsEl.classList.remove('hidden');
    resultsEl.innerHTML = `<p class="text-red-400 text-sm">${String(err).replace(/^.*?:\s*/, '')}</p>`;
  } finally {
    progressEl.classList.add('hidden');
    startBtn.disabled = false;
  }
}

/**
 * 更新进度显示。
 */
function updateProgress(text, percent) {
  if (!_overlay) return;
  const textEl = _overlay.querySelector('#embedEvalProgressText');
  const barEl = _overlay.querySelector('#embedEvalProgressBar');
  if (textEl) textEl.textContent = text;
  if (barEl) barEl.style.width = `${percent}%`;
}

/**
 * 设置进度事件监听器。
 */
function setupProgressListener() {
  if (window.__TAURI__?.event?.listen) {
    window.__TAURI__.event.listen('embed_eval_progress', (event) => {
      const data = /** @type {any} */ (event.payload);
      if (!data || !_overlay) return;

      switch (data.phase) {
        case 'model_started':
          updateProgress(
            t('embed_eval.evaluating_model', { model: data.model, index: data.index + 1, total: data.total }),
            Math.round((data.index / data.total) * 100)
          );
          break;
        case 'embedding_done':
          updateProgress(
            t('embed_eval.embedding_done', { model: data.model, count: data.chunk_count }),
            0 // 进度条在 model_completed 时更新
          );
          break;
        case 'model_completed':
          updateProgress(
            t('embed_eval.model_completed', { model: data.model }),
            Math.round(((data.index ?? 0) + 1) / (data.total ?? 1) * 100)
          );
          break;
        case 'all_completed':
          updateProgress(t('embed_eval.finalizing'), 100);
          break;
      }
    }).then(unlisten => {
      _unlistenProgress = unlisten;
    }).catch(() => {
      // 非阻塞
    });
  }
}

/**
 * 渲染评估结果。
 */
function renderResults(container, results) {
  if (!results || results.length === 0) {
    container.innerHTML = `<p class="text-text-tertiary text-sm">${t('embed_eval.no_results')}</p>`;
    return;
  }

  // 表格
  const tableHtml = `
    <div class="bg-surface-2 border border-border-default rounded-xl overflow-hidden">
      <table class="w-full text-sm">
        <thead>
          <tr class="border-b border-border-default bg-surface-3">
            <th class="px-4 py-2 text-left text-text-secondary font-medium">${t('embed_eval.model')}</th>
            <th class="px-4 py-2 text-right text-text-secondary font-medium">${t('embed_eval.dim')}</th>
            <th class="px-4 py-2 text-right text-text-secondary font-medium">Hit Rate</th>
            <th class="px-4 py-2 text-right text-text-secondary font-medium">MRR</th>
            <th class="px-4 py-2 text-right text-text-secondary font-medium">NDCG</th>
          </tr>
        </thead>
        <tbody>
          ${results.map(r => `
            <tr class="border-b border-border-default last:border-0">
              <td class="px-4 py-2 text-text-primary">${r.model_name}</td>
              <td class="px-4 py-2 text-right text-text-tertiary">${r.dim}</td>
              <td class="px-4 py-2 text-right font-medium" style="color: ${scoreColor(r.metrics.hit_rate)}">${(r.metrics.hit_rate * 100).toFixed(1)}%</td>
              <td class="px-4 py-2 text-right font-medium" style="color: ${scoreColor(r.metrics.mrr)}">${(r.metrics.mrr * 100).toFixed(1)}%</td>
              <td class="px-4 py-2 text-right font-medium" style="color: ${scoreColor(r.metrics.ndcg)}">${(r.metrics.ndcg * 100).toFixed(1)}%</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    </div>
  `;

  // 柱状图
  const chartHtml = renderBarChart(results);

  container.innerHTML = `
    <div class="space-y-4">
      <div>
        <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-2">${t('embed_eval.result_table')}</h4>
        ${tableHtml}
      </div>
      <div>
        <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-2">${t('embed_eval.chart')}</h4>
        ${chartHtml}
      </div>
    </div>
  `;
}

/**
 * 渲染 SVG 柱状图。
 */
function renderBarChart(results) {
  const metrics = ['hit_rate', 'mrr', 'ndcg'];
  const metricLabels = ['Hit Rate', 'MRR', 'NDCG'];
  const colors = ['#3b82f6', '#10b981', '#f59e0b'];

  const barWidth = 60;
  const barGap = 20;
  const groupGap = 40;
  const chartHeight = 200;
  const labelHeight = 30;
  const padding = 20;

  const groupWidth = metrics.length * (barWidth + barGap) - barGap;
  const totalWidth = padding * 2 + results.length * groupWidth + (results.length - 1) * groupGap;
  const totalHeight = chartHeight + labelHeight + padding * 2;

  let bars = '';
  let xLabels = '';

  results.forEach((r, modelIdx) => {
    const groupX = padding + modelIdx * (groupWidth + groupGap);

    metrics.forEach((metric, metricIdx) => {
      const value = r.metrics[metric] || 0;
      const barHeight = Math.max(1, value * chartHeight);
      const barX = groupX + metricIdx * (barWidth + barGap);
      const barY = padding + chartHeight - barHeight;

      bars += `<rect x="${barX}" y="${barY}" width="${barWidth}" height="${barHeight}" fill="${colors[metricIdx]}" rx="2" />`;
      bars += `<text x="${barX + barWidth / 2}" y="${barY - 4}" text-anchor="middle" font-size="10" fill="currentColor" class="text-text-quaternary">${(value * 100).toFixed(0)}%</text>`;
    });

    xLabels += `<text x="${groupX + groupWidth / 2}" y="${padding + chartHeight + 15}" text-anchor="middle" font-size="11" fill="currentColor" class="text-text-secondary">${r.model_name}</text>`;
  });

  // 图例
  let legend = '';
  metricLabels.forEach((label, i) => {
    const lx = padding + i * 100;
    legend += `<rect x="${lx}" y="${totalHeight - 15}" width="12" height="12" fill="${colors[i]}" rx="2" />`;
    legend += `<text x="${lx + 16}" y="${totalHeight - 5}" font-size="11" fill="currentColor" class="text-text-tertiary">${label}</text>`;
  });

  return `
    <div class="bg-surface-2 border border-border-default rounded-xl p-4 overflow-x-auto">
      <svg viewBox="0 0 ${totalWidth} ${totalHeight + 20}" class="w-full h-auto text-text-primary" style="min-width: 400px;">
        <!-- Y 轴网格线 -->
        <line x1="${padding}" y1="${padding}" x2="${totalWidth - padding}" y2="${padding}" stroke="currentColor" stroke-width="0.5" class="text-text-quaternary" opacity="0.3" />
        <line x1="${padding}" y1="${padding + chartHeight / 2}" x2="${totalWidth - padding}" y2="${padding + chartHeight / 2}" stroke="currentColor" stroke-width="0.5" class="text-text-quaternary" opacity="0.3" stroke-dasharray="4 4" />
        <line x1="${padding}" y1="${padding + chartHeight}" x2="${totalWidth - padding}" y2="${padding + chartHeight}" stroke="currentColor" stroke-width="0.5" class="text-text-quaternary" opacity="0.5" />

        <!-- 柱子 -->
        ${bars}

        <!-- X 轴标签 -->
        ${xLabels}

        <!-- 图例 -->
        ${legend}
      </svg>
    </div>
  `;
}

/**
 * 分数颜色映射。
 */
function scoreColor(score) {
  if (score >= 0.8) return '#10b981';
  if (score >= 0.6) return '#f59e0b';
  if (score >= 0.4) return '#f97316';
  return '#ef4444';
}
