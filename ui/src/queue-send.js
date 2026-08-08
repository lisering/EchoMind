/**
 * EchoMind 流式期间排队发送模块 — 允许用户在流式生成期间排队下一条问题。
 *
 * 职责：
 * 1. 在流式生成期间允许用户输入并排队问题
 * 2. 发送按钮在流式时变为"排队"样式
 * 3. 当前回答完成后自动发送队列中的问题
 * 4. 队列管理（入队/出队/清空/查询大小）
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.4.4 流式期间不锁定输入框
 * AC-QA-012：流式期间排队发送
 */

import { get, setState } from './state.js';
import { t } from './i18n.js';

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
  return _queue.length;
}

/**
 * 取出队首问题并从队列中移除。
 *
 * @returns {string|null} 队首问题（队列为空时返回 null）
 */
export function dequeueQuery() {
  if (_queue.length === 0) return null;
  return _queue.shift();
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
 * 清空队列。
 *
 * @returns {void}
 */
export function clearQueue() {
  _queue = [];
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
// 发送按钮状态管理
// ============================================================

/**
 * 更新发送按钮的显示状态（发送/停止合二为一）。
 *
 * - 流式生成中：按钮变为「停止」形态（stop-mode 红色 + 停止方块图标），
 *   点击停止生成（onStop）。
 * - 非流式状态：恢复「发送」形态（accent 底色 + 纸飞机图标）。
 *
 * @returns {void}
 */
export function updateSendButton() {
  const sendBtn = document.getElementById('sendBtn');
  if (!sendBtn) return;

  const streaming = get('streaming');
  const sendIcon = document.getElementById('sendIcon');
  const stopIcon = document.getElementById('stopIcon');

  if (streaming) {
    // 流式期间：停止形态
    sendBtn.classList.add('stop-mode', 'bg-red-500/15', 'text-red-300', 'border-red-400/40', 'hover:bg-red-500/25');
    sendBtn.classList.remove('bg-accent', 'text-ink', 'hover:opacity-90');
    sendBtn.disabled = false; // 停止按钮始终可用
    if (sendIcon) sendIcon.classList.add('hidden');
    if (stopIcon) stopIcon.classList.remove('hidden');
    sendBtn.setAttribute('title', t('chat.stop_generation'));
  } else {
    // 空闲态：发送形态
    sendBtn.classList.remove('stop-mode', 'bg-red-500/15', 'text-red-300', 'border-red-400/40', 'hover:bg-red-500/25');
    sendBtn.classList.add('bg-accent', 'text-ink', 'hover:opacity-90');
    if (sendIcon) sendIcon.classList.remove('hidden');
    if (stopIcon) stopIcon.classList.add('hidden');
    sendBtn.setAttribute('title', t('chat.send'));
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
