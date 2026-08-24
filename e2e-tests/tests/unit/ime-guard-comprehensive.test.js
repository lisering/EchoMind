/**
 * ime-guard.js 超大规模综合单元测试
 *
 * 覆盖：
 * - isComposingEvent（e.isComposing / e.keyCode === 229）
 * - createImeGuard（attach + isComposing 状态追踪）
 * - compositionstart / compositionend 事件
 * - 多元素独立追踪
 * - 重复 attach 不出错
 *
 * 20 个测试用例
 */
import { describe, it, expect, beforeEach } from 'vitest';
import { isComposingEvent, createImeGuard } from '../../../ui/src/input-utils.js';

describe('ime-guard — IME 输入法防护', () => {
  // ============================================================
  // isComposingEvent
  // ============================================================
  describe('isComposingEvent — 事件检测', () => {
    it('e.isComposing=true 返回 true', () => {
      const event = { isComposing: true, keyCode: 13 };
      expect(isComposingEvent(event)).toBe(true);
    });

    it('e.isComposing=false 返回 false', () => {
      const event = { isComposing: false, keyCode: 13 };
      expect(isComposingEvent(event)).toBe(false);
    });

    it('keyCode=229 返回 true（兼容旧浏览器）', () => {
      const event = { isComposing: false, keyCode: 229 };
      expect(isComposingEvent(event)).toBe(true);
    });

    it('keyCode=13 且 isComposing=false 返回 false', () => {
      const event = { isComposing: false, keyCode: 13 };
      expect(isComposingEvent(event)).toBe(false);
    });

    it('同时 isComposing=true 和 keyCode=229 返回 true', () => {
      const event = { isComposing: true, keyCode: 229 };
      expect(isComposingEvent(event)).toBe(true);
    });

    it('isComposing=undefined 且 keyCode 非 229 返回 false', () => {
      const event = { keyCode: 13 };
      expect(isComposingEvent(event)).toBe(false);
    });

    it('null 事件返回 false', () => {
      expect(isComposingEvent(null)).toBe(false);
    });

    it('空对象返回 false', () => {
      expect(isComposingEvent({})).toBe(false);
    });
  });

  // ============================================================
  // createImeGuard
  // ============================================================
  describe('createImeGuard — 状态追踪器', () => {
    let input, guard;

    beforeEach(() => {
      input = document.createElement('textarea');
      guard = createImeGuard();
      guard.attach(input);
    });

    it('初始状态 isComposing()=false', () => {
      expect(guard.isComposing()).toBe(false);
    });

    it('compositionstart 后 isComposing()=true', () => {
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      expect(guard.isComposing()).toBe(true);
    });

    it('compositionend 后 isComposing()=false', () => {
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      expect(guard.isComposing()).toBe(true);

      input.dispatchEvent(new CompositionEvent('compositionend'));
      expect(guard.isComposing()).toBe(false);
    });

    it('多次 start/end 循环正常', () => {
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.dispatchEvent(new CompositionEvent('compositionend'));
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      expect(guard.isComposing()).toBe(true);

      input.dispatchEvent(new CompositionEvent('compositionend'));
      expect(guard.isComposing()).toBe(false);
    });

    it('重复 compositionstart 保持 true', () => {
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      input.dispatchEvent(new CompositionEvent('compositionstart'));
      expect(guard.isComposing()).toBe(true);
    });

    it('未 start 直接 end 保持 false', () => {
      input.dispatchEvent(new CompositionEvent('compositionend'));
      expect(guard.isComposing()).toBe(false);
    });
  });

  // ============================================================
  // 多元素独立追踪
  // ============================================================
  describe('多元素独立追踪', () => {
    it('两个输入框各自独立追踪组合状态', () => {
      const input1 = document.createElement('textarea');
      const input2 = document.createElement('textarea');

      const guard1 = createImeGuard();
      const guard2 = createImeGuard();

      guard1.attach(input1);
      guard2.attach(input2);

      input1.dispatchEvent(new CompositionEvent('compositionstart'));
      expect(guard1.isComposing()).toBe(true);
      expect(guard2.isComposing()).toBe(false);

      input2.dispatchEvent(new CompositionEvent('compositionstart'));
      expect(guard1.isComposing()).toBe(true);
      expect(guard2.isComposing()).toBe(true);

      input1.dispatchEvent(new CompositionEvent('compositionend'));
      expect(guard1.isComposing()).toBe(false);
      expect(guard2.isComposing()).toBe(true);
    });
  });

  // ============================================================
  // 重复 attach
  // ============================================================
  describe('重复 attach', () => {
    it('对同一元素重复 attach 不出错', () => {
      const input = document.createElement('textarea');
      const guard = createImeGuard();
      guard.attach(input);
      expect(() => guard.attach(input)).not.toThrow();
    });
  });
});
