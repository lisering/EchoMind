/**
 * EchoMind 知识图谱可视化模块（REQ-RAG-027 前端图谱可视化 + Session 4 增强）。
 *
 * 使用 D3.js v7 force-directed graph 将 EntityRelation 三元组渲染为交互式知识图谱。
 *
 * 功能：
 * 1. 全屏 overlay 面板（类似 security.js lock overlay）
 * 2. D3 forceSimulation 力导向布局（charge / link / center）
 * 3. 节点拖拽 + 滚轮缩放 + 画布平移
 * 4. 点击节点高亮关联边 + 关联节点
 * 5. 双击节点展开实体详细关系列表
 * 6. 关系类型颜色编码（8 种）
 * 7. 空状态提示
 * 8. Focus Trap + ESC 关闭
 * 9. i18n 国际化
 *
 * Session 4 增强：
 * 10. 实体类型图标（person/proper_noun/tech_term/identifier/date 五种内联 SVG）
 * 11. 子图过滤（按关系类型复选框筛选显示子图）
 * 12. 搜索定位（输入实体文本 → 自动居中+缩放+高亮匹配节点）
 * 13. 图谱导出（SVG 序列化 → Blob 下载 / PNG Canvas 转换）
 * 14. 节点详情面板增强（实体类型徽章 + 来源 chunk 预览）
 *
 * 依赖：D3.js v7（ui/vendor/d3.min.js 全局 window.d3）
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { pushPanel, removePanel } from './panel-stack.js';

// ============================================================
// 常量
// ============================================================

/** 关系类型 → 颜色映射（8 种关系类型 + 默认） */
const RELATION_COLORS = {
  defined_as: '#38bdf8',    // 天蓝
  part_of: '#a78bfa',       // 紫色
  depends_on: '#fb923c',    // 橙色
  uses: '#4ade80',          // 绿色
  implements: '#f472b6',    // 粉色
  extends: '#facc15',       // 黄色
  references: '#22d3ee',    // 青色
  related_to: '#94a3b8',    // 灰色
};

/** 默认节点颜色 */
const DEFAULT_NODE_COLOR = '#64748b';

/** 力导向参数 */
const FORCE_CONFIG = {
  charge_strength: -300,
  link_distance: 80,
  center_strength: 0.05,
  collision_radius: 20,
};

/** 默认加载的关系数量 */
const DEFAULT_LIMIT = 200;

/**
 * 实体类型 → SVG 图标 path 数据（5 种类型 + 默认）。
 *
 * 每个图标设计为 16x16 viewBox，居中绘制在节点圆形上方。
 * 使用 currentColor 继承节点 stroke 颜色。
 */
const ENTITY_TYPE_ICONS = {
  // 人形图标（头 + 肩）
  person: '<circle cx="8" cy="5" r="2.5" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M3 14c0-3 2.5-5 5-5s5 2 5 5" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
  // 标签图标（圆角矩形 + 孔）
  proper_noun: '<rect x="2" y="4" width="12" height="8" rx="2" fill="none" stroke="currentColor" stroke-width="1.2"/><circle cx="5" cy="8" r="1" fill="currentColor"/>',
  // 代码括号图标 < >
  tech_term: '<path d="M5 4L2 8L5 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/><path d="M11 4L14 8L11 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>',
  // # 符号图标
  identifier: '<path d="M4 6h8M4 10h8M6 4l-1 8M10 4l-1 8" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
  // 日历图标
  date: '<rect x="2" y="3" width="12" height="11" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M2 6h12" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M5 1.5v3M11 1.5v3" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
};

/** 实体类型 → i18n 键后缀映射 */
const ENTITY_TYPE_I18N_KEYS = {
  person: 'graph.entity_type_person',
  proper_noun: 'graph.entity_type_proper_noun',
  tech_term: 'graph.entity_type_tech_term',
  identifier: 'graph.entity_type_identifier',
  date: 'graph.entity_type_date',
};

// ============================================================
// 模块状态
// ============================================================

/** 图谱 overlay 元素 */
let _overlayEl = null;

/** 图谱 SVG 元素 */
let _svgEl = null;

/** D3 simulation 实例 */
let _simulation = null;

/** D3 zoom behavior 实例 */
let _zoom = null;

/** D3 SVG 选择器 */
let _svg = null;

/** D3 容器 group 选择器（所有图谱元素的父容器） */
let _container = null;

/** D3 link 选择器（边） */
let _linkSel = null;

/** D3 node 选择器（节点） */
let _nodeSel = null;

/** Focus Trap 实例 */
let _trap = null;

/** 当前图谱数据 */
let _graphData = { nodes: [], links: [] };

/** 当前高亮的节点文本 */
let _highlightedNode = null;

/** 详情面板元素 */
let _detailPanelEl = null;

/** 实体文本 → 类型映射（从后端批量查询） */
let _entityTypeMap = {};

/** 当前启用的关系类型过滤集合（空集 = 全部启用） */
let _enabledRelationTypes = new Set();

/** 当前布局模式（force / hierarchical / radial） */
let _currentLayout = 'force';

/** 路径分析：起点实体文本 */
let _pathFromEntity = null;

/** 路径分析：终点实体文本 */
let _pathToEntity = null;

/** 路径高亮：当前高亮的路径节点集合 */
let _pathHighlightNodes = new Set();

/** 社区检测：实体 → 社区 ID 映射 */
let _communityMap = {};

/** 社区检测：是否已启用社区着色 */
let _communityEnabled = false;

/** 社区颜色调色板（10 种颜色） */
const COMMUNITY_COLORS = [
  '#38bdf8', // 天蓝
  '#a78bfa', // 紫色
  '#fb923c', // 橙色
  '#4ade80', // 绿色
  '#f472b6', // 粉色
  '#facc15', // 黄色
  '#22d3ee', // 青色
  '#fb7185', // 红色
  '#818cf8', // 靛蓝
  '#34d399', // 翡翠绿
];

// ============================================================
// 工具函数（纯逻辑，可测试）
// ============================================================

/**
 * 获取关系类型对应的颜色。
 * @param {string} relationType - 关系类型
 * @returns {string} 颜色 hex 值
 */
export function getRelationColor(relationType) {
  return RELATION_COLORS[relationType] || DEFAULT_NODE_COLOR;
}

/**
 * 获取实体类型对应的 SVG 图标 path。
 * @param {string} entityType - 实体类型
 * @returns {string} SVG inner HTML（path/circle 元素）
 */
export function getEntityIcon(entityType) {
  return ENTITY_TYPE_ICONS[entityType] || '';
}

/**
 * 获取实体类型的 i18n 显示名称。
 * @param {string} entityType - 实体类型
 * @returns {string} 本地化类型名称
 */
export function getEntityTypeName(entityType) {
  const key = ENTITY_TYPE_I18N_KEYS[entityType];
  return key ? t(key) : t('graph.entity_type_unknown');
}

/**
 * 从三元组列表构建图谱数据（节点 + 边）。
 *
 * 将 GraphTriple 列表转换为 D3 force-directed graph 所需的 { nodes, links } 格式。
 * 同名实体会被合并为一个节点，节点大小根据连接数（degree）决定。
 *
 * @param {Array<{subject: string, relation: string, object: string}>} triples - 三元组列表
 * @returns {{ nodes: Array<{id: string, degree: number}>, links: Array<{source: string, target: string, relation: string}> }}
 */
export function buildGraphData(triples) {
  const nodeMap = new Map(); // id → { id, degree }
  const links = [];
  const seenLinks = new Set(); // 去重 (subject→object+relation)

  for (const t of triples) {
    // 确保节点存在
    if (!nodeMap.has(t.subject)) {
      nodeMap.set(t.subject, { id: t.subject, degree: 0 });
    }
    if (!nodeMap.has(t.object)) {
      nodeMap.set(t.object, { id: t.object, degree: 0 });
    }

    // 去重边
    const linkKey = `${t.subject}→${t.object}→${t.relation}`;
    if (!seenLinks.has(linkKey)) {
      seenLinks.add(linkKey);
      links.push({
        source: t.subject,
        target: t.object,
        relation: t.relation,
      });
      // 增加 degree
      nodeMap.get(t.subject).degree += 1;
      nodeMap.get(t.object).degree += 1;
    }
  }

  return {
    nodes: Array.from(nodeMap.values()),
    links,
  };
}

/**
 * 获取关系类型列表（去重，用于图例渲染）。
 * @param {Array<{relation: string}>} links - 边列表
 * @returns {string[]} 去重后的关系类型列表
 */
export function getUniqueRelationTypes(links) {
  const types = new Set();
  for (const l of links) {
    types.add(l.relation);
  }
  return Array.from(types);
}

/**
 * 检查边是否应该可见（其关系类型在过滤集合中）。
 * @param {string} relationType - 关系类型
 * @param {Set<string>} enabledTypes - 启用的关系类型集合
 * @returns {boolean} 是否可见
 */
export function isEdgeVisible(relationType, enabledTypes) {
  if (enabledTypes.size === 0) return true; // 空集 = 全部启用
  return enabledTypes.has(relationType);
}

// ============================================================
// 面板创建与管理
// ============================================================

/**
 * 确保 overlay DOM 元素存在（惰性创建）。
 */
function ensureOverlay() {
  if (_overlayEl) return;

  _overlayEl = document.createElement('div');
  _overlayEl.id = 'graphOverlay';
  _overlayEl.className = 'graph-overlay';
  _overlayEl.setAttribute('role', 'dialog');
  _overlayEl.setAttribute('aria-modal', 'true');
  _overlayEl.innerHTML = `
    <div class="graph-header">
      <div class="graph-header-left">
        <h2 class="graph-title">${t('graph.title')}</h2>
        <div class="graph-stats-bar" id="graphStatsBar"></div>
      </div>
      <button class="graph-close-btn" id="graphCloseBtn" data-i18n-title="graph.close" title="${t('graph.close')}" aria-label="${t('graph.close')}">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>
    <div class="graph-search-bar">
      <input type="text" id="graphSearchInput" class="graph-search-input" placeholder="${t('graph.search_placeholder')}" aria-label="${t('graph.search_placeholder')}" />
    </div>
    <div class="graph-canvas-container" id="graphCanvasContainer">
      <svg class="graph-svg" id="graphSvg"></svg>
      <div class="graph-toolbar">
        <button class="graph-tool-btn" id="graphZoomIn" data-i18n-title="graph.zoom_in" title="${t('graph.zoom_in')}" aria-label="${t('graph.zoom_in')}">+</button>
        <button class="graph-tool-btn" id="graphZoomOut" data-i18n-title="graph.zoom_out" title="${t('graph.zoom_out')}" aria-label="${t('graph.zoom_out')}">−</button>
        <button class="graph-tool-btn" id="graphReset" data-i18n-title="graph.reset_view" title="${t('graph.reset_view')}" aria-label="${t('graph.reset_view')}">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
            <path d="M3 3v5h5"/>
          </svg>
        </button>
        <div class="graph-toolbar-divider"></div>
        <button class="graph-tool-btn graph-export-btn" id="graphExportSvg" data-i18n-title="graph.export_svg" title="${t('graph.export_svg')}" aria-label="${t('graph.export_svg')}">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
        </button>
        <button class="graph-tool-btn graph-export-btn" id="graphExportPng" data-i18n-title="graph.export_png" title="${t('graph.export_png')}" aria-label="${t('graph.export_png')}">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2"/>
            <circle cx="8.5" cy="8.5" r="1.5"/>
            <polyline points="21 15 16 10 5 21"/>
          </svg>
        </button>
        <div class="graph-toolbar-divider"></div>
        <button class="graph-tool-btn graph-export-btn" id="graphExportGraphml" data-i18n-title="graph.export_graphml" title="${t('graph.export_graphml')}" aria-label="${t('graph.export_graphml')}">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
            <line x1="8" y1="13" x2="16" y2="13"/>
            <line x1="8" y1="17" x2="16" y2="17"/>
          </svg>
        </button>
        <button class="graph-tool-btn graph-export-btn" id="graphExportJsonld" data-i18n-title="graph.export_jsonld" title="${t('graph.export_jsonld')}" aria-label="${t('graph.export_jsonld')}">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
            <path d="M10 12a2 2 0 1 0 4 0 2 2 0 1 0-4 0"/>
            <path d="M8 18c0-1.5 1-3 4-3s4 1.5 4 3"/>
          </svg>
        </button>
      </div>
      <div class="graph-legend" id="graphLegend"></div>
      <div class="graph-filter-panel" id="graphFilterPanel"></div>
      <div class="graph-layout-panel" id="graphLayoutPanel">
        <div class="graph-layout-title">${t('graph.layout_title')}</div>
        <div class="graph-layout-buttons">
          <button class="graph-layout-btn graph-layout-active" data-layout="force">${t('graph.layout_force')}</button>
          <button class="graph-layout-btn" data-layout="hierarchical">${t('graph.layout_hierarchical')}</button>
          <button class="graph-layout-btn" data-layout="radial">${t('graph.layout_radial')}</button>
        </div>
      </div>
      <div class="graph-path-panel" id="graphPathPanel">
        <div class="graph-path-title">${t('graph.path_title')}</div>
        <div class="graph-path-selectors">
          <select class="graph-path-select" id="graphPathFrom" data-i18n-placeholder="graph.path_from">
            <option value="">${t('graph.path_from')}</option>
          </select>
          <span class="graph-path-arrow">→</span>
          <select class="graph-path-select" id="graphPathTo" data-i18n-placeholder="graph.path_to">
            <option value="">${t('graph.path_to')}</option>
          </select>
        </div>
        <button class="graph-path-find-btn" id="graphPathFindBtn">${t('graph.path_find')}</button>
        <div class="graph-path-result" id="graphPathResult"></div>
      </div>
      <div class="graph-community-panel">
        <button class="graph-community-btn" id="graphCommunityBtn">${t('graph.community_detect')}</button>
        <span class="graph-community-count" id="graphCommunityCount"></span>
      </div>
      <div class="graph-detail-panel" id="graphDetailPanel"></div>
      <div class="graph-tooltip" id="graphTooltip"></div>
      <div class="graph-empty-state" id="graphEmptyState" style="display:none;">
        <div class="empty-icon">🕸️</div>
        <div class="empty-text">${t('graph.empty_hint')}</div>
      </div>
    </div>
  `;
  document.body.appendChild(_overlayEl);

  // 绑定事件
  document.getElementById('graphCloseBtn').addEventListener('click', closeGraphViewer);
  document.getElementById('graphZoomIn').addEventListener('click', zoomIn);
  document.getElementById('graphZoomOut').addEventListener('click', zoomOut);
  document.getElementById('graphReset').addEventListener('click', resetView);
document.getElementById('graphExportSvg').addEventListener('click', exportSvg);
document.getElementById('graphExportPng').addEventListener('click', exportPng);
document.getElementById('graphExportGraphml').addEventListener('click', () => exportGraphData('graphml'));
document.getElementById('graphExportJsonld').addEventListener('click', () => exportGraphData('jsonld'));

  // 布局切换按钮
  const layoutBtns = _overlayEl.querySelectorAll('.graph-layout-btn');
  for (const btn of layoutBtns) {
    btn.addEventListener('click', () => {
      const mode = btn.getAttribute('data-layout');
      switchLayout(mode);
    });
  }

  // 路径分析：选择器 + 按钮
  document.getElementById('graphPathFindBtn').addEventListener('click', findPath);

  // 社区检测按钮
  document.getElementById('graphCommunityBtn').addEventListener('click', toggleCommunities);

  // 搜索框事件
  const searchInput = document.getElementById('graphSearchInput');
  if (searchInput) {
    searchInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        searchAndLocate(searchInput.value);
      }
    });
  }

  // ESC 关闭
  _overlayEl.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeGraphViewer();
    }
  });

  // 创建 Focus Trap
  _trap = createFocusTrap(_overlayEl);

  // 注册到面板栈（ESC 关闭 + 生命周期追踪）
  pushPanel({ id: 'graph-viewer', close: closeGraphViewer, element: _overlayEl, label: 'Graph Viewer' });
}

/**
 * 打开知识图谱面板。
 *
 * 1. 确保 overlay DOM 存在
 * 2. 调用 get_graph_data IPC 获取三元组数据
 * 3. 调用 get_graph_stats IPC 获取统计信息
 * 4. 批量查询实体类型（get_entity_types IPC）
 * 5. 渲染 D3 force-directed graph
 * 6. 渲染图例 + 过滤面板
 * 7. 激活 Focus Trap
 */
export async function openGraphViewer() {
  ensureOverlay();

  // 显示 loading 状态
  const statsBar = document.getElementById('graphStatsBar');
  if (statsBar) statsBar.textContent = t('graph.loading');

  // 显示 overlay
  _overlayEl.classList.add('graph-visible');

  // 激活 Focus Trap
  if (_trap) _trap.activate();

  try {
    // 并行获取图谱数据和统计信息
    const [triples, stats] = await Promise.all([
      invoke('get_graph_data', { limit: DEFAULT_LIMIT }),
      invoke('get_graph_stats'),
    ]);

    // 更新统计栏
    if (statsBar) {
      statsBar.innerHTML = `
        <span class="stat-item"><span class="stat-value">${stats.total_entities}</span> ${t('graph.total_entities')}</span>
        <span class="stat-item"><span class="stat-value">${stats.total_relations}</span> ${t('graph.total_relations')}</span>
      `;
    }

    // 检查空状态
    if (!triples || triples.length === 0) {
      document.getElementById('graphEmptyState').style.display = 'flex';
      document.getElementById('graphSvg').style.display = 'none';
      document.getElementById('graphFilterPanel').style.display = 'none';
      document.getElementById('graphLegend').style.display = 'none';
      return;
    }

    document.getElementById('graphEmptyState').style.display = 'none';
    document.getElementById('graphSvg').style.display = '';

    // 显示「显示前 N 条，共 M 条」提示
    if (stats.total_relations > DEFAULT_LIMIT) {
      const partialHint = document.createElement('span');
      partialHint.className = 'stat-item';
      partialHint.textContent = t('graph.showing_partial', {
        shown: triples.length,
        total: stats.total_relations,
      });
      statsBar.appendChild(partialHint);
    }

    // 构建图谱数据
    _graphData = buildGraphData(triples);

    // 批量查询实体类型
    const entityIds = _graphData.nodes.map((n) => n.id);
    try {
      _entityTypeMap = await invoke('get_entity_types', { entities: entityIds });
    } catch {
      // 查询失败时降级为空映射（不显示图标）
      _entityTypeMap = {};
    }

    // 初始化过滤集合（空集 = 全部启用）
    _enabledRelationTypes = new Set();

    // 渲染图谱
    renderGraph();

    // 渲染图例
    renderLegend();

    // 渲染过滤面板
    renderFilterPanel();

    // 渲染路径分析选择器
    renderPathSelectors();
  } catch (err) {
    if (statsBar) statsBar.textContent = String(err);
  }
}

/**
 * 关闭知识图谱面板。
 */
export function closeGraphViewer() {
  if (!_overlayEl) return;

  // 从面板栈移除
  removePanel('graph-viewer');

  _overlayEl.classList.remove('graph-visible');

  // 停用 Focus Trap
  if (_trap) _trap.deactivate();

  // 停止 simulation
  if (_simulation) {
    _simulation.stop();
    _simulation = null;
  }

  // 清除高亮
  _highlightedNode = null;

  // 隐藏详情面板
  if (_detailPanelEl) {
    _detailPanelEl.classList.remove('graph-detail-visible');
  }

  // 清除搜索高亮
  clearSearchHighlight();

  // 重置路径分析
  _pathFromEntity = null;
  _pathToEntity = null;
  _pathHighlightNodes = new Set();

  // 重置社区着色
  _communityMap = {};
  _communityEnabled = false;
  const communityCount = document.getElementById('graphCommunityCount');
  if (communityCount) communityCount.textContent = '';

  // 重置布局
  _currentLayout = 'force';

  // 清除路径结果显示
  const pathResult = document.getElementById('graphPathResult');
  if (pathResult) pathResult.textContent = '';
}

// ============================================================
// D3 图谱渲染
// ============================================================

/**
 * 渲染 D3 force-directed graph。
 *
 * 在 #graphSvg 内创建：
 * - <g class="graph-container">：所有图谱元素的父容器（用于 zoom/pan 变换）
 *   - <line class="graph-edge">：边
 *   - <g class="graph-node">：节点组
 *     - <circle class="graph-node-circle">：节点圆形
 *     - <g class="graph-node-icon">：实体类型图标（内联 SVG）
 *     - <text class="graph-node-label">：节点标签
 */
function renderGraph() {
  const container = document.getElementById('graphCanvasContainer');
  if (!container) return;

  const width = container.clientWidth;
  const height = container.clientHeight;

  // 清除旧 SVG 内容
  const svgEl = document.getElementById('graphSvg');
  svgEl.innerHTML = '';
  svgEl.setAttribute('viewBox', `0 0 ${width} ${height}`);

  _svg = d3.select(svgEl);
  _svgEl = svgEl;

  // 创建容器 group（zoom/pan 的目标）
  _container = _svg.append('g').attr('class', 'graph-container');

  // 创建 zoom behavior
  _zoom = d3.zoom()
    .scaleExtent([0.1, 4])
    .on('zoom', (event) => {
      _container.attr('transform', event.transform);
    });

  _svg.call(_zoom);

  // 创建 simulation
  _simulation = d3.forceSimulation(_graphData.nodes)
    .force('link', d3.forceLink(_graphData.links)
      .id((d) => d.id)
      .distance(FORCE_CONFIG.link_distance))
    .force('charge', d3.forceManyBody().strength(FORCE_CONFIG.charge_strength))
    .force('center', d3.forceCenter(width / 2, height / 2).strength(FORCE_CONFIG.center_strength))
    .force('collision', d3.forceCollide().radius((d) => Math.sqrt(d.degree + 1) * FORCE_CONFIG.collision_radius));

  // 渲染边
  _linkSel = _container.append('g')
    .attr('class', 'graph-links')
    .selectAll('line')
    .data(_graphData.links)
    .enter()
    .append('line')
    .attr('class', 'graph-edge')
    .attr('stroke', (d) => getRelationColor(d.relation))
    .attr('stroke-width', 1.5);

  // 渲染节点
  _nodeSel = _container.append('g')
    .attr('class', 'graph-nodes')
    .selectAll('g')
    .data(_graphData.nodes)
    .enter()
    .append('g')
    .attr('class', 'graph-node')
    .attr('data-entity', (d) => d.id)
    .call(d3.drag()
      .on('start', dragStarted)
      .on('drag', dragged)
      .on('end', dragEnded));

  // 节点圆形
  _nodeSel.append('circle')
    .attr('class', 'graph-node-circle')
    .attr('r', (d) => Math.max(8, Math.sqrt(d.degree + 1) * 6))
    .attr('fill', '#1a1a20')
    .attr('stroke', '#38bdf8')
    .attr('stroke-width', 2)
    .style('cursor', 'pointer');

  // 实体类型图标（内联 SVG，居中在节点圆形上方）
  _nodeSel.each(function (d) {
    const entityType = _entityTypeMap[d.id];
    if (entityType) {
      const iconSvg = getEntityIcon(entityType);
      if (iconSvg) {
        d3.select(this)
          .append('g')
          .attr('class', 'graph-node-icon')
          .attr('data-entity-type', entityType)
          .attr('transform', 'translate(-8, -8)')
          .html(`<svg width="16" height="16" viewBox="0 0 16 16">${iconSvg}</svg>`);
      }
    }
  });

  // 节点标签
  _nodeSel.append('text')
    .attr('class', 'graph-node-label')
    .attr('dy', (d) => Math.max(8, Math.sqrt(d.degree + 1) * 6) + 14)
    .text((d) => {
      const maxLen = 20;
      return d.id.length > maxLen ? d.id.substring(0, maxLen) + '…' : d.id;
    });

  // 点击高亮
  _nodeSel.on('click', (event, d) => {
    event.stopPropagation();
    highlightNode(d.id);
  });

  // 双击展开详情
  _nodeSel.on('dblclick', (event, d) => {
    event.stopPropagation();
    showEntityDetail(d.id);
  });

  // hover 工具提示
  _nodeSel.on('mouseenter', (event, d) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) {
      const entityType = _entityTypeMap[d.id];
      const typeLabel = entityType ? `<br><span style="color:var(--accent)">${getEntityTypeName(entityType)}</span>` : '';
      tooltip.innerHTML = `<span class="tooltip-entity">${d.id}</span>${typeLabel}<br><span style="color:var(--text-quaternary)">${d.degree} ${t('graph.total_relations')}</span>`;
      tooltip.classList.add('graph-tooltip-visible');
      const rect = container.getBoundingClientRect();
      tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
      tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
    }
  });

  _nodeSel.on('mousemove', (event) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip && tooltip.classList.contains('graph-tooltip-visible')) {
      const rect = container.getBoundingClientRect();
      tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
      tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
    }
  });

  _nodeSel.on('mouseleave', () => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) tooltip.classList.remove('graph-tooltip-visible');
  });

  // 边 hover 工具提示
  _linkSel.on('mouseenter', (event, d) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) {
      tooltip.innerHTML = `<span class="tooltip-relation">${d.relation}</span><br><span style="color:var(--text-quaternary)">${d.source.id || d.source} → ${d.target.id || d.target}</span>`;
      tooltip.classList.add('graph-tooltip-visible');
      const rect = container.getBoundingClientRect();
      tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
      tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
    }
  });

  _linkSel.on('mouseleave', () => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) tooltip.classList.remove('graph-tooltip-visible');
  });

  // 点击空白区域取消高亮
  _svg.on('click', () => {
    if (_highlightedNode) {
      clearHighlight();
    }
  });

  // simulation tick 更新位置
  _simulation.on('tick', () => {
    _linkSel
      .attr('x1', (d) => d.source.x)
      .attr('y1', (d) => d.source.y)
      .attr('x2', (d) => d.target.x)
      .attr('y2', (d) => d.target.y);

    _nodeSel.attr('transform', (d) => `translate(${d.x},${d.y})`);
  });
}

// ============================================================
// 节点高亮
// ============================================================

/**
 * 高亮指定节点的所有关联边和关联节点。
 * @param {string} entityText - 实体文本
 */
export function highlightNode(entityText) {
  _highlightedNode = entityText;

  if (!_container) return;

  const nodes = _container.selectAll('.graph-node');
  const links = _container.selectAll('.graph-edge');

  // 找到关联的节点和边
  const connectedNodes = new Set([entityText]);
  for (const l of _graphData.links) {
    const sourceId = typeof l.source === 'object' ? l.source.id : l.source;
    const targetId = typeof l.target === 'object' ? l.target.id : l.target;
    if (sourceId === entityText) connectedNodes.add(targetId);
    if (targetId === entityText) connectedNodes.add(sourceId);
  }

  // 节点高亮/暗淡
  nodes.classed('graph-node-highlighted', (d) => d.id === entityText);
  nodes.classed('graph-node-dimmed', (d) => !connectedNodes.has(d.id));

  // 边高亮/暗淡
  links.classed('graph-edge-highlighted', (d) => {
    const sourceId = typeof d.source === 'object' ? d.source.id : d.source;
    const targetId = typeof d.target === 'object' ? d.target.id : d.target;
    return sourceId === entityText || targetId === entityText;
  });
  links.classed('graph-edge-dimmed', (d) => {
    const sourceId = typeof d.source === 'object' ? d.source.id : d.source;
    const targetId = typeof d.target === 'object' ? d.target.id : d.target;
    return sourceId !== entityText && targetId !== entityText;
  });
}

/**
 * 清除高亮状态。
 */
function clearHighlight() {
  _highlightedNode = null;

  if (!_container) return;

  _container.selectAll('.graph-node')
    .classed('graph-node-highlighted', false)
    .classed('graph-node-dimmed', false);

  _container.selectAll('.graph-edge')
    .classed('graph-edge-highlighted', false)
    .classed('graph-edge-dimmed', false);
}

// ============================================================
// 实体详情面板（增强：实体类型徽章 + 来源 chunk 预览）
// ============================================================

/**
 * 显示指定实体的详细关系列表（双击节点触发）。
 *
 * 增强内容：
 * - 实体类型徽章（带 data-entity-type 属性）
 * - 来源 chunk ID 预览
 *
 * @param {string} entityText - 实体文本
 */
async function showEntityDetail(entityText) {
  try {
    const relations = await invoke('get_entity_relations', { entityText });

    const panel = document.getElementById('graphDetailPanel');
    if (!panel) return;

    // 获取实体类型
    const entityType = _entityTypeMap[entityText] || 'unknown';
    const typeBadge = `<span class="graph-entity-badge" data-entity-type="${entityType}">${getEntityTypeName(entityType)}</span>`;

    let html = `
      <div class="graph-detail-header">
        <div class="graph-detail-title-row">
          <h3 class="graph-detail-title">${entityText}</h3>
          ${typeBadge}
        </div>
        <button class="graph-detail-close" id="graphDetailClose" aria-label="${t('common.close')}">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="6" x2="6" y2="18"/>
            <line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </button>
      </div>
      <div class="graph-detail-list">
    `;

    if (relations.length === 0) {
      html += `<div style="color:var(--text-quaternary);font-size:12px;text-align:center;padding:8px;">${t('graph.empty_hint')}</div>`;
    } else {
      for (const r of relations) {
        const isSubject = r.subject === entityText;
        const target = isSubject ? r.object : r.subject;
        const direction = isSubject ? '→' : '←';
        // 截断 chunk ID 显示
        const chunkId = r.chunk_id || '';
        const chunkPreview = chunkId.length > 12 ? chunkId.substring(0, 12) + '…' : chunkId;
        html += `
          <div class="graph-detail-item">
            <span class="detail-relation">${direction} ${r.relation_type}</span>
            <span class="detail-target">${target}</span>
            <span class="detail-confidence">${t('graph.detail_confidence')}: ${(r.confidence * 100).toFixed(0)}%</span>
            <span class="detail-chunk" title="${chunkId}">${t('graph.detail_source')}: ${chunkPreview}</span>
          </div>
        `;
      }
    }

    html += '</div>';
    panel.innerHTML = html;
    panel.classList.add('graph-detail-visible');
    _detailPanelEl = panel;

    // 绑定关闭按钮
    const closeBtn = document.getElementById('graphDetailClose');
    if (closeBtn) {
      closeBtn.addEventListener('click', () => {
        panel.classList.remove('graph-detail-visible');
      });
    }
  } catch {
    // 静默降级
  }
}

// ============================================================
// 缩放控制
// ============================================================

/**
 * 放大视图。
 */
export function zoomIn() {
  if (!_svg || !_zoom) return;
  _svg.transition().call(_zoom.scaleBy, 1.3);
}

/**
 * 缩小视图。
 */
export function zoomOut() {
  if (!_svg || !_zoom) return;
  _svg.transition().call(_zoom.scaleBy, 1 / 1.3);
}

/**
 * 重置视图到初始状态。
 */
export function resetView() {
  if (!_svg || !_zoom) return;
  _svg.transition().call(_zoom.transform, d3.zoomIdentity);
  if (_simulation) {
    _simulation.alpha(0.3).restart();
  }
}

// ============================================================
// 图例渲染
// ============================================================

/**
 * 渲染关系类型颜色图例。
 */
function renderLegend() {
  const legend = document.getElementById('graphLegend');
  if (!legend) return;

  const types = getUniqueRelationTypes(_graphData.links);
  if (types.length === 0) {
    legend.style.display = 'none';
    return;
  }

  legend.style.display = '';
  let html = `<div class="graph-legend-title">${t('graph.legend_title')}</div>`;
  for (const type of types) {
    const color = getRelationColor(type);
    html += `
      <div class="graph-legend-item">
        <span class="graph-legend-color" style="background:${color}"></span>
        <span>${type}</span>
      </div>
    `;
  }
  legend.innerHTML = html;
}

// ============================================================
// 子图过滤面板（Session 4 新增）
// ============================================================

/**
 * 渲染关系类型过滤面板（复选框列表）。
 *
 * 每个关系类型一个复选框，取消勾选后对应类型的边和仅该类型连接的节点隐藏。
 * 空过滤集合 = 全部启用。
 */
function renderFilterPanel() {
  const panel = document.getElementById('graphFilterPanel');
  if (!panel) return;

  const types = getUniqueRelationTypes(_graphData.links);
  if (types.length === 0) {
    panel.style.display = 'none';
    return;
  }

  panel.style.display = '';
  let html = `<div class="graph-filter-title">${t('graph.filter_title')}</div>`;
  html += '<div class="graph-filter-list">';
  for (const type of types) {
    const color = getRelationColor(type);
    const checked = isEdgeVisible(type, _enabledRelationTypes) ? 'checked' : '';
    html += `
      <label class="graph-filter-item">
        <input type="checkbox" class="graph-filter-checkbox" data-relation-type="${type}" ${checked} />
        <span class="graph-filter-color" style="background:${color}"></span>
        <span class="graph-filter-label">${type}</span>
      </label>
    `;
  }
  html += '</div>';
  panel.innerHTML = html;

  // 绑定复选框事件
  const checkboxes = panel.querySelectorAll('.graph-filter-checkbox');
  for (const cb of checkboxes) {
    cb.addEventListener('change', () => {
      const relType = cb.getAttribute('data-relation-type');
      if (cb.checked) {
        _enabledRelationTypes.delete(relType);
      } else {
        _enabledRelationTypes.add(relType);
      }
      applyFilter();
    });
  }
}

/**
 * 应用过滤：隐藏/显示边和节点。
 *
 * 过滤逻辑：
 * - 边：关系类型不在启用集合中的边添加 graph-edge-hidden 类
 * - 节点：所有连接边都被隐藏的节点添加 graph-node-hidden 类
 */
function applyFilter() {
  if (!_container) return;

  const links = _container.selectAll('.graph-edge');
  const nodes = _container.selectAll('.graph-node');

  // 计算每条边是否可见
  const edgeVisible = new Map();
  links.each(function (d) {
    const visible = isEdgeVisible(d.relation, _enabledRelationTypes);
    edgeVisible.set(d, visible);
  });

  // 应用边的可见性
  links.classed('graph-edge-hidden', (d) => !isEdgeVisible(d.relation, _enabledRelationTypes));

  // 计算每个节点是否至少有一条可见边
  const nodeHasVisibleEdge = new Set();
  for (const l of _graphData.links) {
    if (isEdgeVisible(l.relation, _enabledRelationTypes)) {
      const sourceId = typeof l.source === 'object' ? l.source.id : l.source;
      const targetId = typeof l.target === 'object' ? l.target.id : l.target;
      nodeHasVisibleEdge.add(sourceId);
      nodeHasVisibleEdge.add(targetId);
    }
  }

  // 应用节点的可见性
  // 孤立节点（degree=0）始终可见
  nodes.classed('graph-node-hidden', (d) => {
    if (d.degree === 0) return false;
    return !nodeHasVisibleEdge.has(d.id);
  });
}

// ============================================================
// 搜索定位（Session 4 新增）
// ============================================================

/**
 * 搜索实体并自动定位+高亮。
 *
 * 1. 在图谱数据中查找匹配的节点（包含搜索文本）
 * 2. 自动居中+缩放到第一个匹配节点
 * 3. 高亮匹配节点（graph-node-searched 类）
 *
 * @param {string} query - 搜索文本
 */
function searchAndLocate(query) {
  if (!query || !query.trim() || !_graphData.nodes.length) return;

  const lowerQuery = query.trim().toLowerCase();

  // 查找匹配的节点
  const matched = _graphData.nodes.filter((n) =>
    n.id.toLowerCase().includes(lowerQuery)
  );

  if (matched.length === 0) {
    // 无匹配，清除搜索高亮
    clearSearchHighlight();
    return;
  }

  // 清除之前的搜索高亮
  clearSearchHighlight();

  // 高亮所有匹配节点
  if (_container) {
    _container.selectAll('.graph-node')
      .classed('graph-node-searched', (d) =>
        matched.some((m) => m.id === d.id)
      );
  }

  // 自动定位到第一个匹配节点（居中 + 适当缩放）
  const target = matched[0];
  if (target.x !== undefined && target.y !== undefined && _svg && _zoom) {
    const container = document.getElementById('graphCanvasContainer');
    if (container) {
      const width = container.clientWidth;
      const height = container.clientHeight;
      const scale = 1.5;
      const tx = width / 2 - target.x * scale;
      const ty = height / 2 - target.y * scale;
      _svg.transition()
        .duration(500)
        .call(_zoom.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
    }
  }
}

/**
 * 清除搜索高亮。
 */
function clearSearchHighlight() {
  if (!_container) return;
  _container.selectAll('.graph-node').classed('graph-node-searched', false);
}

// ============================================================
// 图谱导出（Session 4 新增）
// ============================================================

/**
 * 导出图谱为 SVG 文件。
 *
 * 序列化 SVG DOM → 添加 XML 声明 → Blob → 下载。
 */
function exportSvg() {
  if (!_svgEl) return;

  try {
    // 克隆 SVG 节点
    const clone = _svgEl.cloneNode(true);
    // 添加 xmlns 属性
    clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    clone.setAttribute('xmlns:xlink', 'http://www.w3.org/1999/xlink');

    // 序列化为字符串
    const serializer = new XMLSerializer();
    let svgStr = serializer.serializeToString(clone);
    // 添加 XML 声明
    svgStr = '<?xml version="1.0" encoding="UTF-8"?>\n' + svgStr;

    // 创建 Blob 并下载
    const blob = new Blob([svgStr], { type: 'image/svg+xml;charset=utf-8' });
    downloadBlob(blob, 'knowledge-graph.svg');
  } catch {
    // 降级：静默
  }
}

/**
 * 导出图谱为 PNG 文件。
 *
 * SVG → Canvas → PNG Blob → 下载。
 */
function exportPng() {
  if (!_svgEl) return;

  try {
    const container = document.getElementById('graphCanvasContainer');
    if (!container) return;

    const width = container.clientWidth;
    const height = container.clientHeight;

    // 克隆 SVG 节点
    const clone = _svgEl.cloneNode(true);
    clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    clone.setAttribute('width', String(width));
    clone.setAttribute('height', String(height));

    // 序列化为字符串
    const serializer = new XMLSerializer();
    const svgStr = serializer.serializeToString(clone);
    const svgBlob = new Blob([svgStr], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(svgBlob);

    // 创建 Image，绘制到 Canvas
    const img = new Image();
    img.onload = function () {
      const canvas = document.createElement('canvas');
      const scale = 2; // 2x 分辨率
      canvas.width = width * scale;
      canvas.height = height * scale;
      const ctx = canvas.getContext('2d');
      if (ctx) {
        ctx.fillStyle = '#0f1115';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        ctx.scale(scale, scale);
        ctx.drawImage(img, 0, 0);
      }
      URL.revokeObjectURL(url);

      // Canvas → PNG Blob → 下载
      canvas.toBlob((pngBlob) => {
        if (pngBlob) {
          downloadBlob(pngBlob, 'knowledge-graph.png');
        }
      }, 'image/png');
    };
    img.onerror = function () {
      URL.revokeObjectURL(url);
    };
    img.src = url;
  } catch {
    // 降级：静默
  }
}

/**
 * 导出知识图谱为 GraphML 或 JSON-LD 格式（REQ-EXP-006）。
 *
 * 调用后端 export_graph IPC 获取格式化字符串，通过 Blob 下载文件。
 *
 * @param {string} format - 导出格式（'graphml' 或 'jsonld'）
 */
async function exportGraphData(format) {
  try {
    const content = await invoke('export_graph', { format });
    if (!content) return;

    const mimeType = format === 'graphml'
      ? 'application/xml;charset=utf-8'
      : 'application/ld+json;charset=utf-8';
    const filename = format === 'graphml'
      ? 'knowledge-graph.graphml'
      : 'knowledge-graph.jsonld';

    const blob = new Blob([content], { type: mimeType });
    downloadBlob(blob, filename);
  } catch (err) {
    // 降级：静默
  }
}

/**
 * 触发 Blob 文件下载。
 *
 * 创建临时 <a download> 元素，点击后移除。
 *
 * @param {Blob} blob - 要下载的 Blob 数据
 * @param {string} filename - 下载文件名
 */
function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  a.style.display = 'none';
  document.body.appendChild(a);
  a.click();
  setTimeout(() => {
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, 100);
}

// ============================================================
// 布局切换（Session 5 新增）
// ============================================================

/**
 * 切换图谱布局模式。
 *
 * 支持三种布局：
 * - force（力导向）：d3.forceSimulation（当前默认）
 * - hierarchical（层次树形）：d3.tree 自上而下分层布局
 * - radial（辐射环形）：d3.tree + 极坐标转换
 *
 * @param {string} mode - 布局模式（force / hierarchical / radial）
 */
export async function switchLayout(mode) {
  if (mode === _currentLayout) return;

  // 验证布局模式有效性
  const validModes = ['force', 'hierarchical', 'radial'];
  if (!validModes.includes(mode)) return;

  // 更新按钮状态
  const btns = document.querySelectorAll('.graph-layout-btn');
  for (const btn of btns) {
    btn.classList.toggle('graph-layout-active', btn.getAttribute('data-layout') === mode);
  }

  _currentLayout = mode;

  // 停止旧 simulation
  if (_simulation) {
    _simulation.stop();
    _simulation = null;
  }

  // 根据布局模式重新渲染
  if (mode === 'force') {
    renderForceLayout();
  } else if (mode === 'hierarchical') {
    renderHierarchicalLayout(false);
  } else if (mode === 'radial') {
    renderHierarchicalLayout(true);
  }
}

/**
 * 渲染力导向布局（d3.forceSimulation）。
 */
function renderForceLayout() {
  const container = document.getElementById('graphCanvasContainer');
  if (!container || !_svg) return;

  const width = container.clientWidth;
  const height = container.clientHeight;

  // 清除旧内容
  _container.selectAll('*').remove();

  _simulation = d3.forceSimulation(_graphData.nodes)
    .force('link', d3.forceLink(_graphData.links)
      .id((d) => d.id)
      .distance(FORCE_CONFIG.link_distance))
    .force('charge', d3.forceManyBody().strength(FORCE_CONFIG.charge_strength))
    .force('center', d3.forceCenter(width / 2, height / 2).strength(FORCE_CONFIG.center_strength))
    .force('collision', d3.forceCollide().radius((d) => Math.sqrt(d.degree + 1) * FORCE_CONFIG.collision_radius));

  // 重新渲染边和节点
  reRenderNodesAndLinks();
}

/**
 * 渲染层次树形/辐射环形布局（d3.tree）。
 *
 * @param {boolean} radial - true=辐射环形，false=层次树形
 */
function renderHierarchicalLayout(radial) {
  const container = document.getElementById('graphCanvasContainer');
  if (!container || !_svg) return;

  const width = container.clientWidth;
  const height = container.clientHeight;

  // 清除旧内容
  _container.selectAll('*').remove();

  // 构建层次结构：选择度最高的节点作为根
  const rootNode = _graphData.nodes.reduce((max, n) => (n.degree > max.degree ? n : max), _graphData.nodes[0]);

  // 构建邻接表
  const adjacency = {};
  for (const n of _graphData.nodes) {
    adjacency[n.id] = [];
  }
  for (const l of _graphData.links) {
    const s = typeof l.source === 'object' ? l.source.id : l.source;
    const t = typeof l.target === 'object' ? l.target.id : l.target;
    if (adjacency[s]) adjacency[s].push(t);
    if (adjacency[t]) adjacency[t].push(s);
  }

  // BFS 构建树结构
  const visited = new Set();
  const buildTree = (nodeId) => {
    visited.add(nodeId);
    const children = (adjacency[nodeId] || [])
      .filter((child) => !visited.has(child))
      .map((child) => buildTree(child));
    return { id: nodeId, children };
  };

  const treeData = buildTree(rootNode.id);

  // 使用 d3.hierarchy + d3.tree
  const root = d3.hierarchy(treeData);
  const treeLayout = d3.tree().size(
    radial ? [2 * Math.PI, Math.min(width, height) / 2 - 50] : [width - 100, height - 100]
  );
  treeLayout(root);

  // 获取布局后的节点位置
  const layoutNodes = root.descendants();
  const nodeMap = new Map(layoutNodes.map((n) => [n.data.id, n]));

  // 更新 _graphData 节点位置
  for (const n of _graphData.nodes) {
    const layoutNode = nodeMap.get(n.id);
    if (layoutNode) {
      if (radial) {
        n.x = layoutNode.x * (height / 2 / Math.min(width, height) * 2) + width / 2;
        n.y = layoutNode.y * (height / Math.min(width, height)) + height / 4;
      } else {
        n.x = layoutNode.x + 50;
        n.y = layoutNode.y + 50;
      }
    }
  }

  // 创建静态边（不使用 simulation）
  _linkSel = _container.append('g')
    .attr('class', 'graph-links')
    .selectAll('line')
    .data(_graphData.links)
    .enter()
    .append('line')
    .attr('class', 'graph-edge')
    .attr('stroke', (d) => getRelationColor(d.relation))
    .attr('stroke-width', 1.5)
    .attr('x1', (d) => {
      const s = typeof d.source === 'object' ? d.source : _graphData.nodes.find((n) => n.id === d.source);
      return s ? s.x : 0;
    })
    .attr('y1', (d) => {
      const s = typeof d.source === 'object' ? d.source : _graphData.nodes.find((n) => n.id === d.source);
      return s ? s.y : 0;
    })
    .attr('x2', (d) => {
      const t = typeof d.target === 'object' ? d.target : _graphData.nodes.find((n) => n.id === d.target);
      return t ? t.x : 0;
    })
    .attr('y2', (d) => {
      const t = typeof d.target === 'object' ? d.target : _graphData.nodes.find((n) => n.id === d.target);
      return t ? t.y : 0;
    });

  // 创建静态节点
  _nodeSel = _container.append('g')
    .attr('class', 'graph-nodes')
    .selectAll('g')
    .data(_graphData.nodes)
    .enter()
    .append('g')
    .attr('class', 'graph-node')
    .attr('data-entity', (d) => d.id)
    .attr('transform', (d) => `translate(${d.x},${d.y})`)
    .call(d3.drag()
      .on('start', (event) => { event.subject.fx = event.subject.x; event.subject.fy = event.subject.y; })
      .on('drag', (event, d) => { d.x = event.x; d.y = event.y; d3.select(event.sourceEvent.target.closest('.graph-node')).attr('transform', `translate(${d.x},${d.y})`); })
      .on('end', (event, d) => { d.fx = null; d.fy = null; }));

  // 渲染节点圆形、图标、标签
  renderNodeVisuals();

  // 绑定交互事件
  bindNodeInteractions();
}

/**
 * 重新渲染节点和边（用于 force 布局重建后）。
 */
function reRenderNodesAndLinks() {
  const container = document.getElementById('graphCanvasContainer');
  if (!container || !_container) return;

  // 渲染边
  _linkSel = _container.append('g')
    .attr('class', 'graph-links')
    .selectAll('line')
    .data(_graphData.links)
    .enter()
    .append('line')
    .attr('class', 'graph-edge')
    .attr('stroke', (d) => getRelationColor(d.relation))
    .attr('stroke-width', 1.5);

  // 渲染节点
  _nodeSel = _container.append('g')
    .attr('class', 'graph-nodes')
    .selectAll('g')
    .data(_graphData.nodes)
    .enter()
    .append('g')
    .attr('class', 'graph-node')
    .attr('data-entity', (d) => d.id)
    .call(d3.drag()
      .on('start', dragStarted)
      .on('drag', dragged)
      .on('end', dragEnded));

  // 渲染节点圆形、图标、标签
  renderNodeVisuals();

  // 绑定交互事件
  bindNodeInteractions();

  // simulation tick 更新位置
  _simulation.on('tick', () => {
    _linkSel
      .attr('x1', (d) => d.source.x)
      .attr('y1', (d) => d.source.y)
      .attr('x2', (d) => d.target.x)
      .attr('y2', (d) => d.target.y);

    _nodeSel.attr('transform', (d) => `translate(${d.x},${d.y})`);
  });
}

/**
 * 渲染节点视觉元素（圆形 + 图标 + 标签）。
 */
function renderNodeVisuals() {
  if (!_nodeSel) return;

  // 节点圆形
  _nodeSel.append('circle')
    .attr('class', 'graph-node-circle')
    .attr('r', (d) => Math.max(8, Math.sqrt(d.degree + 1) * 6))
    .attr('fill', (d) => {
      if (_communityEnabled && _communityMap[d.id] !== undefined) {
        return COMMUNITY_COLORS[_communityMap[d.id] % COMMUNITY_COLORS.length];
      }
      return '#1a1a20';
    })
    .attr('stroke', (d) => {
      if (_communityEnabled && _communityMap[d.id] !== undefined) {
        return COMMUNITY_COLORS[_communityMap[d.id] % COMMUNITY_COLORS.length];
      }
      return '#38bdf8';
    })
    .attr('stroke-width', 2)
    .style('cursor', 'pointer');

  // 实体类型图标
  _nodeSel.each(function (d) {
    const entityType = _entityTypeMap[d.id];
    if (entityType) {
      const iconSvg = getEntityIcon(entityType);
      if (iconSvg) {
        d3.select(this)
          .append('g')
          .attr('class', 'graph-node-icon')
          .attr('data-entity-type', entityType)
          .attr('transform', 'translate(-8, -8)')
          .html(`<svg width="16" height="16" viewBox="0 0 16 16">${iconSvg}</svg>`);
      }
    }
  });

  // 节点标签
  _nodeSel.append('text')
    .attr('class', 'graph-node-label')
    .attr('dy', (d) => Math.max(8, Math.sqrt(d.degree + 1) * 6) + 14)
    .text((d) => {
      const maxLen = 20;
      return d.id.length > maxLen ? d.id.substring(0, maxLen) + '…' : d.id;
    });
}

/**
 * 绑定节点交互事件（点击高亮、双击详情、hover 工具提示）。
 */
function bindNodeInteractions() {
  if (!_nodeSel) return;

  const container = document.getElementById('graphCanvasContainer');

  // 点击高亮
  _nodeSel.on('click', (event, d) => {
    event.stopPropagation();
    highlightNode(d.id);
  });

  // 双击展开详情
  _nodeSel.on('dblclick', (event, d) => {
    event.stopPropagation();
    showEntityDetail(d.id);
  });

  // hover 工具提示
  _nodeSel.on('mouseenter', (event, d) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) {
      const entityType = _entityTypeMap[d.id];
      const typeLabel = entityType ? `<br><span style="color:var(--accent)">${getEntityTypeName(entityType)}</span>` : '';
      tooltip.innerHTML = `<span class="tooltip-entity">${d.id}</span>${typeLabel}<br><span style="color:var(--text-quaternary)">${d.degree} ${t('graph.total_relations')}</span>`;
      tooltip.classList.add('graph-tooltip-visible');
      if (container) {
        const rect = container.getBoundingClientRect();
        tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
        tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
      }
    }
  });

  _nodeSel.on('mousemove', (event) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip && tooltip.classList.contains('graph-tooltip-visible') && container) {
      const rect = container.getBoundingClientRect();
      tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
      tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
    }
  });

  _nodeSel.on('mouseleave', () => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) tooltip.classList.remove('graph-tooltip-visible');
  });

  // 边 hover
  if (_linkSel) {
    _linkSel.on('mouseenter', (event, d) => {
      const tooltip = document.getElementById('graphTooltip');
      if (tooltip && container) {
        tooltip.innerHTML = `<span class="tooltip-relation">${d.relation}</span><br><span style="color:var(--text-quaternary)">${d.source.id || d.source} → ${d.target.id || d.target}</span>`;
        tooltip.classList.add('graph-tooltip-visible');
        const rect = container.getBoundingClientRect();
        tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
        tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
      }
    });

    _linkSel.on('mouseleave', () => {
      const tooltip = document.getElementById('graphTooltip');
      if (tooltip) tooltip.classList.remove('graph-tooltip-visible');
    });
  }

  // 点击空白取消高亮
  if (_svg) {
    _svg.on('click', () => {
      if (_highlightedNode) {
        clearHighlight();
      }
    });
  }
}

// ============================================================
// 路径分析（Session 5 新增）
// ============================================================

/**
 * 渲染路径分析实体选择器（下拉框）。
 *
 * 将所有节点实体填充到两个 <select> 元素中。
 */
function renderPathSelectors() {
  const fromSelect = document.getElementById('graphPathFrom');
  const toSelect = document.getElementById('graphPathTo');
  if (!fromSelect || !toSelect) return;

  // 按字母排序
  const sortedNodes = [..._graphData.nodes].sort((a, b) => a.id.localeCompare(b.id));

  // 清除旧选项（保留第一个占位）
  fromSelect.innerHTML = `<option value="">${t('graph.path_from')}</option>`;
  toSelect.innerHTML = `<option value="">${t('graph.path_to')}</option>`;

  for (const n of sortedNodes) {
    fromSelect.appendChild(new Option(n.id, n.id));
    toSelect.appendChild(new Option(n.id, n.id));
  }

  // 绑定 change 事件
  fromSelect.addEventListener('change', () => {
    _pathFromEntity = fromSelect.value || null;
  });
  toSelect.addEventListener('change', () => {
    _pathToEntity = toSelect.value || null;
  });
}

/**
 * 查找最短路径并高亮显示。
 *
 * 调用 get_shortest_path IPC 获取路径，然后在图谱上高亮路径节点和边。
 */
export async function findPath() {
  if (!_pathFromEntity || !_pathToEntity) {
    const result = document.getElementById('graphPathResult');
    if (result) result.textContent = t('graph.path_select_both');
    return;
  }

  const result = document.getElementById('graphPathResult');
  if (result) result.textContent = t('graph.path_searching');

  try {
    const pathResult = await invoke('get_shortest_path', {
      from: _pathFromEntity,
      to: _pathToEntity,
    });

    if (!pathResult.path || pathResult.path.length === 0) {
      if (result) result.textContent = t('graph.path_no_result');
      _pathHighlightNodes = new Set();
      applyPathHighlight();
      return;
    }

    // 显示路径结果
    const pathStr = pathResult.path.join(' → ');
    if (result) {
      result.innerHTML = `${t('graph.path_length')}: ${pathResult.hops} ${t('graph.path_hops')}<br><span class="path-detail">${pathStr}</span>`;
    }

    // 高亮路径
    _pathHighlightNodes = new Set(pathResult.path);
    applyPathHighlight();
  } catch (err) {
    if (result) result.textContent = t('graph.path_error');
  }
}

/**
 * 应用路径高亮：路径上的节点和边高亮，其他暗淡。
 */
function applyPathHighlight() {
  if (!_container) return;

  const nodes = _container.selectAll('.graph-node');
  const links = _container.selectAll('.graph-edge');

  if (_pathHighlightNodes.size === 0) {
    // 清除路径高亮
    nodes.classed('graph-node-on-path', false).classed('graph-node-dimmed', false);
    links.classed('graph-edge-on-path', false).classed('graph-edge-dimmed', false);
    return;
  }

  // 节点高亮
  nodes.classed('graph-node-on-path', (d) => _pathHighlightNodes.has(d.id));
  nodes.classed('graph-node-dimmed', (d) => !_pathHighlightNodes.has(d.id));

  // 边高亮：两端节点都在路径上的边
  links.classed('graph-edge-on-path', (d) => {
    const s = typeof d.source === 'object' ? d.source.id : d.source;
    const t = typeof d.target === 'object' ? d.target.id : d.target;
    // 检查边是否在路径的相邻节点之间
    const pathArray = Array.from(_pathHighlightNodes);
    for (let i = 0; i < pathArray.length - 1; i++) {
      if ((pathArray[i] === s && pathArray[i + 1] === t) ||
          (pathArray[i] === t && pathArray[i + 1] === s)) {
        return true;
      }
    }
    return false;
  });
  links.classed('graph-edge-dimmed', (d) => {
    const s = typeof d.source === 'object' ? d.source.id : d.source;
    const t = typeof d.target === 'object' ? d.target.id : d.target;
    const pathArray = Array.from(_pathHighlightNodes);
    for (let i = 0; i < pathArray.length - 1; i++) {
      if ((pathArray[i] === s && pathArray[i + 1] === t) ||
          (pathArray[i] === t && pathArray[i + 1] === s)) {
        return false;
      }
    }
    return true;
  });
}

// ============================================================
// 社区检测（Session 5 新增）
// ============================================================

/**
 * 切换社区检测着色。
 *
 * 调用 get_communities IPC 获取社区映射，然后按社区 ID 为节点着色。
 * 再次点击关闭社区着色，恢复默认颜色。
 */
export async function toggleCommunities() {
  if (_communityEnabled) {
    // 关闭社区着色
    _communityEnabled = false;
    _communityMap = {};
    const countEl = document.getElementById('graphCommunityCount');
    if (countEl) countEl.textContent = '';
    // 重新渲染节点颜色
    if (_container) {
      _container.selectAll('.graph-node-circle')
        .attr('fill', '#1a1a20')
        .attr('stroke', '#38bdf8');
    }
    return;
  }

  const countEl = document.getElementById('graphCommunityCount');
  if (countEl) countEl.textContent = t('graph.community_detecting');

  try {
    const result = await invoke('get_communities');

    if (!result.communities || Object.keys(result.communities).length === 0) {
      if (countEl) countEl.textContent = t('graph.community_empty');
      return;
    }

    _communityMap = result.communities;
    _communityEnabled = true;

    // 显示社区数
    if (countEl) {
      countEl.textContent = `${result.community_count} ${t('graph.community_count_label')}`;
    }

    // 应用社区颜色
    if (_container) {
      _container.selectAll('.graph-node-circle')
        .attr('fill', (d) => {
          const cid = _communityMap[d.id];
          if (cid !== undefined) {
            return COMMUNITY_COLORS[cid % COMMUNITY_COLORS.length];
          }
          return '#1a1a20';
        })
        .attr('stroke', (d) => {
          const cid = _communityMap[d.id];
          if (cid !== undefined) {
            return COMMUNITY_COLORS[cid % COMMUNITY_COLORS.length];
          }
          return '#38bdf8';
        });
    }
  } catch {
    if (countEl) countEl.textContent = t('graph.community_error');
  }
}

// ============================================================
// D3 拖拽回调
// ============================================================

/**
 * 拖拽开始：固定节点位置（增加 alpha 目标值使 simulation 重新活跃）。
 */
function dragStarted(event, d) {
  if (!event.active) _simulation.alphaTarget(0.3).restart();
  d.fx = d.x;
  d.fy = d.y;
}

/**
 * 拖拽中：更新节点固定位置。
 */
function dragged(event, d) {
  d.fx = event.x;
  d.fy = event.y;
}

/**
 * 拖拽结束：释放固定位置（让 simulation 重新接管）。
 */
function dragEnded(event, d) {
  if (!event.active) _simulation.alphaTarget(0);
  d.fx = null;
  d.fy = null;
}

// ============================================================
// 窗口大小变化处理
// ============================================================

/** resize 防抖计时器 */
let _resizeTimer = null;

/**
 * 初始化窗口大小变化监听（图谱打开时自动调整 SVG 尺寸）。
 */
export function initGraphResize() {
  window.addEventListener('resize', () => {
    if (!_overlayEl || !_overlayEl.classList.contains('graph-visible')) return;
    clearTimeout(_resizeTimer);
    _resizeTimer = setTimeout(() => {
      if (_simulation && _svgEl) {
        const container = document.getElementById('graphCanvasContainer');
        if (container) {
          const width = container.clientWidth;
          const height = container.clientHeight;
          _svgEl.setAttribute('viewBox', `0 0 ${width} ${height}`);
          _simulation
            .force('center', d3.forceCenter(width / 2, height / 2).strength(FORCE_CONFIG.center_strength))
            .alpha(0.3)
            .restart();
        }
      }
    }, 200);
  });
}
