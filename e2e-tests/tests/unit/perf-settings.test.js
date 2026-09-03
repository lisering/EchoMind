/**
 * EchoMind perf-settings.js 单元测试 — 智能模式 / Contextual Retrieval。
 *
 * 大简化重构后精简版，验证点：
 * 1. renderPerfSettings null 容器安全返回
 * 2. renderPerfSettings 渲染智能模式开关
 * 3. renderPerfSettings 渲染 Contextual Retrieval toggle
 * 4. renderPerfSettings 渲染索引重建按钮
 *
 * 学术 RAG 优化模块已删除：缓存、压缩、检索记忆、渐进式注入、
 * Speculative RAG、质量门控、知识图谱检索、Proposition 索引、Summary Tree。
 *
 * Mock: utils.js, ipc.js, i18n.js, toast.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'get_settings') return Promise.resolve({ contextual_retrieval: true });
    return Promise.resolve(null);
  }),
  settingsApi: {
    update: vi.fn(() => Promise.resolve()),
    get: vi.fn(() => Promise.resolve('')),
    setBool: vi.fn(() => Promise.resolve()),
  },
  smartModeApi: {
    get: vi.fn(() => Promise.resolve(true)),
    set: vi.fn(() => Promise.resolve()),
  },
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

const { renderPerfSettings } = await import('../../../ui/src/perf-settings.js');

describe('perf-settings.js', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="container"></div>';
  });

  it('renderPerfSettings null 容器安全返回', async () => {
    await renderPerfSettings(null);
    // 无异常即通过
  });

  it('renderPerfSettings 渲染智能模式开关', async () => {
    const container = document.getElementById('container');
    await renderPerfSettings(container);
    const toggle = document.getElementById('smartModeToggle');
    expect(toggle).toBeTruthy();
  });

  it('renderPerfSettings 渲染 Contextual Retrieval toggle', async () => {
    const container = document.getElementById('container');
    await renderPerfSettings(container);
    const toggle = document.getElementById('perfContextualToggle');
    expect(toggle).toBeTruthy();
  });

  it('renderPerfSettings 渲染索引重建按钮', async () => {
    const container = document.getElementById('container');
    await renderPerfSettings(container);
    const bm25Btn = document.getElementById('perfRebuildBM25');
    const ctxBtn = document.getElementById('perfRebuildContextualEmbeddings');
    expect(bm25Btn).toBeTruthy();
    expect(ctxBtn).toBeTruthy();
  });
});
