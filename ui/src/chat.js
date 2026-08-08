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
import { runGuard, canSend, requireIdle, requirePro } from './guards.js';
import { removeCaret } from './caret.js';
import { initScrollLock } from './scroll-lock.js';
import { classifyError, renderErrorCard } from './error-detail.js';
import { updateContextUsage, renderContextBar, getContextLevel } from './context-bar.js';
import { filterSlashCommands, renderSlashCommandPanel, navigateSlashCommand, getSelectedSlashCommand, applySlashCommand, removeSlashCommandPanel, processSlashCommand } from './slash-commands.js';
import { extractDocMentions, getDocFilter, renderDocMentionPopup, insertDocMention, removeDocMentionPopup, filterDocuments } from './doc-mention.js';
import { navigateCarousel, updateCarouselDisplay, addCarouselVersion } from './regen-carousel.js';
import { createFeedbackButtons } from './feedback.js';
import { enqueueQuery, processQueue, updateSendButton } from './queue-send.js';
import { renderFollowupSuggestions } from './followup.js';
import { buildHistoryFromTurns, updateVersionAssistant } from './turn-tree.js';
import { renderBranchPagination, removeBranchPagination } from './message-edit.js';
import { detectAndReportImplicit, resetFeedbackTracking } from './retrieval-feedback.js';

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
 * @param {'idle'|'streaming'|'error'} state - 目标状态
 * @param {string} [hint=''] - 状态提示文案
 */
export function setInputState(state, hint = '') {
  const isStreaming = state === 'streaming';
  setState({ streaming: isStreaming });
  const docCount = get('docCount');
  const shouldDisable = isStreaming || docCount === 0;
  // 禁聊：流式生成期间输入框完全禁用（无法输入新问题，需等回答完成）
  $('queryInput').disabled = shouldDisable;
  if (isStreaming) {
    updateSendButton();
  } else {
    $('sendBtn').disabled = shouldDisable;
    updateSendButton();
  }
  $('sendBtn').classList.toggle('opacity-30', shouldDisable && !isStreaming);
  $('sendBtn').classList.toggle('cursor-not-allowed', shouldDisable && !isStreaming);
  $('inputHint').textContent = hint;
  if (!isStreaming && docCount > 0) $('queryInput').focus();
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
export async function send() {
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
    input.value = '';
    input.style.height = '48px';
    // S56：快捷指令展开 — /command query → promptTemplate.replace('{query}', query)
    const slashResult = processSlashCommand(query);
    if (slashResult.matched) {
      query = slashResult.text;
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
  const { onChatDone, onDocStatusChanged, onImportProgress, onEmbeddingProgress, onModelDownload } = callbacks;

  const chatArea = $('chatArea');
  if (chatArea) {
    initScrollLock(chatArea);
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
    if (el) {
      if (el._thinkingPanel) {
        // 需求 5：传 phase 触发图标流转（preparing→时钟 / retrieving→放大镜 / generating→星星）
        el._thinkingPanel.update(p.message, p.phase);
        el._thinkingPanel.appendStage(p.message);
      }
      const textEl = el.querySelector('.thinking-text');
      if (textEl) textEl.textContent = p.message;
    }
    $('inputHint').textContent = p.message;
    resetChatWatchdog();
  });

  listen('chat_reasoning', (e) => {
    const el = get('currentAssistantEl');
    if (el && el._thinkingPanel) {
      el._thinkingPanel.appendReasoning(String(e.payload ?? ''));
    }
  });

  listen('chat_token', (e) => {
    const el = get('currentAssistantEl');
    if (el) {
      if (el._thinkingPanel) {
        // 标记思考完成，但不强制折叠（用户可能已展开面板实时查看推理过程）
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
      finalizeStream(false);
      setInputState('error', sanitizeError(msg));
    }
  });

  listen('chat_done', async (e) => {
    clearChatWatchdog();
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
    if (usage && usage.total_tokens) {
      const hint = $('inputHint');
      if (hint) {
        const badge = document.createElement('span');
        badge.className = 'text-slate-400';
        badge.textContent = `📊 ${usage.prompt_tokens}↑ ${usage.completion_tokens}↓ = ${usage.total_tokens} tokens`;
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

  listen('doc-status-changed', (e) => {
    const p = e.payload;
    if (p.status === 'error') {
      toastError(p.message);
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

    queryInput.addEventListener('keydown', (e) => {
      const panel = document.querySelector('.slash-command-panel');
      if (panel) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          const filtered = filterSlashCommands(queryInput.value);
          navigateSlashCommand(filtered, 'down');
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          const filtered = filterSlashCommands(queryInput.value);
          navigateSlashCommand(filtered, 'up');
        } else if (e.key === 'Enter' || e.key === 'Tab') {
          const filtered = filterSlashCommands(queryInput.value);
          const selected = getSelectedSlashCommand(filtered);
          if (selected) {
            e.preventDefault();
            applySlashCommand(selected, queryInput);
            removeSlashCommandPanel(queryInput.parentElement);
          }
        } else if (e.key === 'Escape') {
          removeSlashCommandPanel(queryInput.parentElement);
        }
      }

      if ((e.metaKey || e.ctrlKey) && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
        const carousel = document.querySelector('.regen-carousel');
        if (carousel) {
          e.preventDefault();
          // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
          navigateCarousel(carousel, e.key === 'ArrowLeft' ? 'left' : 'right');
          const blockEl = carousel.closest('.msg-block');
          if (blockEl) {
            const mdEl = blockEl.querySelector('.md');
            // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
            if (mdEl) updateCarouselDisplay(carousel, mdEl);
          }
        }
      }
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
