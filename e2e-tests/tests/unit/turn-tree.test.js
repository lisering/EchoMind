/**
 * EchoMind 轮次版本树单元测试 — turn-tree.js 模块（TC-TURN-001~010）。
 *
 * 测试覆盖：
 * - TC-TURN-001: buildTurnTree 从无消息构建空树
 * - TC-TURN-002: buildTurnTree 从无 turn_group 消息构建单版本 turns
 * - TC-TURN-003: buildTurnTree 从有 turn_group 消息构建多版本 turns
 * - TC-TURN-004: buildTurnTree 混合消息（部分有 turn_group 部分无）
 * - TC-TURN-005: getTurn 按 turnGroup 查询
 * - TC-TURN-006: getActiveVersion 获取活跃版本
 * - TC-TURN-007: getVersionCount 获取版本总数
 * - TC-TURN-008: generateTurnGroupId 生成唯一 ID
 * - TC-TURN-009: buildHistoryFromTurns 仅取活跃版本构建平坦历史
 * - TC-TURN-010: setTurnTree / getTurnTree 读写全局树
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  buildTurnTree,
  getTurnTree,
  setTurnTree,
  getTurn,
  getActiveVersion,
  getVersionCount,
  generateTurnGroupId,
  buildHistoryFromTurns,
} from '../../../ui/src/turn-tree.js';

// ============================================================
// 测试用例
// ============================================================

describe('turn-tree.js — 轮次版本树', () => {
  beforeEach(() => {
    setTurnTree([]);
  });

  describe('buildTurnTree', () => {
    it('TC-TURN-001: 空消息列表返回空树', () => {
      const turns = buildTurnTree([]);
      expect(turns).toEqual([]);
    });

    it('TC-TURN-002: 无 turn_group 的消息构建单版本 turns', () => {
      const messages = [
        { role: 'user', content: '问题1', turn_group: undefined, version: undefined },
        { role: 'assistant', content: '回答1', turn_group: undefined, version: undefined },
        { role: 'user', content: '问题2', turn_group: undefined, version: undefined },
        { role: 'assistant', content: '回答2', turn_group: undefined, version: undefined },
      ];
      const turns = buildTurnTree(messages);
      expect(turns.length).toBe(2);
      expect(turns[0].versions.length).toBe(1);
      expect(turns[0].versions[0].userContent).toBe('问题1');
      expect(turns[0].versions[0].assistantContent).toBe('回答1');
      expect(turns[0].activeVersion).toBe(1);
    });

    it('TC-TURN-003: 有 turn_group 的消息构建多版本 turns', () => {
      const messages = [
        { role: 'user', content: '旧问题', turn_group: 'turn-abc', version: 1 },
        { role: 'assistant', content: '旧回答', turn_group: 'turn-abc', version: 1 },
        { role: 'user', content: '新问题', turn_group: 'turn-abc', version: 2 },
        { role: 'assistant', content: '新回答', turn_group: 'turn-abc', version: 2 },
      ];
      const turns = buildTurnTree(messages);
      expect(turns.length).toBe(1);
      expect(turns[0].turnGroup).toBe('turn-abc');
      expect(turns[0].versions.length).toBe(2);
      expect(turns[0].versions[0].userContent).toBe('旧问题');
      expect(turns[0].versions[1].userContent).toBe('新问题');
      expect(turns[0].activeVersion).toBe(2);
    });

    it('TC-TURN-004: 混合消息（部分有 turn_group 部分无）', () => {
      const messages = [
        { role: 'user', content: '普通问题', turn_group: undefined, version: undefined },
        { role: 'assistant', content: '普通回答', turn_group: undefined, version: undefined },
        { role: 'user', content: '编辑问题v1', turn_group: 'turn-xyz', version: 1 },
        { role: 'assistant', content: '编辑回答v1', turn_group: 'turn-xyz', version: 1 },
        { role: 'user', content: '编辑问题v2', turn_group: 'turn-xyz', version: 2 },
        { role: 'assistant', content: '编辑回答v2', turn_group: 'turn-xyz', version: 2 },
      ];
      const turns = buildTurnTree(messages);
      expect(turns.length).toBe(2);
      // 第一个 turn 是无 turn_group 的单版本
      expect(turns[0].versions.length).toBe(1);
      // 第二个 turn 是有 turn_group 的两版本
      expect(turns[1].versions.length).toBe(2);
      expect(turns[1].activeVersion).toBe(2);
    });
  });

  describe('查询接口', () => {
    beforeEach(() => {
      const messages = [
        { role: 'user', content: 'v1问题', turn_group: 'turn-test', version: 1 },
        { role: 'assistant', content: 'v1回答', turn_group: 'turn-test', version: 1 },
        { role: 'user', content: 'v2问题', turn_group: 'turn-test', version: 2 },
        { role: 'assistant', content: 'v2回答', turn_group: 'turn-test', version: 2 },
      ];
      buildTurnTree(messages);
    });

    it('TC-TURN-005: getTurn 按 turnGroup 查询', () => {
      const turn = getTurn('turn-test');
      expect(turn).not.toBeNull();
      expect(turn.turnGroup).toBe('turn-test');
      expect(turn.versions.length).toBe(2);
    });

    it('TC-TURN-005b: getTurn 查询不存在的 turnGroup 返回 null', () => {
      const turn = getTurn('nonexistent');
      expect(turn).toBeNull();
    });

    it('TC-TURN-006: getActiveVersion 获取活跃版本（默认最新）', () => {
      const ver = getActiveVersion('turn-test');
      expect(ver).not.toBeNull();
      expect(ver.version).toBe(2);
      expect(ver.userContent).toBe('v2问题');
      expect(ver.assistantContent).toBe('v2回答');
    });

    it('TC-TURN-007: getVersionCount 获取版本总数', () => {
      expect(getVersionCount('turn-test')).toBe(2);
      expect(getVersionCount('nonexistent')).toBe(0);
    });
  });

  describe('工具函数', () => {
    it('TC-TURN-008: generateTurnGroupId 生成唯一 ID', () => {
      const id1 = generateTurnGroupId();
      const id2 = generateTurnGroupId();
      expect(id1).not.toBe(id2);
      expect(id1.startsWith('turn-')).toBe(true);
    });

    it('TC-TURN-009: buildHistoryFromTurns 仅取活跃版本', () => {
      const messages = [
        { role: 'user', content: '旧问题', turn_group: 'turn-h', version: 1 },
        { role: 'assistant', content: '旧回答', turn_group: 'turn-h', version: 1 },
        { role: 'user', content: '新问题', turn_group: 'turn-h', version: 2 },
        { role: 'assistant', content: '新回答', turn_group: 'turn-h', version: 2 },
      ];
      buildTurnTree(messages);
      const history = buildHistoryFromTurns();
      expect(history.length).toBe(2);
      expect(history[0].role).toBe('user');
      expect(history[0].content).toBe('新问题');
      expect(history[1].role).toBe('assistant');
      expect(history[1].content).toBe('新回答');
    });

    it('TC-TURN-010: setTurnTree / getTurnTree 读写全局树', () => {
      const mockTurns = [
        {
          turnGroup: 'mock',
          versions: [{ userContent: 'test', assistantContent: null, sources: null, version: 1 }],
          activeVersion: 1,
        },
      ];
      setTurnTree(mockTurns);
      expect(getTurnTree()).toBe(mockTurns);
    });
  });
});
