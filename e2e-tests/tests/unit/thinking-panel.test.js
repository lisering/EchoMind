/**
 * TC-DS-005~009: thinking-panel.js 单元测试
 *
 * 验证思维链折叠面板的创建、切换、更新、折叠、完成状态。
 */
import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n before importing thinking-panel
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => {
    const map = {
      'chat.thinking_preparing': '正在准备…',
      'chat.thinking_complete': '思考完成',
    };
    return map[key] || key;
  },
}));

// Mock markdown.js（renderReasoning/finalizeReasoning 依赖 renderMarkdown，
// jsdom 环境无 marked/DOMPurify 全局，直接 mock 渲染函数；
// 实现写入 dataset.rawMarkdown 以匹配真实 renderMarkdown 的幂等标记行为）
const { renderMarkdown } = vi.hoisted(() => ({
  renderMarkdown: vi.fn((el, raw) => { el.dataset.rawMarkdown = raw; }),
}));
vi.mock('../../../ui/src/markdown.js', () => ({
  renderMarkdown,
}));

const { createThinkingPanel } = await import('../../../ui/src/thinking-panel.js');

describe('thinking-panel.js — 思维链折叠面板', () => {
  let panel;

  beforeEach(() => {
    renderMarkdown.mockClear();
    panel = createThinkingPanel('正在检索知识库…');
  });

  it('TC-DS-005: createThinkingPanel 返回含 header + content 的容器', () => {
    expect(panel.container).toBeDefined();
    expect(panel.container.className).toContain('thinking-panel');
    const header = panel.container.querySelector('.thinking-panel-header');
    const content = panel.container.querySelector('.thinking-panel-content');
    expect(header).not.toBeNull();
    expect(content).not.toBeNull();
  });

  it('TC-DS-006: 点击 header 切换 content 的 hidden 类', () => {
    const header = panel.container.querySelector('.thinking-panel-header');
    const content = panel.container.querySelector('.thinking-panel-content');
    // 初始折叠
    expect(content.classList.contains('hidden')).toBe(true);
    // 点击展开
    header.click();
    expect(content.classList.contains('hidden')).toBe(false);
    // 再次点击折叠
    header.click();
    expect(content.classList.contains('hidden')).toBe(true);
  });

  it('TC-DS-007: update() 更新 header 文本内容', () => {
    panel.update('正在生成回答…');
    const textEl = panel.container.querySelector('.thinking-panel-text');
    expect(textEl.textContent).toBe('正在生成回答…');
    // 展开内容由 appendStage / appendReasoning 管理，update() 不触碰（保持阶段文本纯净）
    const contentEl = panel.container.querySelector('.thinking-panel-content');
    expect(contentEl.textContent).toBe('');
  });

  it('TC-DS-008: collapse() 隐藏内容并重置 chevron', () => {
    const content = panel.container.querySelector('.thinking-panel-content');
    // 先展开
    panel.expand();
    expect(content.classList.contains('hidden')).toBe(false);
    // 折叠
    panel.collapse();
    expect(content.classList.contains('hidden')).toBe(true);
    const chevron = panel.container.querySelector('.thinking-panel-chevron');
    expect(chevron.style.transform).toBe('');
  });

  it('TC-DS-009: setComplete() 更新 header 文本', async () => {
    await panel.setComplete();
    const textEl = panel.container.querySelector('.thinking-panel-text');
    expect(textEl.textContent).toBe('思考完成');
  });

  it('TC-DS-009b: expand() 展开内容并旋转 chevron', () => {
    const content = panel.container.querySelector('.thinking-panel-content');
    const chevron = panel.container.querySelector('.thinking-panel-chevron');
    expect(content.classList.contains('hidden')).toBe(true);
    panel.expand();
    expect(content.classList.contains('hidden')).toBe(false);
    expect(chevron.style.transform).toBe('rotate(180deg)');
  });

  it('TC-DS-009c: 默认初始文本使用 thinking_preparing', () => {
    const p = createThinkingPanel();
    const textEl = p.container.querySelector('.thinking-panel-text');
    expect(textEl.textContent).toBe('正在准备…');
  });

  // ----------------------------------------------------------
  // 需求 5：思考生动动画（D 组合 = 图标流转 + 图标旋转动画）
  // ----------------------------------------------------------

  it('TC-DS-010: 初始状态无打字点动画（已改用图标旋转动画）', () => {
    const p = createThinkingPanel();
    const dots = p.container.querySelectorAll('.thinking-typing-dot');
    expect(dots.length).toBe(0);
  });

  it('TC-DS-010b: update 传 phase 切换阶段图标（图标流转）+ 添加旋转动画', () => {
    const p = createThinkingPanel();
    const iconEl = p.container.querySelector('.thinking-stage-icon');
    const iconBefore = iconEl.innerHTML;
    // 切到 retrieving 阶段 → 图标应变化（放大镜）+ 旋转动画类
    p.update('正在检索知识库…', 'retrieving');
    expect(iconEl.innerHTML).not.toBe(iconBefore);
    expect(iconEl.classList.contains('thinking-icon-active')).toBe(true);
    // 切到 generating 阶段 → 图标再次变化（星星）
    p.update('正在生成回答…', 'generating');
    expect(iconEl.innerHTML).not.toBe(iconBefore);
  });

  it('TC-DS-010c: setComplete 移除图标旋转动画', async () => {
    const p = createThinkingPanel();
    p.update('正在检索…', 'retrieving');
    const iconEl = p.container.querySelector('.thinking-stage-icon');
    expect(iconEl.classList.contains('thinking-icon-active')).toBe(true);
    await p.setComplete();
    expect(iconEl.classList.contains('thinking-icon-active')).toBe(false);
  });

  it('TC-DS-010d: reset 恢复灯泡图标与旋转动画', async () => {
    const p = createThinkingPanel();
    await p.setComplete();
    p.update('正在生成…', 'generating');
    // reset 后：图标回灯泡 + 旋转动画恢复
    p.reset();
    const iconEl = p.container.querySelector('.thinking-stage-icon');
    expect(iconEl.classList.contains('thinking-icon-active')).toBe(true);
    const textEl = p.container.querySelector('.thinking-panel-text');
    expect(textEl.textContent).toBe('正在准备…');
  });

  it('TC-DS-010e: appendReasoning 流式累加纯文本（不触发 markdown 渲染）', () => {
    const p = createThinkingPanel();
    p.appendReasoning('第一步思考');
    p.appendReasoning('，第二步思考');
    const reasoningEl = p.container.querySelector('.thinking-reasoning');
    expect(reasoningEl).not.toBeNull();
    expect(reasoningEl.textContent).toBe('第一步思考，第二步思考');
    // 流式期间不调用 renderMarkdown
    expect(renderMarkdown).not.toHaveBeenCalled();
  });

  it('TC-DS-010f: finalizeReasoning 把累加文本做一次性 markdown 渲染且幂等', () => {
    const p = createThinkingPanel();
    p.appendReasoning('**加粗** 的思考内容');
    p.finalizeReasoning();
    expect(renderMarkdown).toHaveBeenCalledTimes(1);
    const reasoningEl = p.container.querySelector('.thinking-reasoning');
    expect(reasoningEl.dataset.rawMarkdown).toBeTruthy();
    // 幂等：再次调用不重复渲染
    p.finalizeReasoning();
    expect(renderMarkdown).toHaveBeenCalledTimes(1);
  });

  it('TC-DS-010g: renderReasoning 直接渲染完整内容（版本切换/历史加载场景）', () => {
    const p = createThinkingPanel();
    p.renderReasoning('版本 1 的思考过程');
    const reasoningEl = p.container.querySelector('.thinking-reasoning');
    expect(reasoningEl).not.toBeNull();
    expect(renderMarkdown).toHaveBeenCalledWith(reasoningEl, '版本 1 的思考过程', null, true);
  });
});
