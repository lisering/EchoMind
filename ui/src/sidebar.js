/**
 * EchoMind 侧栏模块 — 折叠 + 文档搜索 + KB Modal 控制。
 * 会话与知识库同屏显示，无需 Tab 切换。
 * REQ-NAV-001：折叠时侧栏向左滑出视口（transform），展开时恢复完整列表。状态持久化。
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { getCurrentWorkspaceId } from './workspace.js';
import { t } from './i18n.js';
import { toast, toastError } from './toast.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { isComposingEvent } from './input-utils.js';

/**
 * 侧栏折叠/展开切换（REQ-NAV-001 AC-1/AC-2）。
 *
 * CSS transition 方案（自然平滑）:
 * 1. #sidebar 使用 position:fixed + transform:translateX(-100%) 滑出视口
 *    （GPU 合成层加速，侧栏滑动流畅）
 * 2. <main> 使用 CSS transition: padding-left 300ms cubic-bezier(0.4,0,0.2,1)
 *    让内容区域宽度渐进变化，文本逐帧重排而非瞬间跳变
 *    （旧 FLIP 方案在切换瞬间瞬时改 padding → 内容宽度突变
 *    → 文本瞬间重排换行 → 高度跳变 = 「跳动」根源）
 * 3. 侧栏 transform 与 main padding-left 使用相同 300ms 缓动曲线 → 两侧同步
 *
 * rAF 锚定循环（动画期间补偿滚动位置）:
 * 1. 用户原本在底部 → scrollTop 钉住 scrollHeight，底部始终贴住视口。
 * 2. 用户在中部 → 记录视口顶部首个可见内容元素的相对偏移，逐帧补偿
 *    重排造成的位移，用户看到的同一批内容停留在同一视口位置。
 * 3. 用户动画期间主动滚动 → 立即停止锚定，不与用户抢滚动。
 * 4. 快速连点时由最后一次切换收尾（代际计数），避免竞态。
 */

/** 距底部多少像素内视为「在底部」，动画期间钉住底部。 */
const BOTTOM_ANCHOR_THRESHOLD = 100;

/** 切换代际计数：快速连点时只允许最后一次切换的收尾生效。 */
let _toggleSeq = 0;
/** 视觉锚定循环的 rAF id（0 = 未运行）。 */
let _anchorRafId = 0;
/** 切换收尾兜底计时器（transitionend 未触发时的保险）。 */
let _toggleEndTimer = null;
/** 是否处于底部模式（逐帧钉 scrollTop = scrollHeight）。 */
let _pinBottom = false;
/** 锚点元素引用及其目标视口偏移。 */
let _anchorEl = null;
let _anchorTargetOffset = 0;
/** 用户是否在动画期间主动滚动（主动滚动则停止锚定）。 */
let _userScrolledDuringToggle = false;
/** 上次程序设置的 scrollTop（用于识别程序自身滚动事件）。 */
let _lastProgrammaticScrollTop = -1;
/** scroll 监听器是否已挂载（幂等，只挂一次）。 */
let _scrollListenerAttached = false;

/**
 * 聊天区 scroll 监听：动画期间补偿浏览器布局变化导致的滚动位移。
 *
 * CSS transition 方案下，padding-left 每帧渐变 → 内容宽度逐帧变化 →
 * 文本逐帧重排 → 浏览器布局变化导致 scrollTop 位移。rAF 在浏览器
 * 样式重算之前运行，无法看到当前帧的布局变化，因此用 scroll 事件
 * 做即时补偿（scroll 事件在布局变化后、绘制前触发）。
 *
 * 用户主动滚动（wheel/touch）由独立监听器检测并设置 _userScrolledDuringToggle。
 */
function onChatScroll() {
  const chatArea = $('chatArea');
  if (!chatArea) return;
  if (chatArea.scrollTop === _lastProgrammaticScrollTop) return; // 程序自身设置
  if (!_anchorRafId) return; // 非动画期间，忽略
  if (_userScrolledDuringToggle) return; // 用户已主动滚动，放弃补偿

  // 动画期间：浏览器布局变化导致的滚动位移 → 立即补偿
  if (_pinBottom) {
    _lastProgrammaticScrollTop = chatArea.scrollHeight;
    chatArea.scrollTop = _lastProgrammaticScrollTop;
  } else if (_anchorEl && _anchorEl.isConnected) {
    const newOffset = _anchorEl.getBoundingClientRect().top - chatArea.getBoundingClientRect().top;
    const delta = newOffset - _anchorTargetOffset;
    if (Math.abs(delta) > 0.5) {
      _lastProgrammaticScrollTop = chatArea.scrollTop + delta;
      chatArea.scrollTop = _lastProgrammaticScrollTop;
    }
  }
}

/**
 * 用户主动滚动检测：wheel/touchstart 事件表示真实用户交互。
 * scroll 事件无法区分「浏览器布局位移」和「用户滚动」，因此用输入事件检测。
 */
function onUserScrollInput() {
  if (_anchorRafId) _userScrolledDuringToggle = true;
}

/** 窗口 resize 防抖计时器（最大化/恢复/拖动窗口后校正滚动位置）。 */
let _resizeTimer = null;

/**
 * 窗口尺寸变化后的滚动校正（REQ-NAV-001 增强）：
 * 最大化/恢复导致文本重排后，若原本贴底则重新钉住底部。
 * WebKit 原生 scroll anchoring 不可靠时的确定性兜底；
 * 200ms 防抖避免拖动窗口过程中与用户交互抢滚动。
 * 侧栏切换动画进行中（_anchorRafId 活跃）不干预。
 */
function initResizeAnchoring() {
  window.addEventListener('resize', () => {
    clearTimeout(_resizeTimer);
    _resizeTimer = setTimeout(() => {
      const chatArea = $('chatArea');
      if (!chatArea || _anchorRafId) return;
      if (chatArea.scrollHeight - chatArea.scrollTop - chatArea.clientHeight <= BOTTOM_ANCHOR_THRESHOLD) {
        _lastProgrammaticScrollTop = chatArea.scrollHeight;
        chatArea.scrollTop = _lastProgrammaticScrollTop;
      }
    }, 200);
  });
}

/** 停止视觉锚定循环。 */
function stopAnchorLoop() {
  if (_anchorRafId) {
    cancelAnimationFrame(_anchorRafId);
    _anchorRafId = 0;
  }
  _anchorEl = null;
}

/** 视觉锚定循环单帧：补偿重排位移后请求下一帧。 */
function anchorLoopFrame() {
  _anchorRafId = 0;
  if (_userScrolledDuringToggle) return; // 用户主动滚动：放弃锚定，循环结束
  const chatArea = $('chatArea');
  if (!chatArea) return;

  if (_pinBottom) {
    // 底部模式：钉住底部（折叠时内容变矮、展开时变高都贴住视口底）
    _lastProgrammaticScrollTop = chatArea.scrollHeight;
    chatArea.scrollTop = _lastProgrammaticScrollTop;
  } else if (_anchorEl && _anchorEl.isConnected) {
    // 中部模式：补偿锚点元素的重排位移，保持其视口偏移不变
    const newOffset = _anchorEl.getBoundingClientRect().top - chatArea.getBoundingClientRect().top;
    const delta = newOffset - _anchorTargetOffset;
    if (Math.abs(delta) > 0.5) {
      _lastProgrammaticScrollTop = chatArea.scrollTop + delta;
      chatArea.scrollTop = _lastProgrammaticScrollTop;
    }
  }
  _anchorRafId = requestAnimationFrame(anchorLoopFrame);
}

/** 启动视觉锚定循环：记录底部/锚点基准状态。 */
function startAnchorLoop(chatArea) {
  if (!chatArea) return;

  // 事件监听器只挂载一次（幂等）
  if (!_scrollListenerAttached) {
    // scroll 事件：动画期间补偿浏览器布局变化导致的滚动位移
    chatArea.addEventListener('scroll', onChatScroll, { passive: true });
    // wheel/touchstart：检测真实用户滚动（区别于浏览器布局位移）
    chatArea.addEventListener('wheel', onUserScrollInput, { passive: true });
    chatArea.addEventListener('touchstart', onUserScrollInput, { passive: true });
    _scrollListenerAttached = true;
  }

  const rect = chatArea.getBoundingClientRect();
  _pinBottom = (chatArea.scrollHeight - chatArea.scrollTop - chatArea.clientHeight) <= BOTTOM_ANCHOR_THRESHOLD;
  const anchor = document.elementFromPoint(rect.left + 40, rect.top + 40);
  if (anchor && anchor !== chatArea) {
    _anchorEl = anchor;
    _anchorTargetOffset = anchor.getBoundingClientRect().top - rect.top;
  } else {
    _anchorEl = null;
  }
  _userScrolledDuringToggle = false;
  _anchorRafId = requestAnimationFrame(anchorLoopFrame);
}

export async function toggleSidebar() {
  const sb = $('sidebar');
  const app = $('app');
  const expanded = $('sidebarExpanded');
  const chatArea = $('chatArea');
  const willCollapse = !sb.classList.contains('sidebar-collapsed');

  // 启动视觉锚定循环（动画期间补偿滚动位置）
  const seq = ++_toggleSeq;
  if (chatArea) startAnchorLoop(chatArea);

  // 切换 CSS 类 — CSS transition 自动处理 padding-left 和 transform 的平滑过渡
  if (willCollapse) {
    sb.classList.add('sidebar-collapsed');
    if (app) app.classList.add('sidebar-collapsed');
    if (expanded) expanded.classList.add('opacity-0', 'pointer-events-none');
    $('collapseBtn').classList.add('hidden');
    $('expandBtn').classList.remove('hidden');
  } else {
    sb.classList.remove('sidebar-collapsed');
    if (app) app.classList.remove('sidebar-collapsed');
    if (expanded) expanded.classList.remove('opacity-0', 'pointer-events-none');
    $('collapseBtn').classList.remove('hidden');
    $('expandBtn').classList.add('hidden');
  }

  // 过渡结束后清理（停止锚定循环）
  // transitionend 优先，setTimeout 兜底
  const finish = () => {
    if (seq !== _toggleSeq) return;
    stopAnchorLoop();
  };
  const onAnimEnd = (ev) => {
    // 只响应 padding-left（main）和 transform（sidebar）的过渡结束
    if (ev.propertyName !== 'padding-left' && ev.propertyName !== 'transform') return;
    sb.removeEventListener('transitionend', onAnimEnd);
    if (app) {
      const main = app.querySelector('main');
      if (main) main.removeEventListener('transitionend', onAnimEnd);
    }
    clearTimeout(_toggleEndTimer);
    _toggleEndTimer = null;
    finish();
  };
  sb.addEventListener('transitionend', onAnimEnd);
  if (app) {
    const main = app.querySelector('main');
    if (main) main.addEventListener('transitionend', onAnimEnd);
  }
  // 兜底：如果 transitionend 未触发
  clearTimeout(_toggleEndTimer);
  _toggleEndTimer = setTimeout(() => {
    sb.removeEventListener('transitionend', onAnimEnd);
    if (app) {
      const main = app.querySelector('main');
      if (main) main.removeEventListener('transitionend', onAnimEnd);
    }
    finish();
  }, 350); // 300ms transition + 50ms buffer

  // 持久化折叠状态（AC-4）
  try {
    await invoke('update_setting', { key: 'ui.sidebar_collapsed', value: String(willCollapse) });
  } catch (_) {
    // IPC 失败时静默降级（E2E 测试环境无 Tauri 运行时）
  }
}

/**
 * 从后端恢复侧栏折叠状态（REQ-NAV-001 AC-4）。
 * 在应用启动时调用。
 *
 * 使用 style.transition='none' 临时禁用过渡避免启动时闪烁。
 */
export async function restoreSidebarState() {
  try {
    const collapsed = await invoke('get_sidebar_collapsed');
    const sb = $('sidebar');
    const app = $('app');
    const expanded = $('sidebarExpanded');
    if (collapsed) {
      // 临时禁用过渡避免启动动画
      sb.style.transition = 'none';
      if (app) {
        const main = app.querySelector('main');
        if (main) main.style.transition = 'none';
      }
      sb.classList.add('sidebar-collapsed');
      if (app) app.classList.add('sidebar-collapsed');
      if (expanded) expanded.classList.add('opacity-0', 'pointer-events-none');
      $('collapseBtn').classList.add('hidden');
      $('expandBtn').classList.remove('hidden');
      // 下一帧恢复过渡
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          sb.style.transition = '';
          if (app) {
            const main = app.querySelector('main');
            if (main) main.style.transition = '';
          }
        });
      });
    }
  } catch (_) {
    // E2E 测试环境或首次启动时静默降级
  }
}

// ============================================================
// 知识库弹框
// ============================================================

/**
 * 打开知识库弹框。加载标签筛选列表后显示。
 */
function openKbModal() {
  const modal = $('kbModal');
  if (modal) {
    modal.classList.remove('hidden');
  }
  loadTagFilter();
}

/**
 * 加载标签筛选列表（REQ-ING-022）。
 * 从后端获取所有标签（含文档计数），填充到 #docTagFilter 下拉框。
 */
async function loadTagFilter() {
  const select = $('docTagFilter');
  if (!select) return;
  // 保留第一个选项（「全部标签」），清除其余
  while (select.options.length > 1) {
    select.remove(1);
  }
  try {
    const tags = await invoke('list_all_tags');
    for (const [tag, count] of tags) {
      const opt = document.createElement('option');
      opt.value = tag;
      opt.textContent = `${tag} (${count})`;
      select.appendChild(opt);
    }
  } catch (_) {
    // E2E 测试环境或 IPC 不可用时静默降级
  }
}

/**
 * 关闭知识库弹框。
 */
function closeKbModal() {
  const modal = $('kbModal');
  if (modal) {
    modal.classList.add('hidden');
  }
}

// ============================================================
// 会话搜索弹框
// ============================================================

/** 搜索弹框当前过滤后的会话列表 */
let _searchFiltered = [];
/** 搜索弹框当前选中索引 */
let _searchSelectedIndex = 0;
/** 搜索模式：'title' 按会话标题搜索 | 'content' 按消息内容搜索（REQ-RAG-040） */
let _searchMode = 'title';
/** 对话搜索结果（content 模式） */
let _messageSearchResults = [];

/**
 * 打开会话搜索弹框：加载全部会话，实时过滤，键盘导航选择。
 */
export function openSearchPopup() {
  const popup = $('convSearchPopup');
  if (!popup) return;
  popup.classList.remove('hidden');
  const input = $('convSearchPopupInput');
  if (input) {
    input.value = '';
    input.focus();
  }
  _searchFiltered = [];
  _messageSearchResults = [];
  _searchSelectedIndex = 0;
  _searchMode = 'title';
  _updateSearchModeUI();
  _renderSearchResults();
  // 注册到面板栈
  pushPanel({ id: 'conv-search-popup', close: closeSearchPopup, element: popup, label: 'Conversation Search' });
}

/** 关闭会话搜索弹框。 */
export function closeSearchPopup() {
  removePanel('conv-search-popup');
  const popup = $('convSearchPopup');
  if (popup) popup.classList.add('hidden');
}

/** 切换搜索模式并更新 UI（REQ-RAG-040）。 */
function _setSearchMode(mode) {
  if (_searchMode === mode) return;
  _searchMode = mode;
  _updateSearchModeUI();
  // 清空结果并重新搜索
  _searchFiltered = [];
  _messageSearchResults = [];
  _searchSelectedIndex = 0;
  const input = $('convSearchPopupInput');
  const term = input ? input.value : '';
  _renderSearchResults(term);
}

/** 更新搜索模式按钮 UI。 */
function _updateSearchModeUI() {
  const titleBtn = $('convSearchModeTitle');
  const contentBtn = $('convSearchModeContent');
  if (titleBtn) {
    if (_searchMode === 'title') {
      titleBtn.className = 'px-2 py-0.5 text-[11px] rounded-xs transition-colors bg-accent/20 text-accent';
    } else {
      titleBtn.className = 'px-2 py-0.5 text-[11px] rounded-xs transition-colors text-text-tertiary hover:text-text-secondary';
    }
  }
  if (contentBtn) {
    if (_searchMode === 'content') {
      contentBtn.className = 'px-2 py-0.5 text-[11px] rounded-xs transition-colors bg-accent/20 text-accent';
    } else {
      contentBtn.className = 'px-2 py-0.5 text-[11px] rounded-xs transition-colors text-text-tertiary hover:text-text-secondary';
    }
  }
  // 更新 placeholder
  const input = $('convSearchPopupInput');
  if (input) {
    input.placeholder = _searchMode === 'content'
      ? t('sidebar.search_messages')
      : t('sidebar.search_conversations');
  }
}

/** 每页渲染数量 */
const SEARCH_PAGE_SIZE = 50;
/** 当前已渲染数量 */
let _searchRenderedCount = 0;

/**
 * 转义 HTML 特殊字符，防止 XSS。
 * @param {string} text
 * @returns {string}
 */
function _escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

/**
 * 高亮搜索关键词：将文本中匹配搜索词的部分包裹在 <mark> 标签中。
 * @param {string} text - 原始文本
 * @param {string} query - 搜索关键词
 * @returns {string} 包含 <mark> 的 HTML 字符串
 */
function _highlightSearchTerm(text, query) {
  const q = query.trim().toLowerCase();
  if (!q || !text) return _escapeHtml(text || '');
  const escaped = _escapeHtml(text);
  const lower = escaped.toLowerCase();
  const pos = lower.indexOf(q.toLowerCase());
  if (pos < 0) return escaped;
  // 转义搜索词中的 HTML 特殊字符后再用于匹配
  const escapedQuery = _escapeHtml(query.trim());
  const before = escaped.substring(0, pos);
  const match = escaped.substring(pos, pos + escapedQuery.length);
  const after = escaped.substring(pos + escapedQuery.length);
  return `${before}<mark class="search-mark">${match}</mark>${after}`;
}

/**
 * 拉取会话列表并渲染到搜索弹框结果区（初始仅渲染前 50 条，滚动加载更多）。
 * content 模式调用 search_conversations IPC 搜索消息内容（REQ-RAG-040）。
 * @param {string} [searchTerm=''] - 搜索关键词
 */
async function _renderSearchResults(searchTerm = '') {
  const term = searchTerm.trim();
  const box = $('convSearchResults');
  if (!box) return;
  box.innerHTML = '';
  _searchSelectedIndex = 0;
  _searchRenderedCount = 0;

  if (_searchMode === 'content') {
    // 对话内容搜索模式（REQ-RAG-040）
    if (!term) {
      _messageSearchResults = [];
      const empty = document.createElement('div');
      empty.className = 'px-4 py-6 text-center text-[11px] text-text-quaternary';
      empty.textContent = t('sidebar.search_messages_hint');
      box.appendChild(empty);
      return;
    }
    try {
      _messageSearchResults = await invoke('search_conversations', { query: term, limit: 50 });
    } catch (_) {
      _messageSearchResults = [];
    }
    if (_messageSearchResults.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'px-4 py-6 text-center text-[11px] text-text-quaternary';
      empty.textContent = t('command_palette.no_results');
      box.appendChild(empty);
      return;
    }
    _renderMessageSearchPage(box);
    return;
  }

  // 会话标题搜索模式（原有逻辑）
  const list = await invoke('get_conversations', { workspaceId: getCurrentWorkspaceId() });
  const lowerTerm = term.toLowerCase();
  _searchFiltered = lowerTerm
    ? list.filter(c => (c.title || '').toLowerCase().includes(lowerTerm))
    : list;

  if (_searchFiltered.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'px-4 py-6 text-center text-[11px] text-text-quaternary';
    empty.textContent = term ? t('command_palette.no_results') : t('chat.empty_messages_desc');
    box.appendChild(empty);
    return;
  }

  _renderSearchPage(box);
}

/**
 * 渲染一页对话搜索结果（content 模式，REQ-RAG-040）。
 * 每条结果显示消息内容摘要 + 会话标题 + 角色标签。
 * @param {HTMLElement} box - 结果容器
 */
function _renderMessageSearchPage(box) {
  const endIndex = Math.min(_searchRenderedCount + SEARCH_PAGE_SIZE, _messageSearchResults.length);
  for (let i = _searchRenderedCount; i < endIndex; i++) {
    const r = _messageSearchResults[i];
    const item = document.createElement('div');
    item.className = `flex flex-col gap-1 px-4 py-2.5 cursor-pointer text-sm transition-colors ${i === _searchSelectedIndex ? 'bg-accent/15' : 'hover:bg-white/5'}`;
    // 角色标签 + 会话标题
    const header = document.createElement('div');
    header.className = 'flex items-center gap-2 text-[11px] text-text-quaternary';
    const roleTag = document.createElement('span');
    roleTag.className = r.role === 'user'
      ? 'px-1 rounded-xs bg-accent/10 text-accent'
      : 'px-1 rounded-xs bg-surface-3 text-text-tertiary';
    roleTag.textContent = r.role === 'user' ? 'Q' : 'A';
    const convTitle = document.createElement('span');
    convTitle.className = 'truncate';
    convTitle.innerHTML = _highlightSearchTerm(r.conversation_title || t('backend.placeholder_title'), _currentSearchTerm);
    header.appendChild(roleTag);
    header.appendChild(convTitle);
    // 消息内容摘要（高亮搜索关键词）
    const content = document.createElement('div');
    content.className = 'text-text-secondary text-xs line-clamp-2';
    const snippet = r.content.length > 120 ? r.content.slice(0, 120) + '…' : r.content;
    content.innerHTML = _highlightSearchTerm(snippet, _currentSearchTerm);
    item.appendChild(header);
    item.appendChild(content);
    item.onclick = () => {
      closeSearchPopup();
      _onSelectConversation(r.conversation_id, _currentSearchTerm);
    };
    box.appendChild(item);
  }
  _searchRenderedCount = endIndex;
}

/**
 * 渲染一页搜索结果（SEARCH_PAGE_SIZE 条）。
 * @param {HTMLElement} box - 结果容器
 */
function _renderSearchPage(box) {
  const endIndex = Math.min(_searchRenderedCount + SEARCH_PAGE_SIZE, _searchFiltered.length);
  for (let i = _searchRenderedCount; i < endIndex; i++) {
    const c = _searchFiltered[i];
    const item = document.createElement('div');
    item.className = `flex items-center gap-2 px-4 py-2.5 cursor-pointer text-sm transition-colors ${i === _searchSelectedIndex ? 'bg-accent/15 text-accent' : 'text-text-secondary hover:bg-white/5'}`;
    const icon = document.createElement('span');
    icon.className = 'shrink-0 text-text-quaternary';
    icon.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>';
    const title = document.createElement('span');
    title.className = 'truncate';
    title.innerHTML = _highlightSearchTerm(c.title || t('backend.placeholder_title'), _currentSearchTerm);
    item.appendChild(icon);
    item.appendChild(title);
    item.onclick = () => {
      closeSearchPopup();
      _onSelectConversation(c.id);
    };
    box.appendChild(item);
  }
  _searchRenderedCount = endIndex;
}

/**
 * 滚动到底部时加载更多搜索结果。
 */
function _loadMoreSearchResults() {
  if (_searchMode === 'content') {
    if (_searchRenderedCount >= _messageSearchResults.length) return;
    const box = $('convSearchResults');
    if (!box) return;
    _renderMessageSearchPage(box);
    return;
  }
  if (_searchRenderedCount >= _searchFiltered.length) return;
  const box = $('convSearchResults');
  if (!box) return;
  _renderSearchPage(box);
}

/** 选中会话后的回调（由 main.js 注入）。 */
/** @type {(id: string, searchQuery?: string) => Promise<void>} */
let _onSelectConversation = async (id) => {};

/** 当前搜索关键词（用于跳转高亮时传递） */
let _currentSearchTerm = '';

/**
 * 初始化搜索弹框事件绑定。
 * @param {(id: string, searchQuery?: string) => Promise<void>} onSelectConversation - 选中会话后的回调
 */
export function initSearchPopup(onSelectConversation) {
  _onSelectConversation = onSelectConversation || (async () => {});
  const popup = $('convSearchPopup');
  if (!popup) return;
  const input = $('convSearchPopupInput');

  // 会话搜索弹框关闭按钮
  if ($('convSearchClose')) {
    $('convSearchClose').onclick = closeSearchPopup;
  }

  // 搜索模式切换按钮（REQ-RAG-040）
  if ($('convSearchModeTitle')) {
    $('convSearchModeTitle').onclick = () => _setSearchMode('title');
  }
  if ($('convSearchModeContent')) {
    $('convSearchModeContent').onclick = () => _setSearchMode('content');
  }

  // 实时过滤
  if (input) {
    let timer = null;
    input.addEventListener('input', () => {
      clearTimeout(timer);
      _currentSearchTerm = input.value.trim();
      timer = setTimeout(() => _renderSearchResults(input.value), 150);
    });
    // 键盘导航
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        closeSearchPopup();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (_searchFiltered.length === 0) return;
        _searchSelectedIndex = Math.min(_searchSelectedIndex + 1, _searchFiltered.length - 1);
        _updateSearchSelection();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (_searchFiltered.length === 0) return;
        _searchSelectedIndex = Math.max(_searchSelectedIndex - 1, 0);
        _updateSearchSelection();
      } else if (e.key === 'Enter') {
        if (isComposingEvent(e)) return; // IME 组合中不触发
        e.preventDefault();
        if (_searchFiltered.length === 0) return;
        const conv = _searchFiltered[_searchSelectedIndex];
        if (conv) {
          closeSearchPopup();
          _onSelectConversation(conv.id);
        }
      }
    });
  }

  // 无限滚动：滚动到底部时加载更多
  const resultsBox = $('convSearchResults');
  if (resultsBox) {
    resultsBox.addEventListener('scroll', () => {
      if (resultsBox.scrollTop + resultsBox.clientHeight >= resultsBox.scrollHeight - 50) {
        _loadMoreSearchResults();
      }
    });
  }
}

/** 更新搜索弹框中的选中项样式。 */
function _updateSearchSelection() {
  const box = $('convSearchResults');
  if (!box) return;

  // 如果选中索引超出已渲染范围，先加载更多
  if (_searchSelectedIndex >= _searchRenderedCount) {
    if (_searchMode === 'content') {
      _renderMessageSearchPage(box);
    } else {
      _renderSearchPage(box);
    }
  }

  const items = box.querySelectorAll('[class*="cursor-pointer"]');
  items.forEach((item, i) => {
    if (i === _searchSelectedIndex) {
      item.classList.add('bg-accent/15');
      item.classList.remove('hover:bg-white/5');
    } else {
      item.classList.remove('bg-accent/15');
      item.classList.add('hover:bg-white/5');
    }
  });
  // 滚动到选中项
  const selected = items[_searchSelectedIndex];
  if (selected) selected.scrollIntoView({ block: 'nearest' });
}

/**
 * 初始化侧栏事件绑定。
 */
/** 筛选条件变更回调（由 main.js 注入，调用 applyKbFilters 而非重新请求后端） */
let _onFilterChange = () => {};

/**
 * 设置筛选条件变更回调（由 main.js 在初始化时注入）。
 * 搜索输入和筛选器变化时调用此回调，触发前端缓存筛选+重新渲染。
 * @param {() => void} fn - 回调函数
 */
export function setFilterChangeCallback(fn) {
  _onFilterChange = fn || (() => {});
}

/** 搜索防抖计时器 */
let _kbSearchTimer = null;

export function initSidebar() {
  $('collapseBtn').onclick = toggleSidebar;
  $('expandBtn').onclick = toggleSidebar;
  // 窗口最大化/恢复后滚动位置校正（底部贴底）
  initResizeAnchoring();

  // 知识库按钮 → 打开 KB Modal
  if ($('kbBtn')) {
    $('kbBtn').onclick = openKbModal;
  }
  if ($('kbCloseBtn')) {
    $('kbCloseBtn').onclick = closeKbModal;
  }
  if ($('kbImport')) {
    $('kbImport').onclick = () => { closeKbModal(); $('plusBtn').click(); };
  }
  // 背景点击关闭（REQ-NAV-002 AC-3：点击遮罩区域关闭弹框）
  if ($('kbModal')) {
    $('kbModal').addEventListener('click', (e) => {
      if (e.target === $('kbModal')) {
        closeKbModal();
      }
    });
  }

// 筛选面板切换（默认折叠）
if ($('kbFilterToggle')) {
$('kbFilterToggle').onclick = () => {
const panel = $('kbFilterPanel');
if (panel) panel.classList.toggle('hidden');
};
}

// 排序面板切换（v1.10 REQ-ING-008）
if ($('kbSortBtn')) {
$('kbSortBtn').onclick = () => {
const panel = $('kbSortPanel');
if (panel) panel.classList.toggle('hidden');
};
}

  // 文档搜索（300ms 防抖）+ 筛选器变化
  if ($('docSearchInput')) {
    $('docSearchInput').addEventListener('input', () => {
      if (_kbSearchTimer) clearTimeout(_kbSearchTimer);
      _kbSearchTimer = setTimeout(() => _onFilterChange(), 300);
    });
  }
  if ($('docStatusFilter')) {
    $('docStatusFilter').addEventListener('change', () => _onFilterChange());
  }
  if ($('docFormatFilter')) {
    $('docFormatFilter').addEventListener('change', () => _onFilterChange());
  }
  if ($('docTagFilter')) {
    $('docTagFilter').addEventListener('change', () => _onFilterChange());
  }

  // 多选模式 + 批量操作（REQ-ING-024 增强：批量删除/移动/标签）
  if ($('kbSelectToggle')) {
    $('kbSelectToggle').onclick = toggleMultiSelect;
  }
  if ($('kbBatchCancel')) {
    $('kbBatchCancel').onclick = exitMultiSelect;
  }
  if ($('kbBatchDelete')) {
    $('kbBatchDelete').onclick = showBatchDeleteConfirm;
  }
  if ($('kbBatchMove')) {
    $('kbBatchMove').onclick = showBatchMoveDialog;
  }
  if ($('kbBatchTag')) {
    $('kbBatchTag').onclick = showBatchTagDialog;
  }
  if ($('kbSelectAll')) {
    $('kbSelectAll').onchange = toggleSelectAll;
  }
  // 旧 kbConfirmDialog 按钮事件移除（已迁移到 showConfirmDialog 统一组件）
  if ($('kbConfirmCancel')) {
    $('kbConfirmCancel').onclick = cancelBatchDelete;
  }
  if ($('kbConfirmOk')) {
    $('kbConfirmOk').onclick = confirmBatchDelete;
  }
}

// ============================================================
// 多选模式 + 批量删除（REQ-ING-009）
// ============================================================

/** 多选模式是否激活 */
let _multiSelectMode = false;

/**
 * 获取多选模式状态。
 * @returns {boolean} 多选模式是否激活
 */
export function isMultiSelectMode() {
  return _multiSelectMode;
}

/** 文档列表重新加载回调（由 main.js 注入） */
let _onReloadDocs = () => {};

/**
 * 设置文档列表重新加载回调（由 main.js 在初始化时注入）。
 * @param {() => void} fn - 回调函数
 */
export function setReloadDocsCallback(fn) {
  _onReloadDocs = fn || (() => {});
}

/**
 * 切换多选模式：进入时显示复选框和批量操作栏，退出时恢复常规模式。
 */
export function toggleMultiSelect() {
  _multiSelectMode = !_multiSelectMode;
  const batchBar = $('kbBatchBar');
  const footer = $('kbFooter');
  if (_multiSelectMode) {
    if (batchBar) { batchBar.classList.remove('hidden'); batchBar.classList.add('flex'); }
    if (footer) { footer.classList.add('hidden'); }
  } else {
    if (batchBar) { batchBar.classList.add('hidden'); batchBar.classList.remove('flex'); }
    if (footer) { footer.classList.remove('hidden'); }
    // 清除所有选中状态
    document.querySelectorAll('#docList input[type="checkbox"]').forEach((cb) => { cb.checked = false; });
  }
  updateSelectedCount();
  // 触发文档列表重新渲染（切换复选框显示）
  _onReloadDocs();
}

/**
 * 退出多选模式（恢复常规模式）。
 */
export function exitMultiSelect() {
  if (_multiSelectMode) toggleMultiSelect();
}

/**
 * 更新已选文档数量显示。
 */
function updateSelectedCount() {
  const checked = document.querySelectorAll('#docList input[type="checkbox"]:checked').length;
  const countEl = $('kbSelectedCount');
  if (countEl) countEl.textContent = t('knowledge_base.selected_count', { count: checked });
}

/**
 * 全选/反选所有文档复选框（REQ-ING-024 AC-1）。
 */
function toggleSelectAll() {
  const selectAll = $('kbSelectAll');
  if (!selectAll) return;
  const checked = selectAll.checked;
  document.querySelectorAll('#docList input[type="checkbox"]').forEach((cb) => {
    cb.checked = checked;
  });
  updateSelectedCount();
}

/**
 * 显示批量删除确认对话框（REQ-ING-024 AC-3，REQ-IX-005 统一确认对话框）。
 * 使用 batch_delete_documents IPC 批量删除（事务保证）。
 */
async function showBatchDeleteConfirm() {
  const checked = document.querySelectorAll('#docList input[type="checkbox"]:checked');
  if (checked.length === 0) return;

  const count = checked.length;
  const ok = await showConfirmDialog({
    title: t('knowledge_base.confirm_delete_title'),
    body: `${count} ${t('knowledge_base.docs_to_delete')}`,
    confirmText: t('knowledge_base.confirm_delete'),
    danger: true,
  });
  if (!ok) return;

  const docIds = Array.from(checked).map((cb) => cb.dataset.docId).filter(Boolean);
  if (docIds.length === 0) return;

  try {
    const result = await invoke('batch_delete_documents', { ids: docIds });
    if (result.failedCount > 0) {
      toastError(t('knowledge_base.batch_delete_partial', {
        success: result.success_count,
        failed: result.failed_count,
      }));
    } else {
      toast(t('knowledge_base.batch_deleted', { count: result.success_count }), 'success');
    }
  } catch (err) {
    toastError(t('knowledge_base.batch_delete_failed'));
  }

  // 退出多选模式并刷新列表
  exitMultiSelect();
  _onReloadDocs();
}

/**
 * 显示批量移动对话框（REQ-ING-024 AC-4）。
 * 加载工作空间列表 → 用户选择目标 → 调用 batch_move_documents IPC。
 */
async function showBatchMoveDialog() {
  const checked = document.querySelectorAll('#docList input[type="checkbox"]:checked');
  if (checked.length === 0) return;

  const docIds = Array.from(checked).map((cb) => cb.dataset.docId).filter(Boolean);
  if (docIds.length === 0) return;

  // 加载工作空间列表
  let workspaces = [];
  try {
    workspaces = await invoke('list_workspaces');
  } catch (_) {
    workspaces = [];
  }

  if (workspaces.length <= 1) {
    toastError(t('knowledge_base.no_other_workspaces'));
    return;
  }

  // 构建选择对话框
  const ok = await showConfirmDialog({
    title: t('knowledge_base.batch_move_title'),
    body: t('knowledge_base.batch_move_body', { count: docIds.length }),
    confirmText: t('knowledge_base.batch_move_confirm'),
    customContent: (() => {
      const select = document.createElement('select');
      select.className = 'w-full mt-2 bg-surface-2 border border-border-default rounded-lg px-3 py-2 text-sm text-text-primary';
      select.id = 'batchMoveTarget';
      for (const ws of workspaces) {
        const opt = document.createElement('option');
        opt.value = ws.id;
        opt.textContent = ws.name;
        select.appendChild(opt);
      }
      return select;
    })(),
  });

  if (!ok) return;

  const targetSelect = document.getElementById('batchMoveTarget');
  if (!targetSelect) return;
  const targetWorkspaceId = targetSelect.value;

  try {
    const result = await invoke('batch_move_documents', {
      ids: docIds,
      targetWorkspaceId,
    });
    if (result.failedCount > 0) {
      toastError(t('knowledge_base.batch_move_partial', {
        success: result.success_count,
        failed: result.failed_count,
      }));
    } else {
      toast(t('knowledge_base.batch_moved', { count: result.success_count }), 'success');
    }
  } catch (err) {
    toastError(String(err?.message || err || t('knowledge_base.batch_move_failed')));
  }

  exitMultiSelect();
  _onReloadDocs();
}

/**
 * 显示批量标签对话框（REQ-ING-024 AC-5）。
 * 用户输入标签名（逗号分隔）→ 调用 batch_add_tags IPC。
 */
async function showBatchTagDialog() {
  const checked = document.querySelectorAll('#docList input[type="checkbox"]:checked');
  if (checked.length === 0) return;

  const docIds = Array.from(checked).map((cb) => cb.dataset.docId).filter(Boolean);
  if (docIds.length === 0) return;

  const ok = await showConfirmDialog({
    title: t('knowledge_base.batch_tag_title'),
    body: t('knowledge_base.batch_tag_body', { count: docIds.length }),
    confirmText: t('knowledge_base.batch_tag_confirm'),
    customContent: (() => {
      const input = document.createElement('input');
      input.type = 'text';
      input.id = 'batchTagInput';
      input.className = 'w-full mt-2 bg-surface-2 border border-border-default rounded-lg px-3 py-2 text-sm text-text-primary';
      input.placeholder = t('knowledge_base.batch_tag_placeholder');
      return input;
    })(),
  });

  if (!ok) return;

  const tagInput = document.getElementById('batchTagInput');
  if (!tagInput) return;
  const tagText = tagInput.value.trim();
  if (!tagText) return;

  // 逗号分隔标签
  const tags = tagText.split(',').map((s) => s.trim()).filter(Boolean);

  try {
    const result = await invoke('batch_add_tags', { ids: docIds, tags });
    if (result.failedCount > 0) {
      toastError(t('knowledge_base.batch_tag_partial', {
        success: result.success_count,
        failed: result.failed_count,
      }));
    } else {
      toast(t('knowledge_base.batch_tagged', { count: result.success_count }), 'success');
    }
  } catch (err) {
    toastError(String(err?.message || err || t('knowledge_base.batch_tag_failed')));
  }

  exitMultiSelect();
  _onReloadDocs();
}

/**
 * 取消批量删除（隐藏旧确认对话框 — 兼容保留）。
 * @deprecated 已迁移到 showConfirmDialog 统一组件，此函数仅保留兼容旧 HTML 按钮事件。
 */
function cancelBatchDelete() {
  const dialog = $('kbConfirmDialog');
  if (dialog) dialog.classList.add('hidden');
}

/**
 * 确认批量删除（兼容旧 HTML 按钮事件 — 已由 showConfirmDialog 统一处理）。
 * @deprecated 新流程在 showBatchDeleteConfirm() 内直接执行删除。
 */
async function confirmBatchDelete() {
  const dialog = $('kbConfirmDialog');
  if (dialog) dialog.classList.add('hidden');
  // 实际删除逻辑已迁移到 showBatchDeleteConfirm() 中的 Promise.then 回调
}

/**
 * 筛选文档列表 — 委托给 main.js 的 applyKbFilters 回调。
 * 保留导出以兼容旧调用点（如 main.js 的 import）。
 */
export function filterDocuments() {
  _onFilterChange();
}
