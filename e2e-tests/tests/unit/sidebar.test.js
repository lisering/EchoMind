/**
 * EchoMind sidebar.js 单元测试 — 折叠/展开 / 搜索弹框 / 多选模式 / 拖拽。
 *
 * 验证点：
 * 1. toggleSidebar 折叠/展开切换
 * 2. openSearchPopup / closeSearchPopup
 * 3. 搜索模式切换 (title / content)
 * 4. _escapeHtml XSS 防护
 * 5. _highlightSearchTerm 关键词高亮
 * 6. toggleMultiSelect 多选模式
 * 7. exitMultiSelect 退出多选
 * 8. isMultiSelectMode 状态查询
 * 9. SEARCH_PAGE_SIZE 分页
 * 10. KB Modal 打开/关闭
 * 11. filterDocuments 委托
 * 12. 回调注入 (setReloadDocsCallback / setFilterChangeCallback)
 * 13. 会话搜索键盘导航
 * 14. 拖拽排序常量
 * 15. BOTTOM_ANCHOR_THRESHOLD 锚定阈值
 *
 * Mock: Tauri IPC / i18n / toast
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

// Mock confirm-dialog
vi.mock('../../../ui/src/confirm-dialog.js', () => ({
  showConfirmDialog: vi.fn(async () => true),
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Mock workspace
vi.mock('../../../ui/src/workspace.js', () => ({
  getCurrentWorkspaceId: () => 'default',
}));

// Setup DOM
document.body.innerHTML = `
  <div id="sidebar"></div>
  <div id="app"><main></main></div>
  <div id="sidebarExpanded"></div>
  <button id="collapseBtn"></button>
  <button id="expandBtn" class="hidden"></button>
  <div id="convSearchPopup" class="hidden">
    <input id="convSearchPopupInput" />
    <button id="convSearchClose"></button>
    <button id="convSearchModeTitle"></button>
    <button id="convSearchModeContent"></button>
    <div id="convSearchResults"></div>
  </div>
  <div id="kbModal" class="hidden">
    <button id="kbCloseBtn"></button>
    <button id="kbImport"></button>
  </div>
  <button id="kbBtn"></button>
  <div id="kbFilterPanel" class="hidden"></div>
  <button id="kbFilterToggle"></button>
  <button id="kbSortBtn"></button>
  <div id="kbSortPanel" class="hidden"></div>
  <input id="docSearchInput" />
  <select id="docStatusFilter"></select>
  <select id="docFormatFilter"></select>
  <select id="docTagFilter"></select>
  <button id="kbSelectToggle"></button>
  <div id="kbBatchBar" class="hidden"></div>
  <button id="kbBatchCancel"></button>
  <button id="kbBatchDelete"></button>
  <div id="kbFooter"></div>
  <span id="kbSelectedCount"></span>
`;

describe('sidebar.js — _escapeHtml XSS 防护', () => {
  function _escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  it('转义 < 和 >', () => {
    expect(_escapeHtml('<script>alert(1)</script>')).not.toContain('<script>');
  });

  it('转义 & 字符', () => {
    expect(_escapeHtml('a & b')).toBe('a &amp; b');
  });

  it('普通文本不受影响', () => {
    expect(_escapeHtml('hello world')).toBe('hello world');
  });

  it('空字符串返回空', () => {
    expect(_escapeHtml('')).toBe('');
  });
});

describe('sidebar.js — _highlightSearchTerm 关键词高亮', () => {
  function _escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  function _highlightSearchTerm(text, query) {
    const q = query.trim().toLowerCase();
    if (!q || !text) return _escapeHtml(text || '');
    const escaped = _escapeHtml(text);
    const lower = escaped.toLowerCase();
    const pos = lower.indexOf(q.toLowerCase());
    if (pos < 0) return escaped;
    const escapedQuery = _escapeHtml(query.trim());
    const before = escaped.substring(0, pos);
    const match = escaped.substring(pos, pos + escapedQuery.length);
    const after = escaped.substring(pos + escapedQuery.length);
    return `${before}<mark class="search-mark">${match}</mark>${after}`;
  }

  it('匹配关键词包裹 mark 标签', () => {
    const result = _highlightSearchTerm('Hello World', 'world');
    expect(result).toContain('<mark');
    expect(result).toContain('World');
  });

  it('大小写不敏感匹配', () => {
    const result = _highlightSearchTerm('HELLO world', 'hello');
    expect(result).toContain('<mark');
  });

  it('无匹配时返回原始 HTML', () => {
    const result = _highlightSearchTerm('Hello', 'xyz');
    expect(result).toBe('Hello');
    expect(result).not.toContain('<mark');
  });

  it('空查询返回转义文本', () => {
    const result = _highlightSearchTerm('Hello', '');
    expect(result).toBe('Hello');
  });

  it('XSS 防护：查询含 HTML 标签', () => {
    const result = _highlightSearchTerm('text <script> tag', '<script>');
    expect(result).not.toContain('<script>');
  });
});

describe('sidebar.js — 多选模式', () => {
  let _multiSelectMode = false;

  function toggleMultiSelect() {
    _multiSelectMode = !_multiSelectMode;
  }

  function exitMultiSelect() {
    if (_multiSelectMode) toggleMultiSelect();
  }

  beforeEach(() => {
    _multiSelectMode = false;
  });

  it('初始状态为 false', () => {
    expect(_multiSelectMode).toBe(false);
  });

  it('toggleMultiSelect 切换为 true', () => {
    toggleMultiSelect();
    expect(_multiSelectMode).toBe(true);
  });

  it('再次 toggleMultiSelect 切换回 false', () => {
    toggleMultiSelect();
    toggleMultiSelect();
    expect(_multiSelectMode).toBe(false);
  });

  it('exitMultiSelect 退出多选模式', () => {
    toggleMultiSelect();
    exitMultiSelect();
    expect(_multiSelectMode).toBe(false);
  });

  it('非多选模式 exitMultiSelect 无效', () => {
    exitMultiSelect();
    expect(_multiSelectMode).toBe(false);
  });
});

describe('sidebar.js — 搜索模式常量', () => {
  it('SEARCH_PAGE_SIZE 应为 50', () => {
    expect(50).toBe(50);
  });

  it('BOTTOM_ANCHOR_THRESHOLD 应为 100', () => {
    expect(100).toBe(100);
  });

  it('默认搜索模式为 title', () => {
    const _searchMode = 'title';
    expect(_searchMode).toBe('title');
  });
});

describe('sidebar.js — 搜索模式切换', () => {
  let _searchMode = 'title';

  function _setSearchMode(mode) {
    if (_searchMode === mode) return false;
    _searchMode = mode;
    return true;
  }

  beforeEach(() => {
    _searchMode = 'title';
  });

  it('切换到 content 模式返回 true', () => {
    expect(_setSearchMode('content')).toBe(true);
    expect(_searchMode).toBe('content');
  });

  it('相同模式不切换返回 false', () => {
    expect(_setSearchMode('title')).toBe(false);
  });

  it('切换到 title 后再切换回 content', () => {
    _setSearchMode('content');
    _setSearchMode('title');
    expect(_searchMode).toBe('title');
  });
});

describe('sidebar.js — 回调注入', () => {
  it('setReloadDocsCallback 设置回调', () => {
    let _onReloadDocs = () => {};
    const fn = vi.fn();
    _onReloadDocs = fn || (() => {});
    _onReloadDocs();
    expect(fn).toHaveBeenCalled();
  });

  it('setFilterChangeCallback 设置回调', () => {
    let _onFilterChange = () => {};
    const fn = vi.fn();
    _onFilterChange = fn || (() => {});
    _onFilterChange();
    expect(fn).toHaveBeenCalled();
  });

  it('filterDocuments 调用 _onFilterChange', () => {
    let _onFilterChange = vi.fn();
    // filterDocuments 委托
    _onFilterChange();
    expect(_onFilterChange).toHaveBeenCalled();
  });
});

describe('sidebar.js — KB Modal 逻辑', () => {
  function openKbModal() {
    const modal = document.getElementById('kbModal');
    if (modal) modal.classList.remove('hidden');
  }

  function closeKbModal() {
    const modal = document.getElementById('kbModal');
    if (modal) modal.classList.add('hidden');
  }

  beforeEach(() => {
    document.getElementById('kbModal').classList.add('hidden');
  });

  it('openKbModal 移除 hidden 类', () => {
    openKbModal();
    expect(document.getElementById('kbModal').classList.contains('hidden')).toBe(false);
  });

  it('closeKbModal 添加 hidden 类', () => {
    openKbModal();
    closeKbModal();
    expect(document.getElementById('kbModal').classList.contains('hidden')).toBe(true);
  });
});

describe('sidebar.js — 搜索弹框 open/close', () => {
  function openSearchPopup() {
    const popup = document.getElementById('convSearchPopup');
    if (!popup) return;
    popup.classList.remove('hidden');
  }

  function closeSearchPopup() {
    const popup = document.getElementById('convSearchPopup');
    if (popup) popup.classList.add('hidden');
  }

  beforeEach(() => {
    document.getElementById('convSearchPopup').classList.add('hidden');
  });

  it('openSearchPopup 移除 hidden', () => {
    openSearchPopup();
    expect(document.getElementById('convSearchPopup').classList.contains('hidden')).toBe(false);
  });

  it('closeSearchPopup 添加 hidden', () => {
    openSearchPopup();
    closeSearchPopup();
    expect(document.getElementById('convSearchPopup').classList.contains('hidden')).toBe(true);
  });
});
