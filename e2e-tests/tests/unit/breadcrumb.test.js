/**
 * EchoMind breadcrumb.js 单元测试 — 面包屑导航 / 层级路径 / 点击跳转。
 *
 * 验证点：
 * 1. initBreadcrumb 创建面包屑容器
 * 2. initBreadcrumb 渲染知识库名称和会话标题
 * 3. updateBreadcrumb 更新会话标题
 * 4. updateBreadcrumb 显示消息数
 * 5. updateBreadcrumb 空标题显示 "新会话"
 * 6. clearBreadcrumb 清空会话信息
 * 7. 点击知识库名触发 onNavigateKb 回调
 * 8. 点击会话标题进入重命名模式
 * 9. initBreadcrumb 无容器时安全返回
 * 10. updateBreadcrumb 显示创建时间
 *
 * Mock: i18n.js, ipc.js, date-utils.js, utils.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback || key,
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
}));

// Mock date-utils
// Mock utils (含 date-utils 合并后的 formatRelativeTime)
vi.mock('../../../ui/src/utils.js', () => ({
$: (id) => document.getElementById(id),
formatRelativeTime: vi.fn((ts) => `relative-${ts}`),
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <main>
      <div class="flex-1">
        <div class="relative">
          <!-- breadcrumb will be inserted here -->
        </div>
      </div>
    </main>
  `;
}

setupDom();

import { initBreadcrumb, updateBreadcrumb, clearBreadcrumb } from '../../../ui/src/doc-nav.js';

describe('breadcrumb.js — 面包屑导航', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
  });

  it('initBreadcrumb 创建面包屑容器', () => {
    initBreadcrumb();
    const bar = document.getElementById('breadcrumbBar');
    expect(bar).not.toBeNull();
  });

  it('initBreadcrumb 渲染知识库名称和会话标题', () => {
    initBreadcrumb();
    const bar = document.getElementById('breadcrumbBar');
    expect(bar.textContent).toContain('Knowledge Base');
    expect(bar.textContent).toContain('New Chat');
  });

  it('updateBreadcrumb 更新会话标题', () => {
    initBreadcrumb();
    updateBreadcrumb('conv-1', 'My Conversation', 5, Date.now());

    const bar = document.getElementById('breadcrumbBar');
    expect(bar.textContent).toContain('My Conversation');
  });

  it('updateBreadcrumb 显示消息数', () => {
    initBreadcrumb();
    updateBreadcrumb('conv-1', 'Test', 10, 0);

    const bar = document.getElementById('breadcrumbBar');
    expect(bar.textContent).toContain('10');
    expect(bar.textContent).toContain('messages');
  });

  it('updateBreadcrumb 空标题显示 "New Chat"', () => {
    initBreadcrumb();
    updateBreadcrumb('conv-1', '', 0, 0);

    const bar = document.getElementById('breadcrumbBar');
    expect(bar.textContent).toContain('New Chat');
  });

  it('clearBreadcrumb 清空会话信息', () => {
    initBreadcrumb();
    updateBreadcrumb('conv-1', 'Some Title', 5, Date.now());

    clearBreadcrumb();

    const bar = document.getElementById('breadcrumbBar');
    expect(bar.textContent).toContain('New Chat');
    // 消息数应该不再显示
    expect(bar.querySelector('.breadcrumb-meta')).toBeNull();
  });

  it('点击知识库名触发 onNavigateKb 回调', () => {
    const onNavigateKb = vi.fn();
    initBreadcrumb({ onNavigateKb });

    const kbName = document.getElementById('breadcrumbKbName');
    kbName.click();

    expect(onNavigateKb).toHaveBeenCalledTimes(1);
  });

  it('点击会话标题进入重命名模式（替换为 input）', () => {
    initBreadcrumb();
    updateBreadcrumb('conv-1', 'Original Title', 0, 0);

    const titleEl = document.getElementById('breadcrumbConvTitle');
    titleEl.click();

    // 重命名模式应创建一个 input 元素
    const input = document.querySelector('.breadcrumb-title-input');
    expect(input).not.toBeNull();
    expect(input.tagName).toBe('INPUT');
    expect(input.value).toBe('Original Title');
  });

  it('updateBreadcrumb 显示创建时间（通过 formatRelativeTime）', () => {
    initBreadcrumb();
    const ts = 1700000000000;
    updateBreadcrumb('conv-1', 'Test', 1, ts);

    const bar = document.getElementById('breadcrumbBar');
    expect(bar.textContent).toContain(`relative-${ts}`);
  });

  it('initBreadcrumb 设置 onRename 回调', () => {
    const onRename = vi.fn();
    initBreadcrumb({ onRename });

    updateBreadcrumb('conv-1', 'Old Title', 0, 0);

    const titleEl = document.getElementById('breadcrumbConvTitle');
    titleEl.click();

    // 进入重命名模式
    const input = document.querySelector('.breadcrumb-title-input');
    expect(input).not.toBeNull();

    // 修改标题并按 Enter
    input.value = 'New Title';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));

    // onRename 回调应被调用
    expect(onRename).toHaveBeenCalledWith('New Title');
  });
});
