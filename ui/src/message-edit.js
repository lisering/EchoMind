/**
 * EchoMind 用户消息编辑 + 分支模块 — 原地编辑用户消息并分叉对话。
 *
 * 职责：
 * 1. 点击「编辑」按钮将用户消息变为可编辑 textarea
 * 2. 「取消」按钮恢复原文
 * 3. 「发送」按钮 → 从该消息位置分叉，更新 Q&A 到当前版本
 * 4. 分页器 `◀ 1/N ▶` 切换查看不同版本（就地替换 Q&A）
 *
 * 键盘行为与聊天输入框完全一致：
 * - Enter（无 Shift）→ 发送
 * - Shift+Enter → 换行
 * - Escape → 取消
 *
 * 分支数据来源于 turn-tree（DB 持久化），非 DOM 元素快照。
 */

import { t } from './i18n.js';
import {
  getTurn, getActiveVersion, getVersionCount,
  setActiveVersion,
} from './turn-tree.js';
import { convApi } from './ipc.js';
import { get } from './state.js';
import { renderMarkdown } from './markdown.js';
import { renderRichContent } from './markdown.js';
import { createImeGuard, isComposingEvent } from './input-utils.js';

// ============================================================
// 状态管理
// ============================================================

/** 当前是否处于编辑模式 */
let _editingBlock = null;

/** 当前编辑块的重新发送回调 */
let _onResendCallback = null;

/** 当前编辑块的文档上传回调（回形针按钮） */
let _onAttachCallback = null;

/** 全局点击监听（用于点击外部取消编辑） */
let _outsideClickHandler = null;

// ============================================================
// 编辑模式控制
// ============================================================

/**
 * 进入编辑模式：将用户消息内容区替换为 textarea + 操作按钮。
 *
 * 操作栏布局：`[📎 上传文档] [取消] [发送]`
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @param {string} originalContent - 原始消息文本
 * @param {function} [onResend] - 重新发送回调（参数为编辑后的文本）
 * @param {function} [onAttach] - 文档上传回调（点击回形针按钮触发，打开文件选择器）
 * @returns {HTMLElement|null} 创建的编辑器容器元素（null=已在编辑模式）
 */
export function enterEditMode(blockEl, originalContent, onResend, onAttach) {
  if (!blockEl) return null;
  // 如果已经在编辑此块，不重复进入
  if (_editingBlock === blockEl) return null;
  // 如果正在编辑其他块，先退出
  if (_editingBlock) {
    exitEditMode(_editingBlock);
  }
  _editingBlock = blockEl;
  _onResendCallback = onResend;
  _onAttachCallback = onAttach;
  blockEl.classList.add('editing');

  const contentEl = blockEl.querySelector('.msg-user-content');
  if (!contentEl) return null;

  // 保存原始内容（用于取消恢复）
  blockEl.dataset.editingOriginal = originalContent;
  // 测量原始内容高度（必须在任何 DOM 修改之前，保证高度精确）
  const contentHeight = contentEl.offsetHeight;
  // 就地编辑：textarea 保持气泡的 padding/bg/radius，高度与气泡内容一致
  const baseHeight = contentHeight;
  // 隐藏原始内容：仅 display:none（不参与布局），由 textarea 以完全相同的
  // 高度接管其文档流位置 → 消息块尺寸不变 → 零布局抖动
  contentEl.style.display = 'none';
  // 用户消息的操作栏在外部兄弟节点 — 用 visibility 隐藏（保留占位，零布局位移）
  const nextSibling = blockEl.nextElementSibling;
  if (nextSibling && nextSibling.dataset.role === 'user-actions') {
    nextSibling.style.visibility = 'hidden';
  }

  // textarea 保持气泡样式（bg/radius/padding/max-width 右对齐），就地编辑
  const textarea = document.createElement('textarea');
  textarea.className = 'msg-edit-textarea msg-edit-full';
  textarea.value = originalContent;
  textarea.rows = 1;
  textarea.style.minHeight = baseHeight + 'px';
  // 自适应高度：与聊天输入框一致的行为（min 48px / max 160px）
  const EDIT_MIN_HEIGHT = Math.max(baseHeight, 48);
  const EDIT_MAX_HEIGHT = 160;
  const autoGrow = () => {
    textarea.style.height = 'auto';
    const newHeight = Math.min(Math.max(textarea.scrollHeight, EDIT_MIN_HEIGHT), EDIT_MAX_HEIGHT);
    textarea.style.height = newHeight + 'px';
  };
  textarea.addEventListener('input', autoGrow);
  // 初始高度
  requestAnimationFrame(autoGrow);

  // 操作按钮栏（插入到消息块外部，右下角）
  const actionBar = document.createElement('div');
  actionBar.className = 'msg-edit-actions msg-edit-actions-below';

  // 回形针按钮（文档上传）
  const attachBtn = document.createElement('button');
  attachBtn.className =
    'msg-edit-btn msg-edit-attach msg-action-btn';
  attachBtn.title = t('chat.edit_attach') || '上传文档';
  attachBtn.setAttribute('aria-label', t('chat.edit_attach') || '上传文档');
  attachBtn.innerHTML =
    '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>';
  attachBtn.onclick = () => {
    if (_onAttachCallback) _onAttachCallback();
  };

  // 取消按钮
  const cancelBtn = document.createElement('button');
  cancelBtn.className =
    'msg-edit-btn msg-edit-cancel px-3 py-1.5 rounded-md text-sm text-text-secondary hover:text-accent hover:bg-accent/10 transition-colors duration-fast border border-transparent';
  cancelBtn.title = t('chat.edit_cancel') || '取消';
  cancelBtn.setAttribute('aria-label', t('chat.edit_cancel') || '取消');
  cancelBtn.textContent = t('chat.edit_cancel') || '取消';
  cancelBtn.onclick = () => exitEditMode(blockEl);

  // 发送按钮 — 主操作（accent 底 + ink 文字），未修改时禁用
  const resendBtn = document.createElement('button');
  resendBtn.className =
    'msg-edit-btn msg-edit-resend px-3 py-1.5 rounded-md text-sm bg-accent text-ink font-medium hover:opacity-90 transition-opacity duration-fast disabled:opacity-40 disabled:cursor-not-allowed';
  resendBtn.title = t('chat.edit_resend') || '发送';
  resendBtn.setAttribute('aria-label', t('chat.edit_resend') || '发送');
  resendBtn.textContent = t('chat.edit_resend') || '发送';
  resendBtn.disabled = true; // 初始禁用：未修改时不允许重发
  resendBtn.onclick = () => confirmEdit(blockEl);

    // IME 组合防护：中文/日文输入法组合期间不更新按钮状态
  const imeGuard = createImeGuard();
  imeGuard.attach(textarea);

  // 监听内容变化：仅当内容被修改后才启用发送按钮
  // IME 组合期间（compositionstart → compositionend）忽略 input 事件，
  // 避免用户正在选词时发送按钮就被启用
  const checkModified = () => {
    if (imeGuard.isComposing()) return; // IME 组合中，不更新按钮状态
    const modified = textarea.value.trim() !== originalContent.trim();
    resendBtn.disabled = !modified || !textarea.value.trim();
  };
  textarea.addEventListener('input', checkModified);
  // compositionend 后需要手动触发一次检查（因为 input 事件被跳过了）
  textarea.addEventListener('compositionend', checkModified);
  // 键盘行为与聊天输入框完全一致：
  // - Enter（无 Shift）→ 发送（仅在内容修改后生效）
  // - Shift+Enter → 换行（浏览器默认行为，不拦截）
  // - Escape → 取消编辑
  // - IME 组合中的 Enter 是「确认候选词」而非「发送」，必须忽略
  textarea.addEventListener('keydown', (e) => {
    if (isComposingEvent(e)) return; // IME 组合中，忽略 Enter/Escape
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault(); // 阻止默认换行
      if (!resendBtn.disabled) confirmEdit(blockEl);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      exitEditMode(blockEl);
    }
  });

  actionBar.appendChild(attachBtn);
  actionBar.appendChild(cancelBtn);
  actionBar.appendChild(resendBtn);

  // textarea 插入消息块，操作栏插入消息块之后（外部）
  blockEl.appendChild(textarea);
  blockEl.after(actionBar);

  // 聚焦 textarea 并选中文本末尾
  textarea.focus();
  const len = textarea.value.length;
  textarea.setSelectionRange(len, len);

  // 添加点击外部取消编辑的监听（延迟绑定，避免当前点击立即触发）
  setTimeout(() => {
    _outsideClickHandler = (e) => {
      // 如果点击在编辑块内部或操作栏内部，不取消
      const actionBarEl = blockEl.nextElementSibling;
      if (blockEl.contains(e.target) || (actionBarEl && actionBarEl.contains(e.target))) {
        return;
      }
      // 点击外部，取消编辑
      exitEditMode(blockEl);
    };
    document.addEventListener('click', _outsideClickHandler);
  }, 0);

  return textarea;
}

/**
 * 退出编辑模式：移除编辑器，恢复原始内容显示。
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @returns {void}
 */
export function exitEditMode(blockEl) {
  if (!blockEl) return;
  // 移除点击外部监听
  if (_outsideClickHandler) {
    document.removeEventListener('click', _outsideClickHandler);
    _outsideClickHandler = null;
  }
  blockEl.classList.remove('editing');
  // 先恢复原始内容（在移除 textarea 之前），避免高度跳变
  const contentEl = blockEl.querySelector('.msg-user-content');
  if (contentEl) {
    contentEl.style.display = '';
  }
  // 移除 textarea（在消息块内部）
  blockEl.querySelectorAll('.msg-edit-full').forEach((el) => el.remove());
  // 移除操作栏（在消息块外部，即下一个兄弟节点）
  const next = blockEl.nextElementSibling;
  if (next && next.classList.contains('msg-edit-actions-below')) next.remove();
  // 恢复用户消息操作栏（外部兄弟节点）
  const actionsBar = blockEl.nextElementSibling;
  if (actionsBar && actionsBar.dataset.role === 'user-actions') {
    actionsBar.style.visibility = '';
  }

  delete blockEl.dataset.editingOriginal;
  if (_editingBlock === blockEl) {
    _editingBlock = null;
    _onResendCallback = null;
    _onAttachCallback = null;
  }
}

/**
 * 确认编辑：获取编辑后文本，触发重新发送回调。
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @param {function} [onResend] - 重新发送回调
 * @returns {void}
 */
export function confirmEdit(blockEl, onResend) {
  if (!blockEl) return;
  const textarea = blockEl.querySelector('.msg-edit-full');
  if (!textarea) return;

  const newContent = textarea.value.trim();
  if (!newContent) return; // 空内容不允许发送

  // 使用传入的回调或存储的回调
  const callback = onResend || _onResendCallback;

  exitEditMode(blockEl);
  if (callback) callback(newContent);
}

// ============================================================
// 分支分页器（基于 turn-tree 版本数据）
// ============================================================

/**
 * 解析 user 块：传入 user 块直接返回；传入 assistant 块则向前查找。
 * 分页器挂在用户消息下方的操作栏中（与复制/编辑按钮同行），两种调用形态都支持。
 *
 * @param {HTMLElement} blockEl - user 块或 assistant 块
 * @returns {HTMLElement|null} user 消息块
 */
function resolveUserBlock(blockEl) {
  if (!blockEl) return null;
  if (blockEl.classList.contains('msg-user')) return blockEl;
  // 向前查找 user 块
  let prev = blockEl.previousElementSibling;
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
 * 查找用户消息块对应的操作栏（data-role="user-actions" 的兄弟节点）。
 * @param {HTMLElement} blockEl - user 块或 assistant 块
 * @returns {HTMLElement|null}
 */
function findUserActionsBar(blockEl) {
  const userBlock = resolveUserBlock(blockEl);
  if (!userBlock) return null;
  const next = userBlock.nextElementSibling;
  if (next && next.dataset.role === 'user-actions')
    // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
    return next;
  return null;
}

/**
 * 在用户操作栏中查找分页器。
 * @param {HTMLElement} blockEl - user 块或 assistant 块
 * @returns {HTMLElement|null}
 */
function findPaginationEl(blockEl) {
  const actionsBar = findUserActionsBar(blockEl);
  // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
  return actionsBar?.querySelector('.branch-pagination') || null;
}

/**
 * 渲染或刷新分支分页器（挂在用户消息下方操作栏中，与复制/编辑按钮同行）。
 *
 * 分页器显示在用户操作栏最左侧：`◀ 1/N ▶  [复制] [编辑]`
 * - 页码 = version 编号（从 1 开始）
 * - 当前页 = activeVersion
 * - 切换页码时：问题区和答案区**就地替换**为当前版本的 Q&A
 * - 无用户操作栏时不渲染（流完成后由 finalizeStream 调用渲染）
 *
 * @param {HTMLElement} blockEl - user 消息块或 assistant 消息块
 * @param {string} turnGroup - 轮次分组 ID
 * @returns {HTMLElement|null} 分页器元素（无多版本或未找到操作栏时返回 null）
 */
export function renderBranchPagination(blockEl, turnGroup) {
  if (!blockEl || !turnGroup) return null;
  const userBlock = resolveUserBlock(blockEl);
  if (!userBlock) return null;
  const actionsBar = findUserActionsBar(userBlock);
  if (!actionsBar) return null;

  removeBranchPagination(userBlock);

  const versionCount = getVersionCount(turnGroup);
  if (versionCount <= 1) return null; // 单版本不显示分页器

  const turn = getTurn(turnGroup);
  if (!turn) return null;

  removeBranchPagination(userBlock);

  const pagination = document.createElement('div');
  pagination.className = 'branch-pagination';
  pagination.dataset.turnGroup = turnGroup;
  pagination.dataset.currentPage = String(turn.activeVersion);
  pagination.dataset.total = String(versionCount);

  // 左箭头（◀）
  const prevBtn = document.createElement('button');
  prevBtn.className = 'branch-pagination-btn branch-pagination-prev';
  prevBtn.innerHTML = '◀';
  prevBtn.setAttribute('aria-label', t('chat.branch_prev') || '查看之前的版本');
  prevBtn.disabled = turn.activeVersion <= 1;
  prevBtn.onclick = () => navigateBranch(userBlock, turnGroup, -1);

  // 计数器（对齐 DeepSeek：「当前 / 总数」带空格格式）
  const counter = document.createElement('span');
  counter.className = 'branch-pagination-counter';
  counter.textContent = `${turn.activeVersion} / ${versionCount}`;

  // 右箭头（▶）
  const nextBtn = document.createElement('button');
  nextBtn.className = 'branch-pagination-btn branch-pagination-next';
  nextBtn.innerHTML = '▶';
  nextBtn.setAttribute('aria-label', t('chat.branch_next') || '查看更新版本');
  nextBtn.disabled = turn.activeVersion >= versionCount;
  nextBtn.onclick = () => navigateBranch(userBlock, turnGroup, 1);

  pagination.appendChild(prevBtn);
  pagination.appendChild(counter);
  pagination.appendChild(nextBtn);

  // 分页器插入到用户操作栏的最后面（编辑按钮右侧）
  actionsBar.appendChild(pagination);

  return pagination;
}

/**
 * 分页导航：切换查看的版本。
 *
 * 切换时问题区和答案区**就地替换**为当前版本的 Q&A：
 * - 更新用户消息文本
 * - 重新渲染助手回答（Markdown + 来源 + 思考过程）
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @param {string} turnGroup - 轮次分组 ID
 * @param {number} delta - 版本号增量（-1 向前 / +1 向后）
 * @returns {void}
 */
export function navigateBranch(blockEl, turnGroup, delta) {
  const turn = getTurn(turnGroup);
  if (!turn) return;

  const versionCount = turn.versions.length;
  let newVersion = turn.activeVersion + delta;
  if (newVersion < 1) newVersion = 1;
  if (newVersion > versionCount) newVersion = versionCount;
  if (newVersion === turn.activeVersion) return;

  // 更新轮次树中的活跃版本
  setActiveVersion(turnGroup, newVersion);

  // 持久化活跃版本到 DB
  const conversationId = get('currentConversationId');
  if (conversationId) {
    convApi.setTurnActiveVersion(conversationId, turnGroup, newVersion).catch((err) => {
      console.warn('set_turn_active_version IPC 失败:', err);
    });
  }

  // 就地替换 Q&A 视图
  applyBranchView(blockEl, turnGroup);

  // 更新分页器显示
  const pagination = findPaginationEl(blockEl);
  if (pagination) {
    pagination.dataset.currentPage = String(newVersion);
    const counter = pagination.querySelector('.branch-pagination-counter');
    if (counter) counter.textContent = `${newVersion} / ${versionCount}`;
    const prevBtn = pagination.querySelector('.branch-pagination-prev');
    if (prevBtn) prevBtn.disabled = newVersion <= 1;
    const nextBtn = pagination.querySelector('.branch-pagination-next');
    if (nextBtn) nextBtn.disabled = newVersion >= versionCount;
  }
}

/**
 * 就地应用当前活跃版本的 Q&A 视图。
 *
 * - 问题区（.msg-user-content）：替换为当前版本的用户消息
 * - 答案区：找到后续的 assistant 块，重新渲染内容
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @param {string} turnGroup - 轮次分组 ID
 * @returns {void}
 */
export function applyBranchView(blockEl, turnGroup) {
  const version = getActiveVersion(turnGroup);
  if (!version) return;

  // 1. 更新问题区文本
  const contentEl = blockEl.querySelector('.msg-user-content');
  if (contentEl) {
    contentEl.textContent = version.userContent;
    blockEl.dataset.fullText = version.userContent;
  }

  // 2. 找到后续的 assistant 块并更新内容
  const assistantEl = findAssistantBlock(blockEl);
  if (!assistantEl) return;

  const mdEl = assistantEl.querySelector('.md');
  const sourcesEl = assistantEl.querySelector('.sources');
  const thinkingPanel = assistantEl._thinkingPanel;

  // 3. 先清空旧版本内容：思考面板必须清理（否则残留上一版本的 reasoning/stages），
  //    移除思考指示器
  if (thinkingPanel) thinkingPanel.clearContent();
  const thinking = assistantEl.querySelector('.thinking-indicator');
  if (thinking) thinking.remove();

  if (version.assistantContent) {
    // 有回答内容：渲染 Markdown
    if (mdEl) {
      mdEl.classList.remove('hidden');
      mdEl.dataset.rawMarkdown = version.assistantContent;
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      renderMarkdown(mdEl, version.assistantContent, version.sources, false);
      // 异步渲染富内容（Mermaid/KaTeX 等）
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      renderRichContent(mdEl).catch(() => {});
    }
    // 渲染来源
    if (sourcesEl && version.sources && version.sources.length > 0) {
      // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
      renderSourcesInto(sourcesEl, version.sources);
    } else if (sourcesEl) {
      sourcesEl.innerHTML = '';
      sourcesEl.className = 'sources mt-2';
    }
    // 思考过程：完成态 + 目标版本的推理内容（markdown 渲染）
    // 展开/折叠状态保持用户上一次操作（持久化），不强制折叠
    if (thinkingPanel) {
      thinkingPanel.setComplete();
      if (version.reasoning) {
        thinkingPanel.renderReasoning(version.reasoning);
      }
      // 恢复持久化的展开/折叠状态（按 assistant 消息关联的 ID）
      const assistantMsgId = assistantEl.dataset.msgId || null;
      thinkingPanel.setMsgId(assistantMsgId);
    }
  } else {
    // 无回答内容（当前版本正在流式生成中）：清空答案区，思考面板折叠清空
    if (mdEl) {
      mdEl.innerHTML = '';
      mdEl.classList.remove('md-fade-in');
      delete mdEl.dataset.rawMarkdown;
    }
    if (sourcesEl) {
      sourcesEl.innerHTML = '';
      sourcesEl.className = 'sources mt-2';
    }
    if (thinkingPanel) {
      thinkingPanel.collapse();
    }
  }
}

/**
 * 找到用户消息块之后紧邻的 assistant 消息块。
 *
 * @param {HTMLElement} userBlockEl - 用户消息块根元素
 * @returns {HTMLElement|null} assistant 消息块
 */
function findAssistantBlock(userBlockEl) {
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
/** 导出供 chat-render 在首次编辑时读取原始答案内容。 */
export { findAssistantBlock };

/**
 * 将引用来源渲染到指定容器（简化版，不依赖全局状态）。
 *
 * @param {HTMLElement} sourcesEl - .sources 容器
 * @param {Array} sources - 引用来源列表
 * @returns {void}
 */
function renderSourcesInto(sourcesEl, sources) {
  sourcesEl.innerHTML = '';
  sourcesEl.className = 'sources mt-2';

  const sorted = [...sources].sort((a, b) => (b.score || 0) - (a.score || 0));

  const toggle = document.createElement('button');
  toggle.className = 'sources-toggle';
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
    list.appendChild(card);
  });

  sourcesEl.appendChild(toggle);
  sourcesEl.appendChild(list);
}

/**
 * 移除分支分页器。
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @returns {void}
 */
export function removeBranchPagination(blockEl) {
  if (!blockEl) return;
  const pagination = findPaginationEl(blockEl);
  if (pagination) pagination.remove();
}

// ============================================================
// 编辑按钮创建
// ============================================================

/**
 * 在用户消息操作栏中创建编辑按钮。
 *
 * @param {HTMLElement} blockEl - 用户消息块根元素
 * @param {string} content - 消息原始内容
 * @param {function} onResend - 重新发送回调
 * @param {function} [onAttach] - 文档上传回调（编辑模式回形针按钮）
 * @returns {HTMLButtonElement} 编辑按钮元素
 */
export function createEditButton(blockEl, content, onResend, onAttach) {
  const editBtn = document.createElement('button');
  editBtn.className = 'msg-action-btn';
  editBtn.title = t('chat.edit') || '编辑';
  editBtn.setAttribute('aria-label', t('chat.edit') || '编辑');
  editBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>';
  editBtn.onclick = () => {
    // 读取当前显示的版本内容（版本切换后 dataset.fullText 已更新，闭包里的 content 可能过期）
    const current = blockEl.dataset.fullText || content;
    enterEditMode(blockEl, current, onResend, onAttach);
  };
  return editBtn;
}
