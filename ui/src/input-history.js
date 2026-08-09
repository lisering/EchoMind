/**
 * EchoMind 输入历史 + 草稿持久化 + Token 估算模块。
 *
 * 功能对标 Claude Code 的输入体验：
 * 1. 输入历史导航 — Up/Down 键回溯历史输入（会话级，最近 50 条）
 * 2. 草稿持久化 — 切换会话时保存未发送的输入文本，切回时恢复
 * 3. Token 估算 — 输入时实时显示估算 token 数
 *
 * 设计原则：
 * - 历史记录存储在内存中（不持久化到 DB，避免 I/O 开销）
 * - 草稿存储在 state.drafts（内存级，随会话切换恢复）
 * - Token 估算使用简单字符数 / 4 启发式（无需 BPE 编码器）
 */

import { get, setState } from './state.js';
import { $ } from './utils.js';

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
