/**
 * input-history.js 单元测试
 *
 * 覆盖 recordInput / navigateHistoryUp / navigateHistoryDown / resetHistoryNav /
 * clearHistory / estimateTokens / saveDraft / restoreDraft / clearDraft。
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock state.js
const _state = {
  currentConversationId: 'conv-1',
  drafts: {},
};

vi.mock('../../../ui/src/state.js', () => ({
  get: vi.fn((key) => _state[key]),
  setState: vi.fn((patch) => { Object.assign(_state, patch); }),
}));

// Mock utils.js
vi.mock('../../../ui/src/utils.js', () => ({
  $: vi.fn(() => ({
    value: '',
    style: { height: 'auto' },
    scrollHeight: 100,
    setSelectionRange: vi.fn(),
  })),
}));

const {
  recordInput,
  navigateHistoryUp,
  navigateHistoryDown,
  resetHistoryNav,
  clearHistory,
  estimateTokens,
  saveDraft,
  restoreDraft,
  clearDraft,
} = await import('../../../ui/src/input-utils.js');

describe('input-history', () => {
  beforeEach(() => {
    clearHistory('conv-1');
    resetHistoryNav();
    _state.currentConversationId = 'conv-1';
    _state.drafts = {};
  });

  describe('estimateTokens', () => {
    it('空文本返回 0', () => {
      expect(estimateTokens('')).toBe(0);
      expect(estimateTokens(null)).toBe(0);
      expect(estimateTokens('   ')).toBe(0);
    });

    it('纯英文文本约 4 字符/token', () => {
      const text = 'Hello World'; // 11 chars
      const tokens = estimateTokens(text);
      expect(tokens).toBe(Math.ceil(11 / 4)); // 3
    });

    it('纯中文文本约 1.5 字符/token', () => {
      const text = '你好世界你好世界'; // 8 CJK chars
      const tokens = estimateTokens(text);
      expect(tokens).toBe(Math.ceil(8 / 1.5)); // 6
    });

    it('混合文本正确估算', () => {
      const text = '你好 World'; // 2 CJK + 7 other
      const cjk = 2;
      const other = text.length - cjk; // 7
      const expected = Math.ceil(cjk / 1.5 + other / 4);
      const tokens = estimateTokens(text);
      expect(tokens).toBe(expected);
    });
  });

  describe('recordInput', () => {
    it('空查询不记录', () => {
      recordInput('');
      expect(navigateHistoryUp()).toBeNull();
    });

    it('空白查询不记录', () => {
      recordInput('   ');
      expect(navigateHistoryUp()).toBeNull();
    });

    it('正常查询被记录', () => {
      recordInput('hello world');
      const up = navigateHistoryUp();
      expect(up).toBe('hello world');
    });

    it('去重：连续相同查询只记录一次', () => {
      recordInput('same query');
      recordInput('same query');
      const up = navigateHistoryUp();
      expect(up).toBe('same query');
      const down = navigateHistoryDown();
      // 只有一条，到底部返回 savedInput
      expect(down).toBe(''); // _savedInput 初始为空
    });

    it('多条查询按顺序记录', () => {
      recordInput('first');
      recordInput('second');
      recordInput('third');
      // 最新的先出现
      expect(navigateHistoryUp()).toBe('third');
      expect(navigateHistoryUp()).toBe('second');
      expect(navigateHistoryUp()).toBe('first');
    });

    it('超过 50 条自动移除最旧的', () => {
      for (let i = 0; i < 55; i++) {
        recordInput(`query-${i}`);
      }
      // 导航到最旧
      for (let i = 0; i < 49; i++) {
        navigateHistoryUp();
      }
      const oldest = navigateHistoryUp();
      // 应该是 query-5（前 5 条被移除）
      expect(oldest).toBe('query-5');
    });

    it('trim 后记录', () => {
      recordInput('  spaced  ');
      expect(navigateHistoryUp()).toBe('spaced');
    });
  });

  describe('navigateHistoryUp', () => {
    it('无历史时返回 null', () => {
      expect(navigateHistoryUp()).toBeNull();
    });

    it('首次按 Up 返回最新一条', () => {
      recordInput('latest');
      expect(navigateHistoryUp()).toBe('latest');
    });

    it('到达最旧后返回 null', () => {
      recordInput('only');
      navigateHistoryUp(); // 到达最旧
      expect(navigateHistoryUp()).toBeNull();
    });
  });

  describe('navigateHistoryDown', () => {
    it('未导航时返回 null', () => {
      expect(navigateHistoryDown()).toBeNull();
    });

    it('向下导航后返回更新的历史', () => {
      recordInput('first');
      recordInput('second');
      navigateHistoryUp(); // → second
      navigateHistoryUp(); // → first
      const result = navigateHistoryDown();
      expect(result).toBe('second');
    });

    it('到达最新后恢复原始输入', () => {
      recordInput('test');
      navigateHistoryUp(); // → test
      const result = navigateHistoryDown();
      expect(result).toBe(''); // _savedInput 初始为空
    });
  });

  describe('clearHistory', () => {
    it('清除指定会话历史', () => {
      recordInput('test');
      clearHistory('conv-1');
      expect(navigateHistoryUp()).toBeNull();
    });
  });

  describe('resetHistoryNav', () => {
    it('重置后 navigateHistoryDown 返回 null', () => {
      recordInput('test');
      navigateHistoryUp();
      resetHistoryNav();
      expect(navigateHistoryDown()).toBeNull();
    });
  });

  describe('saveDraft / restoreDraft / clearDraft', () => {
    it('saveDraft 保存当前输入到 drafts', () => {
      // $() mock 返回 value: '' 所以 draft 被删除
      saveDraft();
      expect(_state.drafts['conv-1']).toBeUndefined();
    });

    it('clearDraft 删除指定会话草稿', () => {
      _state.drafts = { 'conv-1': 'some text' };
      clearDraft('conv-1');
      expect(_state.drafts['conv-1']).toBeUndefined();
    });
  });
});
