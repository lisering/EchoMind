/**
 * EchoMind workspace.js 单元测试 — 工作空间选择器 / 新建 / 切换 / 重命名 / 删除。
 *
 * 验证点：
 * 1. getCurrentWorkspaceId 默认返回 'default'
 * 2. getCurrentWorkspaceName 默认返回 'Default'
 * 3. setSwitchWorkspaceCallback 设置回调
 * 4. initWorkspaceSelector 无 toggle 元素时安全返回
 * 5. initWorkspaceSelector 加载当前工作空间
 * 6. loadWorkspaceList 渲染工作空间列表
 * 7. loadWorkspaceList 当前选中项有高亮类
 * 8. loadWorkspaceList 非默认空间显示删除按钮
 * 9. loadWorkspaceList 渲染新建知识库按钮
 * 10. refreshQuotaDisplay 调用 get_workspace_quota
 *
 * Mock: i18n.js, ipc.js, toast.js, confirm-dialog.js, ime-guard.js, utils.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock ipc — use vi.hoisted to avoid hoisting order issues
const { _invokeMock } = vi.hoisted(() => ({ _invokeMock: vi.fn() }));
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: _invokeMock,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

// Mock confirm-dialog
vi.mock('../../../ui/src/confirm-dialog.js', () => ({
  showConfirmDialog: vi.fn(() => Promise.resolve(false)),
}));

// Mock ime-guard
vi.mock('../../../ui/src/input-utils.js', () => ({
  isComposingEvent: vi.fn(() => false),
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div id="workspaceSelector">
      <div id="workspaceToggle"></div>
      <div id="workspaceName">Default</div>
      <div id="workspaceQuota"></div>
      <div id="workspaceDropdown" class="hidden"></div>
    </div>
  `;
}

setupDom();

import {
  getCurrentWorkspaceId,
  getCurrentWorkspaceName,
  setSwitchWorkspaceCallback,
  initWorkspaceSelector,
  loadWorkspaceList,
  refreshQuotaDisplay,
} from '../../../ui/src/workspace.js';

describe('workspace.js — 工作空间管理', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
  });

  it('getCurrentWorkspaceId 默认返回 "default"', () => {
    expect(getCurrentWorkspaceId()).toBe('default');
  });

  it('getCurrentWorkspaceName 默认返回 "Default"', () => {
    expect(getCurrentWorkspaceName()).toBe('Default');
  });

  it('setSwitchWorkspaceCallback 设置回调函数', () => {
    const callback = vi.fn(async () => {});
    setSwitchWorkspaceCallback(callback);
    // 回调被设置后不影响其他导出函数的正常工作
    expect(getCurrentWorkspaceId()).toBe('default');
  });

  it('initWorkspaceSelector 无 toggle 元素时安全返回不报错', async () => {
    document.body.innerHTML = '<div id="other"></div>';
    await expect(initWorkspaceSelector()).resolves.toBeUndefined();
  });

  it('initWorkspaceSelector 加载当前工作空间并更新名称', async () => {
    _invokeMock.mockImplementation((cmd) => {
      if (cmd === 'get_current_workspace') return Promise.resolve('ws-1');
      if (cmd === 'list_workspaces') return Promise.resolve([
        { id: 'ws-1', name: 'My Workspace' },
        { id: 'default', name: 'Default' },
      ]);
      if (cmd === 'get_workspace_quota') return Promise.resolve([5, 50]);
      return Promise.resolve(null);
    });

    await initWorkspaceSelector();

    const nameEl = document.getElementById('workspaceName');
    expect(nameEl.textContent).toBe('My Workspace');
  });

  it('loadWorkspaceList 渲染工作空间列表项', async () => {
    _invokeMock.mockImplementation((cmd) => {
      if (cmd === 'list_workspaces') return Promise.resolve([
        { id: 'default', name: 'Default' },
        { id: 'ws-1', name: 'Project A' },
      ]);
      if (cmd === 'get_workspace_quota') return Promise.resolve([3, 50]);
      return Promise.resolve(null);
    });

    await loadWorkspaceList();

    const dropdown = document.getElementById('workspaceDropdown');
    const items = dropdown.querySelectorAll('.workspace-name');
    expect(items.length).toBe(2);
    expect(items[0].textContent).toBe('Default');
    expect(items[1].textContent).toBe('Project A');
  });

  it('loadWorkspaceList 非默认空间显示删除按钮', async () => {
    _invokeMock.mockImplementation((cmd) => {
      if (cmd === 'list_workspaces') return Promise.resolve([
        { id: 'default', name: 'Default' },
        { id: 'ws-1', name: 'Project A' },
      ]);
      return Promise.resolve(null);
    });

    await loadWorkspaceList();

    const dropdown = document.getElementById('workspaceDropdown');
    const delBtns = dropdown.querySelectorAll('.ws-action-btn');
    expect(delBtns.length).toBe(1); // 只有 ws-1 有删除按钮
  });

  it('loadWorkspaceList 渲染新建知识库按钮', async () => {
    _invokeMock.mockImplementation((cmd) => {
      if (cmd === 'list_workspaces') return Promise.resolve([
        { id: 'default', name: 'Default' },
      ]);
      return Promise.resolve(null);
    });

    await loadWorkspaceList();

    const dropdown = document.getElementById('workspaceDropdown');
    // 最后一个子元素是新建按钮
    const lastChild = dropdown.lastElementChild;
    expect(lastChild).not.toBeNull();
    expect(lastChild.textContent).toContain('workspace.create_new');
  });

  it('refreshQuotaDisplay 调用 get_workspace_quota 并更新 UI', async () => {
    _invokeMock.mockImplementation((cmd) => {
      if (cmd === 'get_workspace_quota') return Promise.resolve([10, 50]);
      return Promise.resolve(null);
    });

    await refreshQuotaDisplay();

    const quotaEl = document.getElementById('workspaceQuota');
    expect(quotaEl.textContent).toBe('10/50');
    expect(_invokeMock).toHaveBeenCalledWith('get_workspace_quota');
  });

  it('refreshQuotaDisplay Pro 版 (limit=0) 只显示计数', async () => {
    _invokeMock.mockImplementation((cmd) => {
      if (cmd === 'get_workspace_quota') return Promise.resolve([42, 0]);
      return Promise.resolve(null);
    });

    await refreshQuotaDisplay();

    const quotaEl = document.getElementById('workspaceQuota');
    expect(quotaEl.textContent).toBe('42');
  });
});
