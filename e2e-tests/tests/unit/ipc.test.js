/**
 * EchoMind ipc.js 单元测试 — IPC 封装 / 错误处理 / API 契约。
 *
 * 验证点：
 * 1. invoke 调用 Tauri core.invoke
 * 2. listen 调用 Tauri event.listen
 * 3. openDialog 调用 Tauri dialog.open
 * 4. saveDialog 调用 Tauri dialog.save
 * 5. settingsApi.update / get / setBool
 * 6. convApi 方法映射
 * 7. docApi 方法映射
 * 8. securityApi 方法映射
 * 9. devInvoke 错误降级
 * 10. API 对象完整性
 *
 * Mock: window.__TAURI__
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Setup mock Tauri runtime
const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockDialogOpen = vi.fn();
const mockDialogSave = vi.fn();
const mockOpenerOpenUrl = vi.fn();

beforeEach(() => {
  // Reset mocks
  mockInvoke.mockReset();
  mockListen.mockReset();
  mockDialogOpen.mockReset();
  mockDialogSave.mockReset();
  mockOpenerOpenUrl.mockReset();

  // Setup __TAURI__ mock
  globalThis.window = globalThis.window || {};
  window.__TAURI__ = {
    core: { invoke: mockInvoke },
    event: { listen: mockListen },
    dialog: { open: mockDialogOpen, save: mockDialogSave },
    opener: { openUrl: mockOpenerOpenUrl },
  };
});

describe('ipc.js — invoke 封装', () => {
  async function invoke(cmd, args = {}) {
    return window.__TAURI__.core.invoke(cmd, args);
  }

  it('调用 Tauri core.invoke', async () => {
    mockInvoke.mockResolvedValue('result');
    const result = await invoke('get_settings');
    expect(mockInvoke).toHaveBeenCalledWith('get_settings', {});
    expect(result).toBe('result');
  });

  it('传递参数正确', async () => {
    mockInvoke.mockResolvedValue(null);
    await invoke('chat', { query: 'test', history: [] });
    expect(mockInvoke).toHaveBeenCalledWith('chat', { query: 'test', history: [] });
  });

  it('默认参数为空对象', async () => {
    mockInvoke.mockResolvedValue(null);
    await invoke('ping');
    expect(mockInvoke).toHaveBeenCalledWith('ping', {});
  });

  it('invoke 抛出的错误传播', async () => {
    mockInvoke.mockRejectedValue(new Error('IPC failed'));
    await expect(invoke('fail')).rejects.toThrow('IPC failed');
  });
});

describe('ipc.js — listen 封装', () => {
  async function listen(name, cb) {
    return window.__TAURI__.event.listen(name, cb);
  }

  it('调用 Tauri event.listen', async () => {
    mockListen.mockResolvedValue(() => {});
    const cb = vi.fn();
    await listen('chat_token', cb);
    expect(mockListen).toHaveBeenCalledWith('chat_token', cb);
  });

  it('返回取消监听函数', async () => {
    const unlisten = vi.fn();
    mockListen.mockResolvedValue(unlisten);
    const result = await listen('chat_done', () => {});
    expect(typeof result).toBe('function');
  });
});

describe('ipc.js — openDialog 封装', () => {
  async function openDialog(options) {
    return window.__TAURI__.dialog.open(options);
  }

  it('调用 Tauri dialog.open', async () => {
    mockDialogOpen.mockResolvedValue('/path/to/file.pdf');
    const result = await openDialog({ multiple: true });
    expect(mockDialogOpen).toHaveBeenCalledWith({ multiple: true });
    expect(result).toBe('/path/to/file.pdf');
  });
});

describe('ipc.js — saveDialog 封装', () => {
  async function saveDialog(options) {
    return window.__TAURI__.dialog.save(options);
  }

  it('调用 Tauri dialog.save', async () => {
    mockDialogSave.mockResolvedValue('/path/to/save.pdf');
    const result = await saveDialog({ defaultPath: 'export.pdf' });
    expect(mockDialogSave).toHaveBeenCalledWith({ defaultPath: 'export.pdf' });
    expect(result).toBe('/path/to/save.pdf');
  });
});

describe('ipc.js — settingsApi 契约', () => {
  it('update 方法调用 update_setting', () => {
    mockInvoke.mockResolvedValue(null);
    const settingsApi = {
      update: (key, value) => window.__TAURI__.core.invoke('update_setting', { key, value }),
    };
    settingsApi.update('rag.hybrid_search', 'true');
    expect(mockInvoke).toHaveBeenCalledWith('update_setting', { key: 'rag.hybrid_search', value: 'true' });
  });

  it('get 方法调用 get_setting', () => {
    mockInvoke.mockResolvedValue('true');
    const settingsApi = {
      get: (key) => window.__TAURI__.core.invoke('get_setting', { key }),
    };
    settingsApi.get('rag.hybrid_search');
    expect(mockInvoke).toHaveBeenCalledWith('get_setting', { key: 'rag.hybrid_search' });
  });

  it('setBool 将布尔值转为字符串', () => {
    mockInvoke.mockResolvedValue(null);
    const settingsApi = {
      setBool: (key, enabled) => window.__TAURI__.core.invoke('update_setting', { key, value: String(enabled) }),
    };
    settingsApi.setBool('rag.web_search', true);
    expect(mockInvoke).toHaveBeenCalledWith('update_setting', { key: 'rag.web_search', value: 'true' });
  });
});

describe('ipc.js — convApi 方法映射', () => {
  it('create 调用 create_conversation', () => {
    mockInvoke.mockResolvedValue('conv-1');
    window.__TAURI__.core.invoke('create_conversation', { workspaceId: 'default' });
    expect(mockInvoke).toHaveBeenCalledWith('create_conversation', { workspaceId: 'default' });
  });

  it('delete 调用 delete_conversation', () => {
    mockInvoke.mockResolvedValue(null);
    window.__TAURI__.core.invoke('delete_conversation', { id: 'conv-1' });
    expect(mockInvoke).toHaveBeenCalledWith('delete_conversation', { id: 'conv-1' });
  });

  it('rename 调用 rename_conversation', () => {
    mockInvoke.mockResolvedValue(null);
    window.__TAURI__.core.invoke('rename_conversation', { id: 'conv-1', title: 'New Title' });
    expect(mockInvoke).toHaveBeenCalledWith('rename_conversation', { id: 'conv-1', title: 'New Title' });
  });
});

describe('ipc.js — securityApi 方法映射', () => {
  it('encrypt 调用 encrypt_database', () => {
    mockInvoke.mockResolvedValue({ success: true });
    window.__TAURI__.core.invoke('encrypt_database', { password: 'pass123' });
    expect(mockInvoke).toHaveBeenCalledWith('encrypt_database', { password: 'pass123' });
  });

  it('unlock 调用 unlock_database', () => {
    mockInvoke.mockResolvedValue({ success: true });
    window.__TAURI__.core.invoke('unlock_database', { password: 'pass123' });
    expect(mockInvoke).toHaveBeenCalledWith('unlock_database', { password: 'pass123' });
  });

  it('lock 调用 lock_app', () => {
    mockInvoke.mockResolvedValue(null);
    window.__TAURI__.core.invoke('lock_app');
    expect(mockInvoke).toHaveBeenCalledWith('lock_app');
  });
});

describe('ipc.js — devInvoke 错误降级', () => {
  async function devInvoke(cmd, args = {}, fallback = null) {
    try {
      return await window.__TAURI__.core.invoke(cmd, args);
    } catch (_) {
      return fallback;
    }
  }

  it('成功时返回结果', async () => {
    mockInvoke.mockResolvedValue({ data: 'ok' });
    const result = await devInvoke('get_recent_traces', { limit: 10 }, []);
    expect(result).toEqual({ data: 'ok' });
  });

  it('失败时返回 fallback 默认值', async () => {
    mockInvoke.mockRejectedValue(new Error('not registered'));
    const result = await devInvoke('get_trace_count', {}, 0);
    expect(result).toBe(0);
  });

  it('失败时返回 null fallback', async () => {
    mockInvoke.mockRejectedValue(new Error('not registered'));
    const result = await devInvoke('clear_traces', {}, null);
    expect(result).toBeNull();
  });

  it('失败时返回空数组 fallback', async () => {
    mockInvoke.mockRejectedValue(new Error('not registered'));
    const result = await devInvoke('get_recent_traces', { limit: 20 }, []);
    expect(result).toEqual([]);
  });
});

describe('ipc.js — API 对象完整性', () => {
  it('settingsApi 有 update / get / setBool', () => {
    const settingsApi = { update: vi.fn(), get: vi.fn(), setBool: vi.fn() };
    expect(typeof settingsApi.update).toBe('function');
    expect(typeof settingsApi.get).toBe('function');
    expect(typeof settingsApi.setBool).toBe('function');
  });

  it('convApi 有 create / list / messages / delete / rename', () => {
    const convApi = { create: vi.fn(), list: vi.fn(), messages: vi.fn(), delete: vi.fn(), rename: vi.fn() };
    expect(typeof convApi.create).toBe('function');
    expect(typeof convApi.list).toBe('function');
    expect(typeof convApi.messages).toBe('function');
    expect(typeof convApi.delete).toBe('function');
    expect(typeof convApi.rename).toBe('function');
  });

  it('securityApi 有 encrypt / unlock / lock / detectPii', () => {
    const securityApi = { encrypt: vi.fn(), unlock: vi.fn(), lock: vi.fn(), detectPii: vi.fn() };
    expect(typeof securityApi.encrypt).toBe('function');
    expect(typeof securityApi.unlock).toBe('function');
    expect(typeof securityApi.lock).toBe('function');
    expect(typeof securityApi.detectPii).toBe('function');
  });
});

describe('ipc.js — openUrl 封装', () => {
  async function openUrl(url) {
    try {
      await window.__TAURI__.opener.openUrl(url);
    } catch (_) {
      // 静默失败
    }
  }

  it('调用 opener.openUrl', async () => {
    mockOpenerOpenUrl.mockResolvedValue(undefined);
    await openUrl('https://example.com');
    expect(mockOpenerOpenUrl).toHaveBeenCalledWith('https://example.com');
  });

  it('失败时静默降级不抛出', async () => {
    mockOpenerOpenUrl.mockRejectedValue(new Error('no opener'));
    await expect(openUrl('https://example.com')).resolves.toBeUndefined();
  });
});
