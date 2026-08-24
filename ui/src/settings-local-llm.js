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
import { $, formatBytes } from './utils.js';
import { listen, localLlmApi, kvCacheApi } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { t } from './i18n.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { startDownload, pauseDownload, cancelDownload, openDownloadManager } from './download-manager.js';
import { icon } from './utils.js';

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
// 格式化工具（REQ-I18N-003：统一使用 utils.js 的 formatBytes）
// ============================================================

/**
 * 格式化模型文件大小。
 * 统一使用 utils.js 的 formatBytes（支持 B/KB/MB/GB/TB）。
 * @param {number} bytes - 字节数
 * @returns {string} 人类可读的大小字符串
 */
function formatModelSize(bytes) {
  return formatBytes(bytes);
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

// ============================================================
// KV Cache 管理（Pro 功能，跨会话 KV Cache 复用）
// ============================================================

/**
 * 格式化文件大小为人类可读字符串。
 * @param {number} bytes - 字节数
 * @returns {string} 格式化后的大小
 */
function formatCacheSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB';
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB';
}

/**
 * 渲染 KV Cache 管理面板到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderKvCacheSettings(container) {
  if (!container) return;

  let status = null;
  try {
    status = await kvCacheApi.getStatus();
  } catch (_) {
    // 静默降级
  }

  const enabled = status?.enabled || false;
  const fileCount = status?.file_count || 0;
  const totalSize = status?.total_size_bytes || 0;
  const cacheDir = status?.cache_dir || '-';

  container.innerHTML = `
    <div class="border-t border-border-default pt-5 mt-5" id="kvCacheSection">
      <h3 class="text-sm font-semibold m-0 mb-4 flex items-center gap-1">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" style="display:inline; vertical-align:middle; margin-right:6px;">
          <path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/>
          <polyline points="17 21 17 13 7 13 7 21"/>
          <polyline points="7 3 7 8 17 8"/>
        </svg>
        <span data-i18n="kv_cache.title">KV Cache 管理</span>
      </h3>

      <!-- 启用开关 -->
      <div class="flex items-center justify-between py-2">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="kv_cache.enable">启用 KV Cache</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="kv_cache.enable_desc">跨会话复用 KV Cache，加速多轮对话</span>
        </div>
        <button id="kvCacheToggle" class="shrink-0 ml-4 relative w-11 h-6 rounded-full transition-colors ${enabled ? 'bg-accent' : 'bg-slate-600'}" role="switch" aria-checked="${enabled}">
          <span class="absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform ${enabled ? 'translate-x-5' : ''}"></span>
        </button>
      </div>

      <!-- 状态信息 -->
      <div class="bg-surface-2 rounded-lg p-3 mt-2 space-y-1.5">
        <div class="flex items-center justify-between">
          <span class="text-xs text-text-quaternary" data-i18n="kv_cache.file_count">缓存文件数</span>
          <span class="text-xs text-text-primary tabular-nums">${fileCount}</span>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-xs text-text-quaternary" data-i18n="kv_cache.total_size">总占用</span>
          <span class="text-xs text-text-primary tabular-nums">${formatCacheSize(totalSize)}</span>
        </div>
        <div class="flex items-center justify-between">
          <span class="text-xs text-text-quaternary" data-i18n="kv_cache.cache_dir">缓存目录</span>
          <span class="text-xs text-text-tertiary truncate max-w-[200px]" title="${cacheDir}">${cacheDir}</span>
        </div>
      </div>

      <!-- 操作按钮 -->
      <div class="flex gap-2 mt-3">
        <button id="btnSaveKvCache" class="flex-1 text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors" data-i18n="kv_cache.save">保存当前</button>
        <button id="btnLoadKvCache" class="flex-1 text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors" data-i18n="kv_cache.load">加载缓存</button>
        <button id="btnClearKvCache" class="flex-1 text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-red-400 hover:border-red-400/40 transition-colors" data-i18n="kv_cache.clear">清除缓存</button>
      </div>
    </div>
  `;

  initKvCacheHandlers();
}

/**
 * 初始化 KV Cache 事件处理器。
 */
function initKvCacheHandlers() {
  // 启用/禁用开关
  const toggle = $('kvCacheToggle');
  if (toggle) {
    toggle.onclick = async () => {
      const isCurrentlyEnabled = toggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !isCurrentlyEnabled;
      try {
        await kvCacheApi.setEnabled(newEnabled);
        toggle.setAttribute('aria-checked', String(newEnabled));
        toggle.classList.toggle('bg-accent', newEnabled);
        toggle.classList.toggle('bg-slate-600', !newEnabled);
        toggle.querySelector('span').classList.toggle('translate-x-5', newEnabled);
        toastSuccess(newEnabled ? t('kv_cache.enabled', 'KV Cache 已启用') : t('kv_cache.disabled', 'KV Cache 已禁用'));
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 保存当前会话 KV Cache
  const btnSave = $('btnSaveKvCache');
  if (btnSave) {
    btnSave.onclick = async () => {
      const convId = get('currentConversationId');
      if (!convId) {
        toast(t('kv_cache.no_conversation', '请先选择会话'), 'error');
        return;
      }
      try {
        await kvCacheApi.save(convId);
        toastSuccess(t('kv_cache.saved', 'KV Cache 已保存'));
        await renderKvCacheSettings($('kvCacheContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 加载 KV Cache
  const btnLoad = $('btnLoadKvCache');
  if (btnLoad) {
    btnLoad.onclick = async () => {
      const convId = get('currentConversationId');
      if (!convId) {
        toast(t('kv_cache.no_conversation', '请先选择会话'), 'error');
        return;
      }
      try {
        const success = await kvCacheApi.load(convId);
        if (success) {
          toastSuccess(t('kv_cache.loaded', 'KV Cache 已加载'));
        } else {
          toast(t('kv_cache.not_found', '未找到缓存文件'), 'info');
        }
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 清除 KV Cache
  const btnClear = $('btnClearKvCache');
  if (btnClear) {
    btnClear.onclick = async () => {
      const convId = get('currentConversationId');
      if (!convId) {
        toast(t('kv_cache.no_conversation', '请先选择会话'), 'error');
        return;
      }
      const confirmed = await showConfirmDialog({
        title: t('kv_cache.clear_confirm', '确认清除当前会话的 KV Cache？'),
        danger: true,
      });
      if (!confirmed) return;
      try {
        await kvCacheApi.clear(convId);
        toastSuccess(t('kv_cache.cleared', 'KV Cache 已清除'));
        await renderKvCacheSettings($('kvCacheContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  }
}

// ============================================================
// 硬件加速状态显示（Pro 功能）
// ============================================================

/**
 * 渲染硬件加速设备类型状态到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderDeviceKindSettings(container) {
  if (!container) return;

  let deviceKind = 'CPU';
  try {
    deviceKind = await localLlmApi.getDeviceKind();
  } catch (_) {
    // 静默降级，默认 CPU
  }

  const deviceIcon = deviceKind === 'Metal' ? icon('check', 'sm') : deviceKind === 'CUDA' ? icon('check', 'sm') : icon('keyboard', 'sm');
  const deviceLabel = deviceKind === 'Metal' ? 'Apple Metal' : deviceKind === 'CUDA' ? 'NVIDIA CUDA' : 'CPU';
  const deviceColor = deviceKind === 'CPU' ? 'text-text-tertiary' : 'text-success';

  container.innerHTML = `
    <div class="flex items-center justify-between py-2 border-t border-border-default" id="deviceKindSection">
      <div class="flex flex-col gap-0.5 flex-1">
        <span data-i18n="settings.device_kind">硬件加速</span>
        <span class="text-xs text-text-tertiary leading-tight" data-i18n="settings.device_kind_desc">本地推理使用的计算设备</span>
      </div>
      <div class="shrink-0 ml-4 flex items-center gap-1.5">
        <span class="text-base">${deviceIcon}</span>
        <span class="text-sm font-medium ${deviceColor}">${deviceLabel}</span>
      </div>
    </div>
  `;
}

// ============================================================
// 嵌入模型切换（Pro 功能）
// ============================================================

/**
 * 渲染嵌入模型切换面板到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderEmbedderModelSettings(container) {
  if (!container) return;

  let currentModel = 'all-MiniLM-L6-v2';
  try {
    const settings = await import('./ipc.js').then(m => m.llmApi.getSettings());
    currentModel = settings.embedding_model || 'all-MiniLM-L6-v2';
  } catch (_) {
    // 静默降级
  }

  container.innerHTML = `
    <div class="flex items-center justify-between py-2 border-t border-border-default" id="embedderModelSection">
      <div class="flex flex-col gap-0.5 flex-1">
        <span data-i18n="settings.embedder_model">嵌入模型</span>
        <span class="text-xs text-text-tertiary leading-tight" data-i18n="settings.embedder_model_desc">运行时切换 ONNX 嵌入模型（Pro 功能）</span>
      </div>
      <div class="shrink-0 ml-4 flex items-center gap-2">
        <input type="text" id="embedderModelInput" class="px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
               value="${currentModel}" style="width: 180px;" placeholder="all-MiniLM-L6-v2"/>
        <button id="btnSetEmbedderModel" class="text-xs px-3 py-1 rounded-md bg-primary text-surface-0 cursor-pointer hover:bg-primary-hover transition-colors" data-i18n="settings.apply">应用</button>
      </div>
    </div>
  `;

  const btnApply = $('btnSetEmbedderModel');
  const input = $('embedderModelInput');
  if (btnApply && input) {
    btnApply.onclick = async () => {
      const model = input.value.trim();
      if (!model) return;
      try {
        await localLlmApi.setEmbedderModel(model);
        toastSuccess(t('settings.embedder_model_set', '嵌入模型已切换') + ': ' + model);
      } catch (err) {
        toastError(err);
      }
    };
  }
}

// ============================================================
// PagedAttention 设置（Pro 功能）
// ============================================================

/**
 * 渲染 PagedAttention 设置面板到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderPagedAttnSettings(container) {
  if (!container) return;

  const isPro = get('isPro');

  container.innerHTML = `
    <div class="flex items-center justify-between py-2 border-t border-border-default" id="pagedAttnSection">
      <div class="flex flex-col gap-0.5 flex-1">
        <span data-i18n="settings.paged_attn">PagedAttention</span>
        <span class="text-xs text-text-tertiary leading-tight" data-i18n="settings.paged_attn_desc">分页注意力机制，降低显存占用以支持更长上下文</span>
      </div>
      <div class="shrink-0 ml-4 flex items-center gap-2 ${isPro ? '' : 'opacity-40 pointer-events-none'}">
        <button id="pagedAttnToggle" class="relative w-11 h-6 rounded-full transition-colors bg-slate-600" role="switch" aria-checked="false" ${isPro ? '' : 'disabled'}>
          <span class="absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform"></span>
        </button>
        <input type="number" id="pagedAttnBlockSize" class="px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary"
               value="512" min="64" max="8192" step="64" style="width: 90px;" placeholder="512" ${isPro ? '' : 'disabled'} title="Block Size"/>
        <input type="number" id="pagedAttnGpuMem" class="px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary"
               value="512" min="128" max="8192" step="128" style="width: 90px;" placeholder="512" ${isPro ? '' : 'disabled'} title="GPU Memory (MB)"/>
      </div>
    </div>
  `;

  if (!isPro) return;

  const toggle = $('pagedAttnToggle');
  if (toggle) {
    toggle.onclick = async () => {
      const isEnabled = toggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !isEnabled;
      const blockSize = parseInt($('pagedAttnBlockSize')?.value || '512', 10);
      const gpuMem = parseInt($('pagedAttnGpuMem')?.value || '512', 10);
      try {
        await localLlmApi.setPagedAttn(newEnabled, blockSize, gpuMem);
        toggle.setAttribute('aria-checked', String(newEnabled));
        toggle.classList.toggle('bg-accent', newEnabled);
        toggle.classList.toggle('bg-slate-600', !newEnabled);
        toggle.querySelector('span').classList.toggle('translate-x-5', newEnabled);
        toastSuccess(newEnabled ? t('settings.paged_attn_enabled', 'PagedAttention 已启用') : t('settings.paged_attn_disabled', 'PagedAttention 已禁用'));
      } catch (err) {
        toastError(err);
      }
    };
  }
}

// ============================================================
// GEMV 内核模式设置（Pro 功能）
// ============================================================

/**
 * 渲染 GEMV 内核模式选择器到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderKernelModeSettings(container) {
  if (!container) return;

  const isPro = get('isPro');
  let currentMode = 'mistralrs';
  try {
    currentMode = await localLlmApi.getKernelMode();
  } catch (_) {
    // 静默降级
  }

  const isMistral = currentMode === 'mistralrs' || !currentMode;

  container.innerHTML = `
    <div class="flex items-center justify-between py-2 border-t border-border-default" id="kernelModeSection">
      <div class="flex flex-col gap-0.5 flex-1">
        <span data-i18n="settings.kernel_mode">GEMV Kernel Mode</span>
        <span class="text-xs text-text-tertiary leading-tight" data-i18n="settings.kernel_mode_desc">选择本地推理的 GEMV 内核实现</span>
      </div>
      <div class="shrink-0 ml-4 flex items-center gap-1.5 ${isPro ? '' : 'opacity-40 pointer-events-none'}">
        <button class="px-3 py-1.5 text-xs rounded-lg border transition-colors ${isMistral ? 'bg-accent/15 text-accent border-accent/30' : 'border-border-default text-text-tertiary hover:bg-surface-3'}"
          id="kernelBtnMistral" ${isPro ? '' : 'disabled'} data-i18n="settings.kernel_mistral">Mistral.rs</button>
        <button class="px-3 py-1.5 text-xs rounded-lg border transition-colors ${!isMistral ? 'bg-accent/15 text-accent border-accent/30' : 'border-border-default text-text-tertiary hover:bg-surface-3'}"
          id="kernelBtnGemv" ${isPro ? '' : 'disabled'} data-i18n="settings.kernel_custom_gemv">Custom GEMV</button>
      </div>
    </div>
  `;

  if (!isPro) return;

  const btnMistral = $('kernelBtnMistral');
  const btnGemv = $('kernelBtnGemv');
  if (btnMistral) {
    btnMistral.onclick = async () => {
      try {
        await localLlmApi.setKernelMode('mistralrs');
        btnMistral.className = 'px-3 py-1.5 text-xs rounded-lg border transition-colors bg-accent/15 text-accent border-accent/30';
        btnGemv.className = 'px-3 py-1.5 text-xs rounded-lg border transition-colors border-border-default text-text-tertiary hover:bg-surface-3';
        toastSuccess(t('settings.kernel_mode_switched', '内核模式已切换'));
      } catch (err) {
        toastError(err);
      }
    };
  }
  if (btnGemv) {
    btnGemv.onclick = async () => {
      try {
        await localLlmApi.setKernelMode('custom_gemv');
        btnGemv.className = 'px-3 py-1.5 text-xs rounded-lg border transition-colors bg-accent/15 text-accent border-accent/30';
        btnMistral.className = 'px-3 py-1.5 text-xs rounded-lg border transition-colors border-border-default text-text-tertiary hover:bg-surface-3';
        toastSuccess(t('settings.kernel_mode_switched', '内核模式已切换'));
      } catch (err) {
        toastError(err);
      }
    };
  }
}
