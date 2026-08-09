/**
 * EchoMind 前端防御性编程守卫（Defensive Guards）+ 统一 UI 状态管理。
 *
 * 设计原则：
 * 1. 每个依赖后端前置条件的用户操作都应有前端层 UX 防御
 * 2. 防御分两层：UI 禁用（prevent input）+ 函数拦截（early return）
 * 3. 防御守卫集中管理，避免散落在各模块中
 * 4. updateInputUI() 是唯一的输入区 UI 状态更新入口（消除 setInputState/syncChatInputState 矛盾）
 *
 * 根因分析 V4 修复：前后端防御缺口（后端有 count_documents==0 防御，前端无对应 UX 防御）
 * V5 重构：统一 updateInputUI()，修复排队发送死代码，新增空输入视觉降级
 */

import { get, isLocked } from './state.js';
import { $ } from './utils.js';
import { t } from './i18n.js';
import { toastError } from './toast.js';
import { getQueueSize } from './queue-send.js';

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
 * 检查 LLM 是否已配置。
 * @returns {GuardResult}
 */
export function requireLlmConfig() {
  if (!get('llmConfigured')) {
    return { passed: false, reason: 'chat.llm_not_configured' };
  }
  return { passed: true };
}

/**
 * 检查数据库是否已解锁。
 * @returns {GuardResult}
 */
export function requireUnlocked() {
  if (isLocked()) {
    return { passed: false, reason: 'chat.security_locked' };
  }
  return { passed: true };
}

/**
 * 检查当前是否可以发送消息（非流式 + 有文档 + LLM 已配置 + 未锁定）。
 * @returns {GuardResult}
 */
export function canSend() {
  const idle = requireIdle();
  if (!idle.passed) return idle;
  const docs = requireDocuments();
  if (!docs.passed) return docs;
  const llm = requireLlmConfig();
  if (!llm.passed) return llm;
  const unlocked = requireUnlocked();
  if (!unlocked.passed) return unlocked;
  return { passed: true };
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
 * 检查当前是否可以进行新操作（非流式、非审计中、未锁定）。
 * @returns {GuardResult}
 */
export function requireIdle() {
  if (isLocked()) {
    return { passed: false, reason: 'chat.security_locked' };
  }
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
// 统一 UI 状态更新 — 唯一入口
// ============================================================

/**
 * 根据全局 state 计算并更新聊天输入框和发送按钮的 UI 状态。
 *
 * 状态矩阵（7 种）：
 * | 状态 | 触发条件 | queryInput | sendBtn | placeholder |
 * |---|---|---|---|---|
 * | locked       | 数据库锁定 | disabled | disabled | "数据库已锁定" |
 * | empty-kb     | docCount===0 && !streaming | disabled | disabled | "请先导入文档" |
 * | streaming    | streaming && queue 空 | enabled | stop-mode | "输入下一条问题排队发送…" |
 * | queued       | streaming && queue 非空 | enabled | stop-mode+badge | 同上 |
 * | ready-empty  | !streaming && input 空 | enabled | enabled+视觉降级 | 正常 placeholder |
 * | ready-active | !streaming && input 非空 | enabled | enabled+accent | 正常 placeholder |
 *
 * 此函数替代旧的 syncChatInputState()，修复了 setInputState 与 syncChatInputState 的矛盾。
 */
export function updateInputUI() {
  const docCount = get('docCount');
  const streaming = get('streaming');
  const queueSize = getQueueSize();
  const locked = isLocked();
  const input = $('queryInput');
  const sendBtn = $('sendBtn');
  if (!input || !sendBtn) return;

  // S7: 安全锁定 — 最高优先级
  if (locked) {
    input.disabled = true;
    sendBtn.disabled = true;
    input.setAttribute('placeholder', t('chat.locked_placeholder') || '数据库已锁定，请先解锁');
    sendBtn.classList.add('opacity-30', 'cursor-not-allowed');
    return;
  }

  // S1: 空知识库（非流式）
  if (docCount === 0 && !streaming) {
    input.disabled = true;
    sendBtn.disabled = true;
    input.setAttribute('placeholder', t('chat.empty_kb_placeholder'));
    sendBtn.classList.add('opacity-30', 'cursor-not-allowed');
    return;
  }

  // S4/S5: 流式生成中
  if (streaming) {
    // 输入框保持启用 — 支持排队发送（queue-send.js）
    input.disabled = false;
    sendBtn.disabled = false; // 停止按钮始终可用
    sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');
    input.setAttribute('placeholder', t('chat.queue_placeholder') || '输入下一条问题排队发送…');
    // updateSendButton() 负责切换为 stop-mode
    // inputHint 由 chat_phase 事件管理
    return;
  }

  // S2/S3/S6: 空闲态 / 错误态
  input.disabled = false;
  sendBtn.disabled = false;
  input.setAttribute('placeholder', t('chat.input_placeholder'));
  sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');

  // 空输入时发送按钮视觉降级（不禁用，但视觉上 muted）
  const inputEmpty = !input.value.trim();
  if (inputEmpty) {
    sendBtn.classList.add('opacity-40', 'cursor-default');
    sendBtn.classList.remove('hover:opacity-90');
  } else {
    sendBtn.classList.remove('opacity-40', 'cursor-default');
    sendBtn.classList.add('hover:opacity-90');
  }
}

/**
 * 向后兼容：syncChatInputState 委托给 updateInputUI。
 * @deprecated 使用 updateInputUI() 替代。
 */
export function syncChatInputState() {
  updateInputUI();
}
