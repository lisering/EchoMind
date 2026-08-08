/**
 * EchoMind 重新生成轮播模块 — 多次重新生成的回答以轮播模式展示。
 *
 * 职责：
 * 1. 在 assistant 消息块中创建轮播容器
 * 2. 每次重新生成的回答作为一个版本加入轮播
 * 3. 支持 ◀ 1/N ▶ 导航（鼠标点击 + 键盘 ← →）
 * 4. 每个版本独立保留内容和来源
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.13 重新生成轮播
 * AC-QA-014：重新生成改为轮播模式（不再追加末尾）
 */

import { t } from './i18n.js';

// ============================================================
// 类型定义（JSDoc）
// ============================================================

/**
 * @typedef {Object} CarouselVersion
 * @property {string} content - Markdown 原文
 * @property {Array|null} sources - 引用来源
 */

/**
 * @typedef {Object} CarouselState
 * @property {number} current - 当前选中版本索引（0-based）
 * @property {number} total - 版本总数
 * @property {CarouselVersion[]} versions - 所有版本
 */

// ============================================================
// 创建轮播
// ============================================================

/**
 * 在 assistant 消息块中创建轮播容器。
 *
 * 容器插入在 .msg-content 之后、.msg-actions 之前。
 * 初始版本数为 0，在 addCarouselVersion 后增加。
 *
 * @param {HTMLElement} blockEl - assistant 消息块根元素
 * @returns {HTMLElement} 创建的 .regen-carousel 元素
 */
export function createCarousel(blockEl) {
  // 检查是否已有轮播容器
  const existing = blockEl.querySelector('.regen-carousel');
  // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
  if (existing) return existing;

  const carousel = document.createElement('div');
  carousel.className = 'regen-carousel';
  carousel.dataset.currentIndex = '0';
  carousel.dataset.total = '0';

  // 存储版本数据
  /** @type {CarouselVersion[]} */
  carousel._versions = [];

  // 插入到 .msg-content 之后
  const content = blockEl.querySelector('.msg-content');
  if (content && content.nextSibling) {
    content.parentNode.insertBefore(carousel, content.nextSibling);
  } else if (content) {
    content.parentNode.appendChild(carousel);
  } else {
    blockEl.appendChild(carousel);
  }

  return carousel;
}

// ============================================================
// 添加版本
// ============================================================

/**
 * 向轮播中添加一个版本。
 *
 * 新版本添加到末尾，当前索引指向最新版本。
 *
 * @param {HTMLElement} carousel - 轮播容器
 * @param {string} content - Markdown 原文
 * @param {Array|null} sources - 引用来源
 * @returns {number} 新版本的索引
 */
export function addCarouselVersion(carousel, content, sources) {
  if (!carousel._versions) {
    carousel._versions = [];
  }

  /** @type {CarouselVersion} */
  const version = { content, sources };
  carousel._versions.push(version);

  const newIndex = carousel._versions.length - 1;
  carousel.dataset.total = String(carousel._versions.length);
  carousel.dataset.currentIndex = String(newIndex);

  // 如果这是第一个版本之外的版本，渲染控制
  if (carousel._versions.length > 1) {
    renderCarouselControls(carousel);
  }

  return newIndex;
}

// ============================================================
// 导航
// ============================================================

/**
 * 在轮播中导航到上一个或下一个版本。
 *
 * - 'left' → 选中上一个版本，首项时回绕到末项
 * - 'right' → 选中下一个版本，末项时回绕到首项
 * - 仅 1 个版本时不改变索引
 *
 * @param {HTMLElement} carousel - 轮播容器
 * @param {'left'|'right'} direction - 导航方向
 */
export function navigateCarousel(carousel, direction) {
  const total = carousel._versions ? carousel._versions.length : 0;
  if (total <= 1) return;

  let current = parseInt(carousel.dataset.currentIndex || '0', 10);

  if (direction === 'left') {
    current = (current - 1 + total) % total;
  } else {
    current = (current + 1) % total;
  }

  carousel.dataset.currentIndex = String(current);

  // 更新控制显示
  updateControlsDisplay(carousel);
}

// ============================================================
// 状态查询
// ============================================================

/**
 * 获取轮播当前状态。
 *
 * @param {HTMLElement} carousel - 轮播容器
 * @returns {CarouselState} 当前状态（current/total/versions）
 */
export function getCarouselState(carousel) {
  const versions = /** @type {CarouselVersion[]} */ (carousel._versions || []);
  const current = parseInt(carousel.dataset.currentIndex || '0', 10);
  return {
    current,
    total: versions.length,
    versions,
  };
}

// ============================================================
// 显示更新
// ============================================================

/**
 * 更新 .md 容器显示当前选中版本的内容。
 *
 * 将当前版本的 content 写入 mdContainer。
 *
 * @param {HTMLElement} carousel - 轮播容器
 * @param {HTMLElement} mdContainer - .md 渲染容器
 * @returns {void}
 */
export function updateCarouselDisplay(carousel, mdContainer) {
  const state = getCarouselState(carousel);
  if (state.total === 0 || !state.versions[state.current]) return;

  const version = state.versions[state.current];
  // 写入原始 Markdown，由 markdown.js 的 renderMarkdown 进行渲染
  mdContainer.textContent = version.content;
  mdContainer.dataset.rawMarkdown = version.content;

  // 更新来源
  const blockEl = carousel.closest('.msg-block');
  if (blockEl) {
    const sourcesEl = blockEl.querySelector('.sources');
    if (sourcesEl && version.sources) {
      // 触发 sources 重新渲染（由 chat.js renderSources 处理）
      // 这里仅设置 dataset，chat.js 的 renderSources 会读取
      blockEl.dataset.pendingSources = JSON.stringify(version.sources);
    }
  }
}

// ============================================================
// 控制栏渲染
// ============================================================

/**
 * 渲染轮播控制栏（◀ 1/N ▶）。
 *
 * 控制栏插入在轮播容器顶部。
 * 版本数为 0 时不渲染。
 *
 * @param {HTMLElement} carousel - 轮播容器
 * @returns {HTMLElement|null} 创建的控制栏元素
 */
export function renderCarouselControls(carousel) {
  const total = carousel._versions ? carousel._versions.length : 0;
  if (total === 0) return null;

  // 清除已有控制栏
  const existing = carousel.querySelector('.regen-carousel-controls');
  if (existing) existing.remove();

  const controls = document.createElement('div');
  controls.className = 'regen-carousel-controls';

  // 左箭头
  const prevBtn = document.createElement('button');
  prevBtn.className = 'regen-carousel-btn regen-carousel-prev';
  prevBtn.innerHTML = '◀';
  prevBtn.setAttribute('aria-label', t('chat.regen_prev') || '上一个版本');
  prevBtn.onclick = () => navigateCarousel(carousel, 'left');

  // 计数器
  const current = parseInt(carousel.dataset.currentIndex || '0', 10);
  const counter = document.createElement('span');
  counter.className = 'regen-carousel-counter';
  counter.textContent = `${current + 1}/${total}`;

  // 右箭头
  const nextBtn = document.createElement('button');
  nextBtn.className = 'regen-carousel-btn regen-carousel-next';
  nextBtn.innerHTML = '▶';
  nextBtn.setAttribute('aria-label', t('chat.regen_next') || '下一个版本');
  nextBtn.onclick = () => navigateCarousel(carousel, 'right');

  controls.appendChild(prevBtn);
  controls.appendChild(counter);
  controls.appendChild(nextBtn);

  // 插入到轮播容器最前面
  carousel.insertBefore(controls, carousel.firstChild);

  return controls;
}

/**
 * 更新控制栏的显示状态（计数器和箭头禁用状态）。
 *
 * @param {HTMLElement} carousel - 轮播容器
 */
function updateControlsDisplay(carousel) {
  const counter = carousel.querySelector('.regen-carousel-counter');
  const state = getCarouselState(carousel);
  if (counter) {
    counter.textContent = `${state.current + 1}/${state.total}`;
  }
}

/**
 * 检查消息块是否已有轮播容器。
 *
 * @param {HTMLElement} blockEl - 消息块根元素
 * @returns {HTMLElement|null} 轮播容器（不存在时返回 null）
 */
export function getExistingCarousel(blockEl) {
  // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
  return blockEl.querySelector('.regen-carousel') || null;
}
