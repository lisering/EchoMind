/**
 * EchoMind doc-preview.js 单元测试 — 文档预览 / 元数据 / 内容 / Chunk 列表。
 *
 * 验证点：
 * 1. openDocPreview 创建 overlay DOM
 * 2. openDocPreview overlay 设置 aria-modal 和 role=dialog
 * 3. openDocPreview 调用 pushPanel
 * 4. openDocPreview 成功渲染文档元数据
 * 5. openDocPreview 渲染内容预览
 * 6. openDocPreview 渲染 chunk 列表
 * 7. openDocPreview 渲染摘要
 * 8. openDocPreview 渲染标签
 * 9. openDocPreview preview=null 显示未找到提示
 * 10. closeDocPreview 调用 removePanel
 *
 * Mock: i18n.js, ipc.js (docPreviewApi), toast.js, panel-stack.js, zindex.js, utils.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock ipc with docPreviewApi — use vi.hoisted
const { _getPreviewMock } = vi.hoisted(() => ({ _getPreviewMock: vi.fn() }));
vi.mock('../../../ui/src/ipc.js', () => ({
  docPreviewApi: {
    getPreview: _getPreviewMock,
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
  Z_INDEX: { MODAL: 80 },
  zClass: vi.fn(() => 'z-80'),
}));



// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Setup DOM
document.body.innerHTML = '<div id="app"></div>';

import { openDocPreview, closeDocPreview } from '../../../ui/src/doc-panels.js';

const samplePreview = {
  file_path: '/docs/test-file.md',
  status: 'indexed',
  chunk_count: 5,
  created_at: 1700000000,
  file_hash: 'abcdef1234567890',
  content_preview: 'This is a preview of the document content.',
  summary: 'A test document about unit testing.',
  tags: ['rust', 'testing'],
  chunks: [
    { sequence: 0, token_count: 100, content_preview: 'Chunk 0 content' },
    { sequence: 1, token_count: 120, content_preview: 'Chunk 1 content' },
  ],
};

describe('doc-preview.js — 文档预览面板', () => {
  beforeEach(() => {
    document.body.innerHTML = '<div id="app"></div>';
    vi.clearAllMocks();
  });

  it('openDocPreview 创建 overlay DOM', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const overlay = document.querySelector('[role="dialog"]');
    expect(overlay).not.toBeNull();
    expect(overlay.querySelector('#docPreviewTitle')).not.toBeNull();
  });

  it('openDocPreview overlay 设置 aria-modal 和 role=dialog', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const overlay = document.querySelector('[role="dialog"]');
    expect(overlay.getAttribute('aria-modal')).toBe('true');
  });

  it('openDocPreview 调用 pushPanel', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    const { pushPanel } = await import('../../../ui/src/panel-stack.js');
    await openDocPreview('doc-1');

    expect(pushPanel).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'doc-preview' }),
    );
  });

  it('openDocPreview 成功渲染文档元数据（文件名/格式/状态/分块数）', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const body = document.getElementById('docPreviewBody');
    expect(body.textContent).toContain('test-file.md');
    expect(body.textContent).toContain('MD');
    expect(body.textContent).toContain('5');
  });

  it('openDocPreview 渲染内容预览', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const body = document.getElementById('docPreviewBody');
    expect(body.textContent).toContain('This is a preview of the document content.');
  });

  it('openDocPreview 渲染 chunk 列表', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const body = document.getElementById('docPreviewBody');
    expect(body.textContent).toContain('#0');
    expect(body.textContent).toContain('100');
    expect(body.textContent).toContain('#1');
    expect(body.textContent).toContain('120');
  });

  it('openDocPreview 渲染摘要', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const body = document.getElementById('docPreviewBody');
    expect(body.textContent).toContain('A test document about unit testing.');
  });

  it('openDocPreview 渲染标签', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    await openDocPreview('doc-1');

    const body = document.getElementById('docPreviewBody');
    expect(body.textContent).toContain('rust');
    expect(body.textContent).toContain('testing');
  });

  it('openDocPreview preview=null 显示未找到提示', async () => {
    _getPreviewMock.mockResolvedValue(null);
    await openDocPreview('doc-1');

    const body = document.getElementById('docPreviewBody');
    expect(body.textContent).toContain('doc.preview_not_found');
  });

  it('closeDocPreview 调用 removePanel', async () => {
    _getPreviewMock.mockResolvedValue(samplePreview);
    const { removePanel } = await import('../../../ui/src/panel-stack.js');
    await openDocPreview('doc-1');
    closeDocPreview();

    expect(removePanel).toHaveBeenCalledWith('doc-preview');
  });
});
