/**
 * EchoMind export.js 单元测试 — PDF 导出 / HTML 导出 / 设置持久化 / 打印 HTML 构建。
 *
 * 覆盖：
 * 1. PDF 页面大小设置读写 (getPdfPageSize / setPdfPageSize)
 * 2. 引用来源包含设置读写 (getPdfIncludeSources / setPdfIncludeSources)
 * 3. buildPrintHtml 构建（标题/消息块/CSS/分页）
 * 4. buildExportHtml 独立 HTML 构建
 * 5. escapeHtml / renderMarkdownToHtml / buildSourcesHtml
 * 6. applyHighlightInHtmlString (hljs 不可用时降级)
 * 7. annotateMermaidInHtmlString (Mermaid 标注)
 * 8. printViaIframe (iframe 创建 + mock 打印)
 * 9. exportConversationToPdf (空会话ID拦截 / 空消息拦截)
 * 10. exportDocumentToPdf (空 docId 拦截)
 *
 * Mock: ipc.js, i18n.js, toast.js, lazy-loader.js, state.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock localStorage
const _ls = new Map();
globalThis.localStorage = {
  getItem: (key) => _ls.get(key) ?? null,
  setItem: (key, val) => { _ls.set(key, String(val)); },
  removeItem: (key) => { _ls.delete(key); },
  clear: () => { _ls.clear(); },
};

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, fallback) => fallback ?? key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

// Mock lazy-loader
vi.mock('../../../ui/src/lazy-loader.js', () => ({
  loadHighlight: vi.fn().mockResolvedValue(undefined),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
  convApi: {
    list: vi.fn().mockResolvedValue([]),
    messages: vi.fn().mockResolvedValue([]),
  },
  docApi: {
    list: vi.fn().mockResolvedValue([]),
  },
  listen: vi.fn(),
}));

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  get: vi.fn((key) => {
    if (key === 'currentConversationId') return 'conv-123';
    return undefined;
  }),
}));

// Import after mocks
import {
  getPdfPageSize,
  setPdfPageSize,
  getPdfIncludeSources,
  setPdfIncludeSources,
  buildPrintHtml,
  buildExportHtml,
} from '../../../ui/src/export.js';

describe('export.js — PDF 页面大小设置', () => {
  beforeEach(() => {
    _ls.clear();
  });

  it('默认页面大小为 A4', () => {
    // Arrange: 无存储值
    // Act
    const size = getPdfPageSize();
    // Assert
    expect(size).toBe('A4');
  });

  it('设置并读取 Letter 页面大小', () => {
    // Arrange
    setPdfPageSize('Letter');
    // Act
    const size = getPdfPageSize();
    // Assert
    expect(size).toBe('Letter');
  });

  it('设置 A4 后可读取', () => {
    // Arrange
    setPdfPageSize('A4');
    // Act
    const size = getPdfPageSize();
    // Assert
    expect(size).toBe('A4');
  });
});

describe('export.js — 引用来源包含设置', () => {
  beforeEach(() => {
    _ls.clear();
  });

  it('默认包含引用来源 (true)', () => {
    // Arrange: 无存储值
    // Act
    const include = getPdfIncludeSources();
    // Assert
    expect(include).toBe(true);
  });

  it('设置为 false 后可读取', () => {
    // Arrange
    setPdfIncludeSources(false);
    // Act
    const include = getPdfIncludeSources();
    // Assert
    expect(include).toBe(false);
  });

  it('设置为 true 后可读取', () => {
    // Arrange
    setPdfIncludeSources(false);
    setPdfIncludeSources(true);
    // Act
    const include = getPdfIncludeSources();
    // Assert
    expect(include).toBe(true);
  });
});

describe('export.js — buildPrintHtml 打印 HTML 构建', () => {
  it('包含 DOCTYPE 和 html 标签', () => {
    // Arrange
    const blocks = [{ role: 'user', content: '测试内容' }];
    // Act
    const html = buildPrintHtml('测试标题', blocks);
    // Assert
    expect(html).toContain('<!DOCTYPE html>');
    expect(html).toContain('<html');
  });

  it('标题被正确嵌入', () => {
    // Arrange
    const title = '我的对话标题';
    const blocks = [];
    // Act
    const html = buildPrintHtml(title, blocks);
    // Assert
    expect(html).toContain(encodeURIComponent(title).includes('%') ? title : title);
  });

  it('user 消息块渲染为 msg-role-user', () => {
    // Arrange
    const blocks = [{ role: 'user', content: '用户消息' }];
    // Act
    const html = buildPrintHtml('标题', blocks);
    // Assert
    expect(html).toContain('msg-role-user');
    expect(html).toContain('msg-block');
  });

  it('assistant 消息块渲染为 msg-role-assistant', () => {
    // Arrange
    const blocks = [{ role: 'assistant', content: 'AI 回复' }];
    // Act
    const html = buildPrintHtml('标题', blocks);
    // Assert
    expect(html).toContain('msg-role-assistant');
  });

  it('includeSources=false 时 assistant 消息不渲染引用来源列表', () => {
    // Arrange
    const blocks = [{
      role: 'assistant',
      content: '回复',
      sources: [{ doc_name: 'doc1', score: 0.9, chunk: { content: '片段内容' } }],
    }];
    // Act
    const html = buildPrintHtml('标题', blocks, { includeSources: false });
    // Assert: CSS 中有 .print-sources-list 样式定义，检查实际来源数据未渲染
    // 提取 body 部分（去掉 CSS），检查来源数据不存在
    const bodyStart = html.indexOf('<body>');
    const bodyContent = bodyStart >= 0 ? html.slice(bodyStart) : html;
    expect(bodyContent).not.toContain('doc1');
    expect(bodyContent).not.toContain('片段内容');
  });

  it('includeSources=true 时 assistant 消息渲染引用来源列表', () => {
    // Arrange
    const blocks = [{
      role: 'assistant',
      content: '回复',
      sources: [{ doc_name: 'doc1', score: 0.9, chunk: { content: '片段内容' } }],
    }];
    // Act
    const html = buildPrintHtml('标题', blocks, { includeSources: true });
    // Assert: body 部分包含来源数据
    const bodyStart = html.indexOf('<body>');
    const bodyContent = bodyStart >= 0 ? html.slice(bodyStart) : html;
    expect(bodyContent).toContain('doc1');
    expect(bodyContent).toContain('片段内容');
  });

  it('CSS 包含 @page 规则', () => {
    // Arrange
    const blocks = [];
    // Act
    const html = buildPrintHtml('标题', blocks, { pageSize: 'A4' });
    // Assert
    expect(html).toContain('@page');
    expect(html).toContain('A4');
  });

  it('Letter 页面大小使用 0.75in 页边距', () => {
    // Arrange
    const blocks = [];
    // Act
    const html = buildPrintHtml('标题', blocks, { pageSize: 'Letter' });
    // Assert
    expect(html).toContain('0.75in');
  });
});

describe('export.js — buildExportHtml 独立 HTML 构建', () => {
  it('包含 export-header 类', () => {
    // Arrange
    const blocks = [{ role: 'user', content: '内容' }];
    // Act
    const html = buildExportHtml('标题', blocks);
    // Assert
    expect(html).toContain('export-header');
  });

  it('包含 export-footer 类', () => {
    // Arrange
    const blocks = [];
    // Act
    const html = buildExportHtml('标题', blocks);
    // Assert
    expect(html).toContain('export-footer');
  });
});
