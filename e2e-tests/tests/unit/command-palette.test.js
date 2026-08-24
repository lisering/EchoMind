/**
 * EchoMind command-palette.js 单元测试 — 命令面板 / 搜索 / 执行。
 *
 * 验证点：
 * 1. filterCommands 空查询返回全部命令
 * 2. filterCommands 标签匹配
 * 3. filterCommands 分组名匹配
 * 4. filterCommands 无匹配返回空数组
 * 5. highlightMatch 匹配部分包裹 span
 * 6. highlightMatch 无匹配返回原标签
 * 7. highlightMatch 空查询返回原标签
 * 8. renderCommandList 渲染分组标题
 * 9. renderCommandList 空结果渲染提示
 * 10. openCommandPalette 打开面板
 *
 * Mock: state.js, utils.js, i18n.js, panel-stack.js, focus-trap.js, icons.js, ime-guard.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock state
const _state = { cmdFiltered: [], cmdSelectedIndex: 0 };
vi.mock('../../../ui/src/state.js', () => ({
  getState: () => ({ ..._state }),
  setState: (partial) => { Object.assign(_state, partial); return _state; },
  get: (key) => _state[key],
}));

// Mock utils (merged with icons)
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
  icon: vi.fn(() => ''),
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Mock focus-trap
vi.mock('../../../ui/src/focus-trap.js', () => ({
  createFocusTrap: vi.fn(() => ({
    activate: vi.fn(),
    deactivate: vi.fn(),
  })),
}));


// Mock ime-guard
vi.mock('../../../ui/src/input-utils.js', () => ({
  isComposingEvent: vi.fn(() => false),
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div id="commandPalette" class="hidden">
      <div class="scale-in">
        <input id="cmdSearch" />
        <div id="cmdList"></div>
        <button id="cmdPaletteClose"></button>
      </div>
    </div>
  `;
}

setupDom();

// jsdom doesn't implement scrollIntoView
Element.prototype.scrollIntoView = vi.fn();

import { filterCommands, highlightMatch, renderCommandList, openCommandPalette, closeCommandPalette, updateCmdSelection } from '../../../ui/src/search-ui.js';

const sampleCommands = [
  { group: 'Chat', icon: '💬', label: 'New Chat', shortcut: '⌘N', action: vi.fn() },
  { group: 'Chat', icon: '📤', label: 'Export', shortcut: '⌘E', action: vi.fn() },
  { group: 'Settings', icon: '⚙', label: 'Open Settings', shortcut: '⌘,', action: vi.fn() },
  { group: 'Help', icon: '?', label: 'Keyboard Shortcuts', shortcut: '⌘/', action: vi.fn() },
];

describe('command-palette.js — 搜索过滤', () => {
  it('filterCommands 空查询返回全部命令', () => {
    const result = filterCommands('', sampleCommands);
    expect(result).toHaveLength(4);
  });

  it('filterCommands 标签匹配', () => {
    const result = filterCommands('export', sampleCommands);
    expect(result).toHaveLength(1);
    expect(result[0].label).toBe('Export');
  });

  it('filterCommands 分组名匹配', () => {
    const result = filterCommands('settings', sampleCommands);
    expect(result).toHaveLength(1);
    expect(result[0].group).toBe('Settings');
  });

  it('filterCommands 无匹配返回空数组', () => {
    const result = filterCommands('nonexistent', sampleCommands);
    expect(result).toHaveLength(0);
  });
});

describe('command-palette.js — 高亮匹配', () => {
  it('highlightMatch 匹配部分包裹 span', () => {
    const result = highlightMatch('New Chat', 'chat');
    expect(result).toContain('<span class="text-accent font-medium">Chat</span>');
  });

  it('highlightMatch 无匹配返回原标签', () => {
    const result = highlightMatch('New Chat', 'xyz');
    expect(result).toBe('New Chat');
  });

  it('highlightMatch 空查询返回原标签', () => {
    const result = highlightMatch('New Chat', '');
    expect(result).toBe('New Chat');
  });
});

describe('command-palette.js — DOM 渲染', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
    _state.cmdFiltered = [];
    _state.cmdSelectedIndex = 0;
  });

  it('renderCommandList 渲染分组标题', () => {
    renderCommandList(sampleCommands, '');
    const list = document.getElementById('cmdList');
    expect(list.children.length).toBeGreaterThan(0);
    // 应包含分组标题
    const headers = list.querySelectorAll('.text-\\[11px\\]');
    expect(headers.length).toBeGreaterThan(0);
  });

  it('renderCommandList 空结果渲染无结果提示', () => {
    renderCommandList(sampleCommands, 'nonexistent');
    const list = document.getElementById('cmdList');
    expect(list.innerHTML).toContain('command_palette.no_results');
  });

  it('openCommandPalette 打开面板（移除 hidden 类）', () => {
    const palette = document.getElementById('commandPalette');
    expect(palette.classList.contains('hidden')).toBe(true);
    openCommandPalette();
    expect(palette.classList.contains('hidden')).toBe(false);
  });

  it('closeCommandPalette 关闭面板（添加 hidden 类）', () => {
    openCommandPalette();
    closeCommandPalette();
    const palette = document.getElementById('commandPalette');
    expect(palette.classList.contains('hidden')).toBe(true);
  });
});

describe('command-palette.js — 键盘导航', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
    _state.cmdFiltered = [];
    _state.cmdSelectedIndex = 0;
  });

  it('openCommandPalette 打开时清空搜索框', () => {
    const searchInput = document.getElementById('cmdSearch');
    searchInput.value = 'old text';
    openCommandPalette();
    expect(searchInput.value).toBe('');
  });

  it('openCommandPalette 已打开时切换关闭', () => {
    openCommandPalette();
    const palette = document.getElementById('commandPalette');
    expect(palette.classList.contains('hidden')).toBe(false);
    openCommandPalette(); // toggle close
    expect(palette.classList.contains('hidden')).toBe(true);
  });

  it('closeCommandPalette 调用 removePanel', async () => {
    const { removePanel } = await import('../../../ui/src/panel-stack.js');
    closeCommandPalette();
    expect(removePanel).toHaveBeenCalledWith('command-palette');
  });

  it('renderCommandList 设置 role=option 属性', () => {
    renderCommandList(sampleCommands, '');
    const items = document.querySelectorAll('[role="option"]');
    expect(items.length).toBe(sampleCommands.length);
  });

  it('renderCommandList 选中项有 bg-accent 类', () => {
    renderCommandList(sampleCommands, '');
    const items = document.querySelectorAll('[role="option"]');
    // After renderCommandList, setState resets cmdSelectedIndex to 0
    // So items[0] is the selected one (idx=0 === selectedIdx=0)
    expect(items[0].className).toContain('bg-accent');
  });

  it('renderCommandList 点击命令触发 action', () => {
    renderCommandList(sampleCommands, '');
    const items = document.querySelectorAll('[role="option"]');
    items[0].click();
    expect(sampleCommands[0].action).toHaveBeenCalled();
  });

  it('renderCommandList 无快捷键的命令不渲染 kbd', () => {
    const cmdsNoShortcut = [{ group: 'Test', icon: 'T', label: 'No Shortcut', action: vi.fn() }];
    renderCommandList(cmdsNoShortcut, '');
    const items = document.querySelectorAll('[role="option"]');
    expect(items[0].innerHTML).not.toContain('<kbd');
  });

  it('renderCommandList 有快捷键的命令渲染 kbd', () => {
    renderCommandList(sampleCommands, '');
    const items = document.querySelectorAll('[role="option"]');
    expect(items[0].innerHTML).toContain('<kbd');
    expect(items[0].innerHTML).toContain('⌘N');
  });

  it('filterCommands 大小写不敏感', () => {
    // 'CHAT' matches both 'New Chat' label and 'Chat' group
    const result = filterCommands('CHAT', sampleCommands);
    expect(result.length).toBeGreaterThanOrEqual(1);
    expect(result.some((r) => r.label === 'New Chat')).toBe(true);
  });

  it('filterCommands 空格 trim', () => {
    const result = filterCommands('  export  ', sampleCommands);
    expect(result).toHaveLength(1);
    expect(result[0].label).toBe('Export');
  });

  it('highlightMatch 多次匹配只高亮第一次', () => {
    const result = highlightMatch('Chat Chat', 'chat');
    const spanCount = (result.match(/<span/g) || []).length;
    expect(spanCount).toBe(1);
  });

  it('updateCmdSelection 更新选中类名', () => {
    renderCommandList(sampleCommands, '');
    _state.cmdSelectedIndex = 1;
    updateCmdSelection();
    const items = document.querySelectorAll('[role="option"]');
    expect(items[1].className).toContain('bg-accent');
    expect(items[0].className).not.toContain('bg-accent');
  });

  it('renderCommandList 渲染图标字符', () => {
    renderCommandList(sampleCommands, '');
    const items = document.querySelectorAll('[role="option"]');
    expect(items[0].innerHTML).toContain('💬');
  });

  it('openCommandPalette 调用 pushPanel', async () => {
    const { pushPanel } = await import('../../../ui/src/panel-stack.js');
    openCommandPalette();
    expect(pushPanel).toHaveBeenCalledWith(expect.objectContaining({ id: 'command-palette' }));
  });
});
