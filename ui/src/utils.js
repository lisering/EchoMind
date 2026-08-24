/**
 * EchoMind 工具函数模块 — 纯函数集合 + 骨架屏 + 日期格式化。
 *
 * 设计原则：
 * 1. 纯函数 — 输入 → 输出，无副作用，可独立单元测试
 * 2. 单一职责 — 每个函数只做一件事
 *
 * 已合并模块：
 * - skeleton.js（骨架屏）
 * - date-utils.js（日期时间本地化）
 */

import { t } from './i18n.js';

/**
 * 按 id 获取 DOM 元素简写助手。
 *
 * 返回 any 类型以兼容 vanilla JS 中对 .value / .disabled / .dataset
 * 等属性的访问（渐进式类型迁移：后续可逐步替换为具体类型）。
 *
 * @param {string} id - 元素 ID（不含 # 前缀）
 * @returns {any} 对应的 DOM 元素
 */
export function $(id) {
  return document.getElementById(id);
}

/**
 * 脱敏错误消息：过滤 API Key、文件路径中的用户名等敏感信息（REQ-UI-005-AC-2）。
 * @param {string|Error} err - 原始错误对象或字符串
 * @returns {string} 脱敏后的安全错误消息
 */
export function sanitizeError(err) {
  let msg = String(err);
  // 过滤 API Key（sk-xxx 格式，保留前缀 + 末四位）
  msg = msg.replace(/sk-[a-zA-Z0-9]{8,}/g, 'sk-****');
  // 过滤 Windows 用户路径
  msg = msg.replace(/\\Users\\[^\\]+?\\/g, '\\Users\\****\\');
  // 过滤 Unix 用户路径
  msg = msg.replace(/\/Users\/[^/]+?\//g, '/Users/****/');
  return msg;
}

/**
 * 从入库副本路径中剥离 MD5 哈希前缀，返回展示用文件名。
 * 入库副本命名规则 {md5}-{原始文件名}（REQ-ING-001）。
 * @param {string} filePath - 入库后的本地路径
 * @returns {string} 去除哈希前缀后的文件名
 */
export function displayDocName(filePath) {
  const base = filePath.split('/').pop() || filePath;
  return base.length > 33 && base[32] === '-' ? base.slice(33) : base;
}

/**
 * 从文档对象安全提取状态字符串（后端枚举或字符串形式均兼容）。
 * @param {Object} d - 文档对象
 * @returns {string} 状态标识（Pending/Processing/Indexed/Failed）
 */
export function docStatusOf(d) {
  return typeof d.status === 'string' ? d.status : 'Failed';
}

/**
 * 格式化字节大小（REQ-VEC-008 / REQ-I18N-003）。
 * 支持 B / KB / MB / GB / TB，1024 进制，保留 1 位小数。
 * @param {number} bytes - 字节数
 * @returns {string} 人类可读的大小字符串（如 "32.5 MB"、"2.0 GB"）
 */
export function formatBytes(bytes) {
  if (bytes < 1024) return bytes + ' B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const idx = Math.min(i, units.length - 1);
  return (bytes / Math.pow(1024, idx)).toFixed(1) + ' ' + units[idx];
}

/**
 * 格式化数字为千分位分隔符字符串（REQ-I18N-003）。
 * @param {number} num - 数字
 * @returns {string} 千分位分隔字符串（如 "1,234,567"）
 */
export function formatNumber(num) {
  if (num === null || num === undefined) return '';
  if (typeof num !== 'number' || !isFinite(num)) return String(num);
  return num.toLocaleString('en-US');
}

/**
 * 格式化百分比（REQ-I18N-003）。
 * @param {number} value - 0~1 的小数（如 0.873）或 0~100 的整数
 * @returns {string} 整数百分比字符串（如 "87%"）
 */
export function formatPercent(value) {
  if (value === null || value === undefined || typeof value !== 'number') return '';
  const pct = value <= 1 ? Math.round(value * 100) : Math.round(value);
  return pct + '%';
}

/**
 * 从路径提取文件名。
 * @param {string} p - 文件路径
 * @returns {string} 文件名（最后一段）
 */
export function basename(p) {
  return p.split('/').pop() || p;
}

/**
 * 从文件名提取扩展名（小写）。
 * @param {string} p - 文件路径
 * @returns {string} 扩展名（不含点），如 "md"
 */
export function extname(p) {
  const name = basename(p);
  const dot = name.lastIndexOf('.');
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
}

/**
 * 检查当前焦点是否在输入元素上（用于全局快捷键不触发判断）。
 * @returns {boolean} 当前焦点是否在 input/textarea/contenteditable 上
 */
export function isInputFocused() {
  const el = document.activeElement;
  if (!el) return false;
  return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable === true;
}

/**
 * 将多模态管线子阶段标识转换为可读文案（REQ-MM-004）。
 * @param {string} phase - 子阶段标识
 * @returns {string} 可读文案
 */
export function getSubPhaseLabel(phase) {
  const labelKeys = {
    text_extracting: 'doc_phases.text_extracting',
    image_rendering: 'doc_phases.image_rendering',
    ocr: 'doc_phases.ocr',
    vlm_enhancing: 'doc_phases.vlm_enhancing',
  };
  return labelKeys[phase] || phase;
}

/**
 * 文档状态 → 标签 + 样式映射表。
 */
export const DOC_STATUS_STYLE = {
  Pending: ['status_pending', 'text-slate-400 border-slate-500/40'],
  Processing: ['status_processing', 'text-amber-300 border-amber-400/40'],
  Indexed: ['status_indexed', 'text-accent border-accent/40'],
  Failed: ['status_failed', 'text-red-400 border-red-400/40'],
};

/**
 * Provider 预设配置表（REQ-LLM-003）。
 *
 * 所有预设均为 OpenAI 兼容端点。base_url 路径格式由后端
 * `resolve_chat_completions_url()` 智能拼接，兼容标准格式、
 * 版本路径（如 /v4）和完整端点路径。
 */
export const PRESETS = {
  deepseek: { label: 'DeepSeek', base_url: 'https://api.deepseek.com', model: 'deepseek-chat', keyUrl: 'https://platform.deepseek.com/api_keys', needKey: true, descKey: 'presets.deepseek_desc' },
  openai: { label: 'OpenAI', base_url: 'https://api.openai.com', model: 'gpt-4o-mini', keyUrl: 'https://platform.openai.com/api-keys', needKey: true, descKey: 'presets.openai_desc' },
  qwen: { label: 'Qwen', base_url: 'https://dashscope.aliyuncs.com/compatible-mode', model: 'qwen-plus', keyUrl: 'https://dashscope.console.aliyun.com/apiKey', needKey: true, descKey: 'presets.qwen_desc' },
  kimi: { label: 'Kimi', base_url: 'https://api.moonshot.cn', model: 'moonshot-v1-8k', keyUrl: 'https://platform.moonshot.cn/console/api-keys', needKey: true, descKey: 'presets.kimi_desc' },
  glm: { label: 'GLM', base_url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4-flash', keyUrl: 'https://open.bigmodel.cn/usercenter/apikeys', needKey: true, descKey: 'presets.glm_desc' },
  minimax: { label: 'MiniMax', base_url: 'https://api.minimax.chat', model: 'MiniMax-Text-01', keyUrl: 'https://platform.minimaxi.com/user-center/basic-information/interface-key', needKey: true, descKey: 'presets.minimax_desc' },
  mistral: { label: 'Mistral', base_url: 'https://api.mistral.ai', model: 'mistral-large-latest', keyUrl: 'https://console.mistral.ai/api-keys', needKey: true, descKey: 'presets.mistral_desc' },
  grok: { label: 'Grok', base_url: 'https://api.x.ai', model: 'grok-3', keyUrl: 'https://console.x.ai', needKey: true, descKey: 'presets.grok_desc' },
  ollama: { label: 'Ollama', base_url: 'http://localhost:11434', model: 'llama3.1', keyUrl: 'https://ollama.com/library', needKey: false, descKey: 'presets.ollama_desc' },
  custom: { label: 'Custom', base_url: '', model: '', keyUrl: '', needKey: true, descKey: 'presets.custom_desc' },
};

/** 默认工作空间 ID */
export const WORKSPACE = 'default';

/**
 * 统一剪贴板复制工具（REQ-IX-003 AC-5）。
 *
 * 处理非安全上下文检测（`window.isSecureContext === false`）和
 * `navigator.clipboard` API 不可用时的 fallback。
 *
 * 调用方负责根据返回值显示 toast（成功 → toast「已复制」，失败 → toastError「复制失败」）。
 *
 * @param {string} text - 要复制的文本
 * @returns {Promise<boolean>} 是否复制成功
 */
export async function copyToClipboard(text) {
  // 非安全上下文或 clipboard API 不可用 → 使用 execCommand fallback
  if (!window.isSecureContext || !navigator.clipboard || !navigator.clipboard.writeText) {
    try {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.top = '-9999px';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      const ok = document.execCommand('copy');
      document.body.removeChild(textarea);
      return ok;
    } catch (_) {
      return false;
    }
  }

  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (_) {
    return false;
  }
}

// ============================================================
// 骨架屏（原 skeleton.js 已合并到本模块）
// ============================================================

/** Skeleton 定时器映射（container id -> timeout handle） */
const _skeletonTimers = new Map();

/** Skeleton 容器 class */
const SKELETON_CLASS = 'skeleton-container';

/**
 * 创建单个骨架占位项。
 * @param {'doc'|'conv'} type - 骨架类型（文档/会话）
 * @returns {HTMLElement} 骨架项 DOM
 */
function createSkeletonItem(type) {
  const item = document.createElement('div');
  item.className = 'flex items-center gap-2 px-3 py-2';

  const nameBlock = document.createElement('div');
  nameBlock.className = 'h-3 rounded bg-white/5 animate-pulse';
  nameBlock.style.width = type === 'doc' ? '120px' : '160px';
  item.appendChild(nameBlock);

  const badgeBlock = document.createElement('div');
  badgeBlock.className = 'h-3 w-10 rounded bg-white/5 animate-pulse';
  item.appendChild(badgeBlock);

  return item;
}

/**
 * 在指定容器中显示骨架屏（200ms 延迟后插入 DOM）。
 * @param {HTMLElement} container - 目标容器元素
 * @param {'doc'|'conv'} [type='doc'] - 骨架类型
 * @param {number} [count=4] - 骨架项数量
 */
export function showSkeleton(container, type = 'doc', count = 4) {
  if (!container) return;

  hideSkeleton(container);

  const timer = setTimeout(() => {
    if (container.children.length > 0) return;

    const skeleton = document.createElement('div');
    skeleton.className = SKELETON_CLASS;

    for (let i = 0; i < count; i++) {
      skeleton.appendChild(createSkeletonItem(type));
    }

    container.appendChild(skeleton);
  }, 200);

  const key = container.id || `container-${Date.now()}`;
  _skeletonTimers.set(key, timer);
}

/**
 * 移除指定容器中的骨架屏。
 * @param {HTMLElement} container - 目标容器元素
 */
export function hideSkeleton(container) {
  if (!container) return;

  const key = container.id || '';
  if (key && _skeletonTimers.has(key)) {
    clearTimeout(_skeletonTimers.get(key));
    _skeletonTimers.delete(key);
  }

  const skeleton = container.querySelector(`.${SKELETON_CLASS}`);
  if (skeleton) {
    skeleton.remove();
  }
}

// ============================================================
// 日期时间本地化（原 date-utils.js 已合并到本模块）
// ============================================================

/**
 * 将时间戳格式化为「YYYY-MM-DD HH:mm」格式。
 * @param {number|string|Date} timestamp
 * @returns {string}
 */
export function formatDate(timestamp) {
  if (!timestamp) return '';
  const date = _toDate(timestamp);
  if (!date) return '';

  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  const h = String(date.getHours()).padStart(2, '0');
  const min = String(date.getMinutes()).padStart(2, '0');

  return `${y}-${m}-${d} ${h}:${min}`;
}

/**
 * 将时间戳格式化为相对时间。
 * @param {number|string|Date} timestamp
 * @returns {string}
 */
export function formatRelativeTime(timestamp) {
  if (!timestamp) return '';
  const date = _toDate(timestamp);
  if (!date) return '';

  const now = Date.now();
  const diffMs = now - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffMs < 0) return formatDate(timestamp);

  if (diffSec < 180) return t('date.just_now');
  if (diffMin < 60) return t('date.minutes_ago', { n: diffMin });
  if (diffHour < 24) return t('date.hours_ago', { n: diffHour });
  if (diffDay < 7) return t('date.days_ago', { n: diffDay });

  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, '0');
  const d = String(date.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

/**
 * 格式化文件大小为人类可读格式。
 * @param {number} bytes
 * @returns {string}
 */
export function formatFileSize(bytes) {
  if (bytes === null || bytes === undefined || bytes < 0) return '';
  if (bytes < 1024) return `${bytes} B`;

  const units = ['KB', 'MB', 'GB', 'TB'];
  let size = bytes / 1024;
  let unitIdx = 0;

  while (size >= 1024 && unitIdx < units.length - 1) {
    size /= 1024;
    unitIdx++;
  }

  return `${size.toFixed(1)} ${units[unitIdx]}`;
}

// formatNumber 和 formatPercent 已在上方定义（utils.js 原有）

/**
 * 将各种时间输入转换为 Date 对象。
 * @param {number|string|Date} input
 * @returns {Date|null}
 */
function _toDate(input) {
  if (input instanceof Date) return input;
  if (typeof input === 'number') {
    const ms = input < 1e12 ? input * 1000 : input;
    const date = new Date(ms);
    return isNaN(date.getTime()) ? null : date;
  }
  if (typeof input === 'string') {
    const num = Number(input);
    if (!isNaN(num)) return _toDate(num);
    const date = new Date(input);
    return isNaN(date.getTime()) ? null : date;
  }
  return null;
}

// 暴露为全局（E2E 测试需要）
window.__formatDate = formatDate;
window.__formatRelativeTime = formatRelativeTime;
window.__formatFileSize = formatFileSize;
window.__formatNumber = formatNumber;

// ============================================================
// SVG 图标系统（原 icons.js，REQ-DS-004）
// ============================================================

const SVG_ATTRS = 'fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"';

const ICON_PATHS = {
  plus: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>`,
  collapse: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><polyline points="15 18 9 12 15 6"/></svg>`,
  expand: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><polyline points="9 18 15 12 9 6"/></svg>`,
  chat: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.6a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`,
  stop: `<svg viewBox="0 0 24 24" fill="currentColor" stroke="none"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>`,
  drag: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`,
  brand: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M12 2L22 12L12 22L2 12Z"/></svg>`,
  close: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`,
  retry: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>`,
  search: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>`,
  eye: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>`,
  summary: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="9" y1="13" x2="15" y2="13"/><line x1="9" y1="17" x2="13" y2="17"/></svg>`,
  tag: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/><line x1="7" y1="7" x2="7.01" y2="7"/></svg>`,
  copy: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`,
  trash: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>`,
  warning: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>`,
  info: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>`,
  lock: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>`,
  unlock: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 9.9-1"/></svg>`,
  check: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><polyline points="20 6 9 17 4 12"/></svg>`,
  cloud: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M17.5 19a4.5 4.5 0 1 0 0-9h-1.8A7 7 0 1 0 4 14"/></svg>`,
  download: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`,
  book: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>`,
  shield: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>`,
  clipboard: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg>`,
  keyboard: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><rect x="2" y="4" width="20" height="16" rx="2" ry="2"/><path d="M6 8h.01M10 8h.01M14 8h.01M18 8h.01M8 12h.01M12 12h.01M16 12h.01M7 16h10"/></svg>`,
  graph: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><circle cx="12" cy="5" r="2"/><circle cx="5" cy="19" r="2"/><circle cx="19" cy="19" r="2"/><line x1="12" y1="7" x2="6" y2="17"/><line x1="12" y1="7" x2="18" y2="17"/><line x1="6" y1="19" x2="18" y2="19"/></svg>`,
  memory: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M9.5 2A2.5 2.5 0 0 1 12 4.5v15a2.5 2.5 0 0 1-4.96.44 2.5 2.5 0 0 1-2.96-3.08 3 3 0 0 1-.34-5.58 2.5 2.5 0 0 1 1.32-4.24 2.5 2.5 0 0 1 1.98-3A2.5 2.5 0 0 1 9.5 2Z"/><path d="M14.5 2A2.5 2.5 0 0 0 12 4.5v15a2.5 2.5 0 0 0 4.96.44 2.5 2.5 0 0 0 2.96-3.08 3 3 0 0 0 .34-5.58 2.5 2.5 0 0 0-1.32-4.24 2.5 2.5 0 0 0-1.98-3A2.5 2.5 0 0 0 14.5 2Z"/></svg>`,
  trace: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>`,
  chart: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/></svg>`,
  chevronRight: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><polyline points="9 18 15 12 9 6"/></svg>`,
  mic: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/></svg>`,
  send: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>`,
  globe: `<svg viewBox="0 0 24 24" ${SVG_ATTRS}><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>`,
};

const SIZE_CLASSES = {
  sm: 'icon-sm',
  md: 'icon-md',
  lg: 'icon-lg',
};

/**
 * 获取指定图标的 SVG HTML 字符串（带尺寸 CSS 类）。
 */
export function icon(name, size = 'sm') {
  const svg = ICON_PATHS[name];
  if (!svg) return '';
  const cls = SIZE_CLASSES[size] || SIZE_CLASSES.sm;
  return svg.replace('<svg ', `<svg class="${cls}" `);
}

/**
 * 获取图标的纯 SVG HTML 字符串（不含尺寸类）。
 */
export function iconRaw(name) {
  return ICON_PATHS[name] || '';
}

/**
 * 获取文件类型图标。
 */
export function fileIcon(ext, size = 'sm') {
  return icon('summary', size);
}

/**
 * 获取所有可用图标名称列表。
 */
export function listIcons() {
  return Object.keys(ICON_PATHS);
}

/**
 * 使可点击元素键盘可达（V3.1 P3-6 / WCAG 2.1.1 键盘可操作）。
 *
 * 为 div/span 等非原生按钮元素补齐：tabindex=0、role=button（若未设）、
 * Enter/Space 触发点击。已设置过则跳过（幂等）。
 *
 * @param {HTMLElement} el - 目标元素（应已有 onclick 或调用方随后绑定）
 * @returns {HTMLElement} 同一元素（便于链式使用）
 */
export function makeKeyboardClickable(el) {
  if (!el || el.dataset.kbdClickable === 'true') return el;
  el.dataset.kbdClickable = 'true';
  el.tabIndex = 0;
  if (!el.getAttribute('role')) el.setAttribute('role', 'button');
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      e.stopPropagation();
      el.click();
    }
  });
  return el;
}
