/**
 * EchoMind 智能滚动锁定单元测试 — scroll-lock.js 模块（TC-QA-002）。
 *
 * 验证点：
 * 1. initScrollLock 注册滚动事件监听器
 * 2. isUserScrolledUp 在底部时返回 false
 * 3. isUserScrolledUp 在滚向上方时返回 true
 * 4. shouldAutoScroll 在底部时返回 true
 * 5. shouldAutoScroll 在锁定时返回 false
 * 6. resetScrollLock 重置锁定状态
 * 7. createJumpToLatestButton 创建正确的 DOM 元素
 * 8. destroyScrollLock 清理事件监听
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  initScrollLock,
  isUserScrolledUp,
  shouldAutoScroll,
  resetScrollLock,
  createJumpToLatestButton,
  createBackToTopButton,
  destroyScrollLock,
} from '../../../ui/src/chat-utils.js';

describe('Scroll Lock — scroll-lock.js', () => {
  let chatArea;

  beforeEach(() => {
    // 创建一个可滚动的容器（jsdom 不计算布局，用 mock 属性模拟）
    chatArea = document.createElement('div');
    chatArea.id = 'chatArea';
    // jsdom 不计算 scrollHeight/clientHeight，使用 Object.defineProperty mock
    // 模拟：总内容高 1000px，可视区高 200px → 可滚动 800px
    Object.defineProperty(chatArea, 'scrollHeight', { configurable: true, get: () => 1000 });
    Object.defineProperty(chatArea, 'clientHeight', { configurable: true, get: () => 200 });
    document.body.appendChild(chatArea);
    resetScrollLock();
  });

  describe('initScrollLock', () => {
    it('注册后不抛异常', () => {
      expect(() => initScrollLock(chatArea)).not.toThrow();
    });

    it('返回清理函数', () => {
      const cleanup = initScrollLock(chatArea);
      expect(typeof cleanup).toBe('function');
      cleanup();
    });
  });

  describe('isUserScrolledUp', () => {
    it('初始状态（在底部）返回 false', () => {
      initScrollLock(chatArea);
      // 模拟滚动到底部（scrollTop=800, scrollHeight=1000, clientHeight=200 → dist=0）
      Object.defineProperty(chatArea, 'scrollTop', { configurable: true, get: () => 800, set: () => {} });
      chatArea.dispatchEvent(new Event('scroll'));
      expect(isUserScrolledUp()).toBe(false);
    });

    it('滚动到上方时返回 true', () => {
      initScrollLock(chatArea);
      // 模拟滚动到顶部（scrollTop=0, scrollHeight=1000, clientHeight=200 → dist=800 > 100）
      Object.defineProperty(chatArea, 'scrollTop', { configurable: true, get: () => 0, set: () => {} });
      chatArea.dispatchEvent(new Event('scroll'));
      expect(isUserScrolledUp()).toBe(true);
    });
  });

  describe('shouldAutoScroll', () => {
    it('在底部时返回 true', () => {
      initScrollLock(chatArea);
      // 在底部（dist=0 < 100）
      Object.defineProperty(chatArea, 'scrollTop', { configurable: true, get: () => 800, set: () => {} });
      chatArea.dispatchEvent(new Event('scroll'));
      expect(shouldAutoScroll()).toBe(true);
    });

    it('锁定时返回 false', () => {
      initScrollLock(chatArea);
      // 滚到顶部（dist=800 > 100 → 锁定）
      Object.defineProperty(chatArea, 'scrollTop', { configurable: true, get: () => 0, set: () => {} });
      chatArea.dispatchEvent(new Event('scroll'));
      expect(shouldAutoScroll()).toBe(false);
    });
  });

  describe('resetScrollLock', () => {
    it('重置后 shouldAutoScroll 返回 true', () => {
      initScrollLock(chatArea);
      // 滚到顶部触发锁定
      Object.defineProperty(chatArea, 'scrollTop', { configurable: true, get: () => 0, set: () => {} });
      chatArea.dispatchEvent(new Event('scroll'));
      expect(shouldAutoScroll()).toBe(false);
      resetScrollLock();
      expect(shouldAutoScroll()).toBe(true);
    });
  });

  describe('createJumpToLatestButton', () => {
    it('创建带有正确 class 的按钮', () => {
      const btn = createJumpToLatestButton();
      expect(btn).not.toBeNull();
      expect(btn.className).toContain('jump-to-latest');
    });

    it('按钮包含向下箭头 SVG', () => {
      const btn = createJumpToLatestButton();
      const svg = btn.querySelector('svg');
      expect(svg).not.toBeNull();
    });

    it('按钮初始隐藏（display:none）', () => {
      const btn = createJumpToLatestButton();
      expect(btn.style.display).toBe('none');
    });
  });

  describe('createBackToTopButton', () => {
    it('创建带有 back-to-top class 的按钮', () => {
      const btn = createBackToTopButton();
      expect(btn).not.toBeNull();
      expect(btn.className).toContain('back-to-top');
    });

    it('按钮不包含 jump-to-latest class（独立样式）', () => {
      const btn = createBackToTopButton();
      expect(btn.className).not.toContain('jump-to-latest');
    });

    it('按钮初始隐藏（hidden class）', () => {
      const btn = createBackToTopButton();
      expect(btn.classList.contains('hidden')).toBe(true);
    });

    it('按钮包含向上箭头 SVG', () => {
      const btn = createBackToTopButton();
      const svg = btn.querySelector('svg');
      expect(svg).not.toBeNull();
      // 验证是向上箭头（polyline points 包含 18 15 12 9 6 15）
      const polyline = svg.querySelector('polyline');
      expect(polyline).not.toBeNull();
      expect(polyline.getAttribute('points')).toContain('18 15 12 9 6 15');
    });

    it('按钮无文字 span（纯图标）', () => {
      const btn = createBackToTopButton();
      const span = btn.querySelector('span');
      expect(span).toBeNull();
    });

    it('按钮有 aria-label 属性', () => {
      const btn = createBackToTopButton();
      expect(btn.hasAttribute('aria-label')).toBe(true);
      expect(btn.getAttribute('aria-label').length).toBeGreaterThan(0);
    });

    it('按钮 id 为 backToTopBtn', () => {
      const btn = createBackToTopButton();
      expect(btn.id).toBe('backToTopBtn');
    });
  });

  describe('destroyScrollLock', () => {
    it('调用后不抛异常', () => {
      initScrollLock(chatArea);
      expect(() => destroyScrollLock()).not.toThrow();
    });

    it('调用后 resetScrollLock 状态', () => {
      initScrollLock(chatArea);
      Object.defineProperty(chatArea, 'scrollTop', { configurable: true, get: () => 0, set: () => {} });
      chatArea.dispatchEvent(new Event('scroll'));
      destroyScrollLock();
      expect(shouldAutoScroll()).toBe(true);
    });
  });
});
