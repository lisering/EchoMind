/**
 * EchoMind 上下文窗口指示器模块 — 输入框上方细条形进度条。
 *
 * 职责：
 * 1. 根据已用 token 数 / 模型上下文窗口计算用量百分比
 * 2. 绿色（0-60% 正常）/ 黄色（60-80% 注意）/ 红色（80-100% 建议新建会话）
 * 3. 渲染进度条 DOM + tooltip
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.8 上下文窗口指示
 * AC-QA-009：输入框上方显示上下文用量指示器
 */

import { get, setState } from './state.js';
import { t } from './i18n.js';

// ============================================================
// 常量
// ============================================================

/** 默认上下文窗口大小（token 数），部分模型如 GPT-4o 为 128K，这里用保守的 8K 作为默认。 */
const DEFAULT_CONTEXT_LIMIT = 8000;

// ============================================================
// 纯函数：百分比计算与颜色级别
// ============================================================

/**
 * 计算上下文用量百分比（整数，向下取整）。
 *
 * @param {number} used - 已用 token 数
 * @param {number} limit - 模型上下文窗口大小
 * @returns {number} 0-100 的整数百分比
 */
export function getContextPercentage(used, limit) {
  if (!limit || limit <= 0) return 0;
  const pct = (used / limit) * 100;
  return Math.max(0, Math.min(100, Math.round(pct)));
}

/**
 * 根据用量百分比返回颜色级别。
 *
 * - 0-60%：green（正常）
 * - 60-80%：yellow（注意）
 * - 80-100%+：red（建议新建会话）
 *
 * @param {number} used - 已用 token 数
 * @param {number} limit - 模型上下文窗口大小
 * @returns {'green'|'yellow'|'red'} 颜色级别
 */
export function getContextLevel(used, limit) {
  // 使用原始百分比（非四舍五入）进行级别判断，避免边界值误判
  if (!limit || limit <= 0) return 'green';
  const rawPct = (used / limit) * 100;
  if (rawPct > 80) return 'red';     // >80-100% → red (80% inclusive 为 yellow)
  if (rawPct > 60) return 'yellow';  // >60-80% → yellow (80% inclusive)
  return 'green';                    // 0-60% → green (60% inclusive)
}

// ============================================================
// DOM 渲染
// ============================================================

/**
 * 渲染上下文窗口指示器到指定容器。
 *
 * 在容器中创建 `.context-bar` 元素，含进度条填充和 tooltip。
 * 颜色级别由 getContextLevel 计算。
 *
 * @param {HTMLElement} container - 挂载容器
 * @param {number} used - 已用 token 数
 * @param {number} limit - 模型上下文窗口大小
 * @returns {HTMLElement|null} 创建的 .context-bar 元素（limit=0 时返回 null）
 */
export function renderContextBar(container, used, limit) {
  // 清除已有元素
  const existing = container.querySelector('.context-bar');
  if (existing) existing.remove();

  // limit=0 时不渲染（避免除零，表示无上下文限制数据）
  if (!limit || limit <= 0) return null;

  const pct = getContextPercentage(used, limit);
  const level = getContextLevel(used, limit);
  const tooltip = formatContextTooltip(used, limit);

  const bar = document.createElement('div');
  bar.className = `context-bar context-bar-${level}`;
  bar.title = tooltip;
  bar.dataset.pct = String(pct);

  const fill = document.createElement('div');
  fill.className = 'context-bar-fill';
  fill.style.width = `${pct}%`;

  bar.appendChild(fill);
  container.appendChild(bar);
  return bar;
}

// ============================================================
// Tooltip 格式化
// ============================================================

/**
 * 格式化上下文用量 tooltip 文本。
 *
 * 包含已用 token 数 / 模型窗口大小 / 消息条数。
 *
 * @param {number} used - 已用 token 数
 * @param {number} limit - 模型上下文窗口大小
 * @param {number} [messageCount=0] - 当前对话消息条数
 * @returns {string} tooltip 文本
 */
export function formatContextTooltip(used, limit, messageCount = 0) {
  const pct = getContextPercentage(used, limit);
  const parts = [`${used} / ${limit} tokens (${pct}%)`];
  if (messageCount > 0) {
    parts.push(`${messageCount} messages`);
  }
  const hint = t('chat.context_tooltip_hint');
  if (hint && hint !== 'chat.context_tooltip_hint') {
    parts.push(hint);
  }
  return parts.join(' · ');
}

// ============================================================
// 状态更新
// ============================================================

/**
 * 更新上下文用量状态并重新渲染指示器。
 *
 * @param {number} used - 已用 token 数
 * @param {number} limit - 模型上下文窗口大小
 */
export function updateContextUsage(used, limit) {
  setState({
    contextTokens: used,
    contextLimit: limit || DEFAULT_CONTEXT_LIMIT,
  });

  // 尝试在输入框上方渲染/更新
  let container = document.querySelector('.context-bar-container');
  if (container) {
    // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
    renderContextBar(container, used, limit || DEFAULT_CONTEXT_LIMIT);
  }
}
