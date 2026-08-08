/**
 * EchoMind 点赞/踩反馈模块 — assistant 消息的 👍/👎 反馈机制。
 *
 * 职责：
 * 1. 在 assistant 消息操作栏中添加 👍/👎 按钮
 * 2. 点击 👍 标记"有帮助"，本地存储
 * 3. 点击 👎 弹出原因选择弹框（4 种原因）
 * 4. 反馈数据存储到 localStorage，不发送到云端
 * 5. 已反馈的消息按钮高亮
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.14 点赞/踩反馈
 * AC-QA-013：点赞/踩反馈机制
 */

import { t } from './i18n.js';
import { reportThumbsUp, reportThumbsDown } from './retrieval-feedback.js';

// ============================================================
// 常量定义
// ============================================================

/** localStorage 存储键前缀 */
const STORAGE_KEY_PREFIX = 'echomind_feedback_';

/** 反馈原因枚举 */
export const FEEDBACK_REASONS = {
  INACCURATE: 'inaccurate',
  INCOMPLETE: 'incomplete',
  FORMAT: 'format',
  OTHER: 'other',
};

// ============================================================
// 类型定义（JSDoc）
// ============================================================

/**
 * @typedef {Object} FeedbackData
 * @property {string} messageId - 消息 ID
 * @property {boolean} isHelpful - 是否有帮助
 * @property {string|null} reason - 反馈原因（仅 isHelpful=false 时有值）
 * @property {number} timestamp - 时间戳
 */

// ============================================================
// 创建反馈按钮
// ============================================================

/**
 * 在 assistant 消息块的操作栏中创建 👍/👎 按钮。
 *
 * @param {HTMLElement} blockEl - assistant 消息块根元素
 * @returns {void}
 */
export function createFeedbackButtons(blockEl) {
  const actionsBar = blockEl.querySelector('.msg-actions');
  if (!actionsBar) return;

  // 检查是否已有反馈按钮
  if (actionsBar.querySelector('.feedback-like')) return;

  // 👍 有帮助按钮
  const likeBtn = document.createElement('button');
  likeBtn.className = 'msg-action-btn feedback-like';
  likeBtn.setAttribute('aria-label', t('chat.feedback_like') || '有帮助');
  likeBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0"><path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3"/></svg><span>👍</span>';
  likeBtn.onclick = () => handleFeedback(blockEl, 'like');

  // 👎 无帮助按钮
  const dislikeBtn = document.createElement('button');
  dislikeBtn.className = 'msg-action-btn feedback-dislike';
  dislikeBtn.setAttribute('aria-label', t('chat.feedback_dislike') || '无帮助');
  dislikeBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="shrink-0"><path d="M10 15v4a3 3 0 0 0 3 3l4-9V2H5.72a2 2 0 0 0-2 1.7l-1.38 9a2 2 0 0 0 2 2.3zM17 2h3a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-3"/></svg><span>👎</span>';
  dislikeBtn.onclick = () => handleFeedback(blockEl, 'dislike');

  actionsBar.appendChild(likeBtn);
  actionsBar.appendChild(dislikeBtn);

  // 如果已有反馈记录，恢复高亮状态
  const messageId = blockEl.dataset.messageId;
  if (messageId) {
    const existing = getFeedback(messageId);
    if (existing) {
      if (existing.isHelpful) {
        likeBtn.classList.add('active');
      } else {
        dislikeBtn.classList.add('active');
      }
    }
  }
}

// ============================================================
// 反馈处理
// ============================================================

/**
 * 处理反馈按钮点击。
 *
 * - 'like': 标记有帮助（toggle）
 * - 'dislike': 显示原因选择弹框
 *
 * @param {HTMLElement} blockEl - 消息块根元素
 * @param {'like'|'dislike'} type - 反馈类型
 * @returns {void}
 */
export function handleFeedback(blockEl, type) {
  const messageId = blockEl.dataset.messageId || `msg-${Date.now()}`;
  blockEl.dataset.messageId = messageId;

  const likeBtn = blockEl.querySelector('.feedback-like');
  const dislikeBtn = blockEl.querySelector('.feedback-dislike');

  if (type === 'like') {
    // Toggle 逻辑：再次点击取消
    const existing = getFeedback(messageId);
    if (existing && existing.isHelpful) {
      // 取消反馈
      removeFeedback(messageId);
      likeBtn?.classList.remove('active');
    } else {
      saveFeedback(messageId, true, null);
      likeBtn?.classList.add('active');
      dislikeBtn?.classList.remove('active');
      // 关闭可能打开的弹框
      closeDislikePopup(blockEl);
      // REQ-PERF-012 扩展：上报强正信号到检索记忆引擎
      const query = blockEl.dataset.query || '';
      if (query) reportThumbsUp(query);
    }
  } else if (type === 'dislike') {
    // Toggle 逻辑：再次点击取消
    const existing = getFeedback(messageId);
    if (existing && !existing.isHelpful) {
      removeFeedback(messageId);
      dislikeBtn?.classList.remove('active');
      closeDislikePopup(blockEl);
    } else {
      showDislikePopup(blockEl);
    }
  }
}

// ============================================================
// 👎 原因选择弹框
// ============================================================

/**
 * 显示 👎 原因选择弹框。
 *
 * 弹框包含 4 个原因选项：
 * - 信息不准确 (INACCURATE)
 * - 不完整 (INCOMPLETE)
 * - 格式问题 (FORMAT)
 * - 其他 (OTHER)
 *
 * @param {HTMLElement} blockEl - 消息块根元素
 * @returns {void}
 */
export function showDislikePopup(blockEl) {
  // 如果已有弹框，先移除
  closeDislikePopup(blockEl);

  const popup = document.createElement('div');
  popup.className = 'feedback-popup';

  const title = document.createElement('div');
  title.className = 'feedback-popup-title';
  title.textContent = t('chat.feedback_what_wrong') || '哪里有问题？';
  popup.appendChild(title);

  const reasons = [
    { value: FEEDBACK_REASONS.INACCURATE, label: t('chat.feedback_inaccurate') || '信息不准确' },
    { value: FEEDBACK_REASONS.INCOMPLETE, label: t('chat.feedback_incomplete') || '不完整' },
    { value: FEEDBACK_REASONS.FORMAT, label: t('chat.feedback_format') || '格式问题' },
    { value: FEEDBACK_REASONS.OTHER, label: t('chat.feedback_other') || '其他' },
  ];

  reasons.forEach(({ value, label }) => {
    const option = document.createElement('button');
    option.className = 'feedback-reason-option';
    option.textContent = label;
    option.dataset.reason = value;
    option.onclick = () => {
      const messageId = blockEl.dataset.messageId || `msg-${Date.now()}`;
      blockEl.dataset.messageId = messageId;
      saveFeedback(messageId, false, value);
      // 高亮 👎 按钮
      const dislikeBtn = blockEl.querySelector('.feedback-dislike');
      const likeBtn = blockEl.querySelector('.feedback-like');
      dislikeBtn?.classList.add('active');
      likeBtn?.classList.remove('active');
      closeDislikePopup(blockEl);
      // REQ-PERF-012 扩展：上报强负信号到检索记忆引擎
      const query = blockEl.dataset.query || '';
      if (query) reportThumbsDown(query);
    };
    popup.appendChild(option);
  });

  blockEl.appendChild(popup);
}

/**
 * 关闭 👎 原因选择弹框。
 *
 * @param {HTMLElement} blockEl - 消息块根元素
 * @returns {void}
 */
function closeDislikePopup(blockEl) {
  const popup = blockEl.querySelector('.feedback-popup');
  if (popup) popup.remove();
}

// ============================================================
// 本地存储
// ============================================================

/**
 * 保存反馈到 localStorage。
 *
 * @param {string} messageId - 消息 ID
 * @param {boolean} isHelpful - 是否有帮助
 * @param {string|null} reason - 反馈原因
 * @returns {void}
 */
export function saveFeedback(messageId, isHelpful, reason) {
  /** @type {FeedbackData} */
  const data = {
    messageId,
    isHelpful,
    reason,
    timestamp: Date.now(),
  };
  try {
    localStorage.setItem(STORAGE_KEY_PREFIX + messageId, JSON.stringify(data));
  } catch (e) {
    // localStorage 不可用时静默失败
  }
}

/**
 * 获取指定消息的反馈记录。
 *
 * @param {string} messageId - 消息 ID
 * @returns {FeedbackData|null} 反馈数据（不存在时返回 null）
 */
export function getFeedback(messageId) {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_PREFIX + messageId);
    if (!raw) return null;
    return JSON.parse(raw);
  } catch (e) {
    return null;
  }
}

/**
 * 删除指定消息的反馈记录。
 *
 * @param {string} messageId - 消息 ID
 * @returns {void}
 */
function removeFeedback(messageId) {
  try {
    localStorage.removeItem(STORAGE_KEY_PREFIX + messageId);
  } catch (e) {
    // 静默失败
  }
}

/**
 * 获取所有反馈的统计数据。
 *
 * @returns {{helpful: number, notHelpful: number, total: number}} 统计数据
 */
export function getFeedbackStats() {
  let helpful = 0;
  let notHelpful = 0;
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith(STORAGE_KEY_PREFIX)) {
        const data = JSON.parse(localStorage.getItem(key));
        if (data.isHelpful) {
          helpful++;
        } else {
          notHelpful++;
        }
      }
    }
  } catch (e) {
    // 静默失败
  }
  return {
    helpful,
    notHelpful,
    total: helpful + notHelpful,
  };
}
