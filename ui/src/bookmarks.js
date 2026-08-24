/**
 * EchoMind 对话书签模块（REQ-RAG-047）。
 *
 * 职责：
 * 1. 渲染侧栏顶部的收藏夹入口（可折叠书签列表）
 * 2. 会话项的书签切换按钮
 * 3. 书签列表项点击跳转到对应会话
 * 4. 右键书签项支持移除书签 / 编辑备注
 *
 * 设计参考：REQ-RAG-047 AC-1~AC-6
 */

import { t } from './i18n.js';
import { invoke } from './ipc.js';
import { icon } from './utils.js';

/**
 * 创建消息级书签导航器（V3.1 P3-5：统一 4 处复制的高亮跳转回调）。
 *
 * 流程：加载会话 → 等待渲染 → 滚动到目标消息 → amber 高亮 3s。
 *
 * @param {{loadConversation: (convId: string, ...args: any[]) => Promise<void>, renderDelayMs?: number, highlightMs?: number}} deps - 依赖注入
 * @returns {(convId: string, messageId?: string|null) => Promise<void>}
 */
export function createBookmarkNavigator({ loadConversation, renderDelayMs = 200, highlightMs = 3000 }) {
  return async function navigateToMessage(convId, messageId) {
    await loadConversation(convId);
    if (!messageId) return;
    setTimeout(() => {
      const chatArea = document.getElementById('chatArea');
      const msgEl = chatArea?.querySelector(`[data-msg-id="${messageId}"]`);
      if (!msgEl) return;
      msgEl.scrollIntoView({ behavior: 'smooth', block: 'center' });
      msgEl.classList.add('ring-2', 'ring-amber-400/50', 'transition-all');
      setTimeout(() => {
        msgEl.classList.remove('ring-2', 'ring-amber-400/50');
      }, highlightMs);
    }, renderDelayMs);
  };
}

/** @type {boolean} 书签面板是否展开 */
let _bookmarkPanelExpanded = false;

/** @type {Array} 书签缓存 */
let _bookmarkCache = null;

/**
 * 初始化书签模块：加载书签列表并渲染面板。
 *
 * @param {Object} callbacks - 回调函数
 * @param {(conversationId: string, ...args: any[]) => void} callbacks.onNavigate - 点击书签跳转到会话
 * @returns {Promise<void>}
 */
export async function initBookmarks(callbacks) {
  await refreshBookmarks(callbacks);
}

/**
 * 刷新书签列表缓存并重新渲染面板。
 *
 * @param {Object} callbacks - 回调函数
 * @param {(conversationId: string, ...args: any[]) => void} [callbacks.onNavigate] - 点击书签跳转
 * @returns {Promise<void>}
 */
export async function refreshBookmarks(callbacks = {}) {
  try {
    _bookmarkCache = await invoke('list_bookmarks');
  } catch {
    _bookmarkCache = [];
  }
  renderBookmarkPanel(callbacks);
}

/**
 * 渲染侧栏顶部的收藏夹面板。
 *
 * @param {Object} callbacks - 回调函数
 * @param {(conversationId: string, ...args: any[]) => void} [callbacks.onNavigate] - 点击书签跳转
 * @returns {void}
 */
export function renderBookmarkPanel(callbacks = {}) {
  const container = document.getElementById('bookmarkPanel');
  if (!container) return;

  container.innerHTML = '';
  container.className = 'bookmark-panel';

  // 收藏夹标题行（可折叠）
  const header = document.createElement('div');
  header.className = 'bookmark-header cursor-pointer select-none px-3 py-2 flex items-center justify-between text-xs text-text-secondary hover:bg-white/5 rounded-lg transition-colors';
  header.onclick = () => {
    _bookmarkPanelExpanded = !_bookmarkPanelExpanded;
    refreshBookmarks(callbacks);
  };

  const headerLeft = document.createElement('div');
  headerLeft.className = 'flex items-center gap-1.5';
  headerLeft.innerHTML = icon('book', 'sm');
  const title = document.createElement('span');
  title.textContent = t('sidebar.bookmark_title') || '收藏夹';
  headerLeft.appendChild(title);

  const count = _bookmarkCache ? _bookmarkCache.length : 0;
  const badge = document.createElement('span');
  badge.className = 'text-text-quaternary text-[10px]';
  badge.textContent = String(count);
  headerLeft.appendChild(badge);
  header.appendChild(headerLeft);

  // 折叠/展开箭头
  const arrow = document.createElement('span');
  arrow.className = 'text-text-quaternary transition-transform';
  arrow.style.transform = _bookmarkPanelExpanded ? 'rotate(90deg)' : 'rotate(0deg)';
  arrow.innerHTML = icon('chevronRight', 'sm');
  header.appendChild(arrow);

  container.appendChild(header);

  // 展开时显示书签列表
  if (_bookmarkPanelExpanded && _bookmarkCache && _bookmarkCache.length > 0) {
    const list = document.createElement('div');
    list.className = 'bookmark-list mt-1 space-y-0.5';

    for (const bm of _bookmarkCache) {
      const item = document.createElement('div');
      item.className = 'bookmark-item group flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs text-text-secondary hover:bg-white/5 cursor-pointer transition-colors';
      item.dataset.convId = bm.conversation_id;
      if (bm.message_id) {
        item.dataset.msgId = bm.message_id;
      }

      const itemIcon = document.createElement('span');
      itemIcon.className = 'shrink-0 text-amber-400';
      itemIcon.innerHTML = icon('book', 'sm');
      item.appendChild(itemIcon);

      const text = document.createElement('span');
      text.className = 'truncate flex-1';

      // 消息级书签显示 summary，会话级显示会话名
      if (bm.message_id && bm.summary) {
        text.textContent = bm.summary;
        text.title = bm.note || bm.summary;
      } else {
        // 尝试从会话列表中获取标题
        const conv = window.__echomindConversations?.find((c) => c.id === bm.conversation_id);
        const titleText = conv?.title || bm.note || bm.conversation_id;
        text.textContent = titleText;
        text.title = bm.note || titleText;
      }
      item.appendChild(text);

      // 移除按钮（hover 显示）
      const removeBtn = document.createElement('button');
      removeBtn.className = 'invisible group-hover:visible text-text-quaternary hover:text-red-400 px-1 transition-opacity';
      removeBtn.innerHTML = icon('close', 'sm');
      removeBtn.title = t('sidebar.bookmark_remove') || '移除书签';
      removeBtn.onclick = async (e) => {
        e.stopPropagation();
        await toggleBookmark(bm.conversation_id, false);
        await refreshBookmarks(callbacks);
      };
      item.appendChild(removeBtn);

      // 点击跳转
      item.onclick = () => {
        if (typeof callbacks.onNavigate === 'function') {
          // 消息级书签传递 messageId 作为第二个参数
          callbacks.onNavigate(bm.conversation_id, bm.message_id || null);
        }
      };

      list.appendChild(item);
    }

    container.appendChild(list);
  }
}

/**
 * 切换会话书签状态。
 *
 * 如果已加书签则移除，否则添加书签（可选备注）。
 *
 * @param {string} conversationId - 会话 ID
 * @param {boolean} [forceRemove] - 强制移除（用于书签项的移除按钮）
 * @param {string} [note] - 书签备注（添加时可选）
 * @returns {Promise<boolean>} 操作后的书签状态（true=已加书签）
 */
export async function toggleBookmark(conversationId, forceRemove, note) {
  try {
    const isBookmarked = await invoke('is_bookmarked', { conversationId });
    if (isBookmarked || forceRemove) {
      await invoke('remove_bookmark', { conversationId });
      return false;
    } else {
      // 会话级书签（向后兼容）：messageId 和 summary 传 null
      await invoke('add_bookmark', { conversationId, note: note || null, messageId: null, summary: null });
      return true;
    }
  } catch {
    return false;
  }
}

/**
 * 检查会话是否已加书签并更新 UI 图标。
 *
 * @param {string} conversationId - 会话 ID
 * @param {HTMLElement} buttonElement - 书签切换按钮元素
 * @returns {Promise<void>}
 */
export async function updateBookmarkIcon(conversationId, buttonElement) {
  try {
    const isBookmarked = await invoke('is_bookmarked', { conversationId });
    if (isBookmarked) {
      buttonElement.classList.add('text-amber-400');
      buttonElement.title = t('sidebar.bookmark_remove') || '移除书签';
    } else {
      buttonElement.classList.remove('text-amber-400');
      buttonElement.title = t('sidebar.bookmark_add') || '添加书签';
    }
  } catch {
    // 静默降级
  }
}

/**
 * 创建书签切换按钮（用于会话列表项）。
 *
 * @param {string} conversationId - 会话 ID
 * @param {Object} callbacks - 回调函数
 * @param {(bookmarked: boolean) => void} [callbacks.onToggle] - 书签状态切换后的回调
 * @returns {HTMLButtonElement} 书签按钮元素
 */
export function createBookmarkButton(conversationId, callbacks = {}) {
  const btn = document.createElement('button');
  btn.className = 'invisible group-hover:visible text-text-quaternary hover:text-amber-400 px-1 transition-opacity';
  btn.innerHTML = icon('book', 'sm');
  btn.title = t('sidebar.bookmark_add') || '添加书签';
  btn.setAttribute('aria-label', t('sidebar.bookmark_add') || '添加书签');

  // 异步加载当前书签状态
  updateBookmarkIcon(conversationId, btn);

  btn.onclick = async (e) => {
    e.stopPropagation();
    const nowBookmarked = await toggleBookmark(conversationId);
    await updateBookmarkIcon(conversationId, btn);
    if (typeof callbacks.onToggle === 'function') {
      callbacks.onToggle(nowBookmarked);
    }
    // 刷新书签面板
    refreshBookmarks({});
  };

  return btn;
}
