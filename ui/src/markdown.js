/**
 * EchoMind Markdown 渲染引擎 — marked → DOMPurify → highlight.js 链路。
 *
 * 安全铁律：模型输出必须经 DOMPurify 白名单消毒（REQ-SEC-002）。
 *
 * 富内容支持：
 * - 代码块：语言标签 + 复制按钮（highlight.js）
 * - Mermaid 图表：流式占位 → chat_done 后渲染为 SVG
 * - KaTeX 公式：行内 $...$ / 块级 $$...$$
 * - Chart.js：Markdown 表格 → 可切换图表视图
 * - GitHub Alerts：[!NOTE]/[!TIP]/[!IMPORTANT]/[!WARNING]/[!CAUTION]
 * - 行内引用标记：[1] [2] → 可点击 cite-ref
 * - ==高亮标记== → <mark>
 * - 任务列表：GFM checkbox + 完成态
 */

import { $, copyToClipboard } from './utils.js';
import { t } from './i18n.js';
import { enhanceWikiLinks } from './wiki-links.js';
import { loadMermaid, loadKatex, loadChart, loadHighlight } from './lazy-loader.js';

// ============================================================
// Mermaid 图片查看器（P1-4：DeepSeek 风格 Fancy Box）
// ============================================================

/**
 * 弹出全屏 Mermaid SVG 查看器。
 * @param {string} svgHtml - SVG HTML 内容
 */
function _showMermaidLightbox(svgHtml) {
  // 移除已有 lightbox
  const existing = document.getElementById('mermaidLightbox');
  if (existing) { existing.remove(); return; }

  const overlay = document.createElement('div');
  overlay.id = 'mermaidLightbox';
  overlay.className = 'fixed inset-0 z-[9999] bg-black/80 flex items-center justify-center cursor-zoom-out';
  overlay.style.animation = 'fadeInZoomExpand 0.2s ease-out';

  const container = document.createElement('div');
  container.className = 'max-w-[90vw] max-h-[90vh] overflow-auto p-4';
  container.innerHTML = svgHtml;
  // SVG 自适应缩放
  const svg = container.querySelector('svg');
  if (svg) {
    svg.style.maxWidth = '90vw';
    svg.style.maxHeight = '90vh';
    svg.style.width = 'auto';
    svg.style.height = 'auto';
  }

  overlay.appendChild(container);

  // 点击空白处关闭
  overlay.onclick = (e) => {
    if (e.target === overlay) overlay.remove();
  };

  // ESC 关闭
  const escHandler = (e) => {
    if (e.key === 'Escape') {
      overlay.remove();
      document.removeEventListener('keydown', escHandler);
    }
  };
  document.addEventListener('keydown', escHandler);

  document.body.appendChild(overlay);
}

// ============================================================
// 代码块增强
// ============================================================

/** 彩虹括号颜色列表（6 色循环） */
const RAINBOW_BRACKET_COLORS = [
  '#FFD700', // gold
  '#FF6B6B', // coral
  '#4ECDC4', // teal
  '#95E1D3', // mint
  '#C7B3F0', // lavender
  '#FAB1A0', // peach
];

/**
 * P1-5：为代码块中的括号字符添加彩虹颜色层级。
 * 使用 TreeWalker 遍历文本节点，将 ()[]{} 替换为带颜色的 span。
 * 已被 hljs 标记的 token 内的括号也会被着色。
 * @param {HTMLElement} codeEl - <code> 元素
 */
function _applyRainbowBrackets(codeEl) {
  if (codeEl.dataset.rainbowApplied) return;
  codeEl.dataset.rainbowApplied = 'true';

  const walker = document.createTreeWalker(codeEl, NodeFilter.SHOW_TEXT, null);
  const textNodes = [];
  let node;
  while ((node = walker.nextNode())) {
    textNodes.push(node);
  }

  for (const textNode of textNodes) {
    const text = textNode.textContent;
    if (!text || !/[\(\)\[\]\{\}]/.test(text)) continue;

    const fragment = document.createDocumentFragment();
    let depth = 0;
    for (const char of text) {
      if (char === '(' || char === '[' || char === '{') {
        const span = document.createElement('span');
        span.style.color = RAINBOW_BRACKET_COLORS[depth % RAINBOW_BRACKET_COLORS.length];
        span.textContent = char;
        fragment.appendChild(span);
        depth++;
      } else if (char === ')' || char === ']' || char === '}') {
        depth = Math.max(0, depth - 1);
        const span = document.createElement('span');
        span.style.color = RAINBOW_BRACKET_COLORS[depth % RAINBOW_BRACKET_COLORS.length];
        span.textContent = char;
        fragment.appendChild(span);
      } else {
        fragment.appendChild(document.createTextNode(char));
      }
    }
    textNode.parentNode.replaceChild(fragment, textNode);
  }
}

/**
 * 增强代码块：包裹 .code-block + 头部栏（语言标签 + 复制按钮）。
 *
 * # 性能（V3.1 P2-1）
 * `skipHeavy=true`（流式期间）跳过 hljs 语法高亮与彩虹括号：innerHTML 每帧全量
 * 重建使 dataset 标记失效，二者每帧重跑是 O(n²) 热点。chat_done 后由
 * `renderRichContent → highlightPendingCode` 一次性补齐。
 *
 * @param {HTMLElement} mdEl - Markdown 内容容器
 * @param {boolean} [skipHeavy=false] - 流式期间跳过重活（hljs/彩虹括号）
 */
export function enhanceCodeBlocks(mdEl, skipHeavy = false) {
  mdEl.querySelectorAll('pre').forEach((pre) => {
    if (pre.closest('.mermaid-source, .code-block')) return;
    const code = pre.querySelector('code');
    if (!code) return;
    if (!skipHeavy && !code.dataset.highlighted && typeof hljs !== 'undefined') {
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      try { hljs.highlightElement(code); } catch (_) { /* 半截语法容错 */ }
    }
    // P1-5：彩虹括号 — 为括号字符添加颜色层级（流式期间跳过，同上）
    if (!skipHeavy) {
      _applyRainbowBrackets(/** @type {HTMLElement} */(code));
    }
    const langMatch = /language-(\w+)/.exec(code.className);
    const lang = langMatch ? langMatch[1] : 'code';
    const wrapper = document.createElement('div');
    wrapper.className = 'code-block';
    const header = document.createElement('div');
    header.className = 'code-header';
    const langLabel = document.createElement('span');
    langLabel.className = 'code-lang';
    langLabel.textContent = lang;
    header.appendChild(langLabel);
    const btn = document.createElement('button');
    btn.className = 'copy-btn';
    btn.textContent = t('markdown.copy');
    btn.onclick = async () => {
      // REQ-IX-003：统一使用 copyToClipboard（含非安全上下文 fallback）
      const ok = await copyToClipboard(code.innerText ?? '');
      if (ok) {
        btn.textContent = t('markdown.copied');
        setTimeout(() => (btn.textContent = t('markdown.copy')), 1500);
      }
    };
    header.appendChild(btn);
    pre.parentNode.insertBefore(wrapper, pre);
    wrapper.appendChild(header);
    wrapper.appendChild(pre);
  });
}

// ============================================================
// Mermaid 图表占位
// ============================================================

/**
 * 将 mermaid 代码块替换为占位元素（流式过程中不渲染，chat_done 后渲染）。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export function replaceMermaidBlocks(mdEl) {
  mdEl.querySelectorAll('pre code[class*="mermaid"]').forEach((code) => {
    const pre = code.parentElement;
    if (!pre || pre.tagName !== 'PRE') return;
    const raw = code.textContent;
    const div = document.createElement('div');
    div.className = 'mermaid-source';
    div.setAttribute('data-raw', raw);
    const srcPre = document.createElement('pre');
    srcPre.className = 'text-xs text-slate-400';
    srcPre.textContent = raw;
    div.appendChild(srcPre);
    const hint = document.createElement('div');
    hint.className = 'mermaid-placeholder text-xs mt-1';
    hint.textContent = t('markdown.mermaid_loading');
    div.appendChild(hint);
    pre.replaceWith(div);
  });
}

/**
 * 渲染 .mermaid-source 占位元素为 SVG（chat_done 后调用）。
 * 延迟加载 mermaid.min.js（3.4MB）—— 仅在有 mermaid 代码块时才加载。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export async function renderMermaid(mdEl) {
  const sources = mdEl.querySelectorAll('.mermaid-source:not(.mermaid-rendered):not(.mermaid-error)');
  if (sources.length === 0) return;
  const mmd = await loadMermaid();
  if (!mmd) return;
  for (const el of sources) {
    const raw = el.getAttribute('data-raw') || '';
    try {
      const id = 'mmd-' + Math.random().toString(36).slice(2, 10);
      const { svg } = await mmd.render(id, raw);
      el.innerHTML = svg;
      el.classList.add('mermaid-rendered');
      // P1-4：Mermaid 图片查看器 — 点击 SVG 弹出全屏放大查看
      el.style.cursor = 'zoom-in';
      el.addEventListener('click', (ev) => {
        if (ev.target.closest('a')) return; // 不拦截链接点击
        _showMermaidLightbox(el.innerHTML);
      });
    } catch (_) {
      el.classList.add('mermaid-error');
      el.innerHTML = '';
      const errMsg = document.createElement('div');
      errMsg.className = 'text-red-400 text-xs mb-1';
      errMsg.textContent = t('markdown.mermaid_error');
      el.appendChild(errMsg);
      const srcPre = document.createElement('pre');
      srcPre.className = 'text-xs text-slate-400 overflow-x-auto';
      srcPre.textContent = raw;
      el.appendChild(srcPre);
    }
  }
}

// ============================================================
// GitHub Alerts (Callout)
// ============================================================

const ALERT_CONFIG = {
  note: { icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>', label: 'Note' },
  tip: { icon: '💡', label: 'Tip' },
  important: { icon: '❗', label: 'Important' },
  warning: { icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>', label: 'Warning' },
  caution: { icon: '🔴', label: 'Caution' },
};

/**
 * 检测 blockquote 中的 [!NOTE]/[!TIP] 等，转换为 callout div。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export function enhanceCallouts(mdEl) {
  mdEl.querySelectorAll('blockquote').forEach((bq) => {
    const firstP = bq.querySelector('p');
    if (!firstP) return;
    const text = firstP.textContent.trim();
    const match = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]/i.exec(text);
    if (!match) return;
    const type = match[1].toLowerCase();
    const cfg = ALERT_CONFIG[type];
    if (!cfg) return;
    const callout = document.createElement('div');
    callout.className = 'callout callout-' + type;
    const title = document.createElement('div');
    title.className = 'callout-title';
    title.innerHTML = '<span>' + cfg.icon + '</span><span>' + cfg.label + '</span>';
    callout.appendChild(title);
    const rest = text.slice(match[0].length).trim();
    if (rest) { firstP.textContent = rest; }
    else { firstP.remove(); }
    const body = document.createElement('div');
    body.className = 'callout-body';
    while (bq.firstChild) body.appendChild(bq.firstChild);
    callout.appendChild(body);
    bq.replaceWith(callout);
  });
}

// ============================================================
// 行内引用标记 [1] [2] → 可点击 cite-ref
// ============================================================

/**
 * 将文本中的 [1] [2] 等引用标记转换为可点击的 .cite-ref 元素。
 * REQ-RAG-016 AC-3：cite-ref 与 source-card 双向高亮 — 点击/悬停 cite-ref 高亮对应来源卡片，
 * 悬停 source-card 高亮对应 cite-ref。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 * @param {Array|null} sources - 引用来源列表
 */
export function enhanceCitations(mdEl, sources) {
  if (!sources || sources.length === 0) return;
  const walker = document.createTreeWalker(mdEl, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;
      if (parent.closest('a, code, pre, .cite-ref, .callout-title')) return NodeFilter.FILTER_REJECT;
      return /\[(\d+)\]/.test(node.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
    },
  });
  const textNodes = [];
  let node;
  while ((node = walker.nextNode())) textNodes.push(node);
  for (const textNode of textNodes) {
    const text = textNode.nodeValue;
    const regex = /\[(\d+)\]/g;
    let lastIndex = 0;
    let match;
    const fragments = [];
    while ((match = regex.exec(text)) !== null) {
      if (match.index > lastIndex) {
        fragments.push(document.createTextNode(text.slice(lastIndex, match.index)));
      }
      const num = parseInt(match[1], 10);
      if (num >= 1 && num <= sources.length) {
        const ref = document.createElement('span');
        ref.className = 'cite-ref';
        ref.textContent = '[' + num + ']';
        ref.setAttribute('data-cite', String(num));
        // REQ-RAG-016 AC-3：悬停 cite-ref → 高亮对应来源卡片
        ref.onmouseenter = () => {
          const block = mdEl.parentElement;
          if (!block) return;
          const list = block.querySelector('.sources-list');
          if (!list) return;
          // 查找对应编号的 source-card（按排序后的显示位置）
          const cards = list.querySelectorAll('.source-card');
          // 根据原始 sourceIndex 匹配（card.dataset.sourceIndex 是排序前的位置）
          const origIdx = num - 1;
          let matchedCard = null;
          cards.forEach((card) => {
            if (card.dataset.sourceIndex === String(origIdx)) {
              matchedCard = card;
              card.style.background = 'rgba(56,189,248,0.25)';
              card.style.borderColor = 'rgba(56,189,248,0.4)';
            }
          });
        };
        ref.onmouseleave = () => {
          const block = mdEl.parentElement;
          if (!block) return;
          const list = block.querySelector('.sources-list');
          if (!list) return;
          list.querySelectorAll('.source-card').forEach((card) => {
            card.style.background = '';
            card.style.borderColor = '';
          });
        };
        ref.onclick = () => {
          const block = mdEl.parentElement;
          if (!block) return;
          const toggle = block.querySelector('.sources-toggle');
          const list = block.querySelector('.sources-list');
          if (toggle && list && list.style.display === 'none') {
            toggle.classList.add('expanded');
            toggle.querySelector('svg').style.transform = 'rotate(180deg)';
            list.style.display = 'flex';
          }
          // 高亮对应卡片（动画效果）
          if (list) {
            const origIdx = num - 1;
            list.querySelectorAll('.source-card').forEach((card) => {
              if (card.dataset.sourceIndex === String(origIdx)) {
                card.style.transition = 'background 0.3s';
                card.style.background = 'rgba(56,189,248,0.4)';
                setTimeout(() => { card.style.background = ''; }, 1000);
              }
            });
          }
        };
        fragments.push(ref);
      } else {
        fragments.push(document.createTextNode(match[0]));
      }
      lastIndex = regex.lastIndex;
    }
    if (fragments.length > 0) {
      if (lastIndex < text.length) {
        fragments.push(document.createTextNode(text.slice(lastIndex)));
      }
      const parent = textNode.parentElement;
      if (parent) {
        fragments.forEach((f) => parent.insertBefore(f, textNode));
        parent.removeChild(textNode);
      }
    }
  }
}

// ============================================================
// 任务列表
// ============================================================

/**
 * 增强任务列表：标记 task-item / task-done 类。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export function enhanceTaskLists(mdEl) {
  mdEl.querySelectorAll('li').forEach((li) => {
    const cb = li.querySelector('input[type="checkbox"]');
    if (!cb) return;
    li.classList.add('task-item');
    if (cb.checked) li.classList.add('task-done');
    let next = cb.nextSibling;
    if (next && next.nodeType === Node.TEXT_NODE) {
      const span = document.createElement('span');
      span.textContent = next.textContent;
      next.replaceWith(span);
    }
  });
}

// ============================================================
// 表格滚动包裹
// ============================================================

/**
 * 为表格添加滚动包裹容器。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export function wrapTables(mdEl) {
  mdEl.querySelectorAll('table').forEach((table) => {
    if (table.closest('.table-wrap')) return;
    const wrap = document.createElement('div');
    wrap.className = 'table-wrap';
    table.parentNode.insertBefore(wrap, table);
    wrap.appendChild(table);
  });
}

// ============================================================
// KaTeX 数学公式
// ============================================================

/**
 * 在 DOM 元素内渲染 KaTeX 数学公式（REQ-VIZ-002）。
 * 延迟加载 katex.min.js + CSS + mhchem 插件（~280KB）—— 仅在有 $ 公式时才加载。
 * @param {HTMLElement} el - Markdown 内容容器元素
 * @returns {Promise<void>}
 */
export async function renderKatexInElement(el) {
  // 延迟加载 KaTeX（Promise 缓存，重复调用返回同一 Promise）
  const katex = await loadKatex();
  if (!katex) return;
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;
      if (parent.closest('pre, code, .katex, .mermaid-source')) return NodeFilter.FILTER_REJECT;
      return /\$/.test(node.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
    },
  });
  const textNodes = [];
  let node;
  while ((node = walker.nextNode())) textNodes.push(node);
  for (const textNode of textNodes) {
    const text = textNode.nodeValue;
    const regex = /\$\$([\s\S]+?)\$\$|\$([^\$\n]+?)\$/g;
    let lastIndex = 0;
    let match;
    const fragments = [];
    while ((match = regex.exec(text)) !== null) {
      if (match.index > lastIndex) {
        fragments.push(document.createTextNode(text.slice(lastIndex, match.index)));
      }
      const isBlock = !!match[1];
      const latex = (match[1] || match[2] || '').trim();
      try {
        const html = katex.renderToString(latex, { throwOnError: true, displayMode: isBlock, output: 'html' });
        const container = document.createElement(isBlock ? 'div' : 'span');
        container.className = isBlock ? 'katex-block' : 'katex-inline';
        container.innerHTML = html;
        fragments.push(container);
      } catch (_) {
        const pending = document.createElement(isBlock ? 'div' : 'span');
        pending.className = 'katex-pending';
        pending.setAttribute('data-latex', latex);
        pending.setAttribute('data-display', String(isBlock));
        pending.textContent = match[0];
        fragments.push(pending);
      }
      lastIndex = regex.lastIndex;
    }
    if (fragments.length > 0) {
      if (lastIndex < text.length) {
        fragments.push(document.createTextNode(text.slice(lastIndex)));
      }
      const parent = textNode.parentElement;
      if (parent) {
        fragments.forEach((f) => parent.insertBefore(f, textNode));
        parent.removeChild(textNode);
      }
    }
  }
}

/**
 * 重试渲染流式期间未能渲染的 KaTeX 公式（chat_done 后调用）。
 * @param {HTMLElement} mdEl - Markdown 内容容器
 * @returns {Promise<void>}
 */
export async function retryPendingKatex(mdEl) {
  const pendingKatex = mdEl.querySelectorAll('.katex-pending');
  if (pendingKatex.length === 0) return;
  const katex = await loadKatex();
  if (!katex) return;
  for (const kp of pendingKatex) {
    const latex = kp.getAttribute('data-latex') || '';
    const isBlock = kp.getAttribute('data-display') === 'true';
    try {
      const html = katex.renderToString(latex, { throwOnError: true, displayMode: isBlock, output: 'html' });
      kp.innerHTML = html;
      kp.classList.remove('katex-pending');
      kp.classList.add(isBlock ? 'katex-block' : 'katex-inline');
    } catch (_) {
      kp.classList.remove('katex-pending');
      kp.classList.add('katex-error');
      kp.textContent = t('markdown.katex_error') + latex;
    }
  }
}

// ============================================================
// Chart.js 表格 → 图表
// ============================================================

/**
 * 解析 Markdown 表格为 Chart.js 数据格式。
 * @param {HTMLTableElement} table - 表格元素
 * @returns {{labels: string[], datasets: Array}|null}
 */
export function parseTableForChart(table) {
  const headerCells = table.querySelectorAll('thead th, thead td');
  if (headerCells.length === 0) return null;
  const labels = Array.from(headerCells).slice(1).map((c) => c.textContent.trim());
  if (labels.length === 0) return null;
  const bodyRows = table.querySelectorAll('tbody tr');
  if (bodyRows.length === 0) return null;
  const datasets = [];
  const palette = ['#38BDF8', '#f97316', '#22c55e', '#a78bfa', '#f43f5e', '#eab308'];
  for (let i = 0; i < bodyRows.length; i++) {
    const cells = bodyRows[i].querySelectorAll('td');
    if (cells.length < 2) continue;
    datasets.push({
      label: cells[0].textContent.trim(),
      data: Array.from(cells).slice(1).map((c) => parseFloat(c.textContent.replace(/[^0-9.\-]/g, '')) || 0),
      backgroundColor: palette[i % palette.length],
      borderColor: palette[i % palette.length],
    });
  }
  return { labels, datasets };
}

/**
 * 用 Chart.js 渲染图表到 canvas。
 * @param {HTMLCanvasElement} canvas
 * @param {string} type - 图表类型（bar/line/pie）
 * @param {{labels: string[], datasets: Array}} data
 * @returns {unknown}
 */
function renderChart(canvas, type, data) {
  const config = {
    type,
    data: { labels: data.labels, datasets: data.datasets.map((ds) => ({ ...ds, borderWidth: 1 })) },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { labels: { color: '#94a3b8', font: { size: 11 } } },
        tooltip: { backgroundColor: '#1C1C1E', titleColor: '#E2E8F0', bodyColor: '#94a3b8' },
      },
      scales: type === 'pie' ? {} : {
        x: { ticks: { color: '#94a3b8', font: { size: 10 } }, grid: { color: 'rgba(255,255,255,0.08)' } },
        y: { ticks: { color: '#94a3b8', font: { size: 10 } }, grid: { color: 'rgba(255,255,255,0.08)' } },
      },
    },
  };
  return new Chart(canvas, config);
}

/**
 * 为 Markdown 表格注入「切换图表视图」按钮。
 * 延迟加载 chart.umd.min.js（204KB）—— 仅在有数据表格时才加载。
 * @param {HTMLElement} mdEl
 * @returns {Promise<void>}
 */
export async function enhanceTablesForChart(mdEl) {
  // 检测是否有可图表化的表格（有数值数据的表格）
  const tables = mdEl.querySelectorAll('table:not(.chart-enhanced)');
  if (tables.length === 0) return;
  // 预检：至少一个表格有数值数据才加载 Chart
  let hasChartData = false;
  for (const table of tables) {
    if (parseTableForChart(/** @type {HTMLTableElement} */ (table))) {
      hasChartData = true;
      break;
    }
  }
  if (!hasChartData) {
    tables.forEach((tbl) => tbl.classList.add('chart-enhanced'));
    return;
  }
  const Chart = await loadChart();
  if (!Chart) return;
  if (!Chart._echomindDefaults) {
    Chart.defaults.color = '#94a3b8';
    Chart.defaults.borderColor = 'rgba(255,255,255,0.08)';
    Chart._echomindDefaults = true;
  }
  for (const table of tables) {
    table.classList.add('chart-enhanced');
    const data = parseTableForChart(/** @type {HTMLTableElement} */ (table));
    if (!data || data.labels.length === 0 || data.datasets.length === 0) continue;
    const btn = document.createElement('button');
    btn.className = 'chart-toggle';
    btn.textContent = t('markdown.chart_toggle_chart');
    let chartInstance = null;
    let chartContainer = null;
    btn.addEventListener('click', () => {
      if (chartContainer) {
        chartContainer.remove();
        chartContainer = null;
        if (chartInstance) { chartInstance.destroy(); chartInstance = null; }
        btn.textContent = t('markdown.chart_toggle_chart');
        table.style.display = '';
        return;
      }
      table.style.display = 'none';
      chartContainer = document.createElement('div');
      chartContainer.className = 'chart-container';
      const typeBar = document.createElement('div');
      typeBar.className = 'chart-type-bar';
      const types = [
        { type: 'bar', label: t('markdown.chart_bar') },
        { type: 'line', label: t('markdown.chart_line') },
        { type: 'pie', label: t('markdown.chart_pie') },
      ];
      for (const t of types) {
        const tb = document.createElement('button');
        tb.textContent = t.label;
        tb.dataset.type = t.type;
        tb.addEventListener('click', () => {
          typeBar.querySelectorAll('button').forEach((b) => b.classList.remove('active'));
          tb.classList.add('active');
          if (chartInstance) chartInstance.destroy();
          chartInstance = renderChart(canvas, t.type, data);
        });
        typeBar.appendChild(tb);
      }
      const canvas = document.createElement('canvas');
      chartContainer.appendChild(typeBar);
      chartContainer.appendChild(canvas);
      table.parentNode.insertBefore(chartContainer, table.nextSibling);
      typeBar.querySelector('button').click();
      btn.textContent = t('markdown.chart_toggle_table');
    });
    table.parentNode.insertBefore(btn, table);
  }
}

// ============================================================
// 主渲染入口
// ============================================================

/**
 * 将 LLM 常用的字面圆点符号（• ● ◦ ▪ ‣ ⁃）行转换为标准 Markdown 列表项。
 *
 * 背景：LLM 常输出「• 项目」而非「- 项目」，字面符号以字体字形渲染，
 * 在不同字体下呈椭圆/不圆形状；转换为列表项后由 CSS 圆点（.md ul li::before）
 * 渲染为纯正圆形。代码块（``` ~~~）内不转换。
 *
 * @param {string} markdown - 原始 Markdown 文本
 * @returns {string} 转换后的 Markdown
 */
export function normalizeBulletGlyphs(markdown) {
  let inFence = false;
  return markdown.split('\n').map((line) => {
    if (/^\s*(```|~~~)/.test(line)) inFence = !inFence;
    if (!inFence && /^\s*[•●◦▪‣⁃](\s|$)/.test(line)) {
      // 保留前导缩进（嵌套列表），仅替换圆点符号本身
      return line.replace(/^(\s*)[•●◦▪‣⁃](\s|$)/, '$1- ');
    }
    return line;
  }).join('\n');
}

/**
 * 将 currentRawMarkdown 经 marked → DOMPurify → highlight.js 链路渲染到当前 assistant Block。
 * 安全铁律：模型输出必须经 DOMPurify 白名单消毒（REQ-SEC-002）。
 *
 * # 性能优化（流式渲染）
 * `skipHeavy=true`（流式 token 到达期间）跳过 KaTeX 渲染：
 * 公式在流式中间态必然不完整，渲染必然失败并产生 pending 占位，且每帧重复尝试
 * 是 O(n²) 热点。`chat_done` 后 `renderRichContent` 的 `retryPendingKatex` 兜底重试。
 *
 * @param {HTMLElement} mdEl - .md 内容容器
 * @param {string} rawMarkdown - 原始 Markdown 文本
 * @param {Array|null} sources - 引用来源列表
 * @param {boolean} [skipHeavy=false] - 流式期间跳过重活（KaTeX）
 */
export function renderMarkdown(mdEl, rawMarkdown, sources, skipHeavy = false) {
  if (!mdEl) return;
  // 保存原始 Markdown 供编辑分支/轮播等读取（如 commitEdit 注册 v1 答案）
  mdEl.dataset.rawMarkdown = rawMarkdown;
  mdEl.innerHTML = DOMPurify.sanitize(marked.parse(normalizeBulletGlyphs(rawMarkdown)));

  // Mermaid 代码块 → 占位 div
  replaceMermaidBlocks(mdEl);

  // 代码块增强（流式期间跳过 hljs 高亮/彩虹括号 — V3.1 P2-1）
  enhanceCodeBlocks(mdEl, skipHeavy);

  // GitHub Alerts
  enhanceCallouts(mdEl);

  // 任务列表
  enhanceTaskLists(mdEl);

  // 表格滚动包裹
  wrapTables(mdEl);

  // 行内引用标记
  enhanceCitations(mdEl, sources);

  // Wiki 双向链接（REQ-ING-020 [[wiki-link]] 渲染为可点击链接）
  enhanceWikiLinks(mdEl);

  // KaTeX 数学公式（流式期间跳过，chat_done 后由 retryPendingKatex 兜底）
  if (!skipHeavy) {
    // 延迟加载：KaTeX 异步加载后渲染（fire-and-forget）
    renderKatexInElement(mdEl).catch(() => {});
    // 延迟加载：highlight.js 异步加载后高亮代码块（fire-and-forget）
    highlightPendingCode(mdEl).catch(() => {});
  }
}

/**
 * 延迟加载 highlight.js 并高亮所有未高亮的代码块。
 *
 * 将 highlight.js（124KB）从 eager 加载改为按需加载：
 * - 首次调用时加载 hljs 库
 * - 后续调用从 Promise 缓存直接返回
 * - 未加载前代码块仍正常显示（无语法着色），加载后自动高亮
 *
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export async function highlightPendingCode(mdEl) {
  const pending = mdEl.querySelectorAll('pre code:not([data-highlighted])');
  if (pending.length === 0) return;
  const hljs = await loadHighlight();
  if (!hljs) return;
  pending.forEach((code) => {
    // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
    try { hljs.highlightElement(code); } catch (_) { /* 半截语法容错 */ }
    // 流式期间彩虹括号被 skipHeavy 豁免（V3.1 P2-1）→ 此处一次性补齐
    _applyRainbowBrackets(/** @type {HTMLElement} */(code));
  });
}

/**
 * chat_done 后渲染富内容：Mermaid SVG + 重试 KaTeX + 表格图表。
 *
 * 流式期间 `renderMarkdown(skipHeavy=true)` 跳过 KaTeX；此处对完整文本
 * 做一次全量 KaTeX 渲染（renderKatexInElement），再重试仍失败的 pending 公式。
 * @param {HTMLElement} mdEl - .md 内容容器
 */
export async function renderRichContent(mdEl) {
  if (!mdEl) return;
  await renderMermaid(mdEl);
  await highlightPendingCode(mdEl);
  // 流式期间跳过 KaTeX → 此处全量渲染一次完整公式
  await renderKatexInElement(mdEl);
  await retryPendingKatex(mdEl);
  await enhanceTablesForChart(mdEl);
}
