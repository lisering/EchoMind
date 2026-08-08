/**
 * EchoMind 错误体验优化单元测试 — error-detail.js 模块（TC-QA-012~019）。
 *
 * 验证点（对应 AC-QA-008）：
 * 1. classifyError 正确分类网络中断错误
 * 2. classifyError 正确分类 API Key 无效错误
 * 3. classifyError 正确分类限流错误
 * 4. classifyError 正确分类上下文超限错误
 * 5. classifyError 正确分类知识库为空错误
 * 6. classifyError 正确分类本地模型加载失败错误
 * 7. classifyError 对未知错误返回 fallback 分类
 * 8. renderErrorCard 生成正确的 DOM 结构（标题 + 原因 + 操作按钮）
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { classifyError, renderErrorCard } from '../../../ui/src/error-detail.js';

describe('Error Detail — error-detail.js', () => {
  describe('classifyError', () => {
    it('TC-QA-012: 分类网络中断错误', () => {
      const result = classifyError('Network error: ECONNREFUSED');
      expect(result.type).toBe('network');
      expect(result.title).toBeTruthy();
      expect(result.reason).toBeTruthy();
      expect(result.actions.length).toBeGreaterThan(0);
    });

    it('TC-QA-012b: 分类超时错误为网络类型', () => {
      const result = classifyError('Request timed out after 30000ms');
      expect(result.type).toBe('network');
    });

    it('TC-QA-013: 分类 API Key 无效错误', () => {
      const result = classifyError('401 Unauthorized: Invalid API key');
      expect(result.type).toBe('auth');
      expect(result.actions).toContainEqual(
        expect.objectContaining({ action: 'open_settings' }),
      );
    });

    it('TC-QA-014: 分类限流错误', () => {
      const result = classifyError('429 Too Many Requests: Rate limit exceeded');
      expect(result.type).toBe('rate_limit');
      expect(result.actions.length).toBeGreaterThan(0);
    });

    it('TC-QA-015: 分类上下文超限错误', () => {
      const result = classifyError('context length exceeded: 8192 > 4096');
      expect(result.type).toBe('context_overflow');
    });

    it('TC-QA-016: 分类知识库为空错误', () => {
      const result = classifyError('知识库为空，请先导入文档');
      expect(result.type).toBe('kb_empty');
    });

    it('TC-QA-016b: 分类英文知识库为空错误', () => {
      const result = classifyError('Knowledge base is empty');
      expect(result.type).toBe('kb_empty');
    });

    it('TC-QA-017: 分类本地模型加载失败错误', () => {
      const result = classifyError('Model load failed: GGUF file corrupted');
      expect(result.type).toBe('model_load');
    });

    it('TC-QA-018: 未知错误返回 fallback 分类', () => {
      const result = classifyError('Something unexpected happened');
      expect(result.type).toBe('unknown');
      expect(result.title).toBeTruthy();
      expect(result.actions.length).toBeGreaterThan(0);
    });

    it('TC-QA-018b: 空字符串返回 unknown 分类', () => {
      const result = classifyError('');
      expect(result.type).toBe('unknown');
    });

    it('TC-QA-018c: null/undefined 安全降级', () => {
      expect(() => classifyError(null)).not.toThrow();
      expect(() => classifyError(undefined)).not.toThrow();
      expect(classifyError(null).type).toBe('unknown');
    });
  });

  describe('renderErrorCard', () => {
    let container;

    beforeEach(() => {
      container = document.createElement('div');
      document.body.appendChild(container);
    });

    it('TC-QA-019: 生成正确的 DOM 结构（标题 + 原因 + 操作按钮）', () => {
      const errorInfo = classifyError('Network error: ECONNREFUSED');
      renderErrorCard(container, errorInfo);
      const card = container.querySelector('.error-card');
      expect(card).not.toBeNull();
      // 标题
      const title = card.querySelector('.error-card-title');
      expect(title).not.toBeNull();
      expect(title.textContent).toBeTruthy();
      // 原因
      const reason = card.querySelector('.error-card-reason');
      expect(reason).not.toBeNull();
      // 操作按钮
      const actions = card.querySelectorAll('.error-card-action');
      expect(actions.length).toBeGreaterThan(0);
    });

    it('TC-QA-019b: 操作按钮携带 data-action 属性', () => {
      const errorInfo = classifyError('401 Unauthorized: Invalid API key');
      renderErrorCard(container, errorInfo);
      const actions = container.querySelectorAll('.error-card-action');
      actions.forEach((btn) => {
        expect(btn.dataset.action).toBeTruthy();
      });
    });

    it('TC-QA-019c: 网络错误包含重试按钮', () => {
      const errorInfo = classifyError('Network error: ECONNREFUSED');
      renderErrorCard(container, errorInfo);
      const retryBtn = container.querySelector('[data-action="retry"]');
      expect(retryBtn).not.toBeNull();
    });

    it('TC-QA-019d: 认证错误包含打开设置按钮', () => {
      const errorInfo = classifyError('401 Unauthorized');
      renderErrorCard(container, errorInfo);
      const settingsBtn = container.querySelector('[data-action="open_settings"]');
      expect(settingsBtn).not.toBeNull();
    });

    it('TC-QA-019e: 知识库为空包含导入文件按钮', () => {
      const errorInfo = classifyError('知识库为空');
      renderErrorCard(container, errorInfo);
      const importBtn = container.querySelector('[data-action="import_files"]');
      expect(importBtn).not.toBeNull();
    });
  });
});
