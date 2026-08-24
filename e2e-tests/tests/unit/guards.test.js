/**
 * Guards（防御性编程守卫）单元测试。
 *
 * 测试覆盖：
 * - requireDocuments / canSend / requirePro / requireIdle / requireLlmConfig / requireUnlocked
 * - runGuard 执行器
 * - updateInputUI / syncChatInputState UI 同步
 *
 * V5 更新：统一 updateInputUI() 替代 syncChatInputState
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { resetState, setState } from '../../../ui/src/state.js';
import {
  requireDocuments,
  canSend,
  requirePro,
  requireIdle,
  requireLlmConfig,
  requireUnlocked,
  runGuard,
  updateInputUI,
  syncChatInputState,
} from '../../../ui/src/action.js';

// Mock DOM
document.body.innerHTML = `
  <input id="queryInput" />
  <button id="sendBtn"><svg id="sendIcon"></svg><svg id="stopIcon" class="hidden"></svg></button>
  <div id="inputHint"></div>
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

// Mock queue-send getQueueSize
vi.mock('../../../ui/src/chat-utils.js', () => ({
  getQueueSize: () => 0,
}));

describe('Guards', () => {
  beforeEach(() => {
    resetState();
    const input = document.getElementById('queryInput');
    const sendBtn = document.getElementById('sendBtn');
    input.value = '';
    input.disabled = false;
    sendBtn.disabled = false;
    sendBtn.classList.remove('opacity-30', 'cursor-not-allowed', 'opacity-40', 'cursor-default');
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

  describe('requireLlmConfig', () => {
    it('LLM 未配置时不通过', () => {
      setState({ llmConfigured: false });
      const result = requireLlmConfig();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.llm_not_configured');
    });

    it('LLM 已配置时通过', () => {
      setState({ llmConfigured: true });
      const result = requireLlmConfig();
      expect(result.passed).toBe(true);
    });
  });

  describe('requireUnlocked', () => {
    it('数据库锁定时不通过', () => {
      setState({ securityState: 'locked' });
      const result = requireUnlocked();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.security_locked');
    });

    it('数据库未锁定时通过', () => {
      setState({ securityState: 'unencrypted' });
      const result = requireUnlocked();
      expect(result.passed).toBe(true);
    });
  });

  describe('canSend', () => {
    it('流式生成中不通过', () => {
      setState({ docCount: 5, streaming: true, llmConfigured: true });
      const result = canSend();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.streaming_hint');
    });

    it('非流式 + 有文档 + LLM 已配置时通过', () => {
      setState({ docCount: 5, streaming: false, llmConfigured: true });
      const result = canSend();
      expect(result.passed).toBe(true);
    });

    it('非流式 + 无文档时不通过', () => {
      setState({ docCount: 0, streaming: false, llmConfigured: true });
      const result = canSend();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.empty_kb_error');
    });

    it('LLM 未配置时不通过', () => {
      setState({ docCount: 5, streaming: false, llmConfigured: false });
      const result = canSend();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.llm_not_configured');
    });

    it('数据库锁定时不通过', () => {
      setState({ docCount: 5, streaming: false, llmConfigured: true, securityState: 'locked' });
      const result = canSend();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.security_locked');
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

    it('数据库锁定时不通过', () => {
      setState({ streaming: false, securityState: 'locked' });
      const result = requireIdle();
      expect(result.passed).toBe(false);
      expect(result.reason).toBe('chat.security_locked');
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

  describe('updateInputUI', () => {
    it('知识库为空时禁用输入框', () => {
      setState({ docCount: 0, streaming: false });
      updateInputUI();
      const input = document.getElementById('queryInput');
      const sendBtn = document.getElementById('sendBtn');
      expect(input.disabled).toBe(true);
      expect(sendBtn.disabled).toBe(true);
    });

    it('有文档且非流式时启用输入框', () => {
      setState({ docCount: 3, streaming: false });
      const input = document.getElementById('queryInput');
      const btn = document.getElementById('sendBtn');
      input.value = 'test question';
      updateInputUI();
      expect(input.disabled).toBe(false);
      expect(btn.disabled).toBe(false);
    });

    it('流式生成时不禁用输入框（支持排队发送）', () => {
      setState({ docCount: 3, streaming: true });
      updateInputUI();
      const input = document.getElementById('queryInput');
      expect(input.disabled).toBe(false);
    });

    it('流式生成 + 知识库为空时输入框仍启用', () => {
      setState({ docCount: 0, streaming: true });
      updateInputUI();
      const input = document.getElementById('queryInput');
      expect(input.disabled).toBe(false);
    });

    it('空输入时发送按钮视觉降级（opacity-40）', () => {
      setState({ docCount: 3, streaming: false });
      const input = document.getElementById('queryInput');
      input.value = '';
      updateInputUI();
      const sendBtn = document.getElementById('sendBtn');
      expect(sendBtn.classList.contains('opacity-40')).toBe(true);
    });

    it('有输入时发送按钮视觉高亮', () => {
      setState({ docCount: 3, streaming: false });
      const input = document.getElementById('queryInput');
      input.value = 'some question';
      updateInputUI();
      const sendBtn = document.getElementById('sendBtn');
      expect(sendBtn.classList.contains('opacity-40')).toBe(false);
      expect(sendBtn.classList.contains('hover:opacity-90')).toBe(true);
    });

    it('数据库锁定时禁用一切', () => {
      setState({ docCount: 3, streaming: false, securityState: 'locked' });
      updateInputUI();
      const input = document.getElementById('queryInput');
      const sendBtn = document.getElementById('sendBtn');
      expect(input.disabled).toBe(true);
      expect(sendBtn.disabled).toBe(true);
    });
  });

  describe('syncChatInputState (backward compat)', () => {
    it('委托给 updateInputUI', () => {
      setState({ docCount: 0, streaming: false });
      syncChatInputState();
      const input = document.getElementById('queryInput');
      expect(input.disabled).toBe(true);
    });
  });
});
