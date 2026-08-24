/**
 * EchoMind 对话渲染模块 — DOM 渲染 + 引用来源 + 消息操作栏 + 滚动控制。
 *
 * 从 chat.js 拆分而来，职责：
 * 1. 消息 Block 追加与渲染（user/assistant）
 * 2. 流式渲染调度（rAF 节流）
 * 3. 引用来源卡片渲染
 * 4. 消息操作栏（复制/编辑/重新生成）
 * 5. 滚动控制 + 「回到底部」浮动按钮
 * 6. 上下文折叠提示
 *
 * 注意：本模块中的函数 appendUserBlock / showMsgActions 会调用 send()，
 * send() 定义在 chat.js 中。在构建后（build-ui.mjs 拼接）所有函数
 * 在同一作用域内，不存在循环依赖问题。
 */

import { getState, setState, get } from './state.js';
import { $, copyToClipboard } from './utils.js';
import { invoke, openDialog } from './ipc.js';
import { importPaths } from './import.js';
import { toast, toastError } from './toast.js';
import { renderMarkdown, renderRichContent } from './markdown.js';
import { t } from './i18n.js';
// caret.js 已合并到本模块末尾
import { shouldAutoScroll, resetScrollLock, createJumpToLatestButton, beginProgrammaticScroll } from './chat-utils.js';
import { renderEmptyState } from './empty-state.js';
import { createThinkingPanel } from './thinking-panel.js';
// privacy-indicator.js 已合并到本模块末尾
import { createEditButton, enterEditMode, renderBranchPagination, findAssistantBlock } from './message-edit.js';
import { icon } from './utils.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { createTtsButton } from './voice-input.js';
// regen-carousel.js merged into this file (createCarousel, addCarouselVersion, etc.)
import { generateTurnGroupId, getTurn, getActiveVersion, getVersionCount, setTurnTree, buildTurnTree, buildHistoryFromTurns, addVersion, updateVersionAssistant } from './turn-tree.js';
import { convApi } from './ipc.js';

// ============================================================
// chat 动作注册表（V3.1 P4-2：解除 chat-render ⇄ chat 循环依赖。
// chat.js 在模块初始化时注入 send/sendFromEdit，本模块仅经此表调用。）
// ============================================================
const _chatActions = {};

/**
 * 由 chat.js 调用：注入本模块需要的发送动作（单向依赖）。
 * @param {{send: Function, sendFromEdit: Function}} actions
 */
export function registerChatActions(actions) {
  Object.assign(_chatActions, actions);
}

// ============================================================
// 模块级状态
// ============================================================

/** 「回到底部」浮动按钮（null 表示未创建）。 */
let _jumpToLatestBtn = null;

// ============================================================
// 重新生成变体菜单（P0-3：DeepSeek 风格重新生成变体）
// ============================================================

/**
 * 显示重新生成变体菜单（右键 / 长按触发）。
 * @param {number} x - 屏幕坐标 X
 * @param {number} y - 屏幕坐标 Y
 * @param {(variant: 'default'|'concise'|'detailed') => void} callback - 选择变体后的回调
 */
function _showRegenVariantMenu(x, y, callback) {
  // 移除已有菜单
  const existing = document.querySelector('.regen-variant-menu');
  if (existing) existing.remove();

  const menu = document.createElement('div');
  menu.className = 'regen-variant-menu fixed z-50 bg-surface-2 border border-border rounded-lg shadow-lg py-1 min-w-[160px]';
  menu.style.left = x + 'px';
  menu.style.top = y + 'px';

  const variants = [
    { id: 'default', label: t('chat.regen_default') || '重新生成' },
    { id: 'concise', label: t('chat.regen_concise') || '简洁模式' },
    { id: 'detailed', label: t('chat.regen_detailed') || '详细模式' },
  ];

  for (const v of variants) {
    const item = document.createElement('div');
    item.className = 'px-4 py-2 text-sm text-text-secondary hover:bg-accent/10 hover:text-accent cursor-pointer transition-colors';
    item.textContent = v.label;
    item.onclick = () => {
      menu.remove();
      callback(/** @type {'default'|'concise'|'detailed'} */(v.id));
    };
    menu.appendChild(item);
  }

  document.body.appendChild(menu);

  // 点击外部关闭菜单
  setTimeout(() => {
    const closeHandler = (e) => {
      if (!menu.contains(e.target)) {
        menu.remove();
        document.removeEventListener('click', closeHandler);
      }
    };
    document.addEventListener('click', closeHandler);
  }, 0);

  // ESC 键关闭菜单
  const escHandler = (e) => {
    if (e.key === 'Escape') {
      menu.remove();
      document.removeEventListener('keydown', escHandler);
    }
  };
  document.addEventListener('keydown', escHandler);
}

// ============================================================
// 模型标识格式化（TC-QA-004）
// ============================================================

/**
 * 更新输入栏模型指示器药丸。
 *
 * - 远程模式：☁️ + 模型名
 * - 本地模式：💻 + GGUF 简称（去掉路径和扩展名）
 * - 无模型时：隐藏药丸
 */
export function updateModelPill() {
  const pill = document.getElementById('modelPill');
  if (!pill) return;
  const mode = get('currentLlmMode') || 'remote';
  const model = get('currentModel') || '';
  if (!model) { pill.classList.add('hidden'); return; }
  pill.classList.remove('hidden');
  const iconEl = document.getElementById('modelPillIcon');
  const nameEl = document.getElementById('modelPillName');
  if (iconEl) iconEl.innerHTML = mode === 'local' ? icon('keyboard', 'sm') : icon('cloud', 'sm');
  let displayName = model;
  if (mode === 'local') {
    const parts = model.split('/');
    displayName = parts[parts.length - 1].replace(/\.gguf$/i, '');
  }
  if (nameEl) nameEl.textContent = displayName;
  pill.onclick = null;
}

// ============================================================
// 聊天区渲染
// ============================================================

/** 显示配置向导，隐藏主界面。 */
export function showWizard() {
  $('wizard').classList.remove('hidden');
  $('app').classList.add('hidden');
}

/** 隐藏配置向导，显示主界面。 */
export function showApp() {
  $('wizard').classList.add('hidden');
  $('app').classList.remove('hidden');
}

/**
 * 重置聊天区为空状态引导页面（TC-QA-003~011）。
 * @param {string} [title] - 兼容旧签名（新实现忽略）
 * @param {string} [desc] - 兼容旧签名（同上）
 */
export function resetChatArea(title, desc) {
  const chatArea = $('chatArea');
  // REQ-RAG-018: 传入文档名列表以生成动态建议（AC-3）
  const allDocs = get('kbAllDocs') || [];
  const docNames = allDocs.map((d) => d.name || d.title || '').filter(Boolean).slice(0, 5);
  renderEmptyState(chatArea, {
    docNames,
    onPickQuestion: (question) => {
      const input = $('queryInput');
      if (input) {
        input.value = question;
        input.focus();
        // REQ-RAG-018 AC-2: 点击建议卡片自动填入输入框并发送
        const sendBtn = $('sendBtn');
        if (sendBtn && !sendBtn.disabled) {
          sendBtn.click();
        }
      }
    },
    onImport: () => {
      $('plusBtn')?.click();
    },
  });
  const privacyContainer = chatArea.querySelector('.empty-state-privacy') || chatArea.querySelector('.empty-state-wrapper');
  if (privacyContainer) {
    renderPrivacyIndicator(privacyContainer, {
      onClick: () => $('settingsBtn')?.click(),
    });
  }
}

/**
 * 隐藏空状态引导页。
 */
export function hideEmptyState() {
  const area = $('chatArea');
  if (area.children.length === 1 && area.firstElementChild?.classList.contains('empty-state-wrapper')) {
    area.innerHTML = '';
  }
}

/**
 * 追加一条消息 Block 到聊天区（P1-4 扁平全宽布局）。
 * @param {'user'|'assistant'} role - 消息角色
 * @returns {HTMLElement} 追加的 Block 根元素
 */
export function appendBlock(role, parent = null) {
  const container = parent || $('chatArea');
  hideEmptyState();
  const wrap = document.createElement('div');
  wrap.className = `msg-block message-in msg-${role} animate-message-in`;
  if (role === 'user') {
    // DeepSeek 风格：用户消息右对齐气泡（浅蓝背景 + 22px 大圆角 + padding）
    // DeepSeek 实测：background rgb(237,243,254) + border-radius 22px + padding 10px 16px + fontSize 16px
    wrap.className = 'msg-block message-in msg-user animate-message-in flex flex-col items-end mb-1 group';
    wrap.innerHTML = `<div class="msg-content msg-user-content text-text-primary text-base leading-normal whitespace-pre-wrap break-words cursor-text" style="background: var(--msg-user-bg); border: 1px solid var(--msg-user-border); border-radius: var(--msg-user-radius); padding: 10px 16px; max-width: var(--msg-user-max-width); transition: background 0.3s ease;" title="${t('chat.edit') || '点击编辑'}"></div>`;
  } else {
    // DeepSeek 风格：AI 消息透明背景
    // DeepSeek 实测：AI 正文 ds-assistant-message-main-content 用 16px + color rgb(15,17,21) 深色
    // 思考链内 ds-markdown 用 14px + color rgb(97,102,107) 灰色
    wrap.className = 'msg-block message-in msg-assistant animate-message-in w-full mb-8 group';
    const thinkingPanel = createThinkingPanel();
    wrap.innerHTML = `
      <div class="msg-content">
        <div class="md text-text-primary text-base leading-normal"></div>
        <div class="sources mt-2"></div>
      </div>
      <div class="flex gap-1 mt-3 pt-3 opacity-0 group-hover:opacity-100 transition-opacity duration-normal msg-actions"></div>`;
    wrap.querySelector('.msg-content').insertBefore(thinkingPanel.container, wrap.querySelector('.md'));
    wrap._thinkingPanel = thinkingPanel;
  }
  container.appendChild(wrap);
  if (role === 'user') {
    const actions = document.createElement('div');
    actions.className = 'flex justify-end gap-2 pt-3 pb-2 transition-opacity duration-normal';
    actions.dataset.role = 'user-actions';
    wrap.after(actions);
  }
  return wrap;
}

/**
 * 追加用户消息 Block 并滚动到底部。
 * @param {string} text - 用户消息文本
 * @param {string|null} msgId - 消息行 id（用于首次编辑时定位原始行）
 */
export function appendUserBlock(text, msgId = null, parent = null, { scroll = true } = {}) {
  const el = appendBlock('user', parent);
  el.dataset.fullText = text;
  el.dataset.msgId = msgId || '';
  el.querySelector('.msg-user-content').textContent = text;
  // 点击问题文本进入编辑模式（与编辑按钮共用同一回调）
  // 读取 dataset.fullText：版本切换后该字段为当前版本文本，避免编辑旧版本内容
  el.querySelector('.msg-user-content').addEventListener('click', () => {
    if (get('streaming') || get('auditingDocId')) return;
    triggerEdit(el, el.dataset.fullText || text);
  });
  showMsgActions(el, text, 'user');
  if (scroll) scrollToBottom();
  return el;
}

/**
 * 触发编辑模式（统一的编辑入口，点击文本和编辑按钮共用）。
 * @param {HTMLElement} blockEl - 用户消息块
 * @param {string} content - 原始内容
 * @returns {void}
 */
function triggerEdit(blockEl, content) {
  enterEditMode(blockEl, content, (newContent) => commitEdit(blockEl, content, newContent), editAttachHandler);
}

/**
 * 提交编辑内容：持久化新版本并就地重发（REQ-RAG-026）。
 *
 * 由 confirmEdit 调用（Enter / 发送按钮），newContent 为编辑后的文本。
 * 点击问题气泡与点击编辑按钮共用此回调，避免编辑按钮路径重新进入编辑模式。
 *
 * @param {HTMLElement} blockEl - 用户消息块
 * @param {string} originalContent - 原始内容（首次编辑时注册为 v1）
 * @param {string} newContent - 编辑后的内容
 * @returns {Promise<void>}
 */
async function commitEdit(blockEl, originalContent, newContent) {
  // 1. 确定或生成 turn_group
  let turnGroup = blockEl.dataset.turnGroup;
  const isFirstEdit = !turnGroup;
  if (isFirstEdit) {
    turnGroup = generateTurnGroupId();
    blockEl.dataset.turnGroup = turnGroup;
  }

  // 2. 持久化新版本到 DB（首次编辑：把原始问答升级为 v1，新内容为 v2）
  let newVersion;
  try {
    // 首次编辑时传原始消息行 id，供后端把原始问答升级为 version=1；
    // 刚发送的消息前端暂无 id，降级为查询「内容匹配的最后一条无 turn_group user 消息」，
    // 避免编辑历史消息时误选其他消息
    let originalMsgId = blockEl.dataset.msgId || null;
    if (isFirstEdit && !originalMsgId) {
      try {
        const msgs = await invoke('get_messages', { conversationId: get('currentConversationId') });
        const lastUngroupedUser = [...msgs].reverse().find(
          (m) => m.role === 'user' && !m.turn_group && m.content === originalContent,
        );
        originalMsgId = lastUngroupedUser?.id || null;
      } catch (queryErr) {
        console.warn('查询原始消息 id 失败:', queryErr);
      }
    }
    newVersion = await convApi.editUserMessage(
      get('currentConversationId'),
      turnGroup,
      newContent,
      originalMsgId,
    );
    blockEl.dataset.version = String(newVersion);
  } catch (err) {
    // DB 持久化失败不阻断编辑流程（降级为纯内存分支）
    console.warn('edit_user_message IPC 失败，降级为内存分支:', err);
    // 退化版本号
    const existingCount = getVersionCount(turnGroup);
    newVersion = existingCount + 1;
  }

  // 3. 更新轮次树：首次编辑先把原始问答注册为 v1
  if (isFirstEdit) {
    addVersion(turnGroup, 1, originalContent);
    const assistantEl = findAssistantBlock(blockEl);
    // renderMarkdown 会把原始 Markdown 写入 .md 的 dataset.rawMarkdown，
    // 这里直接读取即可捕获原始答案内容
    const rawMd = assistantEl?.querySelector('.md')?.dataset.rawMarkdown || '';
    const reasoning = assistantEl?._thinkingPanel?.getReasoning?.() || null;
    if (rawMd) {
      updateVersionAssistant(turnGroup, 1, rawMd, /** @type {any[] | null} */ (assistantEl?._sources) || null, reasoning);
    }
    // 编辑产生的是 v2（后端返回）；若后端失败降级为 v1，则不重复注册
    if (newVersion > 1) {
      addVersion(turnGroup, newVersion, newContent);
    }
  } else {
    addVersion(turnGroup, newVersion, newContent);
  }

  // 4. 移除后续的 followup 建议
  removeFollowups(blockEl);

  // 5. 就地重发（不创建新的 user/assistant block）
  await _chatActions.sendFromEdit?.(blockEl, newContent, turnGroup, newVersion);
}

/**
 * 编辑模式「回形针」按钮的文档上传处理。
 * @returns {Promise<void>}
 */
async function editAttachHandler() {
  try {
    const selected = await openDialog({
      multiple: true,
      filters: [{ name: t('import.file_filter'), extensions: ['md', 'txt', 'pdf', 'docx', 'html', 'htm', 'pptx', 'epub'] }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      await importPaths(paths, loadDocumentsSafe);
    }
  } catch (err) {
    toastError(err);
  }
}

/**
 * 文档导入后安全刷新列表。
 * @returns {Promise<void>}
 */
async function loadDocumentsSafe() {
  try {
    const docs = await invoke('get_documents');
    setState({ documents: docs });
  } catch (err) {
    eprintlnSafe(`刷新文档列表失败: ${err}`);
  }
}

/** 安全的 stderr 输出。 */
function eprintlnSafe(msg) {
  try {
    console.warn(msg);
  } catch (_) { /* 忽略 */ }
}

/**
 * 向助手消息块追加 AI 免责声明（DeepSeek 风格）。
 * @param {HTMLElement} assistantEl - 助手消息块根元素
 */
export function appendAiDisclaimer(assistantEl) {
  if (!assistantEl) return;
  if (assistantEl.querySelector('.ai-disclaimer')) return;
  const disclaimer = document.createElement('div');
  disclaimer.className = 'ai-disclaimer';
  disclaimer.textContent = t('chat.ai_disclaimer');
  assistantEl.appendChild(disclaimer);
}

// ============================================================
// 流式渲染调度
// ============================================================

/** 流式渲染最小帧间隔 ms（V3.1 P4-1）：rAF 合帧之上再加 fps 上限。
 *  每帧成本 = marked.parse + DOMPurify + innerHTML 全量重建（O(n)），
 *  token 高频到达时限帧可避免 60fps 满帧率下的 CPU 空转。 */
const STREAM_MIN_FRAME_MS = 66; // ≈15fps，视觉流畅度足够

/** 超长文本降频阈值与间隔：>32KB 时降到 ≈8fps */
const STREAM_LONG_TEXT_THRESHOLD = 32 * 1024;
const STREAM_LONG_FRAME_MS = 125;

let _lastStreamRenderAt = 0;
let _streamTimer = null;

/**
 * 调度一次流式渲染（V3.1 P4-1：rAF 合帧 + 最小帧间隔限频）。
 * 非流式调用方请直接调 renderMarkdown。
 */
export function scheduleRender() {
  if (get('renderScheduled')) return;

  // 帧间隔限频：距上次渲染不足最小间隔时，安排一个延时帧（合并不了的长尾 token）
  const len = (get('currentRawMarkdown') || '').length;
  const minInterval = len > STREAM_LONG_TEXT_THRESHOLD ? STREAM_LONG_FRAME_MS : STREAM_MIN_FRAME_MS;
  const elapsed = Date.now() - _lastStreamRenderAt;
  if (elapsed < minInterval) {
    if (_streamTimer) return; // 已有待执行的延时帧
    _streamTimer = setTimeout(() => {
      _streamTimer = null;
      scheduleRender();
    }, minInterval - elapsed);
    return;
  }

  setState({ renderScheduled: true });
  requestAnimationFrame(() => {
    setState({ renderScheduled: false });
    _lastStreamRenderAt = Date.now();
    const assistantEl = get('currentAssistantEl');
    if (!assistantEl) return;
    const mdEl = assistantEl.querySelector('.md');
    if (mdEl) {
      renderMarkdown(mdEl, get('currentRawMarkdown'), get('lastSources'), !!get('streaming'));
      showCaret(mdEl);
    }
    // 编辑场景（turnGroup 存在）：滚动保持被编辑的 assistant 块可见，而非跳到页面底部
    const isEditMode = assistantEl.dataset.turnGroup;
    if (isEditMode) {
      // 编辑中间消息时，仅确保当前 assistant 块底部可见
      assistantEl.scrollIntoView({ behavior: 'instant', block: 'nearest' });
    } else if (shouldAutoScroll()) {
      scrollToBottom();
      hideJumpToLatest();
    } else {
      showJumpToLatest();
    }
  });
}

// ============================================================
// 引用来源渲染
// ============================================================

/**
 * 渲染引用来源到当前 assistant Block 的 .sources 容器。
 * @param {Array<{doc_name: string, score: number, chunk: {content: string, chunk_index?: number}}>} sources
 */
export function renderSources(sources) {
  setState({ lastSources: sources });
  const el = get('currentAssistantEl');
  if (!el) return;
  // 保存来源引用到块（JS 属性），供编辑分支注册 v1 时读取
  el._sources = sources;
  const box = el.querySelector('.sources');
  box.innerHTML = '';
  box.className = 'sources mt-2';

  const sorted = [...sources].sort((a, b) => (b.score || 0) - (a.score || 0));

  const toggle = document.createElement('button');
  toggle.className = 'sources-toggle animate-fade-in';
  toggle.innerHTML = `<svg width="12" height="12" viewBox="0 0 16 16" fill="none" style="transition:transform .2s"><path d="M4 6l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg><span>${sources.length}${t('chat.sources_count')}</span>`;
  toggle.onclick = () => {
    const expanded = toggle.classList.toggle('expanded');
    toggle.querySelector('svg').style.transform = expanded ? 'rotate(180deg)' : '';
    list.style.display = expanded ? 'flex' : 'none';
  };

  const list = document.createElement('div');
  list.className = 'sources-list';
  list.style.display = 'none';
  sorted.forEach((s, i) => {
    const card = document.createElement('div');
    card.className = 'source-card';
    const header = document.createElement('div');
    header.className = 'source-card-header';
    const titleWrap = document.createElement('div');
    titleWrap.className = 'flex items-center gap-2 min-w-0';
    const numLabel = document.createElement('span');
    numLabel.className = 'source-card-chunk shrink-0';
    numLabel.textContent = `[${i + 1}]`;
    const title = document.createElement('span');
    title.className = 'source-card-title';
    title.textContent = s.doc_name || '';
    titleWrap.appendChild(numLabel);
    titleWrap.appendChild(title);
    const score = document.createElement('span');
    score.className = 'source-card-score shrink-0';
    score.textContent = `${Math.round((s.score || 0) * 100)}%`;
    header.appendChild(titleWrap);
    header.appendChild(score);
    card.appendChild(header);

    const previewText = (s.chunk?.content || '').slice(0, 100);
    if (previewText) {
      const preview = document.createElement('div');
      preview.className = 'source-card-preview';
      preview.textContent = previewText + (s.chunk.content.length > 100 ? '…' : '');
      card.appendChild(preview);
    }

    if (s.chunk?.chunk_index !== undefined) {
      const chunkLabel = document.createElement('div');
      chunkLabel.className = 'source-card-chunk';
      chunkLabel.textContent = `#${s.chunk.chunk_index}`;
      card.appendChild(chunkLabel);
    }

    const origIndex = sources.indexOf(s);
    card.dataset.sourceIndex = String(origIndex);
    card.dataset.chunkContent = s.chunk?.content || '';
    card.onclick = async () => {
      highlightCiteRef(el, origIndex);
      const ok = await copyToClipboard(s.chunk?.content || '');
      if (ok) {
        toast(t('chat.copied_to_clipboard'), 'success');
      } else {
        toastError(t('chat.copy_failed'));
      }
    };
    card.onmouseenter = () => { highlightCiteRef(el, origIndex, true); };
    card.onmouseleave = () => { clearCiteRefHighlight(el); };

    list.appendChild(card);
  });

  box.appendChild(toggle);
  box.appendChild(list);
}

/**
 * 高亮对话区中对应引用编号的 cite-ref 标记。
 * @param {HTMLElement} blockEl - 消息 Block 根元素
 * @param {number} sourceIndex - 引用来源索引（0-based）
 * @param {boolean} [persist=false] - 是否持久高亮
 */
function highlightCiteRef(blockEl, sourceIndex, persist = false) {
  const mdEl = blockEl.querySelector('.md');
  if (!mdEl) return;
  const citeNum = sourceIndex + 1;
  const refs = mdEl.querySelectorAll(`.cite-ref[data-cite="${citeNum}"]`);
  refs.forEach((ref) => {
    ref.classList.add('cite-ref-highlighted');
    if (!persist) {
      setTimeout(() => ref.classList.remove('cite-ref-highlighted'), 600);
    }
  });
}

/** 清除对话区中所有 cite-ref 的高亮状态。 */
function clearCiteRefHighlight(blockEl) {
  const mdEl = blockEl.querySelector('.md');
  if (!mdEl) return;
  mdEl.querySelectorAll('.cite-ref-highlighted').forEach((ref) => {
    ref.classList.remove('cite-ref-highlighted');
  });
}

// ============================================================
// 消息操作栏
// ============================================================

/**
 * 填充并显示消息操作栏（REQ-RAG-012）。
 * @param {HTMLElement} blockEl - 消息 Block 根元素
 * @param {string} rawMarkdown - 该消息的原始 Markdown 文本
 * @param {'user'|'assistant'} [role='assistant'] - 消息角色
 */
export function showMsgActions(blockEl, rawMarkdown, role = 'assistant') {
  let bar;
  if (role === 'user') {
    bar = blockEl.nextElementSibling;
    if (!bar || bar.dataset.role !== 'user-actions') bar = null;
  } else {
    bar = blockEl.querySelector('.msg-actions');
  }
  if (!bar) return;
  bar.innerHTML = '';

  if (role === 'user') {
    const copyBtn = document.createElement('button');
    copyBtn.className = 'msg-action-btn';
    copyBtn.title = t('chat.copy_all');
    copyBtn.setAttribute('aria-label', t('chat.copy_all'));
    copyBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
    copyBtn.onclick = async () => {
      // 优先复制当前显示的版本（版本切换后 dataset.fullText 为最新内容）
      const ok = await copyToClipboard(blockEl.dataset.fullText || rawMarkdown);
      if (ok) { toast(t('chat.copied_to_clipboard'), 'success'); }
      else { toastError(t('chat.copy_failed')); }
    };
    bar.appendChild(copyBtn);
  } else {
    const copyBtn = document.createElement('button');
    copyBtn.className = 'msg-action-btn';
    copyBtn.title = t('chat.copy_all');
    copyBtn.setAttribute('aria-label', t('chat.copy_all'));
    copyBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
    copyBtn.onclick = async () => {
      // 优先复制当前显示的版本（版本切换后 dataset 为最新内容）
      const mdEl = blockEl.querySelector('.md');
      const current = mdEl?.dataset.rawMarkdown || rawMarkdown;
      const ok = await copyToClipboard(current);
      if (ok) { toast(t('chat.copied_to_clipboard'), 'success'); }
      else { toastError(t('chat.copy_failed')); }
    };
    bar.appendChild(copyBtn);

    const copyPlainBtn = document.createElement('button');
    copyPlainBtn.className = 'msg-action-btn';
    copyPlainBtn.title = t('chat.copy_plain');
    copyPlainBtn.setAttribute('aria-label', t('chat.copy_plain'));
    copyPlainBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7V4h16v3"/><path d="M9 20h6"/><path d="M12 4v16"/></svg>';
    copyPlainBtn.onclick = async () => {
      const mdEl = blockEl.querySelector('.md');
      let plainText;
      if (mdEl) {
        const clone = mdEl.cloneNode(true);
        clone.querySelectorAll('.code-header, .copy-btn, .code-lang').forEach((el) => el.remove());
        plainText = clone.textContent || '';
      } else {
        plainText = rawMarkdown;
      }
      const ok = await copyToClipboard(plainText.trim());
      if (ok) { toast(t('chat.copied_to_clipboard'), 'success'); }
      else { toastError(t('chat.copy_failed')); }
    };
    bar.appendChild(copyPlainBtn);
  }

  if (role === 'user' && !get('auditingDocId') && !get('streaming')) {
    const editBtn = createEditButton(blockEl, rawMarkdown, (newContent) => {
      // 用当前显示的版本内容作为 originalContent（版本切换后闭包里的 rawMarkdown 已过期）
      commitEdit(blockEl, blockEl.dataset.fullText || rawMarkdown, newContent);
    }, editAttachHandler);
    bar.appendChild(editBtn);
  }

  if (role === 'assistant' && !get('auditingDocId')) {
    // TTS 朗读按钮（REQ-RAG-035）：在重新生成按钮之前插入
    const ttsBtn = createTtsButton(blockEl, rawMarkdown);
    if (ttsBtn) bar.appendChild(ttsBtn);

    // 消息级书签按钮（REQ-RAG-053）：在 TTS 之后、重新生成之前
    const msgId = blockEl.dataset.msgId;
    const convId = get('currentConversationId');
    if (msgId && convId) {
      const bookmarkBtn = document.createElement('button');
      bookmarkBtn.className = 'msg-action-btn msg-action-btn-bookmark';
      bookmarkBtn.title = t('chat.bookmark') || '书签';
      bookmarkBtn.setAttribute('aria-label', t('chat.bookmark') || '书签');
      bookmarkBtn.innerHTML = icon('book', 'sm');
      bookmarkBtn.onclick = async () => {
        try {
          // 检查是否已加书签
          const existing = await invoke('get_message_bookmark', { messageId: msgId });
          if (existing) {
            // 已有书签 → 删除
            await invoke('remove_bookmark', { conversationId: convId });
            bookmarkBtn.classList.remove('text-amber-400');
            bookmarkBtn.title = t('chat.bookmark') || '书签';
            toast(t('chat.bookmark_removed') || '已移除书签', 'info');
          } else {
            // 添加消息级书签
            const mdEl = blockEl.querySelector('.md');
            const content = mdEl?.dataset.rawMarkdown || rawMarkdown || ''
            const summary = content.slice(0, 50);
            await invoke('add_bookmark', {
              conversationId: convId,
              note: null,
              messageId: msgId,
              summary: summary,
            });
            bookmarkBtn.classList.add('text-amber-400');
            bookmarkBtn.title = t('chat.bookmark_remove') || '移除书签';
            toast(t('chat.bookmark_added') || '已添加书签', 'success');
          }
          // 刷新侧栏书签列表
          window.__refreshBookmarks?.();
        } catch (err) {
          toastError(err instanceof Error ? err.message : String(err));
        }
      };

      // 异步加载书签状态
      (async () => {
        try {
          const existing = await invoke('get_message_bookmark', { messageId: msgId });
          if (existing) {
            bookmarkBtn.classList.add('text-amber-400');
            bookmarkBtn.title = t('chat.bookmark_remove') || '移除书签';
          }
        } catch { /* 静默降级 */ }
      })();

      bar.appendChild(bookmarkBtn);
    }

    const regenBtn = document.createElement('button');
    regenBtn.className = 'msg-action-btn';
    regenBtn.title = t('chat.regenerate');
    regenBtn.setAttribute('aria-label', t('chat.regenerate'));
    regenBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>';
    /**
     * 执行重新生成，可选传入变体模式。
     * @param {'default'|'concise'|'detailed'} [variant] - 重新生成变体
     */
    function doRegenerate(variant = 'default') {
      const query = blockEl.dataset.query;
      if (!query) return;
      let carousel = getExistingCarousel(blockEl);
      if (!carousel) {
        carousel = createCarousel(blockEl);
        const mdEl = blockEl.querySelector('.md');
        const currentContent = mdEl ? mdEl.dataset.rawMarkdown || mdEl.textContent : '';
        const currentSources = get('lastSources');
        addCarouselVersion(carousel, currentContent, currentSources);
      }
      // 根据变体模式拼接查询前缀
      let actualQuery = query;
      if (variant === 'concise') {
        actualQuery = query + t('chat.regen_concise_prompt');
      } else if (variant === 'detailed') {
        actualQuery = query + t('chat.regen_detailed_prompt');
      }
      $('queryInput').value = actualQuery;
      // 存轮播引用到 assistantEl（JS 属性），流结束时由 finalizeStream 追加新版本
      blockEl._regenCarousel = carousel;
      _chatActions.send?.();
    }
    // 左键点击：默认重新生成；长按或右键：弹出变体菜单
    let _regenMenuTimer = null;
    regenBtn.onclick = (e) => {
      // 如果有变体菜单已打开，先关闭
      const existingMenu = document.querySelector('.regen-variant-menu');
      if (existingMenu) {
        existingMenu.remove();
        return;
      }
      doRegenerate('default');
    };
    // 右键弹出变体菜单
    regenBtn.oncontextmenu = (e) => {
      e.preventDefault();
      _showRegenVariantMenu(e.clientX, e.clientY, doRegenerate);
    };
    // 长按弹出变体菜单（移动端友好）
    regenBtn.onmousedown = () => {
      _regenMenuTimer = setTimeout(() => {
        const rect = regenBtn.getBoundingClientRect();
        _showRegenVariantMenu(rect.left, rect.bottom + 4, doRegenerate);
      }, 500);
    };
    regenBtn.onmouseup = () => { if (_regenMenuTimer) clearTimeout(_regenMenuTimer); };
    regenBtn.onmouseleave = () => { if (_regenMenuTimer) clearTimeout(_regenMenuTimer); };
    bar.appendChild(regenBtn);
  }

  // 删除消息按钮（REQ-RAG-013）：user 和 assistant 均可删除
  if (!get('streaming') && !get('auditingDocId')) {
    const msgId = blockEl.dataset.msgId;
    const convId = get('currentConversationId');
    if (msgId && convId) {
      const delBtn = document.createElement('button');
      delBtn.className = 'msg-action-btn msg-action-btn-danger';
      delBtn.title = t('chat.delete_message') || '删除';
      delBtn.setAttribute('aria-label', t('chat.delete_message') || '删除');
      delBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>';
      delBtn.onclick = async () => {
        const isUser = role === 'user';
        const confirmMsg = isUser
          ? (t('chat.delete_user_confirm') || '将同时删除对应的 AI 回答，确定删除？')
          : (t('chat.delete_assistant_confirm') || '确定删除这条消息？');
        const confirmed = await showConfirmDialog({
          title: t('chat.delete_message') || '删除消息',
          body: confirmMsg,
          confirmText: t('common.delete') || '删除',
          cancelText: t('common.cancel') || '取消',
          danger: true,
        });
        if (!confirmed) return;
        try {
          await convApi.deleteMessage(convId, msgId);
          // 从 DOM 移除消息块
          if (isUser) {
            // user 消息：移除自身 + 下一个 assistant 块 + 操作栏
            const actionsBar = blockEl.nextElementSibling;
            const assistantBlock = actionsBar?.nextElementSibling;
            if (actionsBar && actionsBar.dataset.role === 'user-actions') actionsBar.remove();
            if (assistantBlock && assistantBlock.classList.contains('md-block')) assistantBlock.remove();
            blockEl.remove();
          } else {
            // assistant 消息：仅移除自身
            blockEl.remove();
          }
          toast(t('chat.deleted') || '已删除', 'success');
        } catch (err) {
          toastError(err instanceof Error ? err.message : String(err));
        }
      };
      bar.appendChild(delBtn);
    }
  }
}

// ============================================================
// 滚动控制
// ============================================================

/**
 * 平滑滚动聊天区到底部。
 */
export function scrollToBottom() {
  const area = $('chatArea');
  beginProgrammaticScroll();
  // 流式期间使用 instant 滚动：smooth 滚动在内容持续增长时会竞争动画导致抖动
  const smooth = !get('streaming');
  area.scrollTo({ top: area.scrollHeight, behavior: smooth ? 'smooth' : 'instant' });
  resetScrollLock();
  hideJumpToLatest();
}

/**
 * 显示「回到底部」浮动按钮。
 */
function showJumpToLatest() {
  if (!_jumpToLatestBtn) {
    _jumpToLatestBtn = createJumpToLatestButton();
    const chatWrapper = $('chatArea')?.parentElement;
    if (chatWrapper) {
      chatWrapper.style.position = 'relative';
      chatWrapper.appendChild(_jumpToLatestBtn);
    }
  }
  if (_jumpToLatestBtn) _jumpToLatestBtn.style.display = 'flex';
}

/** 隐藏「回到底部」浮动按钮。 */
function hideJumpToLatest() {
  if (_jumpToLatestBtn) _jumpToLatestBtn.style.display = 'none';
}

// ============================================================
// 上下文折叠提示（REQ-RAG-017）
// ============================================================

/**
 * 显示「部分历史已折叠」提示并隐藏被截断的消息块。
 * @param {{ truncated_count, total_tokens, retained_tokens, token_limit }} payload
 */
export function showCollapsedHint(payload) {
  const { truncated_count, total_tokens, retained_tokens, token_limit } = payload;
  if (truncated_count === 0) return;

  const chatArea = $('chatArea');
  if (!chatArea) return;
  const blocks = chatArea.querySelectorAll('.animate-message-in');
  if (blocks.length < 2 + truncated_count) return;

  const startIndex = 2;
  const endIndex = 1 + truncated_count;
  for (let i = startIndex; i <= endIndex; i++) {
    const block = blocks[i];
    if (block) {
      block.dataset.collapsed = 'true';
      block.style.display = 'none';
    }
  }

  const hint = document.createElement('div');
  hint.className = 'context-collapsed-hint text-xs text-text-quaternary py-2 text-center border-b border-border-subtle';
  const hintText = t('chat.context_collapsed', { count: truncated_count });
  hint.innerHTML = `<span class="cursor-pointer hover:text-accent">${hintText}</span> <span class="text-text-quaternary/60 ml-2">(${retained_tokens} / ${total_tokens} tokens)</span>`;
  hint.dataset.expanded = 'false';

  hint.onclick = () => {
    const isExpanded = hint.dataset.expanded === 'true';
    if (isExpanded) {
      hint.dataset.expanded = 'false';
      for (let i = startIndex; i <= endIndex; i++) {
        const block = blocks[i];
        if (block) { block.dataset.collapsed = 'true'; }
      }
      setTimeout(() => {
        for (let i = startIndex; i <= endIndex; i++) {
          const block = blocks[i];
          if (block) { block.style.display = 'none'; }
        }
      }, 0);
    } else {
      hint.dataset.expanded = 'true';
      for (let i = startIndex; i <= endIndex; i++) {
        const block = blocks[i];
        if (block) {
          block.dataset.collapsed = 'false';
          block.style.display = '';
        }
      }
    }
  };

  const insertAfter = blocks[1] || blocks[0];
  if (insertAfter) {
    insertAfter.after(hint);
  }
}

// ============================================================
// 流式光标管理（原 caret.js 已合并到本模块）
// ============================================================

/** 光标 Tailwind 工具类集合。 */
const CARET_CLASS = 'inline-block w-[2px] h-[1.1em] bg-accent ml-[2px] animate-caret-blink align-text-bottom rounded-[1px] motion-reduce:animate-none motion-reduce:opacity-50';

/**
 * 在指定容器末尾显示流式光标。
 * @param {HTMLElement} container - Markdown 渲染容器（.md 元素）
 */
export function showCaret(container) {
  if (!container) return;
  if (container.querySelector('[data-stream-caret]')) return;

  const caret = document.createElement('span');
  caret.className = CARET_CLASS;
  caret.setAttribute('data-stream-caret', '');
  caret.setAttribute('aria-hidden', 'true');
  caret.textContent = '▋';
  container.appendChild(caret);
}

/**
 * 从指定容器移除流式光标。
 * @param {HTMLElement} container - Markdown 渲染容器
 */
export function removeCaret(container) {
  if (!container) return;
  const caret = container.querySelector('[data-stream-caret]');
  if (caret) caret.remove();
}

/**
 * 检查指定容器中是否活跃着流式光标。
 * @param {HTMLElement} container - Markdown 渲染容器
 * @returns {boolean}
 */
export function isCaretActive(container) {
  if (!container) return false;
  return container.querySelector('[data-stream-caret]') !== null;
}

// ============================================================
// 隐私状态可视化（原 privacy-indicator.js 已合并到本模块）
// ============================================================

/** 隐私状态文案 i18n key 映射（V3.1 P3-3：原硬编码中文改为三语言 key） */
export const PRIVACY_STATUS_TEXT = {
  encrypted: 'privacy.encrypted',
  notEncrypted: 'privacy.not_encrypted',
  piiOn: 'privacy.pii_on',
  piiOff: 'privacy.pii_off',
  auditChainOk: 'privacy.audit_chain_ok',
  auditChainOff: 'privacy.audit_chain_off',
};

/**
 * 从 state 读取当前隐私状态。
 * @returns {{encrypted: boolean, piiEnabled: boolean, locked: boolean, auditEnabled: boolean}}
 */
export function getPrivacyStatus() {
  const securityState = get('securityState') || 'unencrypted';
  const piiDetectionEnabled = get('piiDetectionEnabled') || false;
  return {
    encrypted: securityState !== 'unencrypted',
    piiEnabled: piiDetectionEnabled,
    locked: securityState === 'locked',
    auditEnabled: securityState !== 'unencrypted',
  };
}

/**
 * 格式化隐私状态文案。
 * @param {{encrypted: boolean, piiEnabled: boolean, auditEnabled: boolean}} status
 * @returns {{encryption: string, pii: string, audit: string}}
 */
export function formatPrivacyText(status) {
  const encryption = status.encrypted
    ? t(PRIVACY_STATUS_TEXT.encrypted)
    : t(PRIVACY_STATUS_TEXT.notEncrypted);

  const pii = status.piiEnabled
    ? t(PRIVACY_STATUS_TEXT.piiOn)
    : t(PRIVACY_STATUS_TEXT.piiOff);

  const audit = status.auditEnabled
    ? t(PRIVACY_STATUS_TEXT.auditChainOk)
    : t(PRIVACY_STATUS_TEXT.auditChainOff);

  return { encryption, pii, audit };
}

/**
 * 在指定容器中渲染隐私状态指示器。
 * @param {HTMLElement} container - 目标容器
 * @param {Object} [options] - 选项
 * @param {Function} [options.onClick] - 点击加密状态时的回调
 * @returns {HTMLElement}
 */
export function renderPrivacyIndicator(container, options = {}) {
  const status = getPrivacyStatus();
  const text = formatPrivacyText(status);

  const existing = container.querySelector('.privacy-indicator');
  if (existing) existing.remove();

  const indicator = document.createElement('div');
  indicator.className = 'privacy-indicator';

  const encryptionEl = document.createElement('span');
  encryptionEl.className = 'privacy-tag privacy-encryption';
  if (status.encrypted) {
    encryptionEl.classList.add('privacy-encrypted');
  } else {
    encryptionEl.classList.add('privacy-not-encrypted');
  }

  const encIcon = document.createElement('span');
  encIcon.className = 'privacy-encryption-icon';
  encIcon.innerHTML = status.encrypted ? '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>' : '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></svg>';
  const encText = document.createElement('span');
  encText.textContent = text.encryption;
  encryptionEl.appendChild(encIcon);
  encryptionEl.appendChild(encText);

  if (options.onClick) {
    encryptionEl.style.cursor = 'pointer';
    encryptionEl.onclick = () => options.onClick();
  }

  const piiEl = document.createElement('span');
  piiEl.className = 'privacy-tag privacy-pii';
  piiEl.classList.toggle('pii-on', status.piiEnabled);
  piiEl.classList.toggle('pii-off', !status.piiEnabled);
  const piiIcon = document.createElement('span');
  piiIcon.innerHTML = icon('shield', 'sm');
  const piiText = document.createElement('span');
  piiText.textContent = text.pii;
  piiEl.appendChild(piiIcon);
  piiEl.appendChild(piiText);

  const auditEl = document.createElement('span');
  auditEl.className = 'privacy-tag privacy-audit';
  auditEl.classList.toggle('audit-ok', status.auditEnabled);
  const auditIcon = document.createElement('span');
  auditIcon.innerHTML = '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg>';
  const auditText = document.createElement('span');
  auditText.textContent = text.audit;
  auditEl.appendChild(auditIcon);
  auditEl.appendChild(auditText);

  indicator.appendChild(encryptionEl);
  indicator.appendChild(piiEl);
  indicator.appendChild(auditEl);

  container.appendChild(indicator);
  return indicator;
}

// ============================================================
// 重新生成轮播（原 regen-carousel.js，AC-QA-014）
// ============================================================

/**
 * @typedef {Object} CarouselVersion
 * @property {string} content - Markdown 原文
 * @property {Array|null} sources - 引用来源
 */

/**
 * @typedef {Object} CarouselState
 * @property {number} current - 当前选中版本索引
 * @property {number} total - 版本总数
 * @property {CarouselVersion[]} versions - 所有版本
 */

/**
 * 在 assistant 消息块中创建轮播容器。
 */
export function createCarousel(blockEl) {
  const existing = blockEl.querySelector('.regen-carousel');
  if (existing) return existing;

  const carousel = document.createElement('div');
  carousel.className = 'regen-carousel';
  carousel.dataset.currentIndex = '0';
  carousel.dataset.total = '0';

  /** @type {CarouselVersion[]} */
  carousel._versions = [];

  const content = blockEl.querySelector('.msg-content');
  if (content && content.nextSibling) {
    content.parentNode.insertBefore(carousel, content.nextSibling);
  } else if (content) {
    content.parentNode.appendChild(carousel);
  } else {
    blockEl.appendChild(carousel);
  }

  return carousel;
}

/**
 * 向轮播中添加一个版本。
 */
export function addCarouselVersion(carousel, content, sources) {
  if (!carousel._versions) {
    carousel._versions = [];
  }

  /** @type {CarouselVersion} */
  const version = { content, sources };
  carousel._versions.push(version);

  const newIndex = carousel._versions.length - 1;
  carousel.dataset.total = String(carousel._versions.length);
  carousel.dataset.currentIndex = String(newIndex);

  if (carousel._versions.length > 1) {
    renderCarouselControls(carousel);
  }

  return newIndex;
}

/**
 * 在轮播中导航到上一个或下一个版本。
 */
export function navigateCarousel(carousel, direction) {
  const total = carousel._versions ? carousel._versions.length : 0;
  if (total <= 1) return;

  let current = parseInt(carousel.dataset.currentIndex || '0', 10);

  if (direction === 'left') {
    current = (current - 1 + total) % total;
  } else {
    current = (current + 1) % total;
  }

  carousel.dataset.currentIndex = String(current);
  updateControlsDisplay(carousel);
}

/**
 * 获取轮播当前状态。
 */
export function getCarouselState(carousel) {
  const versions = /** @type {CarouselVersion[]} */ (carousel._versions || []);
  const current = parseInt(carousel.dataset.currentIndex || '0', 10);
  return {
    current,
    total: versions.length,
    versions,
  };
}

/**
 * 更新 .md 容器显示当前选中版本的内容。
 */
export function updateCarouselDisplay(carousel, mdContainer) {
  const state = getCarouselState(carousel);
  if (state.total === 0 || !state.versions[state.current]) return;

  const version = state.versions[state.current];
  mdContainer.textContent = version.content;
  mdContainer.dataset.rawMarkdown = version.content;

  const blockEl = carousel.closest('.msg-block');
  if (blockEl) {
    const sourcesEl = blockEl.querySelector('.sources');
    if (sourcesEl && version.sources) {
      blockEl.dataset.pendingSources = JSON.stringify(version.sources);
    }
  }
}

/**
 * 渲染轮播控制栏（◀ 1/N ▶）。
 */
export function renderCarouselControls(carousel) {
  const total = carousel._versions ? carousel._versions.length : 0;
  if (total === 0) return null;

  const existing = carousel.querySelector('.regen-carousel-controls');
  if (existing) existing.remove();

  const controls = document.createElement('div');
  controls.className = 'regen-carousel-controls';

  const prevBtn = document.createElement('button');
  prevBtn.className = 'regen-carousel-btn regen-carousel-prev';
  prevBtn.innerHTML = '◀';
  prevBtn.setAttribute('aria-label', t('chat.regen_prev') || '上一个版本');
  prevBtn.onclick = () => navigateCarousel(carousel, 'left');

  const current = parseInt(carousel.dataset.currentIndex || '0', 10);
  const counter = document.createElement('span');
  counter.className = 'regen-carousel-counter';
  counter.textContent = `${current + 1}/${total}`;

  const nextBtn = document.createElement('button');
  nextBtn.className = 'regen-carousel-btn regen-carousel-next';
  nextBtn.innerHTML = '▶';
  nextBtn.setAttribute('aria-label', t('chat.regen_next') || '下一个版本');
  nextBtn.onclick = () => navigateCarousel(carousel, 'right');

  controls.appendChild(prevBtn);
  controls.appendChild(counter);
  controls.appendChild(nextBtn);

  carousel.insertBefore(controls, carousel.firstChild);

  return controls;
}

/**
 * 更新控制栏的显示状态。
 */
function updateControlsDisplay(carousel) {
  const counter = carousel.querySelector('.regen-carousel-counter');
  const state = getCarouselState(carousel);
  if (counter) {
    counter.textContent = `${state.current + 1}/${state.total}`;
  }
}

/**
 * 检查消息块是否已有轮播容器。
 */
export function getExistingCarousel(blockEl) {
  return blockEl.querySelector('.regen-carousel') || null;
}

// ============================================================
// 后续问题建议（followup.js 已合并到此）
// ============================================================

/**
 * EchoMind 后续问题建议模块 — 回答完成后展示 2-3 个后续追问。
 *
 * 职责：
 * 1. 从 assistant 回答中提取关键实体/概念（纯规则，零 LLM 调用）
 * 2. 基于模板生成后续问题建议（3 种策略：实体追问 / 对比 / 例外）
 * 3. 渲染后续建议卡片（可点击发送 + 可关闭）
 * 4. 最多 3 条建议，避免选择困难
 */

/** 后续建议最大数量 */
const MAX_SUGGESTIONS = 3;

const FOLLOWUP_TEMPLATES = {
  entity_deep: [
    '关于 {entity} 还有哪些规定？',
    '{entity} 的具体适用条件是什么？',
    '{entity} 的计算方式是怎样的？',
    '{entity} 有哪些例外情况？',
    'What else should I know about {entity}?',
    'What are the specific conditions for {entity}?',
  ],
  compare: [
    '对比 {entity} 和其他相关规定的异同',
    'Compare {entity} with related regulations',
  ],
  exception: [
    '{entity} 有哪些例外或特殊情形？',
    'What are the exceptions to {entity}?',
  ],
};

const STOP_WORDS = new Set([
  '的', '了', '是', '在', '和', '与', '或', '为', '以', '及', '等', '中',
  '可', '以', '对', '从', '到', '由', '将', '被', '让', '使', '给', '向',
  '这', '那', '其', '此', '该', '之', '所', '一', '不', '无', '有',
  '可以', '应当', '不得', '或者', '但是', '如果', '因此', '所以',
  '根据', '关于', '对于', '按照', '依据', '以下', '以上', '其中',
  '本文', '本条', '本法', '规定', '情形', '以下',
  'the', 'a', 'an', 'is', 'are', 'was', 'were', 'be', 'been', 'being',
  'and', 'or', 'but', 'if', 'then', 'else', 'when', 'where', 'why', 'how',
  'this', 'that', 'these', 'those', 'it', 'its', 'they', 'them', 'their',
  'to', 'of', 'in', 'on', 'at', 'by', 'for', 'with', 'from', 'as', 'such',
  'can', 'may', 'must', 'should', 'would', 'could', 'will', 'shall',
]);

/**
 * 从文本中提取关键实体/概念。
 * @param {string} text - assistant 回答文本
 * @param {number} [maxEntities=5] - 最大实体数量
 * @returns {string[]} 提取的实体列表
 */
export function extractEntities(text, maxEntities = 5) {
  if (!text || typeof text !== 'string') return [];
  const entities = [];
  const seen = new Set();
  const bookMatches = text.matchAll(/《([^》]{2,20})》/g);
  for (const m of bookMatches) {
    const entity = m[1].trim();
    if (entity && !seen.has(entity)) { seen.add(entity); entities.push(entity); }
    if (entities.length >= maxEntities) return entities;
  }
  const quoteMatches = text.matchAll(/[""「]([^""」]{2,30})[""」]/g);
  for (const m of quoteMatches) {
    const entity = m[1].trim();
    if (entity && !seen.has(entity) && !STOP_WORDS.has(entity)) { seen.add(entity); entities.push(entity); }
    if (entities.length >= maxEntities) return entities;
  }
  const headingMatches = text.matchAll(/^#{1,6}\s+(.+)$/gm);
  for (const m of headingMatches) {
    const entity = m[1].trim();
    if (entity && !seen.has(entity) && !STOP_WORDS.has(entity)) { seen.add(entity); entities.push(entity); }
    if (entities.length >= maxEntities) return entities;
  }
  const cjkMatches = text.matchAll(/[\u4e00-\u9fff]{2,6}/g);
  const cjkFreq = new Map();
  for (const m of cjkMatches) {
    const term = m[0];
    if (!STOP_WORDS.has(term)) { cjkFreq.set(term, (cjkFreq.get(term) || 0) + 1); }
  }
  const sortedCjk = [...cjkFreq.entries()].sort((a, b) => b[1] - a[1]).map(([term]) => term);
  for (const term of sortedCjk) {
    if (entities.length >= maxEntities) break;
    if (!seen.has(term) && !STOP_WORDS.has(term)) { seen.add(term); entities.push(term); }
  }
  if (entities.length < maxEntities) {
    const engMatches = text.matchAll(/\b([A-Z][a-z]{2,}|[A-Z]{2,})\b/g);
    for (const m of engMatches) {
      if (entities.length >= maxEntities) break;
      const term = m[1];
      if (!seen.has(term) && !STOP_WORDS.has(term.toLowerCase())) { seen.add(term); entities.push(term); }
    }
  }
  return entities.slice(0, maxEntities);
}

/**
 * 基于提取的实体和用户原始查询生成后续问题建议。
 * @param {string[]} entities
 * @param {string} [userQuery='']
 * @returns {string[]} 后续问题建议列表
 */
export function generateFollowups(entities, userQuery = '') {
  if (!entities || entities.length === 0) {
    return [
      t('chat.followup_generic_1') || '还有哪些相关内容？',
      t('chat.followup_generic_2') || '能否更详细地解释？',
      t('chat.followup_generic_3') || '有什么需要注意的？',
    ].slice(0, MAX_SUGGESTIONS);
  }
  const suggestions = [];
  const seen = new Set();
  if (entities[0]) {
    const q = FOLLOWUP_TEMPLATES.entity_deep[0].replace('{entity}', entities[0]);
    if (!seen.has(q)) { seen.add(q); suggestions.push(q); }
  }
  if (entities[0] && suggestions.length < MAX_SUGGESTIONS) {
    const q = FOLLOWUP_TEMPLATES.exception[0].replace('{entity}', entities[0]);
    if (!seen.has(q)) { seen.add(q); suggestions.push(q); }
  }
  if (entities[1] && suggestions.length < MAX_SUGGESTIONS) {
    const q = FOLLOWUP_TEMPLATES.entity_deep[1].replace('{entity}', entities[1]);
    if (!seen.has(q)) { seen.add(q); suggestions.push(q); }
  }
  if (entities.length >= 2 && suggestions.length < MAX_SUGGESTIONS) {
    const q = FOLLOWUP_TEMPLATES.compare[0].replace('{entity}', `${entities[0]}与${entities[1]}`);
    if (!seen.has(q)) { seen.add(q); suggestions.push(q); }
  }
  let templateIdx = 2;
  let entityIdx = 0;
  while (suggestions.length < MAX_SUGGESTIONS && entityIdx < entities.length) {
    const entity = entities[entityIdx];
    const templates = FOLLOWUP_TEMPLATES.entity_deep;
    if (templateIdx < templates.length) {
      const q = templates[templateIdx].replace('{entity}', entity);
      if (!seen.has(q)) { seen.add(q); suggestions.push(q); }
      templateIdx++;
    } else {
      entityIdx++;
      templateIdx = 0;
    }
  }
  return suggestions.slice(0, MAX_SUGGESTIONS);
}

/**
 * 在 assistant 消息块下方渲染后续问题建议卡片。
 * @param {HTMLElement} blockEl
 * @param {string[]} suggestions
 * @param {function} onPick
 * @returns {HTMLElement|null}
 */
export function renderFollowups(blockEl, suggestions, onPick) {
  if (!blockEl) return null;
  if (!suggestions || suggestions.length === 0) return null;
  removeFollowups(blockEl);
  const container = document.createElement('div');
  container.className = 'followup-suggestions';
  const header = document.createElement('div');
  header.className = 'followup-header';
  const titleText = t('chat.followup_title') || '💡 你可能还想问：';
  header.innerHTML = `<span class="followup-title-text">${titleText}</span>`;
  const closeBtn = document.createElement('button');
  closeBtn.className = 'followup-close-btn';
  closeBtn.setAttribute('aria-label', t('common.close') || '关闭');
  closeBtn.innerHTML = '✕';
  closeBtn.onclick = (e) => { e.stopPropagation(); removeFollowups(blockEl); };
  header.appendChild(closeBtn);
  container.appendChild(header);
  const list = document.createElement('div');
  list.className = 'followup-list';
  suggestions.forEach((text) => {
    const card = document.createElement('button');
    card.className = 'followup-card';
    card.textContent = text;
    card.onclick = () => { if (onPick) onPick(text); };
    list.appendChild(card);
  });
  container.appendChild(list);
  blockEl.appendChild(container);
  return container;
}

/**
 * 移除指定消息块中的后续建议容器。
 * @param {HTMLElement} blockEl
 */
export function removeFollowups(blockEl) {
  if (!blockEl) return;
  const existing = blockEl.querySelector('.followup-suggestions');
  if (existing) existing.remove();
}

/**
 * 从 assistant 回答文本生成后续问题建议并渲染。
 * @param {HTMLElement} blockEl
 * @param {string} answerText
 * @param {string} [userQuery='']
 * @param {function} [onPick]
 * @returns {HTMLElement|null}
 */
export function renderFollowupSuggestions(blockEl, answerText, userQuery = '', onPick) {
  const entities = extractEntities(answerText);
  const suggestions = generateFollowups(entities, userQuery);
  return renderFollowups(blockEl, suggestions, onPick);
}