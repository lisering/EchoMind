/**
 * EchoMind 自定义右键上下文菜单模块（REQ-IX-001 / REQ-IX-003 / REQ-UI-012）。
 *
 * 取代 Tauri WebView 的浏览器默认右键菜单，提供与桌面应用一致的
 * 剪切/复制/粘贴/全选体验。仅在有可编辑文本或选中文本时显示相关项。
 *
 * v1.14 增强（REQ-IX-001）：
 * - 文档列表右键：新增「删除」菜单项（带确认对话框）
 * - 会话列表右键：重命名 / 删除 / 导出 Markdown
 * - 消息 Block 右键：复制全文 / 复制纯文本 / 重新生成 / 删除
 * - 菜单分组（分隔线）+ 快捷键提示 + 边界检测 + 灰显逻辑
 *
 * 技术要点：
 * - contextmenu 事件 preventDefault 抑制浏览器默认菜单
 * - document.execCommand 执行 clipboard 操作（Tauri WebView 仍支持）
 * - 菜单定位考虑视口边界，防溢出
 * - 点击外部 / Escape / 滚动 / 失焦 → 自动关闭
 */

import { $, copyToClipboard } from './utils.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { docExtApi, docApi, docExportApi, saveDialog, convApi } from './ipc.js';
import { exportDocumentToPdf, exportDocumentToHtml } from './export.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { getState, get } from './state.js';
import { invoke } from './ipc.js';
import { isComposingEvent } from './input-utils.js';
import { toggleBookmark, refreshBookmarks } from './bookmarks.js';

/** 当前菜单是否可见 */
let _visible = false;

/**
 * 初始化全局右键菜单：注册 contextmenu 事件 + 关闭监听。
 * 在 main.js boot() 中调用。
 */
export function initContextMenu() {
  const menu = $('ctxMenu');
  if (!menu) return;

  // 全局 contextmenu：抑制浏览器默认菜单，显示自定义菜单
  document.addEventListener('contextmenu', (e) => {
    const target = e.target;

    // REQ-IX-001：会话列表项右键 → 显示会话菜单
    const convItem = target.closest('[data-conv-id]');
    if (convItem) {
      e.preventDefault();
      _showConvMenu(e.clientX, e.clientY, convItem.dataset.convId || '', convItem.dataset.convTitle || '');
      return;
    }

    // REQ-IX-001：消息 Block 右键 → 显示消息菜单
    const msgBlock = target.closest('.msg-block');
    if (msgBlock) {
      e.preventDefault();
      _showMsgMenu(e.clientX, e.clientY, msgBlock);
      return;
    }

    // REQ-IX-003 AC-4：文档列表项右键 → 显示文档菜单
    const docItem = target.closest('[data-doc-name]');
    if (docItem) {
      e.preventDefault();
      _showDocMenu(e.clientX, e.clientY, docItem.dataset.docName || '', docItem.dataset.docId || '');
      return;
    }

    // REQ-IX-003：引用芯片右键 → 显示「复制引用片段」菜单
    const sourceChip = target.closest('.source-chip');
    if (sourceChip && sourceChip.dataset.chunkContent) {
      e.preventDefault();
      _showSourceMenu(e.clientX, e.clientY, sourceChip.dataset.chunkContent);
      return;
    }

    // 输入框/文本域/可编辑区 → 显示编辑菜单
    // @ts-expect-error EventTarget extended with Element properties via dom-ext.d.ts
    const isEditable = _isEditable(target);
    const hasSelection = _hasSelection();

    // 非编辑且无选中 → 不显示菜单（也抑制默认）
    if (!isEditable && !hasSelection) {
      e.preventDefault();
      return;
    }

    e.preventDefault();
    _showMenu(e.clientX, e.clientY, { isEditable, hasSelection });
  });

  // 点击外部关闭
  menu.addEventListener('click', (e) => {
    e.stopPropagation();
    const item = e.target.closest('.ctx-item');
    if (item && !item.classList.contains('disabled')) {
      const action = item.dataset.action;
      _executeAction(action);
      _hideMenu();
    }
  });
  // 全局点击 / Escape / 滚动 → 关闭
  document.addEventListener('click', () => _hideMenu(), true);
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && _visible) {
      e.stopPropagation();
      _hideMenu();
    }
  });
  window.addEventListener('blur', () => _hideMenu());
  document.addEventListener('scroll', () => _hideMenu(), true);
}

/**
 * 显示文档列表项右键菜单（REQ-IX-001 / REQ-IX-003 AC-4 + S4 复盘重分类）。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {string} docName - 文档名称
 * @param {string} docId - 文档 ID（用于重分类）
 */
function _showDocMenu(x, y, docName, docId) {
  const menu = $('ctxMenu');
  if (!menu) return;

  const items = [];
  items.push(_item('copyDocName', t('ctx.copy_doc_name'), '', true));
  // S4 复盘：重分类菜单项（仅对有 ID 的文档显示）
  if (docId) {
    items.push(_separator());
    items.push(_item('exportPdf', t('ctx.export_pdf', '导出为 PDF'), '', true));
    items.push(_item('exportHtml', t('ctx.export_html', '导出为 HTML'), '', true));
    items.push(_item('exportOriginal', t('ctx.export_original', '导出原文'), '', true));
    items.push(_item('reclassifyDoc', t('ctx.reclassify_doc', '重分类文档'), '', true));
    items.push(_separator());
    items.push(_item('rebuildIndex', t('ctx.rebuild_index', '重建索引'), '', true));
    // REQ-IX-001：删除文档（带确认对话框）
    items.push(_separator());
    items.push(_item('migrateDoc', t('ctx.migrate_doc', '迁移至…'), '', true));
    items.push(_item('deleteDoc', t('ctx.delete_doc', '删除文档'), '', true));
  }
  menu.innerHTML = items.join('');

  // 存储文档信息供点击时使用
  menu.dataset.docName = docName;
  menu.dataset.docId = docId;

  _positionMenu(menu, x, y);
}

/**
 * 显示引用芯片右键菜单（REQ-IX-003）。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {string} chunkContent - chunk 原文
 */
function _showSourceMenu(x, y, chunkContent) {
  const menu = $('ctxMenu');
  if (!menu) return;

  const items = [];
  items.push(_item('copyChunk', t('ctx.copy_chunk'), '', true));
  menu.innerHTML = items.join('');

  // 存储 chunk 内容供点击时使用
  menu.dataset.chunkContent = chunkContent;

  _positionMenu(menu, x, y);
}

/**
 * 显示右键菜单。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {{isEditable:boolean, hasSelection:boolean}} ctx - 上下文状态
 */
function _showMenu(x, y, ctx) {
  const menu = $('ctxMenu');
  if (!menu) return;

  // 根据上下文渲染菜单项
  menu.innerHTML = _buildItems(ctx);

  _positionMenu(menu, x, y);
}

/** 隐藏右键菜单。 */
function _hideMenu() {
  if (!_visible) return;
  const menu = $('ctxMenu');
  if (menu) menu.classList.remove('visible');
  _visible = false;
}

/**
 * 构建菜单项 HTML。
 * @param {{isEditable:boolean, hasSelection:boolean}} ctx
 * @returns {string} 菜单项 HTML
 */
function _buildItems(ctx) {
  const mod = '⌘'; // macOS Cmd（Tauri 桌面端为主）
  const items = [];

  // 剪切：仅可编辑且有选中
  items.push(_item('cut', t('ctx.cut'), mod + 'X', ctx.isEditable && ctx.hasSelection));
  // 复制：有选中即可
  items.push(_item('copy', t('ctx.copy'), mod + 'C', ctx.hasSelection));
  // 粘贴：仅可编辑
  items.push(_item('paste', t('ctx.paste'), mod + 'V', ctx.isEditable));
  items.push(_separator());
  // 全选：可编辑或有内容
  items.push(_item('selectAll', t('ctx.selectAll'), mod + 'A', ctx.isEditable || true));

  return items.join('');
}

/**
 * 构建单个菜单项 HTML。
 * @param {string} action - 动作标识
 * @param {string} label - 显示文案
 * @param {string} shortcut - 快捷键提示
 * @param {boolean} enabled - 是否可用
 * @returns {string} HTML
 */
function _item(action, label, shortcut, enabled) {
  const cls = enabled ? 'ctx-item' : 'ctx-item disabled';
  return `<div class="${cls}" data-action="${action}"><span>${label}</span><span class="ctx-shortcut">${shortcut}</span></div>`;
}

/** 构建分隔线 HTML。 */
function _separator() {
  return '<div class="ctx-separator"></div>';
}

/**
 * 执行菜单动作。
 * @param {string} action - cut/copy/paste/selectAll/copyDocName/copyChunk/deleteDoc/convBookmark/convRename/convDelete/convExport/msgCopyFull/msgCopyPlain/msgRegenerateConcise/msgRegenerateDetailed/msgRegenerateForceSearch/msgDelete
 */
async function _executeAction(action) {
  const menu = $('ctxMenu');

  // REQ-RAG-047 AC-1：会话列表 — 添加/移除书签
  if (action === 'convBookmark') {
    const convId = menu?.dataset.convId || '';
    if (!convId) return;
    try {
      const nowBookmarked = await toggleBookmark(convId);
      toastSuccess(nowBookmarked
        ? (t('sidebar.bookmark_add', '添加书签'))
        : (t('sidebar.bookmark_remove', '移除书签')));
      refreshBookmarks({});
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-IX-001：会话列表 — 重命名（进入行内编辑模式）
  if (action === 'convRename') {
    const convId = menu?.dataset.convId || '';
    const convItem = document.querySelector(`[data-conv-id="${convId}"]`);
    if (convItem) {
      const titleSpan = convItem.querySelector('span.truncate');
      if (titleSpan) {
        titleSpan.setAttribute('contenteditable', 'true');
        titleSpan.focus();
        document.execCommand('selectAll', false);
        const finishRename = async () => {
          titleSpan.removeAttribute('contenteditable');
          const newTitle = titleSpan.textContent?.trim() || '';
          if (newTitle && convId) {
            try {
              await convApi.rename(convId, newTitle);
              toastSuccess(t('ctx.rename_success', '重命名成功'));
            } catch (err) {
              toastError(err);
            }
          }
          titleSpan.removeEventListener('blur', finishRename);
          titleSpan.removeEventListener('keydown', onRenameKey);
        };
        const onRenameKey = (e) => {
          if (isComposingEvent(e)) return; // IME 组合中不触发
          if (e.key === 'Enter') { e.preventDefault(); titleSpan.blur(); }
          if (e.key === 'Escape') { e.preventDefault(); titleSpan.blur(); }
        };
        titleSpan.addEventListener('blur', finishRename);
        titleSpan.addEventListener('keydown', onRenameKey);
      }
    }
    return;
  }

  // REQ-IX-001：会话列表 — 删除
  if (action === 'convDelete') {
    const convId = menu?.dataset.convId || '';
    const convTitle = menu?.dataset.convTitle || '';
    if (!convId) return;
    const confirmed = await showConfirmDialog({
      title: t('ctx.delete_conv_title', '删除会话'),
      body: t('ctx.delete_conv_confirm', { name: convTitle }) || `确定删除「${convTitle}」？此操作不可撤销。`,
      confirmText: t('common.delete', '删除'),
      cancelText: t('common.cancel', '取消'),
      danger: true,
    });
    if (!confirmed) return;
    try {
      await convApi.delete(convId);
      toastSuccess(t('ctx.delete_conv_success', '会话已删除'));
      // 刷新会话列表 — 触发全局事件
      document.dispatchEvent(new CustomEvent('ctx-refresh-conversations'));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-IX-001：会话列表 — 导出为 Markdown
  if (action === 'convExport') {
    const convId = menu?.dataset.convId || '';
    const convTitle = menu?.dataset.convTitle || 'conversation';
    if (!convId) return;
    try {
      const md = await convApi.exportMarkdown(convId);
      if (!md) { toastError(t('ctx.export_conv_empty', '对话内容为空')); return; }
      const destPath = await saveDialog({
        defaultPath: convTitle + '.md',
        title: t('ctx.export_conv_title', '导出会话'),
      });
      if (!destPath) return;
      await convApi.saveTextFile(destPath, md);
      toastSuccess(t('ctx.export_conv_success', '导出成功'));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-IX-001：消息 Block — 复制全文（Markdown 原文）
  if (action === 'msgCopyFull') {
    const rawMd = menu?.dataset.msgRawMd || '';
    const ok = await copyToClipboard(rawMd);
    if (ok) toast(t('chat.copied_to_clipboard'), 'success');
    else toastError(t('chat.copy_failed'));
    return;
  }

  // REQ-IX-001：消息 Block — 复制纯文本
  if (action === 'msgCopyPlain') {
    const plainText = menu?.dataset.msgPlain || '';
    const ok = await copyToClipboard(plainText.trim());
    if (ok) toast(t('chat.copied_to_clipboard'), 'success');
    else toastError(t('chat.copy_failed'));
    return;
  }

  // P0-3：消息 Block — 重新生成变体（仅 assistant）
  // 三种变体：简洁（默认）/ 详细 / 强制检索
  if (action === 'msgRegenerateConcise' || action === 'msgRegenerateDetailed' || action === 'msgRegenerateForceSearch') {
    const query = menu?.dataset.msgQuery || '';
    if (!query) return;
    // 根据变体修改查询前缀指令
    let modifiedQuery = query;
    let variant = 'default';
    if (action === 'msgRegenerateConcise') {
      variant = 'concise';
      modifiedQuery = `请简洁回答：${query}`;
    } else if (action === 'msgRegenerateDetailed') {
      variant = 'detailed';
      modifiedQuery = `请详细展开回答，包含更多细节和示例：${query}`;
    } else if (action === 'msgRegenerateForceSearch') {
      variant = 'force_search';
      modifiedQuery = query;
    }
    const input = $('queryInput');
    if (input) { input.value = modifiedQuery; }
    // 触发全局事件让 main.js 处理 send()，传递变体参数
    document.dispatchEvent(new CustomEvent('ctx-regenerate', { detail: { variant, originalQuery: query } }));
    return;
  }

  // REQ-IX-001：消息 Block — 删除消息
  if (action === 'msgDelete') {
    const msgId = menu?.dataset.msgId || '';
    const convId = get('currentConversationId') || '';
    if (!msgId || !convId) return;
    const confirmed = await showConfirmDialog({
      title: t('ctx.delete_msg_title', '删除消息'),
      body: t('ctx.delete_msg_confirm', '确定删除此消息？此操作不可撤销。'),
      confirmText: t('common.delete', '删除'),
      cancelText: t('common.cancel', '取消'),
      danger: true,
    });
    if (!confirmed) return;
    try {
      await convApi.deleteMessage(convId, msgId);
      // 从 DOM 移除消息块
      const msgBlock = document.querySelector(`[data-msg-id="${msgId}"]`);
      if (msgBlock) {
        // user 消息删除时连带下一条 assistant
        if (msgBlock.classList.contains('msg-user')) {
          const next = msgBlock.nextElementSibling;
          if (next && next.classList.contains('msg-assistant')) next.remove();
        }
        // assistant 消息删除时连带前一条 user
        if (msgBlock.classList.contains('msg-assistant')) {
          const prev = msgBlock.previousElementSibling;
          if (prev && prev.classList.contains('msg-user')) prev.remove();
        }
        msgBlock.remove();
      }
      toastSuccess(t('ctx.delete_msg_success', '消息已删除'));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-WS-004：文档跨知识库迁移
  if (action === 'migrateDoc') {
    const docId = menu?.dataset.docId || '';
    const docName = menu?.dataset.docName || '';
    if (!docId) return;
    await _showMigrateDialog(docId, docName);
    return;
  }

  // REQ-IX-001：文档列表 — 删除文档
  if (action === 'deleteDoc') {
    const docId = menu?.dataset.docId || '';
    const docName = menu?.dataset.docName || '';
    if (!docId) return;
    const confirmed = await showConfirmDialog({
      title: t('ctx.delete_doc_title', '删除文档'),
      body: t('ctx.delete_doc_confirm', { name: docName }) || `确定删除「${docName}」？此操作将级联删除所有分块和向量数据，不可撤销。`,
      confirmText: t('common.delete', '删除'),
      cancelText: t('common.cancel', '取消'),
      danger: true,
    });
    if (!confirmed) return;
    try {
      await docApi.delete(docId);
      toastSuccess(t('ctx.delete_doc_success', '文档已删除'));
      document.dispatchEvent(new CustomEvent('ctx-refresh-documents'));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-IX-003 AC-4：复制文件名
  if (action === 'copyDocName') {
    const docName = menu?.dataset.docName || '';
    copyToClipboard(docName).then((ok) => {
      if (ok) {
        toast(t('chat.copied_to_clipboard'), 'success');
      } else {
        toastError(t('chat.copy_failed'));
      }
    });
    return;
  }

  // S1 v1.5：导出知识库文档为 PDF
  if (action === 'exportPdf') {
    const docId = menu?.dataset.docId || '';
    if (!docId) return;
    await exportDocumentToPdf(docId);
    return;
  }

  // S2 v1.6：导出知识库文档为 HTML
  if (action === 'exportHtml') {
    const docId = menu?.dataset.docId || '';
    if (!docId) return;
    await exportDocumentToHtml(docId);
    return;
  }

  // S4 复盘：重分类文档
  if (action === 'reclassifyDoc') {
    const docId = menu?.dataset.docId || '';
    if (!docId) return;
    try {
      toast(t('ctx.reclassifying', '正在重分类…'), 'info');
      const result = await docExtApi.reclassify(docId);
      const newDomain = typeof result === 'string' ? result : result?.domain || result?.new_domain || '';
      toastSuccess(t('ctx.reclassify_success', '重分类完成') + (newDomain ? ': ' + newDomain : ''));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-EXP-004：导出文档原文
  if (action === 'exportOriginal') {
    const docId = menu?.dataset.docId || '';
    const docName = menu?.dataset.docName || 'document';
    if (!docId) return;
    try {
      const destPath = await saveDialog({
        defaultPath: docName,
        title: t('ctx.export_original_title', '导出文档原文'),
      });
      if (!destPath) return; // 用户取消
      await docExportApi.exportOriginal(docId, destPath);
      toastSuccess(t('ctx.export_original_success', '导出成功'));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-VEC-009：重建索引
  if (action === 'rebuildIndex') {
    const docId = menu?.dataset.docId || '';
    const docName = menu?.dataset.docName || '';
    if (!docId) return;
    const confirmed = await showConfirmDialog({
      title: t('ctx.rebuild_index_title', '重建索引'),
      body: t('ctx.rebuild_index_confirm', { name: docName }),
      confirmText: t('ctx.rebuild_index_btn', '重建'),
      cancelText: t('common.cancel', '取消'),
      danger: false,
    });
    if (!confirmed) return;
    try {
      toast(t('ctx.rebuilding', '正在重建索引…'), 'info');
      await docApi.rebuild(docId);
      toastSuccess(t('ctx.rebuild_success', '索引重建完成'));
    } catch (err) {
      toastError(err);
    }
    return;
  }

  // REQ-IX-003：复制引用片段
  if (action === 'copyChunk') {
    const chunkContent = menu?.dataset.chunkContent || '';
    copyToClipboard(chunkContent).then((ok) => {
      if (ok) {
        toast(t('chat.copied_to_clipboard'), 'success');
      } else {
        toastError(t('chat.copy_failed'));
      }
    });
    return;
  }

  // execCommand 虽已 deprecated，但 Tauri WebView 仍完整支持，
  // 且能正确处理选中范围与剪贴板读写，无需额外权限。
  try {
    document.execCommand(action);
  } catch (_) {
    // 静默忽略 — 用户不会看到错误
  }
}

/**
 * 检查元素是否为可编辑文本输入。
 * @param {Element} el
 * @returns {boolean}
 */
function _isEditable(el) {
  if (!el) return false;
  if (el.isContentEditable) return true;
  const tag = el.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA';
}

/**
 * 检查当前是否有文本被选中。
 * @returns {boolean}
 */
function _hasSelection() {
  const sel = window.getSelection();
  return !!sel && sel.toString().trim().length > 0;
}

// ============================================================
// v1.14 新增：会话列表右键菜单（REQ-IX-001）
// ============================================================

/**
 * 显示会话列表项右键菜单（REQ-IX-001）。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {string} convId - 会话 ID
 * @param {string} convTitle - 会话标题
 */
function _showConvMenu(x, y, convId, convTitle) {
  const menu = $('ctxMenu');
  if (!menu) return;

  const items = [];
  // REQ-RAG-047 AC-1：会话列表项右键菜单含「添加/移除书签」选项
  items.push(_item('convBookmark', t('sidebar.bookmark_add', '添加书签'), '', true));
  items.push(_separator());
  items.push(_item('convRename', t('ctx.rename_conv', '重命名'), '', true));
  items.push(_separator());
  items.push(_item('convExport', t('ctx.export_conv', '导出为 Markdown'), '', true));
  items.push(_separator());
  items.push(_item('convDelete', t('ctx.delete_conv', '删除'), '', true));

  menu.innerHTML = items.join('');
  menu.dataset.convId = convId;
  menu.dataset.convTitle = convTitle;

  // 异步检查书签状态，更新菜单项文案
  (async () => {
    try {
      const isBookmarked = await invoke('is_bookmarked', { conversationId: convId });
      const bmItem = menu.querySelector('[data-action="convBookmark"] span:first-child');
      if (bmItem) {
        bmItem.textContent = isBookmarked
          ? (t('sidebar.bookmark_remove', '移除书签'))
          : (t('sidebar.bookmark_add', '添加书签'));
      }
    } catch {
      // 静默降级
    }
  })();

  _positionMenu(menu, x, y);
}

// ============================================================
// v1.14 新增：消息 Block 右键菜单（REQ-IX-001）
// ============================================================

/**
 * 显示消息 Block 右键菜单（REQ-IX-001）。
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 * @param {Element} msgBlock - 消息 Block DOM 元素
 */
function _showMsgMenu(x, y, msgBlock) {
  const menu = $('ctxMenu');
  if (!menu) return;

  const isAssistant = msgBlock.classList.contains('msg-assistant');
  const isUser = msgBlock.classList.contains('msg-user');
  const isStreaming = get('streaming') || false;

  // 提取消息内容
  let rawMd = '';
  let plainText = '';
  if (isAssistant) {
    const mdEl = msgBlock.querySelector('.md');
    rawMd = mdEl?.dataset.rawMarkdown || mdEl?.textContent || '';
    if (mdEl) {
      const clone = mdEl.cloneNode(true);
      clone.querySelectorAll('.code-header, .copy-btn, .code-lang').forEach((el) => el.remove());
      plainText = clone.textContent || '';
    }
  } else if (isUser) {
    rawMd = msgBlock.dataset.fullText || msgBlock.querySelector('.msg-user-content')?.textContent || '';
    plainText = rawMd;
  }

  const msgId = msgBlock.dataset.msgId || '';
  const query = msgBlock.dataset.query || '';

  const items = [];
  // 复制全文
  items.push(_item('msgCopyFull', t('ctx.copy_full', '复制全文'), '⌘C', rawMd.length > 0));
  // 复制纯文本（仅 assistant 有区别）
  if (isAssistant) {
    items.push(_item('msgCopyPlain', t('ctx.copy_plain', '复制纯文本'), '', plainText.length > 0));
  }

  // 重新生成变体（仅 assistant + 非 streaming）— P0-3 DeepSeek 风格重生成菜单
  if (isAssistant && !isStreaming && query) {
    items.push(_separator());
    items.push(_item('msgRegenerateConcise', t('ctx.regenerate_concise', '简洁重生成'), '', true));
    items.push(_item('msgRegenerateDetailed', t('ctx.regenerate_detailed', '详细重生成'), '', true));
    items.push(_item('msgRegenerateForceSearch', t('ctx.regenerate_force_search', '强制检索重生成'), '', true));
  }

  // 删除消息（非 streaming）
  if (!isStreaming && msgId) {
    items.push(_separator());
    items.push(_item('msgDelete', t('ctx.delete_msg', '删除消息'), '', true));
  }

  menu.innerHTML = items.join('');
  menu.dataset.msgRawMd = rawMd;
  menu.dataset.msgPlain = plainText;
  menu.dataset.msgId = msgId;
  menu.dataset.msgQuery = query;

  _positionMenu(menu, x, y);
}

// ============================================================
// v1.14 新增：统一的菜单定位函数（边界检测）
// ============================================================

/**
 * 定位菜单到指定坐标，自动修正边界溢出。
 * @param {HTMLElement} menu - 菜单 DOM 元素
 * @param {number} x - 鼠标 clientX
 * @param {number} y - 鼠标 clientY
 */
function _positionMenu(menu, x, y) {
  // 先显示再测量尺寸（否则 offsetWidth=0）
  menu.classList.add('visible');
  const mw = menu.offsetWidth;
  const mh = menu.offsetHeight;
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  // 边界溢出修正
  if (x + mw > vw - 4) x = vw - mw - 4;
  if (y + mh > vh - 4) y = vh - mh - 4;
  menu.style.left = Math.max(4, x) + 'px';
  menu.style.top = Math.max(4, y) + 'px';

  _visible = true;
}

// ============================================================
// v1.20 新增：文档跨知识库迁移（REQ-WS-004）
// ============================================================

/**
 * 显示迁移文档对话框（REQ-WS-004 AC-1）。
 * 弹出知识库选择列表，用户选择目标库后执行迁移。
 * @param {string} docId - 文档 ID
 * @param {string} docName - 文档名称
 */
async function _showMigrateDialog(docId, docName) {
  // 获取工作空间列表
  let workspaces = [];
  let currentWsId = 'default';
  try {
    workspaces = await invoke('list_workspaces');
    currentWsId = await invoke('get_current_workspace');
  } catch (err) {
    toastError(String(err));
    return;
  }

  // 过滤掉当前工作空间
  const targets = workspaces.filter((ws) => ws.id !== currentWsId);
  if (targets.length === 0) {
    toastError(t('workspace.migrate_no_target', '没有可迁移的目标知识库'));
    return;
  }

  // 创建选择弹窗
  const overlay = document.createElement('div');
  overlay.className = 'fixed inset-0 z-[80] bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[20vh]';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');

  const dialog = document.createElement('div');
  dialog.className = 'w-[400px] max-w-[90vw] rounded-xl bg-surface-1 border border-border-default shadow-2xl p-5';
  dialog.addEventListener('click', (e) => e.stopPropagation());

  const titleEl = document.createElement('div');
  titleEl.className = 'text-sm font-medium text-text-primary mb-1';
  titleEl.textContent = t('workspace.migrate_title', '迁移文档');
  dialog.appendChild(titleEl);

  const descEl = document.createElement('div');
  descEl.className = 'text-xs text-text-tertiary mb-3';
  descEl.textContent = t('workspace.migrate_desc', { name: docName }) || `选择目标知识库迁移「${docName}」`;
  dialog.appendChild(descEl);

  // 工作空间列表
  const listEl = document.createElement('div');
  listEl.className = 'space-y-1 max-h-48 overflow-y-auto scrollbar-thin';

  for (const ws of targets) {
    const item = document.createElement('div');
    item.className = 'flex items-center gap-2 px-3 py-2 cursor-pointer text-sm rounded-lg text-text-secondary hover:bg-accent/10 transition-colors';

    const nameSpan = document.createElement('span');
    nameSpan.className = 'flex-1 truncate';
    nameSpan.textContent = ws.name;
    item.appendChild(nameSpan);

    // 显示目标库当前文档数
    try {
      const stats = await invoke('get_workspace_stats', { id: ws.id });
      const countEl = document.createElement('span');
      countEl.className = 'shrink-0 text-xs text-text-quaternary';
      countEl.textContent = String(stats.document_count);
      item.appendChild(countEl);
    } catch (_) {
      // 忽略
    }

    item.addEventListener('click', () => {
      overlay.remove();
      _doMigrate(docId, docName, ws.id, ws.name);
    });

    listEl.appendChild(item);
  }
  dialog.appendChild(listEl);

  // 取消按钮
  const btnRow = document.createElement('div');
  btnRow.className = 'flex justify-end mt-4';
  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'px-3 py-1.5 text-sm rounded-lg text-text-secondary hover:bg-surface-3 transition-colors';
  cancelBtn.textContent = t('common.cancel');
  cancelBtn.addEventListener('click', () => {
    overlay.remove();
  });
  btnRow.appendChild(cancelBtn);
  dialog.appendChild(btnRow);

  overlay.appendChild(dialog);
  document.body.appendChild(overlay);

  // 点击遮罩关闭
  overlay.addEventListener('click', () => {
    overlay.remove();
  });

  // Escape 关闭
  overlay.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      overlay.remove();
    }
  });
}

/**
 * 执行文档迁移（REQ-WS-004 AC-2~5）。
 * @param {string} docId - 文档 ID
 * @param {string} docName - 文档名称
 * @param {string} targetWsId - 目标工作空间 ID
 * @param {string} targetWsName - 目标工作空间名称
 */
async function _doMigrate(docId, docName, targetWsId, targetWsName) {
  try {
    toast(t('workspace.migrating', { name: docName }), 'info');
    await invoke('migrate_document', { docId, targetWorkspaceId: targetWsId });
    toastSuccess(t('workspace.migrate_success', { name: docName, target: targetWsName }) || `已迁移「${docName}」到「${targetWsName}」`);
    // 刷新文档列表
    document.dispatchEvent(new CustomEvent('ctx-refresh-documents'));
  } catch (err) {
    toastError(String(err));
  }
}
