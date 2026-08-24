/**
 * EchoMind 性能优化设置模块（REQ-PERF-001~012 前端 UI）。
 *
 * 在设置面板中渲染性能优化 section，包含：
 * 1. 语义缓存 toggle + 统计展示 + 清空按钮
 * 2. Prompt 压缩比滑块（1.0 禁用 / 2.0 保守 / 3.0 平衡 / 5.0 激进）
 * 3. 检索记忆 toggle + 统计展示 + 重置按钮
 * 4. 渐进式注入 toggle
 * 5. Speculative RAG toggle
 * 6. 质量门控 toggle
 * 7. 知识图谱检索 toggle
 * 8. Contextual Retrieval toggle + 重建按钮
 * 9. 索引重建按钮（BM25 / Proposition / Summary Tree）
 *
 * 设计遵循现有 settings.js 模式：Tailwind 工具类 + 设计令牌 + i18n。
 */

import { $ } from './utils.js';
import { invoke, settingsApi, smartModeApi } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';

/**
 * 渲染性能优化设置区块到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderPerfSettings(container) {
  if (!container) return;

  // 加载当前状态
  let cacheStats = null;
  let cacheSettings = null;
  let compressionRatio = 1.0;
  let retrievalMemoryStats = [];
  let settings = null;

  try {
    cacheStats = await invoke('get_cache_stats');
    cacheSettings = await invoke('get_cache_settings');
    compressionRatio = parseFloat(await settingsApi.get('compression.ratio')) || 1.0;
    retrievalMemoryStats = await invoke('get_retrieval_memory_stats');
    settings = await invoke('get_settings');
  } catch (err) {
    // 静默失败——设置面板首次打开时后端可能未初始化
  }

  const hitRate = cacheStats && cacheStats.total_queries > 0
    ? Math.round(((cacheStats.exact_hits + cacheStats.semantic_hits + cacheStats.retrieval_hits) / cacheStats.total_queries) * 100)
    : 0;

  let smartModeOn = true;
  try {
    smartModeOn = await smartModeApi.get();
  } catch (_) { /* 默认 true */ }

  const progressiveOn = settings?.progressive_injection ?? false;
  const speculativeOn = settings?.speculative_enabled ?? false;
  const qualityGateOn = settings?.quality_gate_enabled ?? false;
  const graphRetrieverOn = settings?.graph_retriever_enabled ?? false;
  const contextualOn = settings?.contextual_retrieval ?? true;

  container.innerHTML = `
    <div class="bg-surface-2 border border-border-default rounded-xl px-4 py-3 space-y-4">
      <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-6">${t('perf.title')}</h4>

      <!-- 智能模式单一开关（S5 审计 P0-1） -->
      <div class="flex items-center justify-between py-2 border-b border-border-subtle">
        <div class="flex-1">
          <div class="text-sm font-medium text-text-primary">${t('perf.smart_mode')}</div>
          <div class="text-xs text-text-quaternary mt-0.5">${t('perf.smart_mode_desc')}</div>
        </div>
        <label class="relative inline-flex items-center cursor-pointer ml-3">
          <input type="checkbox" id="smartModeToggle" class="sr-only peer" ${smartModeOn ? 'checked' : ''}>
          <div class="w-9 h-5 bg-surface-3 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-accent/30 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-s" after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-accent"></div>
        </label>
      </div>

      <div class="space-y-4 ${smartModeOn ? 'hidden' : ''}">
      <!-- 语义缓存 -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.cache_toggle')}</p>
          <p class="text-xs text-text-quaternary">
            ${t('perf.cache_hit_rate')}: ${hitRate}% ·
            ${t('perf.cache_entries')}: ${cacheStats?.cache_size_entries || 0} ·
            ${t('perf.cache_tokens_saved')}: ${(cacheStats?.estimated_token_saved || 0).toLocaleString()}
          </p>
        </div>
        <div class="flex items-center gap-2">
          <button id="perfCacheClear" class="text-xs px-2 py-1 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors">${t('perf.cache_clear')}</button>
          <div id="perfCacheToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="${cacheSettings?.enabled ?? true}">
            <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
          </div>
        </div>
      </div>

      <!-- Prompt 压缩比 -->
      <div>
        <div class="flex items-center justify-between mb-1">
          <p class="text-sm text-text-secondary">${t('perf.compression_label')}</p>
          <span id="perfCompressionValue" class="text-xs text-text-quaternary">${compressionRatio > 1 ? compressionRatio + 'x' : t('perf.compression_off')}</span>
        </div>
        <input id="perfCompressionSlider" type="range" min="1" max="5" step="1" value="${compressionRatio}" class="w-full h-1 bg-slate-600 rounded-lg appearance-none cursor-pointer accent-accent">
        <div class="flex justify-between text-[10px] text-text-quaternary mt-1">
          <span>${t('perf.compression_off')}</span>
          <span>2x</span>
          <span>3x</span>
          <span>5x</span>
        </div>
      </div>

      <!-- 检索记忆 -->
      <div class="flex items-center justify-between">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.memory_toggle')}</p>
          <p class="text-xs text-text-quaternary">${t('perf.memory_stats')}: ${retrievalMemoryStats.length} records</p>
        </div>
        <div class="flex items-center gap-2">
          <button id="perfMemoryReset" class="text-xs px-2 py-1 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors">${t('perf.memory_reset')}</button>
          <div id="perfMemoryToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="false">
            <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
          </div>
        </div>
      </div>

      <!-- 渐进式注入 toggle -->
      <div class="flex items-center justify-between py-2">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.progressive_toggle')}</p>
        </div>
        <div id="perfProgressiveToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="false">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>

      <!-- Speculative RAG toggle -->
      <div class="flex items-center justify-between py-2">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.speculative_toggle')}</p>
        </div>
        <div id="perfSpeculativeToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="false">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>

      <!-- 质量门控 toggle -->
      <div class="flex items-center justify-between py-2">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.quality_gate_toggle')}</p>
          <p class="text-xs text-text-quaternary">${t('perf.quality_gate_desc')}</p>
        </div>
        <div id="perfQualityGateToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="false">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>

      <!-- 知识图谱检索 toggle -->
      <div class="flex items-center justify-between py-2">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.graph_retriever_toggle')}</p>
          <p class="text-xs text-text-quaternary">${t('perf.graph_retriever_desc')}</p>
        </div>
        <div id="perfGraphRetrieverToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="false">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>

      <!-- Contextual Retrieval toggle -->
      <div class="flex items-center justify-between py-2">
        <div>
          <p class="text-sm text-text-secondary">${t('perf.contextual_retrieval_toggle')}</p>
          <p class="text-xs text-text-quaternary">${t('perf.contextual_retrieval_desc')}</p>
        </div>
        <div id="perfContextualToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="true">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>

      <!-- 索引重建按钮组 -->
      <div class="space-y-2">
        <button id="perfRebuildContextualEmbeddings" class="w-full text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors">${t('perf.rebuild_contextual_embeddings')}</button>
        <button id="perfRebuildBM25" class="w-full text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors">${t('perf.rebuild_bm25')}</button>
        <button id="perfRebuildProposition" class="w-full text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors">${t('perf.rebuild_proposition')}</button>
        <button id="perfBuildSummaryTree" class="w-full text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors">${t('perf.build_summary_tree')}</button>
      </div>
      </div>
    </div>
  `;

  // 智能模式 toggle 事件
  const smartModeToggleEl = document.getElementById('smartModeToggle');
  if (smartModeToggleEl) {
    smartModeToggleEl.onchange = async () => {
      const enabled = smartModeToggleEl.checked;
      try {
        await smartModeApi.set(enabled);
        toast(enabled ? t('perf.smart_mode_on') : t('perf.smart_mode_off'), 'success');
        // 重新渲染面板
        await renderPerfSettings(container);
      } catch (err) {
        toastError(err);
        smartModeToggleEl.checked = !enabled;
      }
    };
  }

  initPerfHandlers(
    cacheSettings?.enabled ?? true,
    progressiveOn,
    speculativeOn,
    qualityGateOn,
    graphRetrieverOn,
    contextualOn,
  );
}

/**
 * 初始化性能优化设置的事件处理器。
 * @param {boolean} cacheEnabled - 缓存初始启用状态
 * @param {boolean} progressiveOn - 渐进式注入初始状态
 * @param {boolean} speculativeOn - Speculative RAG 初始状态
 * @param {boolean} qualityGateOn - 质量门控初始状态
 * @param {boolean} graphRetrieverOn - 知识图谱检索初始状态
 * @param {boolean} contextualOn - Contextual Retrieval 初始状态
 */
function initPerfHandlers(cacheEnabled, progressiveOn, speculativeOn, qualityGateOn, graphRetrieverOn, contextualOn) {
  // 缓存 toggle
  const cacheToggle = $('perfCacheToggle');
  if (cacheToggle) {
    updateToggleVisual(cacheToggle, cacheEnabled);
    cacheToggle.onclick = async () => {
      const enabled = cacheToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(cacheToggle, newEnabled);
      try {
        const settings = await invoke('get_cache_settings');
        settings.enabled = newEnabled;
        await invoke('set_cache_settings', { settings });
      } catch (err) {
        updateToggleVisual(cacheToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // 清空缓存
  const cacheClear = $('perfCacheClear');
  if (cacheClear) {
    cacheClear.onclick = async () => {
      try {
        await invoke('clear_cache');
        toastSuccess(t('perf.cache_cleared'));
        await renderPerfSettings($('perfSettingsContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 压缩比滑块
  const slider = $('perfCompressionSlider');
  const valueLabel = $('perfCompressionValue');
  if (slider && valueLabel) {
    slider.oninput = () => {
      const val = parseInt(slider.value, 10);
      valueLabel.textContent = val > 1 ? val + 'x' : t('perf.compression_off');
    };
    slider.onchange = async () => {
      const val = parseFloat(slider.value);
      try {
        await invoke('update_setting', { key: 'compression.ratio', value: String(val) });
        toast(t('perf.compression_label') + ': ' + (val > 1 ? val + 'x' : t('perf.compression_off')), 'success');
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 检索记忆 toggle
  const memToggle = $('perfMemoryToggle');
  if (memToggle) {
    memToggle.onclick = async () => {
      const enabled = memToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(memToggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'rag.retrieval_memory_enabled', value: String(newEnabled) });
      } catch (err) {
        updateToggleVisual(memToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // 重置检索记忆
  const memReset = $('perfMemoryReset');
  if (memReset) {
    memReset.onclick = async () => {
      try {
        await invoke('reset_retrieval_memory');
        toastSuccess(t('perf.memory_reset_done'));
        await renderPerfSettings($('perfSettingsContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 渐进式注入 toggle
  const progToggle = $('perfProgressiveToggle');
  if (progToggle) {
    updateToggleVisual(progToggle, progressiveOn);
    progToggle.onclick = async () => {
      const enabled = progToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(progToggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'rag.progressive_injection', value: String(newEnabled) });
      } catch (err) {
        updateToggleVisual(progToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // Speculative RAG toggle
  const specToggle = $('perfSpeculativeToggle');
  if (specToggle) {
    updateToggleVisual(specToggle, speculativeOn);
    specToggle.onclick = async () => {
      const enabled = specToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(specToggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'rag.speculative_enabled', value: String(newEnabled) });
      } catch (err) {
        updateToggleVisual(specToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // 质量门控 toggle
  const qgToggle = $('perfQualityGateToggle');
  if (qgToggle) {
    updateToggleVisual(qgToggle, qualityGateOn);
    qgToggle.onclick = async () => {
      const enabled = qgToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(qgToggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'rag.quality_gate_enabled', value: String(newEnabled) });
      } catch (err) {
        updateToggleVisual(qgToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // 知识图谱检索 toggle
  const grToggle = $('perfGraphRetrieverToggle');
  if (grToggle) {
    updateToggleVisual(grToggle, graphRetrieverOn);
    grToggle.onclick = async () => {
      const enabled = grToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(grToggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'rag.graph_retriever_enabled', value: String(newEnabled) });
      } catch (err) {
        updateToggleVisual(grToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // Contextual Retrieval toggle
  const ctxToggle = $('perfContextualToggle');
  if (ctxToggle) {
    updateToggleVisual(ctxToggle, contextualOn);
    ctxToggle.onclick = async () => {
      const enabled = ctxToggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateToggleVisual(ctxToggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'rag.contextual_retrieval', value: String(newEnabled) });
      } catch (err) {
        updateToggleVisual(ctxToggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // 索引重建按钮
  const rebuildButtons = [
    { id: 'perfRebuildContextualEmbeddings', cmd: 'rebuild_contextual_embeddings' },
    { id: 'perfRebuildBM25', cmd: 'rebuild_bm25_index' },
    { id: 'perfRebuildProposition', cmd: 'rebuild_proposition_index' },
    { id: 'perfBuildSummaryTree', cmd: 'build_summary_tree' },
  ];
  for (const { id, cmd } of rebuildButtons) {
    const btn = $(id);
    if (btn) {
      btn.onclick = async () => {
        const originalText = btn.textContent;
        btn.textContent = t('perf.rebuilding');
        btn.disabled = true;
        try {
          await invoke(cmd);
          toastSuccess(t('perf.rebuild_done'));
        } catch (err) {
          toastError(err);
        } finally {
          btn.textContent = originalText;
          btn.disabled = false;
        }
      };
    }
  }
}

/**
 * 更新 toggle 开关的视觉状态。
 * @param {HTMLElement} toggle - toggle 元素
 * @param {boolean} enabled - 是否启用
 */
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
