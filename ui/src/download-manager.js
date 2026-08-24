/**
 * EchoMind 下载管理器模块（REQ-LLM-004 v2）。
 *
 * 职责：
 * 1. 下载管理器面板（查看所有活跃/待恢复下载）
 * 2. 暂停 / 恢复 / 取消按钮逻辑
 * 3. 崩溃恢复提示（启动时扫描 .partial + .meta.json）
 * 4. model_download_progress 事件统一监听
 * 5. 下载速度 / 大小格式化
 *
 * 后端 IPC 全部就绪：
 * - pause_download / abort_download / get_download_status
 * - list_pending_downloads / cleanup_partial_downloads / scan_download_recovery
 * - download_model（启动下载，支持断点续传）
 */

import { $ } from './utils.js';
import { invoke, listen, downloadApi, localLlmApi } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { t } from './i18n.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { createFocusTrap } from './focus-trap.js';
import { pushPanel, removePanel } from './panel-stack.js';

// ============================================================
// 模块级状态
// ============================================================

/** 活跃下载映射：filename -> DownloadState */
const _downloads = new Map();

/** 面板是否打开 */
let _panelOpen = false;

/** Focus Trap 实例 */
let _panelTrap = null;

/** model_download_progress 事件取消监听函数 */
let _unlistenProgress = null;

/** 事件监听是否已注册 */
let _listenerRegistered = false;

/** 启动时崩溃恢复检查是否已执行 */
let _recoveryChecked = false;

/**
 * @typedef {Object} DownloadState
 * @property {string} filename - 文件名
 * @property {string} displayName - 显示名称
 * @property {string} url - 下载 URL（用于恢复）
 * @property {string} status - 状态：downloading / paused / completed / failed / verifying / queued
 * @property {number} downloaded - 已下载字节
 * @property {number} total - 总字节
 * @property {number} speed - 速度（字节/秒）
 * @property {string|null} error - 错误信息
 * @property {number} lastUpdated - 最后更新时间戳
 */

// ============================================================
// 格式化工具
// ============================================================

/**
 * 格式化文件大小（支持 GB 单位，GGUF 模型通常 2~5GB）。
 * @param {number} bytes - 字节数
 * @returns {string} 人类可读的大小字符串
 */
function formatSize(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  if (bytes < 1024 * 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB';
  return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB';
}

/**
 * 格式化下载速度。
 * @param {number} bytesPerSec - 字节/秒
 * @returns {string} 人类可读的速度字符串
 */
function formatSpeed(bytesPerSec) {
  if (!bytesPerSec || bytesPerSec <= 0) return '—';
  if (bytesPerSec < 1024) return bytesPerSec + ' B/s';
  if (bytesPerSec < 1024 * 1024) return (bytesPerSec / 1024).toFixed(0) + ' KB/s';
  if (bytesPerSec < 1024 * 1024 * 1024) return (bytesPerSec / 1024 / 1024).toFixed(1) + ' MB/s';
  return (bytesPerSec / 1024 / 1024 / 1024).toFixed(2) + ' GB/s';
}

/**
 * 计算下载百分比。
 * @param {number} downloaded - 已下载字节
 * @param {number} total - 总字节
 * @returns {number} 百分比（0-100）
 */
function calcPct(downloaded, total) {
  if (total > 0) return Math.min(100, Math.round((downloaded / total) * 100));
  const dl = downloaded || 0;
  return Math.min(95, Math.round(30 + 15 * Math.log10(1 + dl / 65536)));
}

/**
 * 获取状态标签文本。
 * @param {string} status - 下载状态
 * @returns {string} i18n key
 */
function statusLabelKey(status) {
  switch (status) {
    case 'downloading': return 'download.status_downloading';
    case 'paused': return 'download.status_paused';
    case 'completed': return 'download.status_completed';
    case 'failed': return 'download.status_failed';
    case 'verifying': return 'download.status_verifying';
    case 'queued': return 'download.status_queued';
    default: return 'download.status_downloading';
  }
}

/**
 * 获取状态标签颜色类。
 * @param {string} status - 下载状态
 * @returns {string} Tailwind 类名
 */
function statusColorClass(status) {
  switch (status) {
    case 'downloading': return 'bg-accent/15 text-accent border-accent/30';
    case 'paused': return 'bg-amber-500/15 text-amber-400 border-amber-400/30';
    case 'completed': return 'bg-green-500/15 text-green-400 border-green-400/30';
    case 'failed': return 'bg-red-500/15 text-red-400 border-red-400/30';
    case 'verifying': return 'bg-purple-500/15 text-purple-400 border-purple-400/30';
    default: return 'bg-surface-3 text-text-tertiary border-border-default';
  }
}

// ============================================================
// 下载状态管理
// ============================================================

/**
 * 更新下载状态（合并写入，触发 UI 刷新）。
 * @param {string} filename - 文件名
 * @param {Partial<DownloadState>} patch - 要更新的字段
 */
function updateDownload(filename, patch) {
  const existing = _downloads.get(filename) || {
    filename,
    displayName: filename,
    url: '',
    status: 'queued',
    downloaded: 0,
    total: 0,
    speed: 0,
    error: null,
    lastUpdated: Date.now(),
  };
  _downloads.set(filename, {
    ...existing,
    ...patch,
    lastUpdated: Date.now(),
  });
  renderDownloadList();
  updatePanelBadge();
}

/**
 * 从 DownloadStatus 枚举（后端 serde 序列化）提取状态信息。
 *
 * 后端 DownloadStatus 是 tagged enum，serde rename_all = "snake_case"：
 * - { downloading: { completed, total, speed } }
 * - { paused: { completed, total } }
 * - { verifying: { completed, total } }
 * - "completed"
 * - { failed: { error, completed, total } }
 * - "queued"
 *
 * @param {Object|string} status - DownloadStatus 枚举值
 * @returns {{status: string, downloaded: number, total: number, speed: number, error: string|null}}
 */
function parseDownloadStatus(status) {
  if (typeof status === 'string') {
    return { status, downloaded: 0, total: 0, speed: 0, error: null };
  }
  if (status && typeof status === 'object') {
    if (status.downloading) {
      return {
        status: 'downloading',
        downloaded: status.downloading.completed || 0,
        total: status.downloading.total || 0,
        speed: status.downloading.speed || 0,
        error: null,
      };
    }
    if (status.paused) {
      return {
        status: 'paused',
        downloaded: status.paused.completed || 0,
        total: status.paused.total || 0,
        speed: 0,
        error: null,
      };
    }
    if (status.verifying) {
      return {
        status: 'verifying',
        downloaded: status.verifying.completed || 0,
        total: status.verifying.total || 0,
        speed: 0,
        error: null,
      };
    }
    if (status.failed) {
      return {
        status: 'failed',
        downloaded: status.failed.completed || 0,
        total: status.failed.total || 0,
        speed: 0,
        error: status.failed.error || 'Unknown error',
      };
    }
  }
  return { status: 'queued', downloaded: 0, total: 0, speed: 0, error: null };
}

// ============================================================
// 事件监听
// ============================================================

/**
 * 注册 model_download_progress 事件监听（全局，只需注册一次）。
 *
 * 事件 payload 格式（ModelDownloadProgressPayload）：
 * { filename, downloaded, total, speed }
 */
async function registerProgressListener() {
  if (_listenerRegistered) return;
  _listenerRegistered = true;
  try {
    _unlistenProgress = await listen('model_download_progress', (event) => {
      const p = event.payload;
      if (!p || !p.filename) return;
      updateDownload(p.filename, {
        downloaded: p.downloaded || 0,
        total: p.total || 0,
        speed: p.speed || 0,
        status: 'downloading',
        error: null,
      });
    });
  } catch (_) {
    _listenerRegistered = false;
  }
}

// ============================================================
// 面板 UI 渲染
// ============================================================

/**
 * 渲染下载列表。
 */
function renderDownloadList() {
  const container = $('downloadManagerList');
  if (!container) return;

  if (_downloads.size === 0) {
    container.innerHTML = `
      <div class="flex flex-col items-center justify-center py-12 text-center">
        <svg class="w-12 h-12 text-text-quaternary mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
            d="M7 16a4 4 0 01-.88-7.9A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3 3-3M12 12v10"/>
        </svg>
        <p class="text-sm text-text-quaternary" data-i18n="download.empty_hint">${t('download.empty_hint')}</p>
      </div>`;
    return;
  }

  const items = Array.from(_downloads.values()).sort((a, b) => b.lastUpdated - a.lastUpdated);
  container.innerHTML = items.map((dl) => {
    const pct = calcPct(dl.downloaded, dl.total);
    const isUnknownTotal = !dl.total || dl.total <= 0;
    const isActive = dl.status === 'downloading';
    const isPaused = dl.status === 'paused';
    const isCompleted = dl.status === 'completed';
    const isFailed = dl.status === 'failed';
    const isVerifying = dl.status === 'verifying';

    const escapedFilename = dl.filename.replace(/'/g, "\\'");

    // 操作按钮
    let actions = '';
    if (isActive) {
      actions += `<button class="px-2 py-1 text-[11px] rounded-lg bg-amber-500/15 text-amber-400 border border-amber-400/30 hover:bg-amber-500/25 transition-colors"
        onclick="pauseDownload('${escapedFilename}')" data-i18n="download.btn_pause">${t('download.btn_pause')}</button>`;
    }
    if (isPaused) {
      actions += `<button class="px-2 py-1 text-[11px] rounded-lg bg-accent/15 text-accent border border-accent/30 hover:bg-accent/25 transition-colors"
        onclick="resumeDownload('${escapedFilename}')" data-i18n="download.btn_resume">${t('download.btn_resume')}</button>`;
    }
    if (!isCompleted) {
      actions += `<button class="px-2 py-1 text-[11px] rounded-lg bg-red-500/15 text-red-400 border border-red-400/30 hover:bg-red-500/25 transition-colors"
        onclick="cancelDownload('${escapedFilename}')" data-i18n="download.btn_cancel">${t('download.btn_cancel')}</button>`;
    }

    // 进度条
    const barClass = isFailed ? 'bg-red-500' : isCompleted ? 'bg-green-500' : isPaused ? 'bg-amber-500' : 'progress-shimmer';
    const indeterminateClass = isUnknownTotal && isActive ? 'progress-indeterminate' : '';

    // 大小 / 速度信息
    let sizeText = '';
    if (isCompleted) {
      sizeText = formatSize(dl.total || dl.downloaded);
    } else if (isUnknownTotal) {
      sizeText = formatSize(dl.downloaded);
    } else {
      sizeText = formatSize(dl.downloaded) + ' / ' + formatSize(dl.total);
    }

    let speedText = '';
    if (isActive && dl.speed > 0) {
      speedText = formatSpeed(dl.speed);
    } else if (isPaused) {
      speedText = t('download.paused_hint');
    } else if (isVerifying) {
      speedText = t('download.verifying_hint');
    } else if (isFailed) {
      speedText = dl.error || t('download.failed_hint');
    }

    return `
    <div class="bg-surface-2 border border-border-default rounded-xl px-4 py-3 space-y-2" data-download-filename="${dl.filename}">
      <div class="flex items-center justify-between gap-2">
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span class="text-sm text-text-primary truncate">${dl.displayName}</span>
            <span class="shrink-0 px-1.5 py-0.5 rounded text-[10px] border ${statusColorClass(dl.status)}">${t(statusLabelKey(dl.status))}</span>
          </div>
          <div class="text-[11px] text-text-quaternary mt-0.5">${speedText || sizeText}</div>
        </div>
        <div class="flex items-center gap-1 shrink-0">
          ${actions}
        </div>
      </div>
      <div class="flex items-center gap-2">
        <div class="flex-1 w-full bg-surface-0 rounded-full h-1.5 overflow-hidden">
          <div class="${barClass} ${indeterminateClass} h-full rounded-full transition-all duration-300" style="width: ${pct}%"></div>
        </div>
        <span class="text-[11px] text-text-quaternary shrink-0 w-10 text-right">${pct}%</span>
      </div>
      ${!isUnknownTotal && !isCompleted ? `<div class="text-[10px] text-text-quaternary">${sizeText}</div>` : ''}
      ${isFailed && dl.error ? `<div class="text-[10px] text-red-400/80">${dl.error}</div>` : ''}
    </div>`;
  }).join('');
}

/**
 * 更新面板徽章（活跃下载数量）。
 */
function updatePanelBadge() {
  const badge = $('downloadManagerBadge');
  if (!badge) return;
  const activeCount = Array.from(_downloads.values()).filter(
    (dl) => dl.status === 'downloading' || dl.status === 'queued' || dl.status === 'verifying'
  ).length;
  if (activeCount > 0) {
    badge.textContent = String(activeCount);
    badge.classList.remove('hidden');
  } else {
    badge.classList.add('hidden');
  }
}

// ============================================================
// 面板开关
// ============================================================

/**
 * 打开下载管理器面板。
 */
export function openDownloadManager() {
  const overlay = $('downloadManagerOverlay');
  if (!overlay) return;
  overlay.classList.remove('hidden');
  _panelOpen = true;
  // 刷新列表
  refreshDownloadList();
  // Focus Trap
  const panel = $('downloadManagerPanel');
  if (panel) {
    _panelTrap = createFocusTrap(panel);
    _panelTrap.activate();
  }
  // 注册到面板栈
  pushPanel({ id: 'download-manager', close: closeDownloadManager, element: overlay, label: 'Download Manager' });
}

/**
 * 关闭下载管理器面板。
 */
export function closeDownloadManager() {
  removePanel('download-manager');
  const overlay = $('downloadManagerOverlay');
  if (!overlay) return;
  overlay.classList.add('hidden');
  _panelOpen = false;
  if (_panelTrap) {
    _panelTrap.deactivate();
    _panelTrap = null;
  }
}

/**
 * 切换面板可见性。
 */
export function toggleDownloadManager() {
  if (_panelOpen) closeDownloadManager();
  else openDownloadManager();
}

// ============================================================
// 下载操作（暂停 / 恢复 / 取消）
// ============================================================

/**
 * 暂停下载（保留 .partial + .meta.json，可恢复）。
 * @param {string} filename - 文件名
 */
export async function pauseDownload(filename) {
  try {
    await downloadApi.pause(filename);
    updateDownload(filename, { status: 'paused', speed: 0 });
    toast(t('download.paused_msg', { name: filename }), 'info');
  } catch (err) {
    toastError(err);
  }
}
window.pauseDownload = pauseDownload;

/**
 * 恢复暂停的下载（重新调用 download_model，后端自动检测 .partial 续传）。
 * @param {string} filename - 文件名
 */
export async function resumeDownload(filename) {
  const dl = _downloads.get(filename);
  if (!dl || !dl.url) {
    toastError(new Error(t('download.resume_no_url')));
    return;
  }
  try {
    updateDownload(filename, { status: 'downloading', error: null });
    await localLlmApi.download(dl.url, filename);
    toast(t('download.resumed_msg', { name: filename }), 'info');
  } catch (err) {
    updateDownload(filename, { status: 'failed', error: String(err) });
    toastError(err);
  }
}
window.resumeDownload = resumeDownload;

/**
 * 取消下载 + 清理临时文件。
 * @param {string} filename - 文件名
 */
export async function cancelDownload(filename) {
  const ok = await showConfirmDialog({
    title: t('download.cancel_confirm', { name: filename }),
    confirmText: t('download.btn_cancel'),
    danger: true,
  });
  if (!ok) return;

  try {
    await downloadApi.abort(filename);
    _downloads.delete(filename);
    renderDownloadList();
    updatePanelBadge();
    toast(t('download.cancelled_msg', { name: filename }), 'info');
  } catch (err) {
    toastError(err);
  }
}
window.cancelDownload = cancelDownload;

// ============================================================
// 启动下载（统一入口，供 settings-local-llm.js 调用）
// ============================================================

/**
 * 启动模型下载（注册到下载管理器 + 调用后端）。
 *
 * 替代 settings-local-llm.js 中的 downloadLocalModel。
 * 下载进度通过 model_download_progress 事件统一接收。
 *
 * @param {string} url - 下载 URL
 * @param {string} filename - 目标文件名
 * @param {string} [displayName] - 显示名称（可选）
 * @returns {Promise<void>}
 */
export async function startDownload(url, filename, displayName) {
  // 注册到下载管理器
  updateDownload(filename, {
    url,
    displayName: displayName || filename,
    status: 'queued',
    downloaded: 0,
    total: 0,
    speed: 0,
    error: null,
  });

  // 自动打开面板
  openDownloadManager();

  try {
    await localLlmApi.download(url, filename);
    updateDownload(filename, { status: 'completed', speed: 0 });
    toastSuccess(t('download.complete_msg', { name: displayName || filename }));
  } catch (err) {
    // 如果是用户暂停导致的中断，不算失败
    const dl = _downloads.get(filename);
    if (dl && dl.status === 'paused') return;
    updateDownload(filename, { status: 'failed', error: String(err) });
    toastError(err);
  }
}
window.startDownload = startDownload;

// ============================================================
// 列表刷新
// ============================================================

/**
 * 从后端刷新下载列表（合并 list_pending_downloads 结果）。
 */
async function refreshDownloadList() {
  try {
    const pending = await downloadApi.listPending();
    if (!pending || pending.length === 0) return;

    for (const item of pending) {
      const parsed = parseDownloadStatus(item.status);
      const existing = _downloads.get(item.filename);
      // 不覆盖活跃下载的实时进度（后端 listPending 可能比事件慢）
      if (existing && existing.status === 'downloading') continue;

      updateDownload(item.filename, {
        filename: item.filename,
        displayName: existing?.displayName || item.filename,
        url: existing?.url || '',
        status: parsed.status,
        downloaded: parsed.downloaded,
        total: parsed.total || item.total_size,
        speed: parsed.speed,
        error: parsed.error,
      });
    }
  } catch (_) {
    // 静默失败，不阻塞面板
  }
}

// ============================================================
// 崩溃恢复
// ============================================================

/**
 * 启动时检查崩溃恢复（扫描 .partial + .meta.json）。
 *
 * 如果检测到未完成的下载，显示恢复提示 Modal。
 * 用户可选择恢复全部 / 丢弃全部 / 逐个处理。
 */
export async function checkCrashRecovery() {
  if (_recoveryChecked) return;
  _recoveryChecked = true;

  try {
    const manifests = await downloadApi.scanRecovery();
    if (!manifests || manifests.length === 0) return;

    // 注册到下载管理器
    for (const m of manifests) {
      const parsed = parseDownloadStatus(m.status);
      updateDownload(m.filename, {
        filename: m.filename,
        displayName: m.filename,
        url: m.url,
        status: parsed.status === 'queued' ? 'paused' : parsed.status,
        downloaded: parsed.downloaded,
        total: parsed.total || m.total_size,
        speed: 0,
        error: parsed.error,
      });
    }

    // 显示恢复提示 Modal
    showRecoveryModal(manifests);
  } catch (_) {
    // 静默失败，不阻塞启动
  }
}

/**
 * 显示崩溃恢复提示 Modal。
 * @param {Array} manifests - DownloadManifest 列表
 */
function showRecoveryModal(manifests) {
  const modal = $('downloadRecoveryModal');
  const list = $('downloadRecoveryList');
  if (!modal || !list) return;

  // 渲染可恢复文件列表
  list.innerHTML = manifests.map((m) => {
    const parsed = parseDownloadStatus(m.status);
    const pct = calcPct(parsed.downloaded, parsed.total || m.total_size);
    const sizeText = formatSize(parsed.downloaded) + ' / ' + formatSize(m.total_size);
    return `
    <div class="flex items-center justify-between gap-2 py-2 px-3 rounded-lg bg-surface-2 border border-border-default">
      <div class="flex-1 min-w-0">
        <div class="text-sm text-text-primary truncate">${m.filename}</div>
        <div class="text-[11px] text-text-quaternary">${sizeText} · ${pct}%</div>
      </div>
      <div class="w-20 bg-surface-0 rounded-full h-1 overflow-hidden">
        <div class="bg-amber-500 h-full rounded-full" style="width: ${pct}%"></div>
      </div>
    </div>`;
  }).join('');

  modal.classList.remove('hidden');
}

/**
 * 恢复全部崩溃下载。
 */
export async function resumeAllRecovery() {
  const modal = $('downloadRecoveryModal');
  if (modal) modal.classList.add('hidden');

  const items = Array.from(_downloads.values()).filter((dl) => dl.status === 'paused' && dl.url);
  if (items.length === 0) return;

  openDownloadManager();

  // 并行恢复所有暂停的下载
  for (const dl of items) {
    try {
      updateDownload(dl.filename, { status: 'downloading', error: null });
      await localLlmApi.download(dl.url, dl.filename);
      updateDownload(dl.filename, { status: 'completed', speed: 0 });
    } catch (err) {
      const current = _downloads.get(dl.filename);
      if (current && current.status === 'paused') continue;
      updateDownload(dl.filename, { status: 'failed', error: String(err) });
    }
  }
  toastSuccess(t('download.recovery_resumed'));
}
window.resumeAllRecovery = resumeAllRecovery;

/**
 * 丢弃全部崩溃下载（清理 .partial + .meta.json）。
 */
export async function discardAllRecovery() {
  const modal = $('downloadRecoveryModal');
  if (modal) modal.classList.add('hidden');

  try {
    const freedBytes = await downloadApi.cleanupPartials();
    _downloads.clear();
    renderDownloadList();
    updatePanelBadge();
    toast(t('download.recovery_discarded', { size: formatSize(freedBytes) }), 'info');
  } catch (err) {
    toastError(err);
  }
}
window.discardAllRecovery = discardAllRecovery;

// ============================================================
// 清理全部已完成
// ============================================================

/**
 * 清理所有已完成/已失败的下载条目（仅从 UI 移除，不删文件）。
 */
export function clearCompletedDownloads() {
  const toRemove = [];
  for (const [filename, dl] of _downloads) {
    if (dl.status === 'completed' || dl.status === 'failed') {
      toRemove.push(filename);
    }
  }
  if (toRemove.length === 0) {
    toast(t('download.no_completed'), 'info');
    return;
  }
  for (const filename of toRemove) {
    _downloads.delete(filename);
  }
  renderDownloadList();
  updatePanelBadge();
  toast(t('download.cleared', { count: toRemove.length }), 'info');
}
window.clearCompletedDownloads = clearCompletedDownloads;

// ============================================================
// 初始化
// ============================================================

/**
 * 初始化下载管理器（注册事件 + 绑定 DOM + 检查崩溃恢复）。
 *
 * 在 main.js 启动流程中调用。
 */
export async function initDownloadManager() {
  // 注册 model_download_progress 事件监听
  await registerProgressListener();

  // 绑定面板事件
  const overlay = $('downloadManagerOverlay');
  if (overlay) {
    // 点击遮罩关闭
    overlay.addEventListener('click', (e) => {
      if (e.target === overlay) closeDownloadManager();
    });
  }

  const closeBtn = $('downloadManagerClose');
  if (closeBtn) closeBtn.onclick = closeDownloadManager;

  const clearBtn = $('downloadManagerClear');
  if (clearBtn) clearBtn.onclick = clearCompletedDownloads;

  // 恢复 Modal 按钮
  const resumeBtn = $('downloadRecoveryResume');
  if (resumeBtn) resumeBtn.onclick = resumeAllRecovery;

  const discardBtn = $('downloadRecoveryDiscard');
  if (discardBtn) discardBtn.onclick = discardAllRecovery;

  // 启动时检查崩溃恢复
  await checkCrashRecovery();
}
