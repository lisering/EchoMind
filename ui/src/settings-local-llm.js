/**
 * EchoMind 设置面板 — 本地 LLM 模型管理子模块（REQ-LLM-003/004）。
 *
 * 从 settings.js 拆分而来，职责：
 * 1. 推理模式切换（remote / local）
 * 2. 已下载模型列表渲染
 * 3. 推荐模型列表渲染
 * 4. 模型下载（内联进度条 + 下载管理器面板集成）
 * 5. 模型删除（确认对话框）
 * 6. 模型选择 + 自动切换到本地模式
 * 7. model_load_progress 事件监听
 *
 * 下载管理（暂停/恢复/取消/崩溃恢复）委托给 download-manager.js。
 */

import { get } from './state.js';
import { $ } from './utils.js';
import { listen, localLlmApi } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { t } from './i18n.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { startDownload, pauseDownload, cancelDownload, openDownloadManager } from './download-manager.js';

// ============================================================
// 模块级状态
// ============================================================

/** 当前选中的本地模型文件名（从 settings.local_model 恢复） */
let _selectedLocalModel = '';

/** 当前内联进度条追踪的文件名 */
let _inlineProgressFilename = '';

/** 下载进度事件取消监听函数（内联进度条用） */
let _unlistenInlineProgress = null;

/** model_load_progress 事件监听是否已注册（防止重复注册） */
let _modelLoadListenerRegistered = false;

// ============================================================
// 格式化工具
// ============================================================

/**
 * 格式化模型文件大小（支持 GB 单位，GGUF 模型通常 2~5GB）。
 * @param {number} bytes - 字节数
 * @returns {string} 人类可读的大小字符串
 */
function formatModelSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB';
  return (bytes / 1024 / 1024 / 1024).toFixed(1) + ' GB';
}

// ============================================================
// 推理模式切换
// ============================================================

/**
 * 从 get_settings 返回的 llm_mode / local_model 字段恢复 UI 状态。
 * @param {Object} settings - get_settings 命令返回的设置对象
 */
export async function loadLlmModeSetting(settings) {
  const mode = settings.llm_mode || 'remote';
  _selectedLocalModel = settings.local_model || '';
  const radioRemote = $('llmModeRemote');
  const radioLocal = $('llmModeLocal');
  if (radioRemote && radioLocal) {
    if (mode === 'local') {
      radioLocal.checked = true;
    } else {
      radioRemote.checked = true;
    }
  }
}

/**
 * 切换 LLM 推理模式（REQ-LLM-003 AC-4）。
 * Free 用户切换到 local 时被拦截并提示升级 Pro。
 * @param {string} mode - 'remote' 或 'local'
 */
export async function onLlmModeChange(mode) {
  try {
    if (mode === 'local' && !get('isPro')) {
      toast(t('settings.llm_mode_pro_required'), 'info');
      $('llmModeRemote').checked = true;
      return;
    }
    await localLlmApi.setMode(mode);
    toast(
      mode === 'local'
        ? t('settings.llm_mode_switched_local')
        : t('settings.llm_mode_switched_remote'),
      'success'
    );
  } catch (err) {
    toastError(err);
    try {
      const actual = await localLlmApi.getMode();
      if (actual === 'local') {
        $('llmModeLocal').checked = true;
      } else {
        $('llmModeRemote').checked = true;
      }
    } catch (_) {
      $('llmModeRemote').checked = true;
    }
  }
}

// ============================================================
// 模型列表渲染
// ============================================================

/**
 * 加载并渲染已下载的本地模型列表（REQ-LLM-004 AC-1）。
 */
export async function loadLocalModels() {
  const container = $('localModelsList');
  if (!container) return;
  try {
    const models = await localLlmApi.listModels();
    if (models.length === 0) {
      container.innerHTML = `<p class="text-xs text-text-quaternary py-2">${t('settings.llm_local_no_models')}</p>`;
      return;
    }
    container.innerHTML = models
      .map((m) => {
        const isSelected = m.filename === _selectedLocalModel;
        const displayName = `${m.architecture} ${m.param_size}`;
        const escapedFilename = m.filename.replace(/'/g, "\\'");
        const escapedName = displayName.replace(/'/g, "\\'");
        return `
        <div class="flex items-center justify-between gap-2 py-2 px-2 rounded-lg ${isSelected ? 'bg-accent/10' : 'hover:bg-white/5'}">
          <div class="flex-1 min-w-0">
            <div class="text-sm text-text-primary truncate">${displayName}</div>
            <div class="text-[11px] text-text-quaternary">${m.quantization} · ${formatModelSize(m.size_bytes)}</div>
          </div>
          <div class="flex items-center gap-1 shrink-0">
            ${isSelected
              ? `<span class="px-2 py-1 text-[11px] rounded-lg bg-accent/15 text-accent border border-accent/30">${t('settings.llm_local_using')}</span>`
              : `<button class="px-2 py-1 text-[11px] rounded-lg bg-accent/15 text-accent border border-accent/30 hover:bg-accent/25 transition-colors"
                  onclick="selectLocalModel('${escapedFilename}', '${escapedName}')">${t('settings.llm_local_use')}</button>`
            }
            <button class="px-2 py-1 text-[11px] rounded-lg bg-red-500/15 text-red-300 border border-red-400/40 hover:bg-red-500/25 transition-colors"
              onclick="deleteLocalModel('${escapedFilename}', '${escapedName}', ${m.size_bytes})">${t('settings.llm_local_delete')}</button>
          </div>
        </div>`;
      })
      .join('');
  } catch (err) {
    container.innerHTML = `<p class="text-xs text-red-400 py-2">${t('settings.llm_local_load_failed')}</p>`;
  }
}

/**
 * 加载并渲染推荐模型列表（REQ-LLM-004 AC-6）。
 */
export async function loadRecommendedModels() {
  const container = $('recommendedModelsList');
  if (!container) return;
  try {
    const models = await localLlmApi.getRecommended();
    if (models.length === 0) return;
    container.innerHTML = models
      .map((m) => {
        const filename = m.url.split('/').pop() || `${m.name}.gguf`;
        const escapedUrl = m.url.replace(/'/g, "\\'");
        const escapedFilename = filename.replace(/'/g, "\\'");
        const escapedName = m.name.replace(/'/g, "\\'");
        return `
        <div class="flex items-center justify-between gap-2 py-2 px-2 rounded-lg hover:bg-white/5">
          <div class="flex-1 min-w-0">
            <div class="text-sm text-text-primary truncate">${m.name}</div>
            <div class="text-[11px] text-text-quaternary">${m.quantization} · ${m.size_gb.toFixed(1)} GB · ${m.description}</div>
          </div>
          <button class="shrink-0 px-2 py-1 text-[11px] rounded-lg bg-accent/15 text-accent border border-accent/30 hover:bg-accent/25 transition-colors"
            onclick="downloadLocalModel('${escapedUrl}', '${escapedFilename}', '${escapedName}')">${t('settings.llm_local_download')}</button>
        </div>`;
      })
      .join('');
  } catch (err) {
    // 推荐模型加载失败不阻塞设置面板
  }
}

// ============================================================
// 模型下载（委托给 download-manager.js + 内联进度条同步）
// ============================================================

/**
 * 下载本地 GGUF 模型文件（REQ-LLM-004 AC-2 + v2 断点续传）。
 *
 * 委托给 download-manager.js 的 startDownload，同时在本地面板
 * 显示内联进度条。下载管理器面板自动打开，支持暂停/恢复/取消。
 *
 * @param {string} url - 下载 URL（HuggingFace GGUF）
 * @param {string} filename - 目标文件名
 * @param {string} displayName - 用于 UI 显示的模型名称
 */
export async function downloadLocalModel(url, filename, displayName) {
  const progressEl = $('llmDownloadProgress');
  if (!progressEl) return;

  // 显示内联进度条
  progressEl.classList.remove('hidden');
  $('llmDownloadFilename').textContent = displayName || filename;
  _inlineProgressFilename = filename;
  const bar = $('llmDownloadBar');
  if (bar) {
    bar.style.width = '0%';
    bar.classList.remove('progress-complete', 'progress-error', 'progress-indeterminate');
  }
  $('llmDownloadPct').textContent = '0%';
  $('llmDownloadSize').textContent = '— / —';

  // 清理旧的内联监听器
  if (_unlistenInlineProgress) {
    _unlistenInlineProgress();
    _unlistenInlineProgress = null;
  }

  // 注册内联进度条事件监听（过滤当前文件名）
  try {
    _unlistenInlineProgress = await listen('model_download_progress', (event) => {
      const p = event.payload;
      if (!p || p.filename !== _inlineProgressFilename) return;

      const bar = $('llmDownloadBar');
      const pctEl = $('llmDownloadPct');
      const sizeEl = $('llmDownloadSize');

      let pct;
      if (p.total > 0) {
        pct = Math.min(100, Math.round((p.downloaded / p.total) * 100));
      } else {
        const dl = p.downloaded || 0;
        pct = Math.min(95, Math.round(30 + 15 * Math.log10(1 + dl / 65536)));
      }

      if (bar) {
        bar.style.width = pct + '%';
        const isUnknown = !p.total || p.total <= 0;
        bar.classList.toggle('progress-indeterminate', isUnknown && pct < 95);
      }
      if (pctEl) pctEl.textContent = pct + '%';
      if (sizeEl) {
        if (p.total > 0) {
          sizeEl.textContent = formatModelSize(p.downloaded) + ' / ' + formatModelSize(p.total);
        } else {
          sizeEl.textContent = formatModelSize(p.downloaded);
        }
      }
    });
  } catch (_) {
    // 测试环境或事件系统不可用时静默降级
  }

  // 委托给下载管理器启动下载（处理暂停/恢复/取消/面板状态）
  try {
    await startDownload(url, filename, displayName);
    if (bar) {
      bar.style.width = '100%';
      bar.classList.remove('progress-indeterminate');
      bar.classList.add('progress-complete');
    }
    toastSuccess(t('settings.llm_local_download_complete', { name: displayName || filename }));
    await loadLocalModels();
  } catch (err) {
    if (bar) {
      bar.classList.remove('progress-indeterminate');
      bar.classList.add('progress-error');
    }
    toastError(err);
  } finally {
    progressEl.classList.add('hidden');
    _inlineProgressFilename = '';
    if (_unlistenInlineProgress) {
      _unlistenInlineProgress();
      _unlistenInlineProgress = null;
    }
  }
}

// ============================================================
// 模型删除
// ============================================================

/**
 * 删除本地模型文件（REQ-LLM-004 AC-3）。
 * @param {string} filename - 模型文件名
 * @param {string} name - 显示名称
 * @param {number} sizeBytes - 文件大小（字节）
 */
export async function deleteLocalModel(filename, name, sizeBytes) {
  const sizeStr = formatModelSize(sizeBytes);
  const ok = await showConfirmDialog({
    title: t('settings.llm_local_delete_confirm', { name, size: sizeStr }),
    confirmText: t('common.confirm'),
    danger: true,
  });
  if (!ok) return;

  try {
    await localLlmApi.deleteModel(filename);
    toast(t('settings.llm_local_deleted', { name }), 'info');
    if (_selectedLocalModel === filename) {
      _selectedLocalModel = '';
    }
    await loadLocalModels();
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 模型选择
// ============================================================

/**
 * 选择本地模型并切换到本地推理模式（REQ-LLM-003 AC-1）。
 * @param {string} filename - 模型文件名
 * @param {string} name - 显示名称
 */
export async function selectLocalModel(filename, name) {
  try {
    await localLlmApi.setLocalModel(filename);
    _selectedLocalModel = filename;
    await localLlmApi.setMode('local');
    const radioLocal = $('llmModeLocal');
    if (radioLocal) radioLocal.checked = true;
    toast(t('settings.llm_local_select_success', { name }), 'success');
    await loadLocalModels();
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 模型加载状态监听
// ============================================================

/**
 * 处理 model_load_progress 事件（REQ-LLM-003 AC-4）。
 * @param {Object} payload - ModelLoadStatusPayload { model, status, error? }
 */
function handleModelLoadStatus(payload) {
  if (!payload) return;
  switch (payload.status) {
    case 'loading':
      toast(t('settings.llm_local_model_loading') + ' (' + payload.model + ')', 'info');
      break;
    case 'ready':
      toastSuccess(t('settings.llm_local_model_ready') + ' (' + payload.model + ')');
      break;
    case 'error':
      toastError(new Error(t('settings.llm_local_model_load_error', { error: payload.error || '' })));
      break;
    default:
      break;
  }
}

/**
 * 注册 model_load_progress 事件监听器（全局，只需注册一次）。
 */
export async function registerModelLoadListener() {
  if (_modelLoadListenerRegistered) return;
  _modelLoadListenerRegistered = true;
  try {
    await listen('model_load_progress', (event) => {
      handleModelLoadStatus(event.payload);
    });
  } catch (_) {
    _modelLoadListenerRegistered = false;
  }
}

// ============================================================
// 下载控制（暂停 / 取消 — 委托给 download-manager.js）
// ============================================================

/**
 * 暂停当前内联进度条对应的下载。
 */
export async function onPauseDownload() {
  if (!_inlineProgressFilename) return;
  await pauseDownload(_inlineProgressFilename);
}

/**
 * 取消当前内联进度条对应的下载。
 */
export async function onCancelDownload() {
  if (!_inlineProgressFilename) return;
  await cancelDownload(_inlineProgressFilename);
}
