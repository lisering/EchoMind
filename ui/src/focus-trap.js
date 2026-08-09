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
 * 5. 全局 Trap 栈管理：只激活栈顶 Trap，解决多面板叠加时 Tab 行为冲突
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
 * 全局 Focus Trap 栈（后进先出）。
 * 只激活栈顶的 Trap，确保多面板叠加时 Tab 行为正确。
 */
const _trapStack = [];

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
 * 获取栈顶的活跃 Trap（跳过 null 空位）。
 * @returns {Object|null}
 */
function _getTopTrap() {
  for (let i = _trapStack.length - 1; i >= 0; i--) {
    const trap = _trapStack[i];
    if (trap !== null) {
      return trap;
    }
  }
  return null;
}

/**
 * 清理栈尾部的空位（从末尾向前清理连续的 null）。
 */
function _cleanupStack() {
  while (_trapStack.length > 0 && _trapStack[_trapStack.length - 1] === null) {
    _trapStack.pop();
  }
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

  /** 是否已暂停（栈中但非栈顶时暂停） */
  let paused = false;

  /** keydown 事件处理器引用（用于 removeEventListener） */
  let keydownHandler = null;

  /** 此 Trap 在栈中的索引（-1 表示不在栈中） */
  let stackIndex = -1;

  /**
   * keydown 事件处理器：拦截 Tab / Shift+Tab，在容器内循环。
   * @param {KeyboardEvent} e
   */
  function handleKeyDown(e) {
    if (!active || paused) return;
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
   * 暂停 Focus Trap（保留 previouslyFocused，栈中但非栈顶时调用）。
   */
  function _pause() {
    if (!active || paused) return;
    paused = true;

    // 移除 keydown 监听器（暂停 Tab 拦截）
    if (keydownHandler) {
      document.removeEventListener('keydown', keydownHandler, true);
      keydownHandler = null;
    }
  }

  /**
   * 恢复 Focus Trap（从暂停状态恢复到活跃状态）。
   */
  function _resume() {
    if (!active || !paused) return;
    paused = false;

    // 重新注册 keydown 监听器
    keydownHandler = handleKeyDown;
    document.addEventListener('keydown', keydownHandler, true);

    // 重新聚焦到容器内（确保焦点回到正确的面板）
    const focusable = getFocusableElements(element);
    if (focusable.length > 0) {
      focusable[0].focus();
    } else {
      if (!element.hasAttribute('tabindex')) {
        element.setAttribute('tabindex', '-1');
      }
      element.focus();
    }
  }

  /**
   * 激活 Focus Trap。
   * 1. 暂停栈中所有已激活的 Trap
   * 2. 压入新 Trap 到栈顶
   * 3. 记录当前焦点元素
   * 4. 注册 keydown 监听器
   * 5. 聚焦容器内首个可聚焦元素
   */
  function activate() {
    if (active && !paused) return;
    
    if (!active) {
      active = true;
      
      // 暂停栈中所有已激活的 Trap（除自己外的所有Trap）
      for (let i = 0; i < _trapStack.length; i++) {
        const otherTrap = _trapStack[i];
        if (otherTrap !== null && otherTrap.element !== element) {
          otherTrap._pause();
        }
      }
      
      // 压入新 Trap 到栈顶
      _trapStack.push(null); // 预分配位置
      const trap = {
        element,
        activate,
        deactivate,
        _pause,
        _resume,
        get active() { return active && !paused; },
        get stackIndex() { return stackIndex; }
      };
      _trapStack[_trapStack.length - 1] = trap;
      stackIndex = _trapStack.length - 1;
      
      // 记录激活前的焦点元素
      previouslyFocused = document.activeElement;
    }

    // 恢复此 Trap
    _resume();
  }

  /**
   * 停用 Focus Trap。
   * 1. 从栈中移除此 Trap
   * 2. 移除 keydown 监听器
   * 3. 恢复焦点到激活前的元素
   * 4. 恢复栈顶 Trap（如果存在）
   */
  function deactivate() {
    if (!active) return;
    
    // 从栈中移除此 Trap
    if (stackIndex >= 0 && stackIndex < _trapStack.length) {
      _trapStack[stackIndex] = null; // 保留空位以保持索引稳定
    }
    
    active = false;
    paused = false;

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
    
    // 恢复栈顶 Trap（如果存在且不是自己）
    const topTrap = _getTopTrap();
    if (topTrap && topTrap.element !== element && !topTrap.active) {
      topTrap._resume();
    }
    
    // 清理栈中的空位（可选，避免无限增长）
    _cleanupStack();
  }

  return { activate, deactivate };
}