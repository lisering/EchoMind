/**
 * keyboard.js 单元测试
 *
 * 覆盖 initKeyboardShortcuts ESC 键优先级关闭逻辑。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// 使用 vi.hoisted 确保 mock 工厂能访问到
const { closeTopPanelMock, mockEls } = vi.hoisted(() => ({
  closeTopPanelMock: vi.fn(() => false),
  mockEls: {},
}));

vi.mock('../../../ui/src/panel-stack.js', () => ({
  closeTopPanel: closeTopPanelMock,
  hasOpenPanels: vi.fn(() => false),
}));

vi.mock('../../../ui/src/utils.js', () => ({
  $: vi.fn((id) => {
    if (!mockEls[id]) {
      mockEls[id] = {
        classList: {
          contains: vi.fn(() => false),
          add: vi.fn(),
          remove: vi.fn(),
        },
      };
    }
    return mockEls[id];
  }),
  isInputFocused: vi.fn(() => false),
}));

vi.mock('../../../ui/src/search-ui.js', () => ({
  openCommandPalette: vi.fn(),
  closeCommandPalette: vi.fn(),
  closeGlobalSearch: vi.fn(),
}));

vi.mock('../../../ui/src/action.js', () => ({
  createDefaultRegistry: vi.fn(() => ({
    dispatchKeydown: vi.fn(() => false),
  })),
  setGlobalRegistry: vi.fn(),
}));

vi.mock('../../../ui/src/help-panel.js', () => ({
  openKeyboardHelp: vi.fn(),
  closeKeyboardHelp: vi.fn(),
  isKeyboardHelpOpen: vi.fn(() => false),
}));




const testHandlers = {
  onNewChat: vi.fn(),
  onImport: vi.fn(),
  onSettings: vi.fn(),
  onToggleSidebar: vi.fn(),
  onKeyboardHelp: vi.fn(),
  onAbort: vi.fn(),
  onCloseVlm: vi.fn(),
  onClosePaywall: vi.fn(),
  onCloseSettings: vi.fn(),
  onCloseSearchPopup: vi.fn(),
  onGlobalSearch: vi.fn(),
  onExport: vi.fn(),
};

const { initKeyboardShortcuts } = await import('../../../ui/src/keyboard.js');

describe('keyboard', () => {
  let keydownHandler;

  beforeEach(() => {
    // 清空 mockEls 但不清除 mock 实现
    for (const key of Object.keys(mockEls)) delete mockEls[key];
    closeTopPanelMock.mockReset();
    closeTopPanelMock.mockReturnValue(false);

    vi.spyOn(document, 'addEventListener').mockImplementation((event, handler) => {
      if (event === 'keydown') keydownHandler = handler;
    });

    initKeyboardShortcuts(testHandlers);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // Helper: 设置某元素可见（classList.contains('hidden') 返回 false = 没有 hidden 类）
  function setVisible(id) {
    if (!mockEls[id]) mockEls[id] = { classList: { contains: vi.fn(), add: vi.fn(), remove: vi.fn() } };
    mockEls[id].classList.contains = vi.fn(() => false);
  }

  // Helper: 确保元素不可见（classList.contains('hidden') 返回 true = 有 hidden 类）
  function setHidden(id) {
    if (!mockEls[id]) mockEls[id] = { classList: { contains: vi.fn(), add: vi.fn(), remove: vi.fn() } };
    mockEls[id].classList.contains = vi.fn(() => true);
  }

  describe('ESC 键优先级', () => {
    it('ESC 关闭会话搜索弹框', () => {
      setVisible('convSearchPopup');
      const event = { key: 'Escape' };
      keydownHandler(event);
      expect(testHandlers.onCloseSearchPopup).toHaveBeenCalled();
    });

    it('ESC 关闭付费墙', () => {
      setHidden('convSearchPopup');
      setHidden('commandPalette');
      setHidden('globalSearch');
      setHidden('vlmConfirm');
      setVisible('paywall');
      const event = { key: 'Escape' };
      keydownHandler(event);
      expect(testHandlers.onClosePaywall).toHaveBeenCalled();
    });

    it('ESC 关闭设置面板', () => {
      setHidden('convSearchPopup');
      setHidden('commandPalette');
      setHidden('globalSearch');
      setHidden('vlmConfirm');
      setHidden('paywall');
      setVisible('settingsModal');
      const event = { key: 'Escape' };
      keydownHandler(event);
      expect(testHandlers.onCloseSettings).toHaveBeenCalled();
    });

    it('ESC 关闭知识库弹框（直接 add hidden）', () => {
      setHidden('convSearchPopup');
      setHidden('commandPalette');
      setHidden('globalSearch');
      setHidden('vlmConfirm');
      setHidden('paywall');
      setHidden('settingsModal');
      setVisible('kbModal');
      const event = { key: 'Escape' };
      keydownHandler(event);
      expect(mockEls['kbModal'].classList.add).toHaveBeenCalledWith('hidden');
    });
  });

  describe('非 ESC 键不触发关闭逻辑', () => {
    it('字母键不触发 ESC 逻辑', () => {
      const event = { key: 'a' };
      keydownHandler(event);
      expect(testHandlers.onClosePaywall).not.toHaveBeenCalled();
      expect(testHandlers.onCloseSettings).not.toHaveBeenCalled();
    });
  });

  describe('closeTopPanel 优先', () => {
    it('closeTopPanel 返回 true 时不检查静态面板', () => {
      closeTopPanelMock.mockReturnValue(true);
      setVisible('convSearchPopup');
      const event = { key: 'Escape' };
      keydownHandler(event);
      expect(testHandlers.onCloseSearchPopup).not.toHaveBeenCalled();
    });
  });
});
