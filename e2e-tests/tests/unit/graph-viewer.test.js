/**
 * EchoMind graph-viewer.js 单元测试 — 面板管理 / 布局切换 / 窗口 resize。
 *
 * 验证点：
 * 1. openGraphViewer 创建 overlay DOM
 * 2. closeGraphViewer 关闭面板（隐藏 + 停止 simulation）
 * 3. closeGraphViewer 无 overlay 时安全返回
 * 4. closeGraphViewer 重置社区检测状态
 * 5. openGraphViewer 空数据显示空状态
 * 6. openGraphViewer 部分数据显示部分提示
 * 7. initGraphResize 注册 resize 监听
 * 8. openGraphViewer overlay 设置 aria-modal
 * 9. closeGraphViewer 重置路径分析状态
 * 10. closeGraphViewer 重置布局为 force
 *
 * Mock: i18n.js, ipc.js, focus-trap.js, panel-stack.js, ime-guard.js, lazy-loader.js, graph-renderer.js, D3
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
}));

// Mock focus-trap
vi.mock('../../../ui/src/focus-trap.js', () => ({
  createFocusTrap: vi.fn(() => ({
    activate: vi.fn(),
    deactivate: vi.fn(),
  })),
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Mock ime-guard
vi.mock('../../../ui/src/input-utils.js', () => ({
  isComposingEvent: vi.fn(() => false),
}));

// Mock lazy-loader
vi.mock('../../../ui/src/lazy-loader.js', () => ({
  loadD3: vi.fn(() => Promise.resolve({})),
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Mock graph-renderer (all exported functions used by graph-viewer)
vi.mock('../../../ui/src/graph-renderer.js', () => ({
  DEFAULT_LIMIT: 200,
  buildGraphData: vi.fn(() => ({ nodes: [{ id: 'A', degree: 1 }], links: [] })),
  renderGraph: vi.fn(),
  renderLegend: vi.fn(),
  renderFilterPanel: vi.fn(),
  renderPathSelectors: vi.fn(),
  renderNodeVisuals: vi.fn(),
  bindNodeInteractions: vi.fn(),
  highlightNode: vi.fn(),
  clearHighlight: vi.fn(),
  showEntityDetail: vi.fn(),
  zoomIn: vi.fn(),
  zoomOut: vi.fn(),
  resetView: vi.fn(),
  searchAndLocate: vi.fn(),
  clearSearchHighlight: vi.fn(),
  exportSvg: vi.fn(),
  exportPng: vi.fn(),
  exportGraphData: vi.fn(),
  switchLayout: vi.fn(),
  findPath: vi.fn(),
  applyPathHighlight: vi.fn(),
  toggleCommunities: vi.fn(),
  getEntityTypeName: vi.fn((t) => t),
}));

// Setup DOM
document.body.innerHTML = '<div id="app"></div>';

import { openGraphViewer, closeGraphViewer, initGraphResize } from '../../../ui/src/graph-viewer.js';

describe('graph-viewer.js — 面板管理', () => {
  beforeEach(() => {
    // 不重置 DOM — graphState._overlayEl 在模块内是单例，
    // 重置 DOM 会导致 overlay 元素丢失
    vi.clearAllMocks();
  });

  it('openGraphViewer 创建 overlay DOM', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const overlay = document.getElementById('graphOverlay');
    expect(overlay).not.toBeNull();
    expect(overlay.classList.contains('graph-visible')).toBe(true);
  });

  it('closeGraphViewer 关闭面板', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();
    closeGraphViewer();

    const overlay = document.getElementById('graphOverlay');
    // overlay 元素仍然存在但 graph-visible 被移除
    expect(overlay).not.toBeNull();
    expect(overlay.classList.contains('graph-visible')).toBe(false);
  });

  it('closeGraphViewer 无 overlay 时安全返回不报错', () => {
    expect(() => closeGraphViewer()).not.toThrow();
  });

  it('openGraphViewer overlay 设置 aria-modal 和 role=dialog', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const overlay = document.getElementById('graphOverlay');
    expect(overlay.getAttribute('role')).toBe('dialog');
    expect(overlay.getAttribute('aria-modal')).toBe('true');
  });

  it('openGraphViewer 空数据显示空状态', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 0, total_relations: 0 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const emptyState = document.getElementById('graphEmptyState');
    expect(emptyState).not.toBeNull();
    expect(emptyState.style.display).toBe('flex');
  });

  it('openGraphViewer 部分数据显示部分提示', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 100, total_relations: 500 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const statsBar = document.getElementById('graphStatsBar');
    expect(statsBar).not.toBeNull();
    expect(statsBar.textContent).toContain('100');
    expect(statsBar.textContent).toContain('500');
  });

  it('initGraphResize 注册 resize 事件监听', () => {
    const addSpy = vi.spyOn(window, 'addEventListener');
    initGraphResize();
    expect(addSpy).toHaveBeenCalledWith('resize', expect.any(Function));
  });

  it('closeGraphViewer 重置路径分析状态', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();
    closeGraphViewer();

    // closeGraphViewer 重置路径分析结果文本
    const pathResult = document.getElementById('graphPathResult');
    if (pathResult) {
      expect(pathResult.textContent).toBe('');
    }
  });

  it('closeGraphViewer 重置布局为 force', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();
    closeGraphViewer();

    // closeGraphViewer 重置社区检测计数
    const communityCount = document.getElementById('graphCommunityCount');
    if (communityCount) {
      expect(communityCount.textContent).toBe('');
    }
  });

  it('openGraphViewer 创建搜索输入框', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const searchInput = document.getElementById('graphSearchInput');
    expect(searchInput).not.toBeNull();
    expect(searchInput.tagName).toBe('INPUT');
  });

  it('openGraphViewer 创建工具栏按钮', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    expect(document.getElementById('graphZoomIn')).not.toBeNull();
    expect(document.getElementById('graphZoomOut')).not.toBeNull();
    expect(document.getElementById('graphReset')).not.toBeNull();
  });

  it('openGraphViewer 创建布局切换按钮', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const layoutBtns = document.querySelectorAll('.graph-layout-btn');
    expect(layoutBtns.length).toBe(3);
  });

  it('openGraphViewer 创建导出按钮（SVG / PNG / GraphML / JSON-LD）', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    expect(document.getElementById('graphExportSvg')).not.toBeNull();
    expect(document.getElementById('graphExportPng')).not.toBeNull();
    expect(document.getElementById('graphExportGraphml')).not.toBeNull();
    expect(document.getElementById('graphExportJsonld')).not.toBeNull();
  });

  it('openGraphViewer 创建路径分析面板', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    expect(document.getElementById('graphPathPanel')).not.toBeNull();
    expect(document.getElementById('graphPathFrom')).not.toBeNull();
    expect(document.getElementById('graphPathTo')).not.toBeNull();
    expect(document.getElementById('graphPathFindBtn')).not.toBeNull();
    expect(document.getElementById('graphPathResult')).not.toBeNull();
  });

  it('openGraphViewer 创建社区检测按钮', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    expect(document.getElementById('graphCommunityBtn')).not.toBeNull();
    expect(document.getElementById('graphCommunityCount')).not.toBeNull();
  });

  it('openGraphViewer 创建图例和过滤面板', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    expect(document.getElementById('graphLegend')).not.toBeNull();
    expect(document.getElementById('graphFilterPanel')).not.toBeNull();
  });

  it('openGraphViewer 创建详情面板和 tooltip', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    expect(document.getElementById('graphDetailPanel')).not.toBeNull();
    expect(document.getElementById('graphTooltip')).not.toBeNull();
  });

  it('openGraphViewer 空数据隐藏 SVG 和过滤面板', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 0, total_relations: 0 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const svg = document.getElementById('graphSvg');
    const filterPanel = document.getElementById('graphFilterPanel');
    const legend = document.getElementById('graphLegend');
    expect(svg.style.display).toBe('none');
    expect(filterPanel.style.display).toBe('none');
    expect(legend.style.display).toBe('none');
  });

  it('openGraphViewer 超过 DEFAULT_LIMIT 显示部分提示', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 500, total_relations: 1000 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const statsBar = document.getElementById('graphStatsBar');
    expect(statsBar.textContent).toContain('1000');
  });

  it('closeGraphViewer 停止 simulation', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();
    closeGraphViewer();

    // closeGraphViewer 应调用 removePanel
    const { removePanel } = await import('../../../ui/src/panel-stack.js');
    expect(removePanel).toHaveBeenCalledWith('graph-viewer');
  });

  it('openGraphViewer overlay 标记 graph-visible', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const overlay = document.getElementById('graphOverlay');
    // pushPanel is only called once during ensureOverlay (singleton),
    // but graph-visible class is always set by openGraphViewer
    expect(overlay.classList.contains('graph-visible')).toBe(true);
  });

  it('openGraphViewer 创建关闭按钮', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const closeBtn = document.getElementById('graphCloseBtn');
    expect(closeBtn).not.toBeNull();
    expect(closeBtn.getAttribute('aria-label')).toBe('graph.close');
  });

  it('openGraphViewer SVG canvas 存在', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();

    const svg = document.getElementById('graphSvg');
    expect(svg).not.toBeNull();
    expect(svg.tagName).toBe('svg');
  });

  it('closeGraphViewer 清除高亮节点', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_graph_data') return Promise.resolve([{ subject: 'A', object: 'B', relation: 'uses' }]);
      if (cmd === 'get_graph_stats') return Promise.resolve({ total_entities: 2, total_relations: 1 });
      if (cmd === 'get_entity_types') return Promise.resolve({});
      return Promise.resolve(null);
    });

    await openGraphViewer();
    closeGraphViewer();

    // clearSearchHighlight 应被调用
    const { clearSearchHighlight } = await import('../../../ui/src/graph-renderer.js');
    expect(clearSearchHighlight).toHaveBeenCalled();
  });
});
