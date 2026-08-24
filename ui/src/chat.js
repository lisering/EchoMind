/**
 * EchoMind 对话流核心模块 — send / stop / finalize / watchdog / event listeners。
 *
 * 从原 chat.js 拆分而来，保留核心逻辑：
 * 1. 客户端看门狗超时保护
 * 2. 输入框状态机（idle/streaming/error）
 * 3. 流结束收尾（finalizeStream）
 * 4. 发送与停止（send / onStop）
 * 5. 文档审计（auditDocument）
 * 6. 错误操作处理
 * 7. 事件监听注册（chat_token/chat_done/chat_phase/chat_error/chat_sources）
 *
 * DOM 渲染函数已拆分到 chat-render.js。
 */

import { getState, setState, get } from './state.js';
import { $, sanitizeError } from './utils.js';
import { invoke, listen } from './ipc.js';
import { toast, toastError } from './toast.js';
import { renderRichContent } from './markdown.js';
import { t } from './i18n.js';
import { runGuard, canSend, requireIdle, requirePro } from './action.js';
import { removeCaret, registerChatActions } from './chat-render.js';
import { initScrollLock, createBackToTopButton, createJumpToLatestButton, notifyNewMessage } from './chat-utils.js';
import { classifyError, renderErrorCard } from './error-detail.js';

// ============================================================
// 屏幕阅读器状态播报（REQ-A11Y-004）
// ============================================================

/**
 * 向屏幕阅读器播报状态消息（REQ-A11Y-004 AC-1/AC-2/AC-3）。
 * 更新 #srStatus aria-live="polite" 区域。
 * @param {string} message - 播报消息文本
 */
function announceStatus(message) {
  const sr = $('srStatus');
  if (sr) sr.textContent = message;
}

/**
 * 向屏幕阅读器播报错误消息（REQ-A11Y-004 AC-4）。
 * 更新 #srError aria-live="assertive" 区域。
 * @param {string} message - 错误描述
 */
function announceError(message) {
  const sr = $('srError');
  if (sr) sr.textContent = message;
}
import { updateContextUsage, renderContextBar, getContextLevel } from './context-bar.js';
import { filterSlashCommands, renderSlashCommandPanel, navigateSlashCommand, getSelectedSlashCommand, applySlashCommand, removeSlashCommandPanel, processSlashCommand } from './slash-commands.js';
import { extractDocMentions, getDocFilter, renderDocMentionPopup, insertDocMention, removeDocMentionPopup, filterDocuments } from './doc-mention.js';
import { navigateCarousel, updateCarouselDisplay, addCarouselVersion } from './chat-render.js';
import { createFeedbackButtons, detectAndReportImplicit, resetFeedbackTracking } from './feedback.js';
import { enqueueQuery, processQueue, updateSendButton, clearQueue } from './chat-utils.js';
import { updateInputUI } from './action.js';
import { recordInput, resetHistoryNav, saveDraft } from './input-utils.js';
import { renderFollowupSuggestions } from './chat-render.js';
import { buildHistoryFromTurns, updateVersionAssistant } from './turn-tree.js';
import { renderBranchPagination, removeBranchPagination } from './message-edit.js';

// 从 chat-render.js 导入渲染函数（构建后拼接在同一作用域）
import { updateModelPill, resetChatArea, hideEmptyState, appendBlock, appendUserBlock,
         appendAiDisclaimer, scheduleRender, renderSources, showMsgActions,
         scrollToBottom, showCollapsedHint, showApp, showWizard } from './chat-render.js';

// ============================================================
// 客户端看门狗超时（V2 修复：防止后端永久挂起时前端卡死）
// ============================================================

/** 看门狗超时时间（毫秒）。后端 embedder 初始化超时 180s + LLM 流式 120s = 300s 上限。
 *  测试可通过 `window.__ECHOMIND_WATCHDOG_TIMEOUT_MS__` 覆盖（如设为 2000 用于 E2E 测试）。 */
const CHAT_WATCHDOG_TIMEOUT_MS =
  (typeof window !== 'undefined' && window.__ECHOMIND_WATCHDOG_TIMEOUT_MS__) || 300_000;
/** 看门狗计时器引用（null 表示未激活）。 */
let _chatWatchdog = null;

/**
 * 错误去重标志：防止 chat_error 事件与 invoke rejection 双重报告。
 */
let _chatErrorHandled = false;

/**
 * 中断标志：chat_error 中断消息设置，让 chat_done 以 false（非正常完成）处理。
 */
let _chatAborted = false;

/**
* P1-3: /web 命令临时启用网页搜索标志。
* 在 chat_done 后恢复为 false 并关闭网页搜索。
*/
/* P1-3/P1-4 的 /web、/agent 临时标志已并入 state.js（V3.1 P4-4：tempWebSearch/tempAgent） */

/**
 * 启动看门狗计时器。
 */
function startChatWatchdog() {
  clearChatWatchdog();
  _chatWatchdog = setTimeout(() => {
    _chatWatchdog = null;
    if (!get('streaming')) return;
    const msg = t('chat.watchdog_timeout');
    toastError(msg);
    finalizeStream(false);
    setInputState('error', sanitizeError(msg));
  }, CHAT_WATCHDOG_TIMEOUT_MS);
}

/** 重置看门狗计时器（收到活动事件时调用，inactivity 模式）。 */
function resetChatWatchdog() {
  if (_chatWatchdog) {
    clearTimeout(_chatWatchdog);
    _chatWatchdog = setTimeout(() => {
      _chatWatchdog = null;
      if (!get('streaming')) return;
      const msg = t('chat.watchdog_timeout');
      toastError(msg);
      finalizeStream(false);
      setInputState('error', sanitizeError(msg));
    }, CHAT_WATCHDOG_TIMEOUT_MS);
  }
}

/** 清除看门狗计时器（流结束或错误时调用）。 */
function clearChatWatchdog() {
  if (_chatWatchdog) {
    clearTimeout(_chatWatchdog);
    _chatWatchdog = null;
  }
}

// ============================================================
// 输入框状态机
// ============================================================

/**
 * 设置输入框状态机（idle / streaming / error）。
 *
 * V5 重构：不再直接操作 DOM，委托给统一的 updateInputUI()。
 * 流式期间输入框保持启用（支持排队发送），仅由 updateSendButton() 管理按钮形态切换。
 *
 * @param {'idle'|'streaming'|'error'} state - 目标状态
 * @param {string} [hint=''] - 状态提示文案
 */
export function setInputState(state, hint = '') {
  const isStreaming = state === 'streaming';
  setState({ streaming: isStreaming });
  setInputHint(hint);
  updateSendButton();
  updateInputUI();
  if (!isStreaming) {
    const docCount = get('docCount');
    if (docCount > 0 && !requireUnlocked_guard()) {
      $('queryInput').focus();
    }
  }
}

/** 安全锁定守卫内联检查（避免循环导入） */
function requireUnlocked_guard() {
  return get('securityState') === 'locked';
}

/**
 * 统一设置 inputHint 文案（所有 hint 更新都经过此函数，避免碎片化）。
 * @param {string} text - 提示文案（空字符串清空）
 */
export function setInputHint(text) {
  const hint = $('inputHint');
  if (!hint) return;
  hint.textContent = text || '';
}

// ============================================================
// 流结束收尾
// ============================================================

/**
 * 流结束后的状态收尾。
 * @param {boolean} ok - true=正常完成；false=错误/中断
 */
export function finalizeStream(ok) {
  const assistantEl = get('currentAssistantEl');
  const rawMd = get('currentRawMarkdown');
  const sources = get('lastSources');

  if (assistantEl) {
    if (assistantEl._thinkingPanel) {
      assistantEl._thinkingPanel.setComplete();
      // 流完成后把流式累加的纯文本思考内容做一次性 markdown 渲染（对齐 DeepSeek）
      assistantEl._thinkingPanel.finalizeReasoning();
      // 恢复持久化的展开/折叠状态（msgId 在 send/sendFromEdit 中已设置）
      const msgId = assistantEl.dataset.msgId || null;
      if (msgId) {
        assistantEl._thinkingPanel.setMsgId(msgId);
      } else {
        assistantEl._thinkingPanel.collapse();
      }
    }
    const thinking = assistantEl.querySelector('.thinking-indicator');
    if (thinking) thinking.remove();
    const mdEl = assistantEl.querySelector('.md');
    if (mdEl) {
      removeCaret(mdEl);
    }
  if (ok && rawMd) {
      showMsgActions(assistantEl, rawMd);
      createFeedbackButtons(assistantEl);
      renderFollowupSuggestions(assistantEl, rawMd, '', (question) => {
        $('queryInput').value = question;
        send();
      });
      appendAiDisclaimer(assistantEl);
      // 为新元素添加 fade-in 动画，避免突然出现造成视觉跳动
      assistantEl.querySelectorAll('.msg-actions, .followup-suggestions, .ai-disclaimer').forEach((el) => {
        if (!el.classList.contains('animate-fade-in')) {
          el.classList.add('animate-fade-in');
        }
      });
    }
  }

  // 分支版本收尾：更新轮次树中的助手回答内容 + 刷新分页器
  const editTurnGroup = assistantEl?.dataset.turnGroup;
  const editVersion = assistantEl?.dataset.version;
  if (ok && rawMd && editTurnGroup && editVersion) {
    const reasoning = assistantEl?._thinkingPanel?.getReasoning?.() || null;
    updateVersionAssistant(editTurnGroup, parseInt(editVersion, 10), rawMd, get('lastSources'), reasoning);
    // 流完成后渲染/刷新分页器（挂在用户操作栏中，与复制/编辑按钮同行）
    // renderBranchPagination 内部解析 user 块 → 查找 user-actions 操作栏
    const userBlock = findUserBlockForAssistant(assistantEl);
    if (userBlock) {
      renderBranchPagination(userBlock, editTurnGroup);
    }
  }

  // 重新生成（carousel）收尾：把本轮新回答追加为轮播新版本（AC-4）
  // 注：carousel 容器插入在原 assistant 块；此处仅当当前块持有 _regenCarousel 引用时追加
  const regenCarousel = assistantEl?._regenCarousel || null;
  if (ok && rawMd && regenCarousel) {
    addCarouselVersion(regenCarousel, rawMd, get('lastSources'));
    delete assistantEl._regenCarousel;
  }

  if (ok && rawMd) {
    const history = [...get('history'), { role: 'assistant', content: rawMd, sources }];
    setState({ history });
  }

  setState({
    currentRawMarkdown: '',
    lastSources: null,
    currentAssistantEl: null,
  });

  if (ok) {
    processQueue((query) => {
      $('queryInput').value = query;
      send();
    });
  }
}

// ============================================================
// 发送与停止
// ============================================================

/**
 * 发送用户问题：追加 user Block、初始化 assistant Block、调用 chat IPC 启动流式对话。
 */
/** 上次发送时间戳（ms）— 用于冷却期判断 */
let _lastSendTime = 0;
/** 冷却期（ms）— 两次发送之间的最小间隔 */
const SEND_COOLDOWN_MS = 800;

// ============================================================
// P2-1：流恢复机制（autoResume）
// ============================================================

/** localStorage 存储键：未完成的流状态 */
const STREAM_STATE_KEY = 'echomind_stream_state';

/**
 * 保存流状态到 localStorage（在 send() 中调用）。
 * @param {string} conversationId - 会话 ID
 * @param {string} query - 用户查询
 */
function _saveStreamState(conversationId, query) {
  try {
    localStorage.setItem(STREAM_STATE_KEY, JSON.stringify({
      conversationId,
      query,
      timestamp: Date.now(),
    }));
  } catch (_) { /* 隐私模式 */ }
}

/**
 * 清除流状态（在 chat_done / chat_error 中调用）。
 */
function _clearStreamState() {
  try {
    localStorage.removeItem(STREAM_STATE_KEY);
  } catch (_) { /* 隐私模式 */ }
}

/**
 * 检查是否有未完成的流（页面加载时调用）。
 * 如果流状态存在且不超过 5 分钟，显示恢复提示。
 */
export function checkStreamResume() {
  try {
    const raw = localStorage.getItem(STREAM_STATE_KEY);
    if (!raw) return null;
    const state = JSON.parse(raw);
    if (!state || !state.conversationId) return null;
    // 超过 5 分钟的流状态视为过期
    if (Date.now() - (state.timestamp || 0) > 5 * 60 * 1000) {
      localStorage.removeItem(STREAM_STATE_KEY);
      return null;
    }
    return state;
  } catch (_) { return null; }
}

export async function send() {
  // P1-3：操作过快限制 — 冷却期内阻止发送
  const now = Date.now();
  if (now - _lastSendTime < SEND_COOLDOWN_MS && !get('streaming')) {
    const remaining = Math.ceil((SEND_COOLDOWN_MS - (now - _lastSendTime)) / 1000 * 10) / 10;
    toast(t('chat.cooldown_hint', { seconds: remaining }) || `操作过快，请等待 ${remaining}s`, 'warning');
    return;
  }
  _lastSendTime = now;
  let query;
  try {
    const input = $('queryInput');
    if (!input) { console.error('[EchoMind] send() 失败：#queryInput 不存在'); return; }
    query = input.value.trim();
    if (!query) return;
    if (get('streaming')) {
      enqueueQuery(query);
      input.value = '';
      input.style.height = '48px';
      updateSendButton();
      toast(t('chat.queued') || '已排队，将在当前回答完成后自动发送', 'info');
      return;
    }
    if (!runGuard(canSend())) return;
    // 记录输入历史（供 Up/Down 导航回溯）
    recordInput(query);
    // 清除草稿（已发送）
    saveDraft();
    input.value = '';
    input.style.height = '48px';
    resetHistoryNav();
    // S56：快捷指令展开 — /command query → promptTemplate.replace('{query}', query)
    // P1-3: /web 命令特殊处理 — 临时启用网页搜索，不展开 prompt
    const slashResult = processSlashCommand(query);
    setState({ tempWebSearch: false, tempAgent: false });
    if (slashResult.matched) {
      if (slashResult.command?.name === 'web') {
        // /web 命令：使用原始查询，临时启用网页搜索
        setState({ tempWebSearch: true });
        try {
          await invoke('update_setting', { key: 'rag.web_search_enabled', value: 'true' });
        } catch (_e) { /* 网页搜索可能不可用，静默降级 */ }
      } else if (slashResult.command?.name === 'agent') {
        // /agent 命令：使用原始查询，临时启用 Agent 模式
        setState({ tempAgent: true });
        try {
          await invoke('update_setting', { key: 'rag.agent_enabled', value: 'true' });
        } catch (_e) { /* Agent 模式可能不可用，静默降级 */ }
      } else {
        query = slashResult.text;
      }
    }
    // REQ-PERF-012 扩展：隐式反馈采集 — 检测与上一轮查询的相似度
    detectAndReportImplicit(query, false);
    appendUserBlock(query);
  } catch (syncErr) {
    console.error('[EchoMind] send() 同步阶段错误:', syncErr);
    toastError(syncErr);
    setInputState('error', sanitizeError(syncErr));
    return;
  }

    const history = [...get('history'), { role: 'user', content: query, sources: null }];
    setState({
      history,
      currentRawMarkdown: '',
      lastSources: null,
    });
    // 更新输入区 UI（发送后空输入）
    updateInputUI();

  const assistantEl = appendBlock('assistant');
  assistantEl.dataset.query = query;
  // 尽早设置 msgId（使用用户消息块的 ID）用于思考面板状态持久化
  // 新发送的消息前端暂无 DB id，流完成后由 finalizeStream 从后端获取并设置
  const userBlocks = document.querySelectorAll('.msg-user');
  const lastUserBlock = userBlocks[userBlocks.length - 1];
  if (lastUserBlock?.dataset.msgId) {
    assistantEl.dataset.msgId = lastUserBlock.dataset.msgId;
    if (assistantEl._thinkingPanel) {
      assistantEl._thinkingPanel.setMsgId(assistantEl.dataset.msgId);
    }
  }
  setState({ currentAssistantEl: assistantEl });
  scrollToBottom();
  setInputState('streaming', t('chat.retrieving_kb'));
  startChatWatchdog();
  _chatErrorHandled = false;
  _chatAborted = false;
  // P2-1：保存流状态（用于页面刷新后恢复检测）
  _saveStreamState(get('currentConversationId'), query);

  try {
    await invoke('chat', {
      query,
      history: history.slice(0, -1),
      conversationId: get('currentConversationId'),
      docFilter: getDocFilter(extractDocMentions(query)),
    });
  } catch (err) {
    clearChatWatchdog();
    if (!_chatErrorHandled) {
      toastError(err);
    }
    finalizeStream(false);
    setInputState('error', sanitizeError(err));
  }
}

// ============================================================
// 编辑模式发送（就地分支）
// ============================================================

/**
 * 从编辑模式发送：就地更新用户消息 + 清空并重用 assistant 块 + 流式重新生成。
 *
 * 与 send() 的区别：
 * - 不创建新的 user block（就地更新文本）
 * - 不创建新的 assistant block（清空并重用现有的）
 * - 传递 turn_group + version 给 chat IPC（用于 DB 持久化版本信息）
 *
 * @param {HTMLElement} editTargetBlock - 被编辑的用户消息块
 * @param {string} newContent - 编辑后的新问题文本
 * @param {string} turnGroup - 轮次分组 ID
 * @param {number} version - 新版本号
 * @returns {Promise<void>}
 */
export async function sendFromEdit(editTargetBlock, newContent, turnGroup, version) {
  if (!runGuard(canSend())) return;

  // REQ-PERF-012 扩展：编辑重发 → 隐式负信号
  detectAndReportImplicit(newContent, true);

  // 1. 就地更新用户消息内容（带淡入动画）
  const contentEl = editTargetBlock.querySelector('.msg-user-content');
  if (contentEl) {
    contentEl.textContent = newContent;
    contentEl.classList.remove('animate-fade-in');
    // 触发重排以重启动画
    void contentEl.offsetWidth;
    contentEl.classList.add('animate-fade-in');
  }
  editTargetBlock.dataset.fullText = newContent;
  editTargetBlock.dataset.turnGroup = turnGroup;
  editTargetBlock.dataset.version = String(version);

  // 2. 移除旧的分页器（流完成后由 finalizeStream 重新渲染）
  removeBranchPagination(editTargetBlock);

  // 3. 找到或创建 assistant 块
  let assistantEl = findNextAssistantBlock(editTargetBlock);
  if (!assistantEl) {
    // 没有 assistant 块 → 创建一个新的
    assistantEl = appendBlock('assistant');
    // 将 assistant 块移到 user actions bar 之后
    const actionsBar = editTargetBlock.nextElementSibling;
    if (actionsBar && actionsBar.dataset.role === 'user-actions') {
      actionsBar.after(assistantEl);
    } else {
      editTargetBlock.after(assistantEl);
    }
  } else {
    // 旧 assistant 块内容淡出动画
    const oldContent = assistantEl.querySelector('.msg-content');
    if (oldContent) {
      oldContent.classList.add('msg-content-editing-out');
    }
  }

  // 4. 延迟清空 assistant 块内容（等淡出动画完成），重置为流式初始状态
  await new Promise(resolve => setTimeout(resolve, 150));
  resetAssistantBlock(assistantEl);
  assistantEl.dataset.query = newContent;
  assistantEl.dataset.turnGroup = turnGroup;
  assistantEl.dataset.version = String(version);
  // 设置 msgId（使用用户消息块的 ID）用于思考面板状态持久化
  const userMsgId = editTargetBlock.dataset.msgId || null;
  if (userMsgId) {
    assistantEl.dataset.msgId = userMsgId;
    if (assistantEl._thinkingPanel) {
      assistantEl._thinkingPanel.setMsgId(userMsgId);
    }
  }

  // 5. 更新状态
  setState({
    currentRawMarkdown: '',
    lastSources: null,
    currentAssistantEl: assistantEl,
  });

  // 6. 构建历史（使用轮次树的活跃版本，排除当前编辑的轮次）
  const history = buildHistoryFromTurns();
  // 移除最后一条（当前编辑的 user 消息，因为 chat_inner 会自己加）
  const trimmedHistory = history.slice(0, -1);

  // 7. 启动流式 — 编辑场景滚动到被编辑的 assistant 块，而非页面底部
  //    这样编辑中间消息时视口聚焦在当前 Q&A，不会跳到底部
  assistantEl.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  setInputState('streaming', t('chat.retrieving_kb'));
  startChatWatchdog();
  _chatErrorHandled = false;
  _chatAborted = false;

  try {
    await invoke('chat', {
      query: newContent,
      history: trimmedHistory,
      conversationId: get('currentConversationId'),
      turnGroup,
      version,
    });
  } catch (err) {
    clearChatWatchdog();
    if (!_chatErrorHandled) {
      toastError(err);
    }
    finalizeStream(false);
    setInputState('error', sanitizeError(err));
  }
}

/**
 * 找到用户消息块之后紧邻的 assistant 消息块。
 * @param {HTMLElement} userBlockEl
 * @returns {HTMLElement|null}
 */
function findNextAssistantBlock(userBlockEl) {
  let next = userBlockEl.nextElementSibling;
  while (next) {
    if (next.classList.contains('msg-block') && next.classList.contains('msg-assistant')) {
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      return next;
    }
    next = next.nextElementSibling;
  }
  return null;
}

/**
 * 找到 assistant 块对应的 user 块。
 * @param {HTMLElement} assistantEl
 * @returns {HTMLElement|null}
 */
function findUserBlockForAssistant(assistantEl) {
  let prev = assistantEl.previousElementSibling;
  while (prev) {
    if (prev.classList.contains('msg-block') && prev.classList.contains('msg-user')) {
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      return prev;
    }
    prev = prev.previousElementSibling;
  }
  return null;
}

/**
 * 重置 assistant 块为流式初始状态（清空内容、显示思考指示器）。
 * @param {HTMLElement} assistantEl
 * @returns {void}
 */
function resetAssistantBlock(assistantEl) {
  // 移除旧内容淡出动画类
  const oldContent = assistantEl.querySelector('.msg-content');
  if (oldContent) oldContent.classList.remove('msg-content-editing-out');
  // 清空 Markdown 内容
  const mdEl = assistantEl.querySelector('.md');
  if (mdEl) {
    mdEl.innerHTML = '';
    mdEl.classList.remove('md-fade-in');
    delete mdEl.dataset.rawMarkdown;
  }
  // 清空来源
  const sourcesEl = assistantEl.querySelector('.sources');
  if (sourcesEl) {
    sourcesEl.innerHTML = '';
    sourcesEl.className = 'sources mt-2';
  }
  // 清空操作栏
  const actionsEl = assistantEl.querySelector('.msg-actions');
  if (actionsEl) actionsEl.innerHTML = '';
  // 移除旧的轮播/免责声明/后续建议（流式期间不显示，流完成后再渲染）
  // 注：branch-pagination 现在挂在用户操作栏中，由 sendFromEdit 调用 removeBranchPagination 清理
  assistantEl.querySelectorAll('.regen-carousel, .ai-disclaimer, .followup-suggestions').forEach((el) => el.remove());
  // 重置思考面板
  if (assistantEl._thinkingPanel) {
    assistantEl._thinkingPanel.reset();
  }
}

/**
 * 停止按钮点击处理。
 */
export async function onStop() {
  clearChatWatchdog();
  try {
    const auditingDocId = get('auditingDocId');
    if (auditingDocId) {
      await invoke('abort_audit', { docId: auditingDocId });
    } else {
      await invoke('abort_chat', { conversationId: get('currentConversationId') });
    }
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 审计文档
// ============================================================

/**
 * 文档一致性审计（REQ-AUDIT-001~005）。
 * @param {string} docId - 文档 ID
 * @param {string} docName - 文档显示名
 */
export async function auditDocument(docId, docName) {
  if (!runGuard(requireIdle())) return;
  if (!runGuard(requirePro())) return;

  setState({ auditingDocId: docId });
  const assistantEl = appendBlock('assistant');
  setState({ currentAssistantEl: assistantEl });

  const thinkingText = assistantEl.querySelector('.thinking-text');
  if (thinkingText) thinkingText.textContent = t('chat.thinking_audit_init');
  setInputState('streaming', t('chat.thinking_auditing'));

  try {
    await invoke('audit_document', { docId, docName });
  } catch (err) {
    finalizeStream(false);
    setInputState('error', String(err));
    setState({ auditingDocId: null });
  }
}

// ============================================================
// 错误操作处理（P1-3）
// ============================================================

/**
 * 处理错误卡片上的恢复操作按钮点击。
 * @param {string} action - 操作标识
 * @param {string} [queryContext] - 重新生成时使用的查询文本
 */
function handleErrorAction(action, queryContext) {
  switch (action) {
    case 'retry':
      if (queryContext) {
        $('queryInput').value = queryContext;
        send();
      }
      break;
    case 'open_settings':
    case 'switch_model':
    case 'check_model':
      $('settingsBtn')?.click();
      break;
    case 'import_files':
      $('plusBtn')?.click();
      break;
    case 'new_chat':
      $('newChatBtn')?.click();
      break;
    case 'switch_local':
    case 'switch_cloud':
      $('settingsBtn')?.click();
      toast(t('chat.error_action_switch_hint') || '请在设置中切换推理模式', 'info');
      break;
    case 'upgrade_pro':
      $('settingsBtn')?.click();
      break;
    case 'compress_history':
      toast(t('chat.error_action_compress_hint', '正在压缩历史记录…') || '正在压缩历史记录…', 'info');
      break;
    default:
      break;
  }
}

// ============================================================
// 事件监听注册
// ============================================================

/**
 * 注册对话流相关的所有 Tauri 事件监听。
 * 在 main.js 中调用。
 */
export function initChatEventListeners(callbacks) {
  const { onChatDone, onDocStatusChanged, onImportProgress, onEmbeddingProgress, onModelDownload, onIndexingPhase } = callbacks;

  const chatArea = $('chatArea');
  if (chatArea) {
    initScrollLock(chatArea);
    // REQ-NAV-005：添加「回到顶部」浮动按钮
    const chatWrapper = chatArea.parentElement;
    if (chatWrapper && !document.getElementById('backToTopBtn')) {
      const backToTopBtn = createBackToTopButton();
      chatWrapper.appendChild(backToTopBtn);
    }
    // 确保 jump-to-latest 按钮也存在
    if (chatWrapper && !document.querySelector('.jump-to-latest:not(#backToTopBtn)')) {
      const jumpBtn = createJumpToLatestButton();
      chatWrapper.appendChild(jumpBtn);
    }
  }

  listen('audit_phase', (e) => {
    const p = e.payload;
    const el = get('currentAssistantEl');
    if (el) {
      const textEl = el.querySelector('.thinking-text');
      if (textEl) textEl.textContent = p.message;
    }
    $('inputHint').textContent = p.message;
  });

  listen('chat_phase', (e) => {
    const p = e.payload;
    const el = get('currentAssistantEl');
    // REQ-ERR-002 AC-2：重试期间显示"重连中…（n/3）"
    if (p.phase === 'retrying') {
      const attempt = p.attempt || 1;
      const maxRetries = p.max_retries || 3;
      const retryMsg = t('chat.retrying', '重连中…（{n}/{m}）')
        .replace('{n}', String(attempt))
        .replace('{m}', String(maxRetries));
      if (el && el._thinkingPanel) {
        el._thinkingPanel.update(retryMsg, 'retrying');
        el._thinkingPanel.appendStage(retryMsg);
      }
      const textEl = el?.querySelector('.thinking-text');
      if (textEl) textEl.textContent = retryMsg;
      $('inputHint').textContent = retryMsg;
      resetChatWatchdog();
      return;
    }
    if (el) {
      if (el._thinkingPanel) {
        // 首次 chat_phase 时记录思考开始时间（用于耗时计算）
        if (!el._thinkingPanel.isFirstTokenReceived()) {
          el._thinkingPanel.startThinking();
        }
        // 需求 5：传 phase 触发图标流转（preparing→时钟 / retrieving→放大镜 / generating→星星）
        el._thinkingPanel.update(p.message, p.phase);
        el._thinkingPanel.appendStage(p.message);
      }
      const textEl = el.querySelector('.thinking-text');
      if (textEl) textEl.textContent = p.message;
    }
    $('inputHint').textContent = p.message;
    // 屏幕阅读器播报：流式回答开始时播报（REQ-A11Y-004 AC-2）
    if (p.phase === 'generating' || p.phase === 'preparing') {
      announceStatus(t('chat.sr_generating', 'Generating answer...'));
    }
    resetChatWatchdog();
  });

  listen('chat_reasoning', (e) => {
    const el = get('currentAssistantEl');
    if (el && el._thinkingPanel) {
      el._thinkingPanel.appendReasoning(String(e.payload ?? ''));
    }
  });

  // REQ-RAG-052: Agent 步骤可视化 — 监听 agent_step 事件
  listen('agent_step', (e) => {
    const el = get('currentAssistantEl');
    if (el && el._thinkingPanel) {
      const step = e.payload;
      // 展开面板以显示步骤
      el._thinkingPanel.expand();
      el._thinkingPanel.appendAgentStep(step);
      // 更新进度条
      if (step.iteration) {
        el._thinkingPanel.setAgentProgress(step.iteration, 5);
      }
    }
  });

  listen('chat_token', (e) => {
    const el = get('currentAssistantEl');
    if (el) {
      if (el._thinkingPanel) {
        // 标记已收到首个 token（AWAITING_FIRST_CHANGE → GENERATING 过渡）
        if (!el._thinkingPanel.isFirstTokenReceived()) {
          el._thinkingPanel.markFirstTokenReceived();
        }
        // 标记思考完成（显示耗时），但不强制折叠（用户可能已展开面板实时查看推理过程）
        // setComplete 现在是 async（含 ensureMinLoadingDelay），但不阻塞 token 渲染
        el._thinkingPanel.setComplete();
      }
      const thinking = el.querySelector('.thinking-indicator');
      if (thinking) thinking.remove();
      const mdEl = el.querySelector('.md');
      if (mdEl) {
        // .md 已不再使用 hidden 类（初始即为可见空容器，0 高度）
        // 首次显示时添加渐入动画
        if (!mdEl.classList.contains('md-fade-in')) {
          mdEl.classList.add('md-fade-in');
        }
      }
    }
    setState({ currentRawMarkdown: get('currentRawMarkdown') + e.payload });
    scheduleRender();
    resetChatWatchdog();
  });

  listen('chat_sources', (e) => renderSources(e.payload));

  listen('chat_error', (e) => {
    clearChatWatchdog();
    // P2-1：清除流状态
    _clearStreamState();
    _chatErrorHandled = true;
    const msg = String(e.payload);
    if (msg.includes('已中断') || msg.includes('aborted') || msg.includes('Aborted')) {
      // Bug #9 修复：标记中断状态，让后续 chat_done 事件以 finalizeStream(false) 处理
      // 而非 finalizeStream(true)，避免中断 badge 被正常完成状态覆盖
      _chatAborted = true;
      const el = get('currentAssistantEl');
      if (el) {
        const badge = document.createElement('div');
        badge.className = 'mt-2 text-[11px] text-text-quaternary';
        badge.textContent = t('chat.aborted_badge');
        el.querySelector('.md').appendChild(badge);
      }
      toast(t('chat.aborted'), 'info');
    } else {
      const errorInfo = classifyError(msg);
      const el = get('currentAssistantEl');
      if (el) {
        renderErrorCard(el, errorInfo);
        el.querySelectorAll('.error-card-action').forEach((btn) => {
          btn.onclick = () => handleErrorAction(btn.dataset.action, el.dataset.query);
        });
      }
      toastError(`${errorInfo.title}：${errorInfo.reason}`);
      // 屏幕阅读器播报错误（REQ-A11Y-004 AC-4）
      announceError(`${errorInfo.title}：${errorInfo.reason}`);
      finalizeStream(false);
      setInputState('error', sanitizeError(msg));
    }
  });

  listen('chat_done', async (e) => {
    clearChatWatchdog();
    // P2-1：清除流状态
    _clearStreamState();
    // P1-3/P1-4/P0-3：斜杠命令与强制检索的临时开关在会话结束后统一恢复（V3.1 P4-4 状态入 store）
    const { tempWebSearch, tempAgent, regenForceSearch } = getState();
    if (tempWebSearch) {
      setState({ tempWebSearch: false });
      try { await invoke('update_setting', { key: 'rag.web_search_enabled', value: 'false' }); } catch (_e) { /* 静默 */ }
    }
    if (tempAgent) {
      setState({ tempAgent: false });
      try { await invoke('update_setting', { key: 'rag.agent_enabled', value: 'false' }); } catch (_e) { /* 静默 */ }
    }
    if (regenForceSearch) {
      setState({ regenForceSearch: false });
      try { await invoke('update_setting', { key: 'rag.hybrid_search', value: 'false' }); } catch (_e) { /* 静默 */ }
    }
    const usage = e.payload;
    const el = get('currentAssistantEl');
    if (el) {
      const mdEl = el.querySelector('.md');
      if (mdEl) await renderRichContent(mdEl);
    }
    // Bug #9 修复：中断场景以 finalizeStream(false) 处理，保留中断 badge
    const wasAborted = _chatAborted;
    _chatAborted = false;
    finalizeStream(!wasAborted);
    setInputState('idle');
    // 屏幕阅读器播报：回答完成（REQ-A11Y-004 AC-3）
    if (!wasAborted) {
      announceStatus(t('chat.sr_answer_complete', 'Answer complete'));
    }
    if (usage && usage.total_tokens) {
      const hint = $('inputHint');
      if (hint) {
        const badge = document.createElement('span');
        badge.className = 'text-slate-400';
        badge.innerHTML = `<svg class="icon-sm inline-block" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg> ${usage.prompt_tokens}↑ ${usage.completion_tokens}↓ = ${usage.total_tokens} tokens`;
        hint.replaceChildren(badge);
      }
      const totalTokens = usage.total_tokens || 0;
      const contextLimit = get('contextLimit') || 8000;
      updateContextUsage(totalTokens, contextLimit);
      const historyTokens = get('contextTokens') || 0;
      const cumulative = historyTokens + totalTokens;
      setState({ contextTokens: cumulative });
      const ctxBar = document.querySelector('.context-bar-container');
      if (ctxBar) {
        // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
        renderContextBar(ctxBar, cumulative, contextLimit);
        const level = getContextLevel(cumulative, contextLimit);
        if (level === 'red') {
          toast(t('chat.context_high_warning') || '上下文用量较高', 'warning');
        }
      }
    }
    if (get('auditingDocId')) {
      setState({ auditingDocId: null });
    } else {
      await onChatDone?.();
    }
  });

  // P2-2: 远程 LLM 连续失败 → 自动切换本地模型通知
  listen('llm_fallback', () => {
    toast(t('chat.llm_fallback') || '远程 LLM 连续失败，已切换到本地模型', 'warning');
  });

  listen('doc-status-changed', (e) => {
    const p = e.payload;
    if (p.status === 'indexing') {
      // 索引阶段：转发到进度条回调（不弹 toast，避免刷屏）
      onIndexingPhase?.(p);
    } else if (p.status === 'error') {
      toastError(p.message);
    } else if (p.status === 'done') {
      // 完成时静默（导入流程自行处理完成提示）
    } else {
      toast(p.message, 'info');
    }
    onDocStatusChanged?.();
  });

  listen('import-progress', (e) => {
    const p = e.payload;
    if (p.cancelled) {
      toast(t('import.import_cancelled'), 'info');
      onImportProgress?.(null);
      onDocStatusChanged?.();
      return;
    }
    onImportProgress?.(p);
  });

  listen('embedding_progress', (e) => {
    onEmbeddingProgress?.(e.payload);
  });

  listen('model_download_progress', (e) => {
    onModelDownload?.(e.payload);
  });

  listen('chat_context_truncated', (e) => {
    showCollapsedHint(e.payload);
  });

  listen('chat_context_compacted', (e) => {
    const { compacted_count, total_tokens, compacted_tokens, token_limit } = e.payload;
    if (compacted_count === 0) return;
    toast(`部分历史已压缩为摘要（${compacted_count} 条消息 → ${compacted_tokens} tokens，限制 ${token_limit}）`, 'info');
  });

  listen('sync_progress', (e) => {
    const p = e.payload;
    if (p.phase === 'complete') {
      toast(p.message, 'success');
    } else if (p.phase === 'error') {
      toast(p.message, 'error');
    }
  });

  // ============================================================
  // P2-2: 快捷指令面板 — 输入 `/` 触发
  // ============================================================

  const queryInput = $('queryInput');
  if (queryInput) {
    function checkSlashCommand() {
      const value = queryInput.value;
      const inputWrapper = queryInput.parentElement;
      if (!inputWrapper) return;

      if (value.startsWith('/') && !value.includes(' ')) {
        const filtered = filterSlashCommands(value);
        renderSlashCommandPanel(inputWrapper, filtered, (cmd) => {
          applySlashCommand(cmd, queryInput);
          removeSlashCommandPanel(inputWrapper);
        });
      } else {
        removeSlashCommandPanel(inputWrapper);
      }
    }

    function checkDocMention() {
      const value = queryInput.value;
      const cursorPos = queryInput.selectionStart;
      const inputWrapper = queryInput.parentElement;
      if (!inputWrapper) return;

      const beforeCursor = value.substring(0, cursorPos);
      const atMatch = beforeCursor.match(/@(\S*)$/);
      if (atMatch) {
        const query = atMatch[1];
        const docs = get('docList') || [];
        const filtered = filterDocuments(docs, query);
        renderDocMentionPopup(inputWrapper, filtered, (doc) => {
          insertDocMention(queryInput, doc.name);
          removeDocMentionPopup(inputWrapper);
        });
      } else {
        removeDocMentionPopup(inputWrapper);
      }
    }

    queryInput.addEventListener('input', () => {
      checkSlashCommand();
      checkDocMention();
    });


    document.addEventListener('click', (e) => {
      if (!queryInput.contains(e.target)) {
        const wrapper = queryInput.parentElement;
        if (wrapper) {
          removeSlashCommandPanel(wrapper);
          removeDocMentionPopup(wrapper);
        }
      }
    });
  }
}

// V3.1 P4-2：向 chat-render 注入发送动作（解除双向循环依赖——
// chat-render 不再 import chat，依赖方向变为 chat → chat-render 单向 + 运行时注册）
registerChatActions({ send, sendFromEdit });
