/**
 * Guards（防御性编程守卫）单元测试。
 *
 * 测试覆盖：
 * - requireDocuments / canSend / requirePro / requireIdle
 * - runGuard 执行器
 * - syncChatInputState UI 同步
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { resetState, setState } from '../../../ui/src/state.js';
import {
  requireDocuments,
  canSend,
  requirePro,
  requireIdle,
  runGuard,
  syncChatInputState,
} from '../../../ui/src/guards.js';

// Mock DOM
document.body.innerHTML = `
  <input id="queryInput" />
  <button id="sendBtn"></button>
`;

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toastError: vi.fn(),
  toast: vi.fn(),
  toastSuccess: vi.fn(),
}));

describe('Guards', () => {
  beforeEach(() => {
    resetState();
  });

  describe('requireDocuments', () => {
    it('知识库为空时不通过', () => {
      setState({ docCount: 0 });
      const result = requireDocuments();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.empty_kb_error');
    });

    it('知识库有文档时通过', () => {
      setState({ docCount: 5 });
      const result = requireDocuments();
      expect(result.passed).toBe(true);
      expect(result.reason).toBeUndefined();
    });
  });

  describe('canSend', () => {
    it('流式生成中不通过', () => {
      setState({ docCount: 5, streaming: true });
      const result = canSend();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.streaming_hint');
    });

    it('非流式 + 有文档时通过', () => {
      setState({ docCount: 5, streaming: false });
      const result = canSend();
      expect(result.passed).toBe(true);
    });

    it('非流式 + 无文档时不通过', () => {
      setState({ docCount: 0, streaming: false });
      const result = canSend();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.empty_kb_error');
    });
  });

  describe('requirePro', () => {
    it('非 Pro 用户不通过', () => {
      setState({ isPro: false });
      const result = requirePro();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('paywall.reason_default');
    });

    it('Pro 用户通过', () => {
      setState({ isPro: true });
      const result = requirePro();
      expect(result.passed).toBe(true);
    });
  });

  describe('requireIdle', () => {
    it('流式生成中不通过', () => {
      setState({ streaming: true });
      const result = requireIdle();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.streaming_hint');
    });

    it('审计中不通过', () => {
      setState({ streaming: false, auditingDocId: 'doc-123' });
      const result = requireIdle();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.thinking_auditing');
    });

    it('空闲状态通过', () => {
      setState({ streaming: false, auditingDocId: null });
      const result = requireIdle();
      expect(result.passed).toBe(true);
    });
  });

  describe('runGuard', () => {
    it('通过时返回 true', () => {
      expect(runGuard({ passed: true })).toBe(true);
    });

    it('未通过时返回 false', () => {
      expect(runGuard({ passed: false, reason: 'some.error' })).toBe(false);
    });

    it('未通过时无 reason 不报错', () => {
      expect(runGuard({ passed: false })).toBe(false);
    });
  });

  describe('syncChatInputState', () => {
    it('知识库为空时禁用输入框', () => {
      setState({ docCount: 0, streaming: false });
      syncChatInputState();
      const input = document.getElementById('queryInput');
      const sendBtn = document.getElementById('sendBtn');
      expect(input.disabled).toBe(true);
      expect(sendBtn.disabled).toBe(true);
    });

    it('有文档且非流式时启用输入框', () => {
      setState({ docCount: 3, streaming: false });
      syncChatInputState();
      const input = document.getElementById('queryInput');
      const sendBtn = document.getElementById('sendBtn');
      expect(input.disabled).toBe(false);
      expect(sendBtn.disabled).toBe(false);
    });

    it('流式生成时不禁用输入框（支持排队发送）', () => {
      setState({ docCount: 3, streaming: true });
      syncChatInputState();
      const input = document.getElementById('queryInput');
      expect(input.disabled).toBe(false);
    });

    it('流式生成 + 知识库为空时输入框仍启用', () => {
      setState({ docCount: 0, streaming: true });
      syncChatInputState();
      const input = document.getElementById('queryInput');
      expect(input.disabled).toBe(false);
    });
  });
});
