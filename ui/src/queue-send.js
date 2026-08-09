/**
 * EchoMind 流式期间排队发送模块 — 允许用户在流式生成期间排队下一条问题。
 *
 * 职责：
 * 1. 在流式生成期间允许用户输入并排队问题
 * 2. 发送按钮在流式时变为"停止"形态 + 排队徽章
 * 3. 当前回答完成后自动发送队列中的问题
 * 4. 队列管理（入队/出队/清空/查询大小/查看队列）
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.4.4 流式期间不锁定输入框
 * AC-QA-012：流式期间排队发送
 *
 * V5 重构：新增排队徽章 UI + 队列查看面板
 */

import { get, setState } from './state.js';
import { $ } from './utils.js';
import { t } from './i18n.js';
import { updateInputUI } from './guards.js';

// ============================================================
// 模块状态
// ============================================================

/** 待发送问题队列（FIFO） */
let _queue = [];

// ============================================================
// 队列操作
// ============================================================

/**
 * 将一条问题加入发送队列。
 *
 * @param {string} query - 要排队的问题文本
 * @returns {number} 入队后的队列长度
 */
export function enqueueQuery(query) {
  if (!query || !query.trim()) return _queue.length;
  _queue.push(query.trim());
  updateQueueBadge();
  updateQueueHint();
  return _queue.length;
}

/**
 * 取出队首问题并从队列中移除。
 *
 * @returns {string|null} 队首问题（队列为空时返回 null）
 */
export function dequeueQuery() {
  if (_queue.length === 0) return null;
  const q = _queue.shift();
  updateQueueBadge();
  updateQueueHint();
  return q;
}

/**
 * 获取当前队列长度。
 *
 * @returns {number} 队列中的问题数量
 */
export function getQueueSize() {
  return _queue.length;
}

/**
 * 获取队列内容副本（用于查看面板）。
 *
 * @returns {string[]} 队列内容数组
 */
export function getQueueItems() {
  return [..._queue];
}

/**
 * 清空队列。
 *
 * @returns {void}
 */
export function clearQueue() {
  _queue = [];
  updateQueueBadge();
  updateQueueHint();
}

/**
 * 移除队列中指定索引的问题。
 *
 * @param {number} index - 要移除的问题索引
 * @returns {void}
 */
export function removeQueueItem(index) {
  if (index < 0 || index >= _queue.length) return;
  _queue.splice(index, 1);
  updateQueueBadge();
  updateQueueHint();
}

// ============================================================
// 队列模式判断
// ============================================================

/**
 * 判断当前是否处于"队列模式"（流式生成中 + 队列有内容）。
 *
 * @returns {boolean} true=队列模式
 */
export function isQueueMode() {
  return get('streaming') === true && _queue.length > 0;
}

// ============================================================
// 排队徽章 UI
// ============================================================

/**
 * 更新发送按钮上的排队徽章。
 * 流式期间队列非空时显示数字徽章。
 *
 * @returns {void}
 */
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

/**
 * 更新 inputHint 显示排队信息。
 *
 * @returns {void}
 */
function updateQueueHint() {
  if (_queue.length > 0 && get('streaming')) {
    const hint = $('inputHint');
    if (hint) {
      hint.textContent = t('chat.queue_count_hint').replace('{count}', String(_queue.length));
    }
  }
}

// ============================================================
// 发送按钮状态管理
// ============================================================

/**
 * 更新发送按钮的显示状态（发送/停止合二为一）。
 *
 * - 流式生成中：按钮变为「停止」形态（stop-mode 红色 + 停止方块图标），
 *   点击停止生成（onStop）。若有排队，显示排队徽章。
 * - 非流式状态：恢复「发送」形态（accent 底色 + 纸飞机图标），
 *   空输入时视觉降级。
 *
 * @returns {void}
 */
export function updateSendButton() {
  const sendBtn = $('sendBtn');
  if (!sendBtn) return;

  const streaming = get('streaming');
  const sendIcon = $('sendIcon');
  const stopIcon = $('stopIcon');

  if (streaming) {
    // 流式期间：停止形态
    sendBtn.classList.add('stop-mode', 'bg-red-500/15', 'text-red-300', 'border-red-400/40', 'hover:bg-red-500/25');
    sendBtn.classList.remove('bg-accent', 'text-ink', 'hover:opacity-90', 'opacity-40', 'cursor-default');
    sendBtn.disabled = false; // 停止按钮始终可用
    sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');
    if (sendIcon) sendIcon.classList.add('hidden');
    if (stopIcon) stopIcon.classList.remove('hidden');
    sendBtn.setAttribute('title', t('chat.stop_generation'));
    sendBtn.setAttribute('aria-label', t('chat.stop_generation'));
    updateQueueBadge();
  } else {
    // 空闲态：发送形态
    sendBtn.classList.remove('stop-mode', 'bg-red-500/15', 'text-red-300', 'border-red-400/40', 'hover:bg-red-500/25');
    sendBtn.classList.add('bg-accent', 'text-ink', 'hover:opacity-90');
    if (sendIcon) sendIcon.classList.remove('hidden');
    if (stopIcon) stopIcon.classList.add('hidden');
    sendBtn.setAttribute('title', t('chat.send'));
    sendBtn.setAttribute('aria-label', t('chat.send'));
    updateQueueBadge(); // 清除徽章
    updateInputUI(); // 重新评估空输入降级
  }
}

// ============================================================
// 队列处理
// ============================================================

/**
 * 在流式结束后处理队列。
 *
 * 如果队列中有待发送的问题，取出队首问题并调用 sendCallback 发送。
 * 仅在非流式状态时处理（流式中不处理）。
 *
 * @param {Function} sendCallback - 发送回调函数，参数为问题文本
 * @returns {Promise<void>}
 */
export async function processQueue(sendCallback) {
  // 流式中不处理队列
  if (get('streaming')) return;

  // 队列为空不处理
  if (_queue.length === 0) return;

  const query = dequeueQuery();
  if (query && typeof sendCallback === 'function') {
    // 恢复按钮状态
    updateSendButton();
    sendCallback(query);
  }
}

/**
 * 处理流式期间的发送/排队操作。
 *
 * 如果正在流式生成，将问题加入队列而非直接发送。
 * 如果非流式状态，直接发送。
 *
 * @param {string} query - 要发送的问题
 * @param {Function} sendCallback - 直接发送回调
 * @returns {boolean} true=已排队，false=已直接发送
 */
export function handleSendOrQueue(query, sendCallback) {
  if (get('streaming')) {
    // 流式中：排队
    enqueueQuery(query);
    updateSendButton();
    return true;
  } else {
    // 非流式：直接发送
    if (typeof sendCallback === 'function') {
      sendCallback(query);
    }
    return false;
  }
}
