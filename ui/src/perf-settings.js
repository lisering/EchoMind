/**
 * EchoMind 性能优化设置模块（大简化重构后精简版）。
 *
 * 在设置面板中渲染性能优化 section，包含：
 * 1. 智能模式 toggle（统一开关，隐藏高级设置）
 * 2. Contextual Retrieval toggle + 重建按钮
 * 3. BM25 索引重建按钮
 * 4. 全库嵌入重建按钮（嵌入模型切换后使用）
 *
 * 学术 RAG 优化模块已删除：语义缓存、Prompt 压缩、检索记忆、渐进式注入、
 * Speculative RAG、质量门控、Proposition 索引、Summary Tree。
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
  let settings = null;

  try {
    settings = await invoke('get_settings');
  } catch (_) {
    // 静默失败——设置面板首次打开时后端可能未初始化
  }

  let smartModeOn = true;
  try {
    smartModeOn = await smartModeApi.get();
  } catch (_) { /* 默认 true */ }

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

  initPerfHandlers(contextualOn);
}

/**
 * 初始化性能优化设置的事件处理器。
 * @param {boolean} contextualOn - Contextual Retrieval 初始状态
 */
function initPerfHandlers(contextualOn) {
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
