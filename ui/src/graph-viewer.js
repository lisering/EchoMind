/**
 * EchoMind 知识图谱可视化模块（REQ-RAG-027 前端图谱可视化）。
 *
 * 从 v1.21 拆分：渲染逻辑移至 graph-renderer.js。
 * 本模块仅负责：
 * 1. 模块状态管理（graphState 对象）
 * 2. 全屏 overlay 面板创建（ensureOverlay）
 * 3. 打开/关闭面板（openGraphViewer / closeGraphViewer）
 * 4. 窗口大小变化处理（initGraphResize）
 *
 * 依赖：D3.js v7（全局 window.d3），graph-renderer.js
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { isComposingEvent } from './input-utils.js';
import { loadD3 } from './lazy-loader.js';
import {
  DEFAULT_LIMIT,
  buildGraphData,
  renderGraph,
  renderLegend,
  renderFilterPanel,
  renderPathSelectors,
  renderNodeVisuals,
  bindNodeInteractions,
  highlightNode,
  clearHighlight,
  showEntityDetail,
  zoomIn,
  zoomOut,
  resetView,
  searchAndLocate,
  clearSearchHighlight,
  exportSvg,
  exportPng,
  exportGraphData,
  switchLayout,
  findPath,
  applyPathHighlight,
  toggleCommunities,
  getEntityTypeName,
} from './graph-renderer.js';

// ============================================================
// 模块状态
// ============================================================

const graphState = {
  _overlayEl: null,
  _svgEl: null,
  _simulation: null,
  _zoom: null,
  _svg: null,
  _container: null,
  _linkSel: null,
  _nodeSel: null,
  _trap: null,
  _graphData: { nodes: [], links: [] },
  _highlightedNode: null,
  _detailPanelEl: null,
  _entityTypeMap: {},
  _enabledRelationTypes: new Set(),
  _currentLayout: 'force',
  _pathFromEntity: null,
  _pathToEntity: null,
  _pathHighlightNodes: new Set(),
  _communityMap: {},
  _communityEnabled: false,
};

// ============================================================
// 面板创建与管理
// ============================================================

function ensureOverlay() {
  if (graphState._overlayEl) return;

  graphState._overlayEl = document.createElement('div');
  graphState._overlayEl.id = 'graphOverlay';
  graphState._overlayEl.className = 'graph-overlay';
  graphState._overlayEl.setAttribute('role', 'dialog');
  graphState._overlayEl.setAttribute('aria-modal', 'true');
  graphState._overlayEl.innerHTML = `
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
        <div class="empty-icon"><svg class="icon-lg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="5" r="2"/><circle cx="5" cy="19" r="2"/><circle cx="19" cy="19" r="2"/><line x1="12" y1="7" x2="6" y2="17"/><line x1="12" y1="7" x2="18" y2="17"/><line x1="6" y1="19" x2="18" y2="19"/></svg></div>
        <div class="empty-text">${t('graph.empty_hint')}</div>
      </div>
    </div>
  `;
  document.body.appendChild(graphState._overlayEl);

  // 绑定事件
  document.getElementById('graphCloseBtn').addEventListener('click', closeGraphViewer);
  document.getElementById('graphZoomIn').addEventListener('click', () => zoomIn(graphState));
  document.getElementById('graphZoomOut').addEventListener('click', () => zoomOut(graphState));
  document.getElementById('graphReset').addEventListener('click', () => resetView(graphState));
  document.getElementById('graphExportSvg').addEventListener('click', () => exportSvg(graphState));
  document.getElementById('graphExportPng').addEventListener('click', () => exportPng(graphState));
  document.getElementById('graphExportGraphml').addEventListener('click', () => exportGraphData('graphml'));
  document.getElementById('graphExportJsonld').addEventListener('click', () => exportGraphData('jsonld'));

  // 布局切换按钮
  const layoutBtns = graphState._overlayEl.querySelectorAll('.graph-layout-btn');
  for (const btn of layoutBtns) {
    btn.addEventListener('click', () => {
      const mode = btn.getAttribute('data-layout');
      switchLayout(graphState, mode);
    });
  }

  // 路径分析：选择器 + 按钮
  document.getElementById('graphPathFindBtn').addEventListener('click', () => findPath(graphState));

  // 社区检测按钮
  document.getElementById('graphCommunityBtn').addEventListener('click', () => toggleCommunities(graphState));

  // 搜索框事件
  const searchInput = document.getElementById('graphSearchInput');
  if (searchInput) {
    searchInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        if (isComposingEvent(e)) return;
        e.preventDefault();
        searchAndLocate(graphState, searchInput.value);
      }
    });
  }

  // ESC 关闭
  graphState._overlayEl.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeGraphViewer();
    }
  });

  // 创建 Focus Trap
  graphState._trap = createFocusTrap(graphState._overlayEl);

  // 注册到面板栈
  pushPanel({ id: 'graph-viewer', close: closeGraphViewer, element: graphState._overlayEl, label: 'Graph Viewer' });
}

/**
 * 打开知识图谱面板。
 */
export async function openGraphViewer() {
  ensureOverlay();

  const statsBar = document.getElementById('graphStatsBar');
  if (statsBar) statsBar.textContent = t('graph.loading');

  graphState._overlayEl.classList.add('graph-visible');

  if (graphState._trap) graphState._trap.activate();

  // 延迟加载 D3.js v7（273KB）—— 仅知识图谱面板使用
  const d3 = await loadD3();
  if (!d3) {
    if (statsBar) statsBar.textContent = t('graph.loading');
    return;
  }

  try {
    const [triples, stats] = await Promise.all([
      invoke('get_graph_data', { limit: DEFAULT_LIMIT }),
      invoke('get_graph_stats'),
    ]);

    if (statsBar) {
      statsBar.innerHTML = `
        <span class="stat-item"><span class="stat-value">${stats.total_entities}</span> ${t('graph.total_entities')}</span>
        <span class="stat-item"><span class="stat-value">${stats.total_relations}</span> ${t('graph.total_relations')}</span>
      `;
    }

    if (!triples || triples.length === 0) {
      document.getElementById('graphEmptyState').style.display = 'flex';
      document.getElementById('graphSvg').style.display = 'none';
      document.getElementById('graphFilterPanel').style.display = 'none';
      document.getElementById('graphLegend').style.display = 'none';
      return;
    }

    document.getElementById('graphEmptyState').style.display = 'none';
    document.getElementById('graphSvg').style.display = '';

    if (stats.total_relations > DEFAULT_LIMIT) {
      const partialHint = document.createElement('span');
      partialHint.className = 'stat-item';
      partialHint.textContent = t('graph.showing_partial', {
        shown: triples.length,
        total: stats.total_relations,
      });
      statsBar.appendChild(partialHint);
    }

    graphState._graphData = buildGraphData(triples);

    const entityIds = graphState._graphData.nodes.map((n) => n.id);
    try {
      graphState._entityTypeMap = await invoke('get_entity_types', { entities: entityIds });
    } catch {
      graphState._entityTypeMap = {};
    }

    graphState._enabledRelationTypes = new Set();

    renderGraph(graphState);
    renderLegend(graphState);
    renderFilterPanel(graphState);
    renderPathSelectors(graphState);
  } catch (err) {
    if (statsBar) statsBar.textContent = String(err);
  }
}

/**
 * 关闭知识图谱面板。
 */
export function closeGraphViewer() {
  if (!graphState._overlayEl) return;

  removePanel('graph-viewer');

  graphState._overlayEl.classList.remove('graph-visible');

  if (graphState._trap) graphState._trap.deactivate();

  if (graphState._simulation) {
    graphState._simulation.stop();
    graphState._simulation = null;
  }

  graphState._highlightedNode = null;

  if (graphState._detailPanelEl) {
    graphState._detailPanelEl.classList.remove('graph-detail-visible');
  }

  clearSearchHighlight(graphState);

  graphState._pathFromEntity = null;
  graphState._pathToEntity = null;
  graphState._pathHighlightNodes = new Set();

  graphState._communityMap = {};
  graphState._communityEnabled = false;
  const communityCount = document.getElementById('graphCommunityCount');
  if (communityCount) communityCount.textContent = '';

  graphState._currentLayout = 'force';

  const pathResult = document.getElementById('graphPathResult');
  if (pathResult) pathResult.textContent = '';
}

// ============================================================
// 窗口大小变化处理
// ============================================================

let _resizeTimer = null;

export function initGraphResize() {
  window.addEventListener('resize', () => {
    if (!graphState._overlayEl || !graphState._overlayEl.classList.contains('graph-visible')) return;
    clearTimeout(_resizeTimer);
    _resizeTimer = setTimeout(() => {
      if (graphState._simulation && graphState._svgEl) {
        const container = document.getElementById('graphCanvasContainer');
        if (container) {
          const width = container.clientWidth;
          const height = container.clientHeight;
          graphState._svgEl.setAttribute('viewBox', `0 0 ${width} ${height}`);
          // d3 全局变量在 openGraphViewer 中已通过 loadD3() 加载
          if (typeof d3 !== 'undefined') {
            graphState._simulation
              .force('center', d3.forceCenter(width / 2, height / 2).strength(0.05))
              .alpha(0.3)
              .restart();
          }
        }
      }
    }, 200);
  });
}
