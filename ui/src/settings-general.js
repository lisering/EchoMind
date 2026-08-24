/**
 * EchoMind 设置面板 — 通用设置模块（从 settings.js 拆分）。
 *
 * 职责：
 * 1. VLM Toggle 交互（含隐私确认弹窗）
 * 2. Cross-Encoder Rerank Toggle（REQ-RAG-020）
 * 3. HyDE 查询改写 Toggle（REQ-RAG-021）
 * 4. 模型缓存信息 / 下载 / 清理
 * 5. 主题切换器（REQ-UI-011 浅色主题切换）
 * 6. 语音设置（REQ-RAG-034/035 TTS 语速 + STT 配置）
 * 7. PDF 导出设置（REQ-EXP-005）
 * 8. 可观测性设置（REQ-OBS-001/002 日志级别 + 诊断导出）
 * 9. 全量数据备份与恢复（REQ-EXP-002/003）
 */

import { setState, get } from './state.js';
import { $, formatBytes } from './utils.js';
import { invoke } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { t } from './i18n.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { getPdfPageSize, setPdfPageSize, getPdfIncludeSources, setPdfIncludeSources } from './export.js';

// ============================================================
// 模型缓存管理
// ============================================================

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

export async function initEmbedder() {
  try {
    toast(t('settings.downloading_model'), 'info');
    await invoke('init_embedder');
  } catch (err) {
    toastError(err);
  }
}

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

export function onVlmToggle() {
  if (get('vlmEnabled')) {
    setState({ vlmEnabled: false });
    updateVlmToggle();
    saveVlmSetting(false);
  } else {
    $('vlmConfirm').classList.remove('hidden');
  }
}

export async function confirmVlmEnable() {
  $('vlmConfirm').classList.add('hidden');
  setState({ vlmEnabled: true });
  updateVlmToggle();
  await saveVlmSetting(true);
}

export function cancelVlmEnable() {
  $('vlmConfirm').classList.add('hidden');
}

async function saveVlmSetting(enabled) {
  try {
    await invoke('update_setting', { key: 'mm.vlm_enabled', value: String(enabled) });
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

export function onRerankToggle() {
  const newEnabled = !get('rerankEnabled');
  setState({ rerankEnabled: newEnabled });
  updateRerankToggle();
  saveRerankSetting(newEnabled);
}

async function saveRerankSetting(enabled) {
  try {
    await invoke('update_setting', { key: 'rag.rerank_enabled', value: String(enabled) });
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

export function onHydeToggle() {
  const newEnabled = !get('hydeEnabled');
  setState({ hydeEnabled: newEnabled });
  updateHydeToggle();
  saveHydeSetting(newEnabled);
}

async function saveHydeSetting(enabled) {
  try {
    await invoke('update_setting', { key: 'rag.hyde_enabled', value: String(enabled) });
    toast(enabled ? t('settings.hyde_enabled') : t('settings.hyde_disabled'), 'success');
  } catch (err) {
    toastError(err);
    setState({ hydeEnabled: !enabled });
    updateHydeToggle();
  }
}

// ============================================================
// 主题切换器（REQ-UI-011 浅色主题切换）
// ============================================================

export function initThemeSwitcher() {
  const switcher = $('themeSwitcher');
  if (!switcher) return;

  const currentTheme = document.documentElement.dataset.theme || 'dark';
  const buttons = switcher.querySelectorAll('.theme-btn');

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
      if (typeof window.setTheme === 'function') {
        await window.setTheme(theme);
      }
    };
  });
}

// ============================================================
// 语音设置（REQ-RAG-034 / REQ-RAG-035）
// ============================================================

export function initVoiceSettings() {
  const slider = $('voiceRateSlider');
  if (!slider) return;
  const label = $('voiceRateLabel');

  let savedRate = 1.0;
  try {
    const stored = parseFloat(localStorage.getItem('voice.rate') || '1.0');
    if (!isNaN(stored) && stored >= 0.5 && stored <= 2.0) {
      savedRate = stored;
    }
  } catch (_) {}

  slider.value = String(savedRate);
  if (label) label.textContent = savedRate.toFixed(1) + 'x';

  slider.oninput = () => {
    const rate = parseFloat(slider.value);
    if (isNaN(rate)) return;
    if (label) label.textContent = rate.toFixed(1) + 'x';
    try {
      localStorage.setItem('voice.rate', String(rate));
    } catch (_) {}
  };

  initSttConfig();
}

async function initSttConfig() {
  const section = $('sttConfigSection');
  if (!section) return;

  const apiKeyInput = $('sttApiKeyInput');
  const baseUrlInput = $('sttBaseUrlInput');
  const modelInput = $('sttModelInput');
  const langSelect = $('sttLanguageSelect');
  const saveBtn = $('sttSaveBtn');

  if (!apiKeyInput || !baseUrlInput || !modelInput || !saveBtn) return;

  try {
    const { invoke } = await import('./ipc.js');
    const config = await invoke('get_stt_config');

    apiKeyInput.placeholder = config.stt_api_key_masked || t('voice.stt_api_key');
    baseUrlInput.value = config.stt_base_url || '';
    modelInput.value = config.stt_model || 'whisper-1';
    if (langSelect) langSelect.value = config.stt_language || 'zh';
  } catch {}

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

      apiKeyInput.value = '';
      apiKeyInput.placeholder = '****';

      const { toast } = await import('./toast.js');
      toast(t('voice.stt_saved'), 'success');
    } catch (err) {
      const { toastError } = await import('./toast.js');
      toastError(String(err?.message || err || t('settings.save_failed')));
    }
  };
}

// ============================================================
// PDF 导出设置（REQ-EXP-005）
// ============================================================

export function initPdfExportSettings() {
  const pageSizeSelect = $('pdfPageSizeSelect');
  if (pageSizeSelect) {
    pageSizeSelect.value = getPdfPageSize();
    pageSizeSelect.onchange = () => {
      setPdfPageSize(pageSizeSelect.value);
      toastSuccess(t('export_pdf.page_size_saved'));
    };
  }

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

export async function loadObservabilitySettings() {
  try {
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
    console.warn('日志设置加载失败:', err);
  }
}

export async function exportDiagnostics() {
  try {
    toast(t('settings.exporting_diagnostics'), 'info');
    const jsonStr = await invoke('export_diagnostics');
    const filename = 'echomind-diagnostics-' + new Date().toISOString().slice(0, 10) + '.json';
    await invoke('save_text_file', { content: jsonStr, filename: filename });
    toastSuccess(t('settings.diagnostics_exported'));
  } catch (err) {
    toastError(err);
  }
}

export async function exportLogs() {
  try {
    toast(t('settings.exporting_logs'), 'info');
    const logs = await invoke('export_logs', { tailLines: 1000 });
    const filename = 'echomind-logs-' + new Date().toISOString().slice(0, 10) + '.log';
    await invoke('save_text_file', { content: logs, filename: filename });
    toastSuccess(t('settings.logs_exported'));
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 全量数据备份与恢复（REQ-EXP-002/003）
// ============================================================

export async function exportBackup() {
  try {
    toast(t('settings.exporting_backup'), 'info');
    const jsonStr = await invoke('export_backup');
    const filename = 'echomind-backup-' + new Date().toISOString().slice(0, 10) + '.json';
    await invoke('save_text_file', { content: jsonStr, filename });
    toastSuccess(t('settings.backup_exported'));
  } catch (err) {
    toastError(err);
  }
}

export async function importBackup() {
  const confirmed = await showConfirmDialog({
    title: t('settings.import_backup_confirm_title'),
    body: t('settings.import_backup_confirm_msg'),
  });
  if (!confirmed) return;

  try {
    const { openDialog } = await import('./ipc.js');
    const selected = await openDialog({
      multiple: false,
      filters: [{ name: 'EchoMind Backup', extensions: ['json'] }],
    });
    if (!selected) return;

    toast(t('settings.importing_backup'), 'info');

    const filePath = typeof selected === 'string' ? selected : (Array.isArray(selected) ? selected[0] : String(selected));
    const content = await invoke('read_text_file', { path: filePath });

    const result = await invoke('import_backup', { content });
    const stats = JSON.parse(result);

    toastSuccess(t('settings.backup_imported', {
      conversations: stats.conversations,
      messages: stats.messages,
      documents: stats.documents,
    }));

    setTimeout(() => {
      toast(t('settings.restart_after_restore'), 'info');
    }, 2000);
  } catch (err) {
    toastError(err);
  }
}
