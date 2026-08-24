/**
 * disk-space.js 单元测试 — 存储空间卡片（V3.1 P1-5 / REQ-ERR-004）。
 *
 * 覆盖：
 * - 正常渲染：进度条宽度 / 可用空间文案 / aria 属性
 * - 低空间警示态：is_low=true 时红色条
 * - IPC 失败降级：占位文案不抛错
 * - 清理流程：确认 → cleanup → toast + 卡片刷新；取消则不调用 IPC
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

const mocks = vi.hoisted(() => ({
  getInfo: vi.fn(),
  cleanup: vi.fn(),
  confirm: vi.fn(),
  toastSuccess: vi.fn(),
  t: vi.fn((k) => k),
}));

vi.mock('../../../ui/src/ipc.js', () => ({
  diskApi: { getInfo: mocks.getInfo, cleanup: mocks.cleanup, check: vi.fn() },
}));
vi.mock('../../../ui/src/i18n.js', () => ({ t: mocks.t }));
vi.mock('../../../ui/src/confirm-dialog.js', () => ({ showConfirmDialog: mocks.confirm }));
vi.mock('../../../ui/src/toast.js', () => ({ toastSuccess: mocks.toastSuccess, toast: vi.fn(), toastError: vi.fn() }));

import { renderDiskSpaceCard } from '../../../ui/src/disk-space.js';

function info(over = {}) {
  return JSON.stringify({
    free_bytes: 100 * 1024 ** 3,
    total_bytes: 500 * 1024 ** 3,
    used_bytes: 400 * 1024 ** 3,
    free_percent: 20,
    is_low: false,
    threshold_bytes: 1024 ** 3,
    ...over,
  });
}

describe('renderDiskSpaceCard', () => {
  let container;

  beforeEach(() => {
    document.body.innerHTML = '';
    container = document.createElement('div');
    container.id = 'diskSpaceCard';
    document.body.appendChild(container);
    vi.clearAllMocks();
    mocks.t.mockImplementation((k) => (k === 'settings.disk_cleanup_done' ? 'Freed {size}' : k));
  });

  it('正常信息渲染进度条与可用空间', async () => {
    mocks.getInfo.mockResolvedValue(info());
    await renderDiskSpaceCard(container);

    const bar = container.querySelector('[role="progressbar"]');
    expect(bar).toBeTruthy();
    expect(bar.getAttribute('aria-valuenow')).toBe('80');
    // 可用空间 100GB
    expect(container.textContent).toContain('100.0 GB');
    expect(container.querySelector('#diskCleanupBtn')).toBeTruthy();
  });

  it('低空间时进度条使用警示色', async () => {
    mocks.getInfo.mockResolvedValue(info({ is_low: true, free_percent: 3, free_bytes: 15 * 1024 ** 3 }));
    await renderDiskSpaceCard(container);

    expect(container.querySelector('.h-full').className).toContain('bg-red-500');
    expect(container.querySelector('.h-full').className).not.toContain('bg-accent');
  });

  it('IPC 失败时降级占位且不抛错', async () => {
    mocks.getInfo.mockRejectedValue(new Error('boom'));
    await expect(renderDiskSpaceCard(container)).resolves.toBeUndefined();
    expect(container.querySelector('#diskCleanupBtn')).toBeNull();
    expect(container.textContent.length).toBeGreaterThan(0);
  });

  it('null 容器安全无操作', async () => {
    await expect(renderDiskSpaceCard(null)).resolves.toBeUndefined();
    expect(mocks.getInfo).not.toHaveBeenCalled();
  });

  it('确认清理后调用 IPC 并刷新卡片', async () => {
    mocks.getInfo.mockResolvedValue(info());
    mocks.confirm.mockResolvedValue(true);
    mocks.cleanup.mockResolvedValue(512 * 1024 ** 2);

    await renderDiskSpaceCard(container);
    mocks.getInfo.mockClear();

    container.querySelector('#diskCleanupBtn').click();
    await vi.waitFor(() => expect(mocks.cleanup).toHaveBeenCalled());
    await vi.waitFor(() => expect(mocks.toastSuccess).toHaveBeenCalled());
    expect(mocks.toastSuccess.mock.calls[0][0]).toContain('MB');
    // 清理后刷新
    await vi.waitFor(() => expect(mocks.getInfo).toHaveBeenCalled());
  });

  it('取消确认对话框时不调用清理 IPC', async () => {
    mocks.getInfo.mockResolvedValue(info());
    mocks.confirm.mockResolvedValue(false);

    await renderDiskSpaceCard(container);
    container.querySelector('#diskCleanupBtn').click();
    // 等待一个微任务周期确认没有调用
    await new Promise((r) => setTimeout(r, 0));
    expect(mocks.cleanup).not.toHaveBeenCalled();
  });
});
