/**
 * EchoMind 统一确认对话框单元测试 — confirm-dialog.js（REQ-IX-005）。
 *
 * 验证点：
 * 1. showConfirmDialog() 返回 Promise
 * 2. 确认按钮点击后 resolve(true)
 * 3. 取消按钮点击后 resolve(false)
 * 4. Esc 键关闭返回 false
 * 5. Enter 键确认返回 true（仅 500ms 防误触后生效）
 * 6. 点击遮罩层关闭返回 false
 * 7. 确认按钮初始 disabled（500ms 防误触）
 * 8. danger=true 时确认按钮为红色变体
 * 9. danger=false 时确认按钮为 accent 变体
 * 10. 弹窗出现后 z-index 为 55
 * 11. role="alertdialog" 无障碍标记
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// Mock i18n before importing confirm-dialog
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => {
    const map = {
      'common.confirm': 'Confirm',
      'common.cancel': 'Cancel',
    };
    return map[key] || key;
  },
}));

import { showConfirmDialog } from '../../../ui/src/confirm-dialog.js';

describe('ConfirmDialog — confirm-dialog.js（REQ-IX-005）', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = '';
  });

  describe('基础行为', () => {
    it('showConfirmDialog 返回 Promise', () => {
      const result = showConfirmDialog({ title: '测试' });
      expect(result).toBeInstanceOf(Promise);
      // 清理：快速关闭
      vi.advanceTimersByTime(600);
      const cancelBtn = document.querySelector('[data-role="cancel"]');
      cancelBtn.click();
    });

    it('创建 DOM 元素并附加到 body', () => {
      showConfirmDialog({ title: '测试标题' });
      const dialog = document.getElementById('confirmDialog');
      expect(dialog).toBeTruthy();
      expect(dialog.getAttribute('role')).toBe('alertdialog');
      expect(dialog.getAttribute('aria-modal')).toBe('true');
    });

    it('z-index 为 55', () => {
      showConfirmDialog({ title: '测试' });
      const dialog = document.getElementById('confirmDialog');
      expect(dialog.className).toContain('z-[55]');
    });

    it('标题正确渲染', () => {
      showConfirmDialog({ title: '删除文档？' });
      const title = document.getElementById('confirmDialogTitle');
      expect(title.textContent).toBe('删除文档？');
    });

    it('正文正确渲染', () => {
      showConfirmDialog({ title: '测试', body: '此操作不可撤销。' });
      const body = document.getElementById('confirmDialogBody');
      expect(body.textContent).toContain('此操作不可撤销');
    });
  });

  describe('确认/取消按钮', () => {
    it('确认按钮点击后 resolve(true)', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      // 等待 500ms 防误触延迟
      vi.advanceTimersByTime(600);
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      expect(confirmBtn.disabled).toBe(false);
      confirmBtn.click();
      const result = await promise;
      expect(result).toBe(true);
    });

    it('取消按钮点击后 resolve(false)', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      const cancelBtn = document.querySelector('[data-role="cancel"]');
      cancelBtn.click();
      const result = await promise;
      expect(result).toBe(false);
    });
  });

  describe('键盘交互', () => {
    it('Esc 键关闭返回 false', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
      document.dispatchEvent(event);
      const result = await promise;
      expect(result).toBe(false);
    });

    it('Enter 键在 500ms 内不生效（防误触）', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      // 不等待 500ms，直接按 Enter
      const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true });
      document.dispatchEvent(event);
      // 对话框应该还在
      const dialog = document.getElementById('confirmDialog');
      expect(dialog).toBeTruthy();
      // 清理
      vi.advanceTimersByTime(600);
      const cancelBtn = document.querySelector('[data-role="cancel"]');
      cancelBtn.click();
      const result = await promise;
      expect(result).toBe(false);
    });

    it('Enter 键在 500ms 后确认返回 true', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      vi.advanceTimersByTime(600);
      const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true });
      document.dispatchEvent(event);
      const result = await promise;
      expect(result).toBe(true);
    });
  });

  describe('点击遮罩层', () => {
    it('点击遮罩层关闭返回 false', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      const overlay = document.getElementById('confirmDialog');
      // 模拟点击遮罩层本身（非内部容器）
      const clickEvent = new MouseEvent('click', { bubbles: true });
      Object.defineProperty(clickEvent, 'target', { value: overlay });
      overlay.dispatchEvent(clickEvent);
      const result = await promise;
      expect(result).toBe(false);
    });
  });

  describe('防误触机制', () => {
    it('确认按钮初始为 disabled', () => {
      showConfirmDialog({ title: '测试' });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      expect(confirmBtn.disabled).toBe(true);
    });

    it('500ms 后确认按钮启用', () => {
      showConfirmDialog({ title: '测试' });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      expect(confirmBtn.disabled).toBe(true);
      vi.advanceTimersByTime(500);
      expect(confirmBtn.disabled).toBe(false);
    });

    it('disabled 状态下点击确认按钮不触发', async () => {
      const promise = showConfirmDialog({ title: '测试' });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      // 500ms 内点击
      confirmBtn.click();
      // 对话框应该还在
      const dialog = document.getElementById('confirmDialog');
      expect(dialog).toBeTruthy();
      // 清理
      vi.advanceTimersByTime(600);
      const cancelBtn = document.querySelector('[data-role="cancel"]');
      cancelBtn.click();
      const result = await promise;
      expect(result).toBe(false);
    });
  });

  describe('danger 变体样式', () => {
    it('danger=true 时确认按钮为红色变体', () => {
      showConfirmDialog({ title: '测试', danger: true });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      expect(confirmBtn.className).toContain('bg-red-500');
    });

    it('danger=false 时确认按钮为 accent 变体', () => {
      showConfirmDialog({ title: '测试', danger: false });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      expect(confirmBtn.className).toContain('bg-accent');
      expect(confirmBtn.className).not.toContain('bg-red-500');
    });

    it('danger=true 时显示警告图标', () => {
      showConfirmDialog({ title: '测试', danger: true });
      const icon = document.querySelector('#confirmDialog .text-amber-400');
      expect(icon).toBeTruthy();
      // v1.21: 从 emoji ⚠️ 改为 SVG inline 警告图标
      const svg = icon.querySelector('svg');
      expect(svg).not.toBeNull();
    });

    it('danger=false 时不显示警告图标', () => {
      showConfirmDialog({ title: '测试', danger: false });
      const icon = document.querySelector('#confirmDialog .text-amber-400');
      expect(icon).toBeFalsy();
    });
  });

  describe('自定义文案', () => {
    it('confirmText 自定义', () => {
      showConfirmDialog({ title: '测试', confirmText: '删除' });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      expect(confirmBtn.textContent).toBe('删除');
    });

    it('cancelText 自定义', () => {
      showConfirmDialog({ title: '测试', cancelText: '关闭' });
      const cancelBtn = document.querySelector('[data-role="cancel"]');
      expect(cancelBtn.textContent).toBe('关闭');
    });

    it('默认文案来自 i18n', () => {
      showConfirmDialog({ title: '测试' });
      const confirmBtn = document.querySelector('[data-role="confirm"]');
      const cancelBtn = document.querySelector('[data-role="cancel"]');
      expect(confirmBtn.textContent).toBe('Confirm');
      expect(cancelBtn.textContent).toBe('Cancel');
    });
  });

  describe('单例模式', () => {
    it('同时只存在一个对话框', () => {
      showConfirmDialog({ title: '第一个' });
      showConfirmDialog({ title: '第二个' });
      const dialogs = document.querySelectorAll('#confirmDialog');
      expect(dialogs).toHaveLength(1);
      const title = document.getElementById('confirmDialogTitle');
      expect(title.textContent).toBe('第二个');
    });
  });
});
