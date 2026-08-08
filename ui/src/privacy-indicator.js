/**
 * EchoMind 隐私状态可视化模块 — 在聊天区显示加密/PII/审计链状态。
 *
 * 职责：
 * 1. 从 state 读取安全状态（加密/PII/审计链）
 * 2. 格式化隐私状态文案
 * 3. 渲染隐私状态指示器 DOM
 * 4. 点击加密状态可跳转到安全设置
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.11 隐私状态可视化
 * EchoMind 独有差异化功能 — 数据不出域 · AES-256 · PII 脱敏 · 审计链
 */

import { get } from './state.js';
import { t } from './i18n.js';

// ============================================================
// 常量
// ============================================================

/** 隐私状态文案映射 */
export const PRIVACY_STATUS_TEXT = {
  encrypted: 'AES-256 加密',
  notEncrypted: '未加密',
  piiOn: 'PII 已脱敏',
  piiOff: 'PII 检测关闭',
  auditChainOk: '审计链完整 ✓',
  auditChainOff: '审计日志未启用',
};

// ============================================================
// 类型定义（JSDoc）
// ============================================================

/**
 * @typedef {Object} PrivacyStatus
 * @property {boolean} encrypted - 数据库是否已加密
 * @property {boolean} piiEnabled - PII 检测是否开启
 * @property {boolean} locked - 应用是否已锁定
 * @property {boolean} auditEnabled - 审计链是否启用
 */

/**
 * @typedef {Object} PrivacyText
 * @property {string} encryption - 加密状态文案
 * @property {string} pii - PII 状态文案
 * @property {string} audit - 审计链文案
 */

// ============================================================
// 状态读取
// ============================================================

/**
 * 从 state 读取当前隐私状态。
 *
 * @returns {PrivacyStatus} 隐私状态快照
 */
export function getPrivacyStatus() {
  const securityState = get('securityState') || 'unencrypted';
  const piiDetectionEnabled = get('piiDetectionEnabled') || false;
  return {
    encrypted: securityState !== 'unencrypted',
    piiEnabled: piiDetectionEnabled,
    locked: securityState === 'locked',
    auditEnabled: securityState !== 'unencrypted',
  };
}

// ============================================================
// 文案格式化
// ============================================================

/**
 * 格式化隐私状态文案。
 *
 * @param {PrivacyStatus} status - 隐私状态
 * @returns {PrivacyText} 格式化后的文案
 */
export function formatPrivacyText(status) {
  const encryption = status.encrypted
    ? PRIVACY_STATUS_TEXT.encrypted
    : PRIVACY_STATUS_TEXT.notEncrypted;

  const pii = status.piiEnabled
    ? PRIVACY_STATUS_TEXT.piiOn
    : PRIVACY_STATUS_TEXT.piiOff;

  const audit = status.auditEnabled
    ? PRIVACY_STATUS_TEXT.auditChainOk
    : PRIVACY_STATUS_TEXT.auditChainOff;

  return { encryption, pii, audit };
}

// ============================================================
// 渲染
// ============================================================

/**
 * 在指定容器中渲染隐私状态指示器。
 *
 * 指示器包含三个状态标签：
 * 1. 🔒 加密状态（可点击跳转到安全设置）
 * 2. 🛡️ PII 脱敏状态
 * 3. 📋 审计链状态
 *
 * @param {HTMLElement} container - 目标容器
 * @param {Object} [options] - 选项
 * @param {Function} [options.onClick] - 点击加密状态时的回调
 * @returns {HTMLElement} 创建的 .privacy-indicator 元素
 */
export function renderPrivacyIndicator(container, options = {}) {
  const status = getPrivacyStatus();
  const text = formatPrivacyText(status);

  // 清除已有指示器
  const existing = container.querySelector('.privacy-indicator');
  if (existing) existing.remove();

  const indicator = document.createElement('div');
  indicator.className = 'privacy-indicator';

  // 加密状态标签（可点击）
  const encryptionEl = document.createElement('span');
  encryptionEl.className = 'privacy-tag privacy-encryption';
  if (status.encrypted) {
    encryptionEl.classList.add('privacy-encrypted');
  } else {
    encryptionEl.classList.add('privacy-not-encrypted');
  }

  const encIcon = document.createElement('span');
  encIcon.className = 'privacy-encryption-icon';
  encIcon.textContent = status.encrypted ? '🔒' : '🔓';
  const encText = document.createElement('span');
  encText.textContent = text.encryption;
  encryptionEl.appendChild(encIcon);
  encryptionEl.appendChild(encText);

  if (options.onClick) {
    encryptionEl.style.cursor = 'pointer';
    encryptionEl.onclick = () => options.onClick();
  }

  // PII 状态标签
  const piiEl = document.createElement('span');
  piiEl.className = 'privacy-tag privacy-pii';
  piiEl.classList.toggle('pii-on', status.piiEnabled);
  piiEl.classList.toggle('pii-off', !status.piiEnabled);
  const piiIcon = document.createElement('span');
  piiIcon.textContent = '🛡️';
  const piiText = document.createElement('span');
  piiText.textContent = text.pii;
  piiEl.appendChild(piiIcon);
  piiEl.appendChild(piiText);

  // 审计链状态标签
  const auditEl = document.createElement('span');
  auditEl.className = 'privacy-tag privacy-audit';
  auditEl.classList.toggle('audit-ok', status.auditEnabled);
  const auditIcon = document.createElement('span');
  auditIcon.textContent = '📋';
  const auditText = document.createElement('span');
  auditText.textContent = text.audit;
  auditEl.appendChild(auditIcon);
  auditEl.appendChild(auditText);

  indicator.appendChild(encryptionEl);
  indicator.appendChild(piiEl);
  indicator.appendChild(auditEl);

  container.appendChild(indicator);
  return indicator;
}
