/**
 * EchoMind 设置面板模块 — 主协调器（从 v1.21 拆分）。
 *
 * 本模块仅负责：
 * 1. openSettings()：加载 LLM 配置 + 渲染所有子面板
 * 2. closeSettings()：关闭面板 + 停用 Focus Trap
 * 3. initSettings()：事件绑定
 *
 * 通用设置 → settings-general.js
 * 高级设置 → settings-advanced.js
 */

import { setState, get } from './state.js';
import { $ } from './utils.js';
import { invoke, listen } from './ipc.js';
import { toast, toastError } from './toast.js';
import { showWizard } from './chat-render.js';
import { deactivatePro, updateProStatus } from './wizard.js';
import { t, getLocale, setLocale, SUPPORTED_LOCALES } from './i18n.js';
import { renderSecuritySettings } from './security.js';
import { createFocusTrap } from './focus-trap.js';
import { renderPerfSettings } from './perf-settings.js';
import { renderRagEvalSettings } from './rag-eval.js';
import { openEmbedEvalPanel } from './embed-eval.js';
import { openMcpSettingsPanel } from './mcp-settings.js';
import { renderMemorySettings } from './memory-panel.js';
import { renderDiskSpaceCard } from './disk-space.js';
import { renderTraceBudgetSettings } from './trace-panel.js';
import { pushPanel, removePanel } from './panel-stack.js';
import {
  loadLlmModeSetting, loadLocalModels, loadRecommendedModels,
  registerModelLoadListener, onLlmModeChange, downloadLocalModel,
  deleteLocalModel, selectLocalModel, onCancelDownload, onPauseDownload,
  renderKvCacheSettings, renderDeviceKindSettings, renderEmbedderModelSettings,
  renderPagedAttnSettings, renderKernelModeSettings,
} from './settings-local-llm.js';
import { openDownloadManager } from './download-manager.js';

// 通用设置
import {
  loadModelCacheInfo, initEmbedder, clearModelCache,
  updateVlmToggle, onVlmToggle, confirmVlmEnable, cancelVlmEnable,
  updateRerankToggle, onRerankToggle,
  updateHydeToggle, onHydeToggle,
  initThemeSwitcher, initVoiceSettings, initPdfExportSettings,
  loadObservabilitySettings, exportDiagnostics, exportLogs,
  exportBackup, importBackup,
} from './settings-general.js';

// Re-export for main.js backward compatibility
export { initEmbedder, clearModelCache, loadModelCacheInfo };

// 高级设置
import {
  onEmbeddingModelChange, initChunkParams,
  loadCustomModels, onUploadCustomModel, switchToCustomModel, deleteCustomModel,
  onAddWatchedFolder, onRemoveWatchedFolder, loadWatchedFolders,
  initMirrorSourceSelector,
  loadContextTokenLimit, loadTokenCostSettings,
  loadSamplingParams, saveSamplingParams, resetSamplingParams,
  onCoordinatorToggle, onSubAgentToggle,
  renderPromptTemplateSettings,
  renderWindowSettings, renderErrorLogsSettings, renderStartupSettings,
  renderRagLlmParams,
} from './settings-advanced.js';

// 暴露全局函数（内联 onclick 需要）
window.onRemoveWatchedFolder = onRemoveWatchedFolder;
window.selectLocalModel = selectLocalModel;
window.deleteLocalModel = deleteLocalModel;
window.downloadLocalModel = downloadLocalModel;
window.saveSamplingParams = saveSamplingParams;
window.resetSamplingParams = resetSamplingParams;
window.exportDiagnostics = exportDiagnostics;
window.exportLogs = exportLogs;
window.exportBackup = exportBackup;
window.importBackup = importBackup;
window.switchToCustomModel = switchToCustomModel;
window.deleteCustomModel = deleteCustomModel;
window.onUploadCustomModel = onUploadCustomModel;

/** 设置面板的 Focus Trap 实例（REQ-A11Y-002） */
let _settingsTrap = null;

/** 开发者模式标志（S5 审计 P0-6：⌘Shift+D 切换） */
let _devMode = false;

/**
 * 设置面板左侧导航配置
 * 每个分区的 anchorSelector 指向 [data-settings-section="xxx"] 容器
 */
const SETTINGS_TABS = [
  { id: 'appearance', labelKey: 'settings.tab_appearance', icon: 'palette' },
  { id: 'model', labelKey: 'settings.tab_model', icon: 'cpu' },
  { id: 'kb', labelKey: 'settings.tab_kb', icon: 'database' },
  { id: 'retrieval', labelKey: 'settings.tab_retrieval', icon: 'search' },
  { id: 'security', labelKey: 'settings.tab_security', icon: 'shield' },
  { id: 'data', labelKey: 'settings.tab_data', icon: 'archive' },
  { id: 'application', labelKey: 'settings.tab_application', icon: 'app' },
  { id: 'advanced', labelKey: 'settings.tab_advanced', icon: 'sliders' },
];

/** 当前活动 Tab */
let _activeTab = 'appearance';

/**
 * 查找分区元素。
 * @param {string} sectionId - 分区 ID
 * @returns {HTMLElement | null}
 */
function _findTabSection(sectionId) {
  // @ts-expect-error querySelector 返回 Element，但此处确定是 HTMLElement
  return document.querySelector(`[data-settings-section="${sectionId}"]`);
}

/**
 * 切换到指定 Tab：隐藏所有分区，仅显示目标分区。
 * @param {string} tabId - 要激活的 Tab ID
 */
function _switchTab(tabId) {
  // 隐藏所有分区
  for (const tab of SETTINGS_TABS) {
    const el = _findTabSection(tab.id);
    if (el) el.classList.add('hidden');
  }
  // 显示目标分区
  const target = _findTabSection(tabId);
  if (target) {
    target.classList.remove('hidden');
    // 滚动到顶部
    const scrollContainer = document.getElementById('settingsContent');
    if (scrollContainer) scrollContainer.scrollTop = 0;
  }
  // 更新导航高亮
  _updateTabHighlight(tabId);
}

/**
 * 更新导航高亮状态。
 * @param {string} tabId - 要激活的 Tab ID
 */
function _updateTabHighlight(tabId) {
  const tabBar = document.getElementById('settingsTabBar');
  if (!tabBar) return;

  const buttons = tabBar.querySelectorAll('.settings-nav-item');
  buttons.forEach((btn) => {
    const isActive = btn.dataset.tabId === tabId;
    btn.classList.toggle('settings-nav-active', isActive);
    btn.setAttribute('aria-selected', isActive ? 'true' : 'false');
  });

  _activeTab = tabId;
}

/**
 * 导航图标 SVG（Lucide 风格 16px stroke）
 */
const _SETTINGS_NAV_ICONS = {
  home: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>',
  app: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><circle cx="12" cy="12" r="4"/><line x1="4.93" y1="4.93" x2="9.17" y2="9.17"/><line x1="14.83" y1="14.83" x2="19.07" y2="19.07"/><line x1="14.83" y1="9.17" x2="19.07" y2="4.93"/><line x1="4.93" y1="19.07" x2="9.17" y2="14.83"/></svg>',
  cpu: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M15 2v2M15 20v2M2 15h2M2 9h2M20 15h2M20 9h2M9 2v2M9 20v2"/></svg>',
  palette: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r=".5"/><circle cx="17.5" cy="10.5" r=".5"/><circle cx="8.5" cy="7.5" r=".5"/><circle cx="6.5" cy="12.5" r=".5"/><path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z"/></svg>',
  database: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M3 5v14a9 3 0 0 0 18 0V5"/><path d="M3 12a9 3 0 0 0 18 0"/></svg>',
  search: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>',
  shield: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z"/></svg>',
  archive: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="5" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8"/><line x1="10" y1="12" x2="14" y2="12"/></svg>',
  sliders: '<svg class="settings-nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="21" x2="14" y1="4" y2="4"/><line x1="10" x2="3" y1="4" y2="4"/><line x1="21" x2="12" y1="12" y2="12"/><line x1="8" x2="3" y1="12" y2="12"/><line x1="21" x2="16" y1="20" y2="20"/><line x1="12" x2="3" y1="20" y2="20"/><line x1="14" x2="14" y1="2" y2="6"/><line x1="8" x2="8" y1="10" y2="14"/><line x1="16" x2="16" y1="18" y2="22"/></svg>',
};

/**
 * 创建左侧导航栏。
 * 设计：垂直列表，每项包含图标+文字，点击切换到对应分区（页面切换模式）。
 * 点击 Tab 后，右侧只显示对应分区的设置内容，不再滚动。
 */
function createSettingsTabBar() {
  const nav = document.getElementById('settingsTabBar');
  if (!nav) return;
  nav.innerHTML = '';

  for (const tab of SETTINGS_TABS) {
    const btn = document.createElement('button');
    btn.className = 'settings-nav-item' + (_activeTab === tab.id ? ' settings-nav-active' : '');
    btn.dataset.tabId = tab.id;
    btn.setAttribute('role', 'tab');
    btn.setAttribute('aria-selected', _activeTab === tab.id ? 'true' : 'false');

    const iconHtml = _SETTINGS_NAV_ICONS[tab.icon] || '';
    const labelSpan = document.createElement('span');
    labelSpan.className = 'settings-nav-label';
    labelSpan.textContent = t(tab.labelKey);
    btn.innerHTML = iconHtml;
    btn.appendChild(labelSpan);

    btn.onclick = () => {
      _switchTab(tab.id);
    };
    nav.appendChild(btn);
  }

  // 初始化：只显示当前活动 Tab 对应的分区
  _switchTab(_activeTab);
}

/**
 * 打开设置面板：加载当前 LLM 配置与 VLM 开关状态并渲染。
 */
export async function openSettings() {
  try {
    const settings = await invoke('get_settings');
    const info = $('settingsLlmInfo');
    if (settings.has_llm_config) {
      info.innerHTML =
        '<div class="flex justify-between"><span class="text-text-quaternary">' + t('settings.endpoint') + '</span><span class="text-slate-200 break-all text-right max-w-[60%]">' + settings.base_url + '</span></div>' +
        '<div class="flex justify-between"><span class="text-text-quaternary">' + t('settings.model') + '</span><span class="text-slate-200">' + settings.model + '</span></div>' +
        '<div class="flex justify-between"><span class="text-text-quaternary">' + t('settings.key') + '</span><span class="text-slate-200">' + settings.api_key_masked + '</span></div>';
    } else {
      info.innerHTML = '<div class="text-slate-400 text-center py-2">' + t('settings.not_configured') + '</div>';
    }
    setState({ vlmEnabled: settings.vlm_enabled });
    updateVlmToggle();
    setState({ rerankEnabled: settings.rerank_enabled });
    updateRerankToggle();
    setState({ hydeEnabled: settings.hyde_enabled });
    updateHydeToggle();

    setState({
      hybridEnabled: settings.hybrid_search || false,
      agentEnabled: settings.agent_enabled || false,
      subAgentEnabled: settings.sub_agent_enabled || false,
      memoryEnabled: settings.memory_enabled || false,
      webSearchEnabled: settings.web_search_enabled || false,
    });

    const subAgentToggleEl = $('subAgentToggle');
    if (subAgentToggleEl && settings.sub_agent_enabled) {
      subAgentToggleEl.setAttribute('aria-checked', 'true');
      subAgentToggleEl.classList.remove('bg-slate-600');
      subAgentToggleEl.classList.add('bg-accent');
      subAgentToggleEl.querySelector('span').classList.add('translate-x-5');
    }

    const modelSelect = $('embeddingModelSelect');
    if (modelSelect) {
      modelSelect.value = settings.embedding_model || 'bge-small-en-v1.5';
      modelSelect.onchange = onEmbeddingModelChange;
    }

    await initChunkParams();

    // REQ-VEC-017: 镜像源选择器
    await initMirrorSourceSelector();

    const customUploadBtn = $('customModelUploadBtn');
    if (customUploadBtn) {
      customUploadBtn.onclick = onUploadCustomModel;
    }

    const licenseInfo = $('settingsLicenseInfo');
    if (get('isPro')) {
      licenseInfo.innerHTML = '<div class="flex items-center justify-between"><span class="text-accent">' + t('settings.license_pro') + '</span><button id="deactivateBtn" class="text-xs text-red-400 hover:text-red-300 border border-red-400/40 rounded px-2 py-1 transition-colors">' + t('settings.deactivate') + '</button></div>';
      $('deactivateBtn').onclick = deactivatePro;
    } else {
      licenseInfo.innerHTML = '<span class="text-slate-400">' + t('settings.license_free') + '</span>';
    }

    const localeSelect = $('localeSelect');
    if (localeSelect) {
      localeSelect.value = getLocale();
      localeSelect.onchange = async () => {
        const newLocale = localeSelect.value;
        if (SUPPORTED_LOCALES.includes(newLocale) && newLocale !== getLocale()) {
          await setLocale(newLocale);
          updateProStatus();
          toast(t('settings.language_label') + ': ' + localeSelect.options[localeSelect.selectedIndex].text, 'success');
          await openSettings();
        }
      };
    }

    initThemeSwitcher();
    initVoiceSettings();
    initPdfExportSettings();

    $('settingsModal').classList.remove('hidden');
    createSettingsTabBar();
    loadModelCacheInfo();
    loadCustomModels();

    // 渲染 RAG/LLM 参数区块
    const ragLlmContainer = $('ragLlmParamsContainer');
    if (ragLlmContainer) {
      renderRagLlmParams(ragLlmContainer);
    }

    // 渲染安全防御设置区块
    const secContainer = $('securitySettingsContainer');
    if (secContainer) {
      secContainer.innerHTML = '';
      renderSecuritySettings(secContainer);
    }

    // 渲染记忆管理设置区块
    const memContainer = $('memorySettingsContainer');
    if (memContainer) {
      memContainer.innerHTML = '';
      renderMemorySettings(memContainer);
    }

    // 渲染性能优化设置区块（高级区域）
    const perfContainer = $('perfSettingsContainer');
    if (perfContainer) {
      perfContainer.innerHTML = '';
      renderPerfSettings(perfContainer);
    }

    // 渲染存储空间卡片（数据区域，REQ-ERR-004 / V3.1 P1-5）
    const diskCard = $('diskSpaceCard');
    if (diskCard) {
      renderDiskSpaceCard(diskCard);
    }

    // 渲染 RAG 评估指标设置区块（S5 P0-6：仅开发者模式显示，高级区域）
    if (_devMode) {
      const evalContainer = $('ragEvalSettingsContainer');
      if (evalContainer && typeof renderRagEvalSettings === 'function') {
        renderRagEvalSettings(evalContainer);
      }
      const traceContainer = $('traceBudgetContainer');
      if (traceContainer && typeof renderTraceBudgetSettings === 'function') {
        renderTraceBudgetSettings(traceContainer);
      }
      // 嵌入模型评估按钮绑定（REQ-VEC-018）
      const embedEvalBtn = $('embedEvalBtn');
if (embedEvalBtn) {
embedEvalBtn.addEventListener('click', () => openEmbedEvalPanel());
}
// MCP 服务器配置按钮绑定（REQ-ARCH-016）
const mcpBtn = $('mcpBtn');
if (mcpBtn) {
mcpBtn.addEventListener('click', () => openMcpSettingsPanel());
}
    } else {
      ['ragEvalSettingsContainer', 'traceBudgetContainer'].forEach((id) => {
        const el = $(id);
        if (el) el.innerHTML = '';
      });
    }

    // 渲染自定义快捷指令模板区块（高级区域）
    const tmplContainer = $('promptTemplateContainer');
    if (tmplContainer) {
      tmplContainer.innerHTML = '';
      renderPromptTemplateSettings(tmplContainer);
    }

    // 渲染本地 LLM 高级设置区块（高级区域）
    const kvContainer = $('kvCacheContainer');
    if (kvContainer) renderKvCacheSettings(kvContainer);

    const deviceContainer = $('deviceKindContainer');
    if (deviceContainer) renderDeviceKindSettings(deviceContainer);

    const pagedAttnContainer = $('pagedAttnContainer');
    if (pagedAttnContainer) renderPagedAttnSettings(pagedAttnContainer);

    const kernelModeContainer = $('kernelModeContainer');
    if (kernelModeContainer) renderKernelModeSettings(kernelModeContainer);

const embedderContainer = $('embedderModelContainer');
if (embedderContainer) renderEmbedderModelSettings(embedderContainer);

// 嵌入模型对比评估按钮（REQ-VEC-018，开发者工具门控）
// 注意：绑定放在 _devMode 块中（下方），与 rag-eval/trace-panel 一致

    // 渲染窗口管理设置（通用区域）
    const windowSettingsContainer = $('windowSettingsContainer');
    if (windowSettingsContainer) renderWindowSettings(windowSettingsContainer);

    // 渲染错误日志导出（数据与日志区域）
    const errorLogsContainer = $('errorLogsContainer');
    if (errorLogsContainer) renderErrorLogsSettings(errorLogsContainer);

    // 渲染开机自启 + 应用更新检查（通用区域）
    const startupContainer = $('startupSettingsContainer');
    if (startupContainer) renderStartupSettings(startupContainer);

    loadWatchedFolders();

    await loadLlmModeSetting(settings);
    await loadLocalModels();
    await loadRecommendedModels();

    await loadSamplingParams(settings);

    loadContextTokenLimit(settings);
    loadTokenCostSettings(settings);

    await loadObservabilitySettings();

    // 激活 Focus Trap
    if (_settingsTrap) {
      _settingsTrap.deactivate();
    }
    _settingsTrap = createFocusTrap($('settingsModal'));
    _settingsTrap.activate();

    pushPanel({ id: 'settings', close: closeSettings, element: $('settingsModal'), label: 'Settings' });
  } catch (err) {
    toastError(err);
  }
}

/** 关闭设置面板。 */
export function closeSettings() {
  removePanel('settings');
  if (_settingsTrap) {
    _settingsTrap.deactivate();
    _settingsTrap = null;
  }
  // 重置到默认 Tab
  _activeTab = 'appearance';
  $('settingsModal').classList.add('hidden');
}

/**
 * 初始化设置面板事件绑定。
 */
export function initSettings() {
  $('settingsClose').onclick = closeSettings;
  if ($('settingsCloseBtn')) $('settingsCloseBtn').onclick = closeSettings;
  $('settingsEditLlm').onclick = () => { closeSettings(); showWizard(); };
  $('settingsInitEmbedder').onclick = initEmbedder;
  $('settingsClearCache').onclick = clearModelCache;
  $('vlmToggle').onclick = onVlmToggle;
  $('vlmConfirmOk').onclick = confirmVlmEnable;
  $('vlmConfirmCancel').onclick = cancelVlmEnable;
  if ($('vlmConfirmCloseBtn')) $('vlmConfirmCloseBtn').onclick = cancelVlmEnable;
  if ($('rerankToggle')) $('rerankToggle').onclick = onRerankToggle;
  if ($('hydeToggle')) $('hydeToggle').onclick = onHydeToggle;
  if ($('coordinatorToggle')) $('coordinatorToggle').onclick = onCoordinatorToggle;
  if ($('subAgentToggle')) $('subAgentToggle').onclick = onSubAgentToggle;
  if ($('syncAddBtn')) $('syncAddBtn').onclick = onAddWatchedFolder;

  // 本地 LLM 模式切换
  const llmModeRadios = document.querySelectorAll('input[name="llmMode"]');
  llmModeRadios.forEach((radio) => { radio.onchange = () => onLlmModeChange(radio.value); });
  if ($('llmDownloadCancelBtn')) $('llmDownloadCancelBtn').onclick = onCancelDownload;
  if ($('llmDownloadPauseBtn')) $('llmDownloadPauseBtn').onclick = onPauseDownload;
  if ($('openDownloadManagerBtn')) $('openDownloadManagerBtn').onclick = openDownloadManager;

  registerModelLoadListener();

  // S5 审计 P0-6：⌘Shift+D 切换开发者模式
  if (!window._devModeHandlerInstalled) {
    window._devModeHandlerInstalled = true;
    document.addEventListener('keydown', (e) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'D') {
        e.preventDefault();
        _devMode = !_devMode;
        // 如果设置面板已打开，重新渲染
        const modal = $('settingsModal');
        if (modal && !modal.classList.contains('hidden')) {
          openSettings();
        }
        if (typeof toast === 'function') {
          toast(_devMode ? '开发者模式已开启' : '开发者模式已关闭', 'info');
        }
      }
    });
  }
}
