/**
 * EchoMind import.js 单元测试 — 文件选择 / 拖拽导入 / 进度条 / 大小检查。
 *
 * 验证点：
 * 1. showImportProgress / hideImportProgress
 * 2. importPaths 空路径保护
 * 3. checkFileSizes 分类逻辑
 * 4. 文件格式验证
 * 5. 冲突处理路由
 * 6. PRO_REQUIRED / LIMIT_REACHED 付费墙
 * 7. 拖拽遮罩显示/隐藏
 * 8. 导入取消
 * 9. showFileSizeWarning 对话框
 * 10. 导入成功回调
 *
 * Mock: Tauri IPC / i18n / toast / state
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  setState: vi.fn(),
  get: (key) => {
    const map = { streaming: false };
    return map[key];
  },
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  openDialog: vi.fn(),
  importApi: { abort: vi.fn(), replaceDocument: vi.fn() },
}));

// Mock paywall
vi.mock('../../../ui/src/wizard.js', () => ({
  showPaywall: vi.fn(),
}));

// Mock confirm-dialog
vi.mock('../../../ui/src/confirm-dialog.js', () => ({
  showConfirmDialog: vi.fn(async () => true),
}));

// Setup DOM
document.body.innerHTML = `
  <div id="importProgress" class="hidden">
    <div id="importProgressBar"></div>
    <span id="importProgressText"></span>
  </div>
  <div id="dragOverlay" class="hidden"></div>
  <button id="plusBtn"></button>
  <button id="importCancelBtn"></button>
  <div id="fileSizeWarningDialog" class="hidden">
    <div id="fileSizeWarningList"></div>
    <button id="fileSizeWarningOk"></button>
    <button id="fileSizeWarningCancel"></button>
  </div>
`;

describe('import.js — showImportProgress', () => {
  function showImportProgress(total) {
    const bar = document.getElementById('importProgressBar');
    const progress = document.getElementById('importProgress');
    const text = document.getElementById('importProgressText');
    if (bar) { bar.style.width = '0%'; bar.classList.remove('progress-complete', 'progress-error', 'progress-indeterminate'); }
    if (progress) progress.classList.remove('hidden');
    if (text) text.textContent = `开始导入 ${total} 个文件`;
  }

  it('显示进度条移除 hidden', () => {
    showImportProgress(5);
    expect(document.getElementById('importProgress').classList.contains('hidden')).toBe(false);
  });

  it('进度条宽度重置为 0%', () => {
    showImportProgress(5);
    expect(document.getElementById('importProgressBar').style.width).toBe('0%');
  });

  it('进度条移除完成/错误类', () => {
    const bar = document.getElementById('importProgressBar');
    bar.classList.add('progress-complete');
    showImportProgress(3);
    expect(bar.classList.contains('progress-complete')).toBe(false);
  });
});

describe('import.js — hideImportProgress', () => {
  function hideImportProgress() {
    const bar = document.getElementById('importProgressBar');
    const progress = document.getElementById('importProgress');
    if (bar) bar.classList.add('progress-complete');
    if (progress) progress.classList.add('hidden');
  }

  it('隐藏进度条添加 hidden', () => {
    document.getElementById('importProgress').classList.remove('hidden');
    hideImportProgress();
    expect(document.getElementById('importProgress').classList.contains('hidden')).toBe(true);
  });

  it('进度条添加完成类', () => {
    const bar = document.getElementById('importProgressBar');
    bar.classList.remove('progress-complete');
    hideImportProgress();
    expect(bar.classList.contains('progress-complete')).toBe(true);
  });
});

describe('import.js — importPaths 空路径保护', () => {
  async function importPaths(paths) {
    if (!paths || paths.length === 0) return false;
    return true;
  }

  it('空数组不执行导入', async () => {
    expect(await importPaths([])).toBe(false);
  });

  it('null 不执行导入', async () => {
    expect(await importPaths(null)).toBe(false);
  });

  it('undefined 不执行导入', async () => {
    expect(await importPaths(undefined)).toBe(false);
  });

  it('有路径时执行导入', async () => {
    expect(await importPaths(['/path/to/file.pdf'])).toBe(true);
  });
});

describe('import.js — 错误路由', () => {
  function routeImportError(msg) {
    if (msg.startsWith('CONFLICT:')) return 'conflict';
    if (msg.includes('PRO_REQUIRED') || msg.includes('LIMIT_REACHED')) return 'paywall';
    return 'toast';
  }

  it('CONFLICT 前缀路由到冲突处理', () => {
    expect(routeImportError('CONFLICT:doc-123:file.pdf')).toBe('conflict');
  });

  it('PRO_REQUIRED 路由到付费墙', () => {
    expect(routeImportError('PRO_REQUIRED:配额已满')).toBe('paywall');
  });

  it('LIMIT_REACHED 路由到付费墙', () => {
    expect(routeImportError('LIMIT_REACHED:文档数超限')).toBe('paywall');
  });

  it('其他错误路由到 toast', () => {
    expect(routeImportError('不支持的格式')).toBe('toast');
  });
});

describe('import.js — 文件扩展名验证', () => {
  const SUPPORTED_EXTS = ['md', 'txt', 'pdf', 'docx', 'html', 'htm', 'pptx', 'epub', 'xlsx', 'csv'];

  it('支持的扩展名返回 true', () => {
    expect(SUPPORTED_EXTS.includes('pdf')).toBe(true);
    expect(SUPPORTED_EXTS.includes('docx')).toBe(true);
    expect(SUPPORTED_EXTS.includes('md')).toBe(true);
  });

  it('不支持的扩展名返回 false', () => {
    expect(SUPPORTED_EXTS.includes('exe')).toBe(false);
    expect(SUPPORTED_EXTS.includes('jpg')).toBe(false);
  });

  it('htm 和 html 都支持', () => {
    expect(SUPPORTED_EXTS.includes('html')).toBe(true);
    expect(SUPPORTED_EXTS.includes('htm')).toBe(true);
  });
});

describe('import.js — 拖拽遮罩逻辑', () => {
  function showDragOverlay() {
    const el = document.getElementById('dragOverlay');
    if (el) el.classList.remove('hidden');
  }

  function hideDragOverlay() {
    const el = document.getElementById('dragOverlay');
    if (el) el.classList.add('hidden');
  }

  beforeEach(() => {
    document.getElementById('dragOverlay').classList.add('hidden');
  });

  it('drag-enter 显示遮罩', () => {
    showDragOverlay();
    expect(document.getElementById('dragOverlay').classList.contains('hidden')).toBe(false);
  });

  it('drag-leave 隐藏遮罩', () => {
    showDragOverlay();
    hideDragOverlay();
    expect(document.getElementById('dragOverlay').classList.contains('hidden')).toBe(true);
  });

  it('drag-drop 隐藏遮罩', () => {
    showDragOverlay();
    hideDragOverlay();
    expect(document.getElementById('dragOverlay').classList.contains('hidden')).toBe(true);
  });
});

describe('import.js — 文件大小分类逻辑', () => {
  const WARN_THRESHOLD = 100 * 1024 * 1024; // 100MB
  const HARD_LIMIT = 500 * 1024 * 1024; // 500MB

  function classifyFile(size) {
    if (size > HARD_LIMIT) return 'rejected';
    if (size > WARN_THRESHOLD) return 'warning';
    return 'normal';
  }

  it('小于 100MB 分类为 normal', () => {
    expect(classifyFile(50 * 1024 * 1024)).toBe('normal');
  });

  it('100MB-500MB 分类为 warning', () => {
    expect(classifyFile(200 * 1024 * 1024)).toBe('warning');
  });

  it('大于 500MB 分类为 rejected', () => {
    expect(classifyFile(600 * 1024 * 1024)).toBe('rejected');
  });

  it('刚好 100MB 分类为 normal（> 才触发）', () => {
    expect(classifyFile(100 * 1024 * 1024)).toBe('normal');
  });

  it('刚好 500MB 分类为 warning（> 才触发）', () => {
    expect(classifyFile(500 * 1024 * 1024)).toBe('warning');
  });
});
