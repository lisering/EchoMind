/**
 * EchoMind turn-tree.js 单元测试 — 树视图 / 版本切换。
 *
 * 验证点：
 * 1. buildTurnTree 从平坦消息列表构建轮次树
 * 2. buildTurnTree 空列表返回空数组
 * 3. getTurnTree 返回全局轮次树
 * 4. setTurnTree 设置全局轮次树
 * 5. getTurn 按 turnGroup 查询
 * 6. getActiveVersion 返回活跃版本
 * 7. getVersionCount 返回版本数
 * 8. setActiveVersion 设置活跃版本
 * 9. generateTurnGroupId 返回带 turn- 前缀的 ID
 * 10. getActiveVersionMap 返回活跃版本映射
 *
 * Mock: ipc.js, state.js, i18n.js, panel-stack.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  convApi: {
    getConversationTree: vi.fn(() => Promise.resolve(null)),
    setTurnActiveVersion: vi.fn(() => Promise.resolve()),
  },
}));

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  get: (key) => {
    const map = { currentConversationId: 'conv-1' };
    return map[key];
  },
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

import {
  buildTurnTree,
  getTurnTree,
  setTurnTree,
  getTurn,
  getActiveVersion,
  getVersionCount,
  setActiveVersion,
  addVersion,
  updateVersionAssistant,
  applyActiveVersions,
  getActiveVersionMap,
  generateTurnGroupId,
  buildHistoryFromTurns,
  isTreePanelOpen,
} from '../../../ui/src/turn-tree.js';

describe('turn-tree.js — 树构建', () => {
  beforeEach(() => {
    setTurnTree([]);
  });

  it('buildTurnTree 空列表返回空数组', () => {
    const result = buildTurnTree([]);
    expect(result).toEqual([]);
  });

  it('buildTurnTree 从 user+assistant 消息对构建单版本轮次', () => {
    const messages = [
      { id: '1', role: 'user', content: 'hello' },
      { id: '2', role: 'assistant', content: 'hi there', sources: [], reasoning: 'thinking' },
    ];
    const result = buildTurnTree(messages);
    expect(result).toHaveLength(1);
    expect(result[0].versions).toHaveLength(1);
    expect(result[0].versions[0].userContent).toBe('hello');
    expect(result[0].versions[0].assistantContent).toBe('hi there');
    expect(result[0].versions[0].reasoning).toBe('thinking');
    expect(result[0].activeVersion).toBe(1);
  });

  it('buildTurnTree 多版本同一 turn_group 按 version 排序', () => {
    const messages = [
      { id: '1', role: 'user', content: 'v1 question', turn_group: 'tg-1', version: 1 },
      { id: '2', role: 'assistant', content: 'v1 answer', turn_group: 'tg-1', version: 1 },
      { id: '3', role: 'user', content: 'v2 question', turn_group: 'tg-1', version: 2 },
      { id: '4', role: 'assistant', content: 'v2 answer', turn_group: 'tg-1', version: 2 },
    ];
    const result = buildTurnTree(messages);
    expect(result).toHaveLength(1);
    expect(result[0].turnGroup).toBe('tg-1');
    expect(result[0].versions).toHaveLength(2);
    expect(result[0].versions[0].version).toBe(1);
    expect(result[0].versions[1].version).toBe(2);
    expect(result[0].activeVersion).toBe(2);
  });

  it('buildTurnTree 末尾未配对的 user 单独成 turn', () => {
    const messages = [
      { id: '1', role: 'user', content: 'question' },
      // 无 assistant 回答
    ];
    const result = buildTurnTree(messages);
    expect(result).toHaveLength(1);
    expect(result[0].versions[0].userContent).toBe('question');
    expect(result[0].versions[0].assistantContent).toBeNull();
  });
});

describe('turn-tree.js — 查询接口', () => {
  beforeEach(() => {
    const tree = buildTurnTree([
      { id: '1', role: 'user', content: 'q1', turn_group: 'tg-1', version: 1 },
      { id: '2', role: 'assistant', content: 'a1', turn_group: 'tg-1', version: 1 },
      { id: '3', role: 'user', content: 'q2', turn_group: 'tg-1', version: 2 },
      { id: '4', role: 'assistant', content: 'a2', turn_group: 'tg-1', version: 2 },
    ]);
    setTurnTree(tree);
  });

  it('getTurnTree 返回全局轮次树', () => {
    const tree = getTurnTree();
    expect(tree).toHaveLength(1);
  });

  it('getTurn 按 turnGroup 查询', () => {
    const turn = getTurn('tg-1');
    expect(turn).not.toBeNull();
    expect(turn.versions).toHaveLength(2);
  });

  it('getTurn 不存在的 turnGroup 返回 null', () => {
    expect(getTurn('nonexistent')).toBeNull();
  });

  it('getActiveVersion 返回活跃版本', () => {
    const version = getActiveVersion('tg-1');
    expect(version).not.toBeNull();
    expect(version.version).toBe(2);
    expect(version.userContent).toBe('q2');
  });

  it('getVersionCount 返回版本数', () => {
    expect(getVersionCount('tg-1')).toBe(2);
  });

  it('setActiveVersion 设置活跃版本', () => {
    expect(setActiveVersion('tg-1', 1)).toBe(true);
    const version = getActiveVersion('tg-1');
    expect(version.version).toBe(1);
  });

  it('setActiveVersion 不存在的版本返回 false', () => {
    expect(setActiveVersion('tg-1', 999)).toBe(false);
  });

  it('generateTurnGroupId 返回带 turn- 前缀的 ID', () => {
    const id = generateTurnGroupId();
    expect(id).toMatch(/^turn-/);
    expect(id.length).toBeGreaterThan(6);
  });

  it('getActiveVersionMap 返回活跃版本映射', () => {
    const map = getActiveVersionMap();
    expect(map).toHaveLength(1);
    expect(map[0].turn_group).toBe('tg-1');
    expect(map[0].active_version).toBe(2);
  });

  it('buildHistoryFromTurns 返回平坦历史（仅活跃版本）', () => {
    const history = buildHistoryFromTurns();
    expect(history).toHaveLength(2);
    expect(history[0].role).toBe('user');
    expect(history[0].content).toBe('q2');
    expect(history[1].role).toBe('assistant');
    expect(history[1].content).toBe('a2');
  });

  it('isTreePanelOpen 初始返回 false', () => {
    expect(isTreePanelOpen()).toBe(false);
  });
});

describe('turn-tree.js — 版本操作', () => {
  beforeEach(() => {
    setTurnTree([]);
  });

  it('addVersion 向不存在的 turnGroup 添加新版本', () => {
    addVersion('new-tg', 1, 'new question');
    const turn = getTurn('new-tg');
    expect(turn).not.toBeNull();
    expect(turn.versions).toHaveLength(1);
    expect(turn.versions[0].userContent).toBe('new question');
    expect(turn.activeVersion).toBe(1);
  });

  it('addVersion 向已有 turnGroup 追加版本', () => {
    addVersion('tg-1', 1, 'v1 question');
    addVersion('tg-1', 2, 'v2 question');
    const turn = getTurn('tg-1');
    expect(turn.versions).toHaveLength(2);
    expect(turn.activeVersion).toBe(2);
  });

  it('updateVersionAssistant 更新助手回答内容', () => {
    addVersion('tg-2', 1, 'question');
    updateVersionAssistant('tg-2', 1, 'answer', [{ doc: 'test' }], 'reasoning');
    const version = getActiveVersion('tg-2');
    expect(version.assistantContent).toBe('answer');
    expect(version.sources).toEqual([{ doc: 'test' }]);
    expect(version.reasoning).toBe('reasoning');
  });

  it('applyActiveVersions 批量设置活跃版本', () => {
    addVersion('tg-3', 1, 'v1');
    addVersion('tg-3', 2, 'v2');
    addVersion('tg-4', 1, 'v1');
    addVersion('tg-4', 2, 'v2');

    applyActiveVersions([
      { turn_group: 'tg-3', active_version: 1 },
      { turn_group: 'tg-4', active_version: 2 },
    ]);

    expect(getActiveVersion('tg-3').version).toBe(1);
    expect(getActiveVersion('tg-4').version).toBe(2);
  });

  it('applyActiveVersions 空列表安全返回', () => {
    expect(() => applyActiveVersions([])).not.toThrow();
    expect(() => applyActiveVersions(null)).not.toThrow();
  });

  it('generateTurnGroupId 多次调用产生不同 ID', () => {
    const id1 = generateTurnGroupId();
    const id2 = generateTurnGroupId();
    expect(id1).not.toBe(id2);
  });
});
