/**
 * EchoMind 后续问题建议模块 — 回答完成后展示 2-3 个后续追问。
 *
 * 职责：
 * 1. 从 assistant 回答中提取关键实体/概念（纯规则，零 LLM 调用）
 * 2. 基于模板生成后续问题建议（3 种策略：实体追问 / 对比 / 例外）
 * 3. 渲染后续建议卡片（可点击发送 + 可关闭）
 * 4. 最多 3 条建议，避免选择困难
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.5 后续问题建议
 * AC-QA-005：回答完成后显示 2-3 个后续问题建议，可关闭
 */

import { t } from './i18n.js';

// ============================================================
// 常量定义
// ============================================================

/** 后续建议最大数量 */
const MAX_SUGGESTIONS = 3;

/**
 * 模板化追问句式（中英双语）。
 * 每个模板包含一个占位符 {entity}，用于替换为提取的关键实体。
 */
const FOLLOWUP_TEMPLATES = {
  // 实体深入型：对回答中提到的关键概念追问
  entity_deep: [
    '关于 {entity} 还有哪些规定？',
    '{entity} 的具体适用条件是什么？',
    '{entity} 的计算方式是怎样的？',
    '{entity} 有哪些例外情况？',
    'What else should I know about {entity}?',
    'What are the specific conditions for {entity}?',
  ],
  // 对比型：要求对比两个实体
  compare: [
    '对比 {entity} 和其他相关规定的异同',
    'Compare {entity} with related regulations',
  ],
  // 例外型：询问例外或特殊情况
  exception: [
    '{entity} 有哪些例外或特殊情形？',
    'What are the exceptions to {entity}?',
  ],
};

// ============================================================
// 关键实体提取（纯规则，零 LLM）
// ============================================================

/**
 * 中文停用词集合（避免提取无意义词作为实体）。
 */
const STOP_WORDS = new Set([
  '的', '了', '是', '在', '和', '与', '或', '为', '以', '及', '等', '中',
  '可', '以', '对', '从', '到', '由', '将', '被', '让', '使', '给', '向',
  '这', '那', '其', '此', '该', '之', '所', '一', '不', '无', '有',
  '可以', '应当', '不得', '或者', '但是', '如果', '因此', '所以',
  '根据', '关于', '对于', '按照', '依据', '以下', '以上', '其中',
  '本文', '本条', '本法', '规定', '情形', '以下',
  // 英文停用词
  'the', 'a', 'an', 'is', 'are', 'was', 'were', 'be', 'been', 'being',
  'and', 'or', 'but', 'if', 'then', 'else', 'when', 'where', 'why', 'how',
  'this', 'that', 'these', 'those', 'it', 'its', 'they', 'them', 'their',
  'to', 'of', 'in', 'on', 'at', 'by', 'for', 'with', 'from', 'as', 'such',
  'can', 'may', 'must', 'should', 'would', 'could', 'will', 'shall',
]);

/**
 * 从文本中提取关键实体/概念。
 *
 * 策略（纯规则，零 LLM）：
 * 1. 提取书名号内容（中文法律文档常见：《XX法》）
 * 2. 提取引号内容（"XX" 或 'XX'）
 * 3. 提取 Markdown 标题内容（## XX）
 * 4. 提取高频专业术语（长度 ≥ 2 的中英文词组，排除停用词）
 *
 * @param {string} text - assistant 回答文本
 * @param {number} [maxEntities=5] - 最大实体数量
 * @returns {string[]} 提取的实体列表（去重，最多 maxEntities 个）
 */
export function extractEntities(text, maxEntities = 5) {
  if (!text || typeof text !== 'string') return [];

  const entities = [];
  const seen = new Set();

  // 1. 提取书名号内容：《...》
  const bookMatches = text.matchAll(/《([^》]{2,20})》/g);
  for (const m of bookMatches) {
    const entity = m[1].trim();
    if (entity && !seen.has(entity)) {
      seen.add(entity);
      entities.push(entity);
    }
    if (entities.length >= maxEntities) return entities;
  }

  // 2. 提取引号内容："..." 或 「...」
  const quoteMatches = text.matchAll(/[""「]([^""」]{2,30})[""」]/g);
  for (const m of quoteMatches) {
    const entity = m[1].trim();
    if (entity && !seen.has(entity) && !STOP_WORDS.has(entity)) {
      seen.add(entity);
      entities.push(entity);
    }
    if (entities.length >= maxEntities) return entities;
  }

  // 3. 提取 Markdown 标题内容（## XX）
  const headingMatches = text.matchAll(/^#{1,6}\s+(.+)$/gm);
  for (const m of headingMatches) {
    const entity = m[1].trim();
    if (entity && !seen.has(entity) && !STOP_WORDS.has(entity)) {
      seen.add(entity);
      entities.push(entity);
    }
    if (entities.length >= maxEntities) return entities;
  }

  // 4. 提取高频专业术语：连续 2-6 个中文字符 或 英文词组
  // 中文术语
  const cjkMatches = text.matchAll(/[\u4e00-\u9fff]{2,6}/g);
  const cjkFreq = new Map();
  for (const m of cjkMatches) {
    const term = m[0];
    if (!STOP_WORDS.has(term)) {
      cjkFreq.set(term, (cjkFreq.get(term) || 0) + 1);
    }
  }
  // 按频率排序，取高频术语
  const sortedCjk = [...cjkFreq.entries()]
    .sort((a, b) => b[1] - a[1])
    .map(([term]) => term);
  for (const term of sortedCjk) {
    if (entities.length >= maxEntities) break;
    if (!seen.has(term) && !STOP_WORDS.has(term)) {
      seen.add(term);
      entities.push(term);
    }
  }

  // 英文术语（大写开头或全大写缩写，长度 ≥ 3）
  if (entities.length < maxEntities) {
    const engMatches = text.matchAll(/\b([A-Z][a-z]{2,}|[A-Z]{2,})\b/g);
    for (const m of engMatches) {
      if (entities.length >= maxEntities) break;
      const term = m[1];
      if (!seen.has(term) && !STOP_WORDS.has(term.toLowerCase())) {
        seen.add(term);
        entities.push(term);
      }
    }
  }

  return entities.slice(0, maxEntities);
}

// ============================================================
// 后续问题生成
// ============================================================

/**
 * 基于提取的实体和用户原始查询生成后续问题建议。
 *
 * 生成策略：
 * 1. 实体深入型：对回答中提到的关键概念追问
 * 2. 对比型：要求对比两个实体
 * 3. 例外型：询问例外或特殊情形
 *
 * @param {string[]} entities - 从回答中提取的实体列表
 * @param {string} [userQuery=''] - 用户原始查询（用于上下文感知）
 * @returns {string[]} 后续问题建议列表（最多 MAX_SUGGESTIONS 条）
 */
export function generateFollowups(entities, userQuery = '') {
  if (!entities || entities.length === 0) {
    // 无实体时返回通用追问
    return [
      t('chat.followup_generic_1') || '还有哪些相关内容？',
      t('chat.followup_generic_2') || '能否更详细地解释？',
      t('chat.followup_generic_3') || '有什么需要注意的？',
    ].slice(0, MAX_SUGGESTIONS);
  }

  const suggestions = [];
  const seen = new Set();

  // 策略 1：实体深入型（对第一个实体追问）
  if (entities[0]) {
    const template = FOLLOWUP_TEMPLATES.entity_deep[0];
    const q = template.replace('{entity}', entities[0]);
    if (!seen.has(q)) {
      seen.add(q);
      suggestions.push(q);
    }
  }

  // 策略 2：例外型（对第一个实体追问例外）
  if (entities[0] && suggestions.length < MAX_SUGGESTIONS) {
    const template = FOLLOWUP_TEMPLATES.exception[0];
    const q = template.replace('{entity}', entities[0]);
    if (!seen.has(q)) {
      seen.add(q);
      suggestions.push(q);
    }
  }

  // 策略 3：实体深入型（对第二个实体追问，如有）
  if (entities[1] && suggestions.length < MAX_SUGGESTIONS) {
    const template = FOLLOWUP_TEMPLATES.entity_deep[1];
    const q = template.replace('{entity}', entities[1]);
    if (!seen.has(q)) {
      seen.add(q);
      suggestions.push(q);
    }
  }

  // 策略 4：对比型（如有两个实体）
  if (entities.length >= 2 && suggestions.length < MAX_SUGGESTIONS) {
    const template = FOLLOWUP_TEMPLATES.compare[0];
    const q = template.replace('{entity}', `${entities[0]}与${entities[1]}`);
    if (!seen.has(q)) {
      seen.add(q);
      suggestions.push(q);
    }
  }

  // 策略 5：补充实体深入型（使用更多模板填充）
  let templateIdx = 2;
  let entityIdx = 0;
  while (suggestions.length < MAX_SUGGESTIONS && entityIdx < entities.length) {
    const entity = entities[entityIdx];
    const templates = FOLLOWUP_TEMPLATES.entity_deep;
    if (templateIdx < templates.length) {
      const q = templates[templateIdx].replace('{entity}', entity);
      if (!seen.has(q)) {
        seen.add(q);
        suggestions.push(q);
      }
      templateIdx++;
    } else {
      entityIdx++;
      templateIdx = 0;
    }
  }

  return suggestions.slice(0, MAX_SUGGESTIONS);
}

// ============================================================
// 后续建议渲染
// ============================================================

/**
 * 在 assistant 消息块下方渲染后续问题建议卡片。
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.5
 * - 浅色背景，不抢主内容焦点
 * - 点击即发送，无需手动输入
 * - 可关闭（✕ 按钮）
 * - 最多 3 条
 *
 * @param {HTMLElement} blockEl - assistant 消息块根元素
 * @param {string[]} suggestions - 后续问题建议列表
 * @param {function} onPick - 点击建议时的回调（参数为建议文本）
 * @returns {HTMLElement|null} 渲染的建议容器元素（null=无建议或无容器）
 */
export function renderFollowups(blockEl, suggestions, onPick) {
  if (!blockEl) return null;
  if (!suggestions || suggestions.length === 0) return null;

  // 移除已有的后续建议容器
  removeFollowups(blockEl);

  // 创建后续建议容器
  const container = document.createElement('div');
  container.className = 'followup-suggestions';

  // 标题行
  const header = document.createElement('div');
  header.className = 'followup-header';
  const titleText = t('chat.followup_title') || '💡 你可能还想问：';
  header.innerHTML = `<span class="followup-title-text">${titleText}</span>`;

  // 关闭按钮
  const closeBtn = document.createElement('button');
  closeBtn.className = 'followup-close-btn';
  closeBtn.setAttribute('aria-label', t('common.close') || '关闭');
  closeBtn.innerHTML = '✕';
  closeBtn.onclick = (e) => {
    e.stopPropagation();
    removeFollowups(blockEl);
  };
  header.appendChild(closeBtn);

  container.appendChild(header);

  // 建议卡片列表
  const list = document.createElement('div');
  list.className = 'followup-list';

  suggestions.forEach((text) => {
    const card = document.createElement('button');
    card.className = 'followup-card';
    card.textContent = text;
    card.onclick = () => {
      if (onPick) onPick(text);
    };
    list.appendChild(card);
  });

  container.appendChild(list);

  // 插入到 assistant 消息块内部（msg-actions 之后）
  blockEl.appendChild(container);

  return container;
}

/**
 * 移除指定消息块中的后续建议容器。
 *
 * @param {HTMLElement} blockEl - 消息块根元素
 * @returns {void}
 */
export function removeFollowups(blockEl) {
  if (!blockEl) return;
  const existing = blockEl.querySelector('.followup-suggestions');
  if (existing) existing.remove();
}

/**
 * 从 assistant 回答文本生成后续问题建议并渲染。
 *
 * 一站式方法：提取实体 → 生成建议 → 渲染卡片。
 *
 * @param {HTMLElement} blockEl - assistant 消息块根元素
 * @param {string} answerText - assistant 回答文本
 * @param {string} [userQuery=''] - 用户原始查询
 * @param {function} [onPick] - 点击建议时的回调
 * @returns {HTMLElement|null} 渲染的建议容器元素
 */
export function renderFollowupSuggestions(blockEl, answerText, userQuery = '', onPick) {
  const entities = extractEntities(answerText);
  const suggestions = generateFollowups(entities, userQuery);
  return renderFollowups(blockEl, suggestions, onPick);
}
