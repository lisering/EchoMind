/**
 * EchoMind graph-renderer.js 单元测试 — D3 渲染 / 缩放 / 高亮 / 导出。
 *
 * 验证点：
 * 1. RELATION_COLORS 颜色映射（8 种关系类型 + 默认）
 * 2. getRelationColor 已知类型 + 未知类型
 * 3. getEntityIcon 已知实体 + 未知实体
 * 4. buildGraphData 三元组 → 节点+边
 * 5. buildGraphData 去重边
 * 6. buildGraphData 节点 degree 累计
 * 7. getUniqueRelationTypes 去重提取
 * 8. isEdgeVisible 空集合 + 已选集合
 * 9. COMMUNITY_COLORS 长度 + 首尾值
 * 10. DEFAULT_LIMIT 常量
 * 11. highlightNode 设置高亮节点
 * 12. clearHighlight 清除高亮
 * 13. zoomIn 无 ctx._svg 时安全返回
 * 14. exportGraphData 调用 invoke
 * 15. exportGraphData 无内容时安全返回
 *
 * Mock: i18n.js, ipc.js, D3（全局）
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
  graphApi: { getLayout: vi.fn() },
}));

// Setup D3 mock — chainable proxy that returns self for any method call
const _chainable = new Proxy({}, { get: () => vi.fn(() => _chainable) });
globalThis.d3 = _chainable;

// Setup DOM
document.body.innerHTML = '<div id="graphCanvasContainer" style="width:800px;height:600px;"><svg id="graphSvg"></svg><div id="graphLegend"></div><div id="graphFilterPanel"></div><div id="graphTooltip"></div><div id="graphDetailPanel"></div></div>';

import {
  RELATION_COLORS,
  DEFAULT_NODE_COLOR,
  DEFAULT_LIMIT,
  COMMUNITY_COLORS,
  FORCE_CONFIG,
  getRelationColor,
  getEntityIcon,
  buildGraphData,
  getUniqueRelationTypes,
  isEdgeVisible,
  highlightNode,
  clearHighlight,
  zoomIn,
  zoomOut,
  resetView,
  exportGraphData,
} from '../../../ui/src/graph-renderer.js';

describe('graph-renderer.js — 常量定义', () => {
  it('RELATION_COLORS 应包含 8 种关系类型颜色', () => {
    expect(RELATION_COLORS.defined_as).toBe('#38bdf8');
    expect(RELATION_COLORS.part_of).toBe('#a78bfa');
    expect(RELATION_COLORS.depends_on).toBe('#fb923c');
    expect(RELATION_COLORS.uses).toBe('#4ade80');
    expect(RELATION_COLORS.implements).toBe('#f472b6');
    expect(RELATION_COLORS.extends).toBe('#facc15');
    expect(RELATION_COLORS.references).toBe('#22d3ee');
    expect(RELATION_COLORS.related_to).toBe('#94a3b8');
  });

  it('DEFAULT_NODE_COLOR 应为灰色', () => {
    expect(DEFAULT_NODE_COLOR).toBe('#64748b');
  });

  it('DEFAULT_LIMIT 应为 200', () => {
    expect(DEFAULT_LIMIT).toBe(200);
  });

  it('COMMUNITY_COLORS 应有 10 种颜色', () => {
    expect(COMMUNITY_COLORS).toHaveLength(10);
    expect(COMMUNITY_COLORS[0]).toBe('#38bdf8');
    expect(COMMUNITY_COLORS[9]).toBe('#34d399');
  });

  it('FORCE_CONFIG 应包含合理的力导向参数', () => {
    expect(FORCE_CONFIG.charge_strength).toBe(-300);
    expect(FORCE_CONFIG.link_distance).toBe(80);
    expect(FORCE_CONFIG.collision_radius).toBe(20);
  });
});

describe('graph-renderer.js — 工具函数', () => {
  it('getRelationColor 已知类型返回对应颜色', () => {
    expect(getRelationColor('defined_as')).toBe('#38bdf8');
    expect(getRelationColor('part_of')).toBe('#a78bfa');
  });

  it('getRelationColor 未知类型返回默认颜色', () => {
    expect(getRelationColor('unknown_relation')).toBe(DEFAULT_NODE_COLOR);
  });

  it('getEntityIcon 已知实体返回 SVG 字符串', () => {
    const personIcon = getEntityIcon('person');
    expect(personIcon).toContain('<circle');
    expect(personIcon).toContain('stroke');
  });

  it('getEntityIcon 未知实体返回空字符串', () => {
    expect(getEntityIcon('unknown_type')).toBe('');
  });

  it('buildGraphData 从三元组构建节点和边', () => {
    const triples = [
      { subject: 'A', object: 'B', relation: 'uses' },
      { subject: 'B', object: 'C', relation: 'part_of' },
    ];
    const result = buildGraphData(triples);
    expect(result.nodes).toHaveLength(3);
    expect(result.links).toHaveLength(2);
    expect(result.links[0].source).toBe('A');
    expect(result.links[0].relation).toBe('uses');
  });

  it('buildGraphData 去重重复的边', () => {
    const triples = [
      { subject: 'A', object: 'B', relation: 'uses' },
      { subject: 'A', object: 'B', relation: 'uses' },
    ];
    const result = buildGraphData(triples);
    expect(result.links).toHaveLength(1);
  });

  it('buildGraphData 累计节点 degree', () => {
    const triples = [
      { subject: 'A', object: 'B', relation: 'uses' },
      { subject: 'A', object: 'C', relation: 'uses' },
    ];
    const result = buildGraphData(triples);
    const nodeA = result.nodes.find(n => n.id === 'A');
    expect(nodeA.degree).toBe(2);
    const nodeB = result.nodes.find(n => n.id === 'B');
    expect(nodeB.degree).toBe(1);
  });

  it('getUniqueRelationTypes 返回去重的关系类型列表', () => {
    const links = [
      { relation: 'uses' },
      { relation: 'part_of' },
      { relation: 'uses' },
    ];
    const types = getUniqueRelationTypes(links);
    expect(types).toHaveLength(2);
    expect(types).toContain('uses');
    expect(types).toContain('part_of');
  });

  it('isEdgeVisible 空集合时全部可见', () => {
    expect(isEdgeVisible('uses', new Set())).toBe(true);
  });

  it('isEdgeVisible 已选集合时仅选中可见', () => {
    expect(isEdgeVisible('uses', new Set(['uses']))).toBe(true);
    expect(isEdgeVisible('part_of', new Set(['uses']))).toBe(false);
  });
});

describe('graph-renderer.js — 高亮与缩放', () => {
  let ctx;

  beforeEach(() => {
    ctx = {
      _graphData: { nodes: [], links: [] },
      _highlightedNode: null,
      _container: {
        selectAll: vi.fn(() => ({
          classed: vi.fn(() => ({})),
        })),
      },
      _svg: null,
      _zoom: null,
      _simulation: null,
    };
  });

  it('highlightNode 设置高亮节点 ID', () => {
    highlightNode(ctx, 'TestEntity');
    expect(ctx._highlightedNode).toBe('TestEntity');
  });

  it('clearHighlight 清除高亮节点', () => {
    ctx._highlightedNode = 'TestEntity';
    // 使用可链式 classed mock
    const classedMock = vi.fn(() => ({ classed: classedMock }));
    ctx._container = { selectAll: vi.fn(() => ({ classed: classedMock })) };
    clearHighlight(ctx);
    expect(ctx._highlightedNode).toBeNull();
  });

  it('zoomIn 无 _svg 时安全返回', () => {
    expect(() => zoomIn(ctx)).not.toThrow();
  });

  it('zoomOut 无 _svg 时安全返回', () => {
    expect(() => zoomOut(ctx)).not.toThrow();
  });

  it('resetView 无 _svg 时安全返回', () => {
    expect(() => resetView(ctx)).not.toThrow();
  });
});

describe('graph-renderer.js — 导出', () => {
  it('exportGraphData 调用 invoke 获取导出内容', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockResolvedValue('<graphml>content</graphml>');
    await exportGraphData('graphml');
    expect(invoke).toHaveBeenCalledWith('export_graph', { format: 'graphml' });
  });

  it('exportGraphData 无内容时安全返回', async () => {
    const { invoke } = await import('../../../ui/src/ipc.js');
    invoke.mockResolvedValue(null);
    await expect(exportGraphData('jsonld')).resolves.toBeUndefined();
  });
});
