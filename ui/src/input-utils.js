/**
 * EchoMind 输入区工具集 — 合并 input-keymap + input-history + ime-guard。
 *
 * 功能：
 * 1. 统一 keydown 处理器（斜杠命令导航 > Enter 发送 > 历史导航 > Escape 重置）
 * 2. 输入历史导航（Up/Down 回溯历史输入，会话级，最近 50 条）
 * 3. 草稿持久化（切换会话时保存/恢复未发送的输入文本）
 * 4. Token 估算（输入时实时显示估算 token 数）
 * 5. IME 输入法组合事件防护（防 CJK 输入时 Enter 误触发发送）
 *
 * 合并历史：
 * - input-keymap.js（原方案6 统一 keydown 处理器 + ime-guard 合并）
 * - input-history.js（原输入历史 + 草稿持久化 + Token 估算）
 * → S95 合并为 input-utils.js
 */

import { get, setState } from './state.js';
import { $ } from './utils.js';
import { filterSlashCommands, navigateSlashCommand, getSelectedSlashCommand, applySlashCommand, removeSlashCommandPanel } from './slash-commands.js';

// ============================================================
// IME 输入法组合事件防护（原 ime-guard.js）
// ============================================================

/**
 * 检查 keydown 事件是否处于 IME 组合中。
 */
export function isComposingEvent(e) {
  if (!e) return false;
  const ev = /** @type {KeyboardEvent} */ (e);
  return ev.isComposing === true || ev.keyCode === 229;
}

/**
 * 创建一个 IME 组合状态追踪器。
 */
export function createImeGuard() {
  let _composing = false;

  return {
    attach(element) {
      element.addEventListener('compositionstart', () => {
        _composing = true;
      });
      element.addEventListener('compositionend', () => {
        _composing = false;
      });
    },

    isComposing() {
      return _composing;
    },
  };
}

// ============================================================
// 输入历史导航
// ============================================================

/** 每个会话的输入历史（conversationId → string[]），最多 50 条 */
const _histories = new Map();
const MAX_HISTORY = 50;

/** 当前历史导航索引（null = 未在导航中，0 = 最新，n = 最旧） */
let _navIndex = null;

/** 导航前的原始输入（按 Up 之前用户正在输入的文本） */
let _savedInput = '';

/**
 * 记录一条输入到历史。
 *
 * @param {string} query - 已发送的输入文本
 * @returns {void}
 */
export function recordInput(query) {
  if (!query || !query.trim()) return;
  const convId = get('currentConversationId');
  if (!convId) return;

  if (!_histories.has(convId)) {
    _histories.set(convId, []);
  }
  const history = _histories.get(convId);

  // 去重：与上一条相同则不记录
  if (history.length > 0 && history[history.length - 1] === query.trim()) return;

  history.push(query.trim());
  if (history.length > MAX_HISTORY) {
    history.shift();
  }
}

/**
 * 开始历史导航（按 Up 键触发）。
 * 保存当前输入，导航到最新一条历史。
 *
 * @returns {string|null} 替换输入框的历史文本，null 表示无历史
 */
export function navigateHistoryUp() {
  const convId = get('currentConversationId');
  if (!convId) return null;
  const history = _histories.get(convId);
  if (!history || history.length === 0) return null;

  const input = $('queryInput');
  if (!input) return null;

  // 首次按 Up：保存当前输入，导航到最新
  if (_navIndex === null) {
    _savedInput = input.value;
    _navIndex = history.length - 1;
  } else if (_navIndex > 0) {
    _navIndex--;
  } else {
    // 已到达最旧，不移动
    return null;
  }

  return history[_navIndex];
}

/**
 * 向下导航历史（按 Down 键触发）。
 * 到达底部后恢复用户原始输入。
 *
 * @returns {string|null} 替换输入框的文本，null 表示无法继续向下
 */
export function navigateHistoryDown() {
  const convId = get('currentConversationId');
  if (!convId) return null;
  const history = _histories.get(convId);
  if (!history || history.length === 0) return null;
  if (_navIndex === null) return null;

  const input = $('queryInput');
  if (!input) return null;

  if (_navIndex < history.length - 1) {
    _navIndex++;
    return history[_navIndex];
  } else {
    // 已到达最新，恢复用户原始输入
    resetHistoryNav();
    return _savedInput;
  }
}

/**
 * 重置历史导航状态（发送、失焦、切换会话时调用）。
 *
 * @returns {void}
 */
export function resetHistoryNav() {
  _navIndex = null;
  _savedInput = '';
}

/**
 * 清除指定会话的历史记录。
 *
 * @param {string} convId - 会话 ID
 * @returns {void}
 */
export function clearHistory(convId) {
  _histories.delete(convId);
}

// ============================================================
// 草稿持久化
// ============================================================

/**
 * 保存当前输入框内容为当前会话的草稿。
 *
 * @returns {void}
 */
export function saveDraft() {
  const input = $('queryInput');
  if (!input) return;
  const convId = get('currentConversationId');
  if (!convId) return;
  const text = input.value;
  const drafts = { ...get('drafts') };
  if (text.trim()) {
    drafts[convId] = text;
  } else {
    delete drafts[convId];
  }
  setState({ drafts });
}

/**
 * 恢复当前会话的草稿到输入框。
 *
 * @returns {void}
 */
export function restoreDraft() {
  const input = $('queryInput');
  if (!input) return;
  const convId = get('currentConversationId');
  if (!convId) return;
  const drafts = get('drafts');
  const text = drafts[convId] || '';
  input.value = text;
  // 触发 auto-grow
  input.style.height = 'auto';
  input.style.height = Math.min(Math.max(input.scrollHeight, 48), 160) + 'px';
}

/**
 * 清除指定会话的草稿。
 *
 * @param {string} convId - 会话 ID
 * @returns {void}
 */
export function clearDraft(convId) {
  const drafts = { ...get('drafts') };
  delete drafts[convId];
  setState({ drafts });
}

// ============================================================
// Token 估算
// ============================================================

/** Token 估算显示元素 */
let _tokenEstEl = null;

/**
 * 粗略估算文本的 token 数。
 * 启发式：英文约 4 字符/token，中文约 1.5 字符/token，混合取 ~3 字符/token。
 *
 * @param {string} text - 输入文本
 * @returns {number} 估算的 token 数
 */
export function estimateTokens(text) {
  if (!text || !text.trim()) return 0;
  // 统计中文字符数
  const cjkCount = (text.match(/[\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff]/g) || []).length;
  const otherCount = text.length - cjkCount;
  // CJK: ~1.5 char/token, 其他: ~4 char/token
  return Math.ceil(cjkCount / 1.5 + otherCount / 4);
}

/**
 * 更新 token 估算显示。
 * 在输入框右下角显示当前输入的估算 token 数。
 *
 * @returns {void}
 */
export function updateTokenEstimate() {
  const input = $('queryInput');
  if (!input) return;
  const text = input.value;
  const tokens = estimateTokens(text);

  if (!_tokenEstEl) {
    // 懒创建 token 估算元素
    const inputBar = $('inputBar');
    if (!inputBar) return;
    _tokenEstEl = document.createElement('span');
    _tokenEstEl.className = 'token-estimate';
    _tokenEstEl.setAttribute('aria-hidden', 'true');
    _tokenEstEl.style.display = 'none';
    // 挂在右侧按钮组内部（plusBtn 之前），不破坏 toolbar 的 justify-between 布局
    const toolbar = inputBar.querySelector('.flex.items-center.justify-between');
    if (toolbar) {
      const rightSide = toolbar.children[1];
      if (rightSide) {
        rightSide.insertBefore(_tokenEstEl, rightSide.firstChild);
      }
    }
  }

  if (tokens > 0) {
    _tokenEstEl.textContent = `~${tokens} tokens`;
    _tokenEstEl.style.display = 'inline';
  } else {
    _tokenEstEl.textContent = '';
    _tokenEstEl.style.display = 'none';
  }
}

// ============================================================
// 统一 keydown 处理器（原 input-keymap.js）
// ============================================================

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
    } else if (e.key === 'Home') {
      // P2-4: Home 跳转到首项
      e.preventDefault();
      navigateSlashCommand(filtered, 'home');
    } else if (e.key === 'End') {
      // P2-4: End 跳转到末项
      e.preventDefault();
      navigateSlashCommand(filtered, 'end');
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
 * 创建统一的输入 keydown 处理器
 * @param {Object} handlers - 处理函数集合
 * @param {Function} handlers.send - 发送消息函数
 * @returns {Function} 统一的keydown处理器
 */
export function createInputKeyHandler(handlers) {
  return (e) => {
    const { hasSlashPanel, hasDocMention } = checkPopups();
    
    // 0. IME 组合中不拦截任何按键（让浏览器处理候选词导航和确认）
    if (isComposingEvent(e)) return;

    // 1. 斜杠命令导航（最高优先级）
    if (hasSlashPanel || hasDocMention) {
      // P2-4: 添加 Tab/Home/End 到导航键列表
      if (['ArrowUp', 'ArrowDown', 'Escape', 'Enter', 'Tab', 'Home', 'End'].includes(e.key)) {
        handleSlashNavigation(e);
        if (e.defaultPrevented) return;
      }
    }
    
    // 2. Enter 发送（仅当无popup时）
    //    IME 组合中的 Enter 是「确认候选词」而非「发送」，必须忽略
    if (e.key === 'Enter' && !e.shiftKey) {
      if (isComposingEvent(e)) return; // IME 组合中，不拦截 Enter
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
