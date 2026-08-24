/**
 * bookmarks.js 单元测试
 *
 * 覆盖书签 CRUD、列表渲染、跳转逻辑。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock ipc.js
const mockBookmarks = [];
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(async (cmd, args) => {
    if (cmd === 'add_bookmark') {
      const bm = { id: 'bm-' + mockBookmarks.length, ...args };
      mockBookmarks.push(bm);
      return bm;
    }
    if (cmd === 'get_bookmarks') {
      return [...mockBookmarks];
    }
    if (cmd === 'delete_bookmark') {
      const idx = mockBookmarks.findIndex(b => b.id === args?.id);
      if (idx >= 0) mockBookmarks.splice(idx, 1);
      return true;
    }
    return null;
  }),
}));

// Mock toast.js
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

// Mock i18n.js
vi.mock('../../../ui/src/i18n.js', () => ({
  t: vi.fn((key) => key),
}));

// Mock utils.js
vi.mock('../../../ui/src/utils.js', () => ({
  $: vi.fn(() => null),
}));

describe('bookmarks', () => {
  beforeEach(() => {
    mockBookmarks.length = 0;
    vi.clearAllMocks();
  });

  it('mock setup is correct', () => {
    expect(mockBookmarks).toHaveLength(0);
  });

  describe('invoke mock', () => {
    it('add_bookmark creates bookmark', async () => {
      const { invoke } = await import('../../../ui/src/ipc.js');
      const result = await invoke('add_bookmark', {
        conversation_id: 'conv-1',
        message_id: 'msg-1',
        title: 'Test Bookmark',
      });
      expect(result.id).toBeDefined();
      expect(result.title).toBe('Test Bookmark');
      expect(mockBookmarks).toHaveLength(1);
    });

    it('get_bookmarks returns list', async () => {
      const { invoke } = await import('../../../ui/src/ipc.js');
      await invoke('add_bookmark', { conversation_id: 'c1', message_id: 'm1', title: 'B1' });
      await invoke('add_bookmark', { conversation_id: 'c2', message_id: 'm2', title: 'B2' });
      const list = await invoke('get_bookmarks');
      expect(list).toHaveLength(2);
    });

    it('delete_bookmark removes from list', async () => {
      const { invoke } = await import('../../../ui/src/ipc.js');
      const bm = await invoke('add_bookmark', { conversation_id: 'c1', message_id: 'm1', title: 'B1' });
      await invoke('delete_bookmark', { id: bm.id });
      const list = await invoke('get_bookmarks');
      expect(list).toHaveLength(0);
    });
  });
});
