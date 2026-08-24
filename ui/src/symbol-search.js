/**
 * EchoMind 符号搜索面板（REQ-RAG-031 代码感知 RAG 前端 UI）。
 *
 * 功能：
 * 1. 全屏 overlay 面板（类似 graph-viewer.js 模式）
 * 2. 搜索框（输入函数/类名 → search_symbols IPC）
 * 3. 搜索结果列表：符号名 + 类型图标 + 语言 badge + 行号范围 + 签名
 * 4. 重建索引按钮（rebuild_symbol_index IPC，Pro 门控）
 * 5. Focus Trap + ESC 关闭 + i18n
 *
 * 入口：命令面板新增「symbol」命令 或 侧栏按钮
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { Z_INDEX, zClass } from './panel-stack.js';
import { pushPanel, removePanel } from './panel-stack.js';

/** 符号类型 → 图标映射 */
const SYMBOL_ICONS = {
  Function: 'fn',
  Method: 'm',
  Class: 'C',
  Struct: 'S',
  Interface: 'T',
  Enum: 'E',
  Constant: 'c',
  Module: 'M',
};

/** 符号类型 → 颜色映射 */
const SYMBOL_COLORS = {
  Function: '#38bdf8',
  Method: '#38bdf8',
  Class: '#a78bfa',
  Struct: '#a78bfa',
  Interface: '#4ade80',
  Enum: '#facc15',
  Constant: '#fb923c',
  Module: '#94a3b8',
};

/** 符号搜索 Focus Trap 实例 */
let _symbolTrap = null;

/** 搜索防抖定时器 */
let _searchTimer = null;

/**
 * 打开符号搜索面板。
 */
export async function openSymbolSearch() {
  let overlay = $('symbolSearchOverlay');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'symbolSearchOverlay';
    overlay.className = `hidden fixed inset-0 ${zClass(Z_INDEX.PANEL_1)} bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[10vh]`;
    overlay.setAttribute('role', 'dialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.innerHTML = `
      <div class="w-full max-w-xl bg-surface-1 border border-border-strong rounded-lg shadow-modal scale-in overflow-hidden flex flex-col" style="max-height: 70vh">
        <div class="flex items-center justify-between px-5 h-12 border-b border-border-subtle shrink-0">
          <h2 class="text-sm font-semibold text-text-primary" data-i18n="symbol.title"></h2>
          <div class="flex items-center gap-2">
            <button id="symbolRebuildBtn" class="text-xs px-2 py-1 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors" data-i18n="symbol.rebuild_index"></button>
            <button id="symbolCloseBtn" class="text-text-quaternary hover:text-text-secondary transition-colors">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </div>
        </div>
        <div class="p-4 border-b border-border-subtle">
          <input id="symbolSearchInput" type="text" class="w-full px-3 py-2 bg-surface-2 border border-border-default rounded-lg text-sm text-text-primary placeholder-text-quaternary focus:outline-none focus:border-accent transition-colors" placeholder="" autocomplete="off">
        </div>
        <div id="symbolResults" class="overflow-y-auto p-2 flex-1">
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    // 绑定事件
    $('symbolCloseBtn').onclick = closeSymbolSearch;
    $('symbolRebuildBtn').onclick = rebuildSymbolIndex;
    $('symbolSearchInput').addEventListener('input', (e) => {
      const query = e.target.value.trim();
      if (_searchTimer) clearTimeout(_searchTimer);
      _searchTimer = setTimeout(() => doSearch(query), 300);
    });
  }

  // 更新 i18n + placeholder
  overlay.querySelectorAll('[data-i18n]').forEach((el) => {
    el.textContent = t(el.dataset.i18n);
  });
  const input = $('symbolSearchInput');
  if (input) input.placeholder = t('symbol.search_placeholder');

  overlay.classList.remove('hidden');

  // 初始加载所有符号
  await doSearch('');

  // 激活 Focus Trap
  if (_symbolTrap) _symbolTrap.deactivate();
  _symbolTrap = createFocusTrap(overlay);
  _symbolTrap.activate();

  // 聚焦搜索框
  if (input) input.focus();

  // 注册到面板栈（ESC 关闭 + 生命周期追踪）
  pushPanel({ id: 'symbol-search', close: closeSymbolSearch, element: overlay, label: 'Symbol Search' });
}

/**
 * 关闭符号搜索面板。
 */
export function closeSymbolSearch() {
  removePanel('symbol-search');
  const overlay = $('symbolSearchOverlay');
  if (overlay) overlay.classList.add('hidden');
  if (_symbolTrap) {
    _symbolTrap.deactivate();
    _symbolTrap = null;
  }
}

/**
 * 执行符号搜索。
 * @param {string} query - 搜索查询
 */
async function doSearch(query) {
  const container = $('symbolResults');
  if (!container) return;

  try {
    const results = await invoke('search_symbols', { query });
    if (!results || results.length === 0) {
      container.innerHTML = `<p class="text-sm text-text-quaternary text-center py-8">${t('symbol.empty')}</p>`;
      return;
    }

    container.innerHTML = results.map((s) => {
      const icon = SYMBOL_ICONS[s.kind] || '?';
      const color = SYMBOL_COLORS[s.kind] || '#94a3b8';
      return `
        <div class="flex items-center gap-3 px-3 py-2 rounded-lg hover:bg-surface-2 cursor-pointer transition-colors" data-chunk-id="${s.chunk_id}">
          <span class="shrink-0 w-6 h-6 flex items-center justify-center rounded text-[10px] font-mono font-bold" style="background:${color}20;color:${color}">${icon}</span>
          <div class="flex-1 min-w-0">
            <p class="text-sm text-text-primary truncate font-mono">${s.name}</p>
            ${s.signature ? `<p class="text-[10px] text-text-quaternary truncate font-mono">${s.signature}</p>` : ''}
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <span class="text-[10px] px-1.5 py-0.5 rounded bg-surface-3 text-text-quaternary">${s.language}</span>
            <span class="text-[10px] text-text-quaternary">L${s.start_line}-${s.end_line}</span>
          </div>
        </div>
      `;
    }).join('');
  } catch (err) {
    container.innerHTML = `<p class="text-sm text-red-400 text-center py-4">${err}</p>`;
  }
}

/**
 * 重建符号索引。
 */
async function rebuildSymbolIndex() {
  const btn = $('symbolRebuildBtn');
  if (!btn) return;
  const originalText = btn.textContent;
  btn.textContent = t('symbol.rebuilding');
  btn.disabled = true;
  try {
    await invoke('rebuild_symbol_index');
    toastSuccess(t('symbol.rebuild_done'));
  } catch (err) {
    toastError(err);
  } finally {
    btn.textContent = originalText;
    btn.disabled = false;
  }
}

/**
 * 初始化符号搜索面板事件（ESC 关闭）。
 */
export function initSymbolSearch() {
  // ESC 关闭现由 panel-stack 统一管理，此处保留兼容回退
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const overlay = $('symbolSearchOverlay');
      if (overlay && !overlay.classList.contains('hidden')) {
        e.preventDefault();
        e.stopPropagation();
        closeSymbolSearch();
      }
    }
  });
}
