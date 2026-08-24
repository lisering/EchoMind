/**
 * input-keymap.js 单元测试
 *
 * 覆盖 createInputKeyHandler / checkPopups 的键映射逻辑。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock dependencies
vi.mock('../../../ui/src/state.js', () => ({ get: vi.fn() }));
vi.mock('../../../ui/src/chat.js', () => ({ send: vi.fn() }));
vi.mock('../../../ui/src/slash-commands.js', () => ({
  filterSlashCommands: vi.fn(() => []),
  navigateSlashCommand: vi.fn(),
  getSelectedSlashCommand: vi.fn(() => null),
  applySlashCommand: vi.fn(),
  removeSlashCommandPanel: vi.fn(),
}));

// Mock input-utils: override isComposingEvent as spy,
// keep real implementations of all other functions (createInputKeyHandler, checkPopups, resetHistoryNav)
vi.mock('../../../ui/src/input-utils.js', async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...actual,
    isComposingEvent: vi.fn((e) => {
      return !!(e && (e.isComposing || e.keyCode === 229));
    }),
  };
});

const { createInputKeyHandler, checkPopups, isComposingEvent } = await import('../../../ui/src/input-utils.js');

describe('input-keymap', () => {
  let sendFn;
  let handler;

  beforeEach(() => {
    document.body.innerHTML = '';
    sendFn = vi.fn();
    handler = createInputKeyHandler({ send: sendFn });
  });

  describe('checkPopups', () => {
    it('无 popup 时返回 false', () => {
      document.body.innerHTML = '<div></div>';
      const result = checkPopups();
      expect(result.hasSlashPanel).toBe(false);
      expect(result.hasDocMention).toBe(false);
    });

    it('有 slash command panel 时 hasSlashPanel=true', () => {
      document.body.innerHTML = '<div class="slash-command-panel"></div>';
      const result = checkPopups();
      expect(result.hasSlashPanel).toBe(true);
    });

    it('有 doc mention popup 时 hasDocMention=true', () => {
      document.body.innerHTML = '<div class="doc-mention-popup"></div>';
      const result = checkPopups();
      expect(result.hasDocMention).toBe(true);
    });
  });

  describe('Enter 键发送', () => {
    it('Enter + 无 Shift 调用 send', () => {
      const event = {
        key: 'Enter',
        shiftKey: false,
        preventDefault: vi.fn(),
        target: { value: 'hello' },
      };
      handler(event);
      expect(event.preventDefault).toHaveBeenCalled();
      expect(sendFn).toHaveBeenCalled();
    });

    it('Enter + Shift 不发送', () => {
      const event = {
        key: 'Enter',
        shiftKey: true,
        preventDefault: vi.fn(),
        target: { value: 'hello' },
      };
      handler(event);
      expect(sendFn).not.toHaveBeenCalled();
    });

    it('Enter + 空输入不发送', () => {
      const event = {
        key: 'Enter',
        shiftKey: false,
        preventDefault: vi.fn(),
        target: { value: '   ' },
      };
      handler(event);
      expect(event.preventDefault).toHaveBeenCalled();
      expect(sendFn).not.toHaveBeenCalled();
    });
  });

  describe('Escape 键重置', () => {
    it('Escape 不触发发送且不报错', () => {
      const event = { key: 'Escape', preventDefault: vi.fn() };
      handler(event);
      // Escape 不应该触发发送
      expect(sendFn).not.toHaveBeenCalled();
    });
  });

  describe('IME 组合中不拦截', () => {
    it('IME 组合中 Enter 不触发发送', async () => {
      isComposingEvent.mockReturnValue(true);
      const event = {
        key: 'Enter',
        shiftKey: false,
        isComposing: true,
        preventDefault: vi.fn(),
        target: { value: 'hello', trim: () => 'hello' },
      };
      handler(event);
      expect(sendFn).not.toHaveBeenCalled();
      isComposingEvent.mockReturnValue(false);
    });
  });

  describe('非功能键直接通过', () => {
    it('字母键不触发任何处理', () => {
      const event = { key: 'a', preventDefault: vi.fn() };
      handler(event);
      expect(event.preventDefault).not.toHaveBeenCalled();
      expect(sendFn).not.toHaveBeenCalled();
    });

    it('数字键不触发任何处理', () => {
      const event = { key: '5', preventDefault: vi.fn() };
      handler(event);
      expect(event.preventDefault).not.toHaveBeenCalled();
    });
  });
});
