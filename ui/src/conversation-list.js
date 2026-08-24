/**
 * EchoMind 会话列表面板模块（V3.1 P4-3 自 main.js 拆分）。
 *
 * 职责：
 * 1. 会话列表加载 / 时间分组 / 置顶 / 拖拽排序（REQ-IX-002）
 * 2. 长会话两段式渐进渲染（V3.1 P4-5：首屏最近 N 条 + 「查看更早消息」prepend）
 * 3. 会话加载 loadConversation / 删除 / 窗口标题同步
 * 4. 搜索跳转高亮 highlightSearchMatch
 */

import { $, makeKeyboardClickable, icon, showSkeleton, hideSkeleton } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError } from './toast.js';
import { get, setState } from './state.js';
import { convApi } from './ipc.js';
// 渲染层
import {
  appendBlock, appendUserBlock, renderSources,
  showMsgActions, appendAiDisclaimer, resetChatArea, scrollToBottom,
  scheduleRender,
} from './chat-render.js';
import { renderMarkdown, renderRichContent } from './markdown.js';
import { renderBranchPagination } from './message-edit.js';
import { getTurn, buildTurnTree, setTurnTree, applyActiveVersions } from './turn-tree.js';
import { getCurrentWorkspaceId } from './workspace.js';
import { createBookmarkButton, refreshBookmarks } from './bookmarks.js';
import { resetFeedbackTracking } from './feedback.js';

/**
 * 宿主回调注入（main.js 初始化时调用一次）：
 * - onStop: 流式中切换会话前的中止处理
 * - saveDraft/clearQueue/restoreDraft/resetHistoryNav: 草稿与历史导航
 * - closeAllPanels/updateInputUI: 面板与输入区状态
 * - resetHistoryNav 等
 */
/** 长会话首屏渲染的最近消息数上限（V3.1 P4-5） */
const MSG_INITIAL_RENDER_LIMIT = 20;
/** 每次向上加载的历史消息批次大小 */
const MSG_LOAD_MORE_BATCH = 20;
/** 会话列表初始渲染上限（V3.1 P2-4） */
const CONV_INITIAL_RENDER_LIMIT = 50;

const _host = {};

export function setupConversationListHost(host) {
  Object.assign(_host, host);
}

export function _updateWindowTitle(convTitle) {
  const appName = 'EchoMind';
  document.title = convTitle ? `${convTitle} — ${appName}` : appName;
}

function _collectRenderUnits(messages) {
  const units = [];
  const seenTurns = new Set();
  let lastUserMsg = '';
  for (const m of messages) {
    if (m.turn_group) {
      if (seenTurns.has(m.turn_group)) continue;
      seenTurns.add(m.turn_group);
      const turn = getTurn(m.turn_group);
      if (turn) {
        const ver = turn.versions.find(v => v.version === turn.activeVersion);
        if (ver) {
          lastUserMsg = ver.userContent;
          units.push({ kind: /** @type {'turn'} */ ('turn'), m, turn, ver, lastUserMsg });
          continue;
        }
      }
    }
    if (m.role === 'user') lastUserMsg = m.content;
    units.push({ kind: /** @type {'plain'} */ ('plain'), m, lastUserMsg });
  }
  return units;
}

/** 渲染一个 turn 单元（原循环体 turn 分支抽取）。 */
function _renderTurnUnit(unit, parent) {
  const { m, turn, ver, lastUserMsg } = unit;
  const userBlock = appendUserBlock(ver.userContent, ver.userMessageId || null, parent, { scroll: false });
  if (userBlock) {
    userBlock.dataset.turnGroup = m.turn_group;
    userBlock.dataset.version = String(turn.activeVersion);
  }
  if (!ver.assistantContent) return;
  const el = appendBlock('assistant', parent);
  el.dataset.query = lastUserMsg;
  // msgId 用于思考面板状态持久化（使用用户消息 ID 作为唯一标识）
  el.dataset.msgId = ver.userMessageId || m.id || '';
  setState({ currentAssistantEl: el });
  if (el._thinkingPanel) {
    el._thinkingPanel.setComplete();
    if (ver.reasoning) el._thinkingPanel.renderReasoning(ver.reasoning);
    el._thinkingPanel.setMsgId(ver.userMessageId || m.id || null);
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
  return renderRichContent(mdEl).then(() => {
    showMsgActions(el, ver.assistantContent, 'assistant');
    appendAiDisclaimer(el);
    // 多版本时渲染分页器（挂在用户操作栏中）
    if (turn.versions.length > 1 && userBlock) {
      renderBranchPagination(userBlock, m.turn_group);
    }
    setState({ currentAssistantEl: null, currentRawMarkdown: '' });
  });
}

/** 渲染一条普通（无 turn_group）消息单元。 */
function _renderPlainUnit(unit, parent) {
  const { m, lastUserMsg } = unit;
  if (m.role === 'user') {
    appendUserBlock(m.content, m.id || null, parent, { scroll: false });
    return Promise.resolve();
  }
  const el = appendBlock('assistant', parent);
  el.dataset.query = lastUserMsg;
  const lastUserBlock = parent.querySelector('.msg-user:last-of-type') || document.querySelector('.msg-user:last-child');
  const thinkingMsgId = lastUserBlock?.dataset.msgId || m.id || null;
  el.dataset.msgId = thinkingMsgId || '';
  setState({ currentAssistantEl: el });
  if (el._thinkingPanel) {
    el._thinkingPanel.setComplete();
    if (m.reasoning) el._thinkingPanel.renderReasoning(m.reasoning);
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
  return renderRichContent(mdEl).then(() => {
    showMsgActions(el, m.content, 'assistant');
    appendAiDisclaimer(el);
    setState({ currentAssistantEl: null, currentRawMarkdown: '' });
  });
}

/**
 * 「查看更早消息」哨兵按钮：prepend 一批更早的渲染单元并补偿滚动位置。
 * @param {Array} earlyUnits - 待渲染的更早单元（按时间正序）
 * @param {HTMLElement} sentinel - 哨兵按钮元素自身（渲染后移除/更新计数）
 */
async function _prependEarlierUnits(earlyUnits, sentinel) {
  const chatArea = $('chatArea');
  const batch = earlyUnits.splice(0, MSG_LOAD_MORE_BATCH);
  sentinel.disabled = true;
  try {
    const prevHeight = chatArea.scrollHeight;
    const prevTop = chatArea.scrollTop;
    const frag = document.createDocumentFragment();
    for (const unit of batch) {
      if (unit.kind === 'turn') await _renderTurnUnit(unit, frag);
      else await _renderPlainUnit(unit, frag);
    }
    chatArea.insertBefore(frag, sentinel.nextSibling ?? chatArea.firstChild);
    // 补偿滚动：保持视口内内容视觉不动
    chatArea.scrollTop = prevTop + (chatArea.scrollHeight - prevHeight);
  } finally {
    sentinel.disabled = false;
    if (earlyUnits.length === 0) sentinel.remove();
    else {
      sentinel.textContent = t('chat.load_earlier', { count: earlyUnits.length })
        || `查看更早消息（${earlyUnits.length}）`;
    }
  }
}


export async function loadConversation(id, searchQuery) {
  if (get('streaming')) {
    await _host.onStop();
    // 等待 finalizeStream 完成状态清理
    await new Promise((r) => setTimeout(r, 100));
  }
  // 方案5：会话切换时关闭所有面板
  _host.closeAllPanels();
  // 保存当前会话草稿 + 清空排队队列
  _host.saveDraft();
  _host.clearQueue();
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
  // V3.1 P4-5：两段式渐进渲染 — 收集全部渲染单元，只同步渲染最近 N 个；
  // 更早单元经顶部「查看更早消息」哨兵按批 prepend（保持滚动位置）。
  const units = _collectRenderUnits(messages);
  const hiddenUnits = units.slice(0, Math.max(0, units.length - MSG_INITIAL_RENDER_LIMIT));
  const visibleUnits = units.slice(Math.max(0, units.length - MSG_INITIAL_RENDER_LIMIT));

  for (const unit of visibleUnits) {
    if (unit.kind === 'turn') await _renderTurnUnit(unit, $('chatArea'));
    else await _renderPlainUnit(unit, $('chatArea'));
  }
  setState({ currentAssistantEl: null, currentRawMarkdown: '' });

  // 历史哨兵：存在被隐藏的更早单元时插入
  if (hiddenUnits.length > 0) {
    const chatAreaEl = $('chatArea');
    const sentinel = document.createElement('button');
    sentinel.id = 'loadEarlierBtn';
    sentinel.type = 'button';
    sentinel.className = 'mx-auto my-2 px-4 py-1.5 text-xs rounded-full border border-border-default text-text-quaternary hover:text-text-secondary hover:bg-surface-2 transition-colors cursor-pointer bg-transparent';
    sentinel.textContent = t('chat.load_earlier', { count: hiddenUnits.length })
      || `查看更早消息（${hiddenUnits.length}）`;
    sentinel.onclick = () => _prependEarlierUnits(hiddenUnits, sentinel);
    chatAreaEl.insertBefore(sentinel, chatAreaEl.firstChild);
  }

  // 恢复草稿 + 重置历史导航  // 恢复草稿 + 重置历史导航
  _host.resetHistoryNav();
  _host.restoreDraft();
  _host.updateInputUI();
  scrollToBottom();
  await loadConversations();
  // 搜索跳转高亮：如果从搜索结果进入，滚动到匹配消息并高亮
  if (searchQuery) {
    setTimeout(() => highlightSearchMatch(searchQuery), 200);
  }
}

export function highlightSearchMatch(query) {
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

export async function removeConversation(id) {
  await invoke('delete_conversation', { id });
  if (id === get('currentConversationId')) await _host.newChat();
  else await loadConversations();
}

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


// ============================================================
// 会话列表加载与渲染（V3.1 P4-3 自 main.js 移入）
// ============================================================

export async function loadConversations() {
const convBox = $('convList');
if (convBox) showSkeleton(convBox, 'conv', 4);
const list = await invoke('get_conversations', { workspaceId: getCurrentWorkspaceId() });
// 暴露会话列表供书签模块查询标题（REQ-RAG-047）
window.__echomindConversations = list;
// 刷新书签面板（会话标题可能变化）— 兼容消息级书签跳转（V3.1 P3-5 统一导航器）
refreshBookmarks({ onNavigate: (convId, messageId) => _host.navigateToMessage(convId, messageId) });
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
    onToggle: () => { refreshBookmarks({ onNavigate: (convId, messageId) => _host.navigateToMessage(convId, messageId) }); },
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
