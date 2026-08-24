/**
 * EchoMind 状态容器单元测试 — state.js 模块。
 *
 * 验证点：
 * 1. 初始状态正确
 * 2. setState 部分更新不丢失其他字段
 * 3. subscribe 细粒度订阅正确通知
 * 4. subscribeAll 全局订阅正确通知
 * 5. 便捷访问器返回正确值
 * 6. resetState 完全重置
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { getState, get, setState, subscribe, subscribeAll, resetState, isStreaming, isProUser, currentConv, isAuditing } from '../../../ui/src/state.js';

describe('State Store — state.js', () => {
  beforeEach(() => {
    resetState();
  });

  describe('初始状态', () => {
    it('history 初始为空数组', () => {
      expect(getState().history).toEqual([]);
    });

    it('streaming 初始为 false', () => {
      expect(getState().streaming).toBe(false);
    });

    it('currentConversationId 初始为 null', () => {
      expect(getState().currentConversationId).toBeNull();
    });

    it('isPro 初始为 false', () => {
      expect(getState().isPro).toBe(false);
    });

    it('activeSidebarTab 初始为 conversations', () => {
      expect(getState().activeSidebarTab).toBe('conversations');
    });

    it('cmdSelectedIndex 初始为 0', () => {
      expect(getState().cmdSelectedIndex).toBe(0);
    });
  });

  describe('setState 部分更新', () => {
    it('更新 streaming 不丢失 history', () => {
      setState({ history: [{ role: 'user', content: 'hi' }] });
      setState({ streaming: true });
      const s = getState();
      expect(s.streaming).toBe(true);
      expect(s.history).toHaveLength(1);
    });

    it('更新多个字段一次成功', () => {
      setState({ streaming: true, currentConversationId: 'conv-1', isPro: true });
      const s = getState();
      expect(s.streaming).toBe(true);
      expect(s.currentConversationId).toBe('conv-1');
      expect(s.isPro).toBe(true);
    });

    it('相同值不触发订阅', () => {
      let callCount = 0;
      const unsub = subscribe('streaming', () => { callCount++; });
      setState({ streaming: false }); // 值未变
      expect(callCount).toBe(0);
      unsub();
    });
  });

  describe('subscribe 细粒度订阅', () => {
    it('订阅 streaming 变化时被通知', () => {
      let received = null;
      const unsub = subscribe('streaming', (val) => { received = val; });
      setState({ streaming: true });
      expect(received).toBe(true);
      unsub();
    });

    it('取消订阅后不再被通知', () => {
      let callCount = 0;
      const unsub = subscribe('isPro', () => { callCount++; });
      setState({ isPro: true });
      expect(callCount).toBe(1);
      unsub();
      setState({ isPro: false });
      expect(callCount).toBe(1);
    });

    it('不同字段的更新不触发无关订阅', () => {
      let streamingCalls = 0;
      let isProCalls = 0;
      const u1 = subscribe('streaming', () => { streamingCalls++; });
      const u2 = subscribe('isPro', () => { isProCalls++; });
      setState({ streaming: true });
      expect(streamingCalls).toBe(1);
      expect(isProCalls).toBe(0);
      setState({ isPro: true });
      expect(streamingCalls).toBe(1);
      expect(isProCalls).toBe(1);
      u1();
      u2();
    });
  });

  describe('subscribeAll 全局订阅', () => {
    it('任意字段变化时被通知', () => {
      let received = null;
      let changed = [];
      const unsub = subscribeAll((state, keys) => { received = state; changed = keys; });
      setState({ streaming: true });
      expect(received.streaming).toBe(true);
      expect(changed).toContain('streaming');
      unsub();
    });
  });

  describe('便捷访问器', () => {
    it('isStreaming() 返回当前 streaming 状态', () => {
      expect(isStreaming()).toBe(false);
      setState({ streaming: true });
      expect(isStreaming()).toBe(true);
    });

    it('isProUser() 返回当前 isPro 状态', () => {
      expect(isProUser()).toBe(false);
      setState({ isPro: true });
      expect(isProUser()).toBe(true);
    });

    it('currentConv() 返回当前会话 ID', () => {
      expect(currentConv()).toBeNull();
      setState({ currentConversationId: 'conv-42' });
      expect(currentConv()).toBe('conv-42');
    });

    it('isAuditing() 返回审计状态', () => {
      expect(isAuditing()).toBe(false);
      setState({ auditingDocId: 'doc-1' });
      expect(isAuditing()).toBe(true);
    });
  });

  describe('resetState', () => {
    it('重置后所有字段回到初始值', () => {
      setState({ streaming: true, isPro: true, history: [{ role: 'user', content: 'x' }] });
      resetState();
      const s = getState();
      expect(s.streaming).toBe(false);
      expect(s.isPro).toBe(false);
      expect(s.history).toEqual([]);
    });

    it('重置后清除所有订阅者', () => {
      let callCount = 0;
      subscribe('streaming', () => { callCount++; });
      resetState();
      setState({ streaming: true });
      expect(callCount).toBe(0);
    });
  });
});
