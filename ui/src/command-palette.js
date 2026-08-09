/**
 * EchoMind 命令面板 — ⌘K 模糊搜索一切功能（FIGMA_DESIGN_SPEC §7 / REQ-KB-002）。
 *
 * 设计灵感：Raycast + VS Code Command Palette + Linear
 *
 * 依赖：state.js（命令面板状态）、utils.js（$ 工具函数）
 */

import { getState, setState, get } from './state.js';
import { $ } from './utils.js';
import { t } from './i18n.js';
import { pushPanel, removePanel } from './panel-stack.js';

// ============================================================
// 命令定义
// ============================================================

/**
 * 命令对象结构。
 * @typedef {Object} Command
 * @property {string} group - 分组名
 * @property {string} icon - 图标字符
 * @property {string} label - 命令名称
 * @property {string} [shortcut] - 快捷键提示
 * @property {() => void} action - 执行函数
 */

/**
 * 模糊匹配命令：支持中文和分组名搜索。
 * @param {string} query - 搜索关键词
 * @param {Command[]} commands - 全部命令
 * @returns {Command[]} 过滤后的命令列表
 */
export function filterCommands(query, commands) {
  const q = query.trim().toLowerCase();
  if (!q) return [...commands];
  return commands.filter((c) =>
    c.label.toLowerCase().includes(q) ||
    c.group.toLowerCase().includes(q),
  );
}

/**
 * 高亮匹配字符：将匹配部分包裹在 <span> 中。
 * @param {string} label - 原始标签
 * @param {string} query - 搜索关键词
 * @returns {string} 包含高亮 span 的 HTML 字符串
 */
export function highlightMatch(label, query) {
  const q = query.trim().toLowerCase();
  if (!q) return label;
  const pos = label.toLowerCase().indexOf(q);
  if (pos < 0) return label;
  return (
    label.slice(0, pos) +
    '<span class="text-accent font-medium">' +
    label.slice(pos, pos + q.length) +
    '</span>' +
    label.slice(pos + q.length)
  );
}

// ============================================================
// 命令面板 DOM 操作
// ============================================================

/**
 * 渲染命令列表到 DOM。
 * @param {Command[]} commands - 全部命令清单
 * @param {string} query - 当前搜索关键词
 */
export function renderCommandList(commands, query) {
  const filtered = filterCommands(query, commands);
  setState({ cmdFiltered: filtered, cmdSelectedIndex: 0 });

  // 分组
  const groups = {};
  filtered.forEach((c) => { (groups[c.group] ||= []).push(c); });

  const list = $('cmdList');
  list.innerHTML = '';

  if (filtered.length === 0) {
    list.innerHTML =
      '<div class="px-4 py-8 text-center text-sm text-text-quaternary">' +
      '<div class="text-3xl mb-2 opacity-30">🔍</div>' + t('command_palette.no_results') + '</div>';
    return;
  }

  let idx = 0;
  const selectedIdx = get('cmdSelectedIndex');
  for (const [group, cmds] of Object.entries(groups)) {
    const header = document.createElement('div');
    header.className = 'px-4 pt-2 pb-1 text-[11px] uppercase tracking-wider text-text-quaternary';
    header.textContent = group;
    list.appendChild(header);
    for (const c of cmds) {
      const item = document.createElement('div');
      item.setAttribute('role', 'option');
      item.dataset.idx = String(idx);
      const isSel = idx === selectedIdx;
      item.className = `flex items-center gap-3 px-4 h-9 cursor-pointer text-sm transition-colors ${isSel ? 'bg-accent/10 text-text-primary' : 'text-text-secondary hover:bg-surface-3'}`;
      const labelHtml = highlightMatch(c.label, query);
      const shortcutHtml = c.shortcut
        ? `<kbd class="text-[11px] text-text-quaternary bg-surface-3 px-1.5 rounded-xs shrink-0">${c.shortcut}</kbd>`
        : '';
      item.innerHTML = `<span class="text-base shrink-0 w-5 text-center">${c.icon}</span><span class="flex-1 truncate">${labelHtml}</span>${shortcutHtml}`;
      item.onclick = () => { c.action(); closeCommandPalette(); };
      list.appendChild(item);
      idx++;
    }
  }
}

/**
 * 更新命令面板选中项视觉（上下键导航）。
 */
export function updateCmdSelection() {
  const selectedIdx = get('cmdSelectedIndex');
  const items = $('cmdList').querySelectorAll('[role="option"]');
  items.forEach((el, i) => {
    const isSel = i === selectedIdx;
    el.className = `flex items-center gap-3 px-4 h-9 cursor-pointer text-sm transition-colors ${isSel ? 'bg-accent/10 text-text-primary' : 'text-text-secondary hover:bg-surface-3'}`;
    if (isSel) el.scrollIntoView({ block: 'nearest' });
  });
}

/**
 * 打开命令面板：记录焦点、渲染列表、聚焦搜索框。
 */
export function openCommandPalette() {
  if (!$('commandPalette').classList.contains('hidden')) {
    closeCommandPalette();
    return;
  }
  setState({ cmdPrevFocus: document.activeElement });
  $('commandPalette').classList.remove('hidden');
  $('cmdSearch').value = '';
  renderCommandList(_commands, '');
  $('cmdSearch').focus();
  // 注册到面板栈
  pushPanel({ id: 'command-palette', close: closeCommandPalette, element: $('commandPalette'), label: 'Command Palette' });
}

/**
 * 关闭命令面板：隐藏、恢复焦点。
 */
export function closeCommandPalette() {
  removePanel('command-palette');
  $('commandPalette').classList.add('hidden');
  const prev = get('cmdPrevFocus');
  if (prev) prev.focus();
}

// ============================================================
// 事件绑定
// ============================================================

/**
 * 初始化命令面板事件监听。
 * 在 main.js 中调用，传入命令清单和外部 action 函数。
 * @param {Command[]} commands - 命令清单
 */
let _commands = [];

export function initCommandPalette(commands) {
  _commands = commands;

  $('cmdSearch').addEventListener('input', (e) => {
    renderCommandList(_commands, e.target.value);
  });

  $('cmdSearch').addEventListener('keydown', (e) => {
    const filtered = get('cmdFiltered');
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (filtered.length > 0) {
        setState({ cmdSelectedIndex: (get('cmdSelectedIndex') + 1) % filtered.length });
        updateCmdSelection();
      }
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (filtered.length > 0) {
        const newIdx = (get('cmdSelectedIndex') - 1 + filtered.length) % filtered.length;
        setState({ cmdSelectedIndex: newIdx });
        updateCmdSelection();
      }
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const selected = filtered[get('cmdSelectedIndex')];
      if (selected) { selected.action(); closeCommandPalette(); }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeCommandPalette();
    }
  });

  // 关闭按钮
  if ($('cmdPaletteClose')) {
    $('cmdPaletteClose').onclick = closeCommandPalette;
  }
}
