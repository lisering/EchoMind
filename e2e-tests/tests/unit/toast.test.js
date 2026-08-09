/**
 * EchoMind Toast 消息系统单元测试 — toast.js。
 *
 * 验证点：
 * 1. toast 创建 DOM 元素并附加到 #toasts 容器
 * 2. 三种类型（info/success/error）有不同样式
 * 3. toastError 自动脱敏（调用 sanitizeError）
 * 4. toastSuccess 显示成功样式
 * 5. 自动消失（setTimeout 后 DOM 移除）
 * 6. 默认类型为 info
 * 7. role="alert" 无障碍标记
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { toast, toastError, toastSuccess } from '../../../ui/src/toast.js';

describe('Toast — toast.js', () => {

  beforeEach(() => {
    // 创建 #toasts 容器
    document.body.innerHTML = '<div id="toasts"></div>';
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = '';
  });

  describe('toast 基础行为', () => {
    it('创建元素并附加到 #toasts', () => {
      toast('测试消息');
      const toasts = document.querySelectorAll('#toasts > div');
      expect(toasts).toHaveLength(1);
      expect(toasts[0].textContent).toBe('测试消息');
    });

    it('role="alert" 无障碍标记', () => {
      toast('提示');
      const el = document.querySelector('#toasts > div');
      expect(el.getAttribute('role')).toBe('alert');
    });

    it('默认类型为 info', () => {
      toast('默认消息');
      const el = document.querySelector('#toasts > div');
      // info 样式含 text-slate-300
      expect(el.className).toContain('text-slate-300');
    });

    it('info 类型样式', () => {
      toast('信息', 'info');
      const el = document.querySelector('#toasts > div');
      expect(el.className).toContain('border-border-default');
      expect(el.className).toContain('text-slate-300');
    });

    it('success 类型样式', () => {
      toast('成功', 'success');
      const el = document.querySelector('#toasts > div');
      expect(el.className).toContain('border-accent');
      expect(el.className).toContain('text-accent');
    });

    it('error 类型样式', () => {
      toast('错误', 'error');
      const el = document.querySelector('#toasts > div');
      expect(el.className).toContain('border-red-400');
      expect(el.className).toContain('text-red-300');
    });

    it('多条 toast 同时存在', () => {
      toast('第一条');
      toast('第二条');
      toast('第三条');
      expect(document.querySelectorAll('#toasts > div')).toHaveLength(3);
    });
  });

  describe('自动消失', () => {
    it('4200ms 后从 DOM 移除', () => {
      vi.useFakeTimers();
      toast('临时消息');
      expect(document.querySelectorAll('#toasts > div')).toHaveLength(1);
      vi.advanceTimersByTime(4200);
      expect(document.querySelectorAll('#toasts > div')).toHaveLength(0);
    });

    it('多条 toast 各自独立消失', () => {
      vi.useFakeTimers();
      toast('第一条');
      vi.advanceTimersByTime(2000);
      toast('第二条');
      expect(document.querySelectorAll('#toasts > div')).toHaveLength(2);
      vi.advanceTimersByTime(2200); // 第一条到期（2000+2200=4200）
      expect(document.querySelectorAll('#toasts > div')).toHaveLength(1);
      vi.advanceTimersByTime(2000); // 第二条到期
      expect(document.querySelectorAll('#toasts > div')).toHaveLength(0);
    });
  });

  describe('toastSuccess', () => {
    it('显示成功样式 toast', () => {
      toastSuccess('操作成功');
      const el = document.querySelector('#toasts > div');
      expect(el.textContent).toBe('操作成功');
      expect(el.className).toContain('text-accent');
    });
  });

  describe('toastError', () => {
    it('脱敏 API Key 后显示', () => {
      toastError('Error: sk-abcd1234efgh5678 invalid');
      const el = document.querySelector('#toasts > div');
      expect(el.textContent).not.toContain('abcd1234efgh5678');
      expect(el.textContent).toContain('sk-****');
    });

    it('脱敏 Unix 用户路径', () => {
      toastError('File /Users/john/data/test.md not found');
      const el = document.querySelector('#toasts > div');
      expect(el.textContent).not.toContain('john');
      expect(el.textContent).toContain('/Users/****/');
    });

    it('处理 Error 对象', () => {
      const err = new Error('Connection failed with sk-1234567890abcdef');
      toastError(err);
      const el = document.querySelector('#toasts > div');
      expect(el.textContent).toContain('sk-****');
    });

    it('错误样式（红色边框）', () => {
      toastError('出错');
      const el = document.querySelector('#toasts > div');
      expect(el.className).toContain('border-red-400');
      expect(el.className).toContain('text-red-300');
    });
  });
});
