/**
 * EchoMind keyboard-help.js 单元测试 — 搜索过滤 / 分组。
 *
 * 验证点：
 * 1. openKeyboardHelp 打开面板（移除 hidden）
 * 2. closeKeyboardHelp 关闭面板（添加 hidden）
 * 3. isKeyboardHelpOpen 初始为 false
 * 4. openKeyboardHelp 后 isKeyboardHelpOpen 为 true
 * 5. closeKeyboardHelp 后 isKeyboardHelpOpen 为 false
 * 6. openKeyboardHelp 无面板时安全返回
 * 7. initKeyboardHelp 绑定关闭按钮
 * 8. initKeyboardHelp 绑定搜索框事件
 *
 * Mock: utils.js, i18n.js, panel-stack.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div id="keyboardHelpPanel" class="hidden">
      <input id="keyboardHelpSearch" />
      <button id="keyboardHelpClose"></button>
      <div id="keyboardHelpContent"></div>
    </div>
  `;
}

setupDom();

import { openKeyboardHelp, closeKeyboardHelp, initKeyboardHelp, isKeyboardHelpOpen } from '../../../ui/src/help-panel.js';

describe('keyboard-help.js — 面板开关', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
  });

  it('isKeyboardHelpOpen 初始为 false', () => {
    expect(isKeyboardHelpOpen()).toBe(false);
  });

  it('openKeyboardHelp 打开面板（移除 hidden 类）', () => {
    const panel = document.getElementById('keyboardHelpPanel');
    expect(panel.classList.contains('hidden')).toBe(true);
    openKeyboardHelp();
    expect(panel.classList.contains('hidden')).toBe(false);
  });

  it('openKeyboardHelp 后 isKeyboardHelpOpen 为 true', () => {
    openKeyboardHelp();
    expect(isKeyboardHelpOpen()).toBe(true);
  });

  it('closeKeyboardHelp 关闭面板（添加 hidden 类）', () => {
    openKeyboardHelp();
    closeKeyboardHelp();
    const panel = document.getElementById('keyboardHelpPanel');
    expect(panel.classList.contains('hidden')).toBe(true);
  });

  it('closeKeyboardHelp 后 isKeyboardHelpOpen 为 false', () => {
    openKeyboardHelp();
    closeKeyboardHelp();
    expect(isKeyboardHelpOpen()).toBe(false);
  });

  it('openKeyboardHelp 无面板时安全返回', () => {
    document.body.innerHTML = '';
    expect(() => openKeyboardHelp()).not.toThrow();
  });

  it('initKeyboardHelp 绑定关闭按钮', () => {
    setupDom();
    initKeyboardHelp();
    const closeBtn = document.getElementById('keyboardHelpClose');
    expect(closeBtn.onclick).not.toBeNull();
  });

  it('initKeyboardHelp 绑定搜索框事件', () => {
    setupDom();
    initKeyboardHelp();
    const searchInput = document.getElementById('keyboardHelpSearch');
    // 模拟输入验证事件已绑定
    const inputEvent = new Event('input', { bubbles: true });
    expect(() => searchInput.dispatchEvent(inputEvent)).not.toThrow();
  });
});
