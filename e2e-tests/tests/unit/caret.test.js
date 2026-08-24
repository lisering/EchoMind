/**
 * EchoMind 流式光标单元测试 — caret.js 模块（TC-QA-001）。
 *
 * 验证点：
 * 1. showCaret 在指定元素末尾插入光标 DOM 节点
 * 2. removeCaret 移除已存在的光标
 * 3. removeCaret 在无光标时安全无操作
 * 4. isCaretActive 正确检测光标存在性
 * 5. 光标元素具有正确的 CSS class（stream-caret）
 * 6. removeCaret 是幂等的（多次调用不报错）
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { showCaret, removeCaret, isCaretActive } from '../../../ui/src/chat-render.js';

describe('Stream Caret — caret.js', () => {
  let container;

  beforeEach(() => {
    container = document.createElement('div');
    container.innerHTML = '<p>已有内容</p>';
    document.body.appendChild(container);
  });

  describe('showCaret', () => {
    it('在容器末尾插入光标元素', () => {
      showCaret(container);
      const caret = container.querySelector('[data-stream-caret]');
      expect(caret).not.toBeNull();
    });

    it('光标元素是内联元素', () => {
      showCaret(container);
      const caret = container.querySelector('[data-stream-caret]');
      expect(caret.tagName).toBe('SPAN');
    });

    it('不重复插入（已有光标时跳过）', () => {
      showCaret(container);
      showCaret(container);
      const carets = container.querySelectorAll('[data-stream-caret]');
      expect(carets).toHaveLength(1);
    });
  });

  describe('removeCaret', () => {
    it('移除已存在的光标', () => {
      showCaret(container);
      removeCaret(container);
      expect(container.querySelector('[data-stream-caret]')).toBeNull();
    });

    it('无光标时安全无操作（不抛异常）', () => {
      expect(() => removeCaret(container)).not.toThrow();
    });

    it('多次调用幂等', () => {
      showCaret(container);
      removeCaret(container);
      removeCaret(container);
      removeCaret(container);
      expect(container.querySelector('[data-stream-caret]')).toBeNull();
    });
  });

  describe('isCaretActive', () => {
    it('无光标时返回 false', () => {
      expect(isCaretActive(container)).toBe(false);
    });

    it('有光标时返回 true', () => {
      showCaret(container);
      expect(isCaretActive(container)).toBe(true);
    });

    it('移除后返回 false', () => {
      showCaret(container);
      removeCaret(container);
      expect(isCaretActive(container)).toBe(false);
    });
  });
});
