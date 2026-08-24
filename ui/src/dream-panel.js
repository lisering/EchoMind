/**
 * EchoMind AutoDream 知识库整理面板（AutoDream 前端 UI）。
 *
 * 功能：
 * 1. 全屏 overlay 面板（类似 graph-viewer.js 模式）
 * 2. 触发 dream 分析（trigger_dream IPC）
 * 3. dream_progress 事件监听 + 进度条
 * 4. 建议列表按 severity 分组（High / Medium / Low）
 * 5. 中止按钮（abort_dream IPC）
 * 6. 历史建议查看（get_dream_suggestions IPC）
 * 7. Focus Trap + ESC 关闭 + i18n
 *
 * 入口：侧栏底部「知识库整理」按钮（图标：魔法棒）
 */

import { $ } from './utils.js';
import { invoke, listen } from './ipc.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { toast, toastError } from './toast.js';
import { Z_INDEX, zClass } from './panel-stack.js';
import { pushPanel, removePanel } from './panel-stack.js';

/** Dream 面板 Focus Trap 实例 */
let _dreamTrap = null;

/** dream_progress 事件取消监听器 */
let _unlistenDreamProgress = null;

/** 严重等级颜色映射 */
const SEVERITY_COLORS = {
  high: '#ef4444',
  medium: '#eab308',
  low: '#38bdf8',
};

/**
 * 打开 Dream 面板：创建 overlay + 加载历史建议。
 */
export async function openDreamPanel() {
  let overlay = $('dreamPanelOverlay');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'dreamPanelOverlay';
    overlay.className = 'hidden fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[10vh]';
    overlay.setAttribute('role', 'dialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.innerHTML = `
      <div class="w-full max-w-2xl bg-surface-1 border border-border-strong rounded-lg shadow-modal scale-in overflow-hidden flex flex-col" style="max-height: 80vh">
        <div class="flex items-center justify-between px-5 h-12 border-b border-border-subtle shrink-0">
          <h2 class="text-sm font-semibold text-text-primary" data-i18n="dream.title"></h2>
          <button id="dreamCloseBtn" class="text-text-quaternary hover:text-text-secondary transition-colors">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="overflow-y-auto p-5 space-y-4" id="dreamContent">
          <p class="text-xs text-text-quaternary" data-i18n="dream.description"></p>
          <div class="flex items-center gap-3">
            <button id="dreamTriggerBtn" class="px-4 py-2 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent/90 transition-colors" data-i18n="dream.trigger"></button>
            <button id="dreamAbortBtn" class="hidden px-4 py-2 rounded-lg border border-red-400/40 text-red-300 text-sm hover:bg-red-500/10 transition-colors" data-i18n="dream.abort"></button>
          </div>
          <div id="dreamProgress" class="hidden">
            <div class="w-full h-1.5 bg-surface-3 rounded-full overflow-hidden">
              <div id="dreamProgressBar" class="h-full bg-accent transition-all duration-300" style="width: 0%"></div>
            </div>
            <p id="dreamProgressText" class="text-xs text-text-quaternary mt-1"></p>
          </div>
          <div id="dreamSuggestions" class="space-y-3"></div>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    // 绑定事件
    $('dreamCloseBtn').onclick = closeDreamPanel;
    $('dreamTriggerBtn').onclick = triggerDream;
    $('dreamAbortBtn').onclick = abortDream;
  }

  // 更新 i18n
  overlay.querySelectorAll('[data-i18n]').forEach((el) => {
    el.textContent = t(el.dataset.i18n);
  });

  overlay.classList.remove('hidden');

  // 加载历史建议
  await loadDreamSuggestions();

  // 激活 Focus Trap
  if (_dreamTrap) _dreamTrap.deactivate();
  _dreamTrap = createFocusTrap(overlay);
  _dreamTrap.activate();

  // 注册到面板栈（ESC 关闭 + 生命周期追踪）
  pushPanel({ id: 'dream-panel', close: closeDreamPanel, element: overlay, label: 'Dream Panel' });

  // 监听 dream_progress 事件
  if (_unlistenDreamProgress) {
    _unlistenDreamProgress();
  }
  _unlistenDreamProgress = await listen('dream_progress', (event) => {
    const data = event.payload || {};
    const progressEl = $('dreamProgress');
    const barEl = $('dreamProgressBar');
    const textEl = $('dreamProgressText');
    if (progressEl) progressEl.classList.remove('hidden');
    if (barEl) barEl.style.width = Math.round((data.progress || 0) * 100) + '%';
    if (textEl) {
      const phaseKey = 'dream.progress_' + (data.phase || 'scanning');
      textEl.textContent = t(phaseKey) !== phaseKey ? t(phaseKey) : (data.message || '');
    }
    if (data.phase === 'complete' || data.phase === 'aborted') {
      const triggerBtn = $('dreamTriggerBtn');
      const abortBtn = $('dreamAbortBtn');
      if (triggerBtn) triggerBtn.classList.remove('hidden');
      if (abortBtn) abortBtn.classList.add('hidden');
      if (data.phase === 'complete') {
        loadDreamSuggestions();
      }
    }
  });
}

/**
 * 关闭 Dream 面板。
 */
export function closeDreamPanel() {
  removePanel('dream-panel');
  const overlay = $('dreamPanelOverlay');
  if (overlay) overlay.classList.add('hidden');
  if (_dreamTrap) {
    _dreamTrap.deactivate();
    _dreamTrap = null;
  }
  if (_unlistenDreamProgress) {
    _unlistenDreamProgress();
    _unlistenDreamProgress = null;
  }
}

/**
 * 触发 Dream 分析。
 */
async function triggerDream() {
  const triggerBtn = $('dreamTriggerBtn');
  const abortBtn = $('dreamAbortBtn');
  const progressEl = $('dreamProgress');
  const suggestionsEl = $('dreamSuggestions');

  if (triggerBtn) triggerBtn.classList.add('hidden');
  if (abortBtn) abortBtn.classList.remove('hidden');
  if (progressEl) progressEl.classList.remove('hidden');
  if (suggestionsEl) suggestionsEl.innerHTML = '';

  try {
    await invoke('trigger_dream');
  } catch (err) {
    toastError(err);
    if (triggerBtn) triggerBtn.classList.remove('hidden');
    if (abortBtn) abortBtn.classList.add('hidden');
    if (progressEl) progressEl.classList.add('hidden');
  }
}

/**
 * 中止 Dream 分析。
 */
async function abortDream() {
  try {
    await invoke('abort_dream');
    toast(t('dream.progress_aborted'), 'info');
  } catch (err) {
    toastError(err);
  }
}

/**
 * 加载并渲染 Dream 建议。
 */
async function loadDreamSuggestions() {
  const container = $('dreamSuggestions');
  if (!container) return;

  try {
    const result = await invoke('get_dream_suggestions');
    const suggestions = Array.isArray(result) ? result : (result?.suggestions || []);

    if (suggestions.length === 0) {
      container.innerHTML = `<p class="text-sm text-text-quaternary text-center py-8">${t('dream.empty')}</p>`;
      return;
    }

    // 按 severity 分组
    const groups = { high: [], medium: [], low: [] };
    for (const s of suggestions) {
      const sev = (s.severity || 'low').toLowerCase();
      if (groups[sev]) groups[sev].push(s);
      else groups.low.push(s);
    }

    let html = '';
    for (const severity of ['high', 'medium', 'low']) {
      if (groups[severity].length === 0) continue;
      const sevKey = 'dream.severity_' + severity;
      const color = SEVERITY_COLORS[severity];
      html += `<div class="space-y-2">`;
      html += `<div class="flex items-center gap-2"><span class="w-2 h-2 rounded-full" style="background:${color}"></span><span class="text-xs font-medium text-text-secondary">${t(sevKey)}</span></div>`;
      for (const s of groups[severity]) {
        const typeKey = 'dream.suggestion_' + (s.suggestion_type || 'organization');
        const typeLabel = t(typeKey) !== typeKey ? t(typeKey) : s.suggestion_type;
        html += `
          <div class="border-l-2 pl-3 py-2" style="border-color:${color}">
            <p class="text-sm text-text-primary font-medium">${s.title || typeLabel}</p>
            <p class="text-xs text-text-tertiary mt-1">${s.description || ''}</p>
            ${s.doc_names && s.doc_names.length > 0 ? `<div class="flex flex-wrap gap-1 mt-2">${s.doc_names.map(n => `<span class="text-[10px] px-1.5 py-0.5 rounded bg-surface-3 text-text-quaternary">${n}</span>`).join('')}</div>` : ''}
            ${s.similarity != null ? `<p class="text-[10px] text-text-quaternary mt-1">Similarity: ${(s.similarity * 100).toFixed(0)}%</p>` : ''}
          </div>
        `;
      }
      html += `</div>`;
    }
    container.innerHTML = html;
  } catch (err) {
    container.innerHTML = `<p class="text-sm text-red-400">${err}</p>`;
  }
}

/**
 * 初始化 Dream 面板事件（ESC 关闭）。
 */
export function initDreamPanel() {
  // ESC 关闭现由 panel-stack 统一管理，此处保留兼容回退
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const overlay = $('dreamPanelOverlay');
      if (overlay && !overlay.classList.contains('hidden')) {
        e.preventDefault();
        e.stopPropagation();
        closeDreamPanel();
      }
    }
  });
}
