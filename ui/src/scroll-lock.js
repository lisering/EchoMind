/**
 * EchoMind 智能滚动锁定模块 — scroll-lock.js
 *
 * 职责：
 * 1. 监听 chatArea 滚动事件，检测用户是否滚离底部
 * 2. 用户在底部时：新 token 到达自动滚动到底部
 * 3. 用户滚向上方时：锁定滚动位置，不自动拽回
 * 4. 显示/隐藏「回到底部」浮动按钮
 *
 * 设计参考：
 * - Perplexity: 100px 阈值 + "Jump to latest" 浮动按钮
 * - Claude.ai: 用户滚动后停止 auto-scroll
 *
 * 阈值规则：距底部 ≤ 100px 时视为"在底部"，否则视为"已滚离"。
 *
 * 对应 AC: TC-QA-002 (设计提案 §4.4.2)
 */

// ============================================================
// 模块级状态
// ============================================================

/** 用户是否已滚动离开底部区域。 */
let _userScrolledUp = false;

/** 已注册的滚动事件清理函数。 */
let _scrollCleanup = null;

/** 滚动锁定阈值（距底部多少像素内视为"在底部"）。 */
const SCROLL_THRESHOLD = 100;

/**
 * 编程式滚动保护标志。
 *
 * 当 scrollToBottom() 触发平滑滚动时设置为 true，
 * onScroll 监听器在此期间不会将 _userScrolledUp 设为 true。
 * 防止平滑滚动动画期间内容增长导致 scroll-lock 误判。
 * 500ms 后自动清除（覆盖 0.4s 的 messageIn 动画 + 平滑滚动时长）。
 */
let _programmaticScroll = false;

/** 编程式滚动保护的超时计时器引用。 */
let _programmaticScrollTimer = null;

// ============================================================
// 公共 API
// ============================================================

/**
 * 初始化智能滚动锁定监听器。
 *
 * 注册 `scroll` 事件监听器到 chatArea，实时检测用户滚动位置。
 * 当用户距底部超过 `SCROLL_THRESHOLD` 像素时标记为"已滚离"，
 * 此后新 token 到达时不会强制滚动。
 *
 * @param {HTMLElement} chatArea - 聊天区域 DOM 元素（#chatArea）
 * @returns {() => void} 清理函数，调用后移除事件监听器并重置状态
 *
 * @example
 * const cleanup = initScrollLock($('chatArea'));
 * // 不再需要时
 * cleanup();
 */
export function initScrollLock(chatArea) {
  if (!chatArea) return () => {};

  // 清理之前的监听器
  if (_scrollCleanup) _scrollCleanup();

  const onScroll = () => {
    const distFromBottom = chatArea.scrollHeight - chatArea.scrollTop - chatArea.clientHeight;
    // 编程式滚动保护：平滑滚动动画期间内容可能增长（如追加 assistant 消息块），
    // 导致 distFromBottom 暂时超过阈值。此时不应将 _userScrolledUp 设为 true，
    // 否则后续流式 token 的 shouldAutoScroll() 会返回 false，阻止自动滚动。
    if (_programmaticScroll && distFromBottom > SCROLL_THRESHOLD) return;
    _userScrolledUp = distFromBottom > SCROLL_THRESHOLD;
  };

  chatArea.addEventListener('scroll', onScroll, { passive: true });

  _scrollCleanup = () => {
    chatArea.removeEventListener('scroll', onScroll);
    _userScrolledUp = false;
  };

  return _scrollCleanup;
}

/**
 * 用户是否已滚动离开底部区域。
 *
 * @returns {boolean} true=已滚离底部；false=在底部附近
 */
export function isUserScrolledUp() {
  return _userScrolledUp;
}

/**
 * 是否应该自动滚动到底部。
 *
 * 仅当用户未滚离底部时返回 true。
 *
 * @returns {boolean} true=应该自动滚动；false=应锁定位置
 */
export function shouldAutoScroll() {
  return !_userScrolledUp;
}

/**
 * 重置滚动锁定状态（标记用户回到底部）。
 *
 * 调用后 shouldAutoScroll() 将返回 true，
 * 直到用户再次滚离底部。
 */
/**
 * 标记开始编程式滚动。
 *
 * 在调用 scrollTo({ behavior: 'smooth' }) 之前调用此函数。
 * 设置 _programmaticScroll 标志，使 onScroll 监听器在平滑滚动期间
 * 不会因内容增长而误设 _userScrolledUp = true。
 * 500ms 后自动清除标志（覆盖平滑滚动 + CSS 动画时长）。
 */
export function beginProgrammaticScroll() {
  _programmaticScroll = true;
  if (_programmaticScrollTimer) clearTimeout(_programmaticScrollTimer);
  _programmaticScrollTimer = setTimeout(() => {
    _programmaticScroll = false;
    _programmaticScrollTimer = null;
  }, 500);
}

export function resetScrollLock() {
  _userScrolledUp = false;
}

/**
 * 销毁滚动锁定监听器，清理所有状态。
 *
 * 移除事件监听器并重置内部状态。
 * 通常在会话切换或页面卸载时调用。
 */
export function destroyScrollLock() {
  if (_scrollCleanup) {
    _scrollCleanup();
    _scrollCleanup = null;
  }
  _userScrolledUp = false;
}

/**
 * 创建「回到底部」浮动按钮 DOM 元素。
 *
 * 按钮初始隐藏（display:none），调用方在需要时设置 display。
 * 点击按钮时：平滑滚动 chatArea 到底部 + 重置锁定状态。
 *
 * @returns {HTMLButtonElement} 浮动按钮元素（已隐藏）
 *
 * @example
 * const btn = createJumpToLatestButton();
 * chatAreaWrapper.appendChild(btn);
 * // 用户滚离底部时显示
 * btn.style.display = 'flex';
 */
export function createJumpToLatestButton() {
  const btn = document.createElement('button');
  btn.className = 'jump-to-latest';
  btn.style.display = 'none';
  btn.setAttribute('aria-label', '回到底部');
  btn.innerHTML = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg><span>回到底部</span>`;

  btn.onclick = () => {
    const chatArea = document.getElementById('chatArea');
    if (chatArea) {
      chatArea.scrollTo({ top: chatArea.scrollHeight, behavior: 'smooth' });
    }
    resetScrollLock();
    btn.style.display = 'none';
  };

  return btn;
}
