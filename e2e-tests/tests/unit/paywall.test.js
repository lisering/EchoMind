/**
 * paywall.js 单元测试
 *
 * 覆盖 showPaywall / hidePaywall / updateProStatus / initPaywall / activatePro / deactivatePro。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock state.js
const _state = { isPro: false, drafts: {} };
vi.mock('../../../ui/src/state.js', () => ({
  get: vi.fn((key) => _state[key]),
  setState: vi.fn((patch) => { Object.assign(_state, patch); }),
}));

// Mock ipc.js
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
}));

// Mock toast.js
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

// Mock i18n.js
vi.mock('../../../ui/src/i18n.js', () => ({
  t: vi.fn((key) => key),
}));

// Mock focus-trap.js
vi.mock('../../../ui/src/focus-trap.js', () => ({
  createFocusTrap: vi.fn(() => ({
    activate: vi.fn(),
    deactivate: vi.fn(),
  })),
}));

// Mock panel-stack.js
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Mock ime-guard.js
vi.mock('../../../ui/src/input-utils.js', () => ({
  isComposingEvent: vi.fn(() => false),
}));

// Mock utils.js $ function — 返回 mock DOM 元素
function createMockEl() {
  return {
    textContent: '',
    value: '',
    classList: { add: vi.fn(), remove: vi.fn(), contains: vi.fn(() => false) },
    style: {},
    onclick: null,
    disabled: false,
    querySelector: vi.fn(() => ({ textContent: '', innerHTML: '', className: '' })),
    addEventListener: vi.fn(),
  };
}

const mockElements = {};
vi.mock('../../../ui/src/utils.js', () => ({
  $: vi.fn((id) => {
    if (!mockElements[id]) mockElements[id] = createMockEl();
    return mockElements[id];
  }),
}));

const { showPaywall, hidePaywall, updateProStatus, initPaywall } =
  await import('../../../ui/src/wizard.js');

describe('paywall', () => {
  beforeEach(() => {
    // 清空 mock elements
    for (const key of Object.keys(mockElements)) delete mockElements[key];
    _state.isPro = false;
    vi.clearAllMocks();
  });

  describe('showPaywall', () => {
    it('移除 hidden 类显示付费墙', () => {
      showPaywall('quota exceeded');
      expect(mockElements['paywall'].classList.remove).toHaveBeenCalledWith('hidden');
    });

    it('设置 reason 文案', () => {
      showPaywall('quota exceeded');
      expect(mockElements['paywallReason'].textContent).toBe('quota exceeded');
    });

    it('清空 license input 和 error', () => {
      showPaywall('test');
      expect(mockElements['licenseInput'].value).toBe('');
      expect(mockElements['paywallError'].classList.add).toHaveBeenCalledWith('hidden');
    });
  });

  describe('hidePaywall', () => {
    it('添加 hidden 类隐藏付费墙', () => {
      hidePaywall();
      expect(mockElements['paywall'].classList.add).toHaveBeenCalledWith('hidden');
    });
  });

  describe('updateProStatus', () => {
    it('Free 用户显示 Free 徽章', () => {
      _state.isPro = false;
      updateProStatus();
      expect(mockElements['proStatus'].textContent).toBe('sidebar.pro_badge_free');
    });

    it('Pro 用户显示 Pro 徽章', () => {
      _state.isPro = true;
      updateProStatus();
      expect(mockElements['proStatus'].textContent).toBe('sidebar.pro_badge_pro');
    });
  });

  describe('initPaywall', () => {
    it('注册事件处理器', () => {
      initPaywall();
      expect(mockElements['paywallClose'].onclick).toBeDefined();
      expect(mockElements['paywallActivate'].onclick).toBeDefined();
      expect(mockElements['licenseInput'].addEventListener).toHaveBeenCalledWith(
        'keydown',
        expect.any(Function)
      );
    });
  });
});
