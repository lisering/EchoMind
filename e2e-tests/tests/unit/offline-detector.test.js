/**
 * EchoMind offline-detector.js 单元测试 — 在线/离线检测 / 状态切换 / 事件监听。
 *
 * 验证点：
 * 1. isOnline 返回 navigator.onLine
 * 2. initOfflineDetector 防止重复初始化
 * 3. initOfflineDetector 绑定 online/offline 事件
 * 4. _setOfflineForTest(true) 离线时禁用发送按钮
 * 5. _setOfflineForTest(true) 离线时更新输入框 placeholder
 * 6. _setOfflineForTest(false) 在线时恢复发送按钮
 * 7. _setOfflineForTest(false) 在线时恢复输入框 placeholder
 * 8. _setOfflineForTest(true) 离线时显示离线指示器
 *
 * Mock: i18n.js, icons.js, utils.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback || key,
}));

// Mock icons
vi.mock('../../../ui/src/utils.js', () => ({
  icon: vi.fn(() => '<svg></svg>'),
  $: (id) => document.getElementById(id),
}));



// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div id="sidebar">
      <div class="mt-auto"></div>
    </div>
    <button id="sendBtn">Send</button>
    <input id="queryInput" placeholder="Ask anything..." />
    <div id="inputHint">Press Enter to send</div>
  `;
}

setupDom();

import {
  isOnline,
  initOfflineDetector,
  _setOfflineForTest,
} from '../../../ui/src/network-utils.js';

describe('offline-detector.js — 离线检测', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
  });

  it('isOnline 返回 navigator.onLine', () => {
    // jsdom 默认 navigator.onLine = true
    expect(isOnline()).toBe(navigator.onLine);
  });

  it('initOfflineDetector 绑定 online/offline 事件', () => {
    const spy = vi.spyOn(window, 'addEventListener');
    initOfflineDetector();

    expect(spy).toHaveBeenCalledWith('online', expect.any(Function));
    expect(spy).toHaveBeenCalledWith('offline', expect.any(Function));
  });

  it('_setOfflineForTest(true) 离线时禁用发送按钮', () => {
    _setOfflineForTest(true);

    const sendBtn = document.getElementById('sendBtn');
    expect(sendBtn.hasAttribute('disabled')).toBe(true);
  });

  it('_setOfflineForTest(true) 离线时更新输入框 placeholder', () => {
    _setOfflineForTest(true);

    const input = document.getElementById('queryInput');
    expect(input.getAttribute('placeholder')).toContain('Offline');
  });

  it('_setOfflineForTest(false) 在线时恢复发送按钮', () => {
    // 先离线
    _setOfflineForTest(true);
    expect(document.getElementById('sendBtn').hasAttribute('disabled')).toBe(true);

    // 再恢复在线
    _setOfflineForTest(false);
    expect(document.getElementById('sendBtn').hasAttribute('disabled')).toBe(false);
  });

  it('_setOfflineForTest(false) 在线时恢复输入框 placeholder', () => {
    // 先离线
    _setOfflineForTest(true);
    const input = document.getElementById('queryInput');
    expect(input.getAttribute('placeholder')).toContain('Offline');

    // 再恢复在线
    _setOfflineForTest(false);
    expect(input.getAttribute('placeholder')).toBe('Ask anything...');
  });

  it('_setOfflineForTest(true) 离线时显示离线指示器（移除 hidden）', () => {
    _setOfflineForTest(true);

    const indicator = document.getElementById('offlineIndicator');
    expect(indicator).not.toBeNull();
    expect(indicator.classList.contains('hidden')).toBe(false);
  });

  it('_setOfflineForTest(false) 在线时隐藏离线指示器（添加 hidden）', () => {
    // 先离线
    _setOfflineForTest(true);
    const indicator = document.getElementById('offlineIndicator');
    expect(indicator.classList.contains('hidden')).toBe(false);

    // 再恢复在线
    _setOfflineForTest(false);
    expect(indicator.classList.contains('hidden')).toBe(true);
  });
});
