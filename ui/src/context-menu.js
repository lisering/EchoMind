/**
 * EchoMind 自定义右键上下文菜单模块（REQ-IX-003 / REQ-UI-012）。
 *
 * 取代 Tauri WebView 的浏览器默认右键菜单，提供与桌面应用一致的
 * 剪切/复制/粘贴/全选体验。仅在有可编辑文本或选中文本时显示相关项。
 *
 * 技术要点：
 * - contextmenu 事件 preventDefault 抑制浏览器默认菜单
 * - document.execCommand 执行 clipboard 操作（Tauri WebView 仍支持）
 * - 菜单定位考虑视口边界，防溢出
 * - 点击外部 / Escape / 滚动 / 失焦 → 自动关闭
 */

import { $, copyToClipboard } from './utils.js';
import { t } from './i18n.js';
import { toast, toastError } from './toast.js';

/** 当前菜单是否可见 */
let _visible = false;

/**
 * 初始化全局右键菜单：注册 contextmenu 事件 + 关闭监听。
 * 在 main.js boot() 中调用。
 */
export function initContextMenu() {
  const menu = $('ctxMenu');
  if (!menu) return;

  // 全局 contextmenu：抑制浏览器默认菜单，显示自定义菜单
  document.addEventListener('contextmenu', (e) => {
    const target = e.target;

    // REQ-IX-003 AC-4：文档列表项右键 → 显示「复制文件名」菜单
    const docItem = target.closest('[data-doc-name]');
    if (docItem) {
      e.preventDefault();
      _showDocMenu(e.clientX, e.clientY, docItem.dataset.docName || '');
      return;
    }

    // REQ-IX-003：引用芯片右键 → 显示「复制引用片段」菜单
    const sourceChip = target.closest('.source-chip');
    if (sourceChip && sourceChip.dataset.chunkContent) {
      e.preventDefault();
      _showSourceMenu(e.clientX, e.clientY, sourceChip.dataset.chunkContent);
      return;
    }

    // 输入框/文本域/可编辑区 → 显示编辑菜单
    // @ts-expect-error EventTarget extended with Element properties via dom-ext.d.ts
    const isEditable = _isEditable(target);
    const hasSelection = _hasSelection();

    // 非编辑且无选中 → 不显示菜单（也抑制默认）
    if (!isEditable && !hasSelection) {
      e.preventDefault();
      return;
    }

    e.preventDefault();
    _showMenu(e.clientX, e.clientY, { isEditable, hasSelection });
  });

  // 点击外部关闭
  menu.addEventListener('click', (e) => {
    e.stopPropagation();
    const item = e.target.closest('.ctx-item');
    if (item && !item.classList.contains('disabled')) {
      const action = item.dataset.action;
      _executeAction(action);
      _hideMenu();
    }
  });
  // 全局点击 / Escape / 滚动 → 关闭
  document.addEventListener('click', () => _hideMenu(), true);
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && _visible) {
      e.stopPropagation();
      _hideMenu();
    }
  });
  window.addEventListener('blur', () => _hideMenu());
  document.addEventListener('scroll', () => _hideMenu(), true);
}

/**
 * 显示文档列表项右键菜单（REQ-IX-003 AC-4）。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {string} docName - 文档名称
 */
function _showDocMenu(x, y, docName) {
  const menu = $('ctxMenu');
  if (!menu) return;

  const items = [];
  items.push(_item('copyDocName', t('ctx.copy_doc_name'), '', true));
  menu.innerHTML = items.join('');

  // 存储文档名供点击时使用
  menu.dataset.docName = docName;

  // 先显示再测量尺寸
  menu.classList.add('visible');
  const mw = menu.offsetWidth;
  const mh = menu.offsetHeight;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  if (x + mw > vw - 4) x = vw - mw - 4;
  if (y + mh > vh - 4) y = vh - mh - 4;
  menu.style.left = Math.max(4, x) + 'px';
  menu.style.top = Math.max(4, y) + 'px';
  _visible = true;
}

/**
 * 显示引用芯片右键菜单（REQ-IX-003）。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {string} chunkContent - chunk 原文
 */
function _showSourceMenu(x, y, chunkContent) {
  const menu = $('ctxMenu');
  if (!menu) return;

  const items = [];
  items.push(_item('copyChunk', t('ctx.copy_chunk'), '', true));
  menu.innerHTML = items.join('');

  // 存储 chunk 内容供点击时使用
  menu.dataset.chunkContent = chunkContent;

  menu.classList.add('visible');
  const mw = menu.offsetWidth;
  const mh = menu.offsetHeight;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  if (x + mw > vw - 4) x = vw - mw - 4;
  if (y + mh > vh - 4) y = vh - mh - 4;
  menu.style.left = Math.max(4, x) + 'px';
  menu.style.top = Math.max(4, y) + 'px';
  _visible = true;
}

/**
 * 显示右键菜单。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {{isEditable:boolean, hasSelection:boolean}} ctx - 上下文状态
 */
function _showMenu(x, y, ctx) {
  const menu = $('ctxMenu');
  if (!menu) return;

  // 根据上下文渲染菜单项
  menu.innerHTML = _buildItems(ctx);

  // 先显示再测量尺寸（否则 offsetWidth=0）
  menu.classList.add('visible');
  const mw = menu.offsetWidth;
  const mh = menu.offsetHeight;
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  // 边界溢出修正
  if (x + mw > vw - 4) x = vw - mw - 4;
  if (y + mh > vh - 4) y = vh - mh - 4;
  menu.style.left = Math.max(4, x) + 'px';
  menu.style.top = Math.max(4, y) + 'px';

  _visible = true;
}

/** 隐藏右键菜单。 */
function _hideMenu() {
  if (!_visible) return;
  const menu = $('ctxMenu');
  if (menu) menu.classList.remove('visible');
  _visible = false;
}

/**
 * 构建菜单项 HTML。
 * @param {{isEditable:boolean, hasSelection:boolean}} ctx
 * @returns {string} 菜单项 HTML
 */
function _buildItems(ctx) {
  const mod = '⌘'; // macOS Cmd（Tauri 桌面端为主）
  const items = [];

  // 剪切：仅可编辑且有选中
  items.push(_item('cut', t('ctx.cut'), mod + 'X', ctx.isEditable && ctx.hasSelection));
  // 复制：有选中即可
  items.push(_item('copy', t('ctx.copy'), mod + 'C', ctx.hasSelection));
  // 粘贴：仅可编辑
  items.push(_item('paste', t('ctx.paste'), mod + 'V', ctx.isEditable));
  items.push(_separator());
  // 全选：可编辑或有内容
  items.push(_item('selectAll', t('ctx.selectAll'), mod + 'A', ctx.isEditable || true));

  return items.join('');
}

/**
 * 构建单个菜单项 HTML。
 * @param {string} action - 动作标识
 * @param {string} label - 显示文案
 * @param {string} shortcut - 快捷键提示
 * @param {boolean} enabled - 是否可用
 * @returns {string} HTML
 */
function _item(action, label, shortcut, enabled) {
  const cls = enabled ? 'ctx-item' : 'ctx-item disabled';
  return `<div class="${cls}" data-action="${action}"><span>${label}</span><span class="ctx-shortcut">${shortcut}</span></div>`;
}

/** 构建分隔线 HTML。 */
function _separator() {
  return '<div class="ctx-separator"></div>';
}

/**
 * 执行菜单动作。
 * @param {string} action - cut/copy/paste/selectAll/copyDocName/copyChunk
 */
function _executeAction(action) {
  const menu = $('ctxMenu');

  // REQ-IX-003 AC-4：复制文件名
  if (action === 'copyDocName') {
    const docName = menu?.dataset.docName || '';
    copyToClipboard(docName).then((ok) => {
      if (ok) {
        toast(t('chat.copied_to_clipboard'), 'success');
      } else {
        toastError(t('chat.copy_failed'));
      }
    });
    return;
  }

  // REQ-IX-003：复制引用片段
  if (action === 'copyChunk') {
    const chunkContent = menu?.dataset.chunkContent || '';
    copyToClipboard(chunkContent).then((ok) => {
      if (ok) {
        toast(t('chat.copied_to_clipboard'), 'success');
      } else {
        toastError(t('chat.copy_failed'));
      }
    });
    return;
  }

  // execCommand 虽已 deprecated，但 Tauri WebView 仍完整支持，
  // 且能正确处理选中范围与剪贴板读写，无需额外权限。
  try {
    document.execCommand(action);
  } catch (_) {
    // 静默忽略 — 用户不会看到错误
  }
}

/**
 * 检查元素是否为可编辑文本输入。
 * @param {Element} el
 * @returns {boolean}
 */
function _isEditable(el) {
  if (!el) return false;
  if (el.isContentEditable) return true;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA';
}

/**
 * 检查当前是否有文本被选中。
 * @returns {boolean}
 */
function _hasSelection() {
  const sel = window.getSelection();
  return !!sel && sel.toString().trim().length > 0;
}
