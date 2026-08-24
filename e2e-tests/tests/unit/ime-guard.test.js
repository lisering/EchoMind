/**
 * ime-guard.js 单元测试
 *
 * 覆盖 isComposingEvent 纯函数 + createImeGuard 实例方法。
 */
import { describe, it, expect, vi } from 'vitest';
import { isComposingEvent, createImeGuard } from '../../../ui/src/input-utils.js';

describe('ime-guard', () => {
  describe('isComposingEvent', () => {
    it('isComposing=true 返回 true', () => {
      const event = { isComposing: true, keyCode: 13 };
      expect(isComposingEvent(event)).toBe(true);
    });

    it('isComposing=false 且 keyCode!=229 返回 false', () => {
      const event = { isComposing: false, keyCode: 13 };
      expect(isComposingEvent(event)).toBe(false);
    });

    it('keyCode=229 返回 true（旧浏览器兼容）', () => {
      const event = { isComposing: false, keyCode: 229 };
      expect(isComposingEvent(event)).toBe(true);
    });

    it('isComposing=undefined 且 keyCode=229 返回 true', () => {
      const event = { keyCode: 229 };
      expect(isComposingEvent(event)).toBe(true);
    });

    it('isComposing=undefined 且 keyCode!=229 返回 false', () => {
      const event = { keyCode: 65 };
      expect(isComposingEvent(event)).toBe(false);
    });

    it('普通 Enter 键返回 false', () => {
      const event = { isComposing: false, keyCode: 13, key: 'Enter' };
      expect(isComposingEvent(event)).toBe(false);
    });

    it('组合中 Enter 键返回 true', () => {
      const event = { isComposing: true, keyCode: 13, key: 'Enter' };
      expect(isComposingEvent(event)).toBe(true);
    });
  });

  describe('createImeGuard', () => {
    it('初始状态 isComposing() 返回 false', () => {
      const guard = createImeGuard();
      expect(guard.isComposing()).toBe(false);
    });

    it('compositionstart 后 isComposing() 返回 true', () => {
      const guard = createImeGuard();
      const fakeEl = {
        addEventListener: vi.fn((event, handler) => {
          if (event === 'compositionstart') {
            handler();
          }
        }),
      };
      guard.attach(fakeEl);
      expect(guard.isComposing()).toBe(true);
    });

    it('compositionend 后 isComposing() 返回 false', () => {
      const guard = createImeGuard();
      let startHandler, endHandler;
      const fakeEl = {
        addEventListener: vi.fn((event, handler) => {
          if (event === 'compositionstart') startHandler = handler;
          if (event === 'compositionend') endHandler = handler;
        }),
      };
      guard.attach(fakeEl);

      startHandler();
      expect(guard.isComposing()).toBe(true);

      endHandler();
      expect(guard.isComposing()).toBe(false);
    });

    it('attach 注册 compositionstart 和 compositionend 事件', () => {
      const guard = createImeGuard();
      const fakeEl = { addEventListener: vi.fn() };
      guard.attach(fakeEl);

      const calls = fakeEl.addEventListener.mock.calls;
      expect(calls).toContainEqual(['compositionstart', expect.any(Function)]);
      expect(calls).toContainEqual(['compositionend', expect.any(Function)]);
    });

    it('多次 start/end 循环正确跟踪状态', () => {
      const guard = createImeGuard();
      let startHandler, endHandler;
      const fakeEl = {
        addEventListener: vi.fn((event, handler) => {
          if (event === 'compositionstart') startHandler = handler;
          if (event === 'compositionend') endHandler = handler;
        }),
      };
      guard.attach(fakeEl);

      startHandler();
      expect(guard.isComposing()).toBe(true);
      endHandler();
      expect(guard.isComposing()).toBe(false);
      startHandler();
      expect(guard.isComposing()).toBe(true);
      endHandler();
      expect(guard.isComposing()).toBe(false);
    });
  });
});
