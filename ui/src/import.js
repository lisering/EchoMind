/**
 * EchoMind 文档导入模块 — 文件选择 / 拖拽导入 / 进度条 / 配额墙。
 *
 * 职责：
 * 1. 批量导入文件路径（import_files IPC）
 * 2. 拖拽事件监听（tauri://drag-enter/leave/drop）
 * 3. 导入进度条显示/隐藏
 * 4. 拖拽遮罩显示/隐藏
 * 5. 导入取消
 * 6. 文件大小限制与警告（REQ-ING-013）
 */

import { setState, get } from './state.js';
import { $, getSubPhaseLabel, formatBytes } from './utils.js';
import { invoke, listen, openDialog, importApi } from './ipc.js';
import { toast, toastError } from './toast.js';
import { showPaywall } from './wizard.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { t } from './i18n.js';

/**
 * 显示导入进度条（REQ-ING-006）。
 * @param {number} total - 文件总数
 */
export function showImportProgress(total) {
  setState({ importing: true });
  $('importProgress').classList.remove('hidden');
  const bar = $('importProgressBar');
  if (bar) {
    bar.style.width = '0%';
    bar.classList.remove('progress-complete', 'progress-error', 'progress-indeterminate');
  }
  $('importProgressText').textContent = t('import.progress_start', { total });
}

/** 隐藏导入进度条。 */
export function hideImportProgress() {
  setState({ importing: false });
  const bar = $('importProgressBar');
  if (bar) {
    bar.classList.add('progress-complete');
  }
  $('importProgress').classList.add('hidden');
}

/**
 * 检查文件大小并过滤（REQ-ING-013）。
 * - >500MB：直接拒绝，toast 提示
 * - >100MB：弹出警告对话框，用户确认后保留
 * - <=100MB：正常导入
 *
 * @param {string[]} paths - 原始文件路径数组
 * @returns {Promise<string[]>} 过滤后的文件路径数组（用户确认导入的文件）
 */
async function checkFileSizes(paths) {
  try {
    const sizes = await invoke('get_file_sizes', { paths });
    const [warnThreshold, hardLimit] = await invoke('get_file_size_limits');

    const normal = [];
    const warning = [];
    const rejected = [];

    for (const [path, size] of sizes) {
      if (size > hardLimit) {
        rejected.push({ path, size });
      } else if (size > warnThreshold) {
        warning.push({ path, size });
      } else {
        normal.push(path);
      }
    }

    // 拒绝的文件（>500MB）：toast 提示
    for (const { path, size } of rejected) {
      const name = path.split('/').pop() || path;
      toastError(t('import.file_too_large', { name, size: formatBytes(size), limit: formatBytes(hardLimit) }));
    }

    // 警告的文件（>100MB）：弹确认对话框
    if (warning.length > 0) {
      const confirmed = await showFileSizeWarning(warning);
      if (confirmed) {
        normal.push(...warning.map((w) => w.path));
      }
    }

    return normal;
  } catch (err) {
    // get_file_sizes 不可用时，回退为不检查（兼容旧环境）
    console.warn('[import] get_file_sizes failed, skipping size check:', err);
    return paths;
  }
}

/**
 * 显示文件大小警告对话框（REQ-ING-013 AC-1）。
 * @param {Array<{path: string, size: number}>} warningFiles - 超过警告阈值的文件列表
 * @returns {Promise<boolean>} 用户是否确认导入
 */
function showFileSizeWarning(warningFiles) {
  return new Promise((resolve) => {
    const dialog = $('fileSizeWarningDialog');
    if (!dialog) { resolve(true); return; }

    // 填充文件列表
    const listEl = $('fileSizeWarningList');
    if (listEl) {
      listEl.innerHTML = warningFiles
        .map((f) => {
          const name = f.path.split('/').pop() || f.path;
          return `<div class="flex justify-between text-xs py-1"><span class="truncate text-text-secondary">${name}</span><span class="text-amber-400 shrink-0 ml-2">${formatBytes(f.size)}</span></div>`;
        })
        .join('');
    }

    dialog.classList.remove('hidden');

    // 确认按钮
    const okBtn = $('fileSizeWarningOk');
    const cancelBtn = $('fileSizeWarningCancel');

    const cleanup = () => {
      dialog.classList.add('hidden');
      okBtn.onclick = null;
      cancelBtn.onclick = null;
    };

    okBtn.onclick = () => { cleanup(); resolve(true); };
    cancelBtn.onclick = () => { cleanup(); resolve(false); };
  });
}

/**
 * 批量导入文件路径：调用 import_files，成功刷新列表；配额/PDF 触顶时弹出付费墙。
 *
 * 错误处理策略（不依赖 invoke 全局包装器，确保错误一定有用户可见反馈）：
 * - PRO_REQUIRED / LIMIT_REACHED → 弹出付费墙 Modal
 * - 不支持的格式 → toast 提示
 * - 其他错误 → toast 提示原始消息
 *
 * @param {string[]} paths - 原始文件路径数组
 * @param {() => Promise} [onImportDone] - 导入成功后的回调（通常为 loadDocuments）
 */
export async function importPaths(paths, onImportDone) {
  if (!paths || paths.length === 0) return;

  // REQ-ING-013：导入前检查文件大小
  const filteredPaths = await checkFileSizes(paths);
  if (filteredPaths.length === 0) {
    // 所有文件都被拒绝或用户取消了警告
    return;
  }

  showImportProgress(filteredPaths.length);
  try {
    const names = await invoke('import_files', { paths: filteredPaths });
    if (names.length > 0) {
      toast(t('import.import_complete', { names: names.join('、') }), 'success');
    }
    if (onImportDone) await onImportDone();
  } catch (err) {
    const msg = String(err);
    // REQ-ING-012：同名不同内容冲突 → 弹出替换确认
    if (msg.startsWith('CONFLICT:')) {
      const parts = msg.split(':');
      const oldDocId = parts[1] || '';
      const fileName = parts.slice(2).join(':') || '';
      // 恢复输入状态（导入被中断）
      setState({ streaming: false });
      const confirmed = await showConfirmDialog({
        title: t('import.replace_title'),
        body: t('import.replace_confirm', { name: fileName }),
        confirmText: t('import.replace_btn'),
        cancelText: t('common.cancel'),
        danger: false,
      });
      if (confirmed) {
        // 用户确认替换：调用 replace_document
        showImportProgress(1);
        try {
          await importApi.replaceDocument(filteredPaths[0], oldDocId);
          toast(t('import.replace_success', { name: fileName }), 'success');
          if (onImportDone) await onImportDone();
        } catch (replaceErr) {
          toastError(String(replaceErr));
        } finally {
          hideImportProgress();
        }
      }
      // 用户取消 → 跳过该文件
    } else if (msg.includes('PRO_REQUIRED') || msg.includes('LIMIT_REACHED')) {
      // PDF 付费门 / 配额触顶 → 弹出付费墙
      const reason = msg.split(':').slice(1).join(':').trim() || t('paywall.reason_default');
      showPaywall(reason);
    } else {
      // 其他错误（格式不支持、路径非法等）→ toast 提示
      toastError(msg);
    }
  } finally {
    hideImportProgress();
  }
}

/**
 * 初始化拖拽导入事件（REQ-UI-004：Tauri 原生拖拽事件）。
 * @param {() => Promise} [onImportDone] - 导入成功后的回调
 */
export function initDragDrop(onImportDone) {
  listen('tauri://drag-enter', () => $('dragOverlay').classList.remove('hidden'));
  listen('tauri://drag-leave', () => $('dragOverlay').classList.add('hidden'));
  listen('tauri://drag-drop', (e) => {
    $('dragOverlay').classList.add('hidden');
    importPaths(e.payload.paths, onImportDone);
  });
}

/**
 * 初始化文件选择按钮（plusBtn）。
 * @param {() => Promise} [onImportDone] - 导入成功后的回调
 */
export function initFilePicker(onImportDone) {
  $('plusBtn').onclick = async () => {
    try {
      const selected = await openDialog({
        multiple: true,
        filters: [{ name: t('import.file_filter'), extensions: ['md', 'txt', 'pdf', 'docx', 'html', 'htm', 'pptx', 'epub', 'xlsx', 'csv'] }],
      });
      if (selected) importPaths(Array.isArray(selected) ? selected : [selected], onImportDone);
    } catch (err) {
      toastError(err);
    }
  };
}

/**
 * 初始化导入取消按钮。
 */
export function initImportCancel() {
  $('importCancelBtn').onclick = async () => {
    try {
      await invoke('abort_import');
      toast(t('import.cancelling'), 'info');
    } catch (err) {
      toastError(err);
    }
  };
}
