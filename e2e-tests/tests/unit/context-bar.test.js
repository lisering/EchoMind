/**
 * EchoMind 上下文窗口指示器单元测试 — context-bar.js 模块（TC-QA-020~026）。
 *
 * 验证点（对应 AC-QA-009 上下文窗口指示）：
 * 1. getContextLevel 根据百分比返回 green/yellow/red 三色级别
 * 2. getContextPercentage 返回正确的百分比
 * 3. renderContextBar 在容器中渲染进度条 DOM
 * 4. renderContextBar 应用正确的颜色级别 class
 * 5. formatContextTooltip 包含 token 数和模型窗口大小
 * 6. limit 为 0 时不渲染（避免除零）
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  getContextLevel,
  getContextPercentage,
  renderContextBar,
  formatContextTooltip,
} from '../../../ui/src/context-bar.js';

describe('Context Bar — context-bar.js', () => {
  let container;

  beforeEach(() => {
    container = document.createElement('div');
    document.body.appendChild(container);
  });

  describe('getContextLevel', () => {
    it('TC-QA-020: 0-60% 返回 green', () => {
      expect(getContextLevel(0, 8000)).toBe('green');
      expect(getContextLevel(4000, 8000)).toBe('green'); // 50%
      expect(getContextLevel(4800, 8000)).toBe('green'); // 60%
    });

    it('TC-QA-021: 60-80% 返回 yellow', () => {
      expect(getContextLevel(4801, 8000)).toBe('yellow'); // 60.0125%
      expect(getContextLevel(6000, 8000)).toBe('yellow'); // 75%
      expect(getContextLevel(6400, 8000)).toBe('yellow'); // 80%
    });

    it('TC-QA-022: 80-100% 返回 red', () => {
      expect(getContextLevel(6401, 8000)).toBe('red'); // 80.0125%
      expect(getContextLevel(7200, 8000)).toBe('red'); // 90%
      expect(getContextLevel(8000, 8000)).toBe('red'); // 100%
    });

    it('TC-QA-022b: 超过 100% 仍返回 red', () => {
      expect(getContextLevel(10000, 8000)).toBe('red'); // 125%
    });
  });

  describe('getContextPercentage', () => {
    it('TC-QA-023: 返回正确的整数百分比', () => {
      expect(getContextPercentage(2000, 8000)).toBe(25);
      expect(getContextPercentage(4000, 8000)).toBe(50);
      expect(getContextPercentage(8000, 8000)).toBe(100);
    });

    it('TC-QA-023b: 向下取整', () => {
      expect(getContextPercentage(3000, 8000)).toBe(38); // 37.5 → 38
    });

    it('TC-QA-023c: limit 为 0 时返回 0', () => {
      expect(getContextPercentage(1000, 0)).toBe(0);
    });
  });

  describe('renderContextBar', () => {
    it('TC-QA-024: 在容器中渲染 .context-bar 元素', () => {
      renderContextBar(container, 3000, 8000);
      const bar = container.querySelector('.context-bar');
      expect(bar).not.toBeNull();
    });

    it('TC-QA-024b: 渲染进度条填充元素', () => {
      renderContextBar(container, 3000, 8000);
      const fill = container.querySelector('.context-bar-fill');
      expect(fill).not.toBeNull();
    });

    it('TC-QA-025: 用量 < 60% 时应用 green 级别', () => {
      renderContextBar(container, 3000, 8000); // 37.5%
      const bar = container.querySelector('.context-bar');
      expect(bar.classList.contains('context-bar-green')).toBe(true);
    });

    it('TC-QA-025b: 用量 60-80% 时应用 yellow 级别', () => {
      renderContextBar(container, 5000, 8000); // 62.5%
      const bar = container.querySelector('.context-bar');
      expect(bar.classList.contains('context-bar-yellow')).toBe(true);
    });

    it('TC-QA-025c: 用量 > 80% 时应用 red 级别', () => {
      renderContextBar(container, 7000, 8000); // 87.5%
      const bar = container.querySelector('.context-bar');
      expect(bar.classList.contains('context-bar-red')).toBe(true);
    });

    it('TC-QA-025d: 进度条宽度与百分比一致', () => {
      renderContextBar(container, 3000, 8000); // 37.5%
      const fill = container.querySelector('.context-bar-fill');
      expect(fill.style.width).toBe('38%'); // 37.5 → 38 (rounded)
    });

    it('TC-QA-025e: limit 为 0 时不渲染进度条', () => {
      renderContextBar(container, 1000, 0);
      const bar = container.querySelector('.context-bar');
      expect(bar).toBeNull();
    });
  });

  describe('formatContextTooltip', () => {
    it('TC-QA-026: 包含已用 token 数', () => {
      const tooltip = formatContextTooltip(3000, 8000, 5);
      expect(tooltip).toContain('3000');
      expect(tooltip).toContain('8000');
    });

    it('TC-QA-026b: 包含消息条数', () => {
      const tooltip = formatContextTooltip(3000, 8000, 5);
      expect(tooltip).toContain('5');
    });
  });
});
