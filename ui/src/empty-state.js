/**
 * EchoMind 空状态重设计模块 — 知识库摘要 + 隐私状态 + 推荐问题卡片。
 *
 * 职责：
 * 1. 渲染空状态引导页面（logo + tagline + KB 摘要 + 隐私状态 + 推荐问题）
 * 2. 根据知识库文档数生成智能推荐问题
 * 3. 点击推荐问题卡片触发回调（填充输入框并发送）
 * 4. 空知识库时显示导入引导按钮（而非不可用的推荐问题）
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.2 空状态重设计
 * AC-QA-003：空状态显示知识库摘要（文档数/chunk 数）+ 3 个推荐问题
 */

import { t } from './i18n.js';
import { get } from './state.js';
import { icon } from './utils.js';

// ============================================================
// 推荐问题生成（仅有文档时使用）
// ============================================================

/**
 * 默认推荐问题（有文档时使用）。
 * 这些是通用的探索性问题，适用于大多数知识库。
 */
const DEFAULT_RECOMMENDATIONS = [
  'summarize_key_points',
  'compare_differences',
  'extract_key_terms',
];

/**
 * 根据知识库文档名生成动态推荐问题（REQ-RAG-018 AC-3）。
 *
 * - 有文档时：基于文档名生成动态问题 + 通用探索性问题
 * - 无文档时：返回空数组（空知识库使用导入引导按钮）
 *
 * @param {number} docCount - 知识库文档数
 * @param {string[]} [docNames=[]] - 知识库文档名列表（可选）
 * @returns {string[]} 推荐问题文本数组
 */
export function generateRecommendations(docCount, docNames) {
  const hasDocs = docCount > 0;
  if (!hasDocs) return [];
  const names = docNames || [];
  const suggestions = [];

  // 1. 基于文档名生成动态问题（取前 1-2 个文档）
  if (names.length > 0) {
    const firstName = stripExtension(names[0]);
    suggestions.push(formatDocQuestion('summarize_doc', firstName));
    if (names.length >= 2) {
      const secondName = stripExtension(names[1]);
      suggestions.push(formatDocQuestion('compare_docs', firstName, secondName));
    } else {
      suggestions.push(formatDocQuestion('key_topics_doc', firstName));
    }
  }

  // 2. 补充通用探索性问题
  const keys = DEFAULT_RECOMMENDATIONS;
  for (const key of keys) {
    if (suggestions.length >= 4) break;
    const i18nKey = `chat.empty_state_suggestion_${key}`;
    const text = t(i18nKey);
    if (text === i18nKey) {
      suggestions.push(FALLBACK_SUGGESTIONS[key] || key);
    } else {
      suggestions.push(text);
    }
  }

  return suggestions.slice(0, 4);
}

/**
 * 去除文件扩展名，保留文档名。
 * @param {string} name - 文件名
 * @returns {string} 去除扩展名后的文档名
 */
function stripExtension(name) {
  if (!name) return '';
  const lastDot = name.lastIndexOf('.');
  if (lastDot > 0) return name.substring(0, lastDot);
  return name;
}

/**
 * 格式化基于文档名的推荐问题（REQ-RAG-018 AC-3）。
 * @param {string} type - 问题类型
 * @param {string} docName - 文档名
 * @param {string} [docName2] - 第二个文档名（对比类）
 * @returns {string} 格式化后的问题
 */
function formatDocQuestion(type, docName, docName2) {
  const i18nKey = `chat.empty_state_suggestion_${type}`;
  const template = t(i18nKey);
  if (template !== i18nKey && docName2) {
    return template.replace('{doc1}', docName).replace('{doc2}', docName2);
  } else if (template !== i18nKey) {
    return template.replace('{doc}', docName);
  }
  // fallback
  if (type === 'summarize_doc') return `总结《${docName}》的核心要点`;
  if (type === 'compare_docs' && docName2) return `对比《${docName}》和《${docName2}》的主要差异`;
  if (type === 'key_topics_doc') return `《${docName}》涉及哪些关键主题？`;
  return `总结当前知识库中的核心要点`;
}

/** i18n 缺失时的 fallback 文本 */
const FALLBACK_SUGGESTIONS = {
  summarize_key_points: '总结当前知识库中的核心要点',
  compare_differences: '对比不同文档之间的差异',
  extract_key_terms: '提取所有涉及的关键术语',
};

// ============================================================
// 空状态渲染
// ============================================================

/**
 * 渲染空状态引导页面到指定容器。
 *
 * 布局结构：
 * ```
 * .empty-state-wrapper
 *   .empty-state-logo         ◈ EchoMind 灵犀
 *   .empty-state-tagline       「你的本地知识库，有问必答」
 *   .empty-state-cards-row
 *     .empty-state-kb-card     📚 知识库摘要
 *     .empty-state-privacy-card 🔒 隐私状态
 *   [有文档时] .empty-state-suggestions + .empty-state-suggestion-card × 3
 *   [无文档时] .empty-state-import-guide (导入引导按钮 + 格式说明)
 *   .empty-state-hint          或在下方输入你的问题 ⏎
 * ```
 *
 * @param {HTMLElement} container - 目标容器（通常是 #chatArea）
 * @param {Object} [opts] - 渲染选项
 * @param {number} [opts.docCount=0] - 知识库文档数
 * @param {number} [opts.chunkCount=0] - 知识库 chunk 数
 * @param {string[]} [opts.docNames=[]] - 知识库文档名列表（REQ-RAG-018 AC-3 动态建议）
 * @param {boolean} [opts.encrypted=false] - 数据库是否已加密
 * @param {Function} [opts.onPickQuestion] - 点击推荐问题时的回调
 * @param {Function} [opts.onImport] - 点击导入按钮时的回调（空知识库时显示）
 * @returns {void}
 */
export function renderEmptyState(container, opts) {
  if (!container) return;

  // 安全降级：opts 为 null/undefined 时使用默认值
  const docCount = opts?.docCount ?? get('docCount') ?? 0;
  const chunkCount = opts?.chunkCount ?? get('chunkCount') ?? 0;
  const docNames = opts?.docNames ?? get('docNames') ?? [];
  const encrypted = opts?.encrypted ?? (get('securityState') !== 'unencrypted');
  const onPickQuestion = opts?.onPickQuestion;
  const onImport = opts?.onImport;

  // 清空容器
  container.innerHTML = '';

  // 根容器
  const wrapper = document.createElement('div');
  wrapper.className = 'empty-state-wrapper animate-fade-in';

  // Logo + tagline
  const logo = document.createElement('div');
  logo.className = 'empty-state-logo';
  logo.innerHTML = icon('brand', 'lg');
  wrapper.appendChild(logo);

  const appName = document.createElement('div');
  appName.className = 'empty-state-app-name';
  appName.textContent = 'EchoMind ' + (t('app.tagline') || '灵犀');
  wrapper.appendChild(appName);

  const tagline = document.createElement('div');
  tagline.className = 'empty-state-tagline';
  tagline.textContent = t('chat.empty_state_tagline') || '你的本地知识库，有问必答';
  wrapper.appendChild(tagline);

  // 摘要卡片行
  const cardsRow = document.createElement('div');
  cardsRow.className = 'empty-state-cards-row';

  // 知识库摘要卡片
  const kbCard = document.createElement('div');
  kbCard.className = 'empty-state-kb-card';
  const kbIcon = document.createElement('span');
  kbIcon.className = 'empty-state-card-icon';
  kbIcon.innerHTML = icon('book', 'md');
  kbCard.appendChild(kbIcon);
  const kbLabel = document.createElement('div');
  kbLabel.className = 'empty-state-card-label';
  kbLabel.textContent = t('chat.empty_state_kb_title') || '知识库';
  kbCard.appendChild(kbLabel);
  const kbStats = document.createElement('div');
  kbStats.className = 'empty-state-card-stats';
  const docLine = document.createElement('div');
  docLine.textContent = `${docCount} ${t('chat.empty_state_docs') || '篇文档'}`;
  kbStats.appendChild(docLine);
  const chunkLine = document.createElement('div');
  chunkLine.textContent = `${chunkCount.toLocaleString()} ${t('chat.empty_state_chunks') || 'chunks'}`;
  kbStats.appendChild(chunkLine);
  kbCard.appendChild(kbStats);
  cardsRow.appendChild(kbCard);

  // 隐私状态卡片
  const privacyCard = document.createElement('div');
  privacyCard.className = 'empty-state-privacy-card';
  const privacyIcon = document.createElement('span');
  privacyIcon.className = 'empty-state-card-icon';
  privacyIcon.innerHTML = encrypted ? icon('lock', 'md') : icon('unlock', 'md');
  privacyCard.appendChild(privacyIcon);
  const privacyLabel = document.createElement('div');
  privacyLabel.className = 'empty-state-card-label';
  privacyLabel.textContent = t('chat.empty_state_privacy') || '隐私状态';
  privacyCard.appendChild(privacyLabel);
  const privacyStats = document.createElement('div');
  privacyStats.className = 'empty-state-card-stats';
  const encLine = document.createElement('div');
  encLine.textContent = encrypted
    ? (t('security.encrypted') || '已加密')
    : (t('security.not_encrypted') || '未加密');
  privacyStats.appendChild(encLine);
  const piiLine = document.createElement('div');
  const piiEnabled = get('piiDetectionEnabled');
  piiLine.textContent = piiEnabled
    ? (t('chat.empty_state_pii_on') || 'PII 已脱敏')
    : (t('chat.empty_state_pii_off') || 'PII 检测关闭');
  privacyStats.appendChild(piiLine);
  privacyCard.appendChild(privacyStats);
  cardsRow.appendChild(privacyCard);

  wrapper.appendChild(cardsRow);

  if (docCount > 0) {
    // ===== 有文档：显示推荐问题卡片 =====
    const suggestions = generateRecommendations(docCount, docNames);
    if (suggestions.length > 0) {
      const suggLabel = document.createElement('div');
      suggLabel.className = 'empty-state-suggestions-label';
      suggLabel.textContent = t('chat.empty_state_try_these') || '💡 试试这些问题：';
      wrapper.appendChild(suggLabel);

      const suggContainer = document.createElement('div');
      suggContainer.className = 'empty-state-suggestions';
      suggestions.forEach((question) => {
        const card = document.createElement('button');
        card.className = 'empty-state-suggestion-card';
        card.textContent = question;
        card.onclick = () => {
          if (typeof onPickQuestion === 'function') {
            onPickQuestion(question);
          }
        };
        suggContainer.appendChild(card);
      });
      wrapper.appendChild(suggContainer);
    }

    // 底部提示
    const hint = document.createElement('div');
    hint.className = 'empty-state-hint';
    hint.textContent = t('chat.empty_state_input_hint') || '或在下方输入你的问题 ⏎';
    wrapper.appendChild(hint);
  } else {
    // ===== 空知识库：显示导入引导按钮 + 格式说明 =====
    const guideContainer = document.createElement('div');
    guideContainer.className = 'empty-state-import-guide';

    const suggLabel = document.createElement('div');
    suggLabel.className = 'empty-state-suggestions-label';
    suggLabel.textContent = t('chat.empty_state_get_started') || '💡 开始使用：';
    guideContainer.appendChild(suggLabel);

    // 导入按钮 — 点击触发文件选择器
    const importBtn = document.createElement('button');
    importBtn.className = 'empty-state-import-btn';
    const importIcon = document.createElement('span');
    importIcon.className = 'empty-state-import-icon';
    importIcon.textContent = '📄';
    importBtn.appendChild(importIcon);
    const importText = document.createElement('span');
    importText.textContent = t('chat.empty_state_import_btn') || '导入你的第一份文档';
    importBtn.appendChild(importText);
    importBtn.onclick = () => {
      if (typeof onImport === 'function') {
        onImport();
      }
    };
    guideContainer.appendChild(importBtn);

    // 格式说明（纯文本，不可点击）
    const formatHint = document.createElement('div');
    formatHint.className = 'empty-state-format-hint';
    formatHint.textContent = t('chat.empty_state_formats') || '支持 .md / .txt / .pdf 格式';
    guideContainer.appendChild(formatHint);

    wrapper.appendChild(guideContainer);
  }

  container.appendChild(wrapper);
}
