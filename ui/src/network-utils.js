/**
 * EchoMind 网络工具模块 — 离线检测 + 错误恢复。
 *
 * 合并自：offline-detector.js (REQ-ERR-003) + error-recovery.js (REQ-ERR-002 v1.5)
 *
 * 职责：
 * 1. 监听 navigator.onLine + online/offline 事件
 * 2. 离线时显示离线指示器 + 禁用发送
 * 3. 全局未捕获错误边界 — window.onerror / unhandledrejection 拦截
 * 4. 自动重试包装器 — withRetry() 指数退避
 * 5. 错误分类 + 用户友好消息
 */

// ============================================================
// 导入
// ============================================================

import { $ } from './utils.js';
import { t } from './i18n.js';
import { icon } from './utils.js';
import { toast, toastError } from './toast.js';

// ============================================================
// 离线检测（原 offline-detector.js）
// ============================================================

let _initialized = false;
let _originalPlaceholder = null;
let _originalHint = null;

/**
 * 获取或创建离线指示器 DOM 元素。
 */
function _getOfflineIndicator() {
  let el = $('offlineIndicator');
  if (!el) {
    const sidebarFooter = document.querySelector('#sidebar .mt-auto');
    if (!sidebarFooter) return null;
    el = document.createElement('div');
    el.id = 'offlineIndicator';
    el.className = 'offline-indicator hidden';
    sidebarFooter.appendChild(el);
  }
  return el;
}

/**
 * 离线状态 UI 更新。
 */
function _applyOffline() {
  const indicator = _getOfflineIndicator();
  if (indicator) {
    indicator.classList.remove('hidden');
    indicator.innerHTML = `<span class="text-amber-400 text-xs flex items-center gap-1">
      ${icon('warning', 'sm')}
      <span>${t('offline.indicator', 'Offline')}</span>
    </span>`;
  }

  const sendBtn = $('sendBtn');
  if (sendBtn) sendBtn.setAttribute('disabled', '');

  const input = $('queryInput');
  if (input) {
    if (_originalPlaceholder === null) {
      _originalPlaceholder = input.getAttribute('placeholder') || '';
    }
    input.setAttribute('placeholder', t('offline.placeholder', 'Offline — check network connection'));
  }

  const hint = $('inputHint');
  if (hint) {
    if (_originalHint === null) _originalHint = hint.innerHTML;
    hint.innerHTML = `<span class="text-amber-400 flex items-center gap-1">${icon('warning', 'sm')} ${t('offline.hint', 'Offline mode — please check your network connection')}</span>`;
  }
}

/**
 * 在线状态 UI 恢复。
 */
function _applyOnline() {
  const indicator = _getOfflineIndicator();
  if (indicator) {
    indicator.classList.add('hidden');
    indicator.innerHTML = '';
  }

  const sendBtn = $('sendBtn');
  if (sendBtn) sendBtn.removeAttribute('disabled');

  const input = $('queryInput');
  if (input && _originalPlaceholder !== null) {
    input.setAttribute('placeholder', _originalPlaceholder);
  }

  const hint = $('inputHint');
  if (hint && _originalHint !== null) hint.innerHTML = _originalHint;
}

function _updateUI(isOnline) {
  if (isOnline) _applyOnline();
  else _applyOffline();
}

function _onOnline() { _updateUI(true); }
function _onOffline() { _updateUI(false); }

/**
 * 获取当前在线状态。
 */
export function isOnline() {
  return navigator.onLine;
}

/**
 * 初始化离线检测模块。
 */
export function initOfflineDetector() {
  if (_initialized) return;
  _initialized = true;
  window.addEventListener('online', _onOnline);
  window.addEventListener('offline', _onOffline);
  _updateUI(navigator.onLine);
}

/**
 * 手动触发离线状态（测试用）。
 */
export function _setOfflineForTest(offline) {
  _updateUI(!offline);
}

// ============================================================
// 错误分类与恢复（原 error-recovery.js）
// ============================================================

/**
 * 错误分类枚举。
 * @readonly
 * @enum {string}
 */
export const ErrorKind = {
  NETWORK: 'network',
  QUOTA: 'quota',
  AUTH: 'auth',
  NOT_FOUND: 'not_found',
  VALIDATION: 'validation',
  INTERNAL: 'internal',
  UNKNOWN: 'unknown',
};

/**
 * 将原始错误对象分类为 ErrorKind。
 */
export function classifyError(err) {
  const msg = typeof err === 'string' ? err : (err?.message || String(err || ''));
  const lower = msg.toLowerCase();

  if (lower.includes('network') || lower.includes('fetch') || lower.includes('timeout') || lower.includes('econnrefused') || lower.includes('failed to fetch')) {
    return ErrorKind.NETWORK;
  }
  if (lower.includes('quota') || lower.includes('limit') || lower.includes('plan')) {
    return ErrorKind.QUOTA;
  }
  if (lower.includes('auth') || lower.includes('unauthorized') || lower.includes('401') || lower.includes('403') || lower.includes('api key')) {
    return ErrorKind.AUTH;
  }
  if (lower.includes('not found') || lower.includes('404')) {
    return ErrorKind.NOT_FOUND;
  }
  if (lower.includes('validation') || lower.includes('invalid') || lower.includes('400')) {
    return ErrorKind.VALIDATION;
  }
  if (lower.includes('internal') || lower.includes('500') || lower.includes('panic')) {
    return ErrorKind.INTERNAL;
  }
  return ErrorKind.UNKNOWN;
}

/**
 * 判断错误是否可重试。
 */
export function isRetryable(err) {
  const kind = classifyError(err);
  return kind === ErrorKind.NETWORK || kind === ErrorKind.INTERNAL;
}

/**
 * 将错误转为用户友好消息。
 */
export function friendlyErrorMessage(err) {
  const kind = classifyError(err);
  const original = typeof err === 'string' ? err : (err?.message || String(err || ''));

  switch (kind) {
    case ErrorKind.NETWORK:
      return t('error_recovery.network', 'Network error. Please check your connection and retry.');
    case ErrorKind.QUOTA:
      return t('error_recovery.quota', 'Quota limit reached. Upgrade to Pro for more capacity.');
    case ErrorKind.AUTH:
      return t('error_recovery.auth', 'Authentication failed. Please check your API key in Settings.');
    case ErrorKind.NOT_FOUND:
      return t('error_recovery.not_found', 'Resource not found. It may have been deleted.');
    case ErrorKind.VALIDATION:
      return t('error_recovery.validation', 'Invalid input. Please check your data.');
    case ErrorKind.INTERNAL:
      return t('error_recovery.internal', 'Internal error. Retrying…');
    default:
      return original || t('error_recovery.unknown', 'An unexpected error occurred.');
  }
}

// ============================================================
// 自动重试包装器
// ============================================================

/** @typedef {{maxRetries: number, baseDelayMs: number, maxDelayMs: number, retryFilter: (err: any) => boolean}} RetryConfig */

const DEFAULT_RETRY_CONFIG = {
  maxRetries: 3,
  baseDelayMs: 500,
  maxDelayMs: 5000,
  retryFilter: isRetryable,
};

/**
 * 指数退避延迟计算（带抖动）。
 */
function backoffDelay(attempt, config) {
  const exponential = config.baseDelayMs * Math.pow(2, attempt);
  const capped = Math.min(exponential, config.maxDelayMs);
  const jitter = 0.5 + Math.random() * 0.5;
  return Math.round(capped * jitter);
}

/**
 * 对异步操作进行自动重试包装。
 */
export async function withRetry(fn, configOverride = {}) {
  const config = { ...DEFAULT_RETRY_CONFIG, ...configOverride };
  let lastError = null;

  for (let attempt = 0; attempt <= config.maxRetries; attempt++) {
    try {
      return await fn();
    } catch (err) {
      lastError = err;
      if (attempt >= config.maxRetries || !config.retryFilter(err)) {
        throw err;
      }
      const delay = backoffDelay(attempt, config);
      await new Promise((resolve) => setTimeout(resolve, delay));
    }
  }

  throw lastError;
}

/**
 * 带重试和友好错误提示的异步操作包装器。
 */
export async function withRetryAndToast(fn, config = {}) {
  try {
    return await withRetry(fn, config);
  } catch (err) {
    toastError(friendlyErrorMessage(err));
    return null;
  }
}

// ============================================================
// 全局错误边界
// ============================================================

let _errorBoundaryInitialized = false;
let _uncaughtCount = 0;

/**
 * 初始化全局错误边界。
 */
export function initErrorBoundary() {
  if (_errorBoundaryInitialized) return;
  _errorBoundaryInitialized = true;

  window.addEventListener('error', (event) => {
    _uncaughtCount++;
    const err = event.error || event.message || 'Unknown error';
    const kind = classifyError(err);

    if (kind === ErrorKind.NETWORK) {
      toast(friendlyErrorMessage(err), 'error');
    } else {
      console.error('[ErrorBoundary]', err);
    }
  });

  window.addEventListener('unhandledrejection', (event) => {
    _uncaughtCount++;
    const err = event.reason || 'Unhandled rejection';
    const kind = classifyError(err);

    if (kind === ErrorKind.NETWORK) {
      toast(friendlyErrorMessage(err), 'error');
      event.preventDefault?.();
    } else {
      console.error('[ErrorBoundary] unhandledrejection:', err);
    }
  });
}

/**
 * 获取全局错误统计。
 */
export function getErrorStats() {
  return {
    uncaughtCount: _uncaughtCount,
    initialized: _errorBoundaryInitialized,
  };
}
