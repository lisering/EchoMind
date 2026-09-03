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
import { openSymbolSearch, closeSymbolSearch, initSymbolSearch } from './symbol-search.js';
import { initVoiceInput, stopAllTTS } from './voice-input.js';
import { initExportButtons, exportConversationToHtml, exportDocumentToHtml, exportConversationToPdf } from './export.js';
import { initErrorBoundary, initOfflineDetector } from './network-utils.js';
import { initBreadcrumb, updateBreadcrumb, clearBreadcrumb, initDocKeyboard } from './doc-nav.js';
import { openKbStats, closeKbStats, openDocPreview, closeDocPreview } from './doc-panels.js';
import { initBookmarks, refreshBookmarks, createBookmarkButton, createBookmarkNavigator } from './bookmarks.js';
import { loadDocuments, applyKbFilters, removeDocument, retryDocument, _updateKbResultCount, _renderKbDocPage, _createDocItem, initDocSortSelect, setupKbPanelCallbacks } from './kb-panel.js';
import { setupConversationListHost, loadConversation, highlightSearchMatch, removeConversation, loadConversations, _updateWindowTitle } from './conversation-list.js';
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

// ============================================================
// 长会话渐进渲染（V3.1 P4-5）
// ============================================================

/**
 * 收集会话消息的「渲染单元」列表（不产生 DOM 副作用）。
 *
 * 每个单元 = 一个 turn_group 的活跃版本，或一条独立（无 turn_group）消息。
 * 记录渲染时的 lastUserMsg 快照，消除原循环的跨迭代状态依赖。
 *
 * @param {Array} messages - get_messages 返回的全量消息
 * @returns {Array<{kind: 'turn'|'plain', m: Object, turn?: Object, ver?: Object, lastUserMsg: string}>}
 */

/**
 * 高亮搜索匹配文本：在聊天区域中查找包含搜索词的消息块，滚动到该位置并高亮。
 * @param {string} query - 搜索关键词
 */

/**
 * 删除会话并级联清理消息；若删的是当前会话则自动新建。
 * @param {string} id - 会话 ID
 */

/**
 * 计算会话所属时间分组的标签 key。
 *
 * 分组规则（按时间从近到远）：
 *   今天 → 昨天 → 一周内 → 一个月内 → 按月份（YYYY-MM）
 *
 * @param {number} createdAt - Unix 秒级时间戳
 * @returns {string} i18n key 前缀（如 'time.today'）或月份字符串
 */
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

/** doc-status-changed 全量列表刷新合流窗口（V3.1 P2-2） */
const DOC_REFRESH_DEBOUNCE_MS = 500;

// V3.1 P3-5：统一书签/搜索跳转回调（原 4 处内联复制 → 单一实现）。
// 注意：须在首次使用前定义（loadConversations/_renderConvItem 均会引用）。
const _navigateToMessage = createBookmarkNavigator({ loadConversation });
setupKbPanelCallbacks({ updateChatInputState });
setupConversationListHost({
  newChat,
  navigateToMessage: (convId, messageId) => _navigateToMessage(convId, messageId),
  onStop,
  closeAllPanels,
  saveDraft,
  clearQueue,
  restoreDraft,
  resetHistoryNav,
  updateInputUI,
});

// 知识库面板 — 全面重新设计（分页 + 无限滚动 + 模糊搜索）
// ============================================================

import { displayDocName, docStatusOf, DOC_STATUS_STYLE, formatBytes, getSubPhaseLabel } from './utils.js';

/** 每页渲染数量 */
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

// REQ-ING-008 v1.10：文档排序选择处理（V3.1 P4-3 移入 kb-panel.js）
initDocSortSelect();

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
  // 标记需要在 chat_done 后恢复（V3.1 P4-4：状态入 store）
  setState({ regenForceSearch: true });
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

  if ($('symbolBtn')) {
    $('symbolBtn').onclick = () => { openSymbolSearch(); $('toolsMenu')?.classList.add('hidden'); };
  }
  // P1-5: workflowBtn 已移除（F 级模块删除）

  // 延迟初始化面板内部组件（不阻塞首屏）
  _deferInit(() => {
    initGraphResize();
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
