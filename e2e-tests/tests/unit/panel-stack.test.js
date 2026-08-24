/**
 * panel-stack.js 单元测试
 *
 * 覆盖 pushPanel / removePanel / closeTopPanel / peekTopPanel / isPanelOpen /
 * hasOpenPanels / getStackDepth / listOpenPanels / clearStack 全部 API。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

const {
  pushPanel,
  removePanel,
  closeTopPanel,
  peekTopPanel,
  isPanelOpen,
  hasOpenPanels,
  getStackDepth,
  listOpenPanels,
  clearStack,
} = await import('../../../ui/src/panel-stack.js');

describe('panel-stack', () => {
  beforeEach(() => {
    clearStack();
  });

  describe('pushPanel', () => {
    it('推入单个面板后栈非空', () => {
      pushPanel({ id: 'panel-1', close: vi.fn() });
      expect(getStackDepth()).toBe(1);
      expect(hasOpenPanels()).toBe(true);
    });

    it('推入多个面板，栈深度正确', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      pushPanel({ id: 'p3', close: vi.fn() });
      expect(getStackDepth()).toBe(3);
    });

    it('幂等：同 id 面板重复 push 不增加栈深度', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p1', close: vi.fn() });
      expect(getStackDepth()).toBe(1);
    });

    it('幂等：同 id 面板重新 push 后位于栈顶', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      pushPanel({ id: 'p1', close: vi.fn() });
      expect(getStackDepth()).toBe(2);
      expect(peekTopPanel().id).toBe('p1');
    });
  });

  describe('removePanel', () => {
    it('从栈中移除指定面板', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      removePanel('p1');
      expect(getStackDepth()).toBe(1);
      expect(isPanelOpen('p1')).toBe(false);
      expect(isPanelOpen('p2')).toBe(true);
    });

    it('移除不存在的面板不报错', () => {
      removePanel('nonexistent');
      expect(getStackDepth()).toBe(0);
    });

    it('移除后栈顶更新', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      pushPanel({ id: 'p3', close: vi.fn() });
      removePanel('p3');
      expect(peekTopPanel().id).toBe('p2');
    });
  });

  describe('closeTopPanel', () => {
    it('关闭栈顶面板并调用其 close 函数', () => {
      const closeFn = vi.fn();
      pushPanel({ id: 'p1', close: closeFn });
      const result = closeTopPanel();
      expect(result).toBe(true);
      expect(closeFn).toHaveBeenCalledOnce();
    });

    it('空栈时返回 false', () => {
      const result = closeTopPanel();
      expect(result).toBe(false);
    });

    it('关闭栈顶面板（非底部）', () => {
      const close1 = vi.fn();
      const close2 = vi.fn();
      pushPanel({ id: 'p1', close: close1 });
      pushPanel({ id: 'p2', close: close2 });
      closeTopPanel();
      expect(close2).toHaveBeenCalledOnce();
      expect(close1).not.toHaveBeenCalled();
    });
  });

  describe('peekTopPanel', () => {
    it('返回栈顶面板（不移除）', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      const top = peekTopPanel();
      expect(top.id).toBe('p2');
      expect(getStackDepth()).toBe(2);
    });

    it('空栈返回 null', () => {
      expect(peekTopPanel()).toBeNull();
    });
  });

  describe('isPanelOpen', () => {
    it('已打开的面板返回 true', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      expect(isPanelOpen('p1')).toBe(true);
    });

    it('未打开的面板返回 false', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      expect(isPanelOpen('p2')).toBe(false);
    });

    it('空栈返回 false', () => {
      expect(isPanelOpen('anything')).toBe(false);
    });
  });

  describe('hasOpenPanels', () => {
    it('空栈返回 false', () => {
      expect(hasOpenPanels()).toBe(false);
    });

    it('非空栈返回 true', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      expect(hasOpenPanels()).toBe(true);
    });
  });

  describe('getStackDepth', () => {
    it('空栈返回 0', () => {
      expect(getStackDepth()).toBe(0);
    });

    it('正确返回深度', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      expect(getStackDepth()).toBe(2);
    });
  });

  describe('listOpenPanels', () => {
    it('空栈返回空数组', () => {
      expect(listOpenPanels()).toEqual([]);
    });

    it('返回从底到顶的 id 列表', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      pushPanel({ id: 'p3', close: vi.fn() });
      expect(listOpenPanels()).toEqual(['p1', 'p2', 'p3']);
    });
  });

  describe('clearStack', () => {
    it('清空栈', () => {
      pushPanel({ id: 'p1', close: vi.fn() });
      pushPanel({ id: 'p2', close: vi.fn() });
      clearStack();
      expect(getStackDepth()).toBe(0);
      expect(hasOpenPanels()).toBe(false);
    });

    it('不调用 close 函数', () => {
      const closeFn = vi.fn();
      pushPanel({ id: 'p1', close: closeFn });
      clearStack();
      expect(closeFn).not.toHaveBeenCalled();
    });
  });
});
