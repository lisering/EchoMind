/**
 * EchoMind 对话导航条模块 — DeepSeek 风格右侧导航。
 *
 * 聊天区右侧细长导航条，每个 Q&A 对应一段可点击的条段。
 * - 悬停弹出问题摘要 tooltip（独立 fixed 元素，不受父元素变形影响）
 * - 点击跳转到对应消息
 * - 高亮当前可见的 Q&A 段
 * - 使用 MutationObserver 自动检测聊天区变化
 *
 * 设计灵感：chat.deepseek.com 右侧对话导航
 */

import { $ } from './utils.js';

/** @type {Array<{element: HTMLElement, segment: HTMLElement}>} */
let _navItems = [];

/** @type {IntersectionObserver|null} */
let _observer = null;

/** 当前高亮的段索引 */
let _activeIndex = -1;

/** MutationObserver 实例 */
let _mutationObserver = null;

/** 防抖定时器 */
let _updateTimer = null;

/** 独立 tooltip 元素（不挂载在 segment 内部，避免变形） */
let _tooltipEl = null;

/**
 * 创建独立的 tooltip 元素（全局唯一，挂在 body 下）。
 */
function _ensureTooltip() {
  if (_tooltipEl && document.body.contains(_tooltipEl)) return;
  _tooltipEl = document.createElement('div');
  _tooltipEl.className = 'conv-nav-tooltip';
  _tooltipEl.style.display = 'none';
  document.body.appendChild(_tooltipEl);
}

/**
 * 显示 tooltip：根据 segment 的屏幕位置，将 tooltip 定位在其左侧。
 * @param {HTMLElement} segment - 被悬停的导航段
 * @param {string} text - 要显示的文本
 */
function _showTooltip(segment, text) {
  _ensureTooltip();
  const rect = segment.getBoundingClientRect();
  _tooltipEl.textContent = text;
  _tooltipEl.style.display = 'block';
  // 定位：tooltip 右边缘紧贴 segment 左侧，垂直居中
  const tipRect = _tooltipEl.getBoundingClientRect();
  _tooltipEl.style.left = `${rect.left - tipRect.width - 8}px`;
  _tooltipEl.style.top = `${rect.top + rect.height / 2 - tipRect.height / 2}px`;
  _tooltipEl.style.opacity = '1';
}

/** 隐藏 tooltip。 */
function _hideTooltip() {
  if (!_tooltipEl) return;
  _tooltipEl.style.opacity = '0';
  _tooltipEl.style.display = 'none';
}

/**
 * 扫描聊天区中的 Q&A 消息块，构建导航条段。
 * 每个 user 消息块（含 justify-end 的子元素）对应一个导航段。
 */
function _scanAndBuildNav() {
  const nav = $('convNav');
  const chatArea = $('chatArea');
  if (!nav || !chatArea) return;

  // 清空旧数据
  nav.innerHTML = '';
  _navItems = [];
  _activeIndex = -1;

  // 查找所有消息块（msg-block message-in 是 appendBlock 创建的类名）
  const blocks = chatArea.querySelectorAll('.animate-message-in');

  // 筛选出 user 消息块（含 .msg-user-content 的是 user 消息）
  const userBlocks = [];
  blocks.forEach((block) => {
    const userBubble = block.querySelector('.msg-user-content');
    if (userBubble) {
      userBlocks.push({ block, bubble: userBubble });
    }
  });

  if (userBlocks.length === 0) {
    nav.classList.remove('has-items');
    return;
  }

  nav.classList.add('has-items');

  // 为每个 user 消息创建一个导航段
  userBlocks.forEach((item, idx) => {
    const segment = document.createElement('div');
    segment.className = 'conv-nav-segment';
    segment.dataset.idx = String(idx);

    // 提取问题文本（截断到 60 字符）
    const questionText = item.bubble.textContent?.trim().slice(0, 60) || `Q${idx + 1}`;

    // 悬停时显示独立 tooltip（不用子元素，避免继承变形）
    segment.addEventListener('mouseenter', () => _showTooltip(segment, questionText));
    segment.addEventListener('mouseleave', _hideTooltip);

    // 点击跳转
    segment.addEventListener('click', () => {
      item.block.scrollIntoView({ behavior: 'smooth', block: 'start' });
    });

    nav.appendChild(segment);
    _navItems.push({ element: item.block, segment });
  });

  // 设置 IntersectionObserver 追踪当前可见的 Q&A
  _setupObserver();
}

/**
 * 设置 IntersectionObserver 追踪当前可见的消息块。
 */
function _setupObserver() {
  if (_observer) {
    _observer.disconnect();
  }

  const chatArea = $('chatArea');
  if (!chatArea || _navItems.length === 0) return;

  _observer = new IntersectionObserver(
    () => {
      // 找到最顶部可见的条目
      let topVisibleIdx = -1;
      let topVisibleTop = Infinity;

      _navItems.forEach((item, idx) => {
        const rect = item.element.getBoundingClientRect();
        const chatRect = chatArea.getBoundingClientRect();
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
    },
    { root: chatArea, threshold: [0, 0.1, 0.5, 1.0] },
  );

  // 观察所有导航项对应的元素
  _navItems.forEach((item) => {
    _observer.observe(item.element);
  });
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
}

/**
 * 初始化对话导航条。
 * 使用 MutationObserver 监听聊天区变化，自动更新导航条。
 */
export function initConvNav() {
  const chatArea = $('chatArea');
  if (!chatArea) return;

  // 创建独立 tooltip 元素
  _ensureTooltip();

  // 初始构建
  _scanAndBuildNav();

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
 * 销毁对话导航条（清理观察者和 tooltip）。
 */
export function destroyConvNav() {
  if (_observer) {
    _observer.disconnect();
    _observer = null;
  }
  if (_mutationObserver) {
    _mutationObserver.disconnect();
    _mutationObserver = null;
  }
  if (_tooltipEl) {
    _tooltipEl.remove();
    _tooltipEl = null;
  }
  _navItems = [];
  _activeIndex = -1;
}
