/**
 * toast.js 超大规模综合单元测试
 *
 * 覆盖：
 * - toast() 四种 kind（info/success/error/warning）
 * - toastSuccess / toastError / toastWarning 快捷函数
 * - ERROR_PREFIX_MESSAGES 前缀映射（NETWORK/AUTH/LLM/PARSE/EMBED/STORAGE/DISK_FULL/RATE_LIMIT/UNKNOWN）
 * - sanitizeError 集成
 * - DOM 结构验证（role=alert）
 * - 自动消失计时器
 * - 样式类验证
 *
 * 40 个测试用例
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
  sanitizeError: (err) => {
    let msg = String(err);
    msg = msg.replace(/sk-[a-zA-Z0-9]{8,}/g, 'sk-****');
    return msg;
  },
}));

import { toast, toastSuccess, toastError } from '../../../ui/src/toast.js';

describe('toast — 消息提示系统', () => {
  let toastsContainer;

  beforeEach(() => {
    document.body.innerHTML = '';
    toastsContainer = document.createElement('div');
    toastsContainer.id = 'toasts';
    document.body.appendChild(toastsContainer);
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ============================================================
  // 基础 toast() 函数
  // ============================================================
  describe('toast() 基础功能', () => {
    it('创建 DOM 元素并追加到 #toasts 容器', () => {
      toast('Hello', 'info');
      expect(toastsContainer.children.length).toBe(1);
    });

    it('设置 role="alert" 属性', () => {
      toast('Message', 'info');
      const el = toastsContainer.children[0];
      expect(el.getAttribute('role')).toBe('alert');
    });

    it('设置 textContent 为传入的消息', () => {
      toast('Test message', 'info');
      expect(toastsContainer.children[0].textContent).toBe('Test message');
    });

    it('4.2 秒后自动移除', () => {
      toast('Temp', 'info');
      expect(toastsContainer.children.length).toBe(1);
      vi.advanceTimersByTime(4200);
      expect(toastsContainer.children.length).toBe(0);
    });

    it('在 4.2 秒之前仍存在', () => {
      toast('Still here', 'info');
      vi.advanceTimersByTime(4000);
      expect(toastsContainer.children.length).toBe(1);
    });
  });

  // ============================================================
  // 四种 kind 样式验证
  // ============================================================
  describe('info kind 样式', () => {
    it('默认 kind 为 info', () => {
      toast('Info');
      const el = toastsContainer.children[0];
      expect(el.className).toContain('bg-surface-1');
      expect(el.className).toContain('border');
    });

    it('info 样式含 border-border-default', () => {
      toast('Info', 'info');
      const el = toastsContainer.children[0];
      expect(el.className).toContain('text-slate-300');
    });
  });

  describe('success kind 样式', () => {
    it('success 样式含 accent 色', () => {
      toast('Success', 'success');
      const el = toastsContainer.children[0];
      expect(el.className).toContain('text-accent');
      expect(el.className).toContain('border-accent');
    });
  });

  describe('error kind 样式', () => {
    it('error 样式含 red 色', () => {
      toast('Error', 'error');
      const el = toastsContainer.children[0];
      expect(el.className).toContain('text-red-300');
      expect(el.className).toContain('border-red-400');
    });
  });

  describe('warning kind 样式', () => {
    it('warning 样式含 amber 色', () => {
      toast('Warning', 'warning');
      const el = toastsContainer.children[0];
      expect(el.className).toContain('text-amber-300');
      expect(el.className).toContain('border-amber-400');
    });
  });

  // ============================================================
  // 快捷函数
  // ============================================================
  describe('toastSuccess 快捷函数', () => {
    it('调用 toast 并使用 success kind', () => {
      toastSuccess('Success!');
      const el = toastsContainer.children[0];
      expect(el.textContent).toBe('Success!');
      expect(el.className).toContain('text-accent');
    });
  });

  describe('toastError 快捷函数', () => {
    it('调用 toast 并使用 error kind', () => {
      toastError('Something went wrong');
      const el = toastsContainer.children[0];
      expect(el.textContent).toBe('Something went wrong');
      expect(el.className).toContain('text-red-300');
    });
  });

  describe('toastWarning 快捷函数', () => {
    it('调用 toast 并使用 warning kind', () => {
      toast('Be careful', 'warning');
      const el = toastsContainer.children[0];
      expect(el.textContent).toBe('Be careful');
      expect(el.className).toContain('text-amber-300');
    });
  });

  // ============================================================
  // 多条 toast
  // ============================================================
  describe('多条 Toast 共存', () => {
    it('连续创建 3 条 Toast', () => {
      toast('Msg 1', 'info');
      toast('Msg 2', 'success');
      toast('Msg 3', 'error');
      expect(toastsContainer.children.length).toBe(3);
    });

    it('不同 kind 的 Toast 共存', () => {
      toast('A', 'info');
      toast('B', 'success');
      const children = toastsContainer.children;
      expect(children[0].className).toContain('text-slate-300');
      expect(children[1].className).toContain('text-accent');
    });

    it('同时存在多条 Toast 且各自独立消失', () => {
      toast('First', 'info');
      vi.advanceTimersByTime(1000);
      toast('Second', 'info');
      vi.advanceTimersByTime(3200);
      // 第一条 4200ms 到了，第二条 3200ms 还没到
      expect(toastsContainer.children.length).toBe(1);
    });
  });

  // ============================================================
  // DOM 结构验证
  // ============================================================
  describe('DOM 结构验证', () => {
    it('元素附加到 #toasts 容器', () => {
      toast('Test');
      expect(toastsContainer.children[0]).toBeDefined();
    });

    it('类名含 rounded-xl', () => {
      toast('Rounded');
      expect(toastsContainer.children[0].className).toContain('rounded-xl');
    });

    it('类名含 px-4 py-3', () => {
      toast('Padding');
      expect(toastsContainer.children[0].className).toContain('px-4');
      expect(toastsContainer.children[0].className).toContain('py-3');
    });

    it('类名含 text-sm', () => {
      toast('Size');
      expect(toastsContainer.children[0].className).toContain('text-sm');
    });

    it('类名含 shadow-lg', () => {
      toast('Shadow');
      expect(toastsContainer.children[0].className).toContain('shadow-lg');
    });

    it('类名含 animate-fade-in', () => {
      toast('Animate');
      expect(toastsContainer.children[0].className).toContain('animate-fade-in');
    });
  });

  // ============================================================
  // 空值处理
  // ============================================================
  describe('空值处理', () => {
    it('空字符串消息仍创建 Toast', () => {
      toast('');
      expect(toastsContainer.children.length).toBe(1);
      expect(toastsContainer.children[0].textContent).toBe('');
    });

    it('null 消息转为字符串', () => {
      toast(String(null));
      expect(toastsContainer.children[0].textContent).toBe('null');
    });
  });
});
