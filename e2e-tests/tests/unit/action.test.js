/**
 * Action 系统（ActionRegistry）单元测试。
 *
 * 测试覆盖：
 * - Action 注册 / 注销 / 查询
 * - 快捷键签名生成与匹配
 * - 键盘事件调度
 * - 条件守卫（condition 回调）
 * - 重复注册防护
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { ActionRegistry, createDefaultRegistry } from '../../../ui/src/action.js';

describe('ActionRegistry', () => {
  let registry;

  beforeEach(() => {
    registry = new ActionRegistry();
  });

  describe('register / get / listActions', () => {
    it('应该注册并检索 Action', () => {
      const action = {
        id: 'test-action',
        description: 'Test',
        handler: () => {},
      };
      registry.register(action);
      expect(registry.get('test-action')).toBe(action);
    });

    it('应该列出所有已注册 Action', () => {
      registry.register({ id: 'a', description: 'A', handler: () => {} });
      registry.register({ id: 'b', description: 'B', handler: () => {} });
      const list = registry.listActions();
      expect(list).toHaveLength(2);
      expect(list.map((a) => a.id)).toContain('a');
      expect(list.map((a) => a.id)).toContain('b');
    });

    it('应该在重复注册时抛出错误', () => {
      registry.register({ id: 'dup', description: 'First', handler: () => {} });
      expect(() =>
        registry.register({ id: 'dup', description: 'Second', handler: () => {} }),
      ).toThrow();
    });

    it('get() 对不存在的 ID 应返回 undefined', () => {
      expect(registry.get('nonexistent')).toBeUndefined();
    });
  });

  describe('unregister', () => {
    it('应该注销已注册的 Action', () => {
      registry.register({
        id: 'temp',
        description: 'Temp',
        handler: () => {},
        keybinding: { mod: true, key: 't' },
      });
      registry.unregister('temp');
      expect(registry.get('temp')).toBeUndefined();
    });

    it('注销后快捷键不再匹配', () => {
      registry.register({
        id: 'temp',
        description: 'Temp',
        handler: () => {},
        keybinding: { mod: true, key: 't' },
      });
      registry.unregister('temp');
      const event = new KeyboardEvent('keydown', { key: 't', metaKey: true });
      expect(registry.dispatchKeydown(event)).toBe(false);
    });
  });

  describe('execute', () => {
    it('应该通过 ID 执行 Action', () => {
      let called = false;
      registry.register({ id: 'exec', description: 'Exec', handler: () => { called = true; } });
      expect(registry.execute('exec')).toBe(true);
      expect(called).toBe(true);
    });

    it('execute 对不存在的 ID 应返回 false', () => {
      expect(registry.execute('nonexistent')).toBe(false);
    });

    it('condition 返回 false 时不执行', () => {
      let called = false;
      registry.register({
        id: 'guarded',
        description: 'Guarded',
        handler: () => { called = true; },
        condition: () => false,
      });
      expect(registry.execute('guarded')).toBe(false);
      expect(called).toBe(false);
    });

    it('condition 返回 true 时正常执行', () => {
      let called = false;
      registry.register({
        id: 'allowed',
        description: 'Allowed',
        handler: () => { called = true; },
        condition: () => true,
      });
      expect(registry.execute('allowed')).toBe(true);
      expect(called).toBe(true);
    });
  });

  describe('dispatchKeydown', () => {
    it('应该匹配快捷键并执行 handler', () => {
      let called = false;
      registry.register({
        id: 'shortcut',
        description: 'Shortcut',
        keybinding: { mod: true, key: 'k' },
        handler: () => { called = true; },
      });
      const event = new KeyboardEvent('keydown', { key: 'k', metaKey: true });
      expect(registry.dispatchKeydown(event)).toBe(true);
      expect(called).toBe(true);
    });

    it('Ctrl 键也应匹配 mod', () => {
      let called = false;
      registry.register({
        id: 'ctrl-shortcut',
        description: 'Ctrl',
        keybinding: { mod: true, key: 's' },
        handler: () => { called = true; },
      });
      const event = new KeyboardEvent('keydown', { key: 's', ctrlKey: true });
      expect(registry.dispatchKeydown(event)).toBe(true);
      expect(called).toBe(true);
    });

    it('Shift 组合键应该正确匹配', () => {
      let called = false;
      registry.register({
        id: 'shift-shortcut',
        description: 'Shift',
        keybinding: { mod: true, shift: true, key: 'k' },
        handler: () => { called = true; },
      });
      const event = new KeyboardEvent('keydown', { key: 'k', metaKey: true, shiftKey: true });
      expect(registry.dispatchKeydown(event)).toBe(true);
      expect(called).toBe(true);
    });

    it('不匹配的快捷键应返回 false', () => {
      registry.register({
        id: 'shortcut',
        description: 'Shortcut',
        keybinding: { mod: true, key: 'k' },
        handler: () => {},
      });
      const event = new KeyboardEvent('keydown', { key: 'j', metaKey: true });
      expect(registry.dispatchKeydown(event)).toBe(false);
    });

    it('condition 返回 false 时跳过执行', () => {
      let called = false;
      registry.register({
        id: 'guarded',
        description: 'Guarded',
        keybinding: { mod: true, key: 'g' },
        handler: () => { called = true; },
        condition: () => false,
      });
      const event = new KeyboardEvent('keydown', { key: 'g', metaKey: true });
      expect(registry.dispatchKeydown(event)).toBe(false);
      expect(called).toBe(false);
    });
  });

  describe('keybinding signature', () => {
    it('应该按正确顺序生成签名', () => {
      let capturedSig = '';
      registry.register({
        id: 'sig-test',
        description: 'Sig',
        keybinding: { alt: true, shift: true, mod: true, key: 'K' },
        handler: () => {},
      });
      // 触发一个事件来间接验证签名匹配
      const event = new KeyboardEvent('keydown', {
        key: 'k', metaKey: true, shiftKey: true, altKey: true,
      });
      expect(registry.dispatchKeydown(event)).toBe(true);
    });

    it('key 不区分大小写', () => {
      let called = false;
      registry.register({
        id: 'case-test',
        description: 'Case',
        keybinding: { mod: true, key: 'n' },
        handler: () => { called = true; },
      });
      const event = new KeyboardEvent('keydown', { key: 'N', metaKey: true });
      expect(registry.dispatchKeydown(event)).toBe(true);
      expect(called).toBe(true);
    });
  });
});

describe('createDefaultRegistry', () => {
  it('应该注册默认 Action 集合', () => {
    const registry = createDefaultRegistry({
      onNewChat: () => {},
      onImport: () => {},
      onSettings: () => {},
      onCommandPalette: () => {},
    });
    const actions = registry.listActions();
    expect(actions.length).toBeGreaterThanOrEqual(4);
    expect(actions.some((a) => a.id === 'new-chat')).toBe(true);
    expect(actions.some((a) => a.id === 'import-files')).toBe(true);
    expect(actions.some((a) => a.id === 'open-settings')).toBe(true);
    expect(actions.some((a) => a.id === 'command-palette')).toBe(true);
  });

  it('应该注册 toggle-sidebar Action', () => {
    const registry = createDefaultRegistry({
      onNewChat: () => {},
      onImport: () => {},
      onSettings: () => {},
      onToggleSidebar: () => {},
    });
    expect(registry.get('toggle-sidebar')).toBeDefined();
  });

  it('缺少 onCommandPalette 时不报错（使用默认空函数）', () => {
    const registry = createDefaultRegistry({
      onNewChat: () => {},
      onImport: () => {},
      onSettings: () => {},
    });
    // command-palette Action 仍应注册
    expect(registry.get('command-palette')).toBeDefined();
  });
});
