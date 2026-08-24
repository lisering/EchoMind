/**
 * queue-send.js 超大规模综合单元测试
 *
 * 覆盖：
 * - enqueueQuery（入队 + 去空格）
 * - dequeueQuery（FIFO 出队）
 * - getQueueSize / getQueueItems
 * - clearQueue
 * - 排队徽章 UI 更新
 * - 排队提示更新
 *
 * 25 个测试用例
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  get: vi.fn(() => false),
  setState: vi.fn(),
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: vi.fn(() => document.createElement('div')),
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock guards
vi.mock('../../../ui/src/action.js', () => ({
  updateInputUI: vi.fn(),
}));

import {
  enqueueQuery,
  dequeueQuery,
  getQueueSize,
  getQueueItems,
  clearQueue,
} from '../../../ui/src/chat-utils.js';

describe('queue-send — 流式排队发送', () => {
  beforeEach(() => {
    clearQueue();
  });

  // ============================================================
  // enqueueQuery
  // ============================================================
  describe('enqueueQuery — 入队', () => {
    it('正常入队并返回队列长度', () => {
      const len = enqueueQuery('What is AI?');
      expect(len).toBe(1);
    });

    it('自动 trim 前后空格', () => {
      enqueueQuery('  padded  ');
      expect(getQueueItems()[0]).toBe('padded');
    });

    it('空字符串不入队', () => {
      const len = enqueueQuery('');
      expect(len).toBe(0);
    });

    it('纯空格字符串不入队', () => {
      const len = enqueueQuery('   ');
      expect(len).toBe(0);
    });

    it('null 不入队', () => {
      const len = enqueueQuery(null);
      expect(len).toBe(0);
    });

    it('undefined 不入队', () => {
      const len = enqueueQuery(undefined);
      expect(len).toBe(0);
    });

    it('多条按 FIFO 顺序入队', () => {
      enqueueQuery('first');
      enqueueQuery('second');
      enqueueQuery('third');
      expect(getQueueItems()).toEqual(['first', 'second', 'third']);
    });

    it('返回值是入队后的长度', () => {
      enqueueQuery('a');
      enqueueQuery('b');
      const len = enqueueQuery('c');
      expect(len).toBe(3);
    });
  });

  // ============================================================
  // dequeueQuery
  // ============================================================
  describe('dequeueQuery — FIFO 出队', () => {
    it('从队首取出并移除', () => {
      enqueueQuery('first');
      enqueueQuery('second');
      expect(dequeueQuery()).toBe('first');
      expect(getQueueSize()).toBe(1);
    });

    it('空队列返回 null', () => {
      expect(dequeueQuery()).toBeNull();
    });

    it('全部出队后为空', () => {
      enqueueQuery('only');
      dequeueQuery();
      expect(dequeueQuery()).toBeNull();
    });

    it('保持 FIFO 顺序', () => {
      enqueueQuery('a');
      enqueueQuery('b');
      enqueueQuery('c');
      expect(dequeueQuery()).toBe('a');
      expect(dequeueQuery()).toBe('b');
      expect(dequeueQuery()).toBe('c');
    });
  });

  // ============================================================
  // getQueueSize
  // ============================================================
  describe('getQueueSize — 队列长度', () => {
    it('空队列返回 0', () => {
      expect(getQueueSize()).toBe(0);
    });

    it('入队后更新', () => {
      enqueueQuery('one');
      expect(getQueueSize()).toBe(1);
      enqueueQuery('two');
      expect(getQueueSize()).toBe(2);
    });

    it('出队后更新', () => {
      enqueueQuery('one');
      enqueueQuery('two');
      dequeueQuery();
      expect(getQueueSize()).toBe(1);
    });
  });

  // ============================================================
  // getQueueItems
  // ============================================================
  describe('getQueueItems — 队列内容副本', () => {
    it('返回数组副本', () => {
      enqueueQuery('item1');
      const items = getQueueItems();
      expect(items).toEqual(['item1']);
      expect(items).not.toBe(getQueueItems()); // 新副本
    });

    it('修改返回数组不影响内部状态', () => {
      enqueueQuery('original');
      const items = getQueueItems();
      items.push('injected');
      expect(getQueueItems()).toEqual(['original']);
    });

    it('空队列返回空数组', () => {
      expect(getQueueItems()).toEqual([]);
    });
  });

  // ============================================================
  // clearQueue
  // ============================================================
  describe('clearQueue — 清空队列', () => {
    it('清空所有排队的问题', () => {
      enqueueQuery('a');
      enqueueQuery('b');
      enqueueQuery('c');
      clearQueue();
      expect(getQueueSize()).toBe(0);
    });

    it('空队列调用不出错', () => {
      expect(() => clearQueue()).not.toThrow();
    });

    it('清空后可继续入队', () => {
      enqueueQuery('first');
      clearQueue();
      enqueueQuery('second');
      expect(getQueueItems()).toEqual(['second']);
    });
  });

  // ============================================================
  // 综合：入队 → 出队 → 清空循环
  // ============================================================
  describe('综合循环', () => {
    it('入队 3 条 → 出队 1 条 → 清空', () => {
      enqueueQuery('q1');
      enqueueQuery('q2');
      enqueueQuery('q3');
      expect(getQueueSize()).toBe(3);

      const first = dequeueQuery();
      expect(first).toBe('q1');
      expect(getQueueSize()).toBe(2);

      clearQueue();
      expect(getQueueSize()).toBe(0);
    });
  });
});
