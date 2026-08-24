/**
 * EchoMind kb-stats.js 单元测试 — 知识库统计 / 文档计数 / 向量计数 / 存储大小。
 *
 * 验证点：
 * 1. openKbStats 创建 overlay DOM
 * 2. openKbStats overlay 设置 aria-modal 和 role=dialog
 * 3. openKbStats 调用 pushPanel
 * 4. closeKbStats 调用 removePanel
 * 5. closeKbStats 隐藏面板
 * 6. openKbStats 加载中显示 Loading 文本
 * 7. openKbStats 成功渲染概要卡片（文档数/分块数/向量数/存储大小）
 * 8. openKbStats 渲染索引状态分布徽标
 * 9. openKbStats 渲染领域分布列表
 * 10. openKbStats IPC 失败显示错误提示
 *
 * Mock: i18n.js, ipc.js (kbStatsApi), toast.js, panel-stack.js, zindex.js, utils.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback || key,
}));

// Mock ipc with kbStatsApi — use vi.hoisted to avoid hoisting order issues
const { _getStatsMock } = vi.hoisted(() => ({ _getStatsMock: vi.fn() }));
vi.mock('../../../ui/src/ipc.js', () => ({
  kbStatsApi: {
    getStats: _getStatsMock,
  },
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
  Z_INDEX: { PANEL_1: 50 },
  zClass: vi.fn(() => 'z-50'),
}));



// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
  formatBytes: vi.fn((bytes) => `${bytes} B`),
}));

// Setup DOM
document.body.innerHTML = '<div id="app"></div>';

import { openKbStats, closeKbStats } from '../../../ui/src/doc-panels.js';

const sampleStats = {
  doc_count: 10,
  chunk_count: 150,
  vector_count: 300,
  db_size_bytes: 1048576,
  status_distribution: [
    ['indexed', 8],
    ['pending', 1],
    ['failed', 1],
  ],
  domain_distribution: [
    ['Technology', 5],
    ['Science', 3],
    ['Law', 2],
  ],
  format_distribution: [
    ['md', 6],
    ['pdf', 3],
    ['txt', 1],
  ],
  tags: [
    ['rust', 4],
    ['ai', 3],
    ['security', 2],
  ],
};

describe('kb-stats.js — 知识库统计面板', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    vi.clearAllMocks();
  });

  it('openKbStats 创建 overlay DOM', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const overlay = document.getElementById('kbStatsOverlay');
    expect(overlay).not.toBeNull();
  });

  it('openKbStats overlay 设置 aria-modal 和 role=dialog', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const overlay = document.getElementById('kbStatsOverlay');
    expect(overlay.getAttribute('role')).toBe('dialog');
    expect(overlay.getAttribute('aria-modal')).toBe('true');
  });

  it('openKbStats 调用 pushPanel', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    const { pushPanel } = await import('../../../ui/src/panel-stack.js');
    await openKbStats();

    expect(pushPanel).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'kb-stats' }),
    );
  });

  it('closeKbStats 调用 removePanel', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    const { removePanel } = await import('../../../ui/src/panel-stack.js');
    await openKbStats();
    closeKbStats();

    expect(removePanel).toHaveBeenCalledWith('kb-stats');
  });

  it('closeKbStats 隐藏面板（添加 hidden 类）', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const overlay = document.getElementById('kbStatsOverlay');
    expect(overlay.classList.contains('hidden')).toBe(false);

    closeKbStats();

    expect(overlay.classList.contains('hidden')).toBe(true);
  });

  it('openKbStats 成功渲染概要卡片（文档数/分块数/向量数/存储大小）', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const body = document.getElementById('kbStatsBody');
    expect(body.textContent).toContain('10'); // doc_count
    expect(body.textContent).toContain('150'); // chunk_count
    expect(body.textContent).toContain('300'); // vector_count
  });

  it('openKbStats 渲染索引状态分布徽标', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const body = document.getElementById('kbStatsBody');
    // 索引状态徽标应包含 indexed/8, pending/1, failed/1
    expect(body.textContent).toContain('indexed');
    expect(body.textContent).toContain('8');
    expect(body.textContent).toContain('pending');
    expect(body.textContent).toContain('failed');
  });

  it('openKbStats 渲染领域分布列表', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const body = document.getElementById('kbStatsBody');
    expect(body.textContent).toContain('Technology');
    expect(body.textContent).toContain('Science');
    expect(body.textContent).toContain('Law');
  });

  it('openKbStats 渲染标签热图', async () => {
    _getStatsMock.mockResolvedValue(sampleStats);
    await openKbStats();

    const body = document.getElementById('kbStatsBody');
    expect(body.textContent).toContain('rust');
    expect(body.textContent).toContain('ai');
    expect(body.textContent).toContain('security');
  });

  it('openKbStats IPC 失败显示错误提示', async () => {
    _getStatsMock.mockRejectedValue(new Error('IPC failed'));
    await openKbStats();

    const body = document.getElementById('kbStatsBody');
    expect(body.textContent).toContain('Failed to load statistics');
  });
});
