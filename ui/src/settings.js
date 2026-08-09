/**
 * EchoMind 设置面板模块 — LLM 配置 / 推理模式 / 语言切换 / VLM 开关 / 授权状态 / 模型缓存 / 本地模型管理。
 *
 * 职责：
 * 1. 加载并渲染设置面板
 * 2. VLM Toggle 交互（含隐私确认弹窗）
 * 3. 模型缓存信息 / 下载 / 清理
 * 4. 界面语言切换（REQ-I18N-001）
 * 5. 本地 LLM 推理模式切换 + GGUF 模型管理（REQ-LLM-003/004）
 */

import { setState, get } from './state.js';
import { $, formatBytes } from './utils.js';
import { invoke, listen } from './ipc.js';
import { embedModelApi, syncApi, localLlmApi } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { showWizard } from './chat-render.js';
import { deactivatePro, updateProStatus } from './paywall.js';
import { t, getLocale, setLocale, SUPPORTED_LOCALES } from './i18n.js';
import { renderSecuritySettings } from './security.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { createFocusTrap } from './focus-trap.js';
import { renderPerfSettings } from './perf-settings.js';
import { renderMemorySettings } from './memory-panel.js';
import { resetCustomTemplates, loadCustomTemplates } from './slash-commands.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { loadLlmModeSetting, loadLocalModels, loadRecommendedModels,
         registerModelLoadListener, onLlmModeChange, downloadLocalModel,
         deleteLocalModel, selectLocalModel, onCancelDownload, onPauseDownload } from './settings-local-llm.js';
import { openDownloadManager } from './download-manager.js';
import { getPdfPageSize, setPdfPageSize, getPdfIncludeSources, setPdfIncludeSources } from './export.js';

/** 设置面板的 Focus Trap 实例（REQ-A11Y-002） */
let _settingsTrap = null;

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
    
    // 方案4：同步功能toggle状态到state.js
    setState({ 
      hybridEnabled: settings.hybrid_search || false,
      agentEnabled: settings.agent_enabled || false,
      subAgentEnabled: settings.sub_agent_enabled || false,
      memoryEnabled: settings.memory_enabled || false,
      webSearchEnabled: settings.web_search_enabled || false
    });

// 子代理舰队初始状态（REQ-RAG-025 扩展）
const subAgentToggleEl = $('subAgentToggle');
if (subAgentToggleEl && settings.sub_agent_enabled) {
  subAgentToggleEl.setAttribute('aria-checked', 'true');
  subAgentToggleEl.classList.remove('bg-slate-600');
  subAgentToggleEl.classList.add('bg-accent');
  subAgentToggleEl.querySelector('span').classList.add('translate-x-5');
}
// 协调模式初始状态（REQ-RAG-025）
const coordinatorToggleEl = $('coordinatorToggle');
if (coordinatorToggleEl) {
  // 从 settings 读取协调模式状态（若后端返回了该字段）
  // coordinator_enabled 不在 SettingsPayload 中，通过 sub_agent_enabled 间接推断
  // 或从独立的状态读取（此处保持默认 false，用户手动开启）
}

  // 嵌入模型选择器（REQ-VEC-012）
  const modelSelect = $('embeddingModelSelect');
  if (modelSelect) {
    modelSelect.value = settings.embedding_model || 'all-MiniLM-L6-v2';
    modelSelect.onchange = onEmbeddingModelChange;
  }

  // 自定义嵌入模型上传按钮（REQ-VEC-014，Pro 门控）
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

    // 语言选择器
    const localeSelect = $('localeSelect');
    if (localeSelect) {
      localeSelect.value = getLocale();
      localeSelect.onchange = async () => {
        const newLocale = localeSelect.value;
        if (SUPPORTED_LOCALES.includes(newLocale) && newLocale !== getLocale()) {
        await setLocale(newLocale);
        updateProStatus();
        toast(t('settings.language_label') + ': ' + localeSelect.options[localeSelect.selectedIndex].text, 'success');
          // 重新渲染设置面板以更新文案
          await openSettings();
        }
      };
    }

    // 主题切换器（REQ-UI-011 浅色主题切换）
    initThemeSwitcher();

    // 语音设置初始化（REQ-RAG-034/035）
    initVoiceSettings();

    // PDF 导出设置初始化（REQ-EXP-005）
    initPdfExportSettings();

    $('settingsModal').classList.remove('hidden');
    loadModelCacheInfo();
    // 加载自定义嵌入模型列表（REQ-VEC-014，Pro 门控）
    loadCustomModels();

    // 渲染安全防御设置区块
    const secContainer = $('securitySettingsContainer');
    if (secContainer) {
      secContainer.innerHTML = '';
      renderSecuritySettings(secContainer);
    }

    // 渲染记忆管理设置区块（S34-S43 持久化记忆系统）
    const memContainer = $('memorySettingsContainer');
    if (memContainer) {
      memContainer.innerHTML = '';
      renderMemorySettings(memContainer);
    }

    // 渲染性能优化设置区块（S34-S43 性能优化前端 UI）
    const perfContainer = $('perfSettingsContainer');
    if (perfContainer) {
      perfContainer.innerHTML = '';
      renderPerfSettings(perfContainer);
    }

    // 渲染自定义快捷指令模板区块（S56）
    let tmplContainer = $('promptTemplateContainer');
    if (!tmplContainer && perfContainer) {
      // 动态创建容器（index.html 可能未包含）
      tmplContainer = document.createElement('div');
      tmplContainer.id = 'promptTemplateContainer';
      tmplContainer.className = 'mt-5';
      perfContainer.after(tmplContainer);
    }
    if (tmplContainer) {
      tmplContainer.innerHTML = '';
      renderPromptTemplateSettings(tmplContainer);
    }

    // 加载监听文件夹列表（REQ-SYNC-001）
    loadWatchedFolders();

    // 加载本地 LLM 推理模式与模型列表（REQ-LLM-003/004）
    await loadLlmModeSetting(settings);
    await loadLocalModels();
    await loadRecommendedModels();

    // S11：加载采样参数
    await loadSamplingParams(settings);

    // REQ-RAG-017：加载上下文 token 限制
    loadContextTokenLimit(settings);

    // 加载 Token 用量追踪与预算
    loadTokenCostSettings(settings);

    // REQ-OBS-001/002：加载可观测性设置（日志级别 + 诊断导出）
    await loadObservabilitySettings();

    // 激活 Focus Trap（REQ-A11Y-002）：Tab 键锁定在设置面板内
    if (_settingsTrap) {
      _settingsTrap.deactivate();
    }
    _settingsTrap = createFocusTrap($('settingsModal'));
    _settingsTrap.activate();

    // 注册到面板栈
    pushPanel({ id: 'settings', close: closeSettings, element: $('settingsModal'), label: 'Settings' });
  } catch (err) {
    toastError(err);
  }
}

/** 关闭设置面板。 */
export function closeSettings() {
  // 从面板栈移除
  removePanel('settings');
  // 停用 Focus Trap（恢复焦点到触发元素，REQ-A11Y-002）
  if (_settingsTrap) {
    _settingsTrap.deactivate();
    _settingsTrap = null;
  }
  $('settingsModal').classList.add('hidden');
}

/**
 * 加载模型缓存信息（REQ-VEC-008-AC-5）。
 */
export async function loadModelCacheInfo() {
  try {
    const info = await invoke('get_model_cache_info');
    const el = $('settingsCacheInfo');
    if (info.models.length === 0) {
      el.innerHTML = '<div class="text-slate-400">' + t('settings.no_model') + '</div>';
    } else {
      const sizeStr = formatBytes(info.total_size_bytes);
      const modelList = info.models.map((m) => `${m.name} (${formatBytes(m.size_bytes)})`).join(' · ');
      el.innerHTML = '<div class="flex justify-between"><span class="text-text-quaternary">' + t('settings.total_size') + '</span><span class="text-slate-200">' + sizeStr + '</span></div>' +
        '<div class="text-[11px] text-text-quaternary">' + modelList + '</div>';
    }
  } catch (err) {
    toastError(err);
  }
}

/**
 * 显式初始化向量化引擎（REQ-VEC-008-AC-1~4）。
 */
export async function initEmbedder() {
  try {
    toast(t('settings.downloading_model'), 'info');
    await invoke('init_embedder');
  } catch (err) {
    toastError(err);
  }
}

/**
 * 清理模型缓存（REQ-VEC-008-AC-6）。
 */
export async function clearModelCache() {
  try {
    const freed = await invoke('clear_model_cache', { modelName: null });
    toastSuccess(t('settings.cache_cleared', { size: formatBytes(freed) }));
    loadModelCacheInfo();
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// VLM Toggle
// ============================================================

/** 更新 VLM Toggle 开关的视觉状态。 */
export function updateVlmToggle() {
  const toggle = $('vlmToggle');
  const knob = toggle.querySelector('span');
  const privacy = $('vlmPrivacy');
  const enabled = get('vlmEnabled');
  if (enabled) {
    toggle.classList.remove('bg-slate-600');
    toggle.classList.add('bg-accent');
    toggle.setAttribute('aria-checked', 'true');
    knob.classList.add('translate-x-5');
    privacy.classList.remove('hidden');
  } else {
    toggle.classList.add('bg-slate-600');
    toggle.classList.remove('bg-accent');
    toggle.setAttribute('aria-checked', 'false');
    knob.classList.remove('translate-x-5');
    privacy.classList.add('hidden');
  }
}

/** VLM Toggle 点击事件。 */
export function onVlmToggle() {
  if (get('vlmEnabled')) {
    setState({ vlmEnabled: false });
    updateVlmToggle();
    saveVlmSetting(false);
  } else {
    $('vlmConfirm').classList.remove('hidden');
  }
}

/** 隐私确认弹窗：确认开启 VLM。 */
export async function confirmVlmEnable() {
  $('vlmConfirm').classList.add('hidden');
  setState({ vlmEnabled: true });
  updateVlmToggle();
  await saveVlmSetting(true);
}

/** 隐私确认弹窗：取消。 */
export function cancelVlmEnable() {
  $('vlmConfirm').classList.add('hidden');
}

/** 持久化 VLM 开关状态到后端。 */
async function saveVlmSetting(enabled) {
  try {
    await invoke('set_vlm_enabled', { enabled });
    toast(enabled ? t('settings.vlm_enabled') : t('settings.vlm_disabled'), 'success');
  } catch (err) {
    toastError(err);
    setState({ vlmEnabled: !enabled });
    updateVlmToggle();
  }
}

// ============================================================
// Cross-Encoder Rerank Toggle (REQ-RAG-020)
// ============================================================

/** 更新重排序 Toggle 开关的视觉状态。 */
export function updateRerankToggle() {
  const toggle = $('rerankToggle');
  if (!toggle) return;
  const knob = toggle.querySelector('span');
  const enabled = get('rerankEnabled');
  if (enabled) {
    toggle.classList.remove('bg-slate-600');
    toggle.classList.add('bg-accent');
    toggle.setAttribute('aria-checked', 'true');
    knob.classList.add('translate-x-5');
  } else {
    toggle.classList.add('bg-slate-600');
    toggle.classList.remove('bg-accent');
    toggle.setAttribute('aria-checked', 'false');
    knob.classList.remove('translate-x-5');
  }
}

/** 重排序 Toggle 点击事件。 */
export function onRerankToggle() {
  const newEnabled = !get('rerankEnabled');
  setState({ rerankEnabled: newEnabled });
  updateRerankToggle();
  saveRerankSetting(newEnabled);
}

/** 持久化重排序开关状态到后端。 */
async function saveRerankSetting(enabled) {
try {
await invoke('set_rerank_enabled', { enabled });
toast(enabled ? t('settings.rerank_enabled') : t('settings.rerank_disabled'), 'success');
} catch (err) {
toastError(err);
setState({ rerankEnabled: !enabled });
updateRerankToggle();
}
}

// ============================================================
// HyDE Query Rewriting Toggle (REQ-RAG-021)
// ============================================================

/** 更新 HyDE 查询改写 Toggle 开关的视觉状态。 */
export function updateHydeToggle() {
const toggle = $('hydeToggle');
if (!toggle) return;
const knob = toggle.querySelector('span');
const enabled = get('hydeEnabled');
if (enabled) {
toggle.classList.remove('bg-slate-600');
toggle.classList.add('bg-accent');
toggle.setAttribute('aria-checked', 'true');
knob.classList.add('translate-x-5');
} else {
toggle.classList.add('bg-slate-600');
toggle.classList.remove('bg-accent');
toggle.setAttribute('aria-checked', 'false');
knob.classList.remove('translate-x-5');
}
}

/** HyDE Toggle 点击事件。 */
export function onHydeToggle() {
const newEnabled = !get('hydeEnabled');
setState({ hydeEnabled: newEnabled });
updateHydeToggle();
saveHydeSetting(newEnabled);
}

/** 持久化 HyDE 查询改写开关状态到后端。 */
async function saveHydeSetting(enabled) {
try {
await invoke('set_hyde_enabled', { enabled });
toast(enabled ? t('settings.hyde_enabled') : t('settings.hyde_disabled'), 'success');
} catch (err) {
toastError(err);
setState({ hydeEnabled: !enabled });
updateHydeToggle();
}
}

/** 切换多代理协调模式开关（REQ-RAG-025）。 */
export async function onCoordinatorToggle() {
const toggle = $('coordinatorToggle');
const enabled = toggle.getAttribute('aria-checked') === 'true';
const newEnabled = !enabled;
toggle.setAttribute('aria-checked', String(newEnabled));
toggle.classList.toggle('bg-accent', newEnabled);
toggle.classList.toggle('bg-slate-600', !newEnabled);
toggle.querySelector('span').classList.toggle('translate-x-5', newEnabled);
try {
await invoke('set_coordinator_mode', { enabled: newEnabled });
toast(newEnabled ? t('settings.coordinator_enabled') : t('settings.coordinator_disabled'), 'success');
} catch (err) {
toastError(err);
toggle.setAttribute('aria-checked', String(enabled));
toggle.classList.toggle('bg-accent', enabled);
toggle.classList.toggle('bg-slate-600', !enabled);
toggle.querySelector('span').classList.toggle('translate-x-5', enabled);
}
}

/** 切换子代理舰队模式开关（REQ-RAG-025 扩展）。 */
export async function onSubAgentToggle() {
  const toggle = $('subAgentToggle');
  if (!toggle) return;
  const currentEnabled = get('subAgentEnabled');
  const newEnabled = !currentEnabled;
  
  // 乐观更新state
  setState({ subAgentEnabled: newEnabled });
  
  // 更新UI
  toggle.setAttribute('aria-checked', String(newEnabled));
  toggle.classList.toggle('bg-accent', newEnabled);
  toggle.classList.toggle('bg-slate-600', !newEnabled);
  toggle.querySelector('span').classList.toggle('translate-x-5', newEnabled);
  
  try {
    await invoke('set_sub_agent_enabled', { enabled: newEnabled });
    toast(newEnabled ? t('settings.sub_agent_enabled') : t('settings.sub_agent_disabled'), 'success');
  } catch (err) {
    toastError(err);
    // 回滚state
    setState({ subAgentEnabled: currentEnabled });
    toggle.setAttribute('aria-checked', String(currentEnabled));
    toggle.classList.toggle('bg-accent', currentEnabled);
    toggle.classList.toggle('bg-slate-600', !currentEnabled);
    toggle.querySelector('span').classList.toggle('translate-x-5', currentEnabled);
  }
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
  // 文件监听 + 增量同步（REQ-SYNC-001~003）
  if ($('syncAddBtn')) $('syncAddBtn').onclick = onAddWatchedFolder;

  // 本地 LLM 模式切换（REQ-LLM-003）
  const llmModeRadios = document.querySelectorAll('input[name="llmMode"]');
  llmModeRadios.forEach((radio) => { radio.onchange = () => onLlmModeChange(radio.value); });
  if ($('llmDownloadCancelBtn')) $('llmDownloadCancelBtn').onclick = onCancelDownload;
  if ($('llmDownloadPauseBtn')) $('llmDownloadPauseBtn').onclick = onPauseDownload;
  if ($('openDownloadManagerBtn')) $('openDownloadManagerBtn').onclick = openDownloadManager;

  // 注册 model_load_progress 事件监听（REQ-LLM-003 AC-4）
  registerModelLoadListener();
}

// ============================================================
// Embedding Model Selector (REQ-VEC-012)
// ============================================================

/**
 * 嵌入模型选择器 change 事件（REQ-VEC-012）。
 *
 * 切换模型后显示「建议重建索引」提示，不自动触发重建。
 * 如果当前有已索引文档，显示警告。
 */
async function onEmbeddingModelChange() {
  const select = $('embeddingModelSelect');
  if (!select) return;
  const newModel = select.value;
  try {
    await embedModelApi.setModel(newModel);
    // 显示重建索引提示
    const warning = $('embeddingModelWarning');
    if (warning) {
      warning.classList.remove('hidden');
    }
    toast(t('settings.embedding_model_switched', { model: newModel }), 'success');
    // 刷新缓存信息（新模型可能已缓存）
    loadModelCacheInfo();
  } catch (err) {
    toastError(err);
    // 恢复原选择
    await openSettings();
  }
}

// ============================================================
// 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）
// ============================================================

/**
 * 加载自定义模型列表并渲染到设置面板（REQ-VEC-014-AC-2）。
 *
 * 调用 `list_custom_models` IPC 获取已上传的模型列表，
 * 渲染为列表项（名称 + 大小 + 有效性 + 切换/删除按钮）。
 * Free 用户调用会收到 PRO_REQUIRED 错误，显示升级提示。
 */
async function loadCustomModels() {
  const container = $('customModelList');
  if (!container) return;
  try {
    const models = await embedModelApi.listCustomModels();
    if (models.length === 0) {
      container.innerHTML = `<p class="text-text-secondary text-sm py-2">${t('settings.custom_model_empty')}</p>`;
      return;
    }
    container.innerHTML = models
      .map(
        (m) => `
      <div class="flex items-center justify-between gap-2 py-2 px-3 rounded-lg bg-surface-2 border border-border-default" data-custom-model="${m.name}">
        <div class="flex-1 min-w-0">
          <span class="text-text-primary text-sm font-medium truncate">${m.name}</span>
          ${m.is_valid ? '' : `<span class="ml-2 text-xs text-warning">${t('settings.custom_model_invalid')}</span>`}
          <span class="ml-2 text-xs text-text-secondary">${formatBytes(m.size_bytes)}</span>
        </div>
        <div class="flex gap-1">
          <button class="btn-icon text-xs px-2 py-1 rounded text-accent hover:bg-surface-3" onclick="switchToCustomModel('${m.name}')" title="${t('settings.custom_model_switch')}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"/></svg>
          </button>
          <button class="btn-icon text-xs px-2 py-1 rounded text-error hover:bg-surface-3" onclick="deleteCustomModel('${m.name}')" title="${t('settings.custom_model_delete')}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
          </button>
        </div>
      </div>`
      )
      .join('');
  } catch (err) {
    const msg = String(err);
    if (msg.includes('PRO_REQUIRED')) {
      container.innerHTML = `<p class="text-text-secondary text-sm py-2">${t('settings.custom_model_pro_required')}</p>`;
    } else {
      container.innerHTML = `<p class="text-error text-sm py-2">${msg}</p>`;
    }
  }
}

/**
 * 上传自定义 ONNX 嵌入模型（REQ-VEC-014-AC-1）。
 *
 * 1. 弹出输入框获取模型名称
 * 2. 弹出文件选择器选择 ONNX 文件
 * 3. 弹出文件选择器选择 tokenizer 文件（多选）
 * 4. 调用 upload_custom_embedding_model IPC
 */
async function onUploadCustomModel() {
  try {
    // 1. 选择 ONNX 文件
    const onnxPath = await window.__TAURI__.dialog.open({
      multiple: false,
      title: t('settings.custom_model_select_onnx'),
      filters: [{ name: 'ONNX Model', extensions: ['onnx'] }],
    });
    if (!onnxPath) return;

    // 2. 选择 tokenizer 文件（多选）
    const tokenizerPaths = await window.__TAURI__.dialog.open({
      multiple: true,
      title: t('settings.custom_model_select_tokenizer'),
      filters: [{ name: 'Tokenizer Files', extensions: ['json', 'txt'] }],
    });
    if (!tokenizerPaths || (Array.isArray(tokenizerPaths) && tokenizerPaths.length === 0)) return;

    // 3. 获取模型名称
    const defaultName = typeof onnxPath === 'string'
      ? onnxPath.split('/').pop()?.replace('.onnx', '') || 'custom-model'
      : 'custom-model';
    const name = await window.__TAURI__.dialog
      ? prompt(t('settings.custom_model_name'), defaultName)
      : defaultName;
    if (!name || !name.trim()) return;

    // 4. 上传
    const tokenizerFiles = Array.isArray(tokenizerPaths) ? tokenizerPaths : [tokenizerPaths];
    const info = await embedModelApi.uploadCustomModel(
      name.trim(),
      typeof onnxPath === 'string' ? onnxPath : onnxPath[0],
      tokenizerFiles
    );
    toast(t('settings.custom_model_upload_success', { name: info.name }), 'success');
    await loadCustomModels();
  } catch (err) {
    const msg = String(err);
    if (msg.includes('PRO_REQUIRED')) {
      toast(t('settings.custom_model_pro_required'), 'warning');
    } else {
      toast(t('settings.custom_model_upload_failed', { error: msg }), 'error');
    }
  }
}

/**
 * 切换到自定义嵌入模型（REQ-VEC-014-AC-2）。
 *
 * 设置 vec.embedding_model 为 "custom:{name}"，触发 embedder 缓存清除。
 * @param {string} name - 自定义模型名称
 */
async function switchToCustomModel(name) {
  try {
    await embedModelApi.setModel(`custom:${name}`);
    toast(t('settings.custom_model_switched', { name }), 'success');
    await openSettings();
  } catch (err) {
    toastError(err);
  }
}

/**
 * 删除自定义嵌入模型（REQ-VEC-014-AC-4）。
 *
 * 弹出确认对话框，确认后调用 delete_custom_model IPC。
 * @param {string} name - 要删除的自定义模型名称
 */
async function deleteCustomModel(name) {
  const confirmed = confirm(t('settings.custom_model_delete_confirm', { name }));
  if (!confirmed) return;
  try {
    await embedModelApi.deleteCustomModel(name);
    toast(t('settings.custom_model_deleted', { name }), 'success');
    await loadCustomModels();
  } catch (err) {
    toastError(err);
  }
}


/**
 * 添加监听文件夹（REQ-SYNC-001 AC-2）。
 *
 * 弹出系统文件夹选择器，选择后添加到监听列表并触发首次同步。
 */
async function onAddWatchedFolder() {
  try {
    const selected = await window.__TAURI__.dialog.open({
      directory: true,
      multiple: false,
      title: t('settings.sync_select_folder'),
    });
    if (!selected) return;
    const folderPath = typeof selected === 'string' ? selected : selected[0];
    if (!folderPath) return;

    toast(t('settings.sync_starting'), 'info');
    const result = await syncApi.add(folderPath);
    toast(
      t('settings.sync_complete', {
        added: result.added,
        updated: result.updated,
        deleted: result.deleted,
      }),
      'success'
    );
    await loadWatchedFolders();
  } catch (err) {
    toastError(err);
  }
}

/**
 * 移除监听文件夹（REQ-SYNC-001 AC-4）。
 *
 * 仅停止监听，不删除已导入的文档。
 *
 * @param {string} folderPath - 监听文件夹路径
 */
async function onRemoveWatchedFolder(folderPath) {
  try {
    await syncApi.remove(folderPath);
    toast(t('settings.sync_removed'), 'success');
    await loadWatchedFolders();
  } catch (err) {
    toastError(err);
  }
}
// 暴露为全局函数（内联 onclick 需要）
window.onRemoveWatchedFolder = onRemoveWatchedFolder;

/**
 * 加载并渲染监听文件夹列表（REQ-SYNC-001 AC-3）。
 *
 * 每个条目显示路径、同步状态与最后同步时间。
 */
async function loadWatchedFolders() {
  const container = $('syncFolderList');
  if (!container) return;

  try {
    const folders = await syncApi.list();
    if (folders.length === 0) {
      container.innerHTML = `<p class="text-sm text-text-quaternary py-2">${t('settings.sync_empty')}</p>`;
      return;
    }

    container.innerHTML = folders
      .map(
        (f) => `
      <div class="flex items-center justify-between gap-2 py-2 px-3 rounded-lg bg-slate-800/50">
        <div class="flex-1 min-w-0">
          <p class="text-sm text-slate-200 truncate">${f.name}</p>
          <p class="text-xs text-text-quaternary truncate">${f.path}</p>
          <p class="text-xs ${f.sync_status === 'idle' ? 'text-emerald-400' : 'text-text-quaternary'}">
            ${f.sync_status === 'idle' ? t('settings.sync_status_idle') : t('settings.sync_status_stopped')}
            ${f.last_synced_at ? ' · ' + new Date(f.last_synced_at * 1000).toLocaleString() : ''}
          </p>
        </div>
        <button class="shrink-0 px-2 py-1 text-xs rounded-lg bg-red-500/15 text-red-300 border border-red-400/40 hover:bg-red-500/25 transition-colors"
          onclick="onRemoveWatchedFolder('${f.path.replace(/'/g, "\\'")}')"
        >${t('settings.sync_remove')}</button>
      </div>`
      )
      .join('');
  } catch (err) {
    container.innerHTML = `<p class="text-sm text-red-400 py-2">${t('settings.sync_load_error')}</p>`;
  }
}


// ============================================================
// 对话上下文长度管理（REQ-RAG-017）
// ============================================================

/**
 * 从 get_settings 返回的 context_token_limit 字段恢复滑块状态（REQ-RAG-017 AC-4）。
 * @param {Object} settings - get_settings 命令返回的设置对象
 */
function loadContextTokenLimit(settings) {
  const slider = $('contextTokenSlider');
  const valueEl = $('contextTokenValue');
  if (!slider || !valueEl) return;

  const limit = settings.context_token_limit || 4096;
  slider.value = limit;
  valueEl.textContent = t('settings.context_token_value', { value: limit });

  // 滑块变化时更新显示值并保存
  slider.oninput = () => {
    valueEl.textContent = t('settings.context_token_value', { value: slider.value });
  };
  slider.onchange = async () => {
    const limit = parseInt(slider.value, 10);
    if (limit < 2048 || limit > 32768) return;
    try {
      await invoke('set_context_token_limit', { limit });
      toastSuccess(t('settings.context_token_saved'));
    } catch (err) {
      toastError(err);
    }
  };
}

// ============================================================
// Token 用量追踪与预算
// ============================================================

/**
 * 从 get_settings 返回的 token_budget 字段恢复预算输入状态，
 * 并异步加载累计 token 用量数据。
 * @param {Object} settings - get_settings 命令返回的设置对象
 */
function loadTokenCostSettings(settings) {
  // 预算输入框
  const budgetInput = $('tokenBudgetInput');
  if (budgetInput) {
    budgetInput.value = settings.token_budget || 0;
    let saveTimer = null;
    budgetInput.oninput = () => {
      // 防抖保存
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        const budget = parseInt(budgetInput.value, 10) || 0;
        if (budget < 0) return;
        try {
          await invoke('set_token_budget', { budget });
          toastSuccess(t('settings.cost_budget_saved'));
          // 刷新进度条
          await refreshCostUsage();
        } catch (err) {
          toastError(err);
        }
      }, 600);
    };
  }

  // 异步加载累计用量
  refreshCostUsage();
}

/**
 * 刷新累计 token 用量显示（调用 get_conversation_cost 命令）。
 */
async function refreshCostUsage() {
  const promptEl = $('costPromptTokens');
  const completionEl = $('costCompletionTokens');
  const totalEl = $('costTotalTokens');
  const exchangesEl = $('costExchanges');
  const budgetBar = $('costBudgetBar');
  const budgetFill = $('costBudgetFill');
  const budgetUsed = $('costBudgetUsed');
  const budgetLimit = $('costBudgetLimit');

  if (!promptEl) return;

  try {
    const cost = await invoke('get_conversation_cost');
    promptEl.textContent = formatNumber(cost.total_prompt_tokens);
    completionEl.textContent = formatNumber(cost.total_completion_tokens);
    totalEl.textContent = formatNumber(cost.total_tokens);
    if (exchangesEl) {
      exchangesEl.textContent = t('settings.cost_exchanges', { count: cost.exchange_count });
    }

    // 预算进度条
    if (cost.token_budget > 0) {
      if (budgetBar) budgetBar.classList.remove('hidden');
      const pct = Math.min(100, (cost.total_tokens / cost.token_budget) * 100);
      if (budgetFill) {
        budgetFill.style.width = pct + '%';
        // 超限时变红
        budgetFill.classList.toggle('bg-red-500', pct >= 100);
        budgetFill.classList.toggle('bg-accent', pct < 100);
      }
      if (budgetUsed) budgetUsed.textContent = formatNumber(cost.total_tokens);
      if (budgetLimit) budgetLimit.textContent = formatNumber(cost.token_budget);
    } else {
      if (budgetBar) budgetBar.classList.add('hidden');
    }
  } catch (err) {
    // 忽略错误（可能是旧版本后端无此命令）
  }
}

/**
 * 数字格式化（千分位逗号）。
 * @param {number} n
 * @returns {string}
 */
function formatNumber(n) {
  if (n >= 1000000) {
    return (n / 1000000).toFixed(1) + 'M';
  } else if (n >= 1000) {
    return (n / 1000).toFixed(1) + 'K';
  }
  return String(n);
}

// ============================================================
// 采样参数配置（S11）
// ============================================================

/**
 * 采样参数当前值缓存（从 settings.llm_sampling 恢复）。
 */
let _samplingParams = {
  temperature: null,
  top_p: null,
  top_k: null,
  max_tokens: null,
  frequency_penalty: null,
  presence_penalty: null,
};

/**
 * 从 get_settings 返回的 llm_sampling 字段恢复采样参数 UI 状态（S11）。
 * @param {Object} settings - get_settings 命令返回的设置对象
 */
async function loadSamplingParams(settings) {
  const params = settings.llm_sampling;
  if (params) {
    _samplingParams = { ..._samplingParams, ...params };
  }

  // 填充输入框值
  const tempInput = $('samplingTemperature');
  if (tempInput) tempInput.value = params?.temperature ?? '';

  const topPInput = $('samplingTopP');
  if (topPInput) topPInput.value = params?.top_p ?? '';

  const topKInput = $('samplingTopK');
  if (topKInput) topKInput.value = params?.top_k ?? '';

  const maxTokensInput = $('samplingMaxTokens');
  if (maxTokensInput) maxTokensInput.value = params?.max_tokens ?? '';

  const freqPenaltyInput = $('samplingFreqPenalty');
  if (freqPenaltyInput) freqPenaltyInput.value = params?.frequency_penalty ?? '';

  const presPenaltyInput = $('samplingPresPenalty');
  if (presPenaltyInput) presPenaltyInput.value = params?.presence_penalty ?? '';
}

/**
 * 收集当前 UI 输入框中的采样参数值。
 * 空字符串转为 null（表示使用引擎默认值）。
 * @returns {Object} LlmSamplingParams 对象
 */
function collectSamplingParams() {
  const parse = (id) => {
    const el = $(id);
    if (!el || el.value.trim() === '') return null;
    return parseFloat(el.value);
  };
  const parseIntVal = (id) => {
    const el = $(id);
    if (!el || el.value.trim() === '') return null;
    return parseInt(el.value, 10);
  };

  return {
    temperature: parse('samplingTemperature'),
    top_p: parse('samplingTopP'),
    top_k: parseIntVal('samplingTopK'),
    max_tokens: parseIntVal('samplingMaxTokens'),
    frequency_penalty: parse('samplingFreqPenalty'),
    presence_penalty: parse('samplingPresPenalty'),
  };
}

/**
 * 保存采样参数到后端（S11）。
 * Pro 门控：Free 用户调用会被后端拦截。
 */
async function saveSamplingParams() {
  const params = collectSamplingParams();
  try {
    await localLlmApi.setSamplingParams(params);
    _samplingParams = params;
    toastSuccess(t('settings.llm_sampling_saved'));
  } catch (err) {
    toastError(err);
  }
}

/**
 * 重置采样参数为默认值（所有字段清空，即使用引擎默认值）。
 */
async function resetSamplingParams() {
  const tempInput = $('samplingTemperature');
  if (tempInput) tempInput.value = '';
  const topPInput = $('samplingTopP');
  if (topPInput) topPInput.value = '';
  const topKInput = $('samplingTopK');
  if (topKInput) topKInput.value = '';
  const maxTokensInput = $('samplingMaxTokens');
  if (maxTokensInput) maxTokensInput.value = '';
  const freqPenaltyInput = $('samplingFreqPenalty');
  if (freqPenaltyInput) freqPenaltyInput.value = '';
  const presPenaltyInput = $('samplingPresPenalty');
  if (presPenaltyInput) presPenaltyInput.value = '';

  const params = {
    temperature: null,
    top_p: null,
    top_k: null,
    max_tokens: null,
    frequency_penalty: null,
    presence_penalty: null,
  };
  try {
    await localLlmApi.setSamplingParams(params);
    _samplingParams = params;
    toastSuccess(t('settings.llm_sampling_reset_done'));
  } catch (err) {
    toastError(err);
  }
}

// 暴露为全局函数（内联 onclick 需要）
window.selectLocalModel = selectLocalModel;
window.deleteLocalModel = deleteLocalModel;
window.downloadLocalModel = downloadLocalModel;
window.saveSamplingParams = saveSamplingParams;
window.resetSamplingParams = resetSamplingParams;
window.exportDiagnostics = exportDiagnostics;
window.exportLogs = exportLogs;
// 自定义嵌入模型（REQ-VEC-014）
window.switchToCustomModel = switchToCustomModel;
window.deleteCustomModel = deleteCustomModel;
window.onUploadCustomModel = onUploadCustomModel;

// ============================================================
// 主题切换器（REQ-UI-011 浅色主题切换）
// ============================================================

/**
 * 初始化主题切换器：读取当前主题，高亮活跃按钮，绑定点击事件。
 *
 * 三选一分段控件：暗色 / 浅色 / 跟随系统。
 * 点击后调用 window.setTheme(theme) 应用并持久化。
 */
function initThemeSwitcher() {
  const switcher = $('themeSwitcher');
  if (!switcher) return;

  const currentTheme = document.documentElement.dataset.theme || 'dark';
  const buttons = switcher.querySelectorAll('.theme-btn');

  /** 更新按钮高亮状态 */
  function updateActiveState(activeTheme) {
    buttons.forEach((btn) => {
      const isActive = btn.dataset.themeValue === activeTheme;
      if (isActive) {
        btn.classList.add('bg-surface-0', 'text-accent', 'shadow-sm');
        btn.classList.remove('text-text-secondary');
      } else {
        btn.classList.remove('bg-surface-0', 'text-accent', 'shadow-sm');
        btn.classList.add('text-text-secondary');
      }
    });
  }

  updateActiveState(currentTheme);

  buttons.forEach((btn) => {
    btn.onclick = async () => {
      const theme = btn.dataset.themeValue;
      if (!theme) return;
      updateActiveState(theme);
      // 调用 main.js 暴露的 setTheme 全局函数
      if (typeof window.setTheme === 'function') {
        await window.setTheme(theme);
      }
    };
  });
}

// ============================================================
// 语音设置（REQ-RAG-034 / REQ-RAG-035）
// ============================================================

/**
 * 初始化语音设置：TTS 语速滑块。
 * 语速值持久化到 localStorage（key: voice.rate），默认 1.0。
 */
function initVoiceSettings() {
  const slider = $('voiceRateSlider');
  if (!slider) return;
  const label = $('voiceRateLabel');

  // 从 localStorage 读取已保存的语速
  let savedRate = 1.0;
  try {
    const stored = parseFloat(localStorage.getItem('voice.rate') || '1.0');
    if (!isNaN(stored) && stored >= 0.5 && stored <= 2.0) {
      savedRate = stored;
    }
  } catch (_) {
    // localStorage 不可用时使用默认值
  }
  slider.value = String(savedRate);
  if (label) label.textContent = savedRate.toFixed(1) + 'x';

  slider.oninput = () => {
    const rate = parseFloat(slider.value);
    if (isNaN(rate)) return;
    if (label) label.textContent = rate.toFixed(1) + 'x';
    try {
      localStorage.setItem('voice.rate', String(rate));
    } catch (_) {
      // localStorage 不可用时静默忽略
    }
  };

  // STT 配置初始化
  initSttConfig();
}

/**
 * 初始化 STT（语音转写）配置面板。
 * 从后端读取当前配置（voice.stt_api_key / voice.stt_base_url / voice.stt_model /
 * voice.stt_language），提供编辑界面，保存时调用 set_stt_config IPC。
 */
async function initSttConfig() {
  const section = $('sttConfigSection');
  if (!section) return;

  const apiKeyInput = $('sttApiKeyInput');
  const baseUrlInput = $('sttBaseUrlInput');
  const modelInput = $('sttModelInput');
  const langSelect = $('sttLanguageSelect');
  const saveBtn = $('sttSaveBtn');

  if (!apiKeyInput || !baseUrlInput || !modelInput || !saveBtn) return;

  // 从后端加载当前配置
  try {
    const { invoke } = await import('./ipc.js');
    const config = await invoke('get_stt_config');

    apiKeyInput.placeholder = config.stt_api_key_masked || t('voice.stt_api_key');
    baseUrlInput.value = config.stt_base_url || '';
    modelInput.value = config.stt_model || 'whisper-1';
    if (langSelect) langSelect.value = config.stt_language || 'zh';
  } catch {
    // 后端不可用时静默降级
  }

  saveBtn.onclick = async () => {
    try {
      const { invoke } = await import('./ipc.js');
      const apiKey = apiKeyInput.value.trim();
      const baseUrl = baseUrlInput.value.trim();
      const model = modelInput.value.trim();
      const lang = langSelect ? langSelect.value : undefined;

      await invoke('set_stt_config', {
        sttApiKey: apiKey || null,
        sttBaseUrl: baseUrl || null,
        sttModel: model || null,
        sttLanguage: lang || null,
      });

      // 清空 API Key 输入框（安全）
      apiKeyInput.value = '';
      apiKeyInput.placeholder = '****';

      const { toast } = await import('./toast.js');
      toast(t('voice.stt_saved'), 'success');
    } catch (err) {
      const { toastError } = await import('./toast.js');
      toastError(String(err?.message || err || 'Save failed'));
    }
  };
}

// ============================================================
// PDF 导出设置（REQ-EXP-005）
// ============================================================

/**
 * 初始化 PDF 导出设置：页面大小选择器 + 包含引用来源 toggle。
 * 设置持久化到 localStorage（export.pdf_page_size / export.pdf_include_sources）。
 */
function initPdfExportSettings() {
  // 页面大小选择器
  const pageSizeSelect = $('pdfPageSizeSelect');
  if (pageSizeSelect) {
    pageSizeSelect.value = getPdfPageSize();
    pageSizeSelect.onchange = () => {
      setPdfPageSize(pageSizeSelect.value);
      toastSuccess(t('export_pdf.page_size_saved'));
    };
  }

  // 包含引用来源 toggle
  const includeSourcesToggle = $('pdfIncludeSourcesToggle');
  if (includeSourcesToggle) {
    const knob = includeSourcesToggle.querySelector('span');
    const enabled = getPdfIncludeSources();
    if (enabled) {
      includeSourcesToggle.classList.remove('bg-slate-600');
      includeSourcesToggle.classList.add('bg-accent');
      includeSourcesToggle.setAttribute('aria-checked', 'true');
      knob.classList.add('translate-x-5');
    } else {
      includeSourcesToggle.classList.add('bg-slate-600');
      includeSourcesToggle.classList.remove('bg-accent');
      includeSourcesToggle.setAttribute('aria-checked', 'false');
      knob.classList.remove('translate-x-5');
    }
    includeSourcesToggle.onclick = () => {
      const newEnabled = !getPdfIncludeSources();
      setPdfIncludeSources(newEnabled);
      if (newEnabled) {
        includeSourcesToggle.classList.remove('bg-slate-600');
        includeSourcesToggle.classList.add('bg-accent');
        includeSourcesToggle.setAttribute('aria-checked', 'true');
        knob.classList.add('translate-x-5');
      } else {
        includeSourcesToggle.classList.add('bg-slate-600');
        includeSourcesToggle.classList.remove('bg-accent');
        includeSourcesToggle.setAttribute('aria-checked', 'false');
        knob.classList.remove('translate-x-5');
      }
      toastSuccess(newEnabled ? t('export_pdf.include_sources_on') : t('export_pdf.include_sources_off'));
    };
  }
}

// ============================================================
// 可观测性设置（REQ-OBS-001 / REQ-OBS-002）
// ============================================================

/**
 * 加载可观测性设置：日志级别选择器 + 诊断导出按钮。
 */
async function loadObservabilitySettings() {
  try {
    // 日志级别选择器
    const logLevelSelect = $('logLevelSelect');
    if (logLevelSelect) {
      const level = await invoke('get_log_level');
      logLevelSelect.value = level || 'info';
      logLevelSelect.onchange = async () => {
        const newLevel = logLevelSelect.value;
        try {
          await invoke('set_log_level', { level: newLevel });
          toastSuccess(t('settings.log_level_updated') + ': ' + newLevel.toUpperCase());
        } catch (err) {
          toastError(err);
        }
      };
    }
  } catch (err) {
    // 日志系统 IPC 可能不可用（旧版本兼容），静默忽略
    console.warn('日志设置加载失败:', err);
  }
}

/**
 * 导出诊断信息（REQ-OBS-002）。
 * 收集系统信息、数据库规模、知识库规模、最近日志，导出为 JSON 文件。
 */
async function exportDiagnostics() {
  try {
    toast(t('settings.exporting_diagnostics'), 'info');
    const jsonStr = await invoke('export_diagnostics');

    // 通过系统保存对话框选择保存位置
    const filename = 'echomind-diagnostics-' + new Date().toISOString().slice(0, 10) + '.json';
    await invoke('save_text_file', {
      content: jsonStr,
      filename: filename,
    });
    toastSuccess(t('settings.diagnostics_exported'));
  } catch (err) {
    toastError(err);
  }
}

/**
 * 导出日志文件（REQ-OBS-001）。
 * 读取最近 1000 行日志，导出为文本文件。
 */
async function exportLogs() {
  try {
    toast(t('settings.exporting_logs'), 'info');
    const logs = await invoke('export_logs', { tailLines: 1000 });

    const filename = 'echomind-logs-' + new Date().toISOString().slice(0, 10) + '.log';
    await invoke('save_text_file', {
      content: logs,
      filename: filename,
    });
    toastSuccess(t('settings.logs_exported'));
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 自定义快捷指令模板管理（S56）
// ============================================================

/**
 * 渲染自定义快捷指令模板管理区块。
 *
 * 包含：
 * - 标题 + 新增按钮
 * - 模板列表（名称/图标/描述 + 编辑/删除按钮）
 * - 新增/编辑表单（名称/标签/描述/图标/模板内容）
 *
 * @param {HTMLElement} container - 挂载容器
 */
async function renderPromptTemplateSettings(container) {
  // 标题行
  const header = document.createElement('div');
  header.className = 'flex items-center justify-between mb-3';

  const title = document.createElement('h4');
  title.className = 'side-label text-[11px] uppercase tracking-wider text-text-quaternary';
  title.textContent = t('settings.prompt_templates') || '快捷指令模板';
  header.appendChild(title);

  const addBtn = document.createElement('button');
  addBtn.className = 'text-xs px-2 py-1 rounded-md bg-accent text-ink hover:opacity-90 transition-opacity';
  addBtn.textContent = t('settings.template_add') || '+ 新增';
  addBtn.onclick = () => showTemplateForm(container, null);
  header.appendChild(addBtn);

  container.appendChild(header);

  // 加载模板列表
  try {
    const templates = await invoke('list_prompt_templates');
    if (!templates || templates.length === 0) {
      const empty = document.createElement('p');
      empty.className = 'text-xs text-text-quaternary';
      empty.textContent = t('settings.template_empty') || '暂无自定义模板，点击「新增」创建';
      container.appendChild(empty);
    } else {
      const list = document.createElement('div');
      list.className = 'space-y-2';

      templates.forEach((tmpl) => {
        const item = document.createElement('div');
        item.className = 'flex items-center gap-2 p-2 rounded-lg bg-surface-2 border border-border-subtle';

        const icon = document.createElement('span');
        icon.className = 'text-lg shrink-0';
        icon.textContent = tmpl.icon || '⚡';

        const info = document.createElement('div');
        info.className = 'flex-1 min-w-0';

        const name = document.createElement('div');
        name.className = 'text-sm text-text-primary font-medium truncate';
        name.textContent = `/${tmpl.name}`;

        const desc = document.createElement('div');
        desc.className = 'text-xs text-text-tertiary truncate';
        desc.textContent = tmpl.description || tmpl.label;

        info.appendChild(name);
        info.appendChild(desc);

        const editBtn = document.createElement('button');
        editBtn.className = 'text-xs px-2 py-1 rounded text-text-secondary hover:text-text-primary hover:bg-surface-3 transition-colors shrink-0';
        editBtn.textContent = t('settings.template_edit') || '编辑';
        editBtn.onclick = () => showTemplateForm(container, tmpl);

        const delBtn = document.createElement('button');
        delBtn.className = 'text-xs px-2 py-1 rounded text-red-400 hover:text-red-300 hover:bg-surface-3 transition-colors shrink-0';
        delBtn.textContent = t('settings.template_delete') || '删除';
        delBtn.onclick = async () => {
          const confirmed = await showConfirmDialog({
            body: t('settings.template_delete_confirm') || '确定删除此模板？',
            title: t('settings.template_delete_title') || '删除模板',
          });
          if (!confirmed) return;
          try {
            await invoke('delete_prompt_template', { templateId: tmpl.id });
            toastSuccess(t('settings.template_deleted') || '模板已删除');
            resetCustomTemplates();
            await loadCustomTemplates();
            await refreshTemplateList(container);
          } catch (err) {
            toastError(err);
          }
        };

        item.appendChild(icon);
        item.appendChild(info);
        item.appendChild(editBtn);
        item.appendChild(delBtn);
        list.appendChild(item);
      });

      container.appendChild(list);
    }
  } catch (err) {
    const errorP = document.createElement('p');
    errorP.className = 'text-xs text-red-400';
    errorP.textContent = t('settings.template_load_error') || '加载模板失败';
    container.appendChild(errorP);
  }
}

/**
 * 刷新模板列表（不重新渲染整个区块）。
 * @param {HTMLElement} container - 挂载容器
 */
async function refreshTemplateList(container) {
  // 清除列表内容但保留表单（如果有）
  const form = container.querySelector('.template-form');
  container.innerHTML = '';
  await renderPromptTemplateSettings(container);
  if (form) container.appendChild(form);
}

/**
 * 显示模板新增/编辑表单。
 *
 * @param {HTMLElement} container - 挂载容器
 * @param {Object|null} existing - 已有模板（编辑模式）或 null（新增模式）
 */
function showTemplateForm(container, existing) {
  // 移除已有表单
  const oldForm = container.querySelector('.template-form');
  if (oldForm) oldForm.remove();

  const form = document.createElement('div');
  form.className = 'template-form mt-3 p-3 rounded-lg bg-surface-2 border border-border-default space-y-2';

  // 名称输入
  const nameRow = document.createElement('div');
  nameRow.className = 'flex flex-col gap-1';
  const nameLabel = document.createElement('label');
  nameLabel.className = 'text-xs text-text-tertiary';
  nameLabel.textContent = t('settings.template_name') || '指令名称（小写字母/数字/下划线）';
  const nameInput = document.createElement('input');
  nameInput.type = 'text';
  nameInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent';
  nameInput.placeholder = 'my_command';
  nameInput.value = existing?.name || '';
  if (existing) nameInput.disabled = true; // 编辑模式不允许改名
  nameRow.appendChild(nameLabel);
  nameRow.appendChild(nameInput);
  form.appendChild(nameRow);

  // 标签输入
  const labelRow = document.createElement('div');
  labelRow.className = 'flex flex-col gap-1';
  const labelLabel = document.createElement('label');
  labelLabel.className = 'text-xs text-text-tertiary';
  labelLabel.textContent = t('settings.template_label') || '显示标签';
  const labelInput = document.createElement('input');
  labelInput.type = 'text';
  labelInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent';
  labelInput.placeholder = t('settings.template_label_placeholder') || '我的指令';
  labelInput.value = existing?.label || '';
  labelRow.appendChild(labelLabel);
  labelRow.appendChild(labelInput);
  form.appendChild(labelRow);

  // 描述输入
  const descRow = document.createElement('div');
  descRow.className = 'flex flex-col gap-1';
  const descLabel = document.createElement('label');
  descLabel.className = 'text-xs text-text-tertiary';
  descLabel.textContent = t('settings.template_description') || '描述';
  const descInput = document.createElement('input');
  descInput.type = 'text';
  descInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent';
  descInput.placeholder = t('settings.template_description_placeholder') || '指令用途说明';
  descInput.value = existing?.description || '';
  descRow.appendChild(descLabel);
  descRow.appendChild(descInput);
  form.appendChild(descRow);

  // 图标输入
  const iconRow = document.createElement('div');
  iconRow.className = 'flex flex-col gap-1';
  const iconLabel = document.createElement('label');
  iconLabel.className = 'text-xs text-text-tertiary';
  iconLabel.textContent = t('settings.template_icon') || '图标（emoji）';
  const iconInput = document.createElement('input');
  iconInput.type = 'text';
  iconInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent w-20';
  iconInput.placeholder = '⚡';
  iconInput.value = existing?.icon || '⚡';
  iconRow.appendChild(iconLabel);
  iconRow.appendChild(iconInput);
  form.appendChild(iconRow);

  // 模板内容输入
  const tmplRow = document.createElement('div');
  tmplRow.className = 'flex flex-col gap-1';
  const tmplLabel = document.createElement('label');
  tmplLabel.className = 'text-xs text-text-tertiary';
  tmplLabel.textContent = t('settings.template_content') || 'Prompt 模板（使用 {query} 作为占位符）';
  const tmplTextarea = document.createElement('textarea');
  tmplTextarea.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-2 text-sm text-text-primary outline-none focus:border-accent resize-y min-h-[80px]';
  tmplTextarea.placeholder = '请执行以下操作：{query}';
  tmplTextarea.value = existing?.prompt_template || '';
  tmplRow.appendChild(tmplLabel);
  tmplRow.appendChild(tmplTextarea);
  form.appendChild(tmplRow);

  // 按钮行
  const btnRow = document.createElement('div');
  btnRow.className = 'flex gap-2 justify-end';

  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'text-xs px-3 py-1.5 rounded-lg border border-border-default text-text-secondary hover:bg-surface-3 transition-colors';
  cancelBtn.textContent = t('settings.template_cancel') || '取消';
  cancelBtn.onclick = () => form.remove();

  const saveBtn = document.createElement('button');
  saveBtn.className = 'text-xs px-3 py-1.5 rounded-lg bg-accent text-ink hover:opacity-90 transition-opacity';
  saveBtn.textContent = existing
    ? (t('settings.template_update') || '更新')
    : (t('settings.template_create') || '创建');
  saveBtn.onclick = async () => {
    const name = nameInput.value.trim();
    const label = labelInput.value.trim();
    const description = descInput.value.trim();
    const icon = iconInput.value.trim() || '⚡';
    const promptTemplate = tmplTextarea.value.trim();

    if (!name || !label || !promptTemplate) {
      toast(t('settings.template_required') || '名称、标签和模板内容不能为空', 'error');
      return;
    }

    if (!promptTemplate.includes('{query}')) {
      toast(t('settings.template_query_required') || '模板内容必须包含 {query} 占位符', 'error');
      return;
    }

    try {
      await invoke('save_prompt_template', {
        name, label, description, icon, promptTemplate,
      });
      toastSuccess(existing
        ? (t('settings.template_updated') || '模板已更新')
        : (t('settings.template_created') || '模板已创建'));
      resetCustomTemplates();
      await loadCustomTemplates();
      form.remove();
      await refreshTemplateList(container);
    } catch (err) {
      toastError(err);
    }
  };

  btnRow.appendChild(cancelBtn);
  btnRow.appendChild(saveBtn);
  form.appendChild(btnRow);

  container.appendChild(form);
  nameInput.focus();
}
