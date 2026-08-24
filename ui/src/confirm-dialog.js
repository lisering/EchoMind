/**
 * EchoMind 统一确认对话框组件（REQ-IX-005）。
 *
 * 替代所有原生 `confirm()` 调用和零散自定义确认弹框，提供一致的
 * 视觉风格、防误触（500ms 延迟）、键盘交互（Esc / Enter）和
 * Promise 接口，方便 async/await 调用。
 *
 * 设计原则：
 * 1. 纯 DOM — 不依赖外部框架，兼容 jsdom 测试环境
 * 2. Promise 接口 — `showConfirmDialog({...})` → `Promise<boolean>`
 * 3. 防误触 — 确认按钮在弹窗出现后 500ms 内 disabled
 * 4. 键盘友好 — Esc=取消, Enter=确认（仅 500ms 后生效）
 * 5. 单例模式 — 同时只存在一个确认对话框
 */

import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { Z_INDEX, zClass } from './panel-stack.js';
import { pushPanel, removePanel } from './panel-stack.js';

/** @type {HTMLElement|null} 当前活动对话框的 DOM 根节点 */
let _activeDialog = null;

/** @type {((result: boolean) => void)|null} 当前活动对话框的 resolve 回调 */
let _activeResolve = null;

/** @type {number|null} 防误触定时器 ID */
let _enableTimer = null;

/** @type {{ activate: () => void, deactivate: () => void }|null} 当前活动对话框的 Focus Trap 实例 */
let _activeTrap = null;

/** 防误触延迟（毫秒） */
const ANTI_MISTOUCH_DELAY = 500;

/**
 * 关闭当前活动对话框并 resolve 对应 Promise。
 * @param {boolean} result - 用户选择结果（true=确认, false=取消）
 */
function _closeDialog(result) {
  if (_enableTimer !== null) {
    clearTimeout(_enableTimer);
    _enableTimer = null;
  }
  // 从面板栈移除
  removePanel('confirm-dialog');
  // 停用 Focus Trap（恢复焦点到触发元素）
  if (_activeTrap) {
    _activeTrap.deactivate();
    _activeTrap = null;
  }
  if (_activeDialog) {
    document.removeEventListener('keydown', _onKeyDown);
    _activeDialog.remove();
    _activeDialog = null;
  }
  if (_activeResolve) {
    const resolve = _activeResolve;
    _activeResolve = null;
    resolve(result);
  }
}

/**
 * 全局键盘事件处理器：Esc=取消, Enter=确认。
 * @param {KeyboardEvent} e
 */
function _onKeyDown(e) {
  if (!_activeDialog) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    _closeDialog(false);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    e.stopPropagation();
    const confirmBtn = _activeDialog.querySelector('[data-role="confirm"]');
    if (confirmBtn && !confirmBtn.disabled) {
      _closeDialog(true);
    }
    // 如果确认按钮仍 disabled（500ms 内），不做任何操作（防误触）
  }
}

/**
 * 显示统一确认对话框（REQ-IX-005）。
 *
 * @param {Object} opts - 对话框选项
 * @param {string} [opts.title] - 对话框标题
 * @param {string} [opts.body] - 对话框正文（可含 HTML）
 * @param {string} [opts.confirmText] - 确认按钮文案（默认 "确认"）
 * @param {string} [opts.cancelText] - 取消按钮文案（默认 "取消"）
 * @param {boolean} [opts.danger=true] - 是否为危险操作（确认按钮红色变体）
 * @param {string|HTMLElement} [opts.customContent] - 自定义内容（下拉选择/输入框，REQ-ING-024）
 * @returns {Promise<boolean>} 用户选择结果（true=确认, false=取消）
 *
 * @example
 * const ok = await showConfirmDialog({
 *   title: '删除文档？',
 *   body: '文档及其全部索引数据将被删除。此操作不可撤销。',
 *   confirmText: '删除',
 *   danger: true,
 * });
 * if (!ok) return;
 * // 执行删除...
 */
export function showConfirmDialog(opts = {}) {
  const { title = '', body = '', confirmText, cancelText, danger = true, customContent = null } = opts;

  // 如果已有对话框打开，先关闭旧的（视为取消）
  if (_activeDialog) {
    _closeDialog(false);
  }

  return new Promise((resolve) => {
    _activeResolve = resolve;

    // 构建 DOM
    const overlay = document.createElement('div');
    overlay.id = 'confirmDialog';
    overlay.className = `fixed inset-0 ${zClass(Z_INDEX.PANEL_2)} bg-surface-0/95 backdrop-blur-sm flex items-center justify-center`;
    overlay.setAttribute('role', 'alertdialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.setAttribute('aria-labelledby', 'confirmDialogTitle');
    overlay.setAttribute('aria-describedby', 'confirmDialogBody');

    // 点击遮罩层 = 取消（点击容器内不触发）
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) {
        _closeDialog(false);
      }
    });

    // 确认按钮文案
    const confirmLabel = confirmText || t('common.confirm') || '确认';
    const cancelLabel = cancelText || t('common.cancel') || '取消';

    // 确认按钮样式（danger 变体）
    const confirmBtnClass = danger
      ? 'flex-1 bg-red-500/15 text-red-300 border border-red-400/40 rounded-xl px-4 py-3 text-sm hover:bg-red-500/25 transition-colors disabled:opacity-40'
      : 'flex-1 bg-accent/15 text-accent border border-accent/30 rounded-xl px-4 py-3 text-sm hover:bg-accent/25 transition-colors disabled:opacity-40';

    // 内部容器
    const container = document.createElement('div');
    container.className = 'w-full max-w-sm mx-4 bg-surface-1 border border-border-strong rounded-lg p-6';

    // 图标（仅 danger 模式显示）
    if (danger) {
      const icon = document.createElement('div');
      icon.className = 'text-3xl mb-2 text-amber-400';
      icon.innerHTML = '<svg class="icon-md" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>';
      container.appendChild(icon);
    }

    // 标题
    const titleEl = document.createElement('p');
    titleEl.id = 'confirmDialogTitle';
    titleEl.className = 'text-lg font-semibold text-text-primary mb-2';
    titleEl.textContent = title;
    container.appendChild(titleEl);

    // 正文
    if (body) {
      const bodyEl = document.createElement('p');
      bodyEl.id = 'confirmDialogBody';
      bodyEl.className = 'text-sm text-text-secondary leading-relaxed mb-4';
      bodyEl.innerHTML = body;
      container.appendChild(bodyEl);
    } else {
      // 无正文时添加间距占位
      const spacer = document.createElement('div');
      spacer.className = 'mb-4';
      container.appendChild(spacer);
    }

    // 自定义内容（REQ-ING-024：批量移动下拉选择、批量标签输入框）
    if (customContent) {
      const wrapper = document.createElement('div');
      wrapper.className = 'mb-4';
      if (typeof customContent === 'string') {
        wrapper.innerHTML = customContent;
      } else if (customContent instanceof HTMLElement) {
        wrapper.appendChild(customContent);
      }
      container.appendChild(wrapper);
    }

    // 按钮行
    const btnRow = document.createElement('div');
    btnRow.className = 'flex gap-3';

    // 取消按钮（左）
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'flex-1 border border-border-default rounded-xl px-4 py-3 text-sm text-text-secondary hover:bg-white/5 transition-colors';
    cancelBtn.textContent = cancelLabel;
    cancelBtn.setAttribute('data-role', 'cancel');
    cancelBtn.addEventListener('click', () => _closeDialog(false));

    // 确认按钮（右）
    const confirmBtn = document.createElement('button');
    confirmBtn.className = confirmBtnClass;
    confirmBtn.textContent = confirmLabel;
    confirmBtn.setAttribute('data-role', 'confirm');
    confirmBtn.disabled = true; // 初始 disabled（500ms 防误触）
    confirmBtn.addEventListener('click', () => {
      if (!confirmBtn.disabled) {
        _closeDialog(true);
      }
    });

    btnRow.appendChild(cancelBtn);
    btnRow.appendChild(confirmBtn);
    container.appendChild(btnRow);
    overlay.appendChild(container);

    // 挂载到 body
    document.body.appendChild(overlay);
    _activeDialog = overlay;

    // 注册全局键盘事件
    document.addEventListener('keydown', _onKeyDown, true);

    // 激活 Focus Trap（REQ-A11Y-002）：Tab 键锁定在对话框内
    _activeTrap = createFocusTrap(overlay);
    _activeTrap.activate();

    // 注册到面板栈（ESC 关闭 + 生命周期追踪）
    pushPanel({ id: 'confirm-dialog', close: () => _closeDialog(false), element: overlay, label: 'Confirm Dialog' });

    // 500ms 后启用确认按钮
    _enableTimer = setTimeout(() => {
      if (confirmBtn) {
        confirmBtn.disabled = false;
        // 自动聚焦到确认按钮（方便 Enter 确认）
        confirmBtn.focus();
      }
      _enableTimer = null;
    }, ANTI_MISTOUCH_DELAY);
  });
}

// 暴露为全局函数（E2E 测试 + 内联 onclick 需要）
window.showConfirmDialog = showConfirmDialog;
