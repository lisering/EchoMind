/**
 * EchoMind 文档导航模块 — 面包屑导航 + 文档列表键盘快捷键。
 *
 * 合并自：breadcrumb.js (REQ-NAV-004) + doc-keyboard.js (REQ-KB-004)
 *
 * 职责：
 * 1. 对话区顶部面包屑导航：知识库 > 会话标题
 * 2. 文档列表键盘导航：Up/Down/Enter/Delete/Cmd+A
 * 3. 点击面包屑知识库名 → 跳转知识库分区
 * 4. 点击会话标题 → 进入重命名编辑模式
 */

// ============================================================
// 导入
// ============================================================

import { $ } from './utils.js';
import { t } from './i18n.js';
import { formatRelativeTime } from './utils.js';
import { invoke } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { docApi } from './ipc.js';

// ============================================================
// 面包屑导航（原 breadcrumb.js）
// ============================================================

let _currentConvId = null;
let _currentConvTitle = '';
let _messageCount = 0;
let _createdAt = 0;
let _onRenameCallback = null;
let _onNavigateKbCallback = null;

/**
 * 渲染面包屑 HTML。
 * @returns {string}
 */
function _renderBreadcrumbHTML() {
  const kbName = t('sidebar.knowledge_base', 'Knowledge Base');
  const convTitle = _currentConvTitle || t('breadcrumb.new_conversation', 'New Chat');
  const metaParts = [];

  if (_messageCount > 0) {
    metaParts.push(`${_messageCount} ${t('breadcrumb.messages', 'messages')}`);
  }
  if (_createdAt > 0) {
    metaParts.push(`${t('breadcrumb.created', 'Created')} ${formatRelativeTime(_createdAt)}`);
  }
  const metaHTML = metaParts.length > 0
    ? `<span class="breadcrumb-meta">${metaParts.join(' · ')}</span>`
    : '';

  return `<div class="breadcrumb-left">
    <span id="breadcrumbKbName" class="breadcrumb-item" role="button" tabindex="0">${kbName}</span>
    <span class="breadcrumb-separator"><svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg></span>
    <span id="breadcrumbConvTitle" class="breadcrumb-title" role="button" tabindex="0">${convTitle}</span>
  </div>
  ${metaHTML}`;
}

/**
 * 更新面包屑显示。
 */
function _updateBreadcrumb() {
  const bar = $('breadcrumbBar');
  if (!bar) return;
  bar.innerHTML = _renderBreadcrumbHTML();

  const kbName = $('breadcrumbKbName');
  if (kbName) {
    kbName.addEventListener('click', () => {
      if (_onNavigateKbCallback) _onNavigateKbCallback();
    });
    kbName.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        if (_onNavigateKbCallback) _onNavigateKbCallback();
      }
    });
  }

  const convTitle = $('breadcrumbConvTitle');
  if (convTitle) {
    convTitle.addEventListener('click', () => _enterRenameMode());
    convTitle.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        _enterRenameMode();
      }
    });
  }
}

/**
 * 进入重命名编辑模式。
 */
function _enterRenameMode() {
  const titleEl = $('breadcrumbConvTitle');
  if (!titleEl) return;

  const currentText = titleEl.textContent || '';
  const input = document.createElement('input');
  input.type = 'text';
  input.value = currentText;
  input.className = 'breadcrumb-title-input';
  input.maxLength = 100;

  titleEl.replaceWith(input);
  input.focus();
  input.select();

  const _finishRename = async () => {
    const newTitle = input.value.trim();
    if (newTitle && newTitle !== currentText) {
      if (_onRenameCallback) {
        _onRenameCallback(newTitle);
      } else if (_currentConvId) {
        try {
          await invoke('create_conversation', { id: _currentConvId, title: newTitle });
        } catch (_) {}
      }
      _currentConvTitle = newTitle;
    }
    _updateBreadcrumb();
  };

  input.addEventListener('blur', _finishRename);
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      input.blur();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      _updateBreadcrumb();
    }
  });
}

/**
 * 初始化面包屑模块。
 * @param {Object} callbacks - 回调函数
 * @param {Function} [callbacks.onRename] - 重命名回调
 * @param {Function} [callbacks.onNavigateKb] - 跳转知识库回调
 */
export function initBreadcrumb(callbacks = {}) {
  _onRenameCallback = callbacks.onRename || null;
  _onNavigateKbCallback = callbacks.onNavigateKb || null;

  let bar = $('breadcrumbBar');
  if (!bar) {
    const chatHeader = document.querySelector('main .flex-1 .relative');
    if (chatHeader) {
      bar = document.createElement('div');
      bar.id = 'breadcrumbBar';
      bar.className = 'breadcrumb-bar';
      chatHeader.insertBefore(bar, chatHeader.firstChild);
    }
  }

  _updateBreadcrumb();
}

/**
 * 更新当前会话信息。
 */
export function updateBreadcrumb(convId, title, messageCount, createdAt) {
  _currentConvId = convId;
  _currentConvTitle = title || '';
  _messageCount = messageCount || 0;
  _createdAt = createdAt || 0;
  _updateBreadcrumb();
}

/**
 * 清空面包屑（空会话状态）。
 */
export function clearBreadcrumb() {
  _currentConvId = null;
  _currentConvTitle = '';
  _messageCount = 0;
  _createdAt = 0;
  _updateBreadcrumb();
}

// ============================================================
// 文档列表键盘快捷键（原 doc-keyboard.js）
// ============================================================

/** 当前键盘选中索引（-1 = 无选中） */
let _kbSelIdx = -1;

/** 是否正在多选模式 */
let _multiSelectMode = false;

/** 选中的文档 ID 集合 */
let _selectedIds = new Set();

/**
 * 初始化文档列表键盘快捷键。
 * @param {Object} handlers
 * @param {() => void} [handlers.onRefresh] - 删除后刷新文档列表
 */
export function initDocKeyboard(handlers = {}) {
  const docList = $('docList');
  if (!docList) return;

  const kbScroll = $('kbDocScroll');
  if (kbScroll && !kbScroll.hasAttribute('tabindex')) {
    kbScroll.setAttribute('tabindex', '0');
  }

  docList.addEventListener('keydown', (e) => {
    const focused = document.activeElement;
    const inDocList = focused === kbScroll || (focused && focused.closest('#docList'));
    if (!inDocList) return;

    const items = _getDocItems();
    if (items.length === 0) return;

    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        _moveSelection(e.shiftKey ? 1 : 1, e.shiftKey);
        break;
      case 'ArrowUp':
        e.preventDefault();
        _moveSelection(-1, e.shiftKey);
        break;
      case 'Enter':
        e.preventDefault();
        _openPreview();
        break;
      case 'Delete':
      case 'Backspace':
        e.preventDefault();
        _deleteSelected(handlers);
        break;
      case 'a':
      case 'A':
        if (e.metaKey || e.ctrlKey) {
          e.preventDefault();
          _selectAll();
        }
        break;
      case 'Escape':
        e.preventDefault();
        _clearSelection();
        break;
    }
  });

  docList.addEventListener('click', (e) => {
    const item = e.target.closest('[data-doc-id]');
    if (item) {
      const items = _getDocItems();
      _kbSelIdx = items.indexOf(item);
      _updateSelectionUI();
    }
  });
}

/**
 * 获取当前渲染的文档项 DOM 列表。
 * @returns {HTMLElement[]}
 */
function _getDocItems() {
  const docList = $('docList');
  if (!docList) return [];
  return Array.from(docList.querySelectorAll('[data-doc-id]'));
}

/**
 * 移动键盘选中位置。
 */
function _moveSelection(delta, shiftKey) {
  const items = _getDocItems();
  if (items.length === 0) return;

  if (_kbSelIdx < 0 || _kbSelIdx >= items.length) {
    _kbSelIdx = delta > 0 ? 0 : items.length - 1;
  } else {
    _kbSelIdx = Math.max(0, Math.min(items.length - 1, _kbSelIdx + delta));
  }

  if (shiftKey && _multiSelectMode) {
    const itemId = items[_kbSelIdx]?.dataset.docId;
    if (itemId) _selectedIds.add(itemId);
  } else if (!shiftKey) {
    _selectedIds.clear();
  }

  _updateSelectionUI();
  _scrollToSelected(items[_kbSelIdx]);
}

/**
 * 更新选中状态的 UI。
 */
function _updateSelectionUI() {
  const items = _getDocItems();
  items.forEach((item, idx) => {
    const isSelected = idx === _kbSelIdx;
    const isChecked = _selectedIds.has(item.dataset.docId);
    if (isSelected) {
      item.classList.add('kb-keyboard-selected');
      item.setAttribute('aria-selected', 'true');
    } else {
      item.classList.remove('kb-keyboard-selected');
      item.removeAttribute('aria-selected');
    }
    if (isChecked) {
      item.classList.add('kb-keyboard-checked');
    } else {
      item.classList.remove('kb-keyboard-checked');
    }
  });
}

/**
 * 滚动到选中项保持可见。
 */
function _scrollToSelected(item) {
  if (!item) return;
  const scrollContainer = $('kbDocScroll');
  if (!scrollContainer) return;

  const itemRect = item.getBoundingClientRect();
  const containerRect = scrollContainer.getBoundingClientRect();

  if (itemRect.top < containerRect.top) {
    scrollContainer.scrollTop -= containerRect.top - itemRect.top;
  } else if (itemRect.bottom > containerRect.bottom) {
    scrollContainer.scrollTop += itemRect.bottom - containerRect.bottom;
  }
}

/**
 * 打开选中文档的预览。
 */
function _openPreview() {
  const items = _getDocItems();
  if (_kbSelIdx < 0 || _kbSelIdx >= items.length) return;
  const item = items[_kbSelIdx];
  if (!item) return;

  const docName = item.dataset.docName || '';
  const docId = item.dataset.docId || '';

  document.dispatchEvent(new CustomEvent('doc-preview-requested', {
    detail: { docId, docName }
  }));
}

/**
 * 删除选中文档（带确认对话框）。
 */
async function _deleteSelected(handlers) {
  const items = _getDocItems();
  if (_kbSelIdx < 0 || _kbSelIdx >= items.length) return;
  const item = items[_kbSelIdx];
  if (!item) return;

  const docId = item.dataset.docId || '';
  const docName = item.dataset.docName || '';
  if (!docId) return;

  const confirmed = await showConfirmDialog({
    title: t('ctx.delete_doc_title', '删除文档'),
    body: t('ctx.delete_doc_confirm', { name: docName }) || `确定删除「${docName}」？此操作将级联删除所有分块和向量数据，不可撤销。`,
    confirmText: t('common.delete', '删除'),
    cancelText: t('common.cancel', '取消'),
    danger: true,
  });

  if (!confirmed) return;

  try {
    await docApi.delete(docId);
    toastSuccess(t('ctx.delete_doc_success', '文档已删除'));
    _kbSelIdx = -1;
    if (handlers.onRefresh) handlers.onRefresh();
    else document.dispatchEvent(new CustomEvent('ctx-refresh-documents'));
  } catch (err) {
    toastError(err);
  }
}

/**
 * 全选文档（仅多选模式下有效）。
 */
function _selectAll() {
  const items = _getDocItems();
  if (items.length === 0) return;

  _multiSelectMode = true;
  _selectedIds.clear();
  items.forEach((item) => {
    const id = item.dataset.docId;
    if (id) _selectedIds.add(id);
  });
  _updateSelectionUI();
  toast(t('ctx.all_selected', `已选 ${items.length} 个文档`), 'info');
}

/**
 * 清除所有选中状态。
 */
function _clearSelection() {
  _kbSelIdx = -1;
  _selectedIds.clear();
  _multiSelectMode = false;
  _updateSelectionUI();
}

/**
 * 获取当前选中的文档 ID 列表。
 * @returns {string[]}
 */
export function getSelectedDocIds() {
  return Array.from(_selectedIds);
}

/**
 * 设置多选模式开关。
 */
export function setMultiSelectMode(enabled) {
  _multiSelectMode = enabled;
  if (!enabled) _selectedIds.clear();
  _updateSelectionUI();
}
