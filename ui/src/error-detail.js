/**
 * EchoMind 错误体验优化模块 — 分类错误 + 恢复操作按钮。
 *
 * 职责：
 * 1. classifyError(rawMsg) — 将原始错误消息分类为结构化错误信息
 * 2. renderErrorCard(container, errorInfo) — 渲染分类后的错误卡片到 DOM
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.12 错误体验优化
 * AC-QA-008：错误提示分类显示（网络/认证/限流/上下文超限）+ 恢复操作按钮
 *
 * 错误类型映射表：
 * | 类型        | 匹配关键词                          | 恢复操作                          |
 * |------------|-------------------------------------|-----------------------------------|
 * | network    | ECONNREFUSED / timed out / network  | retry / open_settings / local_mode |
 * | auth       | 401 / Unauthorized / API key / 认证 | open_settings / local_mode        |
 * | rate_limit | 429 / rate limit / Too Many         | wait / switch_model               |
 * | context_overflow | context length / 上下文过长    | new_chat / compress_history       |
 * | kb_empty   | 知识库为空 / Knowledge base is empty | import_files                      |
 * | model_load | Model load / GGUF / 模型加载        | check_model / switch_cloud        |
 * | unknown    | （fallback）                         | retry / open_settings             |
 */

import { t } from './i18n.js';

// ============================================================
// 错误分类规则
// ============================================================

/**
 * 错误分类规则表（按优先级从高到低匹配）。
 *
 * 每条规则包含：
 * - type: 错误类型标识
 * - patterns: 匹配关键词数组（大小写不敏感，任一匹配即命中）
 * - titleKey: i18n 标题 key
 * - titleFallback: i18n 缺失时的 fallback 标题
 * - reasonKey: i18n 原因 key
 * - reasonFallback: i18n 缺失时的 fallback 原因
 * - actions: 恢复操作数组
 *
 * @typedef {Object} ErrorRule
 * @property {string} type
 * @property {string[]} patterns
 * @property {string} titleKey
 * @property {string} titleFallback
 * @property {string} reasonKey
 * @property {string} reasonFallback
 * @property {Array<{action: string, labelKey: string, labelFallback: string, icon: string}>} actions
 */

/** @type {ErrorRule[]} */
const ERROR_RULES = [
  {
    type: 'kb_empty',
    patterns: ['知识库为空', 'knowledge base is empty', 'kb is empty', 'no documents'],
    titleKey: 'chat.error_kb_empty_title',
    titleFallback: '📚 知识库为空',
    reasonKey: 'chat.error_kb_empty_reason',
    reasonFallback: '请先导入文档后再提问',
    actions: [
      { action: 'import_files', labelKey: 'chat.error_action_import', labelFallback: '导入文件', icon: '📂' },
      // 注意：import_files 是「导入文档」入口，图标与 plusBtn / 编辑模式上传按钮保持一致
      // （上传箭头 SVG，见 renderErrorCard 中 import_files 特判）
    ],
  },
  {
    type: 'auth',
    patterns: ['401', 'unauthorized', 'api key', '认证失败', 'authentication'],
    titleKey: 'chat.error_auth_title',
    titleFallback: '🔑 认证失败',
    reasonKey: 'chat.error_auth_reason',
    reasonFallback: 'API Key 无效或已过期',
    actions: [
      { action: 'open_settings', labelKey: 'chat.error_action_settings', labelFallback: '打开设置', icon: '⚙️' },
      { action: 'switch_local', labelKey: 'chat.error_action_local_mode', labelFallback: '切换本地模式', icon: '💻' },
    ],
  },
  {
    type: 'rate_limit',
    patterns: ['429', 'rate limit', 'too many requests', '请求过快'],
    titleKey: 'chat.error_rate_limit_title',
    titleFallback: '⏱️ 请求过快',
    reasonKey: 'chat.error_rate_limit_reason',
    reasonFallback: 'API 调用频率超限，请稍后重试',
    actions: [
      { action: 'wait', labelKey: 'chat.error_action_wait', labelFallback: '等待重试', icon: '⏳' },
      { action: 'switch_model', labelKey: 'chat.error_action_switch_model', labelFallback: '切换模型', icon: '🔄' },
    ],
  },
  {
    type: 'context_overflow',
    patterns: ['context length', '上下文过长', 'token limit', 'context window'],
    titleKey: 'chat.error_context_overflow_title',
    titleFallback: '📏 上下文过长',
    reasonKey: 'chat.error_context_overflow_reason',
    reasonFallback: '对话历史超出模型上下文窗口',
    actions: [
      { action: 'new_chat', labelKey: 'chat.error_action_new_chat', labelFallback: '新建会话', icon: '💬' },
      { action: 'compress_history', labelKey: 'chat.error_action_compress', labelFallback: '压缩历史', icon: '📦' },
    ],
  },
  {
    type: 'model_load',
    patterns: ['model load', 'gguf', '模型加载', 'local model', 'mistral.rs'],
    titleKey: 'chat.error_model_load_title',
    titleFallback: '💻 本地模型错误',
    reasonKey: 'chat.error_model_load_reason',
    reasonFallback: 'GGUF 模型文件加载失败',
    actions: [
      { action: 'check_model', labelKey: 'chat.error_action_check_model', labelFallback: '检查模型', icon: '🔍' },
      { action: 'switch_cloud', labelKey: 'chat.error_action_switch_cloud', labelFallback: '切换云端', icon: '☁️' },
    ],
  },
  {
    type: 'network',
    patterns: ['econnrefused', 'timed out', 'timeout', 'network', 'fetch failed', 'connect', '网络连接'],
    titleKey: 'chat.error_network_title',
    titleFallback: '⚠️ 网络连接中断',
    reasonKey: 'chat.error_network_reason',
    reasonFallback: '无法连接到 API 服务器',
    actions: [
      { action: 'retry', labelKey: 'chat.error_action_retry', labelFallback: '重试', icon: '🔄' },
      { action: 'open_settings', labelKey: 'chat.error_action_settings', labelFallback: '检查设置', icon: '⚙️' },
      { action: 'switch_local', labelKey: 'chat.error_action_local_mode', labelFallback: '本地模式', icon: '💻' },
    ],
  },
];

/** unknown 类型的默认规则 */
const UNKNOWN_RULE = {
  type: 'unknown',
  patterns: [],
  titleKey: 'chat.error_unknown_title',
  titleFallback: '⚠️ 发生了错误',
  reasonKey: 'chat.error_unknown_reason',
  reasonFallback: '请重试或检查配置',
  actions: [
    { action: 'retry', labelKey: 'chat.error_action_retry', labelFallback: '重试', icon: '🔄' },
    { action: 'open_settings', labelKey: 'chat.error_action_settings', labelFallback: '检查设置', icon: '⚙️' },
  ],
};

// ============================================================
// 公共 API
// ============================================================

/**
 * 将原始错误消息分类为结构化错误信息。
 *
 * 匹配策略：将错误消息转为小写后，按 ERROR_RULES 优先级依次匹配关键词。
 * 第一个匹配的规则胜出；无匹配时返回 unknown 类型。
 *
 * @param {string} rawMsg - 原始错误消息
 * @returns {{type: string, title: string, reason: string, actions: Array<{action: string, label: string, icon: string}>}}
 *   结构化错误信息
 */
export function classifyError(rawMsg) {
  // 安全降级：null/undefined/非字符串
  const msg = (rawMsg == null) ? '' : String(rawMsg);
  const lower = msg.toLowerCase();

  // 按 ERROR_RULES 优先级匹配
  for (const rule of ERROR_RULES) {
    const matched = rule.patterns.some((p) => lower.includes(p.toLowerCase()));
    if (matched) {
      return buildErrorInfo(rule, msg);
    }
  }

  // fallback: unknown
  return buildErrorInfo(UNKNOWN_RULE, msg);
}

/**
 * 从规则构建结构化错误信息（解析 i18n key）。
 *
 * @param {ErrorRule} rule - 匹配的错误规则
 * @param {string} originalMsg - 原始错误消息
 * @returns {{type: string, title: string, reason: string, actions: Array<{action: string, label: string, icon: string}>, originalMessage: string}}
 * @private
 */
function buildErrorInfo(rule, originalMsg) {
  const title = resolveI18n(rule.titleKey, rule.titleFallback);
  const reason = resolveI18n(rule.reasonKey, rule.reasonFallback);
  const actions = rule.actions.map((a) => ({
    action: a.action,
    label: resolveI18n(a.labelKey, a.labelFallback),
    icon: a.icon,
  }));
  return { type: rule.type, title, reason, actions, originalMessage: originalMsg };
}

/**
 * 解析 i18n key，如果 key 不存在则返回 fallback。
 *
 * @param {string} key - i18n key
 * @param {string} fallback - fallback 文本
 * @returns {string} 解析后的文本
 * @private
 */
function resolveI18n(key, fallback) {
  const text = t(key);
  // t() 在 key 不存在时返回 key 本身
  return (text === key) ? fallback : text;
}

/**
 * 渲染错误卡片到指定容器。
 *
 * DOM 结构：
 * ```
 * .error-card
 *   .error-card-title       ⚠️ 网络连接中断
 *   .error-card-reason       原因：无法连接到 API 服务器
 *   .error-card-actions
 *     button.error-card-action[data-action="retry"]  🔄 重试
 *     button.error-card-action[data-action="..."]     ...
 * ```
 *
 * @param {HTMLElement} container - 目标容器
 * @param {{type: string, title: string, reason: string, actions: Array<{action: string, label: string, icon: string}>}} errorInfo
 * @returns {void}
 */
export function renderErrorCard(container, errorInfo) {
  if (!container || !errorInfo) return;

  // 清空容器中的思考指示器等临时元素
  const thinking = container.querySelector('.thinking-indicator');
  if (thinking) thinking.remove();

  // 移除已有的错误卡片（防止重复）
  const existing = container.querySelector('.error-card');
  if (existing) existing.remove();

  const card = document.createElement('div');
  card.className = 'error-card';

  // 标题
  const title = document.createElement('div');
  title.className = 'error-card-title';
  title.textContent = errorInfo.title;
  card.appendChild(title);

  // 原因
  const reason = document.createElement('div');
  reason.className = 'error-card-reason';
  reason.textContent = errorInfo.reason;
  card.appendChild(reason);

  // 操作按钮
  if (errorInfo.actions && errorInfo.actions.length > 0) {
    const actionsRow = document.createElement('div');
    actionsRow.className = 'error-card-actions';
    errorInfo.actions.forEach((a) => {
      const btn = document.createElement('button');
      btn.className = 'error-card-action';
      btn.dataset.action = a.action;
      // 「导入文档」入口图标统一：import_files 使用上传箭头 SVG
      // （与 plusBtn / 编辑模式上传按钮一致），其他动作保持 emoji 图标
      if (a.action === 'import_files') {
        btn.innerHTML =
          '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg> <span>' +
          a.label +
          '</span>';
      } else {
        btn.textContent = `${a.icon} ${a.label}`;
      }
      actionsRow.appendChild(btn);
    });
    card.appendChild(actionsRow);
  }

  // 插入到 md 区域之前（如果存在），否则追加到末尾
  const mdEl = container.querySelector('.md');
  if (mdEl) {
    mdEl.classList.remove('hidden');
    mdEl.innerHTML = '';
    mdEl.appendChild(card);
  } else {
    container.appendChild(card);
  }
}
