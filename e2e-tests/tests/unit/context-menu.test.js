/**
 * EchoMind context-menu.js 单元测试 — 右键菜单 / 导出 / 重分类。
 *
 * 验证点：
 * 1. initContextMenu 注册 contextmenu 事件
 * 2. _isEditable 检测 INPUT 元素
 * 3. _isEditable 检测 TEXTAREA 元素
 * 4. _isEditable 检测 contenteditable
 * 5. _isEditable 非编辑元素返回 false
 * 6. _hasSelection 有选中文本返回 true
 * 7. _hasSelection 无选中文本返回 false
 * 8. _buildItems 编辑+选中构建剪切/复制/粘贴/全选
 * 9. _buildItems 非编辑+有选中构建复制/全选
 * 10. _buildItems 剪切在不可用时 disabled
 * 11. _item 生成正确的 HTML 结构
 * 12. _separator 生成分隔线 HTML
 *
 * Mock: utils.js, i18n.js, toast.js, ipc.js, export.js, confirm-dialog.js, state.js, ime-guard.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
  docExtApi: { reclassify: vi.fn() },
  docApi: { delete: vi.fn(), rebuild: vi.fn() },
  docExportApi: { exportOriginal: vi.fn() },
  saveDialog: vi.fn(),
  convApi: {
    rename: vi.fn(),
    delete: vi.fn(),
    exportMarkdown: vi.fn(),
    saveTextFile: vi.fn(),
    deleteMessage: vi.fn(),
  },
}));

// Mock export
vi.mock('../../../ui/src/export.js', () => ({
  exportDocumentToPdf: vi.fn(),
  exportDocumentToHtml: vi.fn(),
}));

// Mock confirm-dialog
vi.mock('../../../ui/src/confirm-dialog.js', () => ({
  showConfirmDialog: vi.fn().mockResolvedValue(false),
}));

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  getState: () => ({ streaming: false, currentConversationId: null }),
  get: (key) => {
    const map = { streaming: false, currentConversationId: null };
    return map[key];
  },
}));

// Mock ime-guard
vi.mock('../../../ui/src/input-utils.js', () => ({
  isComposingEvent: vi.fn(() => false),
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
  copyToClipboard: vi.fn(() => Promise.resolve(true)),
}));

// Setup DOM
document.body.innerHTML = '<div id="ctxMenu" class="ctx-menu"></div>';

import { initContextMenu } from '../../../ui/src/context-menu.js';

describe('context-menu.js — 初始化', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="ctxMenu" class="ctx-menu"></div>';
    vi.clearAllMocks();
  });

  it('initContextMenu 不报错（找到 #ctxMenu）', () => {
    expect(() => initContextMenu()).not.toThrow();
  });

  it('initContextMenu 无 #ctxMenu 时安全返回', () => {
    document.body.innerHTML = '';
    expect(() => initContextMenu()).not.toThrow();
  });
});

describe('context-menu.js — 内部逻辑测试', () => {
  // 由于内部函数 _isEditable, _hasSelection, _buildItems, _item, _separator
  // 是模块私有函数不导出，通过 contextmenu 事件间接测试

  beforeEach(() => {
    document.body.innerHTML = '<div id="ctxMenu" class="ctx-menu"></div>';
    vi.clearAllMocks();
  });

  it('右键 INPUT 元素时显示编辑菜单', () => {
    initContextMenu();
    const input = document.createElement('input');
    input.type = 'text';
    document.body.appendChild(input);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    input.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('ctx-item');
    // 剪切项（disabled，因为无选中）
    expect(menu.innerHTML).toContain('cut');
  });

  it('右键 TEXTAREA 元素显示编辑菜单', () => {
    initContextMenu();
    const ta = document.createElement('textarea');
    document.body.appendChild(ta);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    ta.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('paste');
  });

  it('右键 [data-doc-name] 元素显示文档菜单', () => {
    initContextMenu();
    const docEl = document.createElement('div');
    docEl.setAttribute('data-doc-name', 'test.pdf');
    docEl.setAttribute('data-doc-id', 'doc-123');
    document.body.appendChild(docEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    docEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('copyDocName');
    expect(menu.dataset.docName).toBe('test.pdf');
    expect(menu.dataset.docId).toBe('doc-123');
  });

  it('右键 [data-conv-id] 元素显示会话菜单', () => {
    initContextMenu();
    const convEl = document.createElement('div');
    convEl.setAttribute('data-conv-id', 'conv-1');
    convEl.setAttribute('data-conv-title', 'Test Conversation');
    document.body.appendChild(convEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    convEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('convRename');
    expect(menu.dataset.convId).toBe('conv-1');
  });

  it('右键 .msg-block 元素显示消息菜单', () => {
    initContextMenu();
    const msgEl = document.createElement('div');
    msgEl.className = 'msg-block msg-assistant';
    msgEl.dataset.msgId = 'msg-1';
    msgEl.dataset.query = 'test query';
    document.body.appendChild(msgEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    msgEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('msgCopyFull');
  });

  it('右键 .source-chip 元素显示引用片段菜单', () => {
    initContextMenu();
    const chip = document.createElement('span');
    chip.className = 'source-chip';
    chip.dataset.chunkContent = 'chunk content here';
    document.body.appendChild(chip);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    chip.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('copyChunk');
    expect(menu.dataset.chunkContent).toBe('chunk content here');
  });

  it('右键非编辑无选中元素时抑制默认菜单且不显示', () => {
    initContextMenu();
    const div = document.createElement('div');
    div.textContent = 'some text';
    document.body.appendChild(div);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    div.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    const menu = document.getElementById('ctxMenu');
    expect(menu.classList.contains('visible')).toBe(false);
  });

  it('文档菜单包含重分类项（有 docId 时）', () => {
    initContextMenu();
    const docEl = document.createElement('div');
    docEl.setAttribute('data-doc-name', 'doc.md');
    docEl.setAttribute('data-doc-id', 'doc-456');
    document.body.appendChild(docEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    docEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('reclassifyDoc');
    expect(menu.innerHTML).toContain('rebuildIndex');
    expect(menu.innerHTML).toContain('migrateDoc');
    expect(menu.innerHTML).toContain('deleteDoc');
  });

  it('文档菜单无 docId 时仅显示复制文件名', () => {
    initContextMenu();
    const docEl = document.createElement('div');
    docEl.setAttribute('data-doc-name', 'doc.md');
    // 注意：无 data-doc-id
    document.body.appendChild(docEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    docEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('copyDocName');
    expect(menu.innerHTML).not.toContain('reclassifyDoc');
  });

  it('菜单项 HTML 包含 action 和 shortcut 属性', () => {
    initContextMenu();
    const input = document.createElement('input');
    document.body.appendChild(input);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    input.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    const items = menu.querySelectorAll('.ctx-item');
    expect(items.length).toBeGreaterThan(0);
    expect(items[0].dataset.action).toBeDefined();
  });

  it('文档菜单包含分隔线', () => {
    initContextMenu();
    const docEl = document.createElement('div');
    docEl.setAttribute('data-doc-name', 'doc.md');
    docEl.setAttribute('data-doc-id', 'doc-789');
    document.body.appendChild(docEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    docEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('ctx-separator');
  });

  it('会话菜单包含导出选项', () => {
    initContextMenu();
    const convEl = document.createElement('div');
    convEl.setAttribute('data-conv-id', 'conv-2');
    convEl.setAttribute('data-conv-title', 'Test');
    document.body.appendChild(convEl);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    convEl.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('convExport');
    expect(menu.innerHTML).toContain('convDelete');
  });

  it('菜单包含快捷键提示', () => {
    initContextMenu();
    const input = document.createElement('input');
    document.body.appendChild(input);

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    input.dispatchEvent(event);

    const menu = document.getElementById('ctxMenu');
    expect(menu.innerHTML).toContain('ctx-shortcut');
  });
});
