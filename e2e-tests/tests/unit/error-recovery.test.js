/**
 * EchoMind error-recovery.js 单元测试 — 错误分类 / 重试 / 降级方案。
 *
 * 验证点：
 * 1. classifyError 网络错误分类为 NETWORK
 * 2. classifyError 认证错误分类为 AUTH
 * 3. classifyError 配额错误分类为 QUOTA
 * 4. isRetryable 网络错误可重试
 * 5. isRetryable 认证错误不可重试
 * 6. friendlyErrorMessage 返回友好消息
 * 7. withRetry 成功时直接返回结果
 * 8. withRetry 重试后成功
 * 9. withRetry 不可重试错误直接抛出
 * 10. withRetryAndToast 失败时显示 toast 并返回 null
 *
 * Mock: i18n.js, toast.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback || key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

import {
  ErrorKind,
  classifyError,
  isRetryable,
  friendlyErrorMessage,
  withRetry,
  withRetryAndToast,
  initErrorBoundary,
  getErrorStats,
} from '../../../ui/src/network-utils.js';

describe('error-recovery.js — 错误分类', () => {
  it('classifyError 网络错误分类为 NETWORK', () => {
    expect(classifyError(new Error('network error'))).toBe(ErrorKind.NETWORK);
    expect(classifyError(new Error('Failed to fetch'))).toBe(ErrorKind.NETWORK);
    expect(classifyError(new Error('timeout'))).toBe(ErrorKind.NETWORK);
  });

  it('classifyError 认证错误分类为 AUTH', () => {
    expect(classifyError(new Error('Unauthorized 401'))).toBe(ErrorKind.AUTH);
    expect(classifyError(new Error('Invalid API key'))).toBe(ErrorKind.AUTH);
    expect(classifyError('403 Forbidden')).toBe(ErrorKind.AUTH);
  });

  it('classifyError 配额错误分类为 QUOTA', () => {
    expect(classifyError(new Error('quota exceeded'))).toBe(ErrorKind.QUOTA);
    expect(classifyError(new Error('plan limit reached'))).toBe(ErrorKind.QUOTA);
  });

  it('classifyError 未找到错误分类为 NOT_FOUND', () => {
    expect(classifyError(new Error('not found'))).toBe(ErrorKind.NOT_FOUND);
    expect(classifyError('404')).toBe(ErrorKind.NOT_FOUND);
  });

  it('classifyError 验证错误分类为 VALIDATION', () => {
    expect(classifyError(new Error('invalid input'))).toBe(ErrorKind.VALIDATION);
    expect(classifyError('400 Bad Request')).toBe(ErrorKind.VALIDATION);
  });

  it('classifyError 内部错误分类为 INTERNAL', () => {
    expect(classifyError(new Error('internal server error'))).toBe(ErrorKind.INTERNAL);
    expect(classifyError('500')).toBe(ErrorKind.INTERNAL);
    expect(classifyError('panic')).toBe(ErrorKind.INTERNAL);
  });

  it('classifyError 未知错误分类为 UNKNOWN', () => {
    expect(classifyError(new Error('something weird'))).toBe(ErrorKind.UNKNOWN);
    expect(classifyError('')).toBe(ErrorKind.UNKNOWN);
  });
});

describe('error-recovery.js — 可重试判断', () => {
  it('isRetryable 网络错误可重试', () => {
    expect(isRetryable(new Error('network error'))).toBe(true);
    expect(isRetryable(new Error('timeout'))).toBe(true);
  });

  it('isRetryable 内部错误可重试', () => {
    expect(isRetryable(new Error('internal error'))).toBe(true);
  });

  it('isRetryable 认证错误不可重试', () => {
    expect(isRetryable(new Error('Unauthorized'))).toBe(false);
  });

  it('isRetryable 配额错误不可重试', () => {
    expect(isRetryable(new Error('quota exceeded'))).toBe(false);
  });
});

describe('error-recovery.js — 友好错误消息', () => {
  it('friendlyErrorMessage 网络错误返回友好消息', () => {
    const msg = friendlyErrorMessage(new Error('network error'));
    expect(msg).toContain('Network error');
  });

  it('friendlyErrorMessage 认证错误返回友好消息', () => {
    const msg = friendlyErrorMessage(new Error('Unauthorized 401'));
    expect(msg).toContain('Authentication failed');
  });

  it('friendlyErrorMessage 配额错误返回友好消息', () => {
    const msg = friendlyErrorMessage(new Error('quota exceeded'));
    expect(msg).toContain('Quota limit reached');
  });

  it('friendlyErrorMessage 未知错误返回原始消息', () => {
    const msg = friendlyErrorMessage(new Error('something weird'));
    expect(msg).toContain('something weird');
  });
});

describe('error-recovery.js — withRetry 重试包装器', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('withRetry 成功时直接返回结果', async () => {
    const fn = vi.fn(() => Promise.resolve('success'));
    const result = await withRetry(fn, { maxRetries: 3, baseDelayMs: 1 });
    expect(result).toBe('success');
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('withRetry 重试后成功', async () => {
    let attempts = 0;
    const fn = vi.fn(() => {
      attempts++;
      if (attempts < 3) return Promise.reject(new Error('network error'));
      return Promise.resolve('success');
    });
    const result = await withRetry(fn, { maxRetries: 3, baseDelayMs: 1 });
    expect(result).toBe('success');
    expect(fn).toHaveBeenCalledTimes(3);
  });

  it('withRetry 不可重试错误直接抛出', async () => {
    const fn = vi.fn(() => Promise.reject(new Error('Unauthorized')));
    await expect(withRetry(fn, { maxRetries: 3, baseDelayMs: 1 })).rejects.toThrow('Unauthorized');
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('withRetry 重试耗尽后抛出最后一个错误', async () => {
    const fn = vi.fn(() => Promise.reject(new Error('network error')));
    await expect(withRetry(fn, { maxRetries: 2, baseDelayMs: 1 })).rejects.toThrow('network error');
    expect(fn).toHaveBeenCalledTimes(3); // initial + 2 retries
  });
});

describe('error-recovery.js — withRetryAndToast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('withRetryAndToast 成功时返回结果', async () => {
    const fn = vi.fn(() => Promise.resolve('ok'));
    const result = await withRetryAndToast(fn, { maxRetries: 1, baseDelayMs: 1 });
    expect(result).toBe('ok');
  });

  it('withRetryAndToast 失败时显示 toast 并返回 null', async () => {
    const { toastError } = await import('../../../ui/src/toast.js');
    const fn = vi.fn(() => Promise.reject(new Error('Unauthorized')));
    const result = await withRetryAndToast(fn, { maxRetries: 1, baseDelayMs: 1 });
    expect(result).toBeNull();
    expect(toastError).toHaveBeenCalled();
  });
});

describe('error-recovery.js — 全局错误边界', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('initErrorBoundary 防止重复初始化', () => {
    initErrorBoundary();
    const stats1 = getErrorStats();
    initErrorBoundary(); // second call should be no-op
    const stats2 = getErrorStats();
    expect(stats1.initialized).toBe(true);
    expect(stats2.initialized).toBe(true);
  });

  it('getErrorStats 返回初始化状态', () => {
    const stats = getErrorStats();
    expect(stats).toHaveProperty('uncaughtCount');
    expect(stats).toHaveProperty('initialized');
  });
});
