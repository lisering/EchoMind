/**
 * TC-DS-010~012: input-toggles.js 单元测试
 *
 * 验证输入区快速 Toggle 组件的创建、状态切换、初始激活态。
 *
 * 注意：使用 vitest.config.ts 提供的 jsdom 环境，无需自定义 DOM mock。
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => {
    const map = {
      'chat.toggle_hybrid': '混合搜索',
      'chat.toggle_agent': '深度思考',
    };
    return map[key] || key;
  },
}));

// Mock ipc（S09 统一入口：settingsApi.update_setting）
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn(),
  settingsApi: {
    update: vi.fn().mockResolvedValue(undefined),
    setBool: vi.fn().mockResolvedValue(undefined),
    get: vi.fn().mockResolvedValue(''),
  },
}));

// Mock state
vi.mock('../../../ui/src/state.js', () => {
  const state = {};
  return {
    get: (k) => state[k],
    setState: (patch) => { Object.assign(state, patch); },
    subscribe: (_k, _fn) => () => {},
  };
});

import { createInputToggle, getToggleState } from '../../../ui/src/input-toggles.js';

describe('input-toggles.js — 输入区快速 Toggle', () => {
  let toggleEl;

  beforeEach(() => {
    toggleEl = createInputToggle('hybrid', false);
  });

  it('TC-DS-010: createInputToggle 返回含 label 文本的元素', () => {
    expect(toggleEl).toBeDefined();
    expect(toggleEl.className).toContain('input-toggle');
    expect(toggleEl.textContent).toContain('混合搜索');
    expect(toggleEl.querySelector('svg')).not.toBeNull();
  });

  it('TC-DS-011: 点击 toggle 切换 active 类名', () => {
    expect(toggleEl.classList.contains('text-accent')).toBe(false);
    expect(toggleEl.classList.contains('text-text-tertiary')).toBe(true);
    toggleEl.click();
    expect(toggleEl.classList.contains('text-accent')).toBe(true);
    expect(toggleEl.classList.contains('bg-accent/10')).toBe(true);
    expect(toggleEl.classList.contains('text-text-tertiary')).toBe(false);
    toggleEl.click();
    expect(toggleEl.classList.contains('text-accent')).toBe(false);
    expect(toggleEl.classList.contains('text-text-tertiary')).toBe(true);
  });

  it('TC-DS-012: 初始 active=true 时含 text-accent 类', () => {
    const activeToggle = createInputToggle('hybrid', true);
    expect(activeToggle.classList.contains('text-accent')).toBe(true);
    expect(activeToggle.classList.contains('bg-accent/10')).toBe(true);
    expect(activeToggle.getAttribute('aria-checked')).toBe('true');
  });

  it('TC-DS-012b: getToggleState 返回当前状态', () => {
    expect(getToggleState('hybrid')).toBe(false);
    toggleEl.click();
    expect(getToggleState('hybrid')).toBe(true);
  });

  it('TC-DS-012c: role=switch + aria-checked 正确设置', () => {
    expect(toggleEl.getAttribute('role')).toBe('switch');
    expect(toggleEl.getAttribute('aria-checked')).toBe('false');
    toggleEl.click();
    expect(toggleEl.getAttribute('aria-checked')).toBe('true');
  });

  it('TC-DS-012d: Enter/Space 键触发切换', () => {
    const enterEvent = new KeyboardEvent('keydown', { key: 'Enter' });
    toggleEl.dispatchEvent(enterEvent);
    expect(toggleEl.classList.contains('text-accent')).toBe(true);
  });

  it('TC-DS-012e: 未知 settingKey 返回空 div', () => {
    const unknown = createInputToggle('unknown_key', false);
    expect(unknown.tagName).toBe('DIV');
    expect(unknown.children.length).toBe(0);
  });
});
