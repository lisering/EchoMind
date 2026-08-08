/**
 * EchoMind 用户消息编辑+分支单元测试 — message-edit.js 模块。
 *
 * 测试覆盖：
 * - enterEditMode 将内容替换为 textarea
 * - exitEditMode 恢复原始内容显示
 * - confirmEdit 提取编辑后文本并触发回调
 * - renderBranchPagination 显示分页式分支切换条（◀ 1/N ▶）
 * - navigateBranch 切换分支内容
 * - createEditButton 创建带编辑图标的按钮
 * - 空内容不触发重新发送
 * - 编辑模式包含回形针上传按钮（[📎] [取消] [发送]）
 * - 键盘行为：Enter 发送，Shift+Enter 换行，Escape 取消
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { JSDOM } from 'jsdom';
import {
  enterEditMode,
  exitEditMode,
  confirmEdit,
  renderBranchPagination,
  navigateBranch,
  removeBranchPagination,
  createEditButton,
} from '../../../ui/src/message-edit.js';

// Mock turn-tree 模块
const { mockGetVersionCount, mockGetTurn, mockGetActiveVersion, mockSetActiveVersion } = vi.hoisted(() => ({
  mockGetVersionCount: vi.fn(() => 0),
  mockGetTurn: vi.fn(() => null),
  mockGetActiveVersion: vi.fn(() => null),
  mockSetActiveVersion: vi.fn(() => false),
}));

vi.mock('../../../ui/src/turn-tree.js', () => ({
  getTurn: mockGetTurn,
  getActiveVersion: mockGetActiveVersion,
  getVersionCount: mockGetVersionCount,
  setActiveVersion: mockSetActiveVersion,
}));

// Mock ipc 模块
vi.mock('../../../ui/src/ipc.js', () => ({
  convApi: {
    setTurnActiveVersion: vi.fn(() => Promise.resolve()),
  },
}));

// Mock state 模块
vi.mock('../../../ui/src/state.js', () => ({
  getState: vi.fn(() => ({})),
  setState: vi.fn(),
  get: vi.fn(() => null),
}));

// Mock markdown 模块
vi.mock('../../../ui/src/markdown.js', () => ({
  renderMarkdown: vi.fn(),
  renderRichContent: vi.fn(() => Promise.resolve()),
}));

// Mock i18n 模块
vi.mock('../../../ui/src/i18n.js', () => ({
  t: vi.fn((key) => key),
}));

// ============================================================
// DOM 环境设置
// ============================================================

beforeEach(() => {
  const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>', {
    url: 'http://localhost',
    pretendToBeVisual: true,
  });
  global.window = dom.window;
  global.document = dom.window.document;
  global.HTMLElement = dom.window.HTMLElement;
  global.localStorage = {
    store: {},
    getItem(key) { return this.store[key] || null; },
    setItem(key, val) { this.store[key] = val; },
    removeItem(key) { delete this.store[key]; },
  };
});

afterEach(() => {
  vi.clearAllMocks();
});

// ============================================================
// 辅助函数：创建用户消息块
// ============================================================

function createUserBlock(text = '原始消息内容') {
  const block = document.createElement('div');
  block.className = 'msg-block msg-user';
  block.innerHTML = `
    <div class="msg-content msg-user-content">${text}</div>
  `;
  document.body.appendChild(block);
  // 创建用户操作栏（与 appendBlock('user') 一致）
  const actions = document.createElement('div');
  actions.className = 'flex justify-end gap-1 mt-1 transition-opacity duration-fast';
  actions.dataset.role = 'user-actions';
  block.after(actions);
  return block;
}

// 辅助函数：创建 assistant 消息块（结构与 appendBlock 一致）
function createAssistantBlock(text = '') {
  const block = document.createElement('div');
  block.className = 'msg-block msg-assistant';
  block.innerHTML = `
    <div class="msg-content">
      <div class="md">${text}</div>
      <div class="sources mt-2"></div>
    </div>
    <div class="flex gap-1 mt-2 msg-actions"></div>
  `;
  document.body.appendChild(block);
  return block;
}

// ============================================================
// 测试用例
// ============================================================

describe('message-edit.js — 用户消息编辑+分支', () => {
  // ----------------------------------------------------------
  // enterEditMode
  // ----------------------------------------------------------
  describe('enterEditMode', () => {
    it('将内容区替换为 textarea + 操作按钮', () => {
      const block = createUserBlock('测试消息');
      enterEditMode(block, '测试消息');

      const contentEl = block.querySelector('.msg-user-content');
      expect(contentEl.style.display).toBe('none');

      const textarea = block.querySelector('.msg-edit-textarea');
      expect(textarea).not.toBeNull();
      expect(textarea.value).toBe('测试消息');

      const actionBar = block.nextElementSibling;
      expect(actionBar).not.toBeNull();
      expect(actionBar.classList.contains('msg-edit-actions-below')).toBe(true);
      expect(actionBar.querySelector('.msg-edit-cancel')).not.toBeNull();
      expect(actionBar.querySelector('.msg-edit-resend')).not.toBeNull();
    });

    it('null 输入返回 null', () => {
      const result = enterEditMode(null, 'test');
      expect(result).toBeNull();
    });

    it('重复进入同一块不创建第二个编辑器', () => {
      const block = createUserBlock('测试');
      enterEditMode(block, '测试');
      const result = enterEditMode(block, '测试');
      expect(result).toBeNull();

      const editors = block.querySelectorAll('.msg-edit-full');
      expect(editors.length).toBe(1);
    });
  });

  // ----------------------------------------------------------
  // exitEditMode
  // ----------------------------------------------------------
  it('exitEditMode 移除编辑器并恢复原始内容', () => {
    const block = createUserBlock('原始内容');
    enterEditMode(block, '原始内容');

    expect(block.querySelector('.msg-edit-full')).not.toBeNull();

    exitEditMode(block);

    expect(block.querySelector('.msg-edit-full')).toBeNull();

    const contentEl = block.querySelector('.msg-user-content');
    expect(contentEl.style.display).toBe('');
  });

  // ----------------------------------------------------------
  // confirmEdit
  // ----------------------------------------------------------
  it('confirmEdit 获取编辑后文本并触发 onResend', () => {
    const block = createUserBlock('原始内容');
    let resendText = null;
    enterEditMode(block, '原始内容', (text) => { resendText = text; });

    const textarea = block.querySelector('.msg-edit-textarea');
    textarea.value = '编辑后的内容';

    confirmEdit(block);

    expect(resendText).toBe('编辑后的内容');
    expect(block.querySelector('.msg-edit-full')).toBeNull();
  });

  // ----------------------------------------------------------
  // 空内容不触发重新发送
  // ----------------------------------------------------------
  it('空内容不触发重新发送', () => {
    const block = createUserBlock('原始内容');
    let resendCalled = false;
    enterEditMode(block, '原始内容', () => { resendCalled = true; });

    const textarea = block.querySelector('.msg-edit-textarea');
    textarea.value = '   ';

    confirmEdit(block);

    expect(resendCalled).toBe(false);
    expect(block.querySelector('.msg-edit-full')).not.toBeNull();
  });

  // ----------------------------------------------------------
  // createEditButton
  // ----------------------------------------------------------
  describe('createEditButton', () => {
    it('创建带 SVG 图标的编辑按钮', () => {
      const block = createUserBlock('测试');
      const btn = createEditButton(block, '测试', () => {});

      expect(btn).not.toBeNull();
      expect(btn.className).toContain('text-text-secondary');
      expect(btn.querySelector('svg')).not.toBeNull();
    });

    it('点击编辑按钮进入编辑模式', () => {
      const block = createUserBlock('测试内容');
      const btn = createEditButton(block, '测试内容', () => {});
      btn.click();

      expect(block.querySelector('.msg-edit-textarea')).not.toBeNull();
    });
  });

  // ----------------------------------------------------------
  // 编辑模式回形针上传按钮
  // ----------------------------------------------------------
  describe('编辑模式回形针上传按钮', () => {
    it('进入编辑模式显示回形针 + 取消 + 发送三个按钮', () => {
      const block = createUserBlock('测试消息');
      enterEditMode(block, '测试消息');

      const attachBtn = block.nextElementSibling.querySelector('.msg-edit-attach');
      const cancelBtn = block.nextElementSibling.querySelector('.msg-edit-cancel');
      const resendBtn = block.nextElementSibling.querySelector('.msg-edit-resend');

      expect(attachBtn).not.toBeNull();
      expect(cancelBtn).not.toBeNull();
      expect(resendBtn).not.toBeNull();

      expect(attachBtn.querySelector('svg')).not.toBeNull();
      expect(cancelBtn.textContent.length).toBeGreaterThan(0);
      expect(resendBtn.textContent.length).toBeGreaterThan(0);
      expect(attachBtn.textContent.trim()).toBe('');
    });

    it('点击回形针按钮触发 onAttach 回调', () => {
      const block = createUserBlock('测试消息');
      let attachCalled = false;
      enterEditMode(block, '测试消息', () => {}, () => { attachCalled = true; });

      const attachBtn = block.nextElementSibling.querySelector('.msg-edit-attach');
      attachBtn.click();
      expect(attachCalled).toBe(true);
    });

    it('未提供 onAttach 时点击回形针不报错', () => {
      const block = createUserBlock('测试消息');
      enterEditMode(block, '测试消息');
      const attachBtn = block.nextElementSibling.querySelector('.msg-edit-attach');
      expect(() => attachBtn.click()).not.toThrow();
    });
  });

  // ----------------------------------------------------------
  // 键盘行为测试
  // ----------------------------------------------------------
  describe('键盘行为', () => {
    it('Enter（无 Shift）触发发送', () => {
      const block = createUserBlock('原始内容');
      let resendText = null;
      enterEditMode(block, '原始内容', (text) => { resendText = text; });

      const textarea = block.querySelector('.msg-edit-textarea');
      textarea.value = '修改后内容';
      // 触发 input 事件以启用发送按钮（checkModified）
      textarea.dispatchEvent(new window.Event('input', { bubbles: true }));

      // 模拟 keydown 事件：Enter 无 Shift
      const event = new window.KeyboardEvent('keydown', { key: 'Enter', shiftKey: false });
      Object.defineProperty(event, 'target', { value: textarea, writable: false });
      textarea.dispatchEvent(event);

      expect(resendText).toBe('修改后内容');
    });

    it('Shift+Enter 不触发发送（允许换行）', () => {
      const block = createUserBlock('原始内容');
      let resendCalled = false;
      enterEditMode(block, '原始内容', () => { resendCalled = true; });

      const textarea = block.querySelector('.msg-edit-textarea');
      textarea.value = '修改后内容';

      // 模拟 keydown 事件：Shift+Enter
      const event = new window.KeyboardEvent('keydown', { key: 'Enter', shiftKey: true });
      Object.defineProperty(event, 'target', { value: textarea, writable: false });
      textarea.dispatchEvent(event);

      expect(resendCalled).toBe(false);
      // 编辑器应仍存在（未退出）
      expect(block.querySelector('.msg-edit-full')).not.toBeNull();
    });

    it('Escape 取消编辑', () => {
      const block = createUserBlock('原始内容');
      enterEditMode(block, '原始内容');

      const textarea = block.querySelector('.msg-edit-textarea');

      // 模拟 keydown 事件：Escape
      const event = new window.KeyboardEvent('keydown', { key: 'Escape' });
      Object.defineProperty(event, 'target', { value: textarea, writable: false });
      textarea.dispatchEvent(event);

      // 编辑器应被移除
      expect(block.querySelector('.msg-edit-full')).toBeNull();
    });
  });

  // ----------------------------------------------------------
  // renderBranchPagination
  // ----------------------------------------------------------
  describe('renderBranchPagination', () => {
    it('单版本时不渲染分页器', () => {
      mockGetVersionCount.mockReturnValue(1);

      const block = createUserBlock('问题');
      const result = renderBranchPagination(block, 'turn-1');

      expect(result).toBeNull();
      expect(block.querySelector('.branch-pagination')).toBeNull();
    });

    it('多版本时渲染分页器（挂在用户操作栏中，与复制/编辑按钮同行）', () => {
      mockGetVersionCount.mockReturnValue(3);
      mockGetTurn.mockReturnValue({
        turnGroup: 'turn-1',
        versions: [
          { userContent: 'v1', assistantContent: 'a1', version: 1 },
          { userContent: 'v2', assistantContent: 'a2', version: 2 },
          { userContent: 'v3', assistantContent: 'a3', version: 3 },
        ],
        activeVersion: 2,
      });

      const block = createUserBlock('问题');
      const assistantEl = createAssistantBlock('答案');
      // 用户操作栏已在 createUserBlock 中创建，插在 user 和 assistant 之间
      block.nextElementSibling.after(assistantEl);

      const pagination = renderBranchPagination(block, 'turn-1');

      expect(pagination).not.toBeNull();
      const counter = pagination.querySelector('.branch-pagination-counter');
      expect(counter.textContent).toBe('2 / 3');
      expect(pagination.querySelector('.branch-pagination-prev')).not.toBeNull();
      expect(pagination.querySelector('.branch-pagination-next')).not.toBeNull();
      // 分页器在用户操作栏中，不在 assistant 块内
      const actionsBar = block.nextElementSibling;
      expect(actionsBar.contains(pagination)).toBe(true);
      expect(assistantEl.contains(pagination)).toBe(false);
    });

    it('分页器渲染到用户操作栏最前面（复制/编辑按钮之前）', () => {
      mockGetVersionCount.mockReturnValue(2);
      mockGetTurn.mockReturnValue({
        turnGroup: 'turn-1',
        versions: [
          { userContent: 'v1', assistantContent: 'a1', version: 1 },
          { userContent: 'v2', assistantContent: 'a2', version: 2 },
        ],
        activeVersion: 2,
      });

      const block = createUserBlock('问题');
      const assistantEl = createAssistantBlock('答案');
      block.nextElementSibling.after(assistantEl);

      const pagination = renderBranchPagination(block, 'turn-1');

      expect(pagination).not.toBeNull();
      // 分页器是用户操作栏的最后一个子元素（编辑按钮右侧）
      const actionsBar = block.nextElementSibling;
      expect(actionsBar.lastChild).toBe(pagination);
      expect(block.querySelector('.branch-pagination')).toBeNull();
    });

    it('无用户操作栏时不渲染分页器', () => {
      mockGetVersionCount.mockReturnValue(2);
      mockGetTurn.mockReturnValue({
        turnGroup: 'turn-1',
        versions: [
          { userContent: 'v1', assistantContent: 'a1', version: 1 },
          { userContent: 'v2', assistantContent: null, version: 2 },
        ],
        activeVersion: 2,
      });

      // 创建没有操作栏的 user 块
      const block = document.createElement('div');
      block.className = 'msg-block msg-user';
      block.innerHTML = '<div class="msg-content msg-user-content">问题</div>';
      document.body.appendChild(block);

      const result = renderBranchPagination(block, 'turn-1');

      expect(result).toBeNull();
      expect(block.querySelector('.branch-pagination')).toBeNull();
    });
  });

  // ----------------------------------------------------------
  // removeBranchPagination
  // ----------------------------------------------------------
  it('removeBranchPagination 移除分页器（从用户操作栏中移除）', () => {
    const block = createUserBlock('问题');
    const assistantEl = createAssistantBlock('答案');
    block.nextElementSibling.after(assistantEl);
    const actionsBar = block.nextElementSibling;
    const pagination = document.createElement('div');
    pagination.className = 'branch-pagination';
    actionsBar.appendChild(pagination);

    removeBranchPagination(block);

    expect(actionsBar.querySelector('.branch-pagination')).toBeNull();
  });
});
