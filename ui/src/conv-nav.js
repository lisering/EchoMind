/**
 * EchoMind 对话导航面板模块 — 右侧布局列（v2 重设计）。
 *
 * v2 变更：从 fixed 浮层改为正式布局列，面板始终可见（可收起）。
 *
 * DOM 结构：
 *   #rightPanel (.right-panel)        — 右侧面板容器（aside，Grid 布局列）
 *   └ #convNav (.conv-nav)            — 面板内容区
 *     └ .conv-nav-header              — 头部（标题 + 收起按钮）
 *     └ .conv-nav-list                — 滚动列表
 *       └ .conv-nav-segment            — 列表项 [badge] + [文本] + [indicator]
 *     └ .conv-nav-empty               — 空状态提示（无消息时）
 *
 * 交互：
 * - 点击 topBar 右侧面板按钮 → 切换收起/展开
 * - 点击列表项 → 平滑滚动到对应消息
 * - 滚动聊天区 → 自动高亮当前可见项
 */

import { $ } from './utils.js';
import { t } from './i18n.js';

/** @type {Array<{element: HTMLElement, segment: HTMLElement}>} */
let _navItems = [];

/** IntersectionObserver 实例 */
let _observer = null;

/** 当前高亮的段索引 */
let _activeIndex = -1;

/** MutationObserver 实例 */
let _mutationObserver = null;

/** 防抖定时器 */
let _updateTimer = null;

/** 滚动监听（用于追踪当前可见项） */
let _scrollHandler = null;

/** 内层列表容器引用 */
let _listContainer = null;

/** 面板是否已初始化 */
let _initialized = false;

/** localStorage 键名 */
const PANEL_STATE_KEY = 'echomind.rightPanel.collapsed';

/**
 * 从 localStorage 读取面板收起状态。
 * @returns {boolean} true=收起, false=展开
 */
function _loadPanelState() {
  try {
    return localStorage.getItem(PANEL_STATE_KEY) === 'true';
  } catch {
    return false;
  }
}

/**
 * 保存面板收起状态到 localStorage。
 * @param {boolean} collapsed
 */
function _savePanelState(collapsed) {
  try {
    localStorage.setItem(PANEL_STATE_KEY, String(collapsed));
  } catch {
    // localStorage 不可用时静默失败
  }
}

/**
 * 切换右侧面板收起/展开。
 */
export function toggleRightPanel() {
  const panel = $('rightPanel');
  if (!panel) return;
  const isCollapsed = panel.classList.contains('right-panel-collapsed');
  if (isCollapsed) {
    panel.classList.remove('right-panel-collapsed');
    _savePanelState(false);
  } else {
    panel.classList.add('right-panel-collapsed');
    _savePanelState(true);
  }
}

/**
 * 初始化右侧面板切换按钮。
 */
export function initRightPanelToggle() {
  const btn = $('rightPanelToggle');
  if (!btn) return;
  btn.addEventListener('click', toggleRightPanel);

  // 恢复上次状态
  const panel = $('rightPanel');
  if (panel && _loadPanelState()) {
    panel.classList.add('right-panel-collapsed');
  }
}

/**
 * 构建面板头部 HTML。
 */
function _buildHeader() {
  const header = document.createElement('div');
  header.className = 'conv-nav-header';

  const title = document.createElement('span');
  title.className = 'conv-nav-header-title';
  title.textContent = t('conv_nav.title', 'Q&A Navigation');

  const closeBtn = document.createElement('button');
  closeBtn.className = 'conv-nav-close-btn';
  closeBtn.setAttribute('aria-label', t('common.close', 'Close'));
  closeBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  closeBtn.addEventListener('click', toggleRightPanel);

  header.appendChild(title);
  header.appendChild(closeBtn);
  return header;
}

/**
 * 扫描聊天区中的 Q&A 消息块，构建导航条段。
 * 每个 user 消息块对应一个导航列表项。
 */
function _scanAndBuildNav() {
  const nav = $('convNav');
  const chatArea = $('chatArea');
  if (!nav || !chatArea) return;

  // 清空旧数据
  nav.innerHTML = '';
  _navItems = [];
  _activeIndex = -1;
  _listContainer = null;

  // 始终添加头部
  nav.appendChild(_buildHeader());

  // 查找所有消息块
  const blocks = chatArea.querySelectorAll('.animate-message-in');

  // 筛选出 user 消息块
  const userBlocks = [];
  blocks.forEach((block) => {
    const userBubble = block.querySelector('.msg-user-content');
    if (userBubble) {
      userBlocks.push({ block, bubble: userBubble });
    }
  });

  if (userBlocks.length === 0) {
    // 空状态提示
    const empty = document.createElement('div');
    empty.className = 'conv-nav-empty';
    empty.textContent = t('conv_nav.empty', 'No questions yet. Start a conversation to see navigation.');
    nav.appendChild(empty);
    return;
  }

  // 创建内层滚动容器
  const listContainer = document.createElement('div');
  listContainer.className = 'conv-nav-list';
  _listContainer = listContainer;

  // 为每个 user 消息创建导航列表项
  userBlocks.forEach((item, idx) => {
    const segment = document.createElement('div');
    segment.className = 'conv-nav-segment';
    segment.dataset.idx = String(idx);

    // 提取问题文本（截断到 80 字符）
    const questionText = item.bubble.textContent?.trim().slice(0, 80) || `Q${idx + 1}`;

    // 序号 badge
    const badge = document.createElement('span');
    badge.className = 'conv-nav-badge';
    badge.textContent = String(idx + 1);
    segment.appendChild(badge);

    // 问题文本
    const textLabel = document.createElement('span');
    textLabel.className = 'conv-nav-segment-text';
    textLabel.textContent = questionText;
    segment.appendChild(textLabel);

    // 右侧图标容器 + 指示条
    const icon = document.createElement('span');
    icon.className = 'conv-nav-icon';
    const indicator = document.createElement('span');
    indicator.className = 'conv-nav-indicator';
    icon.appendChild(indicator);
    segment.appendChild(icon);

    // 点击跳转
    segment.addEventListener('click', () => {
      item.block.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });

    listContainer.appendChild(segment);
    _navItems.push({ element: item.block, segment });
  });

  // 将列表容器添加到面板
  nav.appendChild(listContainer);

  // 设置滚动监听追踪当前可见项
  _setupScrollListener();
}

/**
 * 设置滚动监听 + IntersectionObserver 追踪当前可见的消息块。
 */
function _setupScrollListener() {
  if (_observer) {
    _observer.disconnect();
  }

  const chatArea = $('chatArea');
  if (!chatArea || _navItems.length === 0) return;

  // 滚动时更新当前项
  _scrollHandler = () => {
    _updateActiveFromScroll();
  };
  chatArea.addEventListener('scroll', _scrollHandler, { passive: true });

  // 初始更新一次
  _updateActiveFromScroll();
}

/**
 * 根据滚动位置更新当前高亮项。
 * 找到在聊天区可视范围内最靠顶部的 user 消息块。
 */
function _updateActiveFromScroll() {
  const chatArea = $('chatArea');
  if (!chatArea || _navItems.length === 0) return;

  const chatRect = chatArea.getBoundingClientRect();
  let topVisibleIdx = -1;
  let topVisibleTop = Infinity;

  _navItems.forEach((item, idx) => {
    const rect = item.element.getBoundingClientRect();
    // 判断元素是否在聊天区可视范围内
    if (rect.bottom > chatRect.top && rect.top < chatRect.bottom) {
      if (rect.top < topVisibleTop) {
        topVisibleTop = rect.top;
        topVisibleIdx = idx;
      }
    }
  });

  if (topVisibleIdx !== -1 && topVisibleIdx !== _activeIndex) {
    _setActive(topVisibleIdx);
  }
}

/**
 * 设置当前高亮的导航段。
 * @param {number} idx - 导航段索引
 */
function _setActive(idx) {
  _activeIndex = idx;
  _navItems.forEach((item, i) => {
    if (i === idx) {
      item.segment.classList.add('active');
    } else {
      item.segment.classList.remove('active');
    }
  });

  // 如果列表有滚动条，滚动到当前项可见
  if (_listContainer && _navItems[idx]) {
    const segment = _navItems[idx].segment;
    const listRect = _listContainer.getBoundingClientRect();
    const segRect = segment.getBoundingClientRect();
    // 如果当前项不在可见范围内，滚动到它
    if (segRect.top < listRect.top || segRect.bottom > listRect.bottom) {
      segment.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
  }
}

/**
 * 初始化对话导航条。
 * 使用 MutationObserver 监听聊天区变化，自动更新导航条。
 */
export function initConvNav() {
  const chatArea = $('chatArea');
  if (!chatArea) return;

  // 初始化面板切换按钮
  initRightPanelToggle();

  // 初始构建
  _scanAndBuildNav();
  _initialized = true;

  // 监听聊天区 DOM 变化（防抖 200ms）
  _mutationObserver = new MutationObserver(() => {
    clearTimeout(_updateTimer);
    _updateTimer = setTimeout(() => {
      _scanAndBuildNav();
    }, 200);
  });

  _mutationObserver.observe(chatArea, {
    childList: true,
    subtree: false,
  });
}

/**
 * 销毁对话导航条（清理观察者和事件监听）。
 */
export function destroyConvNav() {
  if (_observer) {
    _observer.disconnect();
    _observer = null;
  }
  if (_scrollHandler) {
    const chatArea = $('chatArea');
    if (chatArea) chatArea.removeEventListener('scroll', _scrollHandler);
    _scrollHandler = null;
  }
  if (_mutationObserver) {
    _mutationObserver.disconnect();
    _mutationObserver = null;
  }
  _navItems = [];
  _activeIndex = -1;
  _listContainer = null;
  _initialized = false;
}
