/**
 * EchoMind 帮助文档面板（REQ-HELP-001 v1.5）。
 *
 * 内置帮助面板，包含四个 Tab：
 * 1. 快速入门 — 导入文档 → 配置 LLM → 提问
 * 2. 快捷键 — 列出所有快捷键（复用 keyboard-help.js 数据）
 * 3. FAQ — 常见问题解答
 * 4. 隐私说明 — 数据安全与隐私保护
 *
 * 纯前端实现，内容通过 i18n 本地化，不联网。
 */

import { $ } from './utils.js';
import { t } from './i18n.js';
import { invoke, openUrl } from './ipc.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { zClass, Z_INDEX } from './panel-stack.js';

/** 面板是否已创建 */
let _created = false;
/** 面板 DOM 元素引用 */
let _panelEl = null;
/** 当前激活的 Tab */
let _activeTab = 'quickstart';

/**
 * Tab 定义。
 * @type {Array<{id: string, labelKey: string}>}
 */
const TABS = [
  { id: 'quickstart', labelKey: 'help.tab_quickstart' },
  { id: 'shortcuts', labelKey: 'help.tab_shortcuts' },
  { id: 'faq', labelKey: 'help.tab_faq' },
  { id: 'privacy', labelKey: 'help.tab_privacy' },
  { id: 'about', labelKey: 'about.tab_about' },
];

/**
 * 渲染快速入门内容。
 * @returns {string}
 */
function _renderQuickStart() {
  return `
    <div class="space-y-4">
      <h3 class="text-base font-medium text-text-primary">${t('help.qs_title', 'Quick Start')}</h3>
      <ol class="space-y-3 text-sm text-text-secondary list-decimal list-inside">
        <li><strong class="text-text-primary">${t('help.qs_step1_title', 'Import Documents')}</strong><br>${t('help.qs_step1_desc', 'Click the + button or drag files to import Markdown, TXT, PDF, HTML files into your knowledge base.')}</li>
        <li><strong class="text-text-primary">${t('help.qs_step2_title', 'Configure LLM')}</strong><br>${t('help.qs_step2_desc', 'Open Settings (⌘,) and enter your API Key and Base URL. Any OpenAI-compatible endpoint works.')}</li>
        <li><strong class="text-text-primary">${t('help.qs_step3_title', 'Ask Questions')}</strong><br>${t('help.qs_step3_desc', 'Type your question in the input box. AI answers based on your knowledge base documents.')}</li>
        <li><strong class="text-text-primary">${t('help.qs_step4_title', 'Review Sources')}</strong><br>${t('help.qs_step4_desc', 'Each answer shows reference sources. Click to view the original document chunks.')}</li>
      </ol>
    </div>`;
}

/**
 * 渲染快捷键内容。
 * @returns {string}
 */
function _renderShortcuts() {
  const groups = [
    { title: t('help.sc_general', 'General'), items: [
      { keys: '⌘ K', desc: t('help.sc_cmd_palette', 'Command palette') },
      { keys: '⌘ N', desc: t('help.sc_new_chat', 'New conversation') },
      { keys: '⌘ O', desc: t('help.sc_import', 'Import files') },
      { keys: '⌘ ,', desc: t('help.sc_settings', 'Settings') },
      { keys: '⌘ B', desc: t('help.sc_sidebar', 'Toggle sidebar') },
      { keys: '?', desc: t('help.sc_help', 'Keyboard help') },
    ]},
    { title: t('help.sc_chat', 'Chat'), items: [
      { keys: 'Enter', desc: t('help.sc_send', 'Send message') },
      { keys: '⇧ Enter', desc: t('help.sc_newline', 'New line') },
      { keys: 'Esc', desc: t('help.sc_stop', 'Stop / Close') },
    ]},
  ];

  return groups.map(group => `
    <div class="space-y-2">
      <h4 class="text-xs font-medium text-text-tertiary uppercase tracking-wider">${group.title}</h4>
      ${group.items.map(item => `
        <div class="flex items-center justify-between text-sm">
          <span class="text-text-secondary">${item.desc}</span>
          <kbd class="px-2 py-0.5 text-xs font-mono bg-surface-3 border border-line rounded text-text-secondary">${item.keys}</kbd>
        </div>
      `).join('')}
    </div>
  `).join('');
}

/**
 * 渲染 FAQ 内容。
 * @returns {string}
 */
function _renderFaq() {
  const faqs = [
    { q: t('help.faq_q1', 'How do I configure my LLM?'), a: t('help.faq_a1', 'Open Settings (⌘,) and enter your API Key, Base URL, and model name. Any OpenAI-compatible API works (OpenAI, DeepSeek, Ollama, etc.).') },
    { q: t('help.faq_q2', 'What file formats are supported?'), a: t('help.faq_a2', 'Markdown (.md), Text (.txt), PDF (.pdf), HTML (.html), Word (.docx), PowerPoint (.pptx), and EPUB (.epub).') },
    { q: t('help.faq_q3', 'Where is my data stored?'), a: t('help.faq_a3', 'All data stays locally on your device in the app data directory. Nothing is sent to any server except your LLM API calls.') },
    { q: t('help.faq_q4', 'How do I activate Pro?'), a: t('help.faq_a4', 'Go to Settings → License and enter your Pro license key. Pro unlocks: unlimited documents, PDF import, local LLM, OCR, and more.') },
    { q: t('help.faq_q5', 'What is the embedding model?'), a: t('help.faq_a5', 'EchoMind uses all-MiniLM-L6-v2 (384-dimensional, ~30MB) by default. It runs locally via ONNX Runtime. Pro users can upload custom models.') },
    { q: t('help.faq_q6', 'How does encryption work?'), a: t('help.faq_a6', 'Database encryption uses SQLCipher (AES-256). Your password is derived via Argon2id KDF. The database is transparently encrypted at rest.') },
  ];

  return faqs.map(faq => `
    <div class="space-y-1">
      <h4 class="text-sm font-medium text-text-primary">${faq.q}</h4>
      <p class="text-sm text-text-secondary leading-relaxed">${faq.a}</p>
    </div>
  `).join('');
}

/**
 * 渲染隐私说明内容。
 * @returns {string}
 */
function _renderPrivacy() {
  return `
    <div class="space-y-3 text-sm text-text-secondary leading-relaxed">
      <h3 class="text-base font-medium text-text-primary">${t('help.privacy_title', 'Privacy & Security')}</h3>
      <p>${t('help.privacy_local', 'All your documents, embeddings, and conversations are stored locally on your device. No data is sent to any external server.')}</p>
      <p>${t('help.privacy_llm', 'When you ask a question, only your query and relevant document chunks are sent to your configured LLM API endpoint. No other data leaves your device.')}</p>
      <p>${t('help.privacy_encryption', 'Database encryption (SQLCipher AES-256) protects your data at rest. Your password never leaves your device.')}</p>
      <p>${t('help.privacy_pii', 'PII detection can automatically identify and redact 8 types of sensitive information (email, phone, ID card, bank card, IP, SSN, passport, international phone).')}</p>
      <p>${t('help.privacy_audit', 'Audit logging uses a tamper-evident hash chain. Any modification to audit logs is detectable via chain verification.')}</p>
      <p>${t('help.privacy_noproxy', 'The application does not use any proxy. All network connections are direct. You can verify this in the source code — every reqwest client uses .no_proxy().')}</p>
    </div>`;
}

/**
 * 渲染关于页面内容（REQ-HELP-003 v1.6）。
 * @returns {string}
 */
function _renderAbout() {
  const version = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : 'dev';
  return `
    <div class="space-y-4 text-sm text-text-secondary leading-relaxed">
      <div class="flex items-center gap-3">
        <div class="w-12 h-12 rounded-xl bg-accent/10 flex items-center justify-center shrink-0">
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-accent">
            <path d="M9 18h6"/><path d="M10 22h4"/><path d="M15.09 14c.18-.98.65-1.74 1.41-2.5A4.65 4.65 0 0 0 18 8 6 6 0 0 0 6 8c0 1 .23 2.23 1.5 3.5A4.61 4.61 0 0 1 8.91 14"/>
          </svg>
        </div>
        <div>
          <h3 class="text-base font-bold text-text-primary">${t('app.name', 'EchoMind')}</h3>
          <p class="text-xs text-text-tertiary">${t('app.tagline', '灵犀')}</p>
        </div>
      </div>
      <div><strong class="text-text-primary">${t('about.version', 'Version')}</strong>: <span class="font-mono">${version}</span></div>
      <div><strong class="text-text-primary">${t('about.tech_stack', 'Tech Stack')}</strong>: ${t('about.tech_stack_desc', 'Rust + Tauri v2 + fastembed ONNX + SQLite')}</div>
      <div><strong class="text-text-primary">${t('about.license', 'License')}</strong>: ${t('about.license_desc', 'EchoMind is a local-first RAG knowledge base app.')}</div>
      <div><strong class="text-text-primary">${t('about.privacy_policy', 'Privacy Policy')}</strong>: ${t('about.privacy_summary', 'All data is stored locally on your device.')}</div>
      <p class="text-xs text-text-quaternary text-center pt-3 border-t border-line">${t('about.copyright', '© 2026 EchoMind. All rights reserved.')}</p>
    </div>`;
}

/**
 * 根据 Tab ID 渲染内容。
 * @param {string} tabId
 * @returns {string}
 */
function _renderTabContent(tabId) {
  switch (tabId) {
    case 'quickstart': return _renderQuickStart();
    case 'shortcuts': return _renderShortcuts();
    case 'faq': return _renderFaq();
    case 'privacy': return _renderPrivacy();
    case 'about': return _renderAbout();
    default: return '';
  }
}

/**
 * 确保面板 DOM 已创建。
 * @returns {HTMLElement}
 */
function _ensurePanel() {
  if (_created && _panelEl && _panelEl.isConnected) return _panelEl;

  const overlay = document.createElement('div');
  overlay.id = 'helpPanelOverlay';
  overlay.className = `fixed inset-0 bg-black/50 flex items-center justify-center ${zClass(Z_INDEX.PANEL_1)}`;
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-labelledby', 'helpPanelTitle');

  const panel = document.createElement('div');
  panel.className = 'bg-surface-1 border border-line rounded-xl shadow-2xl w-[600px] max-h-[80vh] overflow-hidden flex flex-col';

  // 头部
  const header = document.createElement('div');
  header.className = 'flex items-center justify-between px-6 py-4 border-b border-line';
  const title = document.createElement('h2');
  title.id = 'helpPanelTitle';
  title.className = 'text-lg font-semibold text-text-primary';
  title.textContent = t('help.title', 'Help');
  const closeBtn = document.createElement('button');
  closeBtn.className = 'text-text-tertiary hover:text-text-primary transition-colors p-1 rounded';
  closeBtn.setAttribute('aria-label', t('common.close'));
  closeBtn.innerHTML = '<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  closeBtn.onclick = closeHelpPanel;
  header.appendChild(title);
  header.appendChild(closeBtn);

  // Tab 栏
  const tabBar = document.createElement('div');
  tabBar.id = 'helpTabBar';
  tabBar.className = 'flex gap-1 px-6 py-2 border-b border-line';

  for (const tab of TABS) {
    const btn = document.createElement('button');
    btn.dataset.tabId = tab.id;
    btn.className = 'px-3 py-1.5 text-sm rounded transition-colors';
    btn.textContent = t(tab.labelKey);
    btn.onclick = () => _switchTab(tab.id);
    tabBar.appendChild(btn);
  }

  // 内容区
  const content = document.createElement('div');
  content.id = 'helpContent';
  content.className = 'flex-1 overflow-y-auto px-6 py-4 space-y-4';

  panel.appendChild(header);
  panel.appendChild(tabBar);
  panel.appendChild(content);
  overlay.appendChild(panel);

  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) closeHelpPanel();
  });

  document.body.appendChild(overlay);
  _panelEl = overlay;
  _created = true;
  return overlay;
}

/**
 * 切换 Tab。
 * @param {string} tabId
 */
function _switchTab(tabId) {
  _activeTab = tabId;
  _updateTabUI();
  const content = document.getElementById('helpContent');
  if (content) {
    content.innerHTML = _renderTabContent(tabId);
  }
}

/**
 * 更新 Tab UI 样式。
 */
function _updateTabUI() {
  const tabBar = document.getElementById('helpTabBar');
  if (!tabBar) return;
  tabBar.querySelectorAll('button[data-tab-id]').forEach(btn => {
    if (btn.dataset.tabId === _activeTab) {
      btn.className = 'px-3 py-1.5 text-sm rounded transition-colors bg-accent/20 text-accent';
    } else {
      btn.className = 'px-3 py-1.5 text-sm rounded transition-colors text-text-tertiary hover:text-text-secondary hover:bg-surface-2';
    }
  });
}

/**
 * 打开帮助面板。
 * @param {string} [initialTab='quickstart'] - 初始 Tab
 */
export function openHelpPanel(initialTab = 'quickstart') {
  const overlay = _ensurePanel();
  overlay.classList.remove('hidden');
  _switchTab(initialTab);
  pushPanel({ id: 'help-panel', close: closeHelpPanel, element: overlay, label: 'Help' });
}

/**
 * 关闭帮助面板。
 */
export function closeHelpPanel() {
  removePanel('help-panel');
  if (_panelEl) {
    _panelEl.classList.add('hidden');
  }
}

// ============================================================
// About 面板兼容导出（原 about-panel.js 已合并到 help-panel.js）
// help-panel 已有 "about" Tab，openAboutPanel 直接打开 about Tab。
// ============================================================

/**
 * 打开关于面板（兼容别名，直接打开 help 面板的 About Tab）。
 */
export function openAboutPanel() {
  openHelpPanel('about');
}

/**
 * 关闭关于面板（兼容别名，与 closeHelpPanel 相同）。
 */
export function closeAboutPanel() {
  closeHelpPanel();
}

/**
 * 初始化关于面板（兼容别名，按钮注册委托给 help-panel）。
 */
export function initAboutPanel() {
  const aboutBtn = document.getElementById('aboutBtn');
  if (aboutBtn) {
    aboutBtn.onclick = () => openHelpPanel('about');
  }
}

// ============================================================
// 快捷键帮助面板（原 keyboard-help.js，REQ-KB-005）
// ============================================================

/** 当前搜索关键词 */
let _searchQuery = '';

/**
 * 快捷键分组数据。
 * @returns {Array<{label: string, items: Array<{keys: string, desc: string}>}>}
 */
function getShortcutGroups() {
  const mod = navigator.platform.includes('Mac') ? '⌘' : 'Ctrl';
  return [
    {
      label: t('keyboard_help.group_general'),
      items: [
        { keys: `${mod}K`, desc: t('keyboard_help.cmd_palette') },
        { keys: `${mod}N`, desc: t('keyboard_help.new_chat') },
        { keys: `${mod}O`, desc: t('keyboard_help.import_files') },
        { keys: `${mod},`, desc: t('keyboard_help.settings') },
        { keys: `${mod}B`, desc: t('keyboard_help.toggle_sidebar') },
        { keys: `${mod}E`, desc: t('keyboard_help.export_conversation') },
        { keys: `${mod}⇧F`, desc: t('keyboard_help.global_search') },
        { keys: `${mod}/`, desc: t('keyboard_help.show_help') },
      ],
    },
    {
      label: t('keyboard_help.group_chat'),
      items: [
        { keys: 'Enter', desc: t('keyboard_help.send_message') },
        { keys: 'Shift+Enter', desc: t('keyboard_help.new_line') },
        { keys: `${mod}L`, desc: t('keyboard_help.focus_input') },
        { keys: 'Esc', desc: t('keyboard_help.stop_or_close') },
        { keys: `${mod}.`, desc: t('keyboard_help.stop_generation') },
        { keys: `${mod}⇧⌫`, desc: t('keyboard_help.clear_chat') },
      ],
    },
    {
      label: t('keyboard_help.group_navigation'),
      items: [
        { keys: '↑↓', desc: t('keyboard_help.navigate_results') },
        { keys: 'Enter', desc: t('keyboard_help.select_result') },
        { keys: 'Esc', desc: t('keyboard_help.close_search') },
      ],
    },
    {
      label: t('keyboard_help.group_edit'),
      items: [
        { keys: `${mod}X`, desc: t('keyboard_help.cut') },
        { keys: `${mod}C`, desc: t('keyboard_help.copy') },
        { keys: `${mod}V`, desc: t('keyboard_help.paste') },
        { keys: `${mod}A`, desc: t('keyboard_help.select_all') },
      ],
    },
  ];
}

/** 面板是否已打开 */
let _kbHelpOpen = false;

/**
 * 打开快捷键帮助面板。
 */
export function openKeyboardHelp() {
  const panel = $('keyboardHelpPanel');
  if (!panel) return;
  panel.classList.remove('hidden');
  _kbHelpOpen = true;

  _searchQuery = '';
  const searchInput = $('keyboardHelpSearch');
  if (searchInput) searchInput.value = '';

  renderShortcutList();

  if (searchInput) {
    searchInput.focus();
  } else {
    const closeBtn = $('keyboardHelpClose');
    if (closeBtn) closeBtn.focus();
  }

  pushPanel({
    id: 'keyboard-help',
    close: closeKeyboardHelp,
    element: panel,
    label: 'Keyboard Help',
  });
}

/**
 * 关闭快捷键帮助面板。
 */
export function closeKeyboardHelp() {
  const panel = $('keyboardHelpPanel');
  if (!panel) return;
  panel.classList.add('hidden');
  _kbHelpOpen = false;
  removePanel('keyboard-help');
}

/**
 * 模糊过滤快捷键分组（REQ-KB-005 AC-4）。
 */
function filterShortcutGroups(groups, query) {
  const q = query.trim().toLowerCase();
  if (!q) return groups;
  return groups
    .map(group => ({
      ...group,
      items: group.items.filter(item =>
        item.desc.toLowerCase().includes(q) ||
        item.keys.toLowerCase().includes(q) ||
        group.label.toLowerCase().includes(q),
      ),
    }))
    .filter(group => group.items.length > 0);
}

/**
 * 渲染快捷键列表到面板内容区。
 */
function renderShortcutList() {
  const container = $('keyboardHelpContent');
  if (!container) return;
  container.innerHTML = '';

  const allGroups = getShortcutGroups();
  const groups = filterShortcutGroups(allGroups, _searchQuery);

  if (groups.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'text-sm text-text-quaternary text-center py-8';
    empty.textContent = t('keyboard_help.no_results', 'No matching shortcuts');
    container.appendChild(empty);
    return;
  }

  for (const group of groups) {
    const groupLabel = document.createElement('div');
    groupLabel.className = 'text-xs uppercase tracking-wider text-text-quaternary mb-2 mt-4 first:mt-0';
    groupLabel.textContent = group.label;
    container.appendChild(groupLabel);

    for (const item of group.items) {
      const row = document.createElement('div');
      row.className = 'flex items-center justify-between py-1.5 border-b border-border-subtle last:border-0';

      const desc = document.createElement('span');
      desc.className = 'text-sm text-text-secondary';
      desc.textContent = item.desc;
      row.appendChild(desc);

      const keys = document.createElement('kbd');
      keys.className = 'text-xs font-mono px-2 py-0.5 bg-surface-3 border border-border-default rounded text-text-primary';
      keys.textContent = item.keys;
      row.appendChild(keys);

      container.appendChild(row);
    }
  }
}

/**
 * 初始化快捷键帮助面板事件绑定。
 */
export function initKeyboardHelp() {
  const panel = $('keyboardHelpPanel');
  if (!panel) return;

  const closeBtn = $('keyboardHelpClose');
  if (closeBtn) closeBtn.onclick = closeKeyboardHelp;

  const searchInput = $('keyboardHelpSearch');
  if (searchInput) {
    let searchTimer = null;
    searchInput.addEventListener('input', (e) => {
      _searchQuery = e.target.value || '';
      if (searchTimer) clearTimeout(searchTimer);
      searchTimer = setTimeout(() => renderShortcutList(), 150);
    });
  }

  panel.addEventListener('click', (e) => {
    if (e.target === panel) closeKeyboardHelp();
  });

  panel.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      closeKeyboardHelp();
      return;
    }
    if (e.key === 'Tab') {
      const focusable = panel.querySelectorAll('button, input, [tabindex]:not([tabindex="-1"])');
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    }
  });
}

/**
 * 快捷键帮助面板是否已打开。
 */
export function isKeyboardHelpOpen() {
  return _kbHelpOpen;
}

// ============================================================
// 更新横幅（原 update-banner.js，REQ-HELP-004）
// ============================================================

/** 启动后延迟检查的毫秒数。 */
const STARTUP_DELAY_MS = 5000;

/** 横幅是否已创建。 */
let bannerCreated = false;

/**
 * 初始化更新检查横幅。
 */
export function initUpdateBanner() {
  setTimeout(async () => {
    try {
      const result = await invoke('check_for_updates');
      if (result && result.has_update) {
        showUpdateBanner(result);
      }
    } catch (_) {
      // AC-6: 网络不可用时静默跳过
    }
  }, STARTUP_DELAY_MS);
}

/**
 * 显示更新横幅。
 */
function showUpdateBanner(result) {
  if (bannerCreated) return;
  bannerCreated = true;

  const banner = document.createElement('div');
  banner.id = 'updateBanner';
  banner.className = 'update-banner';
  banner.setAttribute('role', 'alert');
  banner.setAttribute('aria-label', t('update_banner.label') || 'Update available');

  const textDiv = document.createElement('div');
  textDiv.className = 'update-banner-text';

  const versionSpan = document.createElement('span');
  versionSpan.className = 'update-banner-version';
  versionSpan.textContent = `v${result.latest_version}`;

  const descSpan = document.createElement('span');
  descSpan.className = 'update-banner-desc';
  const descText = t('update_banner.desc', {
    current: result.current_version,
    latest: result.latest_version,
  }) || `New version available: you have ${result.current_version}, latest is ${result.latest_version}`;
  descSpan.textContent = descText;

  textDiv.appendChild(versionSpan);
  textDiv.appendChild(descSpan);

  let notesBtn = null;
  if (result.release_notes) {
    notesBtn = document.createElement('button');
    notesBtn.className = 'update-banner-link';
    notesBtn.textContent = t('update_banner.view_notes') || 'Release Notes';
    notesBtn.setAttribute('aria-label', t('update_banner.view_notes') || 'Release Notes');
    notesBtn.onclick = () => {
      const notesContent = result.release_notes || '';
      showReleaseNotesModal(notesContent, result.latest_version);
    };
  }

  const downloadBtn = document.createElement('button');
  downloadBtn.className = 'update-banner-download';
  downloadBtn.textContent = t('update_banner.download') || 'Download';
  downloadBtn.setAttribute('aria-label', t('update_banner.download') || 'Download');
  downloadBtn.onclick = () => {
    const url = result.download_url || 'https://github.com/EchoMind/EchoMind/releases/latest';
    openUrl(url).catch(() => {});
  };

  const closeBtn = document.createElement('button');
  closeBtn.className = 'update-banner-close';
  closeBtn.setAttribute('aria-label', t('update_banner.close') || 'Close');
  closeBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  closeBtn.onclick = () => {
    banner.remove();
    bannerCreated = false;
  };

  banner.appendChild(textDiv);
  if (notesBtn) banner.appendChild(notesBtn);
  banner.appendChild(downloadBtn);
  banner.appendChild(closeBtn);

  document.body.prepend(banner);

  requestAnimationFrame(() => {
    banner.classList.add('update-banner-visible');
  });
}

/**
 * 显示更新日志模态框。
 */
function showReleaseNotesModal(content, version) {
  const modal = document.createElement('div');
  modal.className = 'update-notes-modal';
  modal.setAttribute('role', 'dialog');
  modal.setAttribute('aria-modal', 'true');
  modal.setAttribute('aria-label', t('update_banner.notes_title') || 'Release Notes');

  const overlay = document.createElement('div');
  overlay.className = 'update-notes-overlay';

  const dialog = document.createElement('div');
  dialog.className = 'update-notes-dialog';

  const header = document.createElement('div');
  header.className = 'update-notes-header';
  header.innerHTML = `<span class="update-notes-version">v${version}</span>`;

  const closeHeaderBtn = document.createElement('button');
  closeHeaderBtn.className = 'update-notes-close';
  closeHeaderBtn.setAttribute('aria-label', t('common.close') || 'Close');
  closeHeaderBtn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
  closeHeaderBtn.onclick = () => closeModal();
  header.appendChild(closeHeaderBtn);

  const body = document.createElement('div');
  body.className = 'update-notes-body';
  body.textContent = content;

  dialog.appendChild(header);
  dialog.appendChild(body);
  overlay.appendChild(dialog);
  modal.appendChild(overlay);

  const closeModal = () => { modal.remove(); };

  overlay.onclick = (e) => {
    if (e.target === overlay) closeModal();
  };

  modal.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      closeModal();
    }
  });

  document.body.appendChild(modal);
  closeHeaderBtn.focus();
}
