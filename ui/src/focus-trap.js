/**
 * EchoMind Focus Trap 工具（REQ-A11Y-002）。
 *
 * 实现 WAI-ARIA Authoring Practices — Dialog Modal Pattern：
 * https://www.w3.org/WAI/ARIA/apg/patterns/dialogmodal/
 *
 * 功能：
 * 1. 激活时记录当前焦点元素（previouslyFocused），deactivate 时恢复焦点
 * 2. 激活时自动聚焦容器内首个可聚焦元素
 * 3. Tab 键在容器内首尾元素间循环（不跳出容器）
 * 4. Shift+Tab 反向循环
 *
 * 用法：
 *   const trap = createFocusTrap(modalEl);
 *   trap.activate();   // 模态框打开时
 *   trap.deactivate(); // 模态框关闭时
 */

/** 可聚焦元素的 CSS 选择器（WAI-ARIA Dialog Pattern） */
const FOCUSABLE_SELECTORS = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'textarea:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(', ');

/**
 * 判断元素是否可见（非 display:none / visibility:hidden / 零尺寸）。
 * @param {HTMLElement} el
 * @returns {boolean}
 */
function isVisible(el) {
  if (!el) return false;
  if (el.offsetWidth === 0 && el.offsetHeight === 0) return false;
  const style = window.getComputedStyle(el);
  if (style.display === 'none') return false;
  if (style.visibility === 'hidden') return false;
  return true;
}

/**
 * 获取容器内所有可见且可聚焦的元素（按 DOM 顺序）。
 * @param {HTMLElement} container
 * @returns {Element[]}
 */
function getFocusableElements(container) {
  if (!container) return [];
  return Array.from(
    container.querySelectorAll(FOCUSABLE_SELECTORS)
  ).filter(isVisible);
}

/**
 * 创建 Focus Trap 实例。
 *
 * @param {HTMLElement} element - 需要锁定焦点的容器元素
 * @returns {{ activate: () => void, deactivate: () => void }}
 *   - activate(): 激活焦点陷阱，记录当前焦点并聚焦容器内首个可聚焦元素
 *   - deactivate(): 停用焦点陷阱，恢复焦点到激活前的元素
 */
export function createFocusTrap(element) {
  /** 激活前的焦点元素（用于关闭后恢复） */
  let previouslyFocused = null;

  /** 是否已激活 */
  let active = false;

  /** keydown 事件处理器引用（用于 removeEventListener） */
  let keydownHandler = null;

  /**
   * keydown 事件处理器：拦截 Tab / Shift+Tab，在容器内循环。
   * @param {KeyboardEvent} e
   */
  function handleKeyDown(e) {
    if (!active) return;
    if (e.key !== 'Tab') return;

    const focusable = getFocusableElements(element);
    if (focusable.length === 0) {
      // 容器内无可聚焦元素：阻止 Tab 跳出
      e.preventDefault();
      return;
    }

    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const current = document.activeElement;

    if (e.shiftKey) {
      // Shift+Tab：当前焦点在首个元素 → 跳到末尾
      if (current === first || current === element || !element.contains(current)) {
        e.preventDefault();
        last.focus();
      }
    } else {
      // Tab：当前焦点在末尾元素 → 跳到首个
      if (current === last || current === element || !element.contains(current)) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  /**
   * 激活 Focus Trap。
   * 1. 记录当前焦点元素
   * 2. 注册 keydown 监听器
   * 3. 聚焦容器内首个可聚焦元素
   */
  function activate() {
    if (active) return;
    active = true;

    // 记录激活前的焦点元素
    previouslyFocused = document.activeElement;

    // 注册 keydown 监听器（capture 阶段，确保在其他 handler 之前拦截 Tab）
    keydownHandler = handleKeyDown;
    document.addEventListener('keydown', keydownHandler, true);

    // 聚焦容器内首个可聚焦元素
    const focusable = getFocusableElements(element);
    if (focusable.length > 0) {
      // 同步聚焦（确保在 deactivate 记录 previouslyFocused 之前完成）
      focusable[0].focus();
    } else {
      // 容器内无可聚焦元素：使容器自身可聚焦
      if (!element.hasAttribute('tabindex')) {
        element.setAttribute('tabindex', '-1');
      }
      element.focus();
    }
  }

  /**
   * 停用 Focus Trap。
   * 1. 移除 keydown 监听器
   * 2. 恢复焦点到激活前的元素
   */
  function deactivate() {
    if (!active) return;
    active = false;

    // 移除 keydown 监听器
    if (keydownHandler) {
      document.removeEventListener('keydown', keydownHandler, true);
      keydownHandler = null;
    }

    // 恢复焦点（同步恢复，确保在模态框隐藏前完成）
    if (previouslyFocused && typeof previouslyFocused.focus === 'function') {
      previouslyFocused.focus();
    }
    previouslyFocused = null;
  }

  return { activate, deactivate };
}
