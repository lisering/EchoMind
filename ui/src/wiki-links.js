/**
 * EchoMind Wiki 双向链接模块（REQ-ING-020 Markdown 笔记双向链接）。
 *
 * 功能：
 * 1. [[wiki-link]] 渲染为可点击链接（在 markdown.js 中调用 enhanceWikiLinks）
 * 2. 反向链接面板：显示哪些文档引用了当前文档
 * 3. 正向链接面板：显示当前文档引用了哪些文档
 *
 * 数据来源：后端 wiki_links 表（导入时自动解析 [[link]] 语法建立索引）
 */

import { invoke } from './ipc.js';
import { t } from './i18n.js';

/**
 * 将 Markdown 渲染后的 DOM 中的 [[wiki-link]] 文本替换为可点击链接。
 *
 * marked.js 将 `[[target]]` 渲染为纯文本（非标准 Markdown 语法），
 * 此函数遍历文本节点查找 `[[...]]` 模式并替换为 <a> 元素。
 *
 * 跳过代码块（pre/code）和已有的 wiki-link 元素。
 *
 * @param {HTMLElement} mdEl - Markdown 内容容器
 */
export function enhanceWikiLinks(mdEl) {
  if (!mdEl) return;

  const walker = document.createTreeWalker(mdEl, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => {
      const parent = node.parentElement;
      if (!parent) return NodeFilter.FILTER_REJECT;
      // 跳过代码块、行内代码、已有的 wiki-link
      if (parent.closest('pre, code, .wiki-link')) return NodeFilter.FILTER_REJECT;
      // 检查是否包含 [[...]] 模式
      return /\[\[.+\]\]/.test(node.nodeValue) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_REJECT;
    },
  });

  const textNodes = [];
  let node;
  while ((node = walker.nextNode())) textNodes.push(node);

  const wikiRegex = /\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|[^\]]*)?\]\]/g;

  for (const textNode of textNodes) {
    const text = textNode.nodeValue;
    const fragments = [];
    let lastIndex = 0;
    let match;
    let hasMatch = false;

    while ((match = wikiRegex.exec(text)) !== null) {
      hasMatch = true;
      // 前面的普通文本
      if (match.index > lastIndex) {
        fragments.push(document.createTextNode(text.slice(lastIndex, match.index)));
      }
      // wiki-link 链接
      const target = match[1].trim();
      const link = document.createElement('a');
      link.className = 'wiki-link';
      link.textContent = target;
      link.setAttribute('data-wiki-target', target);
      link.href = '#';
      link.title = t('wiki.link_tooltip') || target;
      link.addEventListener('click', (e) => {
        e.preventDefault();
        // 触发自定义事件，由外部监听处理导航
        const event = new CustomEvent('wiki-link-click', {
          detail: { target },
          bubbles: true,
        });
        link.dispatchEvent(event);
      });
      fragments.push(link);
      lastIndex = match.index + match[0].length;
    }

    if (hasMatch) {
      // 尾部普通文本
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
 * 获取文档的正向链接列表。
 * @param {string} docId - 文档 ID
 * @returns {Promise<Array<{id: string, source_doc_id: string, target: string, chunk_id: string, created_at: number}>>}
 */
export async function getForwardLinks(docId) {
  return invoke('get_forward_links', { docId });
}

/**
 * 获取文档的反向链接列表。
 * @param {string} docName - 文档文件名（不含扩展名）
 * @returns {Promise<Array<{id: string, source_doc_id: string, target: string, chunk_id: string, created_at: number}>>}
 */
export async function getBacklinks(docName) {
  return invoke('get_backlinks', { docName });
}

/**
 * 重建 wiki-link 索引。
 * @returns {Promise<number>} 重建的链接总数
 */
export async function rebuildWikiLinks() {
  return invoke('rebuild_wiki_links');
}

/**
 * 渲染反向链接面板内容。
 *
 * 在文档列表或文档详情页中显示反向链接信息。
 *
 * @param {string} docName - 文档文件名（不含扩展名）
 * @param {HTMLElement} container - 渲染容器
 */
export async function renderBacklinksPanel(docName, container) {
  if (!container) return;

  const links = await getBacklinks(docName);

  if (links.length === 0) {
    container.innerHTML = `<div class="text-text-tertiary text-sm p-3">${t('wiki.no_backlinks')}</div>`;
    return;
  }

  const html = links.map((link) => {
    const date = new Date(link.created_at * 1000).toLocaleDateString();
    return `
      <div class="backlink-item p-2 hover:bg-surface-2 rounded cursor-pointer" data-doc-id="${link.source_doc_id}">
        <div class="text-text-primary text-sm font-medium truncate">${link.target}</div>
        <div class="text-text-tertiary text-xs">${date}</div>
      </div>
    `;
  }).join('');

  container.innerHTML = `
    <div class="wiki-backlinks-panel">
      <div class="wiki-panel-header text-text-secondary text-xs font-medium uppercase tracking-wide px-3 py-2 border-b border-border-default">
        ${t('wiki.backlinks')} (${links.length})
      </div>
      <div class="wiki-panel-body">${html}</div>
    </div>
  `;
}

/**
 * 渲染正向链接面板内容。
 *
 * @param {string} docId - 文档 ID
 * @param {HTMLElement} container - 渲染容器
 */
export async function renderForwardLinksPanel(docId, container) {
  if (!container) return;

  const links = await getForwardLinks(docId);

  if (links.length === 0) {
    container.innerHTML = `<div class="text-text-tertiary text-sm p-3">${t('wiki.no_forward_links')}</div>`;
    return;
  }

  const html = links.map((link) => {
    return `
      <div class="forward-link-item p-2 hover:bg-surface-2 rounded cursor-pointer" data-target="${link.target}">
        <div class="text-text-primary text-sm font-medium truncate">${link.target}</div>
      </div>
    `;
  }).join('');

  container.innerHTML = `
    <div class="wiki-forward-links-panel">
      <div class="wiki-panel-header text-text-secondary text-xs font-medium uppercase tracking-wide px-3 py-2 border-b border-border-default">
        ${t('wiki.forward_links')} (${links.length})
      </div>
      <div class="wiki-panel-body">${html}</div>
    </div>
  `;
}
