/**
 * EchoMind 全局面板栈管理器 — 统一 ESC 键关闭 + 面板生命周期追踪。
 *
 * 设计原理：
 * 1. 栈结构 — 最后打开的面板在栈顶，ESC 优先关闭栈顶面板
 * 2. 单次关闭 — ESC 只关闭一个面板（栈顶），不级联关闭
 * 3. 自动清理 — 面板关闭时从栈中移除，防止内存泄漏
 * 4. 幂等 — 同一面板重复 push 不会创建多个条目
 *
 * 与 keyboard.js 的协作：
 *   keyboard.js ESC 处理器首先调用 closeTopPanel()：
 *   - 如果栈非空 → 关闭栈顶面板 → 返回 true → keyboard.js 不再处理
 *   - 如果栈为空 → 返回 false → keyboard.js 回退到静态面板检查
 *
 * 各面板模块的职责：
 *   打开时：pushPanel({ id, close, element })
 *   关闭时：removePanel(id)  — 在 close 函数内部调用
 *
 * @example
 * // 面板打开
 * import { pushPanel, removePanel } from './panel-stack.js';
 * function openMyPanel() {
 *   pushPanel({ id: 'my-panel', close: closeMyPanel, element: overlayEl });
 *   overlayEl.classList.remove('hidden');
 * }
 * function closeMyPanel() {
 *   removePanel('my-panel'); // 先从栈中移除，防止 ESC 重复触发
 *   overlayEl.classList.add('hidden');
 * }
 */

// ============================================================
// 类型定义（JSDoc）
// ============================================================

/**
 * @typedef {Object} PanelEntry
 * @property {string} id - 面板唯一标识
 * @property {() => void} close - 关闭函数（调用 removePanel + 隐藏 DOM）
 * @property {HTMLElement} [element] - 面板根 DOM 元素（用于调试/焦点恢复）
 * @property {string} [label] - 人类可读名称（调试用）
 */

// ============================================================
// 模块状态
// ============================================================

/** 面板栈：栈顶是最后打开的面板 */
const _stack = [];

// ============================================================
// 公共 API
// ============================================================

/**
 * 将面板推入栈。
 *
 * 如果同一 id 已在栈中，先移除旧条目（幂等），
 * 再将新条目推入栈顶。
 *
 * @param {PanelEntry} panel - 面板入口
 * @returns {void}
 */
export function pushPanel(panel) {
  // 幂等：移除已有同 id 条目
  const existingIdx = _stack.findIndex((p) => p.id === panel.id);
  if (existingIdx >= 0) {
    _stack.splice(existingIdx, 1);
  }
  _stack.push(panel);
}

/**
 * 从栈中移除指定面板（不调用 close 函数）。
 *
 * 面板的 close 函数应先调用 removePanel 再隐藏 DOM，
 * 防止 ESC 再次触发 close 造成重复执行。
 *
 * @param {string} id - 面板唯一标识
 * @returns {void}
 */
export function removePanel(id) {
  const idx = _stack.findIndex((p) => p.id === id);
  if (idx >= 0) {
    _stack.splice(idx, 1);
  }
}

/**
 * 关闭栈顶面板（调用其 close 函数）。
 *
 * close 函数内部会调用 removePanel 从栈中移除自身，
 * 因此此函数不需要额外操作栈。
 *
 * @returns {boolean} true=有面板被关闭；false=栈为空
 */
export function closeTopPanel() {
  const top = _stack[_stack.length - 1];
  if (!top) return false;
  // close 函数内部应先 removePanel 再隐藏 DOM
  top.close();
  return true;
}

/**
 * 查询栈顶面板（不移除）。
 * @returns {PanelEntry | null}
 */
export function peekTopPanel() {
  return _stack[_stack.length - 1] || null;
}

/**
 * 查询指定面板是否在栈中。
 * @param {string} id - 面板唯一标识
 * @returns {boolean}
 */
export function isPanelOpen(id) {
  return _stack.some((p) => p.id === id);
}

/**
 * 查询栈是否非空（是否有面板打开）。
 * @returns {boolean}
 */
export function hasOpenPanels() {
  return _stack.length > 0;
}

/**
 * 获取当前栈深度（打开的面板数量）。
 * @returns {number}
 */
export function getStackDepth() {
  return _stack.length;
}

/**
 * 获取所有打开面板的 id 列表（从底到顶）。
 * @returns {string[]}
 */
export function listOpenPanels() {
  return _stack.map((p) => p.id);
}

/**
 * 清空栈（不调用 close 函数）。
 * 用于会话切换时强制清理所有面板引用。
 * 各面板的 DOM 仍需各自的 close 函数手动处理。
 */
export function clearStack() {
  _stack.length = 0;
}
