/**
 * EchoMind state.js 单元测试补充 — 草稿 / 主题 / 安全 / 功能开关 / 便捷访问器。
 *
 * 已有 state.test.js 覆盖基础状态（16 tests）。
 * 本文件补充覆盖：
 * 1. drafts 草稿读写
 * 2. theme 主题切换
 * 3. securityState 安全状态
 * 4. 功能开关 (hybrid/agent/subAgent/memory/webSearch)
 * 5. vlmEnabled / rerankEnabled / hydeEnabled
 * 6. contextTokens / contextLimit
 * 7. isEncrypted / isLocked
 * 8. currentModel / currentLlmMode
 * 9. docList / kbAllDocs
 * 10. setState 不变性（浅拷贝）
 *
 * 直接导入 state.js（无 Mock 依赖）
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { getState, get, setState, resetState, isEncrypted, isLocked, subscribe } from '../../../ui/src/state.js';

describe('state.js — drafts 草稿读写', () => {
  beforeEach(() => {
    resetState();
  });

  it('drafts 初始为空对象', () => {
    expect(getState().drafts).toEqual({});
  });

  it('设置 draft 后可读取', () => {
    setState({ drafts: { 'conv-1': '草稿内容' } });
    expect(getState().drafts['conv-1']).toBe('草稿内容');
  });

  it('多个会话草稿共存', () => {
    setState({ drafts: { 'conv-1': '文本1', 'conv-2': '文本2' } });
    expect(Object.keys(getState().drafts)).toHaveLength(2);
  });

  it('清除单个 draft 不影响其他', () => {
    setState({ drafts: { 'conv-1': '文本1', 'conv-2': '文本2' } });
    const drafts = getState().drafts;
    delete drafts['conv-1'];
    setState({ drafts });
    expect(getState().drafts['conv-2']).toBe('文本2');
    expect(getState().drafts['conv-1']).toBeUndefined();
  });
});

describe('state.js — theme 主题', () => {
  beforeEach(() => {
    resetState();
  });

  it('初始主题为 dark', () => {
    expect(getState().theme).toBe('dark');
  });

  it('切换到 light 主题', () => {
    setState({ theme: 'light' });
    expect(getState().theme).toBe('light');
  });

  it('切换到 system 主题', () => {
    setState({ theme: 'system' });
    expect(getState().theme).toBe('system');
  });

  it('切换到 high-contrast 主题', () => {
    setState({ theme: 'high-contrast' });
    expect(getState().theme).toBe('high-contrast');
  });
});

describe('state.js — securityState 安全状态', () => {
  beforeEach(() => {
    resetState();
  });

  it('初始状态为 unencrypted', () => {
    expect(getState().securityState).toBe('unencrypted');
  });

  it('加密后状态为 encrypted_unlocked', () => {
    setState({ securityState: 'encrypted_unlocked' });
    expect(getState().securityState).toBe('encrypted_unlocked');
  });

  it('锁定状态为 locked', () => {
    setState({ securityState: 'locked' });
    expect(getState().securityState).toBe('locked');
  });

  it('isEncrypted() 在 unencrypted 时返回 false', () => {
    expect(isEncrypted()).toBe(false);
  });

  it('isEncrypted() 在 encrypted_unlocked 时返回 true', () => {
    setState({ securityState: 'encrypted_unlocked' });
    expect(isEncrypted()).toBe(true);
  });

  it('isLocked() 在 unencrypted 时返回 false', () => {
    expect(isLocked()).toBe(false);
  });

  it('isLocked() 在 locked 时返回 true', () => {
    setState({ securityState: 'locked' });
    expect(isLocked()).toBe(true);
  });
});

describe('state.js — 功能开关', () => {
  beforeEach(() => {
    resetState();
  });

  it('hybridEnabled 初始为 false', () => {
    expect(getState().hybridEnabled).toBe(false);
  });

  it('agentEnabled 初始为 false', () => {
    expect(getState().agentEnabled).toBe(false);
  });

  it('subAgentEnabled 初始为 false', () => {
    expect(getState().subAgentEnabled).toBe(false);
  });

  it('memoryEnabled 初始为 false', () => {
    expect(getState().memoryEnabled).toBe(false);
  });

  it('webSearchEnabled 初始为 false', () => {
    expect(getState().webSearchEnabled).toBe(false);
  });

  it('批量设置所有功能开关', () => {
    setState({
      hybridEnabled: true,
      agentEnabled: true,
      subAgentEnabled: true,
      memoryEnabled: true,
      webSearchEnabled: true,
    });
    const s = getState();
    expect(s.hybridEnabled).toBe(true);
    expect(s.agentEnabled).toBe(true);
    expect(s.subAgentEnabled).toBe(true);
    expect(s.memoryEnabled).toBe(true);
    expect(s.webSearchEnabled).toBe(true);
  });
});

describe('state.js — vlmEnabled / rerankEnabled / hydeEnabled', () => {
  beforeEach(() => {
    resetState();
  });

  it('vlmEnabled 初始为 false', () => {
    expect(getState().vlmEnabled).toBe(false);
  });

  it('rerankEnabled 初始为 false', () => {
    expect(getState().rerankEnabled).toBe(false);
  });

  it('hydeEnabled 初始为 false', () => {
    expect(getState().hydeEnabled).toBe(false);
  });

  it('设置 vlmEnabled 为 true', () => {
    setState({ vlmEnabled: true });
    expect(getState().vlmEnabled).toBe(true);
  });
});

describe('state.js — contextTokens / contextLimit', () => {
  beforeEach(() => {
    resetState();
  });

  it('contextTokens 初始为 0', () => {
    expect(getState().contextTokens).toBe(0);
  });

  it('contextLimit 初始为 8000', () => {
    expect(getState().contextLimit).toBe(8000);
  });

  it('更新 contextTokens', () => {
    setState({ contextTokens: 5000 });
    expect(getState().contextTokens).toBe(5000);
  });

  it('更新 contextLimit', () => {
    setState({ contextLimit: 32000 });
    expect(getState().contextLimit).toBe(32000);
  });
});

describe('state.js — currentModel / currentLlmMode', () => {
  beforeEach(() => {
    resetState();
  });

  it('currentModel 初始为空字符串', () => {
    expect(getState().currentModel).toBe('');
  });

  it('currentLlmMode 初始为 remote', () => {
    expect(getState().currentLlmMode).toBe('remote');
  });

  it('设置本地模型', () => {
    setState({ currentModel: 'qwen2.5-7b.gguf', currentLlmMode: 'local' });
    expect(getState().currentModel).toBe('qwen2.5-7b.gguf');
    expect(getState().currentLlmMode).toBe('local');
  });
});

describe('state.js — docList / kbAllDocs', () => {
  beforeEach(() => {
    resetState();
  });

  it('docList 初始为空数组', () => {
    expect(getState().docList).toEqual([]);
  });

  it('kbAllDocs 初始为空数组', () => {
    expect(getState().kbAllDocs).toEqual([]);
  });

  it('设置 docList', () => {
    setState({ docList: [{ name: 'doc1.pdf' }, { name: 'doc2.pdf' }] });
    expect(getState().docList).toHaveLength(2);
  });
});

describe('state.js — setState 不变性（浅拷贝）', () => {
  beforeEach(() => {
    resetState();
  });

  it('getState 返回的快照修改不影响内部状态', () => {
    const snapshot = getState();
    snapshot.streaming = true;
    expect(get('streaming')).toBe(false);
  });

  it('setState 创建新对象引用', () => {
    const s1 = getState();
    setState({ isPro: true });
    const s2 = getState();
    expect(s1).not.toBe(s2);
  });

  it('相同值不触发订阅者通知', () => {
    // Arrange: 订阅 streaming 字段
    let callCount = 0;
    const unsub = subscribe('streaming', () => { callCount++; });

    // Act: 设置相同值
    setState({ streaming: false });  // false 是初始值，不应触发

    // Assert: 订阅者未被调用
    expect(callCount).toBe(0);
    unsub();
  });
});
