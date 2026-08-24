/**
 * EchoMind doc-keyboard.js 单元测试 — 文档列表键盘导航 / 上下选择 / Enter 打开。
 *
 * 验证点：
 * 1. initDocKeyboard 无 docList 时安全返回
 * 2. getSelectedDocIds 默认返回空数组
 * 3. setMultiSelectMode(false) 关闭多选并清空选中
 * 4. ArrowDown 初始化选中到第一项
 * 5. ArrowUp 初始化选中到最后一项
 * 6. Enter 触发 doc-preview-requested 事件
 * 7. Escape 清除选中状态
 * 8. 点击文档项更新选中索引
 * 9. Cmd+A 全选文档
 * 10. ArrowDown 连续按下导航到第二项
 *
 * Mock: i18n.js, ipc.js (docApi), toast.js, confirm-dialog.js, utils.js
 *
 * 注意：
 * - _kbSelIdx 是模块级单例，跨测试保持状态
 * - jsdom 的 document.activeElement 可能不匹配 kbScroll
 *   → 通过在 docList 子元素上 dispatch 事件 + focus() 确保 inDocList 检查通过
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback || key,
}));

// Mock ipc with docApi
vi.mock('../../../ui/src/ipc.js', () => ({
  docApi: {
    delete: vi.fn(() => Promise.resolve()),
  },
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

// Mock confirm-dialog
vi.mock('../../../ui/src/confirm-dialog.js', () => ({
  showConfirmDialog: vi.fn(() => Promise.resolve(false)),
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// jsdom doesn't implement scrollIntoView
Element.prototype.scrollIntoView = vi.fn();

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div id="kbDocScroll" tabindex="0">
      <div id="docList">
        <div data-doc-id="doc-1" data-doc-name="Doc 1" tabindex="0">Doc 1</div>
        <div data-doc-id="doc-2" data-doc-name="Doc 2" tabindex="0">Doc 2</div>
        <div data-doc-id="doc-3" data-doc-name="Doc 3" tabindex="0">Doc 3</div>
      </div>
    </div>
  `;
}

setupDom();

import {
  initDocKeyboard,
  getSelectedDocIds,
  setMultiSelectMode,
} from '../../../ui/src/doc-nav.js';

/**
 * 辅助：在 docList 内部元素上 focus + dispatch keydown
 * 确保 document.activeElement.closest('#docList') 匹配
 */
function dispatchKeyOnDocList(key, extra = {}) {
  const items = document.querySelectorAll('[data-doc-id]');
  // focus 第一个文档项，使 activeElement.closest('#docList') 匹配
  if (items.length > 0) {
    items[0].focus();
  }
  const docList = document.getElementById('docList');
  docList.dispatchEvent(new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    ...extra,
  }));
}

describe('doc-keyboard.js — 文档列表键盘导航', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
    // 重置多选状态（间接重置部分内部状态）
    setMultiSelectMode(false);
  });

  it('initDocKeyboard 无 docList 时安全返回不报错', () => {
    document.body.innerHTML = '<div id="other"></div>';
    expect(() => initDocKeyboard()).not.toThrow();
  });

  it('getSelectedDocIds 默认返回空数组', () => {
    initDocKeyboard();
    expect(getSelectedDocIds()).toEqual([]);
  });

  it('setMultiSelectMode(false) 关闭多选并清空选中', () => {
    initDocKeyboard();
    setMultiSelectMode(true);
    setMultiSelectMode(false);
    expect(getSelectedDocIds()).toEqual([]);
  });

  it('ArrowDown 初始化选中到第一项', () => {
    initDocKeyboard();
    dispatchKeyOnDocList('ArrowDown');

    const items = document.querySelectorAll('[data-doc-id]');
    expect(items[0].classList.contains('kb-keyboard-selected')).toBe(true);
  });

  it('ArrowUp 初始化选中到最后一项', () => {
    initDocKeyboard();
    // 先重置：Escape 清除选中状态
    dispatchKeyOnDocList('Escape');
    // ArrowUp 应初始化到最后一项
    dispatchKeyOnDocList('ArrowUp');

    const items = document.querySelectorAll('[data-doc-id]');
    expect(items[2].classList.contains('kb-keyboard-selected')).toBe(true);
  });

  it('Enter 触发 doc-preview-requested 事件', () => {
    initDocKeyboard();
    // 先 Escape 重置选中状态
    dispatchKeyOnDocList('Escape');
    // ArrowDown 选中第一项
    dispatchKeyOnDocList('ArrowDown');

    // 确认选中第一项
    const items = document.querySelectorAll('[data-doc-id]');
    expect(items[0].classList.contains('kb-keyboard-selected')).toBe(true);

    // 监听 doc-preview-requested 事件
    let eventDetail = null;
    document.addEventListener('doc-preview-requested', (e) => {
      eventDetail = e.detail;
    });

    // 按 Enter
    dispatchKeyOnDocList('Enter');

    expect(eventDetail).not.toBeNull();
    expect(eventDetail.docId).toBe('doc-1');
  });

  it('Escape 清除选中状态', () => {
    initDocKeyboard();
    // 先 Escape 重置
    dispatchKeyOnDocList('Escape');
    // ArrowDown 选中第一项
    dispatchKeyOnDocList('ArrowDown');
    let items = document.querySelectorAll('[data-doc-id]');
    expect(items[0].classList.contains('kb-keyboard-selected')).toBe(true);

    // 按 Escape 清除
    dispatchKeyOnDocList('Escape');
    items = document.querySelectorAll('[data-doc-id]');
    expect(items[0].classList.contains('kb-keyboard-selected')).toBe(false);
  });

  it('点击文档项更新选中索引', () => {
    initDocKeyboard();
    const items = document.querySelectorAll('[data-doc-id]');
    items[1].click();

    expect(items[1].classList.contains('kb-keyboard-selected')).toBe(true);
    expect(items[0].classList.contains('kb-keyboard-selected')).toBe(false);
  });

  it('Cmd+A 全选文档', () => {
    initDocKeyboard();
    dispatchKeyOnDocList('a', { metaKey: true });

    expect(getSelectedDocIds().length).toBe(3);
  });

  it('ArrowDown 连续按下导航到第二项', () => {
    initDocKeyboard();
    // 先 Escape 清除之前的状态
    dispatchKeyOnDocList('Escape');
    // ArrowDown 两次
    dispatchKeyOnDocList('ArrowDown');
    dispatchKeyOnDocList('ArrowDown');

    const items = document.querySelectorAll('[data-doc-id]');
    expect(items[0].classList.contains('kb-keyboard-selected')).toBe(false);
    expect(items[1].classList.contains('kb-keyboard-selected')).toBe(true);
  });
});
