/**
 * EchoMind 知识图谱渲染模块（从 graph-viewer.js 拆分）。
 *
 * 职责：
 * 1. 常量定义（关系颜色、力导向参数、实体图标、社区调色板）
 * 2. 纯逻辑工具函数（getRelationColor / getEntityIcon / buildGraphData 等）
 * 3. D3 forceSimulation 渲染 + 布局切换（force / hierarchical / radial）
 * 4. 图例渲染 + 子图过滤面板
 * 5. 图谱导出（SVG / PNG / GraphML / JSON-LD）
 *
 * 依赖：D3.js v7（全局 window.d3）
 */

import { t } from './i18n.js';
import { invoke, graphApi } from './ipc.js';

// ============================================================
// 常量
// ============================================================

/** 关系类型 → 颜色映射（8 种关系类型 + 默认） */
const RELATION_COLORS = {
  defined_as: '#38bdf8',
  part_of: '#a78bfa',
  depends_on: '#fb923c',
  uses: '#4ade80',
  implements: '#f472b6',
  extends: '#facc15',
  references: '#22d3ee',
  related_to: '#94a3b8',
};

const DEFAULT_NODE_COLOR = '#64748b';

const FORCE_CONFIG = {
  charge_strength: -300,
  link_distance: 80,
  center_strength: 0.05,
  collision_radius: 20,
};

const DEFAULT_LIMIT = 200;

const ENTITY_TYPE_ICONS = {
  person: '<circle cx="8" cy="5" r="2.5" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M3 14c0-3 2.5-5 5-5s5 2 5 5" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
  proper_noun: '<rect x="2" y="4" width="12" height="8" rx="2" fill="none" stroke="currentColor" stroke-width="1.2"/><circle cx="5" cy="8" r="1" fill="currentColor"/>',
  tech_term: '<path d="M5 4L2 8L5 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/><path d="M11 4L14 8L11 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>',
  identifier: '<path d="M4 6h8M4 10h8M6 4l-1 8M10 4l-1 8" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
  date: '<rect x="2" y="3" width="12" height="11" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M2 6h12" fill="none" stroke="currentColor" stroke-width="1.2"/><path d="M5 1.5v3M11 1.5v3" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>',
};

const ENTITY_TYPE_I18N_KEYS = {
  person: 'graph.entity_type_person',
  proper_noun: 'graph.entity_type_proper_noun',
  tech_term: 'graph.entity_type_tech_term',
  identifier: 'graph.entity_type_identifier',
  date: 'graph.entity_type_date',
};

const COMMUNITY_COLORS = [
  '#38bdf8', '#a78bfa', '#fb923c', '#4ade80', '#f472b6',
  '#facc15', '#22d3ee', '#fb7185', '#818cf8', '#34d399',
];

// 导出常量供其他模块使用
export { RELATION_COLORS, DEFAULT_NODE_COLOR, FORCE_CONFIG, DEFAULT_LIMIT, ENTITY_TYPE_ICONS, ENTITY_TYPE_I18N_KEYS, COMMUNITY_COLORS };

// ============================================================
// 工具函数（纯逻辑，可测试）
// ============================================================

export function getRelationColor(relationType) {
  return RELATION_COLORS[relationType] || DEFAULT_NODE_COLOR;
}

export function getEntityIcon(entityType) {
  return ENTITY_TYPE_ICONS[entityType] || '';
}

export function getEntityTypeName(entityType) {
  const key = ENTITY_TYPE_I18N_KEYS[entityType];
  return key ? t(key) : t('graph.entity_type_unknown');
}

export function buildGraphData(triples) {
  const nodeMap = new Map();
  const links = [];
  const seenLinks = new Set();

  for (const t of triples) {
    if (!nodeMap.has(t.subject)) {
      nodeMap.set(t.subject, { id: t.subject, degree: 0 });
    }
    if (!nodeMap.has(t.object)) {
      nodeMap.set(t.object, { id: t.object, degree: 0 });
    }

    const linkKey = `${t.subject}→${t.object}→${t.relation}`;
    if (!seenLinks.has(linkKey)) {
      seenLinks.add(linkKey);
      links.push({ source: t.subject, target: t.object, relation: t.relation });
      nodeMap.get(t.subject).degree += 1;
      nodeMap.get(t.object).degree += 1;
    }
  }

  return { nodes: Array.from(nodeMap.values()), links };
}

export function getUniqueRelationTypes(links) {
  const types = new Set();
  for (const l of links) {
    types.add(l.relation);
  }
  return Array.from(types);
}

export function isEdgeVisible(relationType, enabledTypes) {
  if (enabledTypes.size === 0) return true;
  return enabledTypes.has(relationType);
}

// ============================================================
// D3 图谱渲染
// ============================================================

/**
 * 渲染 D3 force-directed graph。
 * @param {object} ctx - 渲染上下文（来自 graph-viewer.js 的 state 对象）
 */
export function renderGraph(ctx) {
  const container = document.getElementById('graphCanvasContainer');
  if (!container) return;

  const width = container.clientWidth;
  const height = container.clientHeight;

  const svgEl = document.getElementById('graphSvg');
  svgEl.innerHTML = '';
  svgEl.setAttribute('viewBox', `0 0 ${width} ${height}`);

  ctx._svg = d3.select(svgEl);
  ctx._svgEl = svgEl;

  ctx._container = ctx._svg.append('g').attr('class', 'graph-container');

  ctx._zoom = d3.zoom()
    .scaleExtent([0.1, 4])
    .on('zoom', (event) => {
      ctx._container.attr('transform', event.transform);
    });

  ctx._svg.call(ctx._zoom);

  ctx._simulation = d3.forceSimulation(ctx._graphData.nodes)
    .force('link', d3.forceLink(ctx._graphData.links)
      .id((d) => d.id)
      .distance(FORCE_CONFIG.link_distance))
    .force('charge', d3.forceManyBody().strength(FORCE_CONFIG.charge_strength))
    .force('center', d3.forceCenter(width / 2, height / 2).strength(FORCE_CONFIG.center_strength))
    .force('collision', d3.forceCollide().radius((d) => Math.sqrt(d.degree + 1) * FORCE_CONFIG.collision_radius));

  renderLinks(ctx);
  renderNodes(ctx);
  renderNodeVisuals(ctx);
  bindNodeInteractions(ctx);
  bindSimulationTick(ctx);
}

/** 渲染边 */
function renderLinks(ctx) {
  ctx._linkSel = ctx._container.append('g')
    .attr('class', 'graph-links')
    .selectAll('line')
    .data(ctx._graphData.links)
    .enter()
    .append('line')
    .attr('class', 'graph-edge')
    .attr('stroke', (d) => getRelationColor(d.relation))
    .attr('stroke-width', 1.5);
}

/** 渲染节点（圆形 + 图标 + 标签） */
function renderNodes(ctx) {
  ctx._nodeSel = ctx._container.append('g')
    .attr('class', 'graph-nodes')
    .selectAll('g')
    .data(ctx._graphData.nodes)
    .enter()
    .append('g')
    .attr('class', 'graph-node')
    .attr('data-entity', (d) => d.id)
    .call(d3.drag()
      .on('start', (event, d) => dragStarted(event, d, ctx))
      .on('drag', (event, d) => dragged(event, d))
      .on('end', (event, d) => dragEnded(event, d, ctx)));
}

/** 绑定 simulation tick 更新位置 */
function bindSimulationTick(ctx) {
  ctx._simulation.on('tick', () => {
    ctx._linkSel
      .attr('x1', (d) => d.source.x)
      .attr('y1', (d) => d.source.y)
      .attr('x2', (d) => d.target.x)
      .attr('y2', (d) => d.target.y);

    ctx._nodeSel.attr('transform', (d) => `translate(${d.x},${d.y})`);
  });
}

// ============================================================
// 节点视觉元素渲染
// ============================================================

/** 渲染节点视觉元素（圆形 + 图标 + 标签） */
export function renderNodeVisuals(ctx) {
  if (!ctx._nodeSel) return;

  ctx._nodeSel.append('circle')
    .attr('class', 'graph-node-circle')
    .attr('r', (d) => Math.max(8, Math.sqrt(d.degree + 1) * 6))
    .attr('fill', (d) => {
      if (ctx._communityEnabled && ctx._communityMap[d.id] !== undefined) {
        return COMMUNITY_COLORS[ctx._communityMap[d.id] % COMMUNITY_COLORS.length];
      }
      return '#1a1a20';
    })
    .attr('stroke', (d) => {
      if (ctx._communityEnabled && ctx._communityMap[d.id] !== undefined) {
        return COMMUNITY_COLORS[ctx._communityMap[d.id] % COMMUNITY_COLORS.length];
      }
      return '#38bdf8';
    })
    .attr('stroke-width', 2)
    .style('cursor', 'pointer');

  ctx._nodeSel.each(function (d) {
    const entityType = ctx._entityTypeMap[d.id];
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

  ctx._nodeSel.append('text')
    .attr('class', 'graph-node-label')
    .attr('dy', (d) => Math.max(8, Math.sqrt(d.degree + 1) * 6) + 14)
    .text((d) => {
      const maxLen = 20;
      return d.id.length > maxLen ? d.id.substring(0, maxLen) + '…' : d.id;
    });
}

// ============================================================
// 节点交互事件绑定
// ============================================================

export function bindNodeInteractions(ctx) {
  if (!ctx._nodeSel) return;

  const container = document.getElementById('graphCanvasContainer');

  ctx._nodeSel.on('click', (event, d) => {
    event.stopPropagation();
    highlightNode(ctx, d.id);
  });

  ctx._nodeSel.on('dblclick', (event, d) => {
    event.stopPropagation();
    showEntityDetail(ctx, d.id);
  });

  ctx._nodeSel.on('mouseenter', (event, d) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) {
      const entityType = ctx._entityTypeMap[d.id];
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

  ctx._nodeSel.on('mousemove', (event) => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip && tooltip.classList.contains('graph-tooltip-visible') && container) {
      const rect = container.getBoundingClientRect();
      tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
      tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
    }
  });

  ctx._nodeSel.on('mouseleave', () => {
    const tooltip = document.getElementById('graphTooltip');
    if (tooltip) tooltip.classList.remove('graph-tooltip-visible');
  });

  if (ctx._linkSel) {
    ctx._linkSel.on('mouseenter', (event, d) => {
      const tooltip = document.getElementById('graphTooltip');
      if (tooltip && container) {
        tooltip.innerHTML = `<span class="tooltip-relation">${d.relation}</span><br><span style="color:var(--text-quaternary)">${d.source.id || d.source} → ${d.target.id || d.target}</span>`;
        tooltip.classList.add('graph-tooltip-visible');
        const rect = container.getBoundingClientRect();
        tooltip.style.left = (event.clientX - rect.left + 12) + 'px';
        tooltip.style.top = (event.clientY - rect.top + 12) + 'px';
      }
    });

    ctx._linkSel.on('mouseleave', () => {
      const tooltip = document.getElementById('graphTooltip');
      if (tooltip) tooltip.classList.remove('graph-tooltip-visible');
    });
  }

  if (ctx._svg) {
    ctx._svg.on('click', () => {
      if (ctx._highlightedNode) {
        clearHighlight(ctx);
      }
    });
  }
}

// ============================================================
// 节点高亮
// ============================================================

export function highlightNode(ctx, entityText) {
  ctx._highlightedNode = entityText;

  if (!ctx._container) return;

  const nodes = ctx._container.selectAll('.graph-node');
  const links = ctx._container.selectAll('.graph-edge');

  const connectedNodes = new Set([entityText]);
  for (const l of ctx._graphData.links) {
    const sourceId = typeof l.source === 'object' ? l.source.id : l.source;
    const targetId = typeof l.target === 'object' ? l.target.id : l.target;
    if (sourceId === entityText) connectedNodes.add(targetId);
    if (targetId === entityText) connectedNodes.add(sourceId);
  }

  nodes.classed('graph-node-highlighted', (d) => d.id === entityText);
  nodes.classed('graph-node-dimmed', (d) => !connectedNodes.has(d.id));

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

export function clearHighlight(ctx) {
  ctx._highlightedNode = null;
  if (!ctx._container) return;
  ctx._container.selectAll('.graph-node')
    .classed('graph-node-highlighted', false)
    .classed('graph-node-dimmed', false);
  ctx._container.selectAll('.graph-edge')
    .classed('graph-edge-highlighted', false)
    .classed('graph-edge-dimmed', false);
}

// ============================================================
// 实体详情面板
// ============================================================

export async function showEntityDetail(ctx, entityText) {
  try {
    const relations = await invoke('get_entity_relations', { entityText });

    const panel = document.getElementById('graphDetailPanel');
    if (!panel) return;

    const entityType = ctx._entityTypeMap[entityText] || 'unknown';
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
    ctx._detailPanelEl = panel;

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

export function zoomIn(ctx) {
  if (!ctx._svg || !ctx._zoom) return;
  ctx._svg.transition().call(ctx._zoom.scaleBy, 1.3);
}

export function zoomOut(ctx) {
  if (!ctx._svg || !ctx._zoom) return;
  ctx._svg.transition().call(ctx._zoom.scaleBy, 1 / 1.3);
}

export function resetView(ctx) {
  if (!ctx._svg || !ctx._zoom) return;
  ctx._svg.transition().call(ctx._zoom.transform, d3.zoomIdentity);
  if (ctx._simulation) {
    ctx._simulation.alpha(0.3).restart();
  }
}

// ============================================================
// 图例渲染
// ============================================================

export function renderLegend(ctx) {
  const legend = document.getElementById('graphLegend');
  if (!legend) return;

  const types = getUniqueRelationTypes(ctx._graphData.links);
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
// 子图过滤面板
// ============================================================

export function renderFilterPanel(ctx) {
  const panel = document.getElementById('graphFilterPanel');
  if (!panel) return;

  const types = getUniqueRelationTypes(ctx._graphData.links);
  if (types.length === 0) {
    panel.style.display = 'none';
    return;
  }

  panel.style.display = '';
  let html = `<div class="graph-filter-title">${t('graph.filter_title')}</div>`;
  html += '<div class="graph-filter-list">';
  for (const type of types) {
    const color = getRelationColor(type);
    const checked = isEdgeVisible(type, ctx._enabledRelationTypes) ? 'checked' : '';
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

  const checkboxes = panel.querySelectorAll('.graph-filter-checkbox');
  for (const cb of checkboxes) {
    cb.addEventListener('change', () => {
      const relType = cb.getAttribute('data-relation-type');
      if (cb.checked) {
        ctx._enabledRelationTypes.delete(relType);
      } else {
        ctx._enabledRelationTypes.add(relType);
      }
      applyFilter(ctx);
    });
  }
}

export function applyFilter(ctx) {
  if (!ctx._container) return;

  const links = ctx._container.selectAll('.graph-edge');
  const nodes = ctx._container.selectAll('.graph-node');

  links.classed('graph-edge-hidden', (d) => !isEdgeVisible(d.relation, ctx._enabledRelationTypes));

  const nodeHasVisibleEdge = new Set();
  for (const l of ctx._graphData.links) {
    if (isEdgeVisible(l.relation, ctx._enabledRelationTypes)) {
      const sourceId = typeof l.source === 'object' ? l.source.id : l.source;
      const targetId = typeof l.target === 'object' ? l.target.id : l.target;
      nodeHasVisibleEdge.add(sourceId);
      nodeHasVisibleEdge.add(targetId);
    }
  }

  nodes.classed('graph-node-hidden', (d) => {
    if (d.degree === 0) return false;
    return !nodeHasVisibleEdge.has(d.id);
  });
}

// ============================================================
// 搜索定位
// ============================================================

export function searchAndLocate(ctx, query) {
  if (!query || !query.trim() || !ctx._graphData.nodes.length) return;

  const lowerQuery = query.trim().toLowerCase();
  const matched = ctx._graphData.nodes.filter((n) =>
    n.id.toLowerCase().includes(lowerQuery)
  );

  if (matched.length === 0) {
    clearSearchHighlight(ctx);
    return;
  }

  clearSearchHighlight(ctx);

  if (ctx._container) {
    ctx._container.selectAll('.graph-node')
      .classed('graph-node-searched', (d) =>
        matched.some((m) => m.id === d.id)
      );
  }

  const target = matched[0];
  if (target.x !== undefined && target.y !== undefined && ctx._svg && ctx._zoom) {
    const container = document.getElementById('graphCanvasContainer');
    if (container) {
      const width = container.clientWidth;
      const height = container.clientHeight;
      const scale = 1.5;
      const tx = width / 2 - target.x * scale;
      const ty = height / 2 - target.y * scale;
      ctx._svg.transition()
        .duration(500)
        .call(ctx._zoom.transform, d3.zoomIdentity.translate(tx, ty).scale(scale));
    }
  }
}

export function clearSearchHighlight(ctx) {
  if (!ctx._container) return;
  ctx._container.selectAll('.graph-node').classed('graph-node-searched', false);
}

// ============================================================
// 图谱导出
// ============================================================

export function exportSvg(ctx) {
  if (!ctx._svgEl) return;

  try {
    const clone = ctx._svgEl.cloneNode(true);
    clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    clone.setAttribute('xmlns:xlink', 'http://www.w3.org/1999/xlink');

    const serializer = new XMLSerializer();
    let svgStr = serializer.serializeToString(clone);
    svgStr = '<?xml version="1.0" encoding="UTF-8"?>\n' + svgStr;

    const blob = new Blob([svgStr], { type: 'image/svg+xml;charset=utf-8' });
    downloadBlob(blob, 'knowledge-graph.svg');
  } catch {
    // 降级：静默
  }
}

export function exportPng(ctx) {
  if (!ctx._svgEl) return;

  try {
    const container = document.getElementById('graphCanvasContainer');
    if (!container) return;

    const width = container.clientWidth;
    const height = container.clientHeight;

    const clone = ctx._svgEl.cloneNode(true);
    clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
    clone.setAttribute('width', String(width));
    clone.setAttribute('height', String(height));

    const serializer = new XMLSerializer();
    const svgStr = serializer.serializeToString(clone);
    const svgBlob = new Blob([svgStr], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(svgBlob);

    const img = new Image();
    img.onload = function () {
      const canvas = document.createElement('canvas');
      const scale = 2;
      canvas.width = width * scale;
      canvas.height = height * scale;
      const ctx2d = canvas.getContext('2d');
      if (ctx2d) {
        ctx2d.fillStyle = '#0f1115';
        ctx2d.fillRect(0, 0, canvas.width, canvas.height);
        ctx2d.scale(scale, scale);
        ctx2d.drawImage(img, 0, 0);
      }
      URL.revokeObjectURL(url);

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

export async function exportGraphData(format) {
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
  } catch {
    // 降级：静默
  }
}

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
// 布局切换
// ============================================================

export async function switchLayout(ctx, mode) {
  if (mode === ctx._currentLayout) return;

  const validModes = ['force', 'hierarchical', 'radial'];
  if (!validModes.includes(mode)) return;

  try {
    await graphApi.getLayout();
  } catch {
    // 静默降级
  }

  const btns = document.querySelectorAll('.graph-layout-btn');
  for (const btn of btns) {
    btn.classList.toggle('graph-layout-active', btn.getAttribute('data-layout') === mode);
  }

  ctx._currentLayout = mode;

  if (ctx._simulation) {
    ctx._simulation.stop();
    ctx._simulation = null;
  }

  if (mode === 'force') {
    renderForceLayout(ctx);
  } else if (mode === 'hierarchical') {
    renderHierarchicalLayout(ctx, false);
  } else if (mode === 'radial') {
    renderHierarchicalLayout(ctx, true);
  }
}

function renderForceLayout(ctx) {
  const container = document.getElementById('graphCanvasContainer');
  if (!container || !ctx._svg) return;

  const width = container.clientWidth;
  const height = container.clientHeight;

  ctx._container.selectAll('*').remove();

  ctx._simulation = d3.forceSimulation(ctx._graphData.nodes)
    .force('link', d3.forceLink(ctx._graphData.links)
      .id((d) => d.id)
      .distance(FORCE_CONFIG.link_distance))
    .force('charge', d3.forceManyBody().strength(FORCE_CONFIG.charge_strength))
    .force('center', d3.forceCenter(width / 2, height / 2).strength(FORCE_CONFIG.center_strength))
    .force('collision', d3.forceCollide().radius((d) => Math.sqrt(d.degree + 1) * FORCE_CONFIG.collision_radius));

  reRenderNodesAndLinks(ctx);
}

function renderHierarchicalLayout(ctx, radial) {
  const container = document.getElementById('graphCanvasContainer');
  if (!container || !ctx._svg) return;

  const width = container.clientWidth;
  const height = container.clientHeight;

  ctx._container.selectAll('*').remove();

  const rootNode = ctx._graphData.nodes.reduce((max, n) => (n.degree > max.degree ? n : max), ctx._graphData.nodes[0]);

  const adjacency = {};
  for (const n of ctx._graphData.nodes) {
    adjacency[n.id] = [];
  }
  for (const l of ctx._graphData.links) {
    const s = typeof l.source === 'object' ? l.source.id : l.source;
    const t = typeof l.target === 'object' ? l.target.id : l.target;
    if (adjacency[s]) adjacency[s].push(t);
    if (adjacency[t]) adjacency[t].push(s);
  }

  const visited = new Set();
  const buildTree = (nodeId) => {
    visited.add(nodeId);
    const children = (adjacency[nodeId] || [])
      .filter((child) => !visited.has(child))
      .map((child) => buildTree(child));
    return { id: nodeId, children };
  };

  const treeData = buildTree(rootNode.id);

  const root = d3.hierarchy(treeData);
  const treeLayout = d3.tree().size(
    radial ? [2 * Math.PI, Math.min(width, height) / 2 - 50] : [width - 100, height - 100]
  );
  treeLayout(root);

  const layoutNodes = root.descendants();
  const nodeMap = new Map(layoutNodes.map((n) => [n.data.id, n]));

  for (const n of ctx._graphData.nodes) {
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

  ctx._linkSel = ctx._container.append('g')
    .attr('class', 'graph-links')
    .selectAll('line')
    .data(ctx._graphData.links)
    .enter()
    .append('line')
    .attr('class', 'graph-edge')
    .attr('stroke', (d) => getRelationColor(d.relation))
    .attr('stroke-width', 1.5)
    .attr('x1', (d) => {
      const s = typeof d.source === 'object' ? d.source : ctx._graphData.nodes.find((n) => n.id === d.source);
      return s ? s.x : 0;
    })
    .attr('y1', (d) => {
      const s = typeof d.source === 'object' ? d.source : ctx._graphData.nodes.find((n) => n.id === d.source);
      return s ? s.y : 0;
    })
    .attr('x2', (d) => {
      const t = typeof d.target === 'object' ? d.target : ctx._graphData.nodes.find((n) => n.id === d.target);
      return t ? t.x : 0;
    })
    .attr('y2', (d) => {
      const t = typeof d.target === 'object' ? d.target : ctx._graphData.nodes.find((n) => n.id === d.target);
      return t ? t.y : 0;
    });

  ctx._nodeSel = ctx._container.append('g')
    .attr('class', 'graph-nodes')
    .selectAll('g')
    .data(ctx._graphData.nodes)
    .enter()
    .append('g')
    .attr('class', 'graph-node')
    .attr('data-entity', (d) => d.id)
    .attr('transform', (d) => `translate(${d.x},${d.y})`)
    .call(d3.drag()
      .on('start', (event) => { event.subject.fx = event.subject.x; event.subject.fy = event.subject.y; })
      .on('drag', (event, d) => { d.x = event.x; d.y = event.y; d3.select(event.sourceEvent.target.closest('.graph-node')).attr('transform', `translate(${d.x},${d.y})`); })
      .on('end', (event, d) => { d.fx = null; d.fy = null; }));

  renderNodeVisuals(ctx);
  bindNodeInteractions(ctx);
}

function reRenderNodesAndLinks(ctx) {
  ctx._linkSel = ctx._container.append('g')
    .attr('class', 'graph-links')
    .selectAll('line')
    .data(ctx._graphData.links)
    .enter()
    .append('line')
    .attr('class', 'graph-edge')
    .attr('stroke', (d) => getRelationColor(d.relation))
    .attr('stroke-width', 1.5);

  ctx._nodeSel = ctx._container.append('g')
    .attr('class', 'graph-nodes')
    .selectAll('g')
    .data(ctx._graphData.nodes)
    .enter()
    .append('g')
    .attr('class', 'graph-node')
    .attr('data-entity', (d) => d.id)
    .call(d3.drag()
      .on('start', (event, d) => dragStarted(event, d, ctx))
      .on('drag', (event, d) => dragged(event, d))
      .on('end', (event, d) => dragEnded(event, d, ctx)));

  renderNodeVisuals(ctx);
  bindNodeInteractions(ctx);

  ctx._simulation.on('tick', () => {
    ctx._linkSel
      .attr('x1', (d) => d.source.x)
      .attr('y1', (d) => d.source.y)
      .attr('x2', (d) => d.target.x)
      .attr('y2', (d) => d.target.y);

    ctx._nodeSel.attr('transform', (d) => `translate(${d.x},${d.y})`);
  });
}

// ============================================================
// 路径分析
// ============================================================

export function renderPathSelectors(ctx) {
  const fromSelect = document.getElementById('graphPathFrom');
  const toSelect = document.getElementById('graphPathTo');
  if (!fromSelect || !toSelect) return;

  const sortedNodes = [...ctx._graphData.nodes].sort((a, b) => a.id.localeCompare(b.id));

  fromSelect.innerHTML = `<option value="">${t('graph.path_from')}</option>`;
  toSelect.innerHTML = `<option value="">${t('graph.path_to')}</option>`;

  for (const n of sortedNodes) {
    fromSelect.appendChild(new Option(n.id, n.id));
    toSelect.appendChild(new Option(n.id, n.id));
  }

  fromSelect.addEventListener('change', () => {
    ctx._pathFromEntity = fromSelect.value || null;
  });
  toSelect.addEventListener('change', () => {
    ctx._pathToEntity = toSelect.value || null;
  });
}

export async function findPath(ctx) {
  if (!ctx._pathFromEntity || !ctx._pathToEntity) {
    const result = document.getElementById('graphPathResult');
    if (result) result.textContent = t('graph.path_select_both');
    return;
  }

  const result = document.getElementById('graphPathResult');
  if (result) result.textContent = t('graph.path_searching');

  try {
    const pathResult = await invoke('get_shortest_path', {
      from: ctx._pathFromEntity,
      to: ctx._pathToEntity,
    });

    if (!pathResult.path || pathResult.path.length === 0) {
      if (result) result.textContent = t('graph.path_no_result');
      ctx._pathHighlightNodes = new Set();
      applyPathHighlight(ctx);
      return;
    }

    const pathStr = pathResult.path.join(' → ');
    if (result) {
      result.innerHTML = `${t('graph.path_length')}: ${pathResult.hops} ${t('graph.path_hops')}<br><span class="path-detail">${pathStr}</span>`;
    }

    ctx._pathHighlightNodes = new Set(pathResult.path);
    applyPathHighlight(ctx);
  } catch {
    if (result) result.textContent = t('graph.path_error');
  }
}

export function applyPathHighlight(ctx) {
  if (!ctx._container) return;

  const nodes = ctx._container.selectAll('.graph-node');
  const links = ctx._container.selectAll('.graph-edge');

  if (ctx._pathHighlightNodes.size === 0) {
    nodes.classed('graph-node-on-path', false).classed('graph-node-dimmed', false);
    links.classed('graph-edge-on-path', false).classed('graph-edge-dimmed', false);
    return;
  }

  nodes.classed('graph-node-on-path', (d) => ctx._pathHighlightNodes.has(d.id));
  nodes.classed('graph-node-dimmed', (d) => !ctx._pathHighlightNodes.has(d.id));

  links.classed('graph-edge-on-path', (d) => {
    const s = typeof d.source === 'object' ? d.source.id : d.source;
    const t = typeof d.target === 'object' ? d.target.id : d.target;
    const pathArray = Array.from(ctx._pathHighlightNodes);
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
    const pathArray = Array.from(ctx._pathHighlightNodes);
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
// 社区检测
// ============================================================

export async function toggleCommunities(ctx) {
  if (ctx._communityEnabled) {
    ctx._communityEnabled = false;
    ctx._communityMap = {};
    const countEl = document.getElementById('graphCommunityCount');
    if (countEl) countEl.textContent = '';
    if (ctx._container) {
      ctx._container.selectAll('.graph-node-circle')
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

    ctx._communityMap = result.communities;
    ctx._communityEnabled = true;

    if (countEl) {
      countEl.textContent = `${result.community_count} ${t('graph.community_count_label')}`;
    }

    if (ctx._container) {
      ctx._container.selectAll('.graph-node-circle')
        .attr('fill', (d) => {
          const cid = ctx._communityMap[d.id];
          if (cid !== undefined) {
            return COMMUNITY_COLORS[cid % COMMUNITY_COLORS.length];
          }
          return '#1a1a20';
        })
        .attr('stroke', (d) => {
          const cid = ctx._communityMap[d.id];
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

function dragStarted(event, d, ctx) {
  if (ctx._simulation && !event.active) ctx._simulation.alphaTarget(0.3).restart();
  d.fx = d.x;
  d.fy = d.y;
}

function dragged(event, d) {
  d.fx = event.x;
  d.fy = event.y;
}

function dragEnded(event, d, ctx) {
  if (ctx._simulation && !event.active) ctx._simulation.alphaTarget(0);
  d.fx = null;
  d.fy = null;
}
