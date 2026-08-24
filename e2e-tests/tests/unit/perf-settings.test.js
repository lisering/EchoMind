/**
 * EchoMind perf-settings.js 单元测试 — 智能模式 / 高级设置。
 *
 * 验证点：
 * 1. renderPerfSettings null 容器安全返回
 * 2. renderPerfSettings 渲染智能模式开关
 * 3. renderPerfSettings 渲染缓存统计
 * 4. renderPerfSettings 渲染压缩比滑块
 * 5. renderPerfSettings 渲染索引重建按钮
 * 6. renderPerfSettings 渲染检索记忆 toggle
 * 7. renderPerfSettings 渲染渐进式注入 toggle
 * 8. renderPerfSettings 渲染 Speculative RAG toggle
 * 9. renderPerfSettings 渲染质量门控 toggle
 * 10. renderPerfSettings 渲染知识图谱检索 toggle
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
    if (cmd === 'get_cache_stats') return Promise.resolve({ total_queries: 100, exact_hits: 40, semantic_hits: 30, retrieval_hits: 20, cache_size_entries: 15, estimated_token_saved: 50000 });
    if (cmd === 'get_cache_settings') return Promise.resolve({ enabled: true });
    if (cmd === 'get_compression_ratio') return Promise.resolve(3);
    if (cmd === 'get_retrieval_memory_stats') return Promise.resolve([{ id: 1 }]);
    if (cmd === 'get_settings') return Promise.resolve({ progressive_injection: false, speculative_enabled: false, quality_gate_enabled: false, graph_retriever_enabled: false, contextual_retrieval: true });
    return Promise.resolve(null);
  }),
  smartModeApi: { get: vi.fn(() => Promise.resolve(true)), set: vi.fn(() => Promise.resolve()) },
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

// Setup DOM
function setupDom() {
  document.body.innerHTML = '<div id="perfSettingsContainer"></div>';
}

import { renderPerfSettings } from '../../../ui/src/perf-settings.js';

describe('perf-settings.js — 渲染', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
  });

  it('renderPerfSettings null 容器安全返回', async () => {
    await expect(renderPerfSettings(null)).resolves.toBeUndefined();
  });

  it('renderPerfSettings 渲染智能模式开关', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('smartModeToggle');
  });

  it('renderPerfSettings 渲染缓存统计', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfCacheToggle');
    expect(container.innerHTML).toContain('perfCacheClear');
  });

  it('renderPerfSettings 渲染压缩比滑块', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfCompressionSlider');
    expect(container.innerHTML).toContain('perfCompressionValue');
  });

  it('renderPerfSettings 渲染检索记忆 toggle', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfMemoryToggle');
    expect(container.innerHTML).toContain('perfMemoryReset');
  });

  it('renderPerfSettings 渲染渐进式注入 toggle', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfProgressiveToggle');
  });

  it('renderPerfSettings 渲染 Speculative RAG toggle', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfSpeculativeToggle');
  });

  it('renderPerfSettings 渲染质量门控 toggle', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfQualityGateToggle');
  });

  it('renderPerfSettings 渲染知识图谱检索 toggle', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfGraphRetrieverToggle');
  });

  it('renderPerfSettings 渲染索引重建按钮', async () => {
    const container = document.getElementById('perfSettingsContainer');
    await renderPerfSettings(container);
    expect(container.innerHTML).toContain('perfRebuildBM25');
    expect(container.innerHTML).toContain('perfRebuildProposition');
    expect(container.innerHTML).toContain('perfBuildSummaryTree');
  });
});
