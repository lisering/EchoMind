/**
 * EchoMind 磁盘空间管理模块 — 数据分区「存储空间」卡片（REQ-ERR-004 / V3.1 P1-5）。
 *
 * 职责：
 * 1. 渲染存储空间卡片：数据目录所在磁盘的用量进度条 + 数字
 * 2. 「立即清理」按钮：确认对话框 → cleanup_disk_space → 刷新展示
 * 3. 低空间警告态（is_low=true 时进度条转警示色）
 *
 * IPC 契约（crates/tauri-app/src/commands/document.rs）：
 * - get_disk_space_info() → JSON 字符串（DiskSpaceInfo）
 * - cleanup_disk_space()  → 释放字节数
 */

import { t } from './i18n.js';
import { diskApi } from './ipc.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { toastSuccess } from './toast.js';

/** 字节 → 人类可读体积（GB/MB 自适应，保留 1 位小数）。 */
function formatBytes(bytes) {
  const gb = bytes / 1024 ** 3;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  const mb = bytes / 1024 ** 2;
  if (mb >= 1) return `${mb.toFixed(1)} MB`;
  return `${(bytes / 1024).toFixed(0)} KB`;
}

/**
 * 渲染存储空间卡片到指定容器。
 *
 * 拉取失败时静默降级为占位文案（磁盘命令失败不应阻塞设置面板）。
 *
 * @param {HTMLElement} container - 目标容器元素（#diskSpaceCard）
 * @returns {Promise<void>}
 */
export async function renderDiskSpaceCard(container) {
  if (!container) return;

  let info = null;
  try {
    info = JSON.parse(await diskApi.getInfo());
  } catch (_) {
    container.innerHTML = `<p class="text-xs text-text-quaternary">${t('settings.disk_unavailable') || '磁盘信息不可用'}</p>`;
    return;
  }

  const usedPct = Math.min(100, Math.max(0, 100 - (info.free_percent ?? 0)));
  const low = Boolean(info.is_low);
  const barColor = low ? 'bg-red-500' : 'bg-accent';

  container.innerHTML = `
    <div class="flex items-center justify-between mb-2">
      <span class="text-sm text-text-tertiary">${t('settings.disk_free') || '可用空间'}</span>
      <span class="text-sm font-medium ${low ? 'text-red-400' : 'text-text-primary'}">${formatBytes(info.free_bytes ?? 0)}</span>
    </div>
    <div class="h-1.5 w-full bg-surface-3 rounded-full overflow-hidden" role="progressbar"
         aria-valuemin="0" aria-valuemax="100" aria-valuenow="${Math.round(usedPct)}"
         aria-label="${t('settings.disk_usage') || '磁盘使用率'}">
      <div class="h-full ${barColor} rounded-full transition-all duration-300" style="width: ${usedPct}%"></div>
    </div>
    <p class="text-xs text-text-quaternary mt-2">${t('settings.disk_total') || '总容量'}：${formatBytes(info.total_bytes ?? 0)}</p>
    <button id="diskCleanupBtn" class="mt-3 w-full border border-border-default rounded-lg px-3 py-2 text-sm text-text-secondary hover:bg-surface-3 hover:text-text-primary transition-colors">
      ${t('settings.disk_cleanup') || '清理缓存空间'}
    </button>
  `;

  const btn = /** @type {HTMLButtonElement|null} */ (container.querySelector('#diskCleanupBtn'));
  btn?.addEventListener('click', () => _cleanup(btn));
}

/**
 * 执行清理：确认对话框 → IPC → 结果反馈 + 刷新卡片。
 *
 * @param {HTMLButtonElement} btn - 触发按钮（执行期间禁用防重复点击）
 * @returns {Promise<void>}
 */
async function _cleanup(btn) {
  const ok = await showConfirmDialog({
    title: t('settings.disk_cleanup_confirm_title') || '清理缓存空间？',
    body: t('settings.disk_cleanup_confirm_body') || '将清理模型缓存与临时文件，不影响已导入文档。',
    confirmText: t('settings.disk_cleanup') || '清理',
    danger: false,
  });
  if (!ok) return;

  btn.disabled = true;
  btn.classList.add('opacity-50', 'cursor-not-allowed');
  try {
    const freed = await diskApi.cleanup();
    toastSuccess(
      (t('settings.disk_cleanup_done') || '已释放 {size}').replace('{size}', formatBytes(freed ?? 0))
    );
  } catch (_) {
    // 清理失败不阻塞面板；按钮恢复后用户可重试
  } finally {
    btn.disabled = false;
    btn.classList.remove('opacity-50', 'cursor-not-allowed');
    renderDiskSpaceCard(btn.closest('[data-disk-card]') ?? btn.parentElement);
  }
}
