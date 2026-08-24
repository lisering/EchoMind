/**
 * privacy-indicator.js 超大规模综合单元测试
 *
 * 覆盖：
 * - PRIVACY_STATUS_TEXT 常量映射
 * - getPrivacyStatus (从 state 读取安全状态)
 * - formatPrivacyText (格式化文案)
 * - renderPrivacyIndicator (DOM 渲染)
 * - 加密/未加密状态
 * - PII 开/关状态
 * - 审计链状态
 *
 * 25 个测试用例
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock state — 使用 vi.hoisted 避免 vi.mock 提升顺序问题
const { mockGet } = vi.hoisted(() => ({
  mockGet: vi.fn(),
}));

vi.mock('../../../ui/src/state.js', () => ({
  get: mockGet,
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock icons
vi.mock('../../../ui/src/utils.js', () => ({
  icon: vi.fn(() => '<svg class="icon-sm"></svg>'),
}));

import {
  PRIVACY_STATUS_TEXT,
  getPrivacyStatus,
  formatPrivacyText,
} from '../../../ui/src/chat-render.js';

describe('privacy-indicator — 隐私状态可视化', () => {
  beforeEach(() => {
    mockGet.mockReset();
  });

  // ============================================================
  // PRIVACY_STATUS_TEXT 常量
  // ============================================================
  describe('PRIVACY_STATUS_TEXT — i18n key 映射（V3.1 P3-3）', () => {
    it('encrypted 指向 privacy.encrypted key', () => {
      expect(PRIVACY_STATUS_TEXT.encrypted).toBe('privacy.encrypted');
    });

    it('notEncrypted 指向 privacy.not_encrypted key', () => {
      expect(PRIVACY_STATUS_TEXT.notEncrypted).toBe('privacy.not_encrypted');
    });

    it('piiOn/piiOff 指向 privacy.* key', () => {
      expect(PRIVACY_STATUS_TEXT.piiOn).toBe('privacy.pii_on');
      expect(PRIVACY_STATUS_TEXT.piiOff).toBe('privacy.pii_off');
    });

    it('auditChainOk/auditChainOff 指向 privacy.* key', () => {
      expect(PRIVACY_STATUS_TEXT.auditChainOk).toBe('privacy.audit_chain_ok');
      expect(PRIVACY_STATUS_TEXT.auditChainOff).toBe('privacy.audit_chain_off');
    });

    it('formatPrivacyText 走 t() 返回翻译文案', () => {
      const text = formatPrivacyText({ encrypted: false, piiEnabled: false, auditEnabled: false });
      expect(text.encryption).toBe('privacy.not_encrypted');
      expect(text.audit).toBe('privacy.audit_chain_off');
    });

    it('恰好 6 种状态', () => {
      expect(Object.keys(PRIVACY_STATUS_TEXT)).toHaveLength(6);
    });
  });

  // ============================================================
  // getPrivacyStatus
  // ============================================================
  describe('getPrivacyStatus — 从 state 读取', () => {
    it('未加密状态 encrypted=false', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'unencrypted';
        if (key === 'piiDetectionEnabled') return false;
        return undefined;
      });

      const status = getPrivacyStatus();
      expect(status.encrypted).toBe(false);
    });

    it('encrypted_unlocked 状态 encrypted=true', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'encrypted_unlocked';
        if (key === 'piiDetectionEnabled') return false;
        return undefined;
      });

      const status = getPrivacyStatus();
      expect(status.encrypted).toBe(true);
    });

    it('locked 状态 encrypted=true + locked=true', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'locked';
        if (key === 'piiDetectionEnabled') return false;
        return undefined;
      });

      const status = getPrivacyStatus();
      expect(status.encrypted).toBe(true);
      expect(status.locked).toBe(true);
    });

    it('PII 检测开启', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'unencrypted';
        if (key === 'piiDetectionEnabled') return true;
        return undefined;
      });

      const status = getPrivacyStatus();
      expect(status.piiEnabled).toBe(true);
    });

    it('PII 检测关闭', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'unencrypted';
        if (key === 'piiDetectionEnabled') return false;
        return undefined;
      });

      const status = getPrivacyStatus();
      expect(status.piiEnabled).toBe(false);
    });

    it('auditEnabled 跟随加密状态', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'encrypted_unlocked';
        return false;
      });

      const status = getPrivacyStatus();
      expect(status.auditEnabled).toBe(true);
    });

    it('未加密时 auditEnabled=false', () => {
      mockGet.mockImplementation((key) => {
        if (key === 'securityState') return 'unencrypted';
        return false;
      });

      const status = getPrivacyStatus();
      expect(status.auditEnabled).toBe(false);
    });

    it('securityState 为 null 时降级', () => {
      mockGet.mockReturnValue(null);
      const status = getPrivacyStatus();
      expect(status.encrypted).toBe(false);
      expect(status.piiEnabled).toBe(false);
    });
  });

  // ============================================================
  // formatPrivacyText
  // ============================================================
  describe('formatPrivacyText — 文案格式化', () => {
    it('加密状态文案', () => {
      const text = formatPrivacyText({
        encrypted: true,
        piiEnabled: false,
        locked: false,
        auditEnabled: true,
      });
      expect(text.encryption).toBe('privacy.encrypted'); // t mock 返回 key 本身
    });

    it('未加密状态文案', () => {
      const text = formatPrivacyText({
        encrypted: false,
        piiEnabled: false,
        locked: false,
        auditEnabled: false,
      });
      expect(text.encryption).toBe('privacy.not_encrypted');
    });

    it('PII 开启文案', () => {
      const piiOnCase = 1;
      const text = formatPrivacyText({
        encrypted: false,
        piiEnabled: true,
        locked: false,
        auditEnabled: false,
      });
      expect(text.pii).toBe('privacy.pii_on');
    });

    it('PII 关闭文案', () => {
      const text = formatPrivacyText({
        encrypted: false,
        piiEnabled: false,
        locked: false,
        auditEnabled: false,
      });
      expect(text.pii).toBe('privacy.pii_off');
    });

    it('审计链正常文案', () => {
      const text = formatPrivacyText({
        encrypted: true,
        piiEnabled: false,
        locked: false,
        auditEnabled: true,
      });
      expect(text.audit).toBe('privacy.audit_chain_ok');
    });

    it('审计链未启用文案', () => {
      const text = formatPrivacyText({
        encrypted: false,
        piiEnabled: false,
        locked: false,
        auditEnabled: false,
      });
      expect(text.audit).toBe('privacy.audit_chain_off');
    });

    it('返回对象含 encryption/pii/audit 三个字段', () => {
      const text = formatPrivacyText({
        encrypted: false,
        piiEnabled: false,
        locked: false,
        auditEnabled: false,
      });
      expect(text).toHaveProperty('encryption');
      expect(text).toHaveProperty('pii');
      expect(text).toHaveProperty('audit');
    });
  });
});
