/**
 * EchoMind 聊天工具模块 — 滚动锁定 + 排队发送。
 *
 * 合并自：scroll-lock.js (TC-QA-002) + queue-send.js (AC-QA-012)
 *
 * 职责：
 * 1. 监听 chatArea 滚动事件，检测用户是否滚离底部
 * 2. 用户在底部时：新 token 到达自动滚动到底部
 * 3. 用户滚向上方时：锁定滚动位置，不自动拽回
 * 4. 显示/隐藏「回到底部」浮动按钮
 * 5. 流式期间允许用户输入并排队问题
 * 6. 队列管理（入队/出队/清空/查询大小/查看队列）
 */

// ============================================================
// 导入
// ============================================================

import { get, setState } from './state.js';
import { $ } from './utils.js';
import { t } from './i18n.js';
import { updateInputUI } from './action.js';

// ============================================================
// 滚动锁定（原 scroll-lock.js）
// ============================================================

let _userScrolledUp = false;
let _scrollCleanup = null;
const SCROLL_THRESHOLD = 100;
const SCROLL_TO_TOP_THRESHOLD = 200;
let _hasNewMessages = false;
let _programmaticScroll = false;
let _programmaticScrollTimer = null;

/**
 * 初始化智能滚动锁定监听器。
 */
export function initScrollLock(chatArea) {
  if (!chatArea) return () => {};

  if (_scrollCleanup) _scrollCleanup();

  const onScroll = () => {
    const distFromBottom = chatArea.scrollHeight - chatArea.scrollTop - chatArea.clientHeight;
    const distFromTop = chatArea.scrollTop;
    if (_programmaticScroll && distFromBottom > SCROLL_THRESHOLD) return;
    _userScrolledUp = distFromBottom > SCROLL_THRESHOLD;

    const backToTopBtn = document.getElementById('backToTopBtn');
    if (backToTopBtn) {
      if (distFromTop > SCROLL_TO_TOP_THRESHOLD) {
        backToTopBtn.classList.remove('hidden');
      } else {
        backToTopBtn.classList.add('hidden');
      }
    }

    if (distFromBottom < SCROLL_THRESHOLD) {
      _hasNewMessages = false;
      const newMsgBtn = document.getElementById('newMsgBtn');
      if (newMsgBtn) newMsgBtn.classList.add('hidden');
    }
  };

  chatArea.addEventListener('scroll', onScroll, { passive: true });

  _scrollCleanup = () => {
    chatArea.removeEventListener('scroll', onScroll);
    _userScrolledUp = false;
  };

  return _scrollCleanup;
}

export function isUserScrolledUp() {
  return _userScrolledUp;
}

export function shouldAutoScroll() {
  return !_userScrolledUp;
}

export function beginProgrammaticScroll() {
  _programmaticScroll = true;
  if (_programmaticScrollTimer) clearTimeout(_programmaticScrollTimer);
  _programmaticScrollTimer = setTimeout(() => {
    _programmaticScroll = false;
    _programmaticScrollTimer = null;
  }, 500);
}

export function resetScrollLock() {
  _userScrolledUp = false;
}

export function notifyNewMessage() {
  if (_userScrolledUp) {
    _hasNewMessages = true;
    const newMsgBtn = document.getElementById('newMsgBtn');
    if (newMsgBtn) newMsgBtn.classList.remove('hidden');
  }
}

export function scrollToTop() {
  const chatArea = document.getElementById('chatArea');
  if (chatArea) {
    chatArea.scrollTo({ top: 0, behavior: 'smooth' });
  }
}

export function createBackToTopButton() {
  const btn = document.createElement('button');
  btn.id = 'backToTopBtn';
  btn.className = 'back-to-top hidden';
  btn.setAttribute('aria-label', t('chat.scroll_to_top', '回到顶部'));
  btn.innerHTML = `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 15 12 9 6 15"/></svg>`;

  btn.onclick = () => {
    const chatArea = document.getElementById('chatArea');
    if (chatArea) chatArea.scrollTo({ top: 0, behavior: 'smooth' });
    btn.classList.add('hidden');
  };

  return btn;
}

export function destroyScrollLock() {
  if (_scrollCleanup) {
    _scrollCleanup();
    _scrollCleanup = null;
  }
  _userScrolledUp = false;
}

export function createJumpToLatestButton() {
  const btn = document.createElement('button');
  btn.className = 'jump-to-latest';
  btn.style.display = 'none';
  btn.setAttribute('aria-label', t('chat.scroll_to_bottom', '回到底部'));
  btn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg><span>${t('chat.scroll_to_bottom', '回到底部')}</span>`;

  btn.onclick = () => {
    const chatArea = document.getElementById('chatArea');
    if (chatArea) chatArea.scrollTo({ top: chatArea.scrollHeight, behavior: 'smooth' });
    resetScrollLock();
    btn.style.display = 'none';
  };

  return btn;
}

// ============================================================
// 排队发送（原 queue-send.js）
// ============================================================

let _queue = [];

export function enqueueQuery(query) {
  if (!query || !query.trim()) return _queue.length;
  _queue.push(query.trim());
  updateQueueBadge();
  updateQueueHint();
  return _queue.length;
}

export function dequeueQuery() {
  if (_queue.length === 0) return null;
  const q = _queue.shift();
  updateQueueBadge();
  updateQueueHint();
  return q;
}

export function getQueueSize() {
  return _queue.length;
}

export function getQueueItems() {
  return [..._queue];
}

export function clearQueue() {
  _queue = [];
  updateQueueBadge();
  updateQueueHint();
}

export function removeQueueItem(index) {
  if (index < 0 || index >= _queue.length) return;
  _queue.splice(index, 1);
  updateQueueBadge();
  updateQueueHint();
}

export function isQueueMode() {
  return get('streaming') === true && _queue.length > 0;
}

export function updateQueueBadge() {
  const sendBtn = $('sendBtn');
  if (!sendBtn) return;
  const size = _queue.length;
  let badge = $('queueBadge');

  if (size > 0 && get('streaming')) {
    if (!badge) {
      badge = document.createElement('span');
      badge.id = 'queueBadge';
      badge.className = 'queue-badge';
      sendBtn.style.position = 'relative';
      sendBtn.appendChild(badge);
    }
    badge.textContent = String(size);
    badge.setAttribute('aria-label', t('chat.queue_count_hint').replace('{count}', String(size)));
  } else {
    if (badge) badge.remove();
  }
}

function updateQueueHint() {
  if (_queue.length > 0 && get('streaming')) {
    const hint = $('inputHint');
    if (hint) {
      hint.textContent = t('chat.queue_count_hint').replace('{count}', String(_queue.length));
    }
  }
}

export function updateSendButton() {
  const sendBtn = $('sendBtn');
  if (!sendBtn) return;

  const streaming = get('streaming');
  const sendIcon = $('sendIcon');
  const stopIcon = $('stopIcon');

  if (streaming) {
    sendBtn.classList.add('stop-mode', 'bg-red-500/15', 'text-red-300', 'border-red-400/40', 'hover:bg-red-500/25');
    sendBtn.classList.remove('bg-accent', 'text-ink', 'hover:opacity-90', 'opacity-40', 'cursor-default');
    sendBtn.disabled = false;
    sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');
    if (sendIcon) sendIcon.classList.add('hidden');
    if (stopIcon) stopIcon.classList.remove('hidden');
    sendBtn.setAttribute('title', t('chat.stop_generation'));
    sendBtn.setAttribute('aria-label', t('chat.stop_generation'));
    updateQueueBadge();
  } else {
    sendBtn.classList.remove('stop-mode', 'bg-red-500/15', 'text-red-300', 'border-red-400/40', 'hover:bg-red-500/25');
    sendBtn.classList.add('bg-accent', 'text-ink', 'hover:opacity-90');
    if (sendIcon) sendIcon.classList.remove('hidden');
    if (stopIcon) stopIcon.classList.add('hidden');
    sendBtn.setAttribute('title', t('chat.send'));
    sendBtn.setAttribute('aria-label', t('chat.send'));
    updateQueueBadge();
    updateInputUI();
  }
}

export async function processQueue(sendCallback) {
  if (get('streaming')) return;
  if (_queue.length === 0) return;

  const query = dequeueQuery();
  if (query && typeof sendCallback === 'function') {
    updateSendButton();
    sendCallback(query);
  }
}

export function handleSendOrQueue(query, sendCallback) {
  if (get('streaming')) {
    enqueueQuery(query);
    updateSendButton();
    return true;
  } else {
    if (typeof sendCallback === 'function') sendCallback(query);
    return false;
  }
}
