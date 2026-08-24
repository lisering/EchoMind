/**
 * EchoMind Toast 消息系统 — 跨模块共享的 UI 反馈层。
 *
 * 设计原则：
 * 1. 单一 Toast 入口 — 所有模块通过此模块显示反馈
 * 2. 自动消失 — 4.2 秒后从 DOM 移除
 * 3. 四种语义 — info / success / error / warning
 *
 * REQ-ERR-001：错误前缀感知 — toastError 解析 `PREFIX: detail` 格式，
 * 根据前缀选择 toast kind 和用户友好消息。
 */

import { sanitizeError, $ } from './utils.js';
import { t } from './i18n.js';

/**
 * 错误前缀 → 用户友好消息映射表（REQ-ERR-001）。
 * VALIDATION 前缀特殊处理：显示原始消息（去掉前缀），kind=warning。
 */
const ERROR_PREFIX_MESSAGES = {
  NETWORK: { kind: 'error', msgKey: 'errors.network' },
  AUTH: { kind: 'error', msgKey: 'errors.auth' },
  LLM: { kind: 'error', msgKey: 'errors.llm' },
  PARSE: { kind: 'error', msgKey: 'errors.parse' },
  EMBED: { kind: 'error', msgKey: 'errors.embed' },
  STORAGE: { kind: 'error', msgKey: 'errors.storage' },
  DISK_FULL: { kind: 'error', msgKey: 'errors.disk_full' },
  RATE_LIMIT: { kind: 'warning', msgKey: 'errors.rate_limit' },
  UNKNOWN: { kind: 'error', msgKey: 'errors.unknown' },
};

/**
 * 显示一条 Toast 提示消息，4.2 秒后自动消失。
 * @param {string} message - 提示文本
 * @param {'info'|'error'|'success'|'warning'} [kind='info'] - 提示类型（决定颜色样式）
 */
export function toast(message, kind = 'info') {
  const el = document.createElement('div');
  const color = kind === 'error'
    ? 'border-red-400/40 text-red-300'
    : kind === 'success'
      ? 'border-accent/50 text-accent'
      : kind === 'warning'
        ? 'border-amber-400/40 text-amber-300'
        : 'border-border-default text-slate-300';
  el.className = `bg-surface-1 border ${color} rounded-xl px-4 py-3 text-sm shadow-lg animate-fade-in`;
  el.setAttribute('role', 'alert');
  el.textContent = message;
  $('toasts').appendChild(el);
  setTimeout(() => el.remove(), 4200);
}

/**
 * 成功 Toast 快捷函数。
 * @param {string} message - 提示文本
 */
export function toastSuccess(message) {
  toast(message, 'success');
}

/**
 * 错误 Toast 快捷函数：自动脱敏后展示（REQ-UI-005-AC-2）。
 *
 * REQ-ERR-001：解析错误前缀，根据前缀选择 toast kind 和用户友好消息。
 * - `NETWORK:` → error「网络连接异常」
 * - `AUTH:` → error「认证失败，请检查 API Key」
 * - `LLM:` → error「LLM 服务异常」
 * - `PARSE:` → error「文件解析失败」
 * - `EMBED:` → error「向量化失败」
 * - `STORAGE:` → error「存储异常」
 * - `DISK_FULL:` → error「磁盘空间不足，请清理文件」
 * - `RATE_LIMIT:` → warning「请求过于频繁，请稍后重试」
 * - `VALIDATION:` → warning（显示原始消息）
 * - `LIMIT_REACHED:` / `PRO_REQUIRED:` → 原样返回（调用方负责弹出付费墙）
 * - `UNKNOWN:` → error「未知错误」
 * - 无前缀 → error（脱敏后显示原始消息）
 *
 * @param {string|Error} err - 原始错误
 * @returns {string} 处理后的错误消息（供调用方进一步处理，如触发 paywall）
 */
export function toastError(err) {
  const raw = String(err);
  const sanitized = sanitizeError(raw);

  // 解析错误前缀（PREFIX: detail 格式）
  const colonIdx = sanitized.indexOf(':');
  if (colonIdx > 0) {
    const prefix = sanitized.substring(0, colonIdx);
    const detail = sanitized.substring(colonIdx + 1).trim();

    // VALIDATION: 显示原始消息，kind=warning
    if (prefix === 'VALIDATION') {
      toast(detail || sanitized, 'warning');
      return sanitized;
    }

    // LIMIT_REACHED / PRO_REQUIRED: 原样返回（调用方负责弹出付费墙）
    if (prefix === 'LIMIT_REACHED' || prefix === 'PRO_REQUIRED') {
      return sanitized;
    }

    // 已知错误前缀 → 用户友好消息
    const mapping = ERROR_PREFIX_MESSAGES[prefix];
    if (mapping) {
      toast(t(mapping.msgKey, { detail: detail || '' }), mapping.kind);
      return sanitized;
    }
  }

  // 无前缀或未知前缀 → 脱敏后直接展示
  toast(sanitized, 'error');
  return sanitized;
}
