/**
 * EchoMind 前端防御性编程守卫（Defensive Guards）。
 *
 * 设计原则：
 * 1. 每个依赖后端前置条件的用户操作都应有前端层 UX 防御
 * 2. 防御分两层：UI 禁用（prevent input）+ 函数拦截（early return）
 * 3. 防御守卫集中管理，避免散落在各模块中
 *
 * 根因分析 V4 修复：前后端防御缺口（后端有 count_documents==0 防御，前端无对应 UX 防御）
 */

import { get } from './state.js';
import { $ } from './utils.js';
import { t } from './i18n.js';
import { toastError } from './toast.js';

// ============================================================
// 守卫类型定义
// ============================================================

/**
 * @typedef {Object} GuardResult
 * @property {boolean} passed - 是否通过守卫检查
 * @property {string} [reason] - 未通过时的原因（i18n key）
 */

// ============================================================
// 知识库前置条件守卫
// ============================================================

/**
 * 检查知识库是否有文档。
 * @returns {GuardResult}
 */
export function requireDocuments() {
  if (get('docCount') === 0) {
    return { passed: false, reason: 'chat.empty_kb_error' };
  }
  return { passed: true };
}

/**
 * 检查当前是否可以发送消息（非流式 + 有文档）。
 * @returns {GuardResult}
 */
export function canSend() {
  if (get('streaming')) {
    return { passed: false, reason: 'chat.streaming_hint' };
  }
  return requireDocuments();
}

// ============================================================
// Pro 授权守卫
// ============================================================

/**
 * 检查是否为 Pro 用户。
 * @returns {GuardResult}
 */
export function requirePro() {
  if (!get('isPro')) {
    return { passed: false, reason: 'paywall.reason_default' };
  }
  return { passed: true };
}

// ============================================================
// 流式状态守卫
// ============================================================

/**
 * 检查当前是否可以进行新操作（非流式、非审计中）。
 * @returns {GuardResult}
 */
export function requireIdle() {
  if (get('streaming')) {
    return { passed: false, reason: 'chat.streaming_hint' };
  }
  if (get('auditingDocId')) {
    return { passed: false, reason: 'chat.thinking_auditing' };
  }
  return { passed: true };
}

// ============================================================
// 守卫执行器 — 统一执行守卫检查 + UI 反馈
// ============================================================

/**
 * 执行守卫检查：通过返回 true，未通过显示 toast 并返回 false。
 *
 * @param {GuardResult} result - 守卫检查结果
 * @returns {boolean} 是否通过
 *
 * @example
 * if (!runGuard(canSend())) return;
 * // 继续执行发送逻辑...
 */
export function runGuard(result) {
  if (!result.passed) {
    if (result.reason) {
      toastError(t(result.reason));
    }
    return false;
  }
  return true;
}

// ============================================================
// UI 状态同步 — 根据守卫条件更新 UI 控件状态
// ============================================================

/**
 * 同步聊天输入框和发送按钮的禁用状态。
 * 根据知识库文档数和流式状态自动启用/禁用。
 *
 * 注意：流式期间输入框保持启用（支持排队发送），发送按钮由 updateSendButton() 管理为停止形态。
 * 仅在非流式时根据文档数控制禁用状态。与 setInputState() 行为保持一致。
 */
export function syncChatInputState() {
  const docCount = get('docCount');
  const streaming = get('streaming');
  const input = $('queryInput');
  const sendBtn = $('sendBtn');
  if (!input || !sendBtn) return;

  if (streaming) {
    // 流式期间：输入框保持启用（排队发送），不触碰 sendBtn（由 updateSendButton 管理）
    input.disabled = false;
  } else {
    // 非流式：根据文档数控制
    const shouldDisable = docCount === 0;
    input.disabled = shouldDisable;
    sendBtn.disabled = shouldDisable;
    if (shouldDisable) {
      input.setAttribute('placeholder', t('chat.empty_kb_placeholder'));
      sendBtn.classList.add('opacity-30', 'cursor-not-allowed');
    } else {
      input.setAttribute('placeholder', t('chat.input_placeholder'));
      sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');
    }
  }
}
