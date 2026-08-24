/**
 * EchoMind 全局键盘快捷键模块（FIGMA_DESIGN_SPEC §7.4 / REQ-KB-001）。
 *
 * 重构为 Action 系统委托模式（借鉴 Zed `gpui::Action` 架构）：
 * 所有快捷键通过 ActionRegistry 注册和调度，
 * 不再使用 `if (mod && e.key === ...)` 硬编码。
 *
 * 快捷键清单：
 *   ⌘K   打开/关闭命令面板
 *   ⌘N   新建会话
 *   ⌘O   导入文件
 *   ⌘,   打开设置
 *   ⌘B   折叠/展开侧栏
 *   Esc  关闭最上层 Modal（仍用优先级判断，因为 Esc 不是 Action 而是 UI 层行为）
 */

import { $, isInputFocused } from './utils.js';
import { createDefaultRegistry, setGlobalRegistry } from './action.js';
import { closeTopPanel, hasOpenPanels } from './panel-stack.js';
import { openKeyboardHelp, closeKeyboardHelp, isKeyboardHelpOpen } from './help-panel.js';
import { openCommandPalette, closeCommandPalette, closeGlobalSearch } from './search-ui.js';

/**
 * 初始化全局键盘快捷键。
 *
 * 重构后通过 ActionRegistry 统一注册和调度，
 * Esc 关闭 Modal 逻辑保留在 keydown 监听器中（因为涉及多元素优先级判断）。
 *
 * @param {Object} handlers - 各快捷键对应的 action 函数
 * @param {() => void} handlers.onNewChat - 新建会话
 * @param {() => void} handlers.onImport - 导入文件
 * @param {() => void} handlers.onSettings - 打开设置
 * @param {() => void} [handlers.onToggleSidebar] - 折叠/展开侧栏（⌘B）
 * @param {() => void} [handlers.onKeyboardHelp] - 显示快捷键帮助（⌘/）
 * @param {() => void} handlers.onAbort - 停止生成/审计
 * @param {() => void} handlers.onCloseVlm - 关闭 VLM 确认弹窗
 * @param {() => void} handlers.onClosePaywall - 关闭付费墙
 * @param {() => void} handlers.onCloseSettings - 关闭设置面板
 * @param {() => void} handlers.onCloseSearchPopup - 关闭会话搜索弹框
 * @param {() => void} [handlers.onCloseGraph] - 关闭知识图谱查看器
 * @param {() => void} [handlers.onGlobalSearch] - 全局搜索（⌘⇧F）
 * @param {() => void} [handlers.onExport] - 导出当前对话（⌘E）
 */
export function initKeyboardShortcuts(handlers) {
  handlersRef = handlers;
  // 创建 Action 注册表并注册为全局单例
  const registry = createDefaultRegistry({
    onNewChat: handlers.onNewChat,
    onImport: handlers.onImport,
    onSettings: handlers.onSettings,
    onToggleSidebar: handlers.onToggleSidebar,
    onCommandPalette: openCommandPalette,
    onKeyboardHelp: openKeyboardHelp,
    onGlobalSearch: handlers.onGlobalSearch,
    onExport: handlers.onExport,
    isInputFocused: isInputFocused,
    isAppVisible: () => {
      const app = $('app');
      return app ? !app.classList.contains('hidden') : false;
    },
  });
  setGlobalRegistry(registry);

  document.addEventListener('keydown', (e) => {
    // 1. 尝试通过 ActionRegistry 调度快捷键
    const appVisible = !$('app') ? false : !$('app').classList.contains('hidden');
    const handled = registry.dispatchKeydown(e, {
      appVisible,
      inputFocused: isInputFocused(),
    });
    if (handled) return;

    // 2. Esc — 先尝试关闭面板栈栈顶（动态面板）
    if (e.key === 'Escape') {
      // 优先关闭 panel-stack 中的栈顶面板
      if (closeTopPanel()) return;

      // 静态面板（数据驱动优先级表，V3.1 P3-6：消除硬编码 if 链）
      for (const entry of STATIC_ESC_PANELS) {
        const el = $(entry.id);
        if (el && !el.classList.contains('hidden')) {
          if (typeof entry.close === 'function') entry.close();
          else el.classList.add('hidden');
          return;
        }
      }
    }
  });
}

/**
 * 静态面板 Esc 关闭注册表（按优先级从高到低）。
 * close 为函数时调用之；缺省则直接 addClass('hidden')。
 * 新增面板只需在此表加一行。
 */
const STATIC_ESC_PANELS = [
  { id: 'convSearchPopup', close: () => handlersRef.onCloseSearchPopup() },
  { id: 'commandPalette', close: () => closeCommandPalette() },
  { id: 'globalSearch', close: () => closeGlobalSearch() },
  { id: 'vlmConfirm', close: () => handlersRef.onCloseVlm() },
  { id: 'paywall', close: () => handlersRef.onClosePaywall() },
  { id: 'settingsModal', close: () => handlersRef.onCloseSettings() },
  { id: 'kbModal' }, // REQ-KB-001 AC-5：Esc 关闭知识库弹框
];

/** initKeyboardShortcuts 的 handlers 引用（STATIC_ESC_PANELS 闭包使用） */
let handlersRef = {};
