/**
 * EchoMind 用户消息编辑高级测试 — 新增编辑/重发按钮行为（TC-EDIT-001~010）。
 *
 * 测试覆盖：
 * - TC-EDIT-001: 进入编辑模式时重发按钮初始禁用
 * - TC-EDIT-002: 修改文本后重发按钮启用
 * - TC-EDIT-003: 清空文本后重发按钮禁用
 * - TC-EDIT-004: 取消按钮为 X 图标（无文字）
 * - TC-EDIT-005: 重发按钮为发送图标（无文字，与 #sendBtn 一致）
 * - TC-EDIT-006: 取消和重发按钮尺寸一致
 * - TC-EDIT-007: 未修改时 Ctrl+Enter 不触发重发
 * - TC-EDIT-008: 修改后 Ctrl+Enter 触发重发
 * - TC-EDIT-009: Escape 键取消编辑
 * - TC-EDIT-010: 修改后恢复原文重新禁用重发按钮
 * - TC-EDIT-011: 按钮栏有正确的间距和布局
 * - TC-EDIT-012: 按钮具有 aria-label 属性
 * - TC-EDIT-013: 空内容时重发按钮禁用
 * - TC-EDIT-014: 仅空白字符时重发按钮禁用
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { JSDOM } from 'jsdom';
import {
  enterEditMode,
  exitEditMode,
  confirmEdit,
} from '../../../ui/src/message-edit.js';

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
  if (global.document) {
    const editors = global.document.querySelectorAll('.msg-edit-wrapper');
    editors.forEach(e => e.remove());
  }
});

// ============================================================
// 辅助函数
// ============================================================

function createUserBlock(text = '原始消息内容') {
  const block = document.createElement('div');
  block.className = 'msg-block msg-user';
  block.innerHTML = `
    <div class="msg-role-header">
      <span class="msg-role-icon">👤</span>
      <span class="msg-role-name">用户</span>
    </div>
    <div class="msg-content msg-user-content">${text}</div>
    <div class="msg-actions"></div>
  `;
  document.body.appendChild(block);
  return block;
}

/** 模拟 input 事件（JSDOM 不自动触发） */
function simulateInput(textarea, value) {
  textarea.value = value;
  textarea.dispatchEvent(new window.Event('input', { bubbles: true }));
}

// ============================================================
// 测试用例
// ============================================================

describe('message-edit 高级行为 — 编辑/重发按钮', () => {

  // ----------------------------------------------------------
  // TC-EDIT-001: 进入编辑模式时重发按钮初始禁用
  // ----------------------------------------------------------
  it('TC-EDIT-001: 重发按钮初始状态为 disabled', () => {
    const block = createUserBlock('测试消息');
    enterEditMode(block, '测试消息');

    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');
    expect(resendBtn).not.toBeNull();
    expect(resendBtn.disabled).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-002: 修改文本后重发按钮启用
  // ----------------------------------------------------------
  it('TC-EDIT-002: 修改文本后重发按钮变为 enabled', () => {
    const block = createUserBlock('原始内容');
    enterEditMode(block, '原始内容');

    const textarea = block.querySelector('.msg-edit-textarea');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    // 修改内容
    simulateInput(textarea, '修改后的内容');
    expect(resendBtn.disabled).toBe(false);
  });

  // ----------------------------------------------------------
  // TC-EDIT-003: 清空文本后重发按钮禁用
  // ----------------------------------------------------------
  it('TC-EDIT-003: 清空文本后重发按钮变为 disabled', () => {
    const block = createUserBlock('原始内容');
    enterEditMode(block, '原始内容');

    const textarea = block.querySelector('.msg-edit-textarea');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    // 先修改启用
    simulateInput(textarea, '修改内容');
    expect(resendBtn.disabled).toBe(false);

    // 再清空
    simulateInput(textarea, '');
    expect(resendBtn.disabled).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-004: 取消按钮为文字按钮（参照编辑态设计截图：[取消]）
  // ----------------------------------------------------------
  it('TC-EDIT-004: 取消按钮为文字按钮且有 aria-label', () => {
    const block = createUserBlock('测试');
    enterEditMode(block, '测试');

    const cancelBtn = block.parentElement.querySelector('.msg-edit-cancel');
    expect(cancelBtn).not.toBeNull();

    // 文字按钮（非纯图标）
    expect(cancelBtn.textContent.trim().length).toBeGreaterThan(0);
    expect(cancelBtn.querySelector('svg')).toBeNull();
    // 无障碍标签
    expect(cancelBtn.getAttribute('aria-label')).not.toBeNull();
  });

  // ----------------------------------------------------------
  // TC-EDIT-005: 重发按钮为文字按钮（参照编辑态设计截图：[发送]）
  // ----------------------------------------------------------
  it('TC-EDIT-005: 重发按钮为文字按钮且初始禁用', () => {
    const block = createUserBlock('测试');
    enterEditMode(block, '测试');

    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');
    expect(resendBtn).not.toBeNull();

    // 文字按钮（非纯图标）
    expect(resendBtn.textContent.trim().length).toBeGreaterThan(0);
    expect(resendBtn.querySelector('svg')).toBeNull();
    // 初始禁用
    expect(resendBtn.disabled).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-006: 取消和重发按钮尺寸一致
  // ----------------------------------------------------------
  it('TC-EDIT-006: 取消和重发按钮具有相同的 className 基础类', () => {
    const block = createUserBlock('测试');
    enterEditMode(block, '测试');

    const cancelBtn = block.parentElement.querySelector('.msg-edit-cancel');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    // 两者都应有 msg-edit-btn 基础类
    expect(cancelBtn.classList.contains('msg-edit-btn')).toBe(true);
    expect(resendBtn.classList.contains('msg-edit-btn')).toBe(true);

    // 两者都应有 msg-edit-btn 基础类（CSS 统一尺寸 32x32）
    const cancelStyle = window.getComputedStyle(cancelBtn);
    const resendStyle = window.getComputedStyle(resendBtn);

    // 由于 JSDOM 不计算 CSS，验证 class 一致性即可
    // msg-edit-btn 类在 CSS 中定义了 width: 32px; height: 32px
    expect(cancelBtn.className.split(' ')[0]).toBe(resendBtn.className.split(' ')[0]);
  });

  // ----------------------------------------------------------
  // TC-EDIT-007: 未修改时 Ctrl+Enter 不触发重发
  // ----------------------------------------------------------
  it('TC-EDIT-007: 未修改时 Ctrl+Enter 不触发 onResend 回调', () => {
    const block = createUserBlock('原始内容');
    let resendCalled = false;
    enterEditMode(block, '原始内容', () => { resendCalled = true; });

    const textarea = block.querySelector('.msg-edit-textarea');

    // 模拟 Ctrl+Enter（未修改内容）
    const event = new window.KeyboardEvent('keydown', {
      key: 'Enter',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    textarea.dispatchEvent(event);

    expect(resendCalled).toBe(false);
  });

  // ----------------------------------------------------------
  // TC-EDIT-008: 修改后 Ctrl+Enter 触发重发
  // ----------------------------------------------------------
  it('TC-EDIT-008: 修改后 Ctrl+Enter 触发 onResend 回调', () => {
    const block = createUserBlock('原始内容');
    let resendText = null;
    enterEditMode(block, '原始内容', (text) => { resendText = text; });

    const textarea = block.querySelector('.msg-edit-textarea');

    // 先修改内容
    simulateInput(textarea, '修改后的内容');

    // 模拟 Ctrl+Enter
    const event = new window.KeyboardEvent('keydown', {
      key: 'Enter',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    textarea.dispatchEvent(event);

    expect(resendText).toBe('修改后的内容');
  });

  // ----------------------------------------------------------
  // TC-EDIT-009: Escape 键取消编辑
  // ----------------------------------------------------------
  it('TC-EDIT-009: Escape 键退出编辑模式', () => {
    const block = createUserBlock('测试内容');
    enterEditMode(block, '测试内容');

    const textarea = block.querySelector('.msg-edit-textarea');
    expect(textarea).not.toBeNull();

    // 模拟 Escape
    const event = new window.KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    });
    textarea.dispatchEvent(event);

    // 编辑器应被移除
    expect(block.querySelector('.msg-edit-textarea')).toBeNull();
  });

  // ----------------------------------------------------------
  // TC-EDIT-010: 修改后恢复原文重新禁用重发按钮
  // ----------------------------------------------------------
  it('TC-EDIT-010: 修改后恢复原文，重发按钮重新禁用', () => {
    const block = createUserBlock('原始内容');
    enterEditMode(block, '原始内容');

    const textarea = block.querySelector('.msg-edit-textarea');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    // 修改内容 → 启用
    simulateInput(textarea, '临时修改');
    expect(resendBtn.disabled).toBe(false);

    // 恢复原文 → 禁用
    simulateInput(textarea, '原始内容');
    expect(resendBtn.disabled).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-011: 按钮栏有正确的间距和布局
  // ----------------------------------------------------------
  it('TC-EDIT-011: 操作按钮栏包含 msg-edit-actions-below 类', () => {
    const block = createUserBlock('测试');
    enterEditMode(block, '测试');

    const actionBar = block.parentElement.querySelector('.msg-edit-actions-below');
    expect(actionBar).not.toBeNull();

    // 参照编辑态设计截图：三个按钮 [📎 上传] [取消] [发送]
    const buttons = actionBar.querySelectorAll('button');
    expect(buttons.length).toBe(3);

    // 顺序：回形针 → 取消 → 发送
    expect(buttons[0].classList.contains('msg-edit-attach')).toBe(true);
    expect(buttons[1].classList.contains('msg-edit-cancel')).toBe(true);
    expect(buttons[2].classList.contains('msg-edit-resend')).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-012: 按钮具有 aria-label 属性
  // ----------------------------------------------------------
  it('TC-EDIT-012: 取消和重发按钮都有 aria-label', () => {
    const block = createUserBlock('测试');
    enterEditMode(block, '测试');

    const cancelBtn = block.parentElement.querySelector('.msg-edit-cancel');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    expect(cancelBtn.getAttribute('aria-label')).toBeTruthy();
    expect(resendBtn.getAttribute('aria-label')).toBeTruthy();
  });

  // ----------------------------------------------------------
  // TC-EDIT-013: 空内容时重发按钮禁用
  // ----------------------------------------------------------
  it('TC-EDIT-013: 空内容时重发按钮保持禁用', () => {
    const block = createUserBlock('');
    enterEditMode(block, '');

    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');
    expect(resendBtn.disabled).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-014: 仅空白字符时重发按钮禁用
  // ----------------------------------------------------------
  it('TC-EDIT-014: 仅空白字符时重发按钮禁用', () => {
    const block = createUserBlock('原始');
    enterEditMode(block, '原始');

    const textarea = block.querySelector('.msg-edit-textarea');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    // 修改为纯空白
    simulateInput(textarea, '   ');
    expect(resendBtn.disabled).toBe(true);
  });

  // ----------------------------------------------------------
  // TC-EDIT-015: Cmd+Enter 也触发重发（macOS 快捷键）
  // ----------------------------------------------------------
  it('TC-EDIT-015: 修改后 Cmd+Enter 触发重发', () => {
    const block = createUserBlock('原始');
    let resendText = null;
    enterEditMode(block, '原始', (text) => { resendText = text; });

    const textarea = block.querySelector('.msg-edit-textarea');
    simulateInput(textarea, '新内容');

    const event = new window.KeyboardEvent('keydown', {
      key: 'Enter',
      metaKey: true,
      bubbles: true,
      cancelable: true,
    });
    textarea.dispatchEvent(event);

    expect(resendText).toBe('新内容');
  });

  // ----------------------------------------------------------
  // TC-EDIT-016: 点击取消按钮退出编辑模式
  // ----------------------------------------------------------
  it('TC-EDIT-016: 点击取消按钮退出编辑模式', () => {
    const block = createUserBlock('测试内容');
    enterEditMode(block, '测试内容');

    const cancelBtn = block.parentElement.querySelector('.msg-edit-cancel');
    cancelBtn.click();

    // 编辑器应被移除
    expect(block.querySelector('.msg-edit-textarea')).toBeNull();

    // 原始内容应恢复显示
    const contentEl = block.querySelector('.msg-user-content');
    expect(contentEl.style.display).toBe('');
  });

  // ----------------------------------------------------------
  // TC-EDIT-017: 点击重发按钮（启用状态）触发回调
  // ----------------------------------------------------------
  it('TC-EDIT-017: 启用状态下点击重发按钮触发回调', () => {
    const block = createUserBlock('原始');
    let resendText = null;
    enterEditMode(block, '原始', (text) => { resendText = text; });

    const textarea = block.querySelector('.msg-edit-textarea');
    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');

    // 修改启用
    simulateInput(textarea, '新内容');
    expect(resendBtn.disabled).toBe(false);

    // 点击重发
    resendBtn.click();

    expect(resendText).toBe('新内容');
  });

  // ----------------------------------------------------------
  // TC-EDIT-018: 禁用状态下点击重发按钮不触发回调
  // ----------------------------------------------------------
  it('TC-EDIT-018: 禁用状态下点击重发按钮不触发回调', () => {
    const block = createUserBlock('原始');
    let resendCalled = false;
    enterEditMode(block, '原始', () => { resendCalled = true; });

    const resendBtn = block.parentElement.querySelector('.msg-edit-resend');
    expect(resendBtn.disabled).toBe(true);

    // 尝试点击（disabled 按钮不会触发 onclick）
    resendBtn.click();

    expect(resendCalled).toBe(false);
  });
});
