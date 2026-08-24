/**
 * EchoMind 流式期间排队发送单元测试 — queue-send.js 模块（TC-QA-061~067）。
 *
 * 验证点（对应 AC-QA-012 流式期间排队发送）：
 * 1. enqueueQuery 将问题加入队列
 * 2. getQueueSize 返回正确队列长度
 * 3. dequeueQuery 取出并移除队首问题
 * 4. updateSendButton 在流式时将发送按钮变为"排队"样式
 * 5. processQueue 在流式结束后自动发送排队问题
 * 6. clearQueue 清空队列
 * 7. 队列为空时 dequeueQuery 返回 null
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import {
  enqueueQuery,
  getQueueSize,
  dequeueQuery,
  updateSendButton,
  processQueue,
  clearQueue,
  isQueueMode,
} from '../../../ui/src/chat-utils.js';
import { setState, resetState } from '../../../ui/src/state.js';

describe('Queue Send — queue-send.js', () => {
  let sendBtn;
  let stopBtn;
  let queryInput;
  let inputHint;

  beforeEach(() => {
    resetState();

    // 模拟 DOM 元素
    sendBtn = document.createElement('button');
    sendBtn.id = 'sendBtn';
    sendBtn.className = 'send-btn';

    stopBtn = document.createElement('button');
    stopBtn.id = 'stopBtn';
    stopBtn.className = 'hidden';

    queryInput = document.createElement('textarea');
    queryInput.id = 'queryInput';
    queryInput.value = '';

    inputHint = document.createElement('span');
    inputHint.id = 'inputHint';

    document.body.appendChild(sendBtn);
    document.body.appendChild(stopBtn);
    document.body.appendChild(queryInput);
    document.body.appendChild(inputHint);

    // 清空队列
    clearQueue();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetState();
    clearQueue();
  });

  describe('enqueueQuery / getQueueSize', () => {
    it('TC-QA-061: 将问题加入队列，队列长度增加', () => {
      enqueueQuery('第一个问题');
      expect(getQueueSize()).toBe(1);
      enqueueQuery('第二个问题');
      expect(getQueueSize()).toBe(2);
    });
  });

  describe('dequeueQuery', () => {
    it('TC-QA-062: 取出队首问题并移除', () => {
      enqueueQuery('第一个问题');
      enqueueQuery('第二个问题');
      const first = dequeueQuery();
      expect(first).toBe('第一个问题');
      expect(getQueueSize()).toBe(1);
    });

    it('TC-QA-067: 队列为空时 dequeueQuery 返回 null', () => {
      const result = dequeueQuery();
      expect(result).toBeNull();
    });
  });

  describe('isQueueMode', () => {
    it('TC-QA-062b: 流式期间 + 队列有内容 = 队列模式', () => {
      setState({ streaming: true });
      enqueueQuery('排队问题');
      expect(isQueueMode()).toBe(true);
    });

    it('TC-QA-062c: 非流式状态 = 非队列模式', () => {
      setState({ streaming: false });
      enqueueQuery('排队问题');
      expect(isQueueMode()).toBe(false);
    });
  });

  describe('updateSendButton', () => {
    it('TC-QA-063: 流式期间发送按钮变为"停止"样式', () => {
      setState({ streaming: true });
      updateSendButton();
      // 发送/停止按钮合二为一（用户反馈项）：流式期间为 stop-mode 停止形态
      expect(sendBtn.classList.contains('stop-mode')).toBe(true);
    });

    it('TC-QA-063b: 非流式状态恢复普通发送按钮', () => {
      setState({ streaming: false });
      updateSendButton();
      expect(sendBtn.classList.contains('stop-mode')).toBe(false);
    });
  });

  describe('processQueue', () => {
    it('TC-QA-064: 流式结束后自动发送排队问题', async () => {
      const sendCallback = vi.fn();
      enqueueQuery('排队的问题');
      setState({ streaming: false });
      await processQueue(sendCallback);
      expect(sendCallback).toHaveBeenCalledWith('排队的问题');
      expect(getQueueSize()).toBe(0);
    });

    it('TC-QA-064b: 队列为空时不调用 sendCallback', async () => {
      const sendCallback = vi.fn();
      setState({ streaming: false });
      await processQueue(sendCallback);
      expect(sendCallback).not.toHaveBeenCalled();
    });

    it('TC-QA-064c: 流式中不处理队列（需等待流式结束）', async () => {
      const sendCallback = vi.fn();
      enqueueQuery('排队的问题');
      setState({ streaming: true });
      await processQueue(sendCallback);
      expect(sendCallback).not.toHaveBeenCalled();
    });
  });

  describe('clearQueue', () => {
    it('TC-QA-066: 清空队列', () => {
      enqueueQuery('问题1');
      enqueueQuery('问题2');
      enqueueQuery('问题3');
      clearQueue();
      expect(getQueueSize()).toBe(0);
    });
  });
});
