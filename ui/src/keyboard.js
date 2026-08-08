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
import { openCommandPalette, closeCommandPalette } from './command-palette.js';
import { createDefaultRegistry, setGlobalRegistry } from './action.js';

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
 * @param {() => void} handlers.onAbort - 停止生成/审计
 * @param {() => void} handlers.onCloseVlm - 关闭 VLM 确认弹窗
 * @param {() => void} handlers.onClosePaywall - 关闭付费墙
 * @param {() => void} handlers.onCloseSettings - 关闭设置面板
 * @param {() => void} handlers.onCloseSearchPopup - 关闭会话搜索弹框
 * @param {() => void} [handlers.onCloseGraph] - 关闭知识图谱查看器
 */
export function initKeyboardShortcuts(handlers) {
  // 创建 Action 注册表并注册为全局单例
  const registry = createDefaultRegistry({
    onNewChat: handlers.onNewChat,
    onImport: handlers.onImport,
    onSettings: handlers.onSettings,
    onToggleSidebar: handlers.onToggleSidebar,
    onCommandPalette: openCommandPalette,
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

    // 2. Esc — 按优先级关闭最上层 Modal（非 Action，涉及多元素优先级）
    if (e.key === 'Escape') {
      // 命令面板优先级最高
      const searchPopup = $('convSearchPopup');
      if (searchPopup && !searchPopup.classList.contains('hidden')) {
        handlers.onCloseSearchPopup();
        return;
      }
      const cmdPalette = $('commandPalette');
      if (cmdPalette && !cmdPalette.classList.contains('hidden')) {
        closeCommandPalette();
        return;
      }
      const vlmConfirm = $('vlmConfirm');
      if (vlmConfirm && !vlmConfirm.classList.contains('hidden')) {
        handlers.onCloseVlm();
        return;
      }
      const paywall = $('paywall');
      if (paywall && !paywall.classList.contains('hidden')) {
        handlers.onClosePaywall();
        return;
      }
      const settingsModal = $('settingsModal');
      if (settingsModal && !settingsModal.classList.contains('hidden')) {
        handlers.onCloseSettings();
        return;
      }
      // 知识库弹框（REQ-KB-001 AC-5: Esc 关闭弹窗）
      const kbModal = $('kbModal');
      if (kbModal && !kbModal.classList.contains('hidden')) {
        kbModal.classList.add('hidden');
        return;
      }
    }
  });
}
