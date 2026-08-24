/**
 * EchoMind 搜索界面模块 — 全局搜索 + 命令面板（合并自 global-search.js + command-palette.js）。
 *
 * 职责：
 * 1. ⌘K 命令面板：模糊搜索一切功能（REQ-KB-002）
 * 2. ⌘⇧F 全局搜索：同时搜索会话/文档/消息（REQ-NAV-002）
 * 3. 关键词高亮、键盘导航、Focus Trap
 *
 * 依赖：state.js / utils.js / i18n.js / panel-stack.js / focus-trap.js / ipc.js
 */

import { getState, setState, get } from './state.js';
import { $, icon } from './utils.js';
import { t } from './i18n.js';
import { invoke } from './ipc.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { createFocusTrap } from './focus-trap.js';
import { isComposingEvent } from './input-utils.js';

// ============================================================
// 命令面板（原 command-palette.js）
// ============================================================

/**
 * 命令对象结构。
 * @typedef {Object} Command
 * @property {string} group - 分组名
 * @property {string} icon - 图标字符
 * @property {string} label - 命令名称
 * @property {string} [shortcut] - 快捷键提示
 * @property {() => void} action - 执行函数
 */

/**
 * 模糊匹配命令：支持中文和分组名搜索。
 * @param {string} query - 搜索关键词
 * @param {Command[]} commands - 全部命令
 * @returns {Command[]} 过滤后的命令列表
 */
export function filterCommands(query, commands) {
  const q = query.trim().toLowerCase();
  if (!q) return [...commands];
  return commands.filter((c) =>
    c.label.toLowerCase().includes(q) ||
    c.group.toLowerCase().includes(q),
  );
}

/**
 * 高亮匹配字符：将匹配部分包裹在 <span> 中。
 * @param {string} label - 原始标签
 * @param {string} query - 搜索关键词
 * @returns {string} 包含高亮 span 的 HTML 字符串
 */
export function highlightMatch(label, query) {
  const q = query.trim().toLowerCase();
  if (!q) return label;
  const pos = label.toLowerCase().indexOf(q);
  if (pos < 0) return label;
  return (
    label.slice(0, pos) +
    '<span class="text-accent font-medium">' +
    label.slice(pos, pos + q.length) +
    '</span>' +
    label.slice(pos + q.length)
  );
}

/**
 * 渲染命令列表到 DOM。
 * @param {Command[]} commands - 全部命令清单
 * @param {string} query - 当前搜索关键词
 */
export function renderCommandList(commands, query) {
  const filtered = filterCommands(query, commands);
  setState({ cmdFiltered: filtered, cmdSelectedIndex: 0 });

  const groups = {};
  filtered.forEach((c) => { (groups[c.group] ||= []).push(c); });

  const list = $('cmdList');
  list.innerHTML = '';

  if (filtered.length === 0) {
    list.innerHTML =
      '<div class="px-4 py-8 text-center text-sm text-text-quaternary">' +
      '<div class="mb-2 opacity-30 flex justify-center">' + icon('search', 'lg') + '</div>' + t('command_palette.no_results') + '</div>';
    return;
  }

  let idx = 0;
  const selectedIdx = get('cmdSelectedIndex');
  for (const [group, cmds] of Object.entries(groups)) {
    const header = document.createElement('div');
    header.className = 'px-4 pt-2 pb-1 text-[11px] uppercase tracking-wider text-text-quaternary';
    header.textContent = group;
    list.appendChild(header);
    for (const c of cmds) {
      const item = document.createElement('div');
      item.setAttribute('role', 'option');
      item.dataset.idx = String(idx);
      const isSel = idx === selectedIdx;
      item.className = `flex items-center gap-3 px-4 h-9 cursor-pointer text-sm transition-colors ${isSel ? 'bg-accent/10 text-text-primary' : 'text-text-secondary hover:bg-surface-3'}`;
      const labelHtml = highlightMatch(c.label, query);
      const shortcutHtml = c.shortcut
        ? `<kbd class="text-[11px] text-text-quaternary bg-surface-3 px-1.5 rounded-xs shrink-0">${c.shortcut}</kbd>`
        : '';
      item.innerHTML = `<span class="text-base shrink-0 w-5 text-center">${c.icon}</span><span class="flex-1 truncate">${labelHtml}</span>${shortcutHtml}`;
      item.onclick = () => { c.action(); closeCommandPalette(); };
      list.appendChild(item);
      idx++;
    }
  }
}

/**
 * 更新命令面板选中项视觉（上下键导航）。
 */
export function updateCmdSelection() {
  const selectedIdx = get('cmdSelectedIndex');
  const items = $('cmdList').querySelectorAll('[role="option"]');
  items.forEach((el, i) => {
    const isSel = i === selectedIdx;
    el.className = `flex items-center gap-3 px-4 h-9 cursor-pointer text-sm transition-colors ${isSel ? 'bg-accent/10 text-text-primary' : 'text-text-secondary hover:bg-surface-3'}`;
    if (isSel) el.scrollIntoView({ block: 'nearest' });
  });
}

/** Focus Trap 实例 */
let _cmdTrap = null;

/** 命令面板内层容器（focus-trap 目标） */
function _getCmdInnerEl() {
  return $('commandPalette').querySelector('.scale-in');
}

/**
 * 打开命令面板。
 */
export function openCommandPalette() {
  if (!$('commandPalette').classList.contains('hidden')) {
    closeCommandPalette();
    return;
  }
  $('commandPalette').classList.remove('hidden');
  $('cmdSearch').value = '';
  renderCommandList(_commands, '');
  const inner = _getCmdInnerEl();
  if (inner) {
    _cmdTrap = createFocusTrap(inner);
    _cmdTrap.activate();
  }
  $('cmdSearch').focus();
  pushPanel({ id: 'command-palette', close: closeCommandPalette, element: $('commandPalette'), label: 'Command Palette' });
}

/**
 * 关闭命令面板。
 */
export function closeCommandPalette() {
  removePanel('command-palette');
  $('commandPalette').classList.add('hidden');
  if (_cmdTrap) {
    _cmdTrap.deactivate();
    _cmdTrap = null;
  }
}

let _commands = [];

/**
 * 初始化命令面板事件监听。
 * @param {Command[]} commands - 命令清单
 */
export function initCommandPalette(commands) {
  _commands = commands;

  $('cmdSearch').addEventListener('input', (e) => {
    renderCommandList(_commands, e.target.value);
  });

  $('cmdSearch').addEventListener('keydown', (e) => {
    const filtered = get('cmdFiltered');
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (filtered.length > 0) {
        setState({ cmdSelectedIndex: (get('cmdSelectedIndex') + 1) % filtered.length });
        updateCmdSelection();
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (filtered.length > 0) {
        const newIdx = (get('cmdSelectedIndex') - 1 + filtered.length) % filtered.length;
        setState({ cmdSelectedIndex: newIdx });
        updateCmdSelection();
      }
    } else if (e.key === 'Enter') {
      if (isComposingEvent(e)) return;
      e.preventDefault();
      const selected = filtered[get('cmdSelectedIndex')];
      if (selected) { selected.action(); closeCommandPalette(); }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeCommandPalette();
    }
  });

  if ($('cmdPaletteClose')) {
    $('cmdPaletteClose').onclick = closeCommandPalette;
  }
}

// ============================================================
// 全局搜索（原 global-search.js）
// ============================================================

/** 每组最大结果数 */
const MAX_PER_GROUP = 5;

/** 搜索防抖延迟（ms） */
const SEARCH_DEBOUNCE_MS = 250;

/** Focus Trap 实例 */
let _gsTrap = null;

/** 防抖计时器 */
let _debounceTimer = null;

/** 当前搜索关键词 */
let _currentQuery = '';

/** 上一次搜索的查询（避免重复搜索） */
let _lastSearchedQuery = '';

/** 外部回调：加载会话 */
let _onLoadConversation = null;

/** 外部回调：打开文档预览 */
let _onOpenDocPreview = null;

/** 外部回调：打开知识图谱查看器 */
let _onOpenGraphViewer = null;

/** 全局搜索内层容器（focus-trap 目标） */
function _getGsInnerEl() {
  return $('globalSearch')?.querySelector('.gs-inner');
}

/**
 * 高亮匹配文本：将匹配部分包裹在 <mark> 中。
 * @param {string} text - 原始文本
 * @param {string} query - 搜索关键词
 * @returns {string} 包含 <mark> 的 HTML 字符串
 */
export function highlightSearchMatch(text, query) {
  const q = query.trim().toLowerCase();
  if (!q || !text) return _escapeHtml(text || '');
  const escaped = _escapeHtml(text);
  const lower = escaped.toLowerCase();
  const pos = lower.indexOf(q.toLowerCase());
  if (pos < 0) return escaped;
  const posOrig = _findMatchPos(text, query);
  if (posOrig < 0) return escaped;
  return (
    _escapeHtml(text.slice(0, posOrig)) +
    '<mark class="bg-accent/20 text-accent rounded-sm px-0.5">' +
    _escapeHtml(text.slice(posOrig, posOrig + query.length)) +
    '</mark>' +
    _escapeHtml(text.slice(posOrig + query.length))
  );
}

function _findMatchPos(text, query) {
  return text.toLowerCase().indexOf(query.toLowerCase());
}

function _escapeHtml(s) {
  if (!s) return '';
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

/**
 * 截取内容摘要。
 * @param {string} content - 完整内容
 * @param {string} query - 搜索关键词
 * @param {number} [maxLen=80] - 最大摘要长度
 * @returns {string} 摘要文本
 */
export function makeSnippet(content, query, maxLen = 80) {
  if (!content) return '';
  const pos = _findMatchPos(content, query);
  if (pos < 0) {
    return content.length > maxLen ? content.slice(0, maxLen) + '…' : content;
  }
  const halfLen = Math.floor(maxLen / 2);
  const start = Math.max(0, pos - halfLen);
  const end = Math.min(content.length, pos + query.length + halfLen);
  const prefix = start > 0 ? '…' : '';
  const suffix = end < content.length ? '…' : '';
  return prefix + content.slice(start, end) + suffix;
}

/**
 * 执行全局搜索。
 * @param {string} query - 搜索关键词
 * @returns {Promise<{messages: Array, documents: Array, entities: Array}>}
 */
export async function executeGlobalSearch(query) {
  const q = query.trim();
  if (!q) {
    return { messages: [], documents: [], entities: [] };
  }

  try {
    const results = await invoke('global_search', { query: q, limit: MAX_PER_GROUP });
    if (!results) return { messages: [], documents: [], entities: [] };

    const messages = (results.messages || []).map((m) => ({
      type: 'message',
      id: m.message_id || m.id || '',
      title: m.conversation_title || t('global_search.untitled'),
      snippet: m.content || '',
      conversationId: m.conversation_id || '',
      messageId: m.message_id || m.id || '',
    }));

    const documents = (results.documents || []).map((d) => {
      const name = (d.file_path || '').split('/').pop() || t('global_search.untitled');
      return {
        type: 'document',
        id: d.doc_id,
        title: name,
        snippet: d.summary || '',
        matchType: d.match_type || 'title',
      };
    });

    const entities = (results.entities || []).map((e) => ({
      type: 'entity',
      id: e.entity_text,
      title: e.entity_text,
      snippet: e.entity_type || '',
      entityText: e.entity_text,
      entityType: e.entity_type || '',
      docId: e.doc_id || '',
    }));

    return { messages, documents, entities };
  } catch (_) {
    return { messages: [], documents: [], entities: [] };
  }
}

/**
 * 渲染搜索结果到 DOM。
 * @param {{messages: Array, documents: Array, entities: Array}} results
 * @param {string} query - 搜索关键词
 */
export function renderSearchResults(results, query) {
  const container = $('globalSearchResults');
  if (!container) return;

  container.innerHTML = '';
  const total = (results.messages?.length || 0) + (results.documents?.length || 0) + (results.entities?.length || 0);

  if (total === 0) {
    const empty = document.createElement('div');
    empty.className = 'gs-empty px-4 py-10 text-center text-sm text-text-quaternary';
    empty.innerHTML = `<div class="mb-2 opacity-30 flex justify-center"><svg class="icon-lg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg></div>${t('global_search.no_results')}`;
    container.appendChild(empty);
    return;
  }

  if (results.messages?.length > 0) {
    container.appendChild(_renderGroup('message', results.messages, query));
  }
  if (results.documents?.length > 0) {
    container.appendChild(_renderGroup('document', results.documents, query));
  }
  if (results.entities?.length > 0) {
    container.appendChild(_renderGroup('entity', results.entities, query));
  }
}

function _renderGroup(type, items, query) {
  const group = document.createElement('div');
  group.className = 'gs-group';

  const header = document.createElement('div');
  header.className = 'px-4 pt-2 pb-1 text-[11px] uppercase tracking-wider text-text-quaternary';
  const labelKey = type === 'message' ? 'global_search.group_messages'
    : type === 'document' ? 'global_search.group_documents'
    : 'global_search.group_entities';
  header.textContent = t(labelKey);
  group.appendChild(header);

  items.forEach((item) => {
    const el = document.createElement('div');
    el.className = 'gs-result-item flex items-center gap-3 px-4 h-10 cursor-pointer text-sm transition-colors hover:bg-surface-3';
    el.dataset.type = type;
    el.dataset.id = item.id;
    el.setAttribute('role', 'option');

    const iconEl = document.createElement('span');
    iconEl.className = 'shrink-0 w-5 flex items-center justify-center';
    const iconSvg = type === 'message'
      ? '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>'
      : type === 'document'
      ? '<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/>'
      : '<circle cx="12" cy="12" r="3"/><circle cx="5" cy="5" r="2"/><circle cx="19" cy="5" r="2"/><circle cx="5" cy="19" r="2"/><circle cx="19" cy="19" r="2"/><line x1="6.5" y1="6.5" x2="10" y2="10"/><line x1="17.5" y1="6.5" x2="14" y2="10"/><line x1="6.5" y1="17.5" x2="10" y2="14"/><line x1="17.5" y1="17.5" x2="14" y2="14"/>';
    iconEl.innerHTML = `<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${iconSvg}</svg>`;
    el.appendChild(iconEl);

    const text = document.createElement('div');
    text.className = 'flex-1 min-w-0';

    const title = document.createElement('div');
    title.className = 'truncate text-text-primary';
    title.innerHTML = highlightSearchMatch(item.title, query);
    text.appendChild(title);

    if (item.snippet) {
      const snippet = document.createElement('div');
      snippet.className = 'truncate text-xs text-text-quaternary mt-0.5';
      snippet.innerHTML = highlightSearchMatch(makeSnippet(item.snippet, query), query);
      text.appendChild(snippet);
    }

    el.appendChild(text);
    el.onclick = () => _handleResultClick(type, item);
    group.appendChild(el);
  });

  return group;
}

function _handleResultClick(type, item) {
  closeGlobalSearch();
  if (type === 'message' && _onLoadConversation && item.conversationId) {
    _onLoadConversation(item.conversationId);
    if (item.messageId) {
      setTimeout(() => _scrollToMessage(item.messageId), 500);
    }
  } else if (type === 'document' && _onOpenDocPreview) {
    _onOpenDocPreview(item.id);
  } else if (type === 'entity' && _onOpenGraphViewer) {
    _onOpenGraphViewer(item.entityText || item.title);
  }
}

function _scrollToMessage(messageId) {
  const msgEl = document.querySelector(`[data-msg-id="${messageId}"]`);
  if (msgEl) {
    msgEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
    msgEl.classList.add('ring-2', 'ring-accent', 'ring-offset-2', 'ring-offset-transparent');
    setTimeout(() => {
      msgEl.classList.remove('ring-2', 'ring-accent', 'ring-offset-2', 'ring-offset-transparent');
    }, 2000);
  }
}

/**
 * 打开全局搜索面板。
 */
export function openGlobalSearch() {
  if (!$('globalSearch')) return;
  if (!$('globalSearch').classList.contains('hidden')) {
    closeGlobalSearch();
    return;
  }
  $('globalSearch').classList.remove('hidden');
  const input = $('globalSearchInput');
  if (input) {
    input.value = '';
    _currentQuery = '';
    _lastSearchedQuery = '';
  }
  const results = $('globalSearchResults');
  if (results) results.innerHTML = '';
  const inner = _getGsInnerEl();
  if (inner) {
    _gsTrap = createFocusTrap(inner);
    _gsTrap.activate();
  }
  if (input) input.focus();
  pushPanel({ id: 'global-search', close: closeGlobalSearch, element: $('globalSearch'), label: 'Global Search' });
}

/**
 * 关闭全局搜索面板。
 */
export function closeGlobalSearch() {
  removePanel('global-search');
  const panel = $('globalSearch');
  if (panel) panel.classList.add('hidden');
  if (_gsTrap) {
    _gsTrap.deactivate();
    _gsTrap = null;
  }
  if (_debounceTimer) {
    clearTimeout(_debounceTimer);
    _debounceTimer = null;
  }
}

/**
 * 初始化全局搜索面板事件监听。
 * @param {Object} callbacks
 */
export function initGlobalSearch(callbacks) {
  _onLoadConversation = callbacks.onLoadConversation || null;
  _onOpenDocPreview = callbacks.onOpenDocPreview || null;
  _onOpenGraphViewer = callbacks.onOpenGraphViewer || null;

  const input = $('globalSearchInput');
  if (!input) return;

  input.addEventListener('input', (e) => {
    _currentQuery = e.target.value;
    if (_debounceTimer) clearTimeout(_debounceTimer);
    _debounceTimer = setTimeout(async () => {
      const query = _currentQuery.trim();
      if (!query) {
        const results = $('globalSearchResults');
        if (results) results.innerHTML = '';
        _lastSearchedQuery = '';
        return;
      }
      if (query === _lastSearchedQuery) return;
      _lastSearchedQuery = query;

      const container = $('globalSearchResults');
      if (container) {
        container.innerHTML = `<div class="px-4 py-8 text-center text-sm text-text-quaternary">${t('global_search.searching')}</div>`;
      }

      try {
        const results = await executeGlobalSearch(query);
        renderSearchResults(results, query);
      } catch (_) {
        if (container) {
          container.innerHTML = `<div class="px-4 py-8 text-center text-sm text-text-quaternary">${t('global_search.search_error')}</div>`;
        }
      }
    }, SEARCH_DEBOUNCE_MS);
  });

  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      _navigateResults(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      _navigateResults(-1);
    } else if (e.key === 'Enter') {
      if (isComposingEvent(e)) return;
      e.preventDefault();
      _executeSelected();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeGlobalSearch();
    }
  });

  const closeBtn = $('globalSearchClose');
  if (closeBtn) {
    closeBtn.onclick = closeGlobalSearch;
  }
}

function _navigateResults(direction) {
  const items = $('globalSearchResults')?.querySelectorAll('.gs-result-item');
  if (!items || items.length === 0) return;

  let currentIdx = -1;
  items.forEach((el, i) => {
    if (el.classList.contains('bg-accent/10')) currentIdx = i;
  });

  let nextIdx;
  if (currentIdx === -1) {
    nextIdx = direction === 1 ? 0 : items.length - 1;
  } else {
    nextIdx = (currentIdx + direction + items.length) % items.length;
  }

  items.forEach((el, i) => {
    el.classList.toggle('bg-accent/10', i === nextIdx);
    el.classList.toggle('text-text-primary', i === nextIdx);
    if (i === nextIdx) el.scrollIntoView({ block: 'nearest' });
  });
}

function _executeSelected() {
  const selected = $('globalSearchResults')?.querySelector('.gs-result-item.bg-accent\\/10');
  if (!selected) return;
  const type = selected.dataset.type;
  const id = selected.dataset.id;
  if (!type || !id) return;

  const titleEl = selected.querySelector('.truncate');
  const title = titleEl?.textContent || '';
  const snippetEl = selected.querySelector('.text-text-quaternary');
  const snippet = snippetEl?.textContent || '';
  const conversationId = selected.dataset.conversationId || '';
  const messageId = selected.dataset.messageId || '';

  _handleResultClick(type, {
    id,
    type: type,
    title,
    snippet,
    conversationId,
    messageId,
  });
}
