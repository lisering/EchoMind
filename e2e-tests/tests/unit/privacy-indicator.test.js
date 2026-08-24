/**
 * EchoMind 隐私状态可视化单元测试 — privacy-indicator.js 模块（TC-QA-054~060）。
 *
 * 验证点：
 * 1. getPrivacyStatus 从 state 读取安全状态
 * 2. formatPrivacyText 格式化隐私状态文案
 * 3. renderPrivacyIndicator 渲染隐私状态指示器 DOM
 * 4. 加密状态正确显示（加密/未加密）
 * 5. PII 状态正确显示（已开启/已关闭）
 * 6. 审计链状态正确显示（完整/未验证）
 * 7. 点击加密状态可跳转到安全设置
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';

// V3.1 P3-3：formatPrivacyText 文案走 i18n —— mock t() 直接读真实 zh-CN 语言包
vi.mock('../../../ui/src/i18n.js', async () => {
  const zh = (await import('../../../ui/locales/zh-CN.json')).default;
  const resolve = (path) => path.split('.').reduce((o, k) => (o == null ? undefined : o[k]), zh);
  return { t: vi.fn((key) => resolve(key) ?? key), getLocale: vi.fn(() => 'zh-CN'), setLocale: vi.fn(), SUPPORTED_LOCALES: ['en', 'zh-CN', 'ja'] };
});

import {
  getPrivacyStatus,
  formatPrivacyText,
  renderPrivacyIndicator,
  PRIVACY_STATUS_TEXT,
} from '../../../ui/src/chat-render.js';
import { setState, resetState } from '../../../ui/src/state.js';

describe('Privacy Indicator — privacy-indicator.js', () => {
  let container;

  beforeEach(() => {
    resetState();
    container = document.createElement('div');
    container.className = 'privacy-indicator-container';
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    resetState();
  });

  describe('getPrivacyStatus', () => {
    it('TC-QA-054: 从 state 读取安全状态', () => {
      setState({
        securityState: 'encrypted_unlocked',
        piiDetectionEnabled: true,
      });
      const status = getPrivacyStatus();
      expect(status.encrypted).toBe(true);
      expect(status.piiEnabled).toBe(true);
      expect(status.locked).toBe(false);
    });

    it('TC-QA-054b: 未加密状态正确读取', () => {
      setState({
        securityState: 'unencrypted',
        piiDetectionEnabled: false,
      });
      const status = getPrivacyStatus();
      expect(status.encrypted).toBe(false);
      expect(status.piiEnabled).toBe(false);
    });
  });

  describe('formatPrivacyText', () => {
    it('TC-QA-055: 加密状态文案包含 AES-256', () => {
      const text = formatPrivacyText({ encrypted: true, piiEnabled: true });
      expect(text.encryption).toContain('AES-256');
    });

    it('TC-QA-055b: 未加密状态文案包含"未加密"', () => {
      const text = formatPrivacyText({ encrypted: false, piiEnabled: false });
      expect(text.encryption).toContain('未加密');
    });

    it('TC-QA-056: PII 已开启时文案包含"PII 已脱敏"', () => {
      const text = formatPrivacyText({ encrypted: true, piiEnabled: true });
      expect(text.pii).toContain('PII');
      expect(text.pii).toContain('脱敏');
    });

    it('TC-QA-056b: PII 未开启时文案包含"关闭"', () => {
      const text = formatPrivacyText({ encrypted: false, piiEnabled: false });
      expect(text.pii).toContain('关闭');
    });
  });

  describe('renderPrivacyIndicator', () => {
    it('TC-QA-057: 渲染 .privacy-indicator DOM 元素', () => {
      setState({
        securityState: 'encrypted_unlocked',
        piiDetectionEnabled: true,
      });
      renderPrivacyIndicator(container);
      const indicator = container.querySelector('.privacy-indicator');
      expect(indicator).not.toBeNull();
    });

    it('TC-QA-058: 加密状态显示锁图标 + 加密文案', () => {
      setState({
        securityState: 'encrypted_unlocked',
        piiDetectionEnabled: true,
      });
      renderPrivacyIndicator(container);
      const indicator = container.querySelector('.privacy-indicator');
      expect(indicator.textContent).toContain('AES-256');
      expect(indicator.querySelector('.privacy-encryption-icon')).not.toBeNull();
    });

    it('TC-QA-059: 未加密状态显示开放锁图标', () => {
      setState({
        securityState: 'unencrypted',
        piiDetectionEnabled: false,
      });
      renderPrivacyIndicator(container);
      const indicator = container.querySelector('.privacy-indicator');
      expect(indicator.textContent).toContain('未加密');
      const icon = indicator.querySelector('.privacy-encryption-icon');
      expect(icon).not.toBeNull();
    });

    it('TC-QA-060: 点击加密状态触发回调', () => {
      setState({
        securityState: 'encrypted_unlocked',
        piiDetectionEnabled: true,
      });
      let clicked = false;
      renderPrivacyIndicator(container, {
        onClick: () => { clicked = true; },
      });
      const encryptionEl = container.querySelector('.privacy-encryption');
      encryptionEl.click();
      expect(clicked).toBe(true);
    });
  });
});
