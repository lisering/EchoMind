/**
 * EchoMind 文档范围筛选模块 — 输入 `@` 触发文档选择弹框。
 *
 * 职责：
 * 1. 从文本中提取 @docname 引用（支持多个）
 * 2. 按名称部分匹配过滤文档列表
 * 3. 渲染浮动文档选择弹框
 * 4. 选中文档后在输入框光标位置插入 @docname
 * 5. 生成 doc_filter 数组传递给 chat IPC
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.9 文档范围筛选
 * AC-QA-011：输入 `@` 触发文档选择弹框，限定检索范围
 */

import { t } from './i18n.js';

// ============================================================
// 文本解析
// ============================================================

/**
 * 从文本中提取 @docname 引用。
 *
 * 规则：
 * - `@` 后跟非空白字符序列即为引用
 * - `@` 后紧跟空白不视为引用
 * - 重复引用自动去重
 *
 * @param {string} text - 输入文本
 * @returns {string[]} 文档名数组（去重，保持出现顺序）
 */
export function extractDocMentions(text) {
  if (!text) return [];

  // 匹配 @ 后跟非空白字符序列
  const regex = /@(\S+)/g;
  const mentions = [];
  const seen = new Set();

  let match;
  while ((match = regex.exec(text)) !== null) {
    const docName = match[1];
    // 排除纯标点（如 @，。等）
    if (docName && !seen.has(docName)) {
      seen.add(docName);
      mentions.push(docName);
    }
  }

  return mentions;
}

// ============================================================
// 文档过滤
// ============================================================

/**
 * 按名称部分匹配过滤文档列表。
 *
 * - 空查询 → 返回全部文档
 * - 大小写不敏感匹配
 *
 * @param {Array<{id: string, name: string}>} docs - 文档列表
 * @param {string} query - 查询字符串
 * @returns {Array<{id: string, name: string}>} 过滤后的文档列表
 */
export function filterDocuments(docs, query) {
  if (!docs || docs.length === 0) return [];
  if (!query) return [...docs];

  const lowerQuery = query.toLowerCase();
  return docs.filter((doc) => {
    const name = (doc.name || '').toLowerCase();
    return name.includes(lowerQuery);
  });
}

// ============================================================
// DOM 渲染
// ============================================================

/**
 * 渲染文档选择弹框。
 *
 * 在容器中创建 `.doc-mention-popup` 元素，包含文档列表项。
 * 点击文档项触发 onSelect 回调。
 *
 * @param {HTMLElement} container - 挂载容器
 * @param {Array<{id: string, name: string}>} docs - 文档列表
 * @param {(doc: {id: string, name: string}) => void} [onSelect] - 选中文档时的回调
 * @returns {HTMLElement} 创建的弹框根元素
 */
export function renderDocMentionPopup(container, docs, onSelect) {
  // 清除已有弹框
  const existing = container.querySelector('.doc-mention-popup');
  if (existing) existing.remove();

  const popup = document.createElement('div');
  popup.className = 'doc-mention-popup';

  if (!docs || docs.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'doc-mention-empty';
    empty.textContent = t('chat.doc_mention_empty') || '无匹配文档';
    popup.appendChild(empty);
  } else {
    docs.forEach((doc) => {
      const item = document.createElement('div');
      item.className = 'doc-mention-item';
      item.dataset.docId = doc.id || '';
      item.dataset.docName = doc.name || '';

      const icon = document.createElement('span');
      icon.className = 'doc-mention-item-icon';
      icon.textContent = '📄';

      const name = document.createElement('span');
      name.className = 'doc-mention-item-name';
      name.textContent = doc.name || '';

      item.appendChild(icon);
      item.appendChild(name);

      item.onclick = () => {
        if (onSelect) onSelect(doc);
      };

      popup.appendChild(item);
    });
  }

  container.appendChild(popup);
  return popup;
}

// ============================================================
// 输入框插入
// ============================================================

/**
 * 在输入框光标位置插入 @docname。
 *
 * 插入后光标位于 @docname 之后。
 *
 * @param {HTMLInputElement|HTMLTextAreaElement} inputEl - 输入框元素
 * @param {string} docName - 文档名
 */
export function insertDocMention(inputEl, docName) {
  const start = inputEl.selectionStart || 0;
  const end = inputEl.selectionEnd || 0;
  const before = inputEl.value.substring(0, start);
  const after = inputEl.value.substring(end);
  // 插入 @docname 并附带尾部空格，便于用户继续输入
  const insertion = `@${docName} `;

  inputEl.value = before + insertion + after;

  // 光标置于插入文本之后
  const newCursorPos = start + insertion.length;
  inputEl.setSelectionRange(newCursorPos, newCursorPos);
  inputEl.focus();
}

// ============================================================
// IPC 参数生成
// ============================================================

/**
 * 生成 doc_filter 数组供 chat IPC 使用。
 *
 * @param {string[]} mentions - 文档名引用数组
 * @returns {string[]} 文档名数组（直接传递给 chat 命令的 doc_filter 参数）
 */
export function getDocFilter(mentions) {
  if (!mentions || mentions.length === 0) return [];
  return [...mentions];
}

/**
 * 移除文档选择弹框（如果存在）。
 * @param {HTMLElement} container - 弹框挂载容器
 */
export function removeDocMentionPopup(container) {
  const popup = container?.querySelector('.doc-mention-popup');
  if (popup) popup.remove();
}
