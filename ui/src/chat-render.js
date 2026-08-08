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
import { showCaret, removeCaret } from './caret.js';
import { shouldAutoScroll, resetScrollLock, createJumpToLatestButton, beginProgrammaticScroll } from './scroll-lock.js';
import { send } from './chat.js';
import { renderEmptyState } from './empty-state.js';
import { renderPrivacyIndicator } from './privacy-indicator.js';
import { createThinkingPanel } from './thinking-panel.js';
import { createEditButton, enterEditMode, renderBranchPagination, findAssistantBlock } from './message-edit.js';
import { removeFollowups } from './followup.js';
import { createTtsButton } from './voice-input.js';
import { getExistingCarousel, createCarousel, addCarouselVersion } from './regen-carousel.js';
import { generateTurnGroupId, getTurn, getActiveVersion, getVersionCount, setTurnTree, buildTurnTree, buildHistoryFromTurns, addVersion, updateVersionAssistant } from './turn-tree.js';
import { convApi } from './ipc.js';
import { sendFromEdit } from './chat.js';

// ============================================================
// 模块级状态
// ============================================================

/** 「回到底部」浮动按钮（null 表示未创建）。 */
let _jumpToLatestBtn = null;

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
  if (iconEl) iconEl.textContent = mode === 'local' ? '💻' : '☁️';
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
  renderEmptyState(chatArea, {
    onPickQuestion: (question) => {
      const input = $('queryInput');
      if (input) {
        input.value = question;
        input.focus();
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
export function appendBlock(role) {
  hideEmptyState();
  const wrap = document.createElement('div');
  wrap.className = `msg-block message-in msg-${role} animate-message-in`;
  if (role === 'user') {
    // DeepSeek 风格：用户消息纯文本右对齐（无气泡背景/边框）
    wrap.className = 'msg-block message-in msg-user animate-message-in w-fit max-w-[var(--msg-user-max-width)] ml-auto mr-0 mb-1 group';
    wrap.innerHTML = `<div class="msg-content msg-user-content text-text-primary text-base leading-normal whitespace-pre-wrap break-words cursor-text" title="${t('chat.edit') || '点击编辑'}"></div>`;
  } else {
    wrap.className = 'msg-block message-in msg-assistant animate-message-in w-full mb-8 group';
    const thinkingPanel = createThinkingPanel();
    wrap.innerHTML = `
      <div class="msg-content">
        <div class="md text-text-primary text-base leading-normal"></div>
        <div class="sources mt-2"></div>
      </div>
      <div class="flex gap-1 mt-2 opacity-0 group-hover:opacity-100 transition-opacity duration-fast msg-actions"></div>`;
    wrap.querySelector('.msg-content').insertBefore(thinkingPanel.container, wrap.querySelector('.md'));
    wrap._thinkingPanel = thinkingPanel;
  }
  $('chatArea').appendChild(wrap);
  if (role === 'user') {
    const actions = document.createElement('div');
    actions.className = 'flex justify-end gap-1 mt-1 transition-opacity duration-fast';
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
export function appendUserBlock(text, msgId = null) {
  const el = appendBlock('user');
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
  scrollToBottom();
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
  await sendFromEdit(blockEl, newContent, turnGroup, newVersion);
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
    const docs = await invoke('list_documents');
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

/**
 * 调度一次 rAF 渲染周期（防抖：已有待执行帧时跳过）。
 */
export function scheduleRender() {
  if (get('renderScheduled')) return;
  setState({ renderScheduled: true });
  requestAnimationFrame(() => {
    setState({ renderScheduled: false });
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
    copyBtn.className = 'msg-action-btn p-1.5 rounded-md text-text-secondary hover:text-accent hover:bg-accent/10 transition-colors duration-fast';
    copyBtn.title = t('chat.copy_all');
    copyBtn.setAttribute('aria-label', t('chat.copy_all'));
    copyBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
    copyBtn.onclick = async () => {
      // 优先复制当前显示的版本（版本切换后 dataset.fullText 为最新内容）
      const ok = await copyToClipboard(blockEl.dataset.fullText || rawMarkdown);
      if (ok) { toast(t('chat.copied_to_clipboard'), 'success'); }
      else { toastError(t('chat.copy_failed')); }
    };
    bar.appendChild(copyBtn);
  } else {
    const copyBtn = document.createElement('button');
    copyBtn.className = 'msg-action-btn p-1.5 rounded-md text-text-secondary hover:text-accent hover:bg-accent/10 transition-colors duration-fast';
    copyBtn.title = t('chat.copy_all');
    copyBtn.setAttribute('aria-label', t('chat.copy_all'));
    copyBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
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
    copyPlainBtn.className = 'msg-action-btn p-1.5 rounded-md text-text-secondary hover:text-accent hover:bg-accent/10 transition-colors duration-fast';
    copyPlainBtn.title = t('chat.copy_plain');
    copyPlainBtn.setAttribute('aria-label', t('chat.copy_plain'));
    copyPlainBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 7V4h16v3"/><path d="M9 20h6"/><path d="M12 4v16"/></svg>';
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

    const regenBtn = document.createElement('button');
    regenBtn.className = 'msg-action-btn p-1.5 rounded-md text-text-secondary hover:text-accent hover:bg-accent/10 transition-colors duration-fast';
    regenBtn.title = t('chat.regenerate');
    regenBtn.setAttribute('aria-label', t('chat.regenerate'));
    regenBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>';
    regenBtn.onclick = () => {
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
      $('queryInput').value = query;
      // 存轮播引用到 assistantEl（JS 属性），流结束时由 finalizeStream 追加新版本
      blockEl._regenCarousel = carousel;
      send();
    };
    bar.appendChild(regenBtn);
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
