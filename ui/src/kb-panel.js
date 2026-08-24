/**
 * EchoMind 知识库文档面板模块（V3.1 P4-3 自 main.js 拆分）。
 *
 * 职责：
 * 1. 文档列表加载 / 筛选 / 排序（REQ-ING-008）
 * 2. 分页无限滚动渲染（IntersectionObserver，KB_PAGE_SIZE=20）
 * 3. 文档条目构建（状态点/标签/摘要/预览/重试/审计动作）
 * 4. 文档删除 / 重试索引 / 摘要面板
 */

import { $, displayDocName, showSkeleton, hideSkeleton, makeKeyboardClickable, docStatusOf, DOC_STATUS_STYLE, icon, fileIcon } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { get, setState } from './state.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { isMultiSelectMode } from './sidebar.js';
import { openDocPreview } from './doc-panels.js';
import { auditDocument } from './chat.js';

/**
 * 行为回调（由 main.js 注入，避免反向依赖）：
 * - updateChatInputState: 文档数变化后刷新输入区禁用状态
 */
const _callbacks = {};

/**
 * 注入宿主回调（main.js 初始化时调用一次）。
 * @param {{updateChatInputState: Function}} cbs
 */
export function setupKbPanelCallbacks(cbs) {
  Object.assign(_callbacks, cbs);
}

const KB_PAGE_SIZE = 20;

/** 缓存的全部文档（从后端一次性加载，前端分页渲染） */
let _kbAllDocs = [];

/** 筛选后的文档列表 */
let _kbFilteredDocs = [];

/** 已渲染数量 */
let _kbRenderedCount = 0;

/** IntersectionObserver 实例 */
let _kbObserver = null;

/** 文件类型图标 — 使用 SVG inline 图标（REQ-DS-004）替代 Emoji */
function getFileIcon(ext) {
  return fileIcon(ext, 'sm');
}

/** 状态颜色映射（圆点指示器） */
const STATUS_DOT_COLORS = {
  Indexed: 'bg-emerald-400',
  Processing: 'bg-amber-400 animate-pulse',
  Pending: 'bg-slate-400',
  Failed: 'bg-red-400',
};

/**
 * 加载知识库文档列表（从后端获取全部，前端分页渲染）。
 * 同步更新配额计数，然后调用 applyKbFilters() 进行筛选和渲染。
 * REQ-ING-008 v1.10：支持排序参数传递。
 */
let _kbSortBy = null;
let _kbSortOrder = null;

export async function loadDocuments() {
  const docList = $('docList');
  if (docList) showSkeleton(docList, 'doc', 4);
  try {
    _kbAllDocs = await invoke('get_documents', { sortBy: _kbSortBy, sortOrder: _kbSortOrder });
    const isPro = get('isPro');
    setState({ docCount: _kbAllDocs.length, kbAllDocs: _kbAllDocs });
    _callbacks.updateChatInputState?.();
    const countEl = $('kbDocCount');
    if (countEl) {
      countEl.textContent = isPro
        ? t('knowledge_base.doc_count_pro', { count: _kbAllDocs.length })
        : t('knowledge_base.doc_count_free', { count: _kbAllDocs.length });
    }
    applyKbFilters();
  } catch (err) {
    toastError(err);
  } finally {
    if (docList) hideSkeleton(docList);
  }
}

/**
 * 应用筛选条件（搜索 + 状态 + 格式 + 标签），重新渲染文档列表。
 * 从缓存 _kbAllDocs 中筛选，不重新请求后端。
 */
export function applyKbFilters() {
  const searchQ = ($('docSearchInput')?.value || '').trim().toLowerCase();
  const statusFilter = $('docStatusFilter')?.value || '';
  const formatFilter = $('docFormatFilter')?.value || '';
  const tagFilter = $('docTagFilter')?.value || '';

  _kbFilteredDocs = _kbAllDocs.filter((d) => {
    const name = displayDocName(d.file_path);
    const nameLower = name.toLowerCase();
    const status = docStatusOf(d);
    const ext = name.split('.').pop()?.toLowerCase() || '';
    const tags = d.tags || [];

    // 模糊搜索：文件名 + 标签
    const matchSearch = !searchQ ||
      nameLower.includes(searchQ) ||
      tags.some((tag) => tag.toLowerCase().includes(searchQ));
    const matchStatus = !statusFilter || status === statusFilter;
    const matchFormat = !formatFilter || ext === formatFilter;
    const matchTag = !tagFilter || tags.includes(tagFilter);

    return matchSearch && matchStatus && matchFormat && matchTag;
  });

  _kbRenderedCount = 0;
  const box = $('docList');
  if (box) box.innerHTML = '';

  if (_kbObserver) { _kbObserver.disconnect(); _kbObserver = null; }

  if (_kbFilteredDocs.length === 0) {
    _renderKbEmptyState(box, _kbAllDocs.length === 0);
  } else {
    _renderKbDocPage();
    _setupKbObserver();
  }
  _updateKbResultCount();
}

/**
 * 渲染一页文档（KB_PAGE_SIZE 条）。
 */
export function _renderKbDocPage() {
  const box = $('docList');
  if (!box) return;

  const isPro = get('isPro');
  const multiSelect = isMultiSelectMode();
  const start = _kbRenderedCount;
  const end = Math.min(start + KB_PAGE_SIZE, _kbFilteredDocs.length);

  for (let i = start; i < end; i++) {
    box.appendChild(_createDocItem(_kbFilteredDocs[i], isPro, multiSelect));
  }
  _kbRenderedCount = end;
}

/**
 * 创建文档列表项 DOM 元素。
 * 设计：图标 + 状态圆点 + 文件名（flex-1 truncate）+ 内联标签 + hover 操作按钮
 */
export function _createDocItem(d, isPro, multiSelect) {
  const name = displayDocName(d.file_path);
  const status = docStatusOf(d);
  const statusColor = STATUS_DOT_COLORS[status] || STATUS_DOT_COLORS.Failed;
  const [labelKey] = DOC_STATUS_STYLE[status] || DOC_STATUS_STYLE.Failed;
  const ext = name.split('.').pop()?.toLowerCase() || '';

  const wrapper = document.createElement('div');
  wrapper.dataset.docId = d.id;
  wrapper.dataset.docName = name;
  wrapper.dataset.docStatus = status;
  wrapper.dataset.docFormat = ext;
  wrapper.dataset.docTags = JSON.stringify(d.tags || []);

  const item = document.createElement('div');
  item.className = 'group flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs hover:bg-white/5 transition-colors';

  // 多选模式：复选框
  if (multiSelect) {
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.dataset.docId = d.id;
    checkbox.className = 'shrink-0 w-3.5 h-3.5 accent-accent cursor-pointer';
    checkbox.onclick = (ev) => ev.stopPropagation();
    checkbox.onchange = () => {
      const checked = document.querySelectorAll('#docList input[type="checkbox"]:checked').length;
      const countEl = $('kbSelectedCount');
      if (countEl) countEl.textContent = t('knowledge_base.selected_count', { count: checked });
    };
    item.appendChild(checkbox);
  }

  // 文件类型图标（SVG inline，REQ-DS-004）
  const fileIconEl = document.createElement('span');
  fileIconEl.className = 'shrink-0 leading-none';
  fileIconEl.innerHTML = getFileIcon(ext);
  item.appendChild(fileIconEl);

  // 状态指示圆点
  const dot = document.createElement('span');
  dot.className = `shrink-0 w-1.5 h-1.5 rounded-full ${statusColor}`;
  dot.title = t('knowledge_base.' + labelKey);
  item.appendChild(dot);

  // 文件名（可点击预览 REQ-ING-010）
  const left = document.createElement('span');
  left.className = 'truncate flex-1 text-text-primary cursor-pointer hover:text-accent transition-colors';
  left.textContent = name;
  left.title = d.file_path;
  left.onclick = () => openDocPreview(d.id);
  // V3.1 P3-6：键盘可达（Enter/Space 触发预览）
  makeKeyboardClickable(left);
  item.appendChild(left);

  // 标签（内联显示，最多 3 个 + 溢出计数）
  if (d.tags && d.tags.length > 0 && !multiSelect) {
    const tagContainer = document.createElement('span');
    tagContainer.className = 'shrink-0 flex items-center gap-1';
    const maxTags = 3;
    for (let i = 0; i < Math.min(d.tags.length, maxTags); i++) {
      const tagEl = document.createElement('span');
      tagEl.className = 'px-1 py-0.5 rounded text-[9px] bg-accent/10 text-accent border border-accent/15';
      tagEl.textContent = d.tags[i];
      tagContainer.appendChild(tagEl);
    }
    if (d.tags.length > maxTags) {
      const more = document.createElement('span');
      more.className = 'text-[9px] text-text-quaternary';
      more.textContent = `+${d.tags.length - maxTags}`;
      tagContainer.appendChild(more);
    }
    item.appendChild(tagContainer);
  }

  // 操作按钮（hover 显示）
  const actions = document.createElement('span');
  actions.className = 'shrink-0 flex items-center gap-0.5';

  // 文档预览按钮（REQ-ING-010）
  const previewBtn = document.createElement('button');
  previewBtn.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  previewBtn.innerHTML = icon('eye', 'sm');
  previewBtn.title = t('doc.preview_title') || '预览';
  previewBtn.onclick = (ev) => { ev.stopPropagation(); openDocPreview(d.id); };
  actions.appendChild(previewBtn);

  const sumToggle = document.createElement('button');
  sumToggle.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  sumToggle.innerHTML = icon('summary', 'sm');
  sumToggle.title = t('knowledge_base.summary_toggle');
  sumToggle.style.display = status === 'Indexed' ? '' : 'none';
  sumToggle.onclick = async (ev) => {
    ev.stopPropagation();
    const panel = wrapper.querySelector('.doc-summary-panel');
    if (panel) { panel.classList.toggle('hidden'); return; }
    const newPanel = document.createElement('div');
    newPanel.className = 'doc-summary-panel px-2.5 pb-2 pt-1 text-[11px] text-text-secondary leading-relaxed';
    newPanel.innerHTML = `<span class="text-text-quaternary">${t('knowledge_base.summary_loading')}</span>`;
    wrapper.appendChild(newPanel);
    try {
      const summary = await invoke('get_document_summary', { docId: d.id });
      renderSummaryPanel(newPanel, d.id, name, summary);
    } catch (err) {
      newPanel.innerHTML = `<span class="text-red-400">${t('knowledge_base.summary_load_failed')}</span>`;
    }
  };
  actions.appendChild(sumToggle);

  const retry = document.createElement('button');
  retry.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  retry.innerHTML = icon('retry', 'sm');
  retry.title = t('knowledge_base.retry_index');
  retry.style.display = status === 'Failed' ? '' : 'none';
  retry.onclick = async (ev) => { ev.stopPropagation(); await retryDocument(d.id, name); };
  actions.appendChild(retry);

  const audit = document.createElement('button');
  audit.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  audit.innerHTML = icon('search', 'sm');
  audit.title = t('knowledge_base.audit_doc');
  audit.style.display = (isPro && status === 'Indexed') ? '' : 'none';
  audit.onclick = async (ev) => { ev.stopPropagation(); await auditDocument(d.id, name); };
  actions.appendChild(audit);

  const tagBtn = document.createElement('button');
  tagBtn.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  tagBtn.innerHTML = icon('tag', 'sm');
  tagBtn.title = t('knowledge_base.tag_input_placeholder');
  tagBtn.onclick = (ev) => {
    ev.stopPropagation();
    const inputRow = wrapper.querySelector('.tag-input-row');
    if (inputRow) {
      inputRow.classList.toggle('hidden');
      if (!inputRow.classList.contains('hidden')) inputRow.querySelector('input')?.focus();
    }
  };
  actions.appendChild(tagBtn);

  const del = document.createElement('button');
  del.className = 'invisible group-hover:visible text-text-quaternary hover:text-red-400 px-1 transition-opacity';
  del.dataset.action = 'delete';
  del.innerHTML = icon('close', 'sm');
  del.title = t('knowledge_base.delete_doc');
  del.onclick = async (ev) => { ev.stopPropagation(); await removeDocument(d.id, name); };
  actions.appendChild(del);

  if (multiSelect) {
    retry.style.display = 'none';
    del.style.display = 'none';
    audit.style.display = 'none';
    sumToggle.style.display = 'none';
    tagBtn.style.display = 'none';
  }

  item.appendChild(actions);
  wrapper.appendChild(item);

  // 标签输入行（点击 # 按钮切换显示，Esc 关闭，失焦自动收起）
  const tagInputRow = document.createElement('div');
  tagInputRow.className = 'tag-input-row hidden px-2.5 pb-1.5 flex items-center gap-1';
  const tagInput = document.createElement('input');
  tagInput.type = 'text';
  tagInput.placeholder = t('knowledge_base.tag_input_placeholder');
  tagInput.className = 'flex-1 px-1.5 py-0.5 rounded text-[10px] bg-surface-3 border border-border-default text-text-primary placeholder:text-text-quaternary focus:outline-none focus:border-accent';
  tagInput.onkeydown = async (ev) => {
    if (ev.key === 'Enter' && tagInput.value.trim()) {
      ev.preventDefault();
      try {
        await invoke('add_document_tag', { docId: d.id, tag: tagInput.value.trim() });
        tagInput.value = '';
        loadDocuments();
      } catch (err) { toastError(err); }
    }
    if (ev.key === 'Escape') { tagInput.value = ''; tagInputRow.classList.add('hidden'); }
  };
  tagInput.onblur = () => { if (!tagInput.value.trim()) tagInputRow.classList.add('hidden'); };
  tagInputRow.appendChild(tagInput);
  wrapper.appendChild(tagInputRow);

  return wrapper;
}

/**
 * 设置 IntersectionObserver 实现无限滚动。
 * 当 sentinel 元素进入视口时，加载下一页文档。
 */
function _setupKbObserver() {
  const sentinel = $('kbScrollSentinel');
  if (!sentinel) return;
  if (_kbObserver) _kbObserver.disconnect();
  _kbObserver = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting && _kbRenderedCount < _kbFilteredDocs.length) {
      _renderKbDocPage();
    }
  }, { root: $('kbDocScroll'), threshold: 0.1 });
  _kbObserver.observe(sentinel);
}

/**
 * 渲染空状态：知识库为空 或 搜索/筛选无结果。
 */
function _renderKbEmptyState(box, isEmpty) {
  const empty = document.createElement('div');
  empty.className = 'px-3 py-8 text-center text-xs text-text-quaternary';
  if (isEmpty) {
    empty.innerHTML = `
      <div class="mb-3 opacity-50 flex justify-center">${icon('book', 'lg')}</div>
      <div class="mb-1">${t('knowledge_base.empty_title')}</div>
      <div class="text-[11px] opacity-70">${t('knowledge_base.empty_hint')}</div>
      <div class="text-[10px] opacity-50 mt-2">${t('knowledge_base.empty_formats')}</div>`;
  } else {
    empty.innerHTML = `
      <div class="mb-2 opacity-50 flex justify-center">${icon('search', 'md')}</div>
      <div>${t('knowledge_base.no_match')}</div>`;
  }
  box.appendChild(empty);
}

/**
 * 更新底部结果计数显示。
 */
export function _updateKbResultCount() {
  const el = $('kbResultCount');
  if (!el) return;
  const total = _kbAllDocs.length;
  const filtered = _kbFilteredDocs.length;
  if (total === 0) {
    el.textContent = '';
  } else if (filtered < total) {
    el.textContent = `${filtered}/${total}`;
  } else {
    el.textContent = `${total}`;
  }
}

/**
 * 渲染文档摘要面板内容（REQ-ING-019）。
 *
 * 若摘要存在，显示摘要文本 + 重新生成按钮；
 * 若摘要为空，显示「暂无摘要」+ 生成按钮。
 *
 * @param {HTMLElement} panel - 摘要面板容器
 * @param {string} docId - 文档 ID
 * @param {string} docName - 文档名（用于 toast 提示）
 * @param {string|null} summary - 摘要文本（null 表示尚未生成）
 */
function renderSummaryPanel(panel, docId, docName, summary) {
  if (summary && summary.trim()) {
    panel.innerHTML = '';
    const text = document.createElement('p');
    text.className = 'mb-1.5 text-text-secondary';
    text.textContent = summary;
    panel.appendChild(text);
  } else {
    panel.innerHTML = `<p class="mb-1.5 text-text-quaternary">${t('knowledge_base.summary_empty')}</p>`;
  }
  // 重新生成 / 生成按钮
  const regenBtn = document.createElement('button');
  regenBtn.className = 'text-accent hover:text-accent/80 text-[10px] underline';
  regenBtn.textContent = summary
    ? t('knowledge_base.summary_regenerate')
    : t('knowledge_base.summary_generate');
  regenBtn.onclick = async (ev) => {
    ev.stopPropagation();
    regenBtn.disabled = true;
    regenBtn.textContent = t('knowledge_base.summary_generating');
    try {
      const newSummary = await invoke('regenerate_summary', { docId });
      renderSummaryPanel(panel, docId, docName, newSummary);
      toast(t('knowledge_base.summary_generated', { name: docName }), 'info');
    } catch (err) {
      regenBtn.disabled = false;
      regenBtn.textContent = t('knowledge_base.summary_regenerate');
      toastError(err);
    }
  };
  panel.appendChild(regenBtn);
}

/**
 * 删除指定文档并刷新列表。
 */
export async function removeDocument(id, name) {
  try {
    await invoke('delete_document', { id });
    toast(t('knowledge_base.deleted', { name }), 'info');
    await loadDocuments();
  } catch (err) {
    toastError(err);
  }
}

/**
 * 重试索引指定文档。
 */
export async function retryDocument(docId, docName) {
  try {
    await invoke('retry_index', { id: docId });
    toast(t('knowledge_base.retrying', { name: docName }), 'info');
  } catch (err) {
    toastError(err);
  }
}


/**
 * 绑定文档排序选择器（REQ-ING-008 v1.10；V3.1 P4-3 自 main.js 移入）。
 */
export function initDocSortSelect() {
  if (!$('docSortSelect')) return;
  $('docSortSelect').addEventListener('change', async () => {
    const val = $('docSortSelect').value;
    if (val) {
      const [by, order] = val.split(':');
      _kbSortBy = by;
      _kbSortOrder = order;
    } else {
      _kbSortBy = null;
      _kbSortOrder = null;
    }
    await loadDocuments();
  });
}
