/**
 * EchoMind 前端入口 — 模块编排 + 初始化 + 启动流程。
 *
 * 这是整个前端的「胶水层」：
 * 1. 从各功能模块导入函数
 * 2. 注册跨模块事件回调
 * 3. 绑定 DOM 事件
 * 4. 执行启动引导流程
 *
 * 从 index.html 的 <script type="module" src="src/main.js"> 加载。
 */

// ============================================================
// 模块导入
// ============================================================

import { getState, setState, get, subscribe } from './state.js';
import { $, WORKSPACE, isInputFocused, makeKeyboardClickable } from './utils.js';
import { invoke, listen, openDialog } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { resetChatArea, appendBlock, appendUserBlock, showApp, showWizard, scrollToBottom,
        scheduleRender, renderSources, showMsgActions, showCollapsedHint,
        hideEmptyState, updateModelPill, appendAiDisclaimer } from './chat-render.js';
import { send, onStop, auditDocument, setInputState, finalizeStream,
        initChatEventListeners, checkStreamResume } from './chat.js';
import { resetFeedbackTracking } from './feedback.js';
import { renderMarkdown, renderRichContent } from './markdown.js';
import { initSidebar, toggleSidebar, openSearchPopup, closeSearchPopup, initSearchPopup, filterDocuments, isMultiSelectMode, setReloadDocsCallback, setFilterChangeCallback, restoreSidebarState } from './sidebar.js';
import { initCommandPalette, openCommandPalette, closeCommandPalette, initGlobalSearch, openGlobalSearch, closeGlobalSearch } from './search-ui.js';
import { initKeyboardShortcuts } from './keyboard.js';
import { initKeyboardHelp } from './help-panel.js';
import { initWizard, renderPresetCards, applyPreset, startWizard, showWizardStep } from './wizard.js';
import { demoModeApi } from './ipc.js';
import { initSettings, openSettings, closeSettings, initEmbedder, clearModelCache } from './settings.js';
import { initPaywall, showPaywall, hidePaywall, activatePro, deactivatePro, updateProStatus } from './wizard.js';
import { initDragDrop, initFilePicker, initImportCancel, importPaths,
         showImportProgress, hideImportProgress } from './import.js';
import { initI18n, t } from './i18n.js';
import { setMermaidInitFn } from './lazy-loader.js';
import { initConvNav } from './conv-nav.js';
import { initContextMenu } from './context-menu.js';
// doc-keyboard + breadcrumb merged into doc-nav.js
import { initSecurity, getSecurityCommands } from './security.js';
import { createInputToggle } from './input-toggles.js';
import { updateInputUI } from './action.js';
import { navigateHistoryUp, navigateHistoryDown, resetHistoryNav, saveDraft, restoreDraft, clearDraft, updateTokenEstimate } from './input-utils.js';
import { clearQueue, getQueueSize } from './chat-utils.js';
import { initDownloadManager } from './download-manager.js';
import { openGraphViewer, closeGraphViewer, initGraphResize } from './graph-viewer.js';
import { buildTurnTree, setTurnTree, getTurn, applyActiveVersions, getVersionCount, openConversationTreePanel, closeConversationTreePanel } from './turn-tree.js';
import { renderBranchPagination } from './message-edit.js';
import { openDreamPanel, closeDreamPanel, initDreamPanel } from './dream-panel.js';
import { openSymbolSearch, closeSymbolSearch, initSymbolSearch } from './symbol-search.js';
import { initVoiceInput, stopAllTTS } from './voice-input.js';
import { initExportButtons, exportConversationToHtml, exportDocumentToHtml, exportConversationToPdf } from './export.js';
import { initErrorBoundary, initOfflineDetector } from './network-utils.js';
import { initBreadcrumb, updateBreadcrumb, clearBreadcrumb, initDocKeyboard } from './doc-nav.js';
import { openKbStats, closeKbStats, openDocPreview, closeDocPreview } from './doc-panels.js';
import { initBookmarks, refreshBookmarks, createBookmarkButton, createBookmarkNavigator } from './bookmarks.js';
import { showSkeleton, hideSkeleton } from './utils.js';
import { openHelpPanel, closeHelpPanel, openAboutPanel, closeAboutPanel, initAboutPanel } from './help-panel.js';
import { formatDate, formatRelativeTime, formatFileSize, formatNumber } from './utils.js';
import { loadCustomTemplates, loadSkills } from './slash-commands.js';
import { initUpdateBanner } from './help-panel.js';
import { convApi } from './ipc.js';
import { createInputKeyHandler } from './input-utils.js';
import { initWorkspaceSelector, getCurrentWorkspaceId, setSwitchWorkspaceCallback } from './workspace.js';
import { icon, fileIcon } from './utils.js';

// ============================================================
// 聊天输入框状态控制
// ============================================================

/**
 * 根据知识库文档数和流式状态更新聊天输入框和发送按钮的禁用状态。
 * 直接委托 updateInputUI（action.js 集中守卫）。
 */
function updateChatInputState() {
  updateInputUI();
}

// ============================================================
// 面板管理
// ============================================================

/**
 * 会话切换时关闭所有打开的 overlay 面板（方案5）。
 * 注意：不关闭 lock-overlay（安全相关）、confirm-dialog（同步 Promise）、dbError Modal（手动确认）
 */
function closeAllPanels() {
  // 关闭所有动态面板（通过 panel-stack）
  closeSettings();
  closeGraphViewer();
  closeDreamPanel();
  closeSymbolSearch();
  closeSearchPopup();
  closeCommandPalette();
  closeGlobalSearch();
  hidePaywall();
  closeConversationTreePanel();
}

// ============================================================
// 会话生命周期
// ============================================================

/**
 * 新建会话（懒创建）：不立即调用 create_conversation，而是前端生成 UUID。
 * 后端 chat 命令有兜底机制（会话不存在时幂等创建），首次发送消息时自动落库。
 * 如果已经在新建会话状态（isNewConversation=true），不做任何操作。
 *
 * 流式期间：自动中断当前流（保存已生成内容），然后继续新建会话。
 */
/**
 * 更新窗口标题为当前会话标题（窗口标题动态显示）。
 * @param {string} [convTitle] - 会话标题；空则使用默认标题
 */
function _updateWindowTitle(convTitle) {
  const appName = 'EchoMind';
  document.title = convTitle ? `${convTitle} — ${appName}` : appName;
}

async function newChat() {
  if (get('streaming')) {
    await onStop();
    // 等待 finalizeStream 完成状态清理
    await new Promise((r) => setTimeout(r, 100));
  }
  // 方案5：会话切换时关闭所有面板
  closeAllPanels();
  // 停止所有 TTS 朗读（防止跨对话朗读残留，REQ-RAG-035）
  stopAllTTS();
  // 保存当前会话草稿 + 清空排队队列
  saveDraft();
  clearQueue();
// 已经在新建会话状态，不做任何操作
if (get('isNewConversation')) { restoreDraft(); updateInputUI(); return; }
  // 前端生成 UUID，不创建 DB 记录
  const id = crypto.randomUUID();
  setState({ currentConversationId: id, history: [], isNewConversation: true });
  resetHistoryNav();
  resetChatArea(t('chat.placeholder_title'), t('chat.placeholder_desc'));
  _updateWindowTitle(); // 窗口标题重置为默认
  resetFeedbackTracking(); // REQ-PERF-012：新对话重置反馈跟踪
  await loadConversations();
  $('queryInput').focus();
}

/**
 * 加载历史会话：拉取消息列表，逐条渲染 user/assistant Block。
 * REQ-RAG-012：加载时为每条消息渲染操作栏，assistant 消息存储对应 query。
 *
 * 流式期间：自动中断当前流（保存已生成内容），然后继续切换会话。
 * @param {string} id - 会话 ID
 */
async function loadConversation(id, searchQuery) {
  if (get('streaming')) {
    await onStop();
    // 等待 finalizeStream 完成状态清理
    await new Promise((r) => setTimeout(r, 100));
  }
  // 方案5：会话切换时关闭所有面板
  closeAllPanels();
  // 保存当前会话草稿 + 清空排队队列
  saveDraft();
  clearQueue();
  const messages = await invoke('get_messages', { conversationId: id });
  // 重建轮次版本树（DB 持久化的编辑版本）
  const turns = buildTurnTree(messages);
  setTurnTree(turns);
  // 加载 DB 中持久化的活跃版本号，应用到轮次树
  try {
    const activeVersions = await convApi.getTurnActiveVersions(id);
    applyActiveVersions(activeVersions);
  } catch (err) {
    console.warn('get_turn_active_versions IPC 失败:', err);
  }
  setState({ currentConversationId: id, history: messages, isNewConversation: false });
  // 窗口标题动态显示：查找会话标题
  const convList = await invoke('get_conversations', { workspaceId: getCurrentWorkspaceId() });
  const conv = convList.find(c => c.id === id);
  _updateWindowTitle(conv?.title);
  resetFeedbackTracking(); // REQ-PERF-012：切换会话重置反馈跟踪
  $('chatArea').innerHTML = '';
  if (messages.length === 0) {
    resetChatArea(t('chat.empty_messages_title'), t('chat.empty_messages_desc'));
  }
  // 按 turn_group 分组渲染：每个 turn 只渲染活跃版本，但保留分页器
  const renderedTurnGroups = new Set();
  let lastUserMsg = '';
  for (const m of messages) {
    // 跳过同一 turn_group 的非活跃版本（避免重复渲染）
    if (m.turn_group) {
      if (renderedTurnGroups.has(m.turn_group)) continue;
      renderedTurnGroups.add(m.turn_group);
      const turn = getTurn(m.turn_group);
      if (turn) {
        const ver = turn.versions.find(v => v.version === turn.activeVersion);
        if (ver) {
          lastUserMsg = ver.userContent;
          const userBlock = appendUserBlock(ver.userContent, ver.userMessageId || null);
          if (userBlock) {
            userBlock.dataset.turnGroup = m.turn_group;
            userBlock.dataset.version = String(turn.activeVersion);
          }
          if (ver.assistantContent) {
            const el = appendBlock('assistant');
            el.dataset.query = ver.userContent;
            // 设置 msgId 用于思考面板状态持久化（使用用户消息 ID 作为唯一标识）
            const thinkingMsgId = ver.userMessageId || m.id || null;
            el.dataset.msgId = thinkingMsgId || '';
            setState({ currentAssistantEl: el });
            if (el._thinkingPanel) {
              el._thinkingPanel.setComplete();
              if (ver.reasoning) {
                el._thinkingPanel.renderReasoning(ver.reasoning);
              }
              // 恢复持久化的展开/折叠状态（按消息 ID）
              el._thinkingPanel.setMsgId(thinkingMsgId);
            }
            const thinking = el.querySelector('.thinking-indicator');
            if (thinking) thinking.remove();
            el.querySelector('.md').classList.remove('hidden');
            setState({ currentRawMarkdown: ver.assistantContent });
            if (ver.sources && ver.sources.length > 0) renderSources(ver.sources);
            const mdEl = el.querySelector('.md');
            // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
            renderMarkdown(mdEl, get('currentRawMarkdown'), get('lastSources'));
            // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
            await renderRichContent(mdEl);
            showMsgActions(el, ver.assistantContent, 'assistant');
            appendAiDisclaimer(el);
            // 如果有多个版本，渲染分页器（挂在用户操作栏中，与复制/编辑按钮同行）
            if (turn.versions.length > 1) {
              renderBranchPagination(userBlock, m.turn_group);
            }
          }
          continue;
        }
      }
    }
    // 无 turn_group 的消息：正常渲染
    if (m.role === 'user') {
      lastUserMsg = m.content;
      appendUserBlock(m.content, m.id || null);
    } else {
      const el = appendBlock('assistant');
      el.dataset.query = lastUserMsg;
      // 设置 msgId 用于思考面板状态持久化（使用用户消息 ID 作为唯一标识）
      const lastUserBlock = document.querySelector('.msg-user:last-child');
      const thinkingMsgId = lastUserBlock?.dataset.msgId || m.id || null;
      el.dataset.msgId = thinkingMsgId || '';
      setState({ currentAssistantEl: el });
      if (el._thinkingPanel) {
        el._thinkingPanel.setComplete();
        if (m.reasoning) {
          el._thinkingPanel.renderReasoning(m.reasoning);
        }
        // 恢复持久化的展开/折叠状态（按消息 ID）
        el._thinkingPanel.setMsgId(thinkingMsgId);
      }
      const thinking = el.querySelector('.thinking-indicator');
      if (thinking) thinking.remove();
      el.querySelector('.md').classList.remove('hidden');
      setState({ currentRawMarkdown: m.content });
      if (m.sources && m.sources.length > 0) renderSources(m.sources);
      const mdEl = el.querySelector('.md');
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      renderMarkdown(mdEl, get('currentRawMarkdown'), get('lastSources'));
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      await renderRichContent(mdEl);
      showMsgActions(el, m.content, 'assistant');
      appendAiDisclaimer(el);
    }
  }
  setState({ currentAssistantEl: null, currentRawMarkdown: '' });
  // 恢复草稿 + 重置历史导航
  resetHistoryNav();
  restoreDraft();
  updateInputUI();
  scrollToBottom();
  await loadConversations();
  // 搜索跳转高亮：如果从搜索结果进入，滚动到匹配消息并高亮
  if (searchQuery) {
    setTimeout(() => highlightSearchMatch(searchQuery), 200);
  }
}

/**
 * 高亮搜索匹配文本：在聊天区域中查找包含搜索词的消息块，滚动到该位置并高亮。
 * @param {string} query - 搜索关键词
 */
function highlightSearchMatch(query) {
  if (!query || !query.trim()) return;
  const term = query.trim().toLowerCase();
  const chatArea = $('chatArea');
  if (!chatArea) return;
  // 遍历所有消息块，查找包含搜索词的文本
  const blocks = chatArea.querySelectorAll('.msg-block');
  for (const block of blocks) {
    const text = (block.textContent || '').toLowerCase();
    if (text.includes(term)) {
      // 滚动到匹配的消息块
      block.scrollIntoView({ block: 'center', behavior: 'smooth' });
      // 添加高亮动画类
      block.classList.add('search-highlight-flash');
      // 3 秒后移除高亮类
      setTimeout(() => {
        block.classList.remove('search-highlight-flash');
      }, 3000);
      return;
    }
  }
}

/**
 * 删除会话并级联清理消息；若删的是当前会话则自动新建。
 * @param {string} id - 会话 ID
 */
async function removeConversation(id) {
  await invoke('delete_conversation', { id });
  if (id === get('currentConversationId')) await newChat();
  else await loadConversations();
}

/**
 * 计算会话所属时间分组的标签 key。
 *
 * 分组规则（按时间从近到远）：
 *   今天 → 昨天 → 一周内 → 一个月内 → 按月份（YYYY-MM）
 *
 * @param {number} createdAt - Unix 秒级时间戳
 * @returns {string} i18n key 前缀（如 'time.today'）或月份字符串
 */
function getConvTimeBucket(createdAt) {
  const now = new Date();
  const conv = new Date(createdAt * 1000);

  // 今天：同一天
  if (now.getFullYear() === conv.getFullYear() &&
      now.getMonth() === conv.getMonth() &&
      now.getDate() === conv.getDate()) {
    return 'time.today';
  }

  // 昨天：当前日期 -1
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (yesterday.getFullYear() === conv.getFullYear() &&
      yesterday.getMonth() === conv.getMonth() &&
      yesterday.getDate() === conv.getDate()) {
    return 'time.yesterday';
  }

  // 前天：当前日期 -2
  const dayBefore = new Date(now);
  dayBefore.setDate(now.getDate() - 2);
  if (dayBefore.getFullYear() === conv.getFullYear() &&
      dayBefore.getMonth() === conv.getMonth() &&
      dayBefore.getDate() === conv.getDate()) {
    return 'time.day_before';
  }

  // 一周内：7 天内（但不是今天/昨天/前天）
  const weekAgo = new Date(now);
  weekAgo.setDate(now.getDate() - 7);
  if (conv >= weekAgo) {
    return 'time.within_week';
  }

  // 一个月内：30 天内（但不在一周内）
  const monthAgo = new Date(now);
  monthAgo.setDate(now.getDate() - 30);
  if (conv >= monthAgo) {
    return 'time.within_month';
  }

  // 更早：按月份分组（YYYY-MM）
  const year = conv.getFullYear();
  const month = String(conv.getMonth() + 1).padStart(2, '0');
  return `month:${year}-${month}`;
}

/**
 * 获取时间分组的显示文案。
 * @param {string} bucket - getConvTimeBucket 返回的分组标识
 * @returns {string} 本地化的分组标签
 */
function getConvBucketLabel(bucket) {
  if (bucket.startsWith('month:')) {
    // 月份格式：从 i18n 模板渲染
    const ym = bucket.slice(6); // "2026-06"
    const [year, month] = ym.split('-');
    return t('time.month_format', { year, month });
  }
  return t(bucket);
}

/**
 * 拉取会话列表并渲染到侧栏，按时间分组（今天/昨天/一周内/一个月内/月份），
 * 高亮当前会话。支持搜索框过滤。支持拖拽排序（REQ-IX-002）。
 */

/** 拖拽中的会话 ID（REQ-IX-002） */
let _draggedConvId = null;

/**
 * 收集 #convList 中所有会话 ID 的当前 DOM 顺序。
 * 跳过分组头元素（仅收集 [data-conv-id] 的元素）。
 * @returns {string[]}
 */
function _collectConvOrder() {
  const box = $('convList');
  if (!box) return [];
  return Array.from(box.querySelectorAll('[data-conv-id]'))
    .map(el => el.dataset.convId)
    .filter(Boolean);
}

/**
 * 拖拽排序后持久化并重新渲染（REQ-IX-002 AC-4）。
 * 收集 DOM 中的新顺序，调用 IPC 持久化，然后重新加载列表。
 */
async function _persistConvReorder() {
  const orderedIds = _collectConvOrder();
  if (orderedIds.length === 0) return;
  try {
    await invoke('reorder_conversations', { orderedIds });
  } catch (_) {
    // IPC 失败时静默降级（E2E 测试环境）
  }
  // 不重新加载列表 — DOM 已是用户期望的顺序，避免闪烁
}

/**
 * 为会话列表项绑定拖拽事件（REQ-IX-002）。
 * @param {HTMLElement} item - 会话列表项 DOM 元素
 */
function _bindDragEvents(item) {
  item.draggable = true;

  item.addEventListener('dragstart', (e) => {
    _draggedConvId = item.dataset.convId;
    item.classList.add('opacity-50');
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', _draggedConvId);
    }
  });

  item.addEventListener('dragover', (e) => {
    e.preventDefault();
    if (!_draggedConvId || _draggedConvId === item.dataset.convId) return;
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
    // 计算插入位置：鼠标 Y < 项中点 → 上方，否则下方
    const rect = item.getBoundingClientRect();
    const isAbove = e.clientY < rect.top + rect.height / 2;
    // 移除其他项的插入线
    document.querySelectorAll('#convList .conv-drop-indicator').forEach(el => el.remove());
    // 添加插入线
    const indicator = document.createElement('div');
    indicator.className = 'conv-drop-indicator h-0.5 bg-accent mx-2 rounded-full';
    if (isAbove) {
      item.parentNode.insertBefore(indicator, item);
    } else {
      item.parentNode.insertBefore(indicator, item.nextSibling);
    }
  });

  item.addEventListener('drop', (e) => {
    e.preventDefault();
    if (!_draggedConvId || _draggedConvId === item.dataset.convId) return;
    // 移除插入线
    document.querySelectorAll('#convList .conv-drop-indicator').forEach(el => el.remove());
    // DOM 移动：将拖拽项移动到目标位置
    const draggedEl = document.querySelector(`#convList [data-conv-id="${_draggedConvId}"]`);
    if (!draggedEl || draggedEl === item) return;
    const rect = item.getBoundingClientRect();
    const isAbove = e.clientY < rect.top + rect.height / 2;
    if (isAbove) {
      item.parentNode.insertBefore(draggedEl, item);
    } else {
      item.parentNode.insertBefore(draggedEl, item.nextSibling);
    }
    // 持久化新顺序
    _persistConvReorder();
  });

  item.addEventListener('dragend', () => {
    item.classList.remove('opacity-50');
    document.querySelectorAll('#convList .conv-drop-indicator').forEach(el => el.remove());
    _draggedConvId = null;
  });
}

// ============================================================
// 字符计数器（P1-2：DeepSeek 风格输入字符计数 + 超限拦截）
// ============================================================

/** 最大输入字符数（超过此值阻止发送） */
const MAX_INPUT_CHARS = 32000;

/**
 * 更新字符计数器显示。
 * @param {string} text - 当前输入文本
 */
function _updateCharCounter(text) {
  const counter = $('charCounter');
  if (!counter) return;
  const len = text.length;
  if (len === 0) {
    counter.classList.add('hidden');
    return;
  }
  counter.classList.remove('hidden');
  counter.textContent = `${len.toLocaleString()} / ${MAX_INPUT_CHARS.toLocaleString()}`;
  // 接近上限时变色警示
  if (len > MAX_INPUT_CHARS * 0.9) {
    counter.className = 'text-[11px] text-amber-400 ml-auto';
  } else if (len > MAX_INPUT_CHARS) {
    counter.className = 'text-[11px] text-red-400 ml-auto font-medium';
  } else {
    counter.className = 'text-[11px] text-text-quaternary ml-auto';
  }
  // 超限拦截：禁用发送按钮
  const sendBtn = $('sendBtn');
  if (sendBtn && !get('streaming')) {
    sendBtn.disabled = len > MAX_INPUT_CHARS;
  }
}

/** 会话列表初始渲染限制（P2-3：性能优化，避免大量会话一次性渲染） */
const CONV_INITIAL_RENDER_LIMIT = 50;
/** doc-status-changed 全量列表刷新合流窗口（V3.1 P2-2） */
const DOC_REFRESH_DEBOUNCE_MS = 500;

// V3.1 P3-5：统一书签/搜索跳转回调（原 4 处内联复制 → 单一实现）。
// 注意：须在首次使用前定义（loadConversations/_renderConvItem 均会引用）。
const _navigateToMessage = createBookmarkNavigator({ loadConversation });

async function loadConversations() {
const convBox = $('convList');
if (convBox) showSkeleton(convBox, 'conv', 4);
const list = await invoke('get_conversations', { workspaceId: getCurrentWorkspaceId() });
// 暴露会话列表供书签模块查询标题（REQ-RAG-047）
window.__echomindConversations = list;
// 刷新书签面板（会话标题可能变化）— 兼容消息级书签跳转（V3.1 P3-5 统一导航器）
refreshBookmarks({ onNavigate: (convId, messageId) => _navigateToMessage(convId, messageId) });
const box = $('convList');
if (box) hideSkeleton(box);
  box.innerHTML = '';

  if (list.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'px-3 py-4 text-center text-[11px] text-text-quaternary';
    empty.textContent = t('chat.empty_messages_desc');
    box.appendChild(empty);
    return;
  }

  // 新建会话不显示在列表中（尚未持久化）
  const filtered = get('isNewConversation')
    ? list.filter(c => c.id !== get('currentConversationId'))
    : list;

  // 置顶会话（从 localStorage 读取）
  const pinnedIds = _getPinnedConvIds();
  const pinnedConvs = filtered.filter(c => pinnedIds.has(c.id));
  const normalConvs = filtered.filter(c => !pinnedIds.has(c.id));

  // 渲染置顶分组（如果有置顶会话）
  if (pinnedConvs.length > 0) {
    const header = document.createElement('div');
    header.className = 'conv-group-header';
    header.textContent = t('chat.pinned') || '置顶';
    box.appendChild(header);

    for (const c of pinnedConvs) {
      _renderConvItem(box, c, get('currentConversationId'), true);
    }
  }

  // 按时间分组（保持倒序，因为后端已按 created_at 倒序返回）
  const groups = new Map();
  for (const c of normalConvs) {
    const bucket = getConvTimeBucket(c.created_at);
    if (!groups.has(bucket)) groups.set(bucket, []);
    groups.get(bucket).push(c);
  }

  // V3.1 P2-4：分批渲染 — 超过 CONV_INITIAL_RENDER_LIMIT 时先渲染前 N 条，
  // 追加「加载更多」按钮续渲（死常量接线；百级会话用户切换侧栏不再整列重建）
  let renderedCount = pinnedConvs.length;
  const remainingGroups = [];

  // 渲染每个分组
  for (const [bucket, convs] of groups) {
    if (renderedCount >= CONV_INITIAL_RENDER_LIMIT) {
      remainingGroups.push([bucket, convs]);
      continue;
    }
    // 分组头（sticky）
    const header = document.createElement('div');
    header.className = 'conv-group-header';
    header.textContent = getConvBucketLabel(bucket);
    box.appendChild(header);

    // 分组下的会话条目
    for (const c of convs) {
      if (renderedCount >= CONV_INITIAL_RENDER_LIMIT) {
        remainingGroups.push([null, [c]]);
        continue;
      }
      _renderConvItem(box, c, get('currentConversationId'), false);
      renderedCount++;
    }
  }

  // 「加载更多」：逐批渲染剩余会话（每次点击 +50）
  if (remainingGroups.length > 0) {
    const hiddenCount = remainingGroups.reduce((sum, [, cs]) => sum + cs.length, 0);
    const moreBtn = document.createElement('button');
    moreBtn.id = 'convLoadMoreBtn';
    moreBtn.type = 'button';
    moreBtn.className = 'w-full px-3 py-2 text-center text-[11px] text-text-quaternary hover:text-text-secondary hover:bg-surface-2 transition-colors cursor-pointer border-none bg-transparent';
    moreBtn.textContent = t('chat.load_more', { count: hiddenCount }) || `加载更多（${hiddenCount}）`;
    moreBtn.onclick = () => {
      // 续渲：把剩余项按原顺序补到列表尾部（分组头仅在批次起点重建）
      let budget = CONV_INITIAL_RENDER_LIMIT;
      let i = 0;
      while (i < remainingGroups.length && budget > 0) {
        const [bucket, cs] = remainingGroups[i];
        if (bucket !== null) {
          const header = document.createElement('div');
          header.className = 'conv-group-header';
          header.textContent = getConvBucketLabel(bucket);
          box.appendChild(header);
        }
        for (const c of cs) {
          if (budget <= 0) break;
          _renderConvItem(box, c, get('currentConversationId'), false);
          budget--;
        }
        i++;
      }
      remainingGroups.splice(0, i);
      const left = remainingGroups.reduce((sum, [, cs]) => sum + cs.length, 0);
      if (left > 0) {
        moreBtn.textContent = t('chat.load_more', { count: left }) || `加载更多（${left}）`;
      } else {
        moreBtn.remove();
      }
    };
    box.appendChild(moreBtn);
  }
}

// ============================================================
// 会话置顶功能（P0-4：DeepSeek 风格会话置顶）
// ============================================================

/** localStorage 存储键：置顶会话 ID 列表 */
const PINNED_CONVS_KEY = 'echomind_pinned_conversations';

/**
 * 获取置顶会话 ID 集合。
 * @returns {Set<string>}
 */
function _getPinnedConvIds() {
  try {
    const raw = localStorage.getItem(PINNED_CONVS_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    return new Set(Array.isArray(arr) ? arr : []);
  } catch (_) { return new Set(); }
}

/**
 * 切换会话置顶状态。
 * @param {string} convId - 会话 ID
 */
function _togglePinConversation(convId) {
  const pinned = _getPinnedConvIds();
  if (pinned.has(convId)) {
    pinned.delete(convId);
  } else {
    pinned.add(convId);
  }
  try {
    localStorage.setItem(PINNED_CONVS_KEY, JSON.stringify([...pinned]));
  } catch (_) { /* 隐私模式 */ }
}

/**
 * 渲染单个会话条目（复用逻辑：置顶和普通分组共用）。
 * @param {HTMLElement} box - 容器
 * @param {object} c - 会话对象
 * @param {string} currentConvId - 当前会话 ID
 * @param {boolean} isPinned - 是否置顶
 */
function _renderConvItem(box, c, currentConvId, isPinned) {
  const item = document.createElement('div');
  const active = c.id === currentConvId;
  item.className = `group flex items-center justify-between px-3 py-2.5 rounded-xl text-sm cursor-pointer ${active ? 'bg-accent/15 text-accent' : 'text-slate-300 hover:bg-white/5'}`;
  item.dataset.convId = c.id;
  item.dataset.convTitle = c.title || '';
  item.setAttribute('role', 'listitem');
  item.setAttribute('aria-label', c.title || t('backend.placeholder_title'));
  const title = document.createElement('span');
  title.className = 'truncate flex-1';
  title.textContent = c.title;
  // 置顶图标（固定指示器）
  if (isPinned) {
    const pinIcon = document.createElement('span');
    pinIcon.className = 'shrink-0 text-text-quaternary mr-1';
    pinIcon.innerHTML = '<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-8H8a2 2 0 0 0 0 8h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/></svg>';
    title.insertBefore(pinIcon, title.firstChild);
  }
  // 置顶/取消置顶按钮
  const pinBtn = document.createElement('button');
  pinBtn.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent ml-1 shrink-0 px-1';
  pinBtn.title = isPinned ? (t('chat.unpin') || '取消置顶') : (t('chat.pin') || '置顶');
  pinBtn.setAttribute('aria-label', isPinned ? (t('chat.unpin') || '取消置顶') : (t('chat.pin') || '置顶'));
  pinBtn.innerHTML = isPinned
    ? '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="17" x2="12" y2="22"/><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-8H8a2 2 0 0 0 0 8h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/></svg>'
    : '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-8H8a2 2 0 0 0 0 8h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"/><line x1="12" y1="17" x2="12" y2="22"/></svg>';
  pinBtn.onclick = async (ev) => {
    ev.stopPropagation();
    _togglePinConversation(c.id);
    await loadConversations();
  };
  const del = document.createElement('button');
  del.className = 'invisible group-hover:visible text-text-quaternary hover:text-red-400 ml-2 shrink-0 px-1';
  del.innerHTML = icon('close', 'sm');
  del.title = t('chat.delete_conversation');
  del.setAttribute('aria-label', t('chat.delete_conversation'));
  del.onclick = async (ev) => { ev.stopPropagation(); await removeConversation(c.id); };
  item.appendChild(title);
  // 书签按钮（REQ-RAG-047）
  const bookmarkBtn = createBookmarkButton(c.id, {
    onToggle: () => { refreshBookmarks({ onNavigate: (convId, messageId) => _navigateToMessage(convId, messageId) }); },
  });
  item.appendChild(bookmarkBtn);
  item.appendChild(pinBtn);
  item.appendChild(del);
  item.onclick = () => loadConversation(c.id);
  // V3.1 P3-6：键盘可达（Enter/Space 触发）
  makeKeyboardClickable(item);
  // 拖拽排序（REQ-IX-002）
  _bindDragEvents(item);
  box.appendChild(item);
}

// ============================================================
// 知识库面板 — 全面重新设计（分页 + 无限滚动 + 模糊搜索）
// ============================================================

import { displayDocName, docStatusOf, DOC_STATUS_STYLE, formatBytes, getSubPhaseLabel } from './utils.js';

/** 每页渲染数量 */
const KB_PAGE_SIZE = 20;

/** 缓存的全部文档（从后端一次性加载，前端分页渲染） */
let _kbAllDocs = [];

/** 筛选后的文档列表 */
let _kbFilteredDocs = [];

/** 已渲染数量 */
let _kbRenderedCount = 0;

/** IntersectionObserver 实例 */
let _kbObserver = null;

/** 文件类型图标 — 使用 SVG inline 图标（REQ-DS-004）替代 Emoji */
function getFileIcon(ext) {
  return fileIcon(ext, 'sm');
}

/** 状态颜色映射（圆点指示器） */
const STATUS_DOT_COLORS = {
  Indexed: 'bg-emerald-400',
  Processing: 'bg-amber-400 animate-pulse',
  Pending: 'bg-slate-400',
  Failed: 'bg-red-400',
};

/**
 * 加载知识库文档列表（从后端获取全部，前端分页渲染）。
 * 同步更新配额计数，然后调用 applyKbFilters() 进行筛选和渲染。
 * REQ-ING-008 v1.10：支持排序参数传递。
 */
let _kbSortBy = null;
let _kbSortOrder = null;

async function loadDocuments() {
  const docList = $('docList');
  if (docList) showSkeleton(docList, 'doc', 4);
  try {
    _kbAllDocs = await invoke('get_documents', { sortBy: _kbSortBy, sortOrder: _kbSortOrder });
    const isPro = get('isPro');
    setState({ docCount: _kbAllDocs.length, kbAllDocs: _kbAllDocs });
    updateChatInputState();
    const countEl = $('kbDocCount');
    if (countEl) {
      countEl.textContent = isPro
        ? t('knowledge_base.doc_count_pro', { count: _kbAllDocs.length })
        : t('knowledge_base.doc_count_free', { count: _kbAllDocs.length });
    }
    applyKbFilters();
  } catch (err) {
    toastError(err);
  } finally {
    if (docList) hideSkeleton(docList);
  }
}

/**
 * 应用筛选条件（搜索 + 状态 + 格式 + 标签），重新渲染文档列表。
 * 从缓存 _kbAllDocs 中筛选，不重新请求后端。
 */
function applyKbFilters() {
  const searchQ = ($('docSearchInput')?.value || '').trim().toLowerCase();
  const statusFilter = $('docStatusFilter')?.value || '';
  const formatFilter = $('docFormatFilter')?.value || '';
  const tagFilter = $('docTagFilter')?.value || '';

  _kbFilteredDocs = _kbAllDocs.filter((d) => {
    const name = displayDocName(d.file_path);
    const nameLower = name.toLowerCase();
    const status = docStatusOf(d);
    const ext = name.split('.').pop()?.toLowerCase() || '';
    const tags = d.tags || [];

    // 模糊搜索：文件名 + 标签
    const matchSearch = !searchQ ||
      nameLower.includes(searchQ) ||
      tags.some((tag) => tag.toLowerCase().includes(searchQ));
    const matchStatus = !statusFilter || status === statusFilter;
    const matchFormat = !formatFilter || ext === formatFilter;
    const matchTag = !tagFilter || tags.includes(tagFilter);

    return matchSearch && matchStatus && matchFormat && matchTag;
  });

  _kbRenderedCount = 0;
  const box = $('docList');
  if (box) box.innerHTML = '';

  if (_kbObserver) { _kbObserver.disconnect(); _kbObserver = null; }

  if (_kbFilteredDocs.length === 0) {
    _renderKbEmptyState(box, _kbAllDocs.length === 0);
  } else {
    _renderKbDocPage();
    _setupKbObserver();
  }
  _updateKbResultCount();
}

/**
 * 渲染一页文档（KB_PAGE_SIZE 条）。
 */
function _renderKbDocPage() {
  const box = $('docList');
  if (!box) return;

  const isPro = get('isPro');
  const multiSelect = isMultiSelectMode();
  const start = _kbRenderedCount;
  const end = Math.min(start + KB_PAGE_SIZE, _kbFilteredDocs.length);

  for (let i = start; i < end; i++) {
    box.appendChild(_createDocItem(_kbFilteredDocs[i], isPro, multiSelect));
  }
  _kbRenderedCount = end;
}

/**
 * 创建文档列表项 DOM 元素。
 * 设计：图标 + 状态圆点 + 文件名（flex-1 truncate）+ 内联标签 + hover 操作按钮
 */
function _createDocItem(d, isPro, multiSelect) {
  const name = displayDocName(d.file_path);
  const status = docStatusOf(d);
  const statusColor = STATUS_DOT_COLORS[status] || STATUS_DOT_COLORS.Failed;
  const [labelKey] = DOC_STATUS_STYLE[status] || DOC_STATUS_STYLE.Failed;
  const ext = name.split('.').pop()?.toLowerCase() || '';

  const wrapper = document.createElement('div');
  wrapper.dataset.docId = d.id;
  wrapper.dataset.docName = name;
  wrapper.dataset.docStatus = status;
  wrapper.dataset.docFormat = ext;
  wrapper.dataset.docTags = JSON.stringify(d.tags || []);

  const item = document.createElement('div');
  item.className = 'group flex items-center gap-2 px-2.5 py-2 rounded-lg text-xs hover:bg-white/5 transition-colors';

  // 多选模式：复选框
  if (multiSelect) {
    const checkbox = document.createElement('input');
    checkbox.type = 'checkbox';
    checkbox.dataset.docId = d.id;
    checkbox.className = 'shrink-0 w-3.5 h-3.5 accent-accent cursor-pointer';
    checkbox.onclick = (ev) => ev.stopPropagation();
    checkbox.onchange = () => {
      const checked = document.querySelectorAll('#docList input[type="checkbox"]:checked').length;
      const countEl = $('kbSelectedCount');
      if (countEl) countEl.textContent = t('knowledge_base.selected_count', { count: checked });
    };
    item.appendChild(checkbox);
  }

  // 文件类型图标（SVG inline，REQ-DS-004）
  const fileIconEl = document.createElement('span');
  fileIconEl.className = 'shrink-0 leading-none';
  fileIconEl.innerHTML = getFileIcon(ext);
  item.appendChild(fileIconEl);

  // 状态指示圆点
  const dot = document.createElement('span');
  dot.className = `shrink-0 w-1.5 h-1.5 rounded-full ${statusColor}`;
  dot.title = t('knowledge_base.' + labelKey);
  item.appendChild(dot);

  // 文件名（可点击预览 REQ-ING-010）
  const left = document.createElement('span');
  left.className = 'truncate flex-1 text-text-primary cursor-pointer hover:text-accent transition-colors';
  left.textContent = name;
  left.title = d.file_path;
  left.onclick = () => openDocPreview(d.id);
  // V3.1 P3-6：键盘可达（Enter/Space 触发预览）
  makeKeyboardClickable(left);
  item.appendChild(left);

  // 标签（内联显示，最多 3 个 + 溢出计数）
  if (d.tags && d.tags.length > 0 && !multiSelect) {
    const tagContainer = document.createElement('span');
    tagContainer.className = 'shrink-0 flex items-center gap-1';
    const maxTags = 3;
    for (let i = 0; i < Math.min(d.tags.length, maxTags); i++) {
      const tagEl = document.createElement('span');
      tagEl.className = 'px-1 py-0.5 rounded text-[9px] bg-accent/10 text-accent border border-accent/15';
      tagEl.textContent = d.tags[i];
      tagContainer.appendChild(tagEl);
    }
    if (d.tags.length > maxTags) {
      const more = document.createElement('span');
      more.className = 'text-[9px] text-text-quaternary';
      more.textContent = `+${d.tags.length - maxTags}`;
      tagContainer.appendChild(more);
    }
    item.appendChild(tagContainer);
  }

  // 操作按钮（hover 显示）
  const actions = document.createElement('span');
  actions.className = 'shrink-0 flex items-center gap-0.5';

  // 文档预览按钮（REQ-ING-010）
  const previewBtn = document.createElement('button');
  previewBtn.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  previewBtn.innerHTML = icon('eye', 'sm');
  previewBtn.title = t('doc.preview_title') || '预览';
  previewBtn.onclick = (ev) => { ev.stopPropagation(); openDocPreview(d.id); };
  actions.appendChild(previewBtn);

  const sumToggle = document.createElement('button');
  sumToggle.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  sumToggle.innerHTML = icon('summary', 'sm');
  sumToggle.title = t('knowledge_base.summary_toggle');
  sumToggle.style.display = status === 'Indexed' ? '' : 'none';
  sumToggle.onclick = async (ev) => {
    ev.stopPropagation();
    const panel = wrapper.querySelector('.doc-summary-panel');
    if (panel) { panel.classList.toggle('hidden'); return; }
    const newPanel = document.createElement('div');
    newPanel.className = 'doc-summary-panel px-2.5 pb-2 pt-1 text-[11px] text-text-secondary leading-relaxed';
    newPanel.innerHTML = `<span class="text-text-quaternary">${t('knowledge_base.summary_loading')}</span>`;
    wrapper.appendChild(newPanel);
    try {
      const summary = await invoke('get_document_summary', { docId: d.id });
      renderSummaryPanel(newPanel, d.id, name, summary);
    } catch (err) {
      newPanel.innerHTML = `<span class="text-red-400">${t('knowledge_base.summary_load_failed')}</span>`;
    }
  };
  actions.appendChild(sumToggle);

  const retry = document.createElement('button');
  retry.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  retry.innerHTML = icon('retry', 'sm');
  retry.title = t('knowledge_base.retry_index');
  retry.style.display = status === 'Failed' ? '' : 'none';
  retry.onclick = async (ev) => { ev.stopPropagation(); await retryDocument(d.id, name); };
  actions.appendChild(retry);

  const audit = document.createElement('button');
  audit.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  audit.innerHTML = icon('search', 'sm');
  audit.title = t('knowledge_base.audit_doc');
  audit.style.display = (isPro && status === 'Indexed') ? '' : 'none';
  audit.onclick = async (ev) => { ev.stopPropagation(); await auditDocument(d.id, name); };
  actions.appendChild(audit);

  const tagBtn = document.createElement('button');
  tagBtn.className = 'invisible group-hover:visible text-text-quaternary hover:text-accent px-1 transition-opacity';
  tagBtn.innerHTML = icon('tag', 'sm');
  tagBtn.title = t('knowledge_base.tag_input_placeholder');
  tagBtn.onclick = (ev) => {
    ev.stopPropagation();
    const inputRow = wrapper.querySelector('.tag-input-row');
    if (inputRow) {
      inputRow.classList.toggle('hidden');
      if (!inputRow.classList.contains('hidden')) inputRow.querySelector('input')?.focus();
    }
  };
  actions.appendChild(tagBtn);

  const del = document.createElement('button');
  del.className = 'invisible group-hover:visible text-text-quaternary hover:text-red-400 px-1 transition-opacity';
  del.dataset.action = 'delete';
  del.innerHTML = icon('close', 'sm');
  del.title = t('knowledge_base.delete_doc');
  del.onclick = async (ev) => { ev.stopPropagation(); await removeDocument(d.id, name); };
  actions.appendChild(del);

  if (multiSelect) {
    retry.style.display = 'none';
    del.style.display = 'none';
    audit.style.display = 'none';
    sumToggle.style.display = 'none';
    tagBtn.style.display = 'none';
  }

  item.appendChild(actions);
  wrapper.appendChild(item);

  // 标签输入行（点击 # 按钮切换显示，Esc 关闭，失焦自动收起）
  const tagInputRow = document.createElement('div');
  tagInputRow.className = 'tag-input-row hidden px-2.5 pb-1.5 flex items-center gap-1';
  const tagInput = document.createElement('input');
  tagInput.type = 'text';
  tagInput.placeholder = t('knowledge_base.tag_input_placeholder');
  tagInput.className = 'flex-1 px-1.5 py-0.5 rounded text-[10px] bg-surface-3 border border-border-default text-text-primary placeholder:text-text-quaternary focus:outline-none focus:border-accent';
  tagInput.onkeydown = async (ev) => {
    if (ev.key === 'Enter' && tagInput.value.trim()) {
      ev.preventDefault();
      try {
        await invoke('add_document_tag', { docId: d.id, tag: tagInput.value.trim() });
        tagInput.value = '';
        loadDocuments();
      } catch (err) { toastError(err); }
    }
    if (ev.key === 'Escape') { tagInput.value = ''; tagInputRow.classList.add('hidden'); }
  };
  tagInput.onblur = () => { if (!tagInput.value.trim()) tagInputRow.classList.add('hidden'); };
  tagInputRow.appendChild(tagInput);
  wrapper.appendChild(tagInputRow);

  return wrapper;
}

/**
 * 设置 IntersectionObserver 实现无限滚动。
 * 当 sentinel 元素进入视口时，加载下一页文档。
 */
function _setupKbObserver() {
  const sentinel = $('kbScrollSentinel');
  if (!sentinel) return;
  if (_kbObserver) _kbObserver.disconnect();
  _kbObserver = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting && _kbRenderedCount < _kbFilteredDocs.length) {
      _renderKbDocPage();
    }
  }, { root: $('kbDocScroll'), threshold: 0.1 });
  _kbObserver.observe(sentinel);
}

/**
 * 渲染空状态：知识库为空 或 搜索/筛选无结果。
 */
function _renderKbEmptyState(box, isEmpty) {
  const empty = document.createElement('div');
  empty.className = 'px-3 py-8 text-center text-xs text-text-quaternary';
  if (isEmpty) {
    empty.innerHTML = `
      <div class="mb-3 opacity-50 flex justify-center">${icon('book', 'lg')}</div>
      <div class="mb-1">${t('knowledge_base.empty_title')}</div>
      <div class="text-[11px] opacity-70">${t('knowledge_base.empty_hint')}</div>
      <div class="text-[10px] opacity-50 mt-2">${t('knowledge_base.empty_formats')}</div>`;
  } else {
    empty.innerHTML = `
      <div class="mb-2 opacity-50 flex justify-center">${icon('search', 'md')}</div>
      <div>${t('knowledge_base.no_match')}</div>`;
  }
  box.appendChild(empty);
}

/**
 * 更新底部结果计数显示。
 */
function _updateKbResultCount() {
  const el = $('kbResultCount');
  if (!el) return;
  const total = _kbAllDocs.length;
  const filtered = _kbFilteredDocs.length;
  if (total === 0) {
    el.textContent = '';
  } else if (filtered < total) {
    el.textContent = `${filtered}/${total}`;
  } else {
    el.textContent = `${total}`;
  }
}

/**
 * 渲染文档摘要面板内容（REQ-ING-019）。
 *
 * 若摘要存在，显示摘要文本 + 重新生成按钮；
 * 若摘要为空，显示「暂无摘要」+ 生成按钮。
 *
 * @param {HTMLElement} panel - 摘要面板容器
 * @param {string} docId - 文档 ID
 * @param {string} docName - 文档名（用于 toast 提示）
 * @param {string|null} summary - 摘要文本（null 表示尚未生成）
 */
function renderSummaryPanel(panel, docId, docName, summary) {
  if (summary && summary.trim()) {
    panel.innerHTML = '';
    const text = document.createElement('p');
    text.className = 'mb-1.5 text-text-secondary';
    text.textContent = summary;
    panel.appendChild(text);
  } else {
    panel.innerHTML = `<p class="mb-1.5 text-text-quaternary">${t('knowledge_base.summary_empty')}</p>`;
  }
  // 重新生成 / 生成按钮
  const regenBtn = document.createElement('button');
  regenBtn.className = 'text-accent hover:text-accent/80 text-[10px] underline';
  regenBtn.textContent = summary
    ? t('knowledge_base.summary_regenerate')
    : t('knowledge_base.summary_generate');
  regenBtn.onclick = async (ev) => {
    ev.stopPropagation();
    regenBtn.disabled = true;
    regenBtn.textContent = t('knowledge_base.summary_generating');
    try {
      const newSummary = await invoke('regenerate_summary', { docId });
      renderSummaryPanel(panel, docId, docName, newSummary);
      toast(t('knowledge_base.summary_generated', { name: docName }), 'info');
    } catch (err) {
      regenBtn.disabled = false;
      regenBtn.textContent = t('knowledge_base.summary_regenerate');
      toastError(err);
    }
  };
  panel.appendChild(regenBtn);
}

/**
 * 删除指定文档并刷新列表。
 */
async function removeDocument(id, name) {
  try {
    await invoke('delete_document', { id });
    toast(t('knowledge_base.deleted', { name }), 'info');
    await loadDocuments();
  } catch (err) {
    toastError(err);
  }
}

/**
 * 重试索引指定文档。
 */
async function retryDocument(docId, docName) {
  try {
    await invoke('retry_index', { id: docId });
    toast(t('knowledge_base.retrying', { name: docName }), 'info');
  } catch (err) {
    toastError(err);
  }
}

/**
 * 初始化会话列表。
 */
async function initConversations() {
  try {
    const list = await invoke('get_conversations', { workspaceId: getCurrentWorkspaceId() });
    if (list.length === 0) await newChat();
    else await loadConversation(list[0].id);
    await loadDocuments();
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 命令面板命令清单
// ============================================================

function getCommands() {
  return [
    { group: t('command_palette.group_navigation'), icon: icon('chat', 'sm'), label: t('command_palette.cmd_new_chat'), shortcut: '⌘N', action: () => newChat() },
    { group: t('command_palette.group_navigation'), icon: icon('search', 'sm'), label: t('command_palette.cmd_search_conv'), action: () => openSearchPopup() },
    { group: t('command_palette.group_navigation'), icon: icon('plus', 'sm'), label: t('command_palette.cmd_import'), shortcut: '⌘O', action: () => $('plusBtn').click() },
    { group: t('command_palette.group_navigation'), icon: icon('settings', 'sm'), label: t('command_palette.cmd_settings'), shortcut: '⌘,', action: () => openSettings() },
    { group: t('command_palette.group_navigation'), icon: icon('collapse', 'sm'), label: t('command_palette.cmd_toggle_sidebar'), action: () => toggleSidebar() },
    { group: t('command_palette.group_chat'), icon: icon('send', 'sm'), label: t('command_palette.cmd_focus_input'), shortcut: '⌘L', action: () => $('queryInput').focus() },
    { group: t('command_palette.group_chat'), icon: icon('stop', 'sm'), label: t('command_palette.cmd_stop'), shortcut: 'Esc', action: () => { if (get('streaming')) onStop(); } },
    { group: t('command_palette.group_chat'), icon: icon('trash', 'sm'), label: t('command_palette.cmd_clear_chat'), shortcut: '⌘⇧⌫', action: () => { if (!get('streaming')) newChat(); } },
    { group: t('command_palette.group_config'), icon: icon('eye', 'sm'), label: t('command_palette.cmd_toggle_vlm'), action: () => { /* settings.js onVlmToggle */ } },
    { group: t('command_palette.group_config'), icon: icon('download', 'sm'), label: t('command_palette.cmd_download_model'), action: () => initEmbedder() },
    { group: t('command_palette.group_license'), icon: icon('brand', 'sm'), label: t('command_palette.cmd_activate_pro'), action: () => { if (!get('isPro')) showPaywall(t('paywall.reason_cmd_palette')); } },
    ...getSecurityCommands().map(cmd => ({ group: t('security.section_title', 'Security'), icon: cmd.id.includes('lock') ? icon('lock', 'sm') : cmd.id.includes('verify') ? icon('check', 'sm') : icon('clipboard', 'sm'), label: cmd.label, action: cmd.action })),
    { group: t('command_palette.group_help'), icon: icon('keyboard', 'sm'), label: t('command_palette.cmd_shortcuts'), shortcut: '⌘/', action: () => toast(t('command_palette.shortcuts_hint'), 'info') },
  ];
}

// ============================================================
// 全屏检测（macOS traffic lights 显隐联动）
// ============================================================

/**
 * 初始化全屏检测：监听窗口尺寸变化，检测 macOS 全屏状态。
 * 全屏时 traffic lights 消失 → 添加 .topbar-fs 类 → padding-left 从 78px 缩到 8px。
 * 退出全屏 → 移除 .topbar-fs 类 → 恢复 78px padding。
 * 对 Tauri 运行时不可用的环境（E2E 测试）静默降级。
 */
function initFullscreenDetection() {
  const topBar = $('topBar');
  if (!topBar) return;

  /** 更新 topBar 的全屏样式类 */
  async function updateFullscreenClass() {
    try {
      const tauriWindow = window.__TAURI__?.window;
      if (!tauriWindow || !tauriWindow.getCurrentWindow) return;
      const win = tauriWindow.getCurrentWindow();
      if (!win || typeof win.isFullscreen !== 'function') return;
      const isFs = await win.isFullscreen();
      topBar.classList.toggle('topbar-fs', isFs);
    } catch (_) {
      // Tauri window API 不可用（如 E2E 测试环境），静默忽略
    }
  }

  // 初始检测
  updateFullscreenClass();

  // 监听窗口尺寸变化（进入/退出全屏会触发 resize）
  try {
    const tauriWindow = window.__TAURI__?.window;
    if (tauriWindow && tauriWindow.getCurrentWindow) {
      const win = tauriWindow.getCurrentWindow();
      if (win && typeof win.onResized === 'function') {
        win.onResized(() => updateFullscreenClass());
      }
    }
  } catch (_) {
    // 降级：仅初始检测一次，不监听后续变化
  }
}

// ============================================================
// 启动流程
// ============================================================

/**
 * 应用启动入口：主题初始化 → i18n 初始化 → Mermaid 初始化 → 预设渲染 → LLM 配置检查。
 */
async function boot() {
  // 防御性检查前置（V3.1 P2-3）：Tauri 运行时未就绪时直接重试，
  // 不先执行 initTheme/initI18n（其内部有 IPC 调用，未就绪时必然失败；
  // 且整体重启 boot 会导致主题/i18n 重复初始化）。
  if (!window.__TAURI__ || !window.__TAURI__.core || !window.__TAURI__.event) {
    console.warn('[EchoMind] Tauri runtime not ready, retrying boot() in 200ms…');
    setTimeout(boot, 200);
    return;
  }

  // 主题初始化（REQ-UI-011）：必须在 UI 渲染前完成，避免暗色闪烁
  await initTheme();

  // i18n 初始化（必须在 UI 渲染前完成，确保所有文案就绪）
  await initI18n();

// S56：加载自定义快捷指令模板（非阻塞，失败时静默降级）
loadCustomTemplates().catch(() => {});

// B09 v1.8：加载 Skill 文件（非阻塞，失败时静默降级）
loadSkills().catch(() => {});

  // 全屏检测：macOS 全屏时 traffic lights 消失，移除 78px padding 让按钮左移填补
  initFullscreenDetection();

  // ==text== 高亮标记扩展
  marked.use({
    extensions: [{
      name: 'highlight',
      level: 'inline',
      start(src) { return src.indexOf('=='); },
      tokenizer(src) {
        const match = /^==(?=\S)([\s\S]*?\S)==/.exec(src);
        if (match) return { type: 'highlight', raw: match[0], text: match[1] };
      },
      renderer(token) { return '<mark>' + token.text + '</mark>'; },
    }],
  });
  marked.setOptions({ breaks: true, gfm: true });

  // Mermaid 延迟加载回调（mermaid.min.js 3.4MB 仅在有图表时加载）
  // applyTheme() 中的 typeof mermaid !== 'undefined' 守卫保证主题切换安全
  setMermaidInitFn(() => {
    const theme = document.documentElement.dataset.theme || 'dark';
    const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches;
    let effective = theme === 'system' ? (prefersLight ? 'light' : 'dark') : theme;
    if (theme === 'high-contrast') effective = 'dark';
    if (typeof mermaid !== 'undefined') {
      mermaid.initialize({
        startOnLoad: false,
        theme: effective,
        securityLevel: 'strict',
        fontFamily: 'inherit',
      });
    }
  });

  // 初始化各模块
  renderPresetCards();
  applyPreset();
  initWizard(initConversations);
  initSidebar();

// 初始化书签面板（REQ-RAG-047 + REQ-RAG-053）
initBookmarks({ onNavigate: _navigateToMessage });
// 暴露刷新函数供 chat-render.js 的书签按钮调用（REQ-RAG-053）
window.__refreshBookmarks = () => refreshBookmarks({ onNavigate: _navigateToMessage });

  // REQ-A11Y-001 AC-6 / REQ-A11Y-004 AC-5：自动为按钮内装饰性 SVG 添加 aria-hidden
  document.querySelectorAll('button[aria-label] svg, button[data-i18n-aria-label] svg').forEach((svg) => {
    svg.setAttribute('aria-hidden', 'true');
  });

setReloadDocsCallback(loadDocuments);
setFilterChangeCallback(applyKbFilters);

// REQ-ING-008 v1.10：文档排序选择处理
if ($('docSortSelect')) {
$('docSortSelect').addEventListener('change', async () => {
const val = $('docSortSelect').value;
if (val) {
const [by, order] = val.split(':');
_kbSortBy = by;
_kbSortOrder = order;
} else {
_kbSortBy = null;
_kbSortOrder = null;
}
await loadDocuments();
});
}

// REQ-NAV-001 AC-4：恢复侧栏折叠状态
restoreSidebarState();

  // REQ-WS-001：初始化知识库选择器 + 注册切换回调
  setSwitchWorkspaceCallback(async () => {
    await loadConversations();
    await loadDocuments();
  });
  initWorkspaceSelector().catch(() => {});

  initSearchPopup(loadConversation);
  initCommandPalette(/** @type {any} */ (getCommands()));
  initGlobalSearch({
    onLoadConversation: (id) => loadConversation(id),
    onOpenDocPreview: (docId) => openDocPreview(docId),
    onOpenGraphViewer: (entityText) => openGraphViewerWithEntity(entityText),
  });
  initKeyboardHelp();
  initPaywall();
  initSettings();
  initDragDrop(async () => { await loadDocuments(); });
  initFilePicker(async () => { await loadDocuments(); });
  if ($('importCancelBtn')) initImportCancel();
  initConvNav();
  initContextMenu();

  // REQ-IX-001：暴露 send() 供右键菜单「重新生成」调用
  window.__echomindSend = send;

  // REQ-IX-001：右键菜单事件监听
  document.addEventListener('ctx-refresh-conversations', async () => {
    await loadConversations();
  });
  document.addEventListener('ctx-refresh-documents', async () => {
    await loadDocuments();
  });
document.addEventListener('ctx-regenerate', (e) => {
// 由 context-menu.js 触发的重新生成 — 直接调用 send()
// P0-3：支持变体参数（concise/detailed/force_search）
const detail = /** @type {CustomEvent} */ (e).detail || {};
const variant = detail.variant || 'default';
if (variant === 'force_search') {
  // 强制检索模式：临时启用混合搜索，chat_done 后恢复
  invoke('update_setting', { key: 'rag.hybrid_search', value: 'true' }).catch(() => {});
  // 标记需要在 chat_done 后恢复
  window.__echomindRegenForceSearch = true;
}
const sendFn = window.__echomindSend;
if (typeof sendFn === 'function') sendFn();
});

  // REQ-KB-004：文档列表键盘快捷键初始化
  initDocKeyboard({
    onRefresh: async () => { await loadDocuments(); },
  });

  // 初始化安全模块（同步加密状态、监听锁屏事件、绑定活动监听）
  initSecurity();

  // 初始化下载管理器（事件监听 + 崩溃恢复检查，REQ-LLM-004 v2）
  initDownloadManager();

  // 初始化离线检测（REQ-ERR-003 离线模式降级）
  initOfflineDetector();

  // 初始化面包屑（REQ-NAV-004 面包屑与上下文指示）
  initBreadcrumb({
    onNavigateKb: () => {
      // 跳转到知识库分区
      const kbTab = $('kbTab');
      if (kbTab) kbTab.click();
    },
  });

  // 延迟初始化非关键面板（性能优化：不阻塞首屏渲染）
  // 图谱/代码执行/AutoDream/符号搜索/工作流面板仅在用户交互时需要
  const _deferInit = (fn) => {
    if (typeof requestIdleCallback === 'function') {
      requestIdleCallback(fn, { timeout: 2000 });
    } else {
      setTimeout(fn, 100);
    }
  };

  // P1-1: 工具下拉菜单切换
  if ($('toolsBtn')) {
    $('toolsBtn').onclick = (e) => {
      e.stopPropagation();
      const menu = $('toolsMenu');
      if (menu) menu.classList.toggle('hidden');
    };
  }
  // 点击外部关闭工具菜单
  document.addEventListener('click', (e) => {
    const menu = $('toolsMenu');
    const dropdown = $('toolsDropdown');
    if (menu && dropdown && !dropdown.contains(e.target)) {
      menu.classList.add('hidden');
    }
  });

// 按钮点击处理立即绑定（面板打开时会懒初始化）
if ($('graphBtn')) {
  $('graphBtn').onclick = () => { openGraphViewer(); $('toolsMenu')?.classList.add('hidden'); };
}

/**
 * 打开知识图谱查看器并定位到指定实体（REQ-IX-008 全局搜索跳转）。
 * @param {string} entityText - 实体文本
 */
async function openGraphViewerWithEntity(entityText) {
  await openGraphViewer();
  // 延迟执行搜索定位，等待图谱面板和 D3 加载完成
  setTimeout(() => {
    const searchInput = document.getElementById('graphSearchInput');
    if (searchInput) {
      searchInput.value = entityText;
      searchInput.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    }
  }, 800);
}

if ($('dreamBtn')) {
  $('dreamBtn').onclick = () => { openDreamPanel(); $('toolsMenu')?.classList.add('hidden'); };
}
  if ($('symbolBtn')) {
    $('symbolBtn').onclick = () => { openSymbolSearch(); $('toolsMenu')?.classList.add('hidden'); };
  }
  // P1-5: workflowBtn 已移除（F 级模块删除）

  // 延迟初始化面板内部组件（不阻塞首屏）
  _deferInit(() => {
    initGraphResize();
    initDreamPanel();
    initSymbolSearch();
  });

  // 对话分支树按钮（REQ-RAG-039）
  if ($('branchTreeBtn')) {
    $('branchTreeBtn').onclick = () => { openConversationTreePanel(); $('toolsMenu')?.classList.add('hidden'); };
  }

  // 包装 invoke 以集中处理导入错误（付费墙 / 格式错误）
  const _origInvoke = window.__TAURI__.core.invoke;
  window.__TAURI__.core.invoke = async function(cmd, args) {
    try {
      return await _origInvoke.call(this, cmd, args);
    } catch (err) {
      const msg = String(err);
      if (msg.includes('PRO_REQUIRED') || msg.includes('LIMIT_REACHED')) {
        showPaywall(msg.split(':').slice(1).join(':').trim() || t('paywall.reason_default'));
      } else if (msg.startsWith('不支持的格式') || msg.startsWith('Unsupported format')) {
        toastError(msg);
      }
      throw err;
    }
  };

// 输入框事件
// 发送/停止合二为一：流式态点击 = 停止，空闲态点击 = 发送
$('sendBtn').onclick = () => {
  if (get('streaming')) onStop();
  else send();
};
$('newChatBtn').onclick = newChat;
$('settingsBtn').onclick = () => openSettings();

// DeepSeek 风格输入区快速 Toggle（S5 审计精简：5 → 2，仅保留混合搜索 + Agent）
const inputTogglesContainer = $('inputToggles');
if (inputTogglesContainer) {
  const hybridToggle = createInputToggle('hybrid', true);
  inputTogglesContainer.appendChild(hybridToggle);
  const agentToggle = createInputToggle('agent', false);
  inputTogglesContainer.appendChild(agentToggle);
}

// ⌘ J / Ctrl J 全局快捷键 → 新对话（DeepSeek 风格）
document.addEventListener('keydown', (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key === 'j') {
    e.preventDefault();
    newChat();
  }
});

  // 会话搜索弹框
  if ($('convSearchBtn')) {
    $('convSearchBtn').onclick = () => openSearchPopup();
  }

  // Pro 状态变更后刷新文档列表（审计按钮可见性依赖 isPro）
  subscribe('isPro', () => { loadDocuments(); });

  // 输入框微交互 — Shift+Enter 换行，Enter 发送，Up/Down 历史导航
  // 初始高度 48px（两行 leading-6），Shift+Enter 换行体验充足
  const MIN_INPUT_HEIGHT = 48;
  const MAX_INPUT_HEIGHT = 160;
  const queryInputEl = $('queryInput');
  // 方案6：使用统一的输入keydown处理器
  const inputKeyHandler = createInputKeyHandler({ send });
  queryInputEl.addEventListener('keydown', inputKeyHandler);
  queryInputEl.addEventListener('input', (e) => {
    const ta = e.target;
    // 禁用 transition：auto-grow 需要即时高度反馈，不能有动画延迟
    ta.style.transition = 'none';
    ta.style.height = 'auto';
    const newHeight = Math.min(Math.max(ta.scrollHeight, MIN_INPUT_HEIGHT), MAX_INPUT_HEIGHT);
    ta.style.height = newHeight + 'px';
    // 强制重排后恢复 transition，使 send() 的高度收缩有平滑动画
    void ta.offsetHeight;
    ta.style.transition = '';
    // V5：实时更新发送按钮视觉状态 + token 估算
    if (!get('streaming')) {
      updateInputUI();
    }
    updateTokenEstimate();
    // P1-2：字符计数器更新
    _updateCharCounter(ta.value);
  });
  // 失焦时保存草稿 + 重置历史导航
  queryInputEl.addEventListener('blur', () => {
    saveDraft();
    resetHistoryNav();
  });

  // 全局键盘快捷键
  initKeyboardShortcuts({
    onNewChat: () => newChat(),
    onImport: () => $('plusBtn').click(),
    onSettings: () => openSettings(),
    onToggleSidebar: () => toggleSidebar(),
    onAbort: () => { if (get('streaming')) onStop(); },
    onCloseVlm: () => $('vlmConfirmCancel').click(),
    onClosePaywall: () => hidePaywall(),
    onCloseSettings: () => closeSettings(),
    onCloseSearchPopup: () => closeSearchPopup(),
    onCloseGraph: () => closeGraphViewer(),
    onGlobalSearch: () => openGlobalSearch(),
    onExport: () => {
      const convId = get('currentConversationId');
      if (convId) exportConversationToPdf(convId);
    },
  });

// 初始化语音输入（REQ-RAG-034）：检测 Web Speech API 支持，绑定麦克风按钮
initVoiceInput();

// 初始化 PDF 导出按钮（REQ-EXP-005）
initExportButtons();

// 初始化快捷键帮助面板（REQ-HELP-002 v1.5）
initKeyboardHelp();

// 初始化全局错误边界（REQ-ERR-002 v1.5）
initErrorBoundary();

// KB 统计仪表盘按钮（REQ-KB-003 v1.5）
{
  const kbStatsBtn = document.getElementById('kbStatsBtn');
  if (kbStatsBtn) kbStatsBtn.onclick = openKbStats;
}

// 暴露给 E2E 测试
window.__openKbStats = openKbStats;
window.__closeKbStats = closeKbStats;
window.__openHelpPanel = openHelpPanel;
window.__closeHelpPanel = closeHelpPanel;
window.__loadConversations = loadConversations;

// 导出为 HTML 全局函数（REQ-EXP-007）
window.exportConversationToHtml = exportConversationToHtml;
window.exportDocumentToHtml = exportDocumentToHtml;

// 关于面板（REQ-HELP-003 v1.6）
initAboutPanel();
window.__openAboutPanel = openAboutPanel;
window.__closeAboutPanel = closeAboutPanel;

// 帮助面板按钮（REQ-HELP-001 v1.5）
{
  const helpBtn = document.getElementById('helpBtn');
  if (helpBtn) helpBtn.onclick = () => openHelpPanel();
}

// 注册对话流事件监听
// V3.1 P2-2：doc-status-changed 合流刷新（trailing 500ms）。
// 批量导入时每个文件发射 4-5 次状态事件，若每次都全量 get_documents + 列表重建，
// 导入 50 文件会产生 ~200 次 IPC 风暴与 skeleton 闪动；合流后每个静默窗口最多刷新一次。
let docRefreshTimer = null;
const debouncedLoadDocuments = () => {
  if (docRefreshTimer) clearTimeout(docRefreshTimer);
  docRefreshTimer = setTimeout(async () => {
    docRefreshTimer = null;
    await loadDocuments();
  }, DOC_REFRESH_DEBOUNCE_MS);
};
window.addEventListener('pagehide', () => {
  if (docRefreshTimer) { clearTimeout(docRefreshTimer); docRefreshTimer = null; }
});

initChatEventListeners({
    onChatDone: async () => { setState({ isNewConversation: false }); await loadConversations(); },
    onDocStatusChanged: debouncedLoadDocuments,
    onImportProgress: (p) => {
      if (!p) { hideImportProgress(); return; }
      const percent = p.total > 0 ? Math.round((p.completed / p.total) * 100) : 0;
      $('importProgressBar').style.width = `${percent}%`;
      $('importProgressText').textContent = t('import.progress_importing', { done: p.completed, total: p.total, file: p.current_file });
    },
    onIndexingPhase: (p) => {
      const bar = $('importProgressBar');
      if (bar) {
        bar.classList.add('progress-indeterminate');
        bar.style.width = '8%';
      }
      $('importProgressText').textContent = p.message || '正在索引…';
    },
    onEmbeddingProgress: (p) => {
      const bar = $('importProgressBar');
      if (bar) bar.classList.remove('progress-indeterminate');
      const rawPercent = p.total > 0 ? Math.round((p.embedded / p.total) * 100) : 0;
      const percent = 30 + Math.round(rawPercent * 0.7);
      bar.style.width = `${percent}%`;
      $('importProgressText').textContent = t('import.progress_vectorizing', { done: p.embedded, total: p.total, doc: p.doc_name });
    },
    onModelDownload: (p) => {
      if (p.done) {
        toastSuccess(t('settings.model_downloaded'));
      } else if (p.error) {
        toastError(p.error.message || t('wizard.download_failed'));
      }
    },
  });

  // REQ-ERR-004：监听数据库完整性错误事件，显示损坏提示 Modal
  listen('db-integrity-error', (event) => {
    const msg = event.payload || '';
    const modal = $('dbError');
    if (modal) {
      const detail = $('dbErrorDetail');
      if (detail && msg) detail.textContent = msg;
      modal.classList.remove('hidden');
    }
  });

  // 数据库异常 Modal 按钮事件
  {
    const modal = $('dbError');
    if (modal) {
      const close = () => modal.classList.add('hidden');
      $('dbErrorClose').onclick = close;
      $('dbErrorCloseBtn').onclick = close;
      $('dbErrorOpenDir').onclick = async () => {
        try {
          const path = await invoke('open_data_dir');
          // 通过 opener 插件在系统文件管理器中打开
          if (window.__TAURI__?.opener?.openPath) {
            await window.__TAURI__.opener.openPath(path);
          }
        } catch (err) {
          toastError(err);
        }
      };
    }
  }

// 启动引导：3 步流程检查（向量模型 → LLM 配置 → 主界面）
resetChatArea(t('chat.placeholder_title'), t('chat.placeholder_desc'));

try {
// V3.1 P2-3：启动 IPC 并行化 — 4 个独立调用从串行（~4×RTT）改为并行（~1×RTT）
const [embedderStatus, settings, isDemoMode, proStatus] = await Promise.all([
  invoke('check_embedder_status'),
  invoke('get_settings'),
  demoModeApi.isDemoMode().catch(() => false),
  invoke('get_pro_status'),
]);
const embedderReady = embedderStatus === 'ready';
const llmConfigured = settings.has_llm_config;

if (isDemoMode) {
// 演示模式：直接进入主界面，显示提示栏
setState({
  isPro: proStatus,
  demoMode: true,
  llmConfigured: true,
  currentModel: t('wizard.demo_mode'),
  currentLlmMode: 'remote',
  hybridEnabled: settings.hybrid_search || false,
  agentEnabled: settings.agent_enabled || false,
  subAgentEnabled: settings.sub_agent_enabled || false,
  memoryEnabled: settings.memory_enabled || false,
  webSearchEnabled: settings.web_search_enabled || false,
});
updateModelPill();
updateProStatus();
showApp();
await initConversations();
await loadDocuments();
updateChatInputState();
showDemoModeBanner();
} else if (embedderReady && llmConfigured) {
// 全部就绪：直接进入主界面
setState({
  isPro: proStatus,
  vlmEnabled: settings.vlm_enabled,
  currentModel: settings.model || '',
  currentLlmMode: settings.llm_mode || 'remote',
  llmConfigured: true,
  // 方案4：初始化功能toggle状态
  hybridEnabled: settings.hybrid_search || false,
  agentEnabled: settings.agent_enabled || false,
  subAgentEnabled: settings.sub_agent_enabled || false,
  memoryEnabled: settings.memory_enabled || false,
  webSearchEnabled: settings.web_search_enabled || false,
});
updateModelPill();
updateProStatus();
showApp();
await initConversations();
await loadDocuments();
updateChatInputState();
} else if (!embedderReady) {
// 向量模型未就绪：从 Step 1 开始
showWizard();
showWizardStep(1);
} else {
// 向量模型就绪但 LLM 未配置：从 Step 2 开始
showWizard();
showWizardStep(2);
}
} catch (_) {
showWizard();
showWizardStep(1);
}

  // P2-1：流恢复检测 — 页面加载时检查是否有未完成的流
  const streamState = checkStreamResume();
  if (streamState) {
    toast(t('chat.stream_resume_detected', { query: streamState.query?.slice(0, 30) }) || `检测到上次未完成的对话：${streamState.query?.slice(0, 30)}…`, 'info');
  }

  // REQ-HELP-004 S87：启动后 5s 异步检查更新（非阻塞，网络不可用时静默跳过）
  initUpdateBanner();
}

// ============================================================
// 主题管理（REQ-UI-011 浅色主题切换）
// ============================================================

/**
 * 应用主题到 DOM（同步执行，不涉及持久化）。
 *
 * 设置 document.documentElement.dataset.theme，CSS 变量自动切换。
 * 同时更新 Mermaid 主题 + color-scheme + state.theme。
 *
 * @param {string} theme - 主题模式：'dark' / 'light' / 'system'
 */
function applyTheme(theme) {
  const root = document.documentElement;
  // 临时禁用过渡动画，避免主题切换闪烁（DeepSeek 风格）
  document.body.classList.add('change-theme');
  root.dataset.theme = theme;

  // 计算实际生效的主题（system 模式需解析 prefers-color-scheme）
  const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches;
  let effective = theme === 'system' ? (prefersLight ? 'light' : 'dark') : theme;
  // 高对比度模式使用暗色 Mermaid 主题（REQ-A11Y-005）
  if (theme === 'high-contrast') effective = 'dark';

  // 更新 Mermaid 主题（mermaid 是全局常量，vendor.d.ts 已声明）
  if (typeof mermaid !== 'undefined') {
    mermaid.initialize({
      startOnLoad: false,
      theme: effective,
      securityLevel: 'strict',
      fontFamily: 'inherit',
    });
  }

  // 更新 state（如果 state 模块已加载）
  try { setState({ theme }); } catch (_) { /* state 可能尚未初始化 */ }

  // 双 rAF 后恢复过渡动画（确保浏览器完成一次重绘）
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      document.body.classList.remove('change-theme');
    });
  });
}

/**
 * 设置主题并持久化（REQ-UI-011 AC-3）。
 *
 * 先同步应用主题到 DOM，再异步持久化到后端 settings 表。
 * 后端不可用时（如 E2E 测试）降级到 localStorage。
 *
 * @param {string} theme - 主题模式：'dark' / 'light' / 'system'
 */
async function setTheme(theme) {
  applyTheme(theme);
  try {
    await window.__TAURI__.core.invoke('set_theme', { theme });
  } catch (_) {
    localStorage.setItem('echomind.theme', theme);
  }
}

/**
 * 初始化主题：从后端或 localStorage 恢复持久化的主题偏好（REQ-UI-011 AC-3）。
 *
 * 优先从后端 settings 表读取（Tauri 环境），降级到 localStorage（E2E 测试）。
 * 同时注册 prefers-color-scheme 变化监听器（system 模式自动切换，AC-4）。
 */
async function initTheme() {
  let theme = localStorage.getItem('echomind.theme') || 'dark';
  try {
    const saved = await window.__TAURI__.core.invoke('get_theme');
    if (saved && typeof saved === 'string') theme = saved;
  } catch (_) {
    // 后端不可用（如 E2E 测试），使用 localStorage 值
  }
  applyTheme(theme);

  // 注册系统主题变化监听器（system 模式自动切换，AC-4）
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (document.documentElement.dataset.theme === 'system') {
      applyTheme('system');
    }
  });
}

// 暴露为全局函数（设置面板需要调用）
window.setTheme = setTheme;

// ============================================================
// REQ-RAG-051: 演示模式提示栏
// ============================================================

/** 显示演示模式提示栏（REQ-RAG-051 AC-4） */
function showDemoModeBanner() {
  let banner = $('demoModeBanner');
  if (!banner) {
    banner = document.createElement('div');
    banner.id = 'demoModeBanner';
    banner.className = 'demo-mode-banner';
    banner.innerHTML = `<span class="demo-mode-text">${t('wizard.demo_mode_banner')}</span>`;
    const closeBtn = document.createElement('button');
    closeBtn.className = 'demo-mode-close';
    closeBtn.textContent = '×';
    closeBtn.setAttribute('aria-label', t('common.close'));
    closeBtn.onclick = () => { banner.remove(); };
    banner.appendChild(closeBtn);
    // 插入到主容器顶部
    const mainContent = document.querySelector('main') || document.body;
    mainContent.insertBefore(banner, mainContent.firstChild);
  }
  banner.classList.remove('hidden');
}

/** 隐藏演示模式提示栏 */
function hideDemoModeBanner() {
  const banner = $('demoModeBanner');
  if (banner) banner.remove();
}

/**
 * 从设置面板配置 LLM 后退出演示模式（REQ-RAG-051 AC-5）。
 * 用户配置 API Key 后自动退出演示模式，清除示例文档。
 */
async function exitDemoModeIfActive() {
  try {
    const isDemo = await demoModeApi.isDemoMode();
    if (isDemo) {
      await demoModeApi.exit();
      setState({ demoMode: false });
      hideDemoModeBanner();
      toast(t('wizard.demo_mode_exited'), 'success');
    }
  } catch (_) { /* 静默降级 */ }
}

// 暴露为全局函数（设置面板 LLM 配置保存后调用）
window.exitDemoModeIfActive = exitDemoModeIfActive;

// 启动
boot();
