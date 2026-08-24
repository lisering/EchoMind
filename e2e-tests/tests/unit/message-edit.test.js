/**
 * EchoMind message-edit.js 单元测试 — 编辑 / 分支 / IME 防护。
 *
 * 验证点：
 * 1. enterEditMode 创建 textarea 编辑器
 * 2. enterEditMode null 参数安全返回
 * 3. enterEditMode 重复进入同一块不重复创建
 * 4. enterEditMode 初始发送按钮禁用（未修改时）
 * 5. exitEditMode 移除编辑器恢复原内容
 * 6. exitEditMode null 参数安全返回
 * 7. confirmEdit 空内容不允许发送
 * 8. confirmEdit 有效内容触发回调
 * 9. createEditButton 返回按钮元素
 * 10. removeBranchPagination 无分页器时安全返回
 * 11. renderBranchPagination 单版本不显示分页器
 * 12. navigateBranch 切换版本时调用 setActiveVersion
 *
 * Mock: i18n.js, ipc.js, state.js, markdown.js, ime-guard.js, turn-tree.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  convApi: {
    setTurnActiveVersion: vi.fn(() => Promise.resolve()),
    rename: vi.fn(() => Promise.resolve()),
    delete: vi.fn(() => Promise.resolve()),
    deleteMessage: vi.fn(() => Promise.resolve()),
  },
}));

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  get: (key) => {
    const map = { currentConversationId: 'conv-1', streaming: false };
    return map[key];
  },
  getState: () => ({ currentConversationId: 'conv-1', streaming: false }),
}));

// Mock markdown
vi.mock('../../../ui/src/markdown.js', () => ({
  renderMarkdown: vi.fn(),
  renderRichContent: vi.fn(() => Promise.resolve()),
}));

// Mock ime-guard
vi.mock('../../../ui/src/input-utils.js', () => ({
  createImeGuard: () => ({
    attach: vi.fn(),
    isComposing: () => false,
  }),
  isComposingEvent: vi.fn(() => false),
}));

// Mock turn-tree
const { _mockGetTurn, _mockGetVersionCount, _mockSetActiveVersion, _mockGetActiveVersion } = vi.hoisted(() => ({
  _mockGetTurn: vi.fn(() => ({
    turnGroup: 'tg-1',
    versions: [
      { version: 1, userContent: 'old text', assistantContent: 'answer1', sources: null, reasoning: null },
      { version: 2, userContent: 'new text', assistantContent: 'answer2', sources: null, reasoning: null },
    ],
    activeVersion: 2,
  })),
  _mockGetVersionCount: vi.fn(() => 2),
  _mockSetActiveVersion: vi.fn(() => true),
  _mockGetActiveVersion: vi.fn(() => ({ userContent: 'active text', assistantContent: 'active answer', sources: [], reasoning: '' })),
}));

vi.mock('../../../ui/src/turn-tree.js', () => ({
  getTurn: _mockGetTurn,
  getActiveVersion: _mockGetActiveVersion,
  getVersionCount: _mockGetVersionCount,
  setActiveVersion: _mockSetActiveVersion,
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div class="msg-block msg-user" id="userBlock" data-msg-id="msg-1" data-full-text="original text">
      <div class="msg-user-content">original text</div>
    </div>
    <div class="msg-block msg-assistant" id="assistantBlock" data-msg-id="msg-2">
      <div class="md" data-raw-markdown="answer"></div>
      <div class="sources"></div>
    </div>
  `;
}

import { enterEditMode, exitEditMode, confirmEdit, createEditButton, removeBranchPagination, renderBranchPagination, navigateBranch } from '../../../ui/src/message-edit.js';

describe('message-edit.js — 编辑模式', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
    _mockGetVersionCount.mockReturnValue(2);
    _mockGetTurn.mockReturnValue({
      turnGroup: 'tg-1',
      versions: [
        { version: 1, userContent: 'old text', assistantContent: 'answer1', sources: null, reasoning: null },
        { version: 2, userContent: 'new text', assistantContent: 'answer2', sources: null, reasoning: null },
      ],
      activeVersion: 2,
    });
    _mockSetActiveVersion.mockReturnValue(true);
  });

  it('enterEditMode null 参数安全返回 null', () => {
    const result = enterEditMode(null, 'text');
    expect(result).toBeNull();
  });

  it('enterEditMode 创建 textarea 编辑器', () => {
    const block = document.getElementById('userBlock');
    const result = enterEditMode(block, 'original text');
    expect(result).not.toBeNull();
    expect(result.tagName).toBe('TEXTAREA');
    expect(block.classList.contains('editing')).toBe(true);
    const content = block.querySelector('.msg-user-content');
    expect(content.style.display).toBe('none');
  });

  it('enterEditMode 初始发送按钮禁用（未修改时）', () => {
    const block = document.getElementById('userBlock');
    enterEditMode(block, 'original text');
    const resendBtn = document.querySelector('.msg-edit-resend');
    expect(resendBtn).not.toBeNull();
    expect(resendBtn.disabled).toBe(true);
  });

  it('enterEditMode 重复进入同一块不重复创建', () => {
    const block = document.getElementById('userBlock');
    enterEditMode(block, 'original text');
    const result = enterEditMode(block, 'original text');
    expect(result).toBeNull();
    const textareas = block.querySelectorAll('.msg-edit-full');
    expect(textareas).toHaveLength(1);
  });

  it('exitEditMode 移除编辑器恢复原内容', () => {
    const block = document.getElementById('userBlock');
    enterEditMode(block, 'original text');
    exitEditMode(block);
    expect(block.classList.contains('editing')).toBe(false);
    expect(block.querySelector('.msg-edit-full')).toBeNull();
    const content = block.querySelector('.msg-user-content');
    expect(content.style.display).toBe('');
  });

  it('exitEditMode null 参数安全返回', () => {
    expect(() => exitEditMode(null)).not.toThrow();
  });

  it('confirmEdit 空内容不允许发送', () => {
    const block = document.getElementById('userBlock');
    const callback = vi.fn();
    enterEditMode(block, 'original text');
    const textarea = block.querySelector('.msg-edit-full');
    textarea.value = '   ';
    confirmEdit(block, callback);
    expect(callback).not.toHaveBeenCalled();
  });

  it('confirmEdit 有效内容触发回调', () => {
    const block = document.getElementById('userBlock');
    const callback = vi.fn();
    enterEditMode(block, 'original text');
    const textarea = block.querySelector('.msg-edit-full');
    textarea.value = 'modified text';
    confirmEdit(block, callback);
    expect(callback).toHaveBeenCalledWith('modified text');
  });

  it('createEditButton 返回按钮元素', () => {
    const block = document.getElementById('userBlock');
    const btn = createEditButton(block, 'content', () => {});
    expect(btn.tagName).toBe('BUTTON');
    expect(btn.getAttribute('aria-label')).toBeDefined();
  });

  it('removeBranchPagination 无分页器时安全返回', () => {
    const block = document.getElementById('userBlock');
    expect(() => removeBranchPagination(block)).not.toThrow();
  });

  it('renderBranchPagination 单版本不显示分页器', () => {
    _mockGetVersionCount.mockReturnValueOnce(1);
    const block = document.getElementById('userBlock');
    const result = renderBranchPagination(block, 'tg-1');
    expect(result).toBeNull();
  });

  it('navigateBranch 切换版本时调用 setActiveVersion', () => {
    const block = document.getElementById('userBlock');
    navigateBranch(block, 'tg-1', -1);
    expect(_mockSetActiveVersion).toHaveBeenCalledWith('tg-1', 1);
  });
});
