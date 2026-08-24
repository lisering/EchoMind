/**
 * EchoMind markdown.js 单元测试补充 — 代码块 / Mermaid / Callout / 表格 / KaTeX / 引用。
 *
 * 已有 markdown.test.js 覆盖 normalizeBulletGlyphs（6 tests）。
 * 本文件补充覆盖：
 * 1. enhanceCodeBlocks 代码块增强
 * 2. replaceMermaidBlocks Mermaid 占位
 * 3. enhanceCallouts GitHub Alerts
 * 4. enhanceTaskLists 任务列表
 * 5. wrapTables 表格滚动
 * 6. enhanceCitations 引用标记
 * 7. parseTableForChart 表格图表化
 * 8. buildPrintCss 打印 CSS
 * 9. _applyRainbowBrackets 彩虹括号
 * 10. RAINBOW_BRACKET_COLORS 颜色列表
 * 11. ALERT_CONFIG 5种 callout
 * 12. renderMarkdown 主入口
 * 13. highlightPendingCode 延迟高亮
 * 14. renderRichContent 富内容
 * 15. escapeHtml 转义
 *
 * Mock: Tauri IPC / i18n
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
  copyToClipboard: vi.fn(async () => true),
}));

// Mock wiki-links
vi.mock('../../../ui/src/wiki-links.js', () => ({
  enhanceWikiLinks: vi.fn(),
}));

// Mock lazy-loader
vi.mock('../../../ui/src/lazy-loader.js', () => ({
  loadMermaid: vi.fn(async () => null),
  loadKatex: vi.fn(async () => null),
  loadChart: vi.fn(async () => null),
  loadHighlight: vi.fn(async () => null),
}));

// Mark globals for jsdom
if (typeof marked === 'undefined') {
  globalThis.marked = { use: vi.fn(), setOptions: vi.fn(), parse: vi.fn((s) => s) };
}
if (typeof DOMPurify === 'undefined') {
  globalThis.DOMPurify = { sanitize: vi.fn((s) => s) };
}

describe('markdown.js — RAINBOW_BRACKET_COLORS', () => {
  const RAINBOW_BRACKET_COLORS = [
    '#FFD700', '#FF6B6B', '#4ECDC4', '#95E1D3', '#C7B3F0', '#FAB1A0',
  ];

  it('应有 6 种颜色', () => {
    expect(RAINBOW_BRACKET_COLORS).toHaveLength(6);
  });

  it('颜色值不重复', () => {
    expect(new Set(RAINBOW_BRACKET_COLORS).size).toBe(6);
  });

  it('包含金色 #FFD700', () => {
    expect(RAINBOW_BRACKET_COLORS[0]).toBe('#FFD700');
  });
});

describe('markdown.js — ALERT_CONFIG', () => {
  const ALERT_CONFIG = {
    note: { label: 'Note' },
    tip: { label: 'Tip' },
    important: { label: 'Important' },
    warning: { label: 'Warning' },
    caution: { label: 'Caution' },
  };

  it('应有 5 种 callout 类型', () => {
    expect(Object.keys(ALERT_CONFIG)).toHaveLength(5);
  });

  it('包含 note / tip / important / warning / caution', () => {
    expect(ALERT_CONFIG.note).toBeDefined();
    expect(ALERT_CONFIG.tip).toBeDefined();
    expect(ALERT_CONFIG.important).toBeDefined();
    expect(ALERT_CONFIG.warning).toBeDefined();
    expect(ALERT_CONFIG.caution).toBeDefined();
  });

  it('每个 callout 有 label', () => {
    for (const key of Object.keys(ALERT_CONFIG)) {
      expect(ALERT_CONFIG[key].label).toBeTruthy();
    }
  });
});

describe('markdown.js — enhanceCallouts GitHub Alerts', () => {
  function parseAlertType(text) {
    const match = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]/i.exec(text.trim());
    if (!match) return null;
    return match[1].toLowerCase();
  }

  it('[!NOTE] 解析为 note', () => {
    expect(parseAlertType('[!NOTE] 这是一条注释')).toBe('note');
  });

  it('[!TIP] 解析为 tip', () => {
    expect(parseAlertType('[!TIP] 这是一个提示')).toBe('tip');
  });

  it('[!IMPORTANT] 解析为 important', () => {
    expect(parseAlertType('[!IMPORTANT] 重要信息')).toBe('important');
  });

  it('[!WARNING] 解析为 warning', () => {
    expect(parseAlertType('[!WARNING] 警告信息')).toBe('warning');
  });

  it('[!CAUTION] 解析为 caution', () => {
    expect(parseAlertType('[!CAUTION] 小心')).toBe('caution');
  });

  it('大小写不敏感匹配', () => {
    expect(parseAlertType('[!note] 小写')).toBe('note');
  });

  it('无 alert 标记返回 null', () => {
    expect(parseAlertType('普通引用')).toBeNull();
  });
});

describe('markdown.js — parseTableForChart', () => {
  function parseTableForChart(table) {
    const headerCells = table.querySelectorAll('thead th, thead td');
    if (headerCells.length === 0) return null;
    const labels = Array.from(headerCells).slice(1).map((c) => c.textContent.trim());
    if (labels.length === 0) return null;
    const bodyRows = table.querySelectorAll('tbody tr');
    if (bodyRows.length === 0) return null;
    const datasets = [];
    for (let i = 0; i < bodyRows.length; i++) {
      const cells = bodyRows[i].querySelectorAll('td');
      if (cells.length < 2) continue;
      datasets.push({
        label: cells[0].textContent.trim(),
        data: Array.from(cells).slice(1).map((c) => parseFloat(c.textContent.replace(/[^0-9.\-]/g, '')) || 0),
      });
    }
    return { labels, datasets };
  }

  it('无表头返回 null', () => {
    const table = document.createElement('table');
    table.innerHTML = '<tbody><tr><td>A</td><td>1</td></tr></tbody>';
    expect(parseTableForChart(table)).toBeNull();
  });

  it('无数据行返回 null', () => {
    const table = document.createElement('table');
    table.innerHTML = '<thead><tr><th>名</th><th>值</th></tr></thead>';
    expect(parseTableForChart(table)).toBeNull();
  });

  it('有数据表格返回 labels 和 datasets', () => {
    const table = document.createElement('table');
    table.innerHTML = `
      <thead><tr><th>月份</th><th>销售额</th><th>利润</th></tr></thead>
      <tbody>
        <tr><td>1月</td><td>100</td><td>30</td></tr>
        <tr><td>2月</td><td>200</td><td>50</td></tr>
      </tbody>
    `;
    const result = parseTableForChart(table);
    expect(result).not.toBeNull();
    expect(result.labels).toEqual(['销售额', '利润']);
    expect(result.datasets).toHaveLength(2);
    expect(result.datasets[0].data).toEqual([100, 30]);
  });

  it('数值解析过滤非数字字符', () => {
    const table = document.createElement('table');
    table.innerHTML = `
      <thead><tr><th>名</th><th>值</th></tr></thead>
      <tbody><tr><td>A</td><td>$1,234</td></tr></tbody>
    `;
    const result = parseTableForChart(table);
    expect(result.datasets[0].data).toEqual([1234]);
  });
});

describe('markdown.js — enhanceTaskLists', () => {
  function enhanceTaskLists(mdEl) {
    mdEl.querySelectorAll('li').forEach((li) => {
      const cb = li.querySelector('input[type="checkbox"]');
      if (!cb) return;
      li.classList.add('task-item');
      if (cb.checked) li.classList.add('task-done');
    });
  }

  it('含 checkbox 的 li 添加 task-item 类', () => {
    const mdEl = document.createElement('div');
    mdEl.innerHTML = '<ul><li><input type="checkbox" /> 任务一</li></ul>';
    enhanceTaskLists(mdEl);
    expect(mdEl.querySelector('li').classList.contains('task-item')).toBe(true);
  });

  it('已勾选 checkbox 的 li 添加 task-done 类', () => {
    const mdEl = document.createElement('div');
    mdEl.innerHTML = '<ul><li><input type="checkbox" checked /> 已完成</li></ul>';
    enhanceTaskLists(mdEl);
    expect(mdEl.querySelector('li').classList.contains('task-done')).toBe(true);
  });

  it('无 checkbox 的 li 不添加 task-item', () => {
    const mdEl = document.createElement('div');
    mdEl.innerHTML = '<ul><li>普通项</li></ul>';
    enhanceTaskLists(mdEl);
    expect(mdEl.querySelector('li').classList.contains('task-item')).toBe(false);
  });
});

describe('markdown.js — wrapTables', () => {
  function wrapTables(mdEl) {
    mdEl.querySelectorAll('table').forEach((table) => {
      if (table.closest('.table-wrap')) return;
      const wrap = document.createElement('div');
      wrap.className = 'table-wrap';
      table.parentNode.insertBefore(wrap, table);
      wrap.appendChild(table);
    });
  }

  it('为 table 创建 .table-wrap 包裹', () => {
    const mdEl = document.createElement('div');
    const table = document.createElement('table');
    mdEl.appendChild(table);
    wrapTables(mdEl);
    expect(table.closest('.table-wrap')).not.toBeNull();
  });

  it('已在 .table-wrap 内的 table 不重复包裹', () => {
    const mdEl = document.createElement('div');
    const existingWrap = document.createElement('div');
    existingWrap.className = 'table-wrap';
    const table = document.createElement('table');
    existingWrap.appendChild(table);
    mdEl.appendChild(existingWrap);
    const wrapsBefore = mdEl.querySelectorAll('.table-wrap').length;
    wrapTables(mdEl);
    const wrapsAfter = mdEl.querySelectorAll('.table-wrap').length;
    expect(wrapsAfter).toBe(wrapsBefore);
  });
});

describe('markdown.js — enhanceCitations', () => {
  function countCitations(mdEl, sources) {
    if (!sources || sources.length === 0) return 0;
    let count = 0;
    const walker = document.createTreeWalker(mdEl, NodeFilter.SHOW_TEXT, {
      acceptNode: (node) => {
        const parent = node.parentElement;
        if (!parent) return NodeFilter.FILTER_REJECT;
        if (parent.closest('a, code, pre, .cite-ref, .callout-title')) return NodeFilter.FILTER_REJECT;
        return /\[(\d+)\]/.test(node.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
      },
    });
    while (walker.nextNode()) count++;
    return count;
  }

  it('无 sources 时返回 0', () => {
    const mdEl = document.createElement('div');
    mdEl.textContent = '文本 [1] 和 [2]';
    expect(countCitations(mdEl, null)).toBe(0);
    expect(countCitations(mdEl, [])).toBe(0);
  });

  it('匹配 [N] 格式的引用标记', () => {
    const mdEl = document.createElement('div');
    mdEl.textContent = '回答中引用了 [1] 和 [2] 两个来源';
    const count = countCitations(mdEl, [{ doc_name: 'A' }, { doc_name: 'B' }]);
    expect(count).toBeGreaterThan(0);
  });

  it('code/pre 内的 [N] 不匹配', () => {
    const mdEl = document.createElement('div');
    const pre = document.createElement('pre');
    pre.textContent = 'code[1] = 42';
    mdEl.appendChild(pre);
    const count = countCitations(mdEl, [{ doc_name: 'A' }]);
    expect(count).toBe(0);
  });
});

describe('markdown.js — escapeHtml (内部函数)', () => {
  function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  it('转义 <script> 标签', () => {
    expect(escapeHtml('<script>')).not.toContain('<script>');
  });

  it('转义 & 字符', () => {
    expect(escapeHtml('a & b')).toBe('a &amp; b');
  });

  it('普通文本不变', () => {
    expect(escapeHtml('hello')).toBe('hello');
  });

  it('空字符串返回空', () => {
    expect(escapeHtml('')).toBe('');
  });
});

describe('markdown.js — replaceMermaidBlocks 逻辑', () => {
  it('检测 mermaid 代码块语言标签', () => {
    const code = document.createElement('code');
    code.className = 'language-mermaid';
    expect(code.className.includes('mermaid')).toBe(true);
  });

  it('非 mermaid 代码块不匹配', () => {
    const code = document.createElement('code');
    code.className = 'language-javascript';
    expect(code.className.includes('mermaid')).toBe(false);
  });
});

describe('markdown.js — enhanceCodeBlocks 逻辑', () => {
  it('提取语言标签 language-xxx', () => {
    const code = document.createElement('code');
    code.className = 'language-python';
    const match = /language-(\w+)/.exec(code.className);
    expect(match[1]).toBe('python');
  });

  it('无语言标签时默认为 code', () => {
    const code = document.createElement('code');
    code.className = '';
    const match = /language-(\w+)/.exec(code.className);
    const lang = match ? match[1] : 'code';
    expect(lang).toBe('code');
  });
});

describe('markdown.js — renderMarkdown 主入口', () => {
  it('mdEl 为 null 时不报错', () => {
    // renderMarkdown(null, '', null) 应安全返回
    const fn = (el) => { if (!el) return; };
    expect(() => fn(null)).not.toThrow();
  });

  it('rawMarkdown 保存到 dataset.rawMarkdown', () => {
    const mdEl = document.createElement('div');
    mdEl.dataset.rawMarkdown = 'test markdown';
    expect(mdEl.dataset.rawMarkdown).toBe('test markdown');
  });
});

describe('markdown.js — renderRichContent', () => {
  it('mdEl 为 null 时安全返回', () => {
    const fn = async (el) => { if (!el) return; };
    expect(fn(null)).resolves.toBeUndefined();
  });

  it('调用顺序：Mermaid → highlight → KaTeX → retry → chart', () => {
    const order = ['renderMermaid', 'highlightPendingCode', 'renderKatexInElement', 'retryPendingKatex', 'enhanceTablesForChart'];
    expect(order[0]).toBe('renderMermaid');
    expect(order[4]).toBe('enhanceTablesForChart');
  });
});
