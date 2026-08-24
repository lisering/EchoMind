/**
 * EchoMind 文档面板模块 — KB 统计仪表盘 + 文档内容预览。
 *
 * 合并自：kb-stats.js (REQ-KB-003) + doc-preview.js (REQ-ING-010)
 *
 * 职责：
 * 1. KB 统计仪表盘面板（文档数/分块数/向量数/存储大小/领域分布/格式分布/标签热图）
 * 2. 文档内容预览面板（元数据 + 原文前 500 字 + Chunk 列表）
 */

// ============================================================
// 导入
// ============================================================

import { $, formatBytes } from './utils.js';
import { kbStatsApi, docPreviewApi } from './ipc.js';
import { t } from './i18n.js';
import { toastError } from './toast.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { zClass, Z_INDEX } from './panel-stack.js';

// ============================================================
// KB 统计仪表盘（原 kb-stats.js）
// ============================================================

let _kbStatsCreated = false;
let _kbStatsPanelEl = null;

/**
 * 创建概要卡片 HTML。
 */
function _summaryCard(label, value, icon = '') {
  return `<div class="bg-surface-2 border border-line rounded-lg p-4 flex flex-col gap-1">
    <div class="flex items-center gap-2 text-text-tertiary text-xs uppercase tracking-wider">${icon}${label}</div>
    <div class="text-2xl font-semibold text-text-primary">${value}</div>
  </div>`;
}

const STATUS_COLORS = {
  pending: { text: 'text-text-quaternary', bg: 'bg-surface-3', border: 'border-line' },
  processing: { text: 'text-amber-300', bg: 'bg-amber-400/10', border: 'border-amber-400/40' },
  indexed: { text: 'text-accent', bg: 'bg-accent/10', border: 'border-accent/40' },
  failed: { text: 'text-red-400', bg: 'bg-red-400/10', border: 'border-red-400/40' },
};

const STATUS_LABELS = {
  pending: 'kb_stats.status_pending',
  processing: 'kb_stats.status_processing',
  indexed: 'kb_stats.status_indexed',
  failed: 'kb_stats.status_failed',
};

function _statusBadges(statusDistribution) {
  if (!statusDistribution || statusDistribution.length === 0) {
    return `<div class="space-y-1">
      <h3 class="text-xs font-medium text-text-tertiary uppercase tracking-wider mb-2">${t('kb_stats.status_distribution', 'Index Status')}</h3>
      <div class="text-sm text-text-quaternary">${t('kb_stats.empty', 'No data')}</div>
    </div>`;
  }

  const badges = statusDistribution.map(([status, count]) => {
    const colors = STATUS_COLORS[status] || STATUS_COLORS.pending;
    const label = t(STATUS_LABELS[status] || 'kb_stats.status_pending', status);
    return `<div class="flex items-center gap-2">
      <span class="px-2 py-1 rounded border ${colors.border} ${colors.bg} ${colors.text} text-xs font-medium">${label}</span>
      <span class="text-lg font-semibold text-text-primary">${count}</span>
    </div>`;
  }).join('');

  return `<div class="space-y-2">
    <h3 class="text-xs font-medium text-text-tertiary uppercase tracking-wider mb-2">${t('kb_stats.status_distribution', 'Index Status')}</h3>
    <div class="flex flex-wrap gap-4">${badges}</div>
  </div>`;
}

function _distributionList(title, items, maxCount) {
  if (!items || items.length === 0) {
    return `<div class="space-y-1">
      <h3 class="text-xs font-medium text-text-tertiary uppercase tracking-wider mb-2">${title}</h3>
      <div class="text-sm text-text-quaternary">${t('kb_stats.empty', 'No data')}</div>
    </div>`;
  }

  const rows = items.map(([name, count]) => {
    const pct = maxCount > 0 ? (count / maxCount) * 100 : 0;
    return `<div class="flex items-center gap-2">
      <span class="text-sm text-text-secondary w-24 truncate shrink-0">${name}</span>
      <div class="flex-1 bg-surface-3 rounded-full h-5 overflow-hidden relative" role="progressbar" aria-valuenow="${count}" aria-valuemin="0" aria-valuemax="${maxCount}">
        <div class="bg-accent/30 h-full rounded-full transition-all" style="width: ${pct}%" aria-hidden="true"></div>
        <span class="absolute inset-0 flex items-center px-2 text-xs text-text-secondary">${count}</span>
      </div>
    </div>`;
  }).join('');

  return `<div class="space-y-1.5">
    <h3 class="text-xs font-medium text-text-tertiary uppercase tracking-wider mb-2">${title}</h3>
    ${rows}
  </div>`;
}

function _tagHeatmap(tags) {
  if (!tags || tags.length === 0) {
    return `<div class="space-y-1">
      <h3 class="text-xs font-medium text-text-tertiary uppercase tracking-wider mb-2">${t('kb_stats.tag_heatmap', 'Tag Heatmap')}</h3>
      <div class="text-sm text-text-quaternary">${t('kb_stats.empty', 'No data')}</div>
    </div>`;
  }

  const maxCount = Math.max(...tags.map(([, c]) => c));
  const chips = tags.map(([name, count]) => {
    const ratio = maxCount > 0 ? count / maxCount : 0;
    const sizeClass = ratio > 0.66 ? 'text-base' : ratio > 0.33 ? 'text-sm' : 'text-xs';
    const opacity = 0.3 + ratio * 0.7;
    return `<span class="${sizeClass} px-2 py-1 rounded-full bg-accent" style="opacity: ${opacity}">${name} <span class="text-text-quaternary">(${count})</span></span>`;
  }).join(' ');

  return `<div class="space-y-2">
    <h3 class="text-xs font-medium text-text-tertiary uppercase tracking-wider mb-2">${t('kb_stats.tag_heatmap', 'Tag Heatmap')}</h3>
    <div class="flex flex-wrap gap-2">${chips}</div>
  </div>`;
}

function _ensureKbStatsPanel() {
  if (_kbStatsCreated && _kbStatsPanelEl && _kbStatsPanelEl.isConnected) return _kbStatsPanelEl;

  const overlay = document.createElement('div');
  overlay.id = 'kbStatsOverlay';
  overlay.className = `fixed inset-0 bg-black/50 flex items-center justify-center ${zClass(Z_INDEX.PANEL_1)}`;
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-labelledby', 'kbStatsTitle');

  const panel = document.createElement('div');
  panel.className = 'bg-surface-1 border border-line rounded-xl shadow-2xl w-[560px] max-h-[80vh] overflow-y-auto';

  const header = document.createElement('div');
  header.className = 'flex items-center justify-between px-6 py-4 border-b border-line sticky top-0 bg-surface-1';
  const title = document.createElement('h2');
  title.id = 'kbStatsTitle';
  title.className = 'text-lg font-semibold text-text-primary';
  title.textContent = t('kb_stats.title', 'Knowledge Base Statistics');
  const closeBtn = document.createElement('button');
  closeBtn.className = 'text-text-tertiary hover:text-text-primary transition-colors p-1 rounded';
  closeBtn.setAttribute('aria-label', t('common.close'));
  closeBtn.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  closeBtn.onclick = closeKbStats;
  header.appendChild(title);
  header.appendChild(closeBtn);

  const body = document.createElement('div');
  body.id = 'kbStatsBody';
  body.className = 'px-6 py-4 space-y-6';
  body.innerHTML = `<div class="text-center text-text-tertiary py-8">${t('kb_stats.loading', 'Loading…')}</div>`;

  panel.appendChild(header);
  panel.appendChild(body);
  overlay.appendChild(panel);

  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) closeKbStats();
  });

  document.body.appendChild(overlay);
  _kbStatsPanelEl = overlay;
  _kbStatsCreated = true;
  return overlay;
}

function _renderStats(stats) {
  const body = document.getElementById('kbStatsBody');
  if (!body) return;

  const maxDomain = Math.max(1, ...stats.domain_distribution.map(([, c]) => c));
  const maxFormat = Math.max(1, ...stats.format_distribution.map(([, c]) => c));

  const docIcon = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>';
  const chunkIcon = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/></svg>';
  const vectorIcon = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M12 1v6m0 6v6m11-7h-6m-6 0H1"/></svg>';
  const sizeIcon = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/></svg>';

  const vectorCount = stats.vector_count || 0;

  body.innerHTML = `
    <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
      ${_summaryCard(t('kb_stats.doc_count', 'Documents'), String(stats.doc_count), docIcon)}
      ${_summaryCard(t('kb_stats.chunk_count', 'Chunks'), String(stats.chunk_count), chunkIcon)}
      ${_summaryCard(t('kb_stats.vector_count', 'Vectors'), String(vectorCount), vectorIcon)}
      ${_summaryCard(t('kb_stats.storage_size', 'Storage'), formatBytes(stats.db_size_bytes), sizeIcon)}
    </div>
    ${_statusBadges(stats.status_distribution)}
    ${_distributionList(t('kb_stats.domain_distribution', 'Domain Distribution'), stats.domain_distribution, maxDomain)}
    ${_distributionList(t('kb_stats.format_distribution', 'Format Distribution'), stats.format_distribution, maxFormat)}
    ${_tagHeatmap(stats.tags)}
  `;
}

/**
 * 打开 KB 统计仪表盘面板。
 */
export async function openKbStats() {
  const overlay = _ensureKbStatsPanel();
  overlay.classList.remove('hidden');
  pushPanel({ id: 'kb-stats', close: closeKbStats, element: overlay, label: 'KB Statistics' });
  await _refreshKbStats();
}

async function _refreshKbStats() {
  const body = document.getElementById('kbStatsBody');
  if (body) {
    body.innerHTML = `<div class="text-center text-text-tertiary py-8">${t('kb_stats.loading', 'Loading…')}</div>`;
  }
  try {
    const stats = await kbStatsApi.getStats();
    _renderStats(stats);
  } catch (err) {
    if (body) {
      body.innerHTML = `<div class="text-center text-red-400 py-8">${t('kb_stats.error', 'Failed to load statistics')}</div>`;
    }
    toastError(err);
  }
}

/**
 * 关闭 KB 统计仪表盘面板。
 */
export function closeKbStats() {
  removePanel('kb-stats');
  if (_kbStatsPanelEl) {
    _kbStatsPanelEl.classList.add('hidden');
  }
}

// ============================================================
// 文档内容预览（原 doc-preview.js）
// ============================================================

let _docPreviewCreated = false;
let _docPreviewPanelEl = null;

function _formatDate(timestamp) {
  if (!timestamp) return '-';
  const d = new Date(timestamp * 1000);
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  const hour = String(d.getHours()).padStart(2, '0');
  const min = String(d.getMinutes()).padStart(2, '0');
  return `${year}-${month}-${day} ${hour}:${min}`;
}

function _fileName(filePath) {
  if (!filePath) return '-';
  const parts = filePath.replace(/\\/g, '/').split('/');
  return parts[parts.length - 1] || filePath;
}

function _fileExt(filePath) {
  const name = _fileName(filePath);
  const dot = name.lastIndexOf('.');
  return dot >= 0 ? name.substring(dot + 1).toUpperCase() : '-';
}

function _statusText(status) {
  const key = `doc.status_${status}`;
  const text = t(key);
  return text !== key ? text : status;
}

function _createPreviewSkeleton() {
  const overlay = document.createElement('div');
  overlay.className = `fixed inset-0 flex items-center justify-center ${zClass(Z_INDEX.MODAL)}`;
  overlay.style.backgroundColor = 'rgba(0,0,0,0.5)';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-labelledby', 'docPreviewTitle');

  const panel = document.createElement('div');
  panel.className = 'bg-surface-1 border border-line rounded-2xl shadow-2xl w-full max-w-3xl max-h-[80vh] flex flex-col overflow-hidden';

  const header = document.createElement('div');
  header.className = 'flex items-center justify-between px-6 py-4 border-b border-line';
  const title = document.createElement('h2');
  title.id = 'docPreviewTitle';
  title.className = 'text-lg font-semibold text-text-primary';
  title.textContent = t('doc.preview_title') || '文档预览';
  header.appendChild(title);
  const closeBtn = document.createElement('button');
  closeBtn.className = 'text-text-tertiary hover:text-text-primary transition-colors text-xl px-2';
  closeBtn.innerHTML = '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  closeBtn.setAttribute('aria-label', t('common.close') || '关闭');
  closeBtn.onclick = closeDocPreview;
  header.appendChild(closeBtn);
  panel.appendChild(header);

  const body = document.createElement('div');
  body.id = 'docPreviewBody';
  body.className = 'flex-1 overflow-y-auto px-6 py-4 space-y-4';
  panel.appendChild(body);

  const loading = document.createElement('div');
  loading.id = 'docPreviewLoading';
  loading.className = 'text-text-tertiary text-center py-8';
  loading.textContent = t('doc.preview_loading') || '加载中…';
  body.appendChild(loading);

  overlay.appendChild(panel);
  return overlay;
}

function _renderPreviewContent(preview) {
  const body = document.getElementById('docPreviewBody');
  if (!body) return;
  body.innerHTML = '';

  const metaSection = document.createElement('div');
  metaSection.className = 'grid grid-cols-2 gap-3 text-sm';

  const metaItems = [
    [t('doc.preview_filename') || '文件名', _fileName(preview.file_path)],
    [t('doc.preview_format') || '格式', _fileExt(preview.file_path)],
    [t('doc.preview_status') || '状态', _statusText(preview.status)],
    [t('doc.preview_chunks') || '分块数', String(preview.chunk_count)],
    [t('doc.preview_imported') || '导入时间', _formatDate(preview.created_at)],
    [t('doc.preview_hash') || '指纹', (preview.file_hash || '').substring(0, 12) + '…'],
  ];

  for (const [label, value] of metaItems) {
    const item = document.createElement('div');
    item.className = 'flex flex-col gap-1';
    const labelEl = document.createElement('span');
    labelEl.className = 'text-text-tertiary text-xs';
    labelEl.textContent = label;
    const valueEl = document.createElement('span');
    valueEl.className = 'text-text-secondary font-mono text-xs break-all';
    valueEl.textContent = value;
    item.appendChild(labelEl);
    item.appendChild(valueEl);
    metaSection.appendChild(item);
  }
  body.appendChild(metaSection);

  if (preview.summary) {
    const summaryDiv = document.createElement('div');
    summaryDiv.className = 'space-y-1';
    const summaryLabel = document.createElement('div');
    summaryLabel.className = 'text-text-tertiary text-xs font-semibold';
    summaryLabel.textContent = t('doc.preview_summary') || '摘要';
    summaryDiv.appendChild(summaryLabel);
    const summaryText = document.createElement('div');
    summaryText.className = 'text-text-secondary text-sm leading-relaxed';
    summaryText.textContent = preview.summary;
    summaryDiv.appendChild(summaryText);
    body.appendChild(summaryDiv);
  }

  if (preview.tags && preview.tags.length > 0) {
    const tagsDiv = document.createElement('div');
    tagsDiv.className = 'flex flex-wrap gap-2';
    for (const tag of preview.tags) {
      const tagEl = document.createElement('span');
      tagEl.className = 'px-2 py-0.5 rounded text-xs bg-accent/10 text-accent border border-accent/20';
      tagEl.textContent = tag;
      tagsDiv.appendChild(tagEl);
    }
    body.appendChild(tagsDiv);
  }

  const contentSection = document.createElement('div');
  contentSection.className = 'space-y-1';
  const contentLabel = document.createElement('div');
  contentLabel.className = 'text-text-tertiary text-xs font-semibold';
  contentLabel.textContent = t('doc.preview_content') || '内容预览（前 500 字）';
  contentSection.appendChild(contentLabel);
  const contentText = document.createElement('div');
  contentText.className = 'text-text-secondary text-sm leading-relaxed bg-surface-2 rounded-lg p-3 max-h-48 overflow-y-auto whitespace-pre-wrap';
  contentText.textContent = preview.content_preview || t('doc.preview_no_content') || '（无内容）';
  contentSection.appendChild(contentText);
  body.appendChild(contentSection);

  if (preview.chunks && preview.chunks.length > 0) {
    const chunkSection = document.createElement('div');
    chunkSection.className = 'space-y-2';
    const chunkLabel = document.createElement('div');
    chunkLabel.className = 'text-text-tertiary text-xs font-semibold';
    chunkLabel.textContent = t('doc.preview_chunk_list') || `分块列表（${preview.chunks.length} 个）`;
    chunkSection.appendChild(chunkLabel);

    for (const chunk of preview.chunks) {
      const chunkItem = document.createElement('details');
      chunkItem.className = 'rounded-lg border border-line';
      const summary = document.createElement('summary');
      summary.className = 'cursor-pointer px-3 py-2 text-xs text-text-secondary select-none hover:bg-white/5';
      summary.textContent = `#${chunk.sequence} · ${chunk.token_count} tokens`;
      chunkItem.appendChild(summary);
      const chunkContent = document.createElement('div');
      chunkContent.className = 'px-3 pb-2 text-xs text-text-tertiary whitespace-pre-wrap';
      chunkContent.textContent = chunk.content_preview;
      chunkItem.appendChild(chunkContent);
      chunkSection.appendChild(chunkItem);
    }
    body.appendChild(chunkSection);
  }
}

/**
 * 打开文档预览面板。
 */
export async function openDocPreview(docId) {
  if (_docPreviewCreated) closeDocPreview();

  _docPreviewPanelEl = _createPreviewSkeleton();
  document.body.appendChild(_docPreviewPanelEl);
  _docPreviewCreated = true;
  pushPanel({ id: 'doc-preview', close: closeDocPreview, element: _docPreviewPanelEl, label: 'Doc Preview' });

  try {
    const preview = await docPreviewApi.getPreview(docId);
    if (!preview) {
      const body = document.getElementById('docPreviewBody');
      if (body) {
        body.innerHTML = '';
        const err = document.createElement('div');
        err.className = 'text-text-tertiary text-center py-8';
        err.textContent = t('doc.preview_not_found') || '文档不存在';
        body.appendChild(err);
      }
      return;
    }
    _renderPreviewContent(preview);
  } catch (err) {
    toastError(`${t('doc.preview_error') || '预览加载失败'}: ${err instanceof Error ? err.message : String(err)}`);
    closeDocPreview();
  }
}

/**
 * 关闭文档预览面板。
 */
export function closeDocPreview() {
  if (_docPreviewPanelEl && _docPreviewPanelEl.parentNode) {
    _docPreviewPanelEl.parentNode.removeChild(_docPreviewPanelEl);
  }
  _docPreviewPanelEl = null;
  _docPreviewCreated = false;
  removePanel('doc-preview');
}
