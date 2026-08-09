/**
 * 输入区统一 keydown 处理器（方案6）。
 *
 * 合并 main.js 和 chat.js 中 queryInput 的两个 keydown 监听器，
 * 提供统一的键映射处理逻辑。
 */

import { get } from './state.js';
import { send } from './chat.js';
import { navigateHistoryUp, navigateHistoryDown, resetHistoryNav } from './input-history.js';
import { filterSlashCommands, navigateSlashCommand, getSelectedSlashCommand, applySlashCommand, removeSlashCommandPanel } from './slash-commands.js';

/**
 * 检查当前是否有 popup 面板打开
 * @returns {{ hasSlashPanel: boolean, hasDocMention: boolean }}
 */
export function checkPopups() {
  const slashPanel = document.querySelector('.slash-command-panel');
  const docMentionPopup = document.querySelector('.doc-mention-popup');
  return {
    hasSlashPanel: !!slashPanel,
    hasDocMention: !!docMentionPopup
  };
}

/**
 * 处理斜杠命令导航
 * @param {KeyboardEvent} e
 */
function handleSlashNavigation(e) {
  const panel = document.querySelector('.slash-command-panel');
  if (panel) {
    const target = /** @type {HTMLTextAreaElement} */ (e.target);
    const filtered = filterSlashCommands(target.value);
    
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      navigateSlashCommand(filtered, 'down');
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      navigateSlashCommand(filtered, 'up');
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      const selected = getSelectedSlashCommand(filtered);
      if (selected) {
        e.preventDefault();
        applySlashCommand(selected, target);
        removeSlashCommandPanel(target.parentElement);
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      removeSlashCommandPanel(target.parentElement);
    }
    return;
  }
  
  // TODO: 文档提及导航逻辑在这里
  // 目前先让chat.js处理
}

/**
 * 历史记录导航
 * @param {KeyboardEvent} e
 * @param {HTMLTextAreaElement} inputEl
 */
function handleHistoryNavigation(e, inputEl) {
  const target = /** @type {HTMLTextAreaElement} */ (e.target);
  const cursorAtFirstLine = target.selectionStart === 0 ||
    target.value.substring(0, target.selectionStart).indexOf('\n') === -1;
  const cursorAtLastLine = target.selectionStart === target.value.length ||
    target.value.substring(target.selectionStart).indexOf('\n') === -1;

  if (e.key === 'ArrowUp' && cursorAtFirstLine) {
    const histText = navigateHistoryUp();
    if (histText !== null) {
      e.preventDefault();
      target.value = histText;
      // 触发 auto-grow
      target.style.height = 'auto';
      const MIN_INPUT_HEIGHT = 48;
      const MAX_INPUT_HEIGHT = 160;
      target.style.height = Math.min(Math.max(target.scrollHeight, MIN_INPUT_HEIGHT), MAX_INPUT_HEIGHT) + 'px';
      // 光标移到末尾
      const len = target.value.length;
      target.setSelectionRange(len, len);
    }
    return;
  }

  if (e.key === 'ArrowDown' && cursorAtLastLine) {
    const histText = navigateHistoryDown();
    if (histText !== null) {
      e.preventDefault();
      target.value = histText;
      // 触发 auto-grow
      target.style.height = 'auto';
      const MIN_INPUT_HEIGHT = 48;
      const MAX_INPUT_HEIGHT = 160;
      target.style.height = Math.min(Math.max(target.scrollHeight, MIN_INPUT_HEIGHT), MAX_INPUT_HEIGHT) + 'px';
      // 光标移到末尾
      const len = target.value.length;
      target.setSelectionRange(len, len);
    }
    return;
  }
}

/**
 * 处理 ⌘+Arrow 轮播导航（占位符，目前无用）
 * @param {KeyboardEvent} e
 * @returns {boolean} 是否处理了事件
 */
function handleCarouselNavigation(e) {
  // 目前没有实现轮播导航，保留接口
  return false;
}

/**
 * 创建统一的输入 keydown 处理器
 * @param {Object} handlers - 处理函数集合
 * @param {Function} handlers.send - 发送消息函数
 * @returns {Function} 统一的keydown处理器
 */
export function createInputKeyHandler(handlers) {
  return (e) => {
    const { hasSlashPanel, hasDocMention } = checkPopups();
    
    // 1. 斜杠命令导航（最高优先级）
    if (hasSlashPanel || hasDocMention) {
      if (['ArrowUp', 'ArrowDown', 'Escape', 'Enter'].includes(e.key)) {
        handleSlashNavigation(e);
        if (e.defaultPrevented) return;
      }
    }
    
    // 2. Enter 发送（仅当无popup时）
    if (e.key === 'Enter' && !e.shiftKey) {
      if (hasSlashPanel || hasDocMention) {
        // chat.js 会处理选择和发送
        return;
      }
      e.preventDefault();
      const target = /** @type {HTMLTextAreaElement} */ (e.target);
      if (!target.value.trim()) return;
      handlers.send();
      return;
    }
    
    // 3. 历史导航（仅无 popup 时）
    if (!hasSlashPanel && !hasDocMention) {
      if (['ArrowUp', 'ArrowDown'].includes(e.key)) {
        handleHistoryNavigation(e, e.target);
        return;
      }
    }
    
    // 4. Escape 重置历史导航
    if (e.key === 'Escape') {
      resetHistoryNav();
    }
  };
}