/**
 * EchoMind embed-eval.js 单元测试 — 嵌入模型对比评估面板 (REQ-VEC-018)。
 *
 * 验证点：
 * 1. openEmbedEvalPanel 创建并显示面板 overlay
 * 2. closeEmbedEvalPanel 移除面板 overlay
 * 3. 面板包含模型选择 checkbox 列表（6 个模型）
 * 4. 面板包含数据集选择（内置/自定义）
 * 5. 面板包含 Top-K 输入框
 * 6. 面板包含开始评估按钮
 * 7. startEvaluation 验证至少选 2 个模型
 * 8. startEvaluation 调用 invoke('run_embed_comparison')
 * 9. renderResults 渲染表格（模型名/维度/Hit Rate/MRR/NDCG）
 * 10. renderResults 渲染 SVG 柱状图
 * 11. scoreColor 分数颜色映射正确
 * 12. 进度区域初始隐藏
 *
 * Mock: utils.js, ipc.js, i18n.js, toast.js, panel-stack.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn((cmd) => {
    if (cmd === 'get_kb_stats') {
      return Promise.resolve({ total_chunks: 42, total_documents: 5 });
    }
    if (cmd === 'run_embed_comparison') {
      return Promise.resolve([
        { model_name: 'model-a', dim: 384, metrics: { hit_rate: 0.8, mrr: 0.6, ndcg: 0.7 }, sample_count: 5 },
        { model_name: 'model-b', dim: 512, metrics: { hit_rate: 0.6, mrr: 0.4, ndcg: 0.5 }, sample_count: 5 },
      ]);
    }
    return Promise.resolve(null);
  }),
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, params) => {
    if (typeof params === 'object' && params !== null) return key + JSON.stringify(params);
    return key;
  },
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = '<div id="root"></div>';
}

describe('embed-eval.js — 嵌入模型对比评估面板', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
    vi.resetModules();
  });

  // TC-VEC-EVAL-UI-001: openEmbedEvalPanel 创建面板 overlay
  it('TC-VEC-EVAL-UI-001: openEmbedEvalPanel 创建并显示面板 overlay', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const overlay = document.getElementById('embedEvalOverlay');
    expect(overlay).toBeTruthy();
    expect(overlay.classList.contains('fixed')).toBe(true);
  });

  // TC-VEC-EVAL-UI-002: closeEmbedEvalPanel 移除面板
  it('TC-VEC-EVAL-UI-002: closeEmbedEvalPanel 移除面板 overlay', async () => {
    const { openEmbedEvalPanel, closeEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();
    expect(document.getElementById('embedEvalOverlay')).toBeTruthy();

    closeEmbedEvalPanel();
    expect(document.getElementById('embedEvalOverlay')).toBeNull();
  });

  // TC-VEC-EVAL-UI-003: 面板包含 6 个模型 checkbox
  it('TC-VEC-EVAL-UI-003: 面板包含 6 个模型 checkbox', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const checkboxes = document.querySelectorAll('.embed-model-cb');
    expect(checkboxes.length).toBe(6);
  });

  // TC-VEC-EVAL-UI-004: 面板包含数据集选择（内置/自定义 radio）
  it('TC-VEC-EVAL-UI-004: 面板包含数据集选择 radio', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const radios = document.querySelectorAll('input[name="embedDataset"]');
    expect(radios.length).toBe(2);
    expect(radios[0].value).toBe('builtin');
    expect(radios[1].value).toBe('custom');
  });

  // TC-VEC-EVAL-UI-005: 面板包含 Top-K 输入框
  it('TC-VEC-EVAL-UI-005: 面板包含 Top-K 输入框', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const topKInput = document.getElementById('embedEvalTopK');
    expect(topKInput).toBeTruthy();
    expect(topKInput.type).toBe('number');
    expect(topKInput.value).toBe('5');
  });

  // TC-VEC-EVAL-UI-006: 面板包含开始评估按钮
  it('TC-VEC-EVAL-UI-006: 面板包含开始评估按钮', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const startBtn = document.getElementById('embedEvalStart');
    expect(startBtn).toBeTruthy();
    expect(startBtn.tagName).toBe('BUTTON');
  });

  // TC-VEC-EVAL-UI-007: 选少于 2 个模型时 toast error
  it('TC-VEC-EVAL-UI-007: 选少于 2 个模型时显示错误 toast', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    const { toast } = await import('../../../ui/src/toast.js');
    await openEmbedEvalPanel();

    // 不勾选任何模型，直接点击开始
    const startBtn = document.getElementById('embedEvalStart');
    startBtn.click();

    // 等待微任务
    await new Promise(r => setTimeout(r, 10));

    expect(toast).toHaveBeenCalledWith(expect.any(String), 'error');
  });

  // TC-VEC-EVAL-UI-008: 选 2 个模型后调用 invoke
  it('TC-VEC-EVAL-UI-008: 选 2 个模型后调用 run_embed_comparison', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    const { invoke } = await import('../../../ui/src/ipc.js');
    await openEmbedEvalPanel();

    // 勾选前 2 个模型
    const checkboxes = document.querySelectorAll('.embed-model-cb');
    checkboxes[0].checked = true;
    checkboxes[1].checked = true;

    const startBtn = document.getElementById('embedEvalStart');
    startBtn.click();

    // 等待异步操作完成
    await new Promise(r => setTimeout(r, 50));

    expect(invoke).toHaveBeenCalledWith('run_embed_comparison', expect.objectContaining({
      request: expect.objectContaining({
        model_names: expect.arrayContaining(['all-MiniLM-L6-v2']),
        top_k: 5,
      }),
    }));
  });

  // TC-VEC-EVAL-UI-009: 评估完成后渲染结果表格
  it('TC-VEC-EVAL-UI-009: 评估完成后渲染结果表格', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    // 勾选前 2 个模型
    const checkboxes = document.querySelectorAll('.embed-model-cb');
    checkboxes[0].checked = true;
    checkboxes[1].checked = true;

    const startBtn = document.getElementById('embedEvalStart');
    startBtn.click();

    // 等待异步操作完成
    await new Promise(r => setTimeout(r, 100));

    const resultsEl = document.getElementById('embedEvalResults');
    expect(resultsEl.classList.contains('hidden')).toBe(false);
    // 验证表格存在
    const table = resultsEl.querySelector('table');
    expect(table).toBeTruthy();
    // 验证表头包含 Hit Rate / MRR / NDCG
    const headers = Array.from(table.querySelectorAll('th')).map(th => th.textContent);
    expect(headers.some(h => h.includes('Hit Rate'))).toBe(true);
    expect(headers.some(h => h.includes('MRR'))).toBe(true);
    expect(headers.some(h => h.includes('NDCG'))).toBe(true);
  });

  // TC-VEC-EVAL-UI-010: 评估完成后渲染 SVG 柱状图
  it('TC-VEC-EVAL-UI-010: 评估完成后渲染 SVG 柱状图', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    // 勾选前 2 个模型
    const checkboxes = document.querySelectorAll('.embed-model-cb');
    checkboxes[0].checked = true;
    checkboxes[1].checked = true;

    const startBtn = document.getElementById('embedEvalStart');
    startBtn.click();

    await new Promise(r => setTimeout(r, 100));

    const resultsEl = document.getElementById('embedEvalResults');
    const svg = resultsEl.querySelector('svg');
    expect(svg).toBeTruthy();
    // 验证有 rect 柱子
    const rects = svg.querySelectorAll('rect');
    expect(rects.length).toBeGreaterThan(0);
  });

  // TC-VEC-EVAL-UI-011: 进度区域初始隐藏
  it('TC-VEC-EVAL-UI-011: 进度区域初始隐藏', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const progressEl = document.getElementById('embedEvalProgress');
    expect(progressEl.classList.contains('hidden')).toBe(true);
  });

  // TC-VEC-EVAL-UI-012: 面板关闭按钮可关闭面板
  it('TC-VEC-EVAL-UI-012: 面板关闭按钮可关闭面板', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const closeBtn = document.getElementById('embedEvalClose');
    expect(closeBtn).toBeTruthy();
    closeBtn.click();

    expect(document.getElementById('embedEvalOverlay')).toBeNull();
  });

  // TC-VEC-EVAL-UI-013: 自定义数据集 radio 切换显示 textarea
  it('TC-VEC-EVAL-UI-013: 自定义数据集 radio 切换显示 textarea', async () => {
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    const textarea = document.getElementById('embedEvalDatasetJson');
    expect(textarea.classList.contains('hidden')).toBe(true);

    // 选择自定义
    const customRadio = document.querySelector('input[name="embedDataset"][value="custom"]');
    customRadio.checked = true;
    customRadio.dispatchEvent(new Event('change'));

    expect(textarea.classList.contains('hidden')).toBe(false);
  });

  // TC-VEC-EVAL-UI-014: openEmbedEvalPanel 调用 pushPanel
  it('TC-VEC-EVAL-UI-014: openEmbedEvalPanel 调用 pushPanel', async () => {
    const { pushPanel } = await import('../../../ui/src/panel-stack.js');
    const { openEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();

    expect(pushPanel).toHaveBeenCalledWith(expect.objectContaining({
      id: 'embed-eval-panel',
      close: expect.any(Function),
      element: expect.any(HTMLElement),
    }));
  });

  // TC-VEC-EVAL-UI-015: closeEmbedEvalPanel 调用 removePanel
  it('TC-VEC-EVAL-UI-015: closeEmbedEvalPanel 调用 removePanel', async () => {
    const { removePanel } = await import('../../../ui/src/panel-stack.js');
    const { openEmbedEvalPanel, closeEmbedEvalPanel } = await import('../../../ui/src/embed-eval.js');
    await openEmbedEvalPanel();
    closeEmbedEvalPanel();

    expect(removePanel).toHaveBeenCalledWith('embed-eval-panel');
  });
});
