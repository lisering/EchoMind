/**
 * EchoMind 点赞/踩反馈单元测试 — feedback.js 模块（TC-QA-047~053）。
 *
 * 验证点（对应 AC-QA-013 点赞/踩反馈）：
 * 1. createFeedbackButtons 在 assistant 消息块创建 👍/👎 按钮
 * 2. 点击 👍 标记"有帮助"，本地存储
 * 3. 点击 👎 弹出原因选择弹框
 * 4. 选择原因后保存反馈数据
 * 5. 已反馈的消息按钮高亮
 * 6. getFeedback 返回保存的反馈
 * 7. getFeedbackStats 统计点赞/踩总数
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import {
  createFeedbackButtons,
  handleFeedback,
  showDislikePopup,
  saveFeedback,
  getFeedback,
  getFeedbackStats,
  FEEDBACK_REASONS,
} from '../../../ui/src/feedback.js';

// jsdom 环境下 localStorage polyfill（部分版本不提供全局 localStorage）
const _storage = {};
const localStorageMock = {
  getItem: (key) => _storage[key] ?? null,
  setItem: (key, val) => { _storage[key] = String(val); },
  removeItem: (key) => { delete _storage[key]; },
  clear: () => { Object.keys(_storage).forEach((k) => delete _storage[k]); },
  get length() { return Object.keys(_storage).length; },
  key: (i) => Object.keys(_storage)[i] ?? null,
};
if (typeof globalThis.localStorage === 'undefined') {
  globalThis.localStorage = localStorageMock;
}

describe('Feedback — feedback.js', () => {
  let blockEl;
  let actionsBar;

  beforeEach(() => {
    // 清除 localStorage
    localStorage.clear();

    blockEl = document.createElement('div');
    blockEl.className = 'msg-block msg-assistant';
    blockEl.dataset.messageId = 'test-msg-001';

    actionsBar = document.createElement('div');
    actionsBar.className = 'msg-actions';
    blockEl.appendChild(actionsBar);

    document.body.appendChild(blockEl);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    localStorage.clear();
  });

  describe('createFeedbackButtons', () => {
    it('TC-QA-047: 在 .msg-actions 中创建 👍/👎 按钮', () => {
      createFeedbackButtons(blockEl);
      const likeBtn = actionsBar.querySelector('.feedback-like');
      const dislikeBtn = actionsBar.querySelector('.feedback-dislike');
      expect(likeBtn).not.toBeNull();
      expect(dislikeBtn).not.toBeNull();
    });

    it('TC-QA-047b: 按钮包含正确的 aria-label', () => {
      createFeedbackButtons(blockEl);
      const likeBtn = actionsBar.querySelector('.feedback-like');
      const dislikeBtn = actionsBar.querySelector('.feedback-dislike');
      expect(likeBtn.getAttribute('aria-label')).toBeTruthy();
      expect(dislikeBtn.getAttribute('aria-label')).toBeTruthy();
    });
  });

  describe('saveFeedback / getFeedback', () => {
    it('TC-QA-048: 点击 👍 保存 helpful=true 到本地存储', () => {
      saveFeedback('test-msg-001', true, null);
      const fb = getFeedback('test-msg-001');
      expect(fb).not.toBeNull();
      expect(fb.isHelpful).toBe(true);
      expect(fb.reason).toBeNull();
    });

    it('TC-QA-049: 点击 👎 并选择原因后保存 helpful=false + reason', () => {
      saveFeedback('test-msg-001', false, FEEDBACK_REASONS.INACCURATE);
      const fb = getFeedback('test-msg-001');
      expect(fb).not.toBeNull();
      expect(fb.isHelpful).toBe(false);
      expect(fb.reason).toBe(FEEDBACK_REASONS.INACCURATE);
    });

    it('TC-QA-050: 已反馈的消息按钮添加 .active 高亮类', () => {
      createFeedbackButtons(blockEl);
      handleFeedback(blockEl, 'like');
      const likeBtn = actionsBar.querySelector('.feedback-like');
      expect(likeBtn.classList.contains('active')).toBe(true);
    });

    it('TC-QA-050b: 再次点击相同反馈取消高亮', () => {
      createFeedbackButtons(blockEl);
      handleFeedback(blockEl, 'like');
      handleFeedback(blockEl, 'like'); // 再次点击取消
      const likeBtn = actionsBar.querySelector('.feedback-like');
      expect(likeBtn.classList.contains('active')).toBe(false);
    });
  });

  describe('showDislikePopup', () => {
    it('TC-QA-051: 点击 👎 显示原因选择弹框', () => {
      createFeedbackButtons(blockEl);
      showDislikePopup(blockEl);
      const popup = blockEl.querySelector('.feedback-popup');
      expect(popup).not.toBeNull();
      expect(popup.style.display).not.toBe('none');
    });

    it('TC-QA-052: 弹框包含 4 个原因选项', () => {
      createFeedbackButtons(blockEl);
      showDislikePopup(blockEl);
      const popup = blockEl.querySelector('.feedback-popup');
      const options = popup.querySelectorAll('.feedback-reason-option');
      expect(options.length).toBe(4);
    });
  });

  describe('getFeedbackStats', () => {
    it('TC-QA-053: 统计点赞和踩的总数', () => {
      saveFeedback('msg-1', true, null);
      saveFeedback('msg-2', true, null);
      saveFeedback('msg-3', false, FEEDBACK_REASONS.INCOMPLETE);
      const stats = getFeedbackStats();
      expect(stats.helpful).toBe(2);
      expect(stats.notHelpful).toBe(1);
      expect(stats.total).toBe(3);
    });
  });
});
