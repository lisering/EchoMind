/**
 * EchoMind 多知识库管理模块（REQ-WS-001/003/002/004）。
 *
 * 功能：
 * 1. 知识库选择器（侧栏顶部下拉）
 * 2. 新建知识库
 * 3. 重命名知识库（双击编辑模式）
 * 4. 删除知识库（确认对话框显示数据量，级联清理）
 * 5. 切换知识库后刷新文档/会话列表
 * 6. 持久化当前选择（重启恢复）
 * 7. 配额用量显示（REQ-WS-002）
 * 8. 文档跨知识库迁移（REQ-WS-004）
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError } from './toast.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { isComposingEvent } from './input-utils.js';

/** @type {string} 当前工作空间 ID */
let _currentWorkspaceId = 'default';

/** @type {string} 当前工作空间名称 */
let _currentWorkspaceName = 'Default';

/** @type {boolean} 下拉菜单是否展开 */
let _dropdownOpen = false;

/**
 * 获取当前工作空间 ID。
 * @returns {string}
 */
export function getCurrentWorkspaceId() {
  return _currentWorkspaceId;
}

/**
 * 获取当前工作空间名称。
 * @returns {string}
 */
export function getCurrentWorkspaceName() {
  return _currentWorkspaceName;
}

/** 切换工作空间后的回调（由 main.js 注入，刷新文档+会话列表） */
let _onSwitchWorkspace = async () => {};

/**
 * 设置切换工作空间回调（由 main.js 注入）。
 * @param {() => Promise<void>} fn
 */
export function setSwitchWorkspaceCallback(fn) {
  _onSwitchWorkspace = fn || (async () => {});
}

/**
 * 初始化知识库选择器。
 * 在应用启动时调用：加载工作空间列表 + 恢复当前选择。
 */
export async function initWorkspaceSelector() {
  const toggle = $('workspaceToggle');
  if (!toggle) return;

  toggle.addEventListener('click', (e) => {
    e.stopPropagation();
    toggleDropdown();
  });

  // 点击外部关闭下拉
  document.addEventListener('click', (e) => {
    const selector = $('workspaceSelector');
    if (selector && !selector.contains(e.target) && _dropdownOpen) {
      closeDropdown();
    }
  });

  // 加载当前工作空间
  try {
    _currentWorkspaceId = await invoke('get_current_workspace');
    const workspaces = await invoke('list_workspaces');
    const current = workspaces.find((ws) => ws.id === _currentWorkspaceId);
    if (current) {
      _currentWorkspaceName = current.name;
    }
  } catch (_) {
    // E2E 测试环境或首次启动，使用默认值
  }

  updateWorkspaceDisplay();
}

/**
 * 加载并渲染工作空间列表到下拉菜单。
 */
export async function loadWorkspaceList() {
  const dropdown = $('workspaceDropdown');
  if (!dropdown) return;
  dropdown.innerHTML = '';

  let workspaces = [];
  try {
    workspaces = await invoke('list_workspaces');
  } catch (_) {
    return;
  }

  for (const ws of workspaces) {
    const item = document.createElement('div');
    item.className = `flex items-center gap-2 px-3 py-2 cursor-pointer text-sm transition-colors hover:bg-accent/10 ${ws.id === _currentWorkspaceId ? 'bg-accent/15 text-accent' : 'text-text-secondary'}`;

    // 知识库名称（双击重命名）
    const nameSpan = document.createElement('span');
    nameSpan.className = 'flex-1 truncate workspace-name';
    nameSpan.textContent = ws.name;
    nameSpan.dataset.wsId = ws.id;
    nameSpan.dataset.wsName = ws.name;

    // 双击重命名（REQ-WS-003 AC-1）
    nameSpan.addEventListener('dblclick', (e) => {
      e.stopPropagation();
      startRename(nameSpan, ws);
    });
    // 单击不冒泡到 item（避免双击时第一次 click 关闭下拉）
    nameSpan.addEventListener('click', (e) => {
      e.stopPropagation();
    });

    // 单击切换
    item.addEventListener('click', (e) => {
      // 如果点击的是操作按钮，不切换
      if (e.target.closest('.ws-action-btn')) return;
      if (ws.id !== _currentWorkspaceId) {
        switchWorkspace(ws.id, ws.name);
      }
      closeDropdown();
    });

    item.appendChild(nameSpan);

    // 删除按钮（非默认工作空间 + 多于一个时显示）
    if (ws.id !== 'default' && workspaces.length > 1) {
      const delBtn = document.createElement('button');
      delBtn.className = 'ws-action-btn shrink-0 w-5 h-5 flex items-center justify-center rounded text-text-quaternary hover:text-error transition-colors';
      delBtn.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>';
      delBtn.title = t('workspace.delete');
      delBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        confirmDeleteWorkspace(ws);
      });
      item.appendChild(delBtn);
    }

    // 当前选中标记
    if (ws.id === _currentWorkspaceId) {
      const check = document.createElement('span');
      check.className = 'shrink-0 text-accent';
      check.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg>';
      item.appendChild(check);
    }

    dropdown.appendChild(item);
  }

  // 新建知识库按钮
  const createBtn = document.createElement('div');
  createBtn.className = 'flex items-center gap-2 px-3 py-2 cursor-pointer text-sm border-t border-border-subtle text-accent hover:bg-accent/10 transition-colors';
  createBtn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 5v14M5 12h14"/></svg>';
  createBtn.appendChild(document.createTextNode(t('workspace.create_new')));
  createBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    showCreateDialog();
  });
  dropdown.appendChild(createBtn);
}

/**
 * 切换下拉菜单展开/折叠。
 */
function toggleDropdown() {
  if (_dropdownOpen) {
    closeDropdown();
  } else {
    openDropdown().catch(() => {});
  }
}

/**
 * 打开下拉菜单并加载工作空间列表。
 */
async function openDropdown() {
const dropdown = $('workspaceDropdown');
if (!dropdown) return;
await loadWorkspaceList();
dropdown.classList.remove('hidden');
_dropdownOpen = true;
}

/**
 * 关闭下拉菜单。
 */
function closeDropdown() {
  const dropdown = $('workspaceDropdown');
  if (dropdown) dropdown.classList.add('hidden');
  _dropdownOpen = false;
}

/**
 * 更新当前工作空间显示名称 + 配额用量（REQ-WS-002）。
 */
function updateWorkspaceDisplay() {
  const nameEl = $('workspaceName');
  if (nameEl) {
    nameEl.textContent = _currentWorkspaceName;
  }
  // REQ-WS-002：配额用量显示
  updateQuotaDisplay();
}

/**
 * 更新配额用量显示（REQ-WS-002 AC-2）。
 * 在知识库选择器旁显示 "N/50"（免费版）或 "N/∞"（Pro 版）。
 */
async function updateQuotaDisplay() {
  const quotaEl = $('workspaceQuota');
  if (!quotaEl) return;
  try {
    const [count, limit] = await invoke('get_workspace_quota');
    if (limit === 0) {
      // Pro 版不受限
      quotaEl.textContent = String(count);
      quotaEl.classList.remove('text-warning');
      quotaEl.classList.add('text-text-quaternary');
    } else {
      quotaEl.textContent = `${count}/${limit}`;
      // 接近上限时高亮
      if (count >= limit * 0.9) {
        quotaEl.classList.remove('text-text-quaternary');
        quotaEl.classList.add('text-warning');
      } else {
        quotaEl.classList.remove('text-warning');
        quotaEl.classList.add('text-text-quaternary');
      }
    }
  } catch (_) {
    // E2E 或未初始化环境，静默
  }
}

/**
 * 切换工作空间（REQ-WS-001 AC-3）。
 * @param {string} id - 工作空间 ID
 * @param {string} name - 工作空间名称
 */
async function switchWorkspace(id, name) {
  try {
    await invoke('switch_workspace', { workspaceId: id });
    _currentWorkspaceId = id;
    _currentWorkspaceName = name;
    updateWorkspaceDisplay();
    await _onSwitchWorkspace();
    toast(t('workspace.switched', { name }), 'info');
  } catch (err) {
    toastError(t('workspace.switch_failed') + ': ' + String(err));
  }
}

/**
 * 刷新配额显示（供外部调用，REQ-WS-002）。
 * 在文档导入/删除后调用以同步更新配额用量。
 */
export async function refreshQuotaDisplay() {
  await updateQuotaDisplay();
}

/**
 * 显示新建知识库对话框（REQ-WS-001 AC-2）。
 */
async function showCreateDialog() {
  closeDropdown();
  // 使用内联输入框
  const name = await showTextInputDialog(
    t('workspace.create_title'),
    t('workspace.create_placeholder'),
  );
  if (!name || !name.trim()) return;

  try {
    const id = await invoke('create_workspace', { name: name.trim() });
    // 切换到新建的知识库
    await switchWorkspace(id, name.trim());
    // 重新加载列表
    openDropdown();
  } catch (err) {
    toastError(t('workspace.create_failed') + ': ' + String(err));
  }
}

/**
 * 开始重命名编辑模式（REQ-WS-003 AC-1）。
 * @param {HTMLElement} nameSpan
 * @param {{id: string, name: string}} ws
 */
function startRename(nameSpan, ws) {
  const input = document.createElement('input');
  input.type = 'text';
  input.value = ws.name;
  input.className = 'flex-1 bg-transparent outline-none text-sm text-text-primary border-b border-accent';
  input.maxLength = 100;
  nameSpan.replaceWith(input);
  input.focus();
  input.select();

  const finishRename = async () => {
    const newName = input.value.trim();
    if (!newName || newName === ws.name) {
      // 无变化，恢复
      input.replaceWith(nameSpan);
      return;
    }
    try {
      await invoke('rename_workspace', { id: ws.id, name: newName });
      if (ws.id === _currentWorkspaceId) {
        _currentWorkspaceName = newName;
        updateWorkspaceDisplay();
      }
      // 重新加载列表
      loadWorkspaceList();
      toast(t('workspace.renamed', { name: newName }), 'info');
    } catch (err) {
      toastError(t('workspace.rename_failed') + ': ' + String(err));
      input.replaceWith(nameSpan);
    }
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      if (isComposingEvent(e)) return; // IME 组合中不触发
      e.preventDefault();
      finishRename();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      input.replaceWith(nameSpan);
    }
  });
  input.addEventListener('blur', finishRename);
}

/**
 * 确认删除工作空间（REQ-WS-003 AC-3/AC-4/AC-5）。
 * @param {{id: string, name: string}} ws
 */
async function confirmDeleteWorkspace(ws) {
  closeDropdown();

  // AC-5：最后一个知识库禁删
  let workspaces = [];
  try {
    workspaces = await invoke('list_workspaces');
  } catch (_) {
    return;
  }
  if (workspaces.length <= 1) {
    toastError(t('workspace.cannot_delete_last'));
    return;
  }
  if (ws.id === 'default') {
    toastError(t('workspace.cannot_delete_default'));
    return;
  }

  // AC-3：显示将删除的数据量
  let stats = { document_count: 0, conversation_count: 0 };
  try {
    stats = await invoke('get_workspace_stats', { id: ws.id });
  } catch (_) {
    // 忽略统计失败，继续删除流程
  }

  const ok = await showConfirmDialog({
    title: t('workspace.delete_title'),
    body: t('workspace.delete_confirm', {
      name: ws.name,
      docs: stats.document_count,
      conversations: stats.conversation_count,
    }),
    confirmText: t('workspace.delete'),
    danger: true,
  });

  if (!ok) return;

  try {
    await invoke('delete_workspace', { id: ws.id });
    // 如果删除的是当前工作空间，切换到 default
    if (ws.id === _currentWorkspaceId) {
      _currentWorkspaceId = 'default';
      const defaultWs = workspaces.find((w) => w.id === 'default');
      _currentWorkspaceName = defaultWs ? defaultWs.name : 'Default';
      updateWorkspaceDisplay();
      await _onSwitchWorkspace();
    }
    toast(t('workspace.deleted', { name: ws.name }), 'info');
    openDropdown();
  } catch (err) {
    toastError(t('workspace.delete_failed') + ': ' + String(err));
  }
}

/**
 * 简易文本输入对话框（基于 showConfirmDialog 模式）。
 * @param {string} title
 * @param {string} placeholder
 * @returns {Promise<string|null>}
 */
function showTextInputDialog(title, placeholder) {
  return new Promise((resolve) => {
    // 创建对话框
    const overlay = document.createElement('div');
    overlay.className = 'fixed inset-0 z-[80] bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[20vh]';
    overlay.setAttribute('role', 'dialog');
    overlay.setAttribute('aria-modal', 'true');

    const dialog = document.createElement('div');
    dialog.className = 'w-[400px] max-w-[90vw] rounded-xl bg-surface-1 border border-border-default shadow-2xl p-5';
    dialog.addEventListener('click', (e) => e.stopPropagation());

    const titleEl = document.createElement('div');
    titleEl.className = 'text-sm font-medium text-text-primary mb-3';
    titleEl.textContent = title;
    dialog.appendChild(titleEl);

    const input = document.createElement('input');
    input.type = 'text';
    input.placeholder = placeholder;
    input.className = 'w-full bg-surface-2 border border-border-default rounded-lg px-3 py-2 text-sm text-text-primary placeholder:text-text-quaternary outline-none focus:border-accent';
    dialog.appendChild(input);

    const btnRow = document.createElement('div');
    btnRow.className = 'flex justify-end gap-2 mt-4';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'px-3 py-1.5 text-sm rounded-lg text-text-secondary hover:bg-surface-3 transition-colors';
    cancelBtn.textContent = t('common.cancel');
    cancelBtn.addEventListener('click', () => {
      overlay.remove();
      resolve(null);
    });

    const okBtn = document.createElement('button');
    okBtn.className = 'px-3 py-1.5 text-sm rounded-lg bg-accent text-white hover:bg-accent/80 transition-colors';
    okBtn.textContent = t('common.ok');
    okBtn.addEventListener('click', () => {
      const val = input.value.trim();
      overlay.remove();
      resolve(val || null);
    });

    btnRow.appendChild(cancelBtn);
    btnRow.appendChild(okBtn);
    dialog.appendChild(btnRow);
    overlay.appendChild(dialog);
    document.body.appendChild(overlay);

    // 点击遮罩关闭
    overlay.addEventListener('click', () => {
      overlay.remove();
      resolve(null);
    });

    input.focus();
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        if (isComposingEvent(e)) return; // IME 组合中不触发
        e.preventDefault();
        const val = input.value.trim();
        overlay.remove();
        resolve(val || null);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        overlay.remove();
        resolve(null);
      }
    });
  });
}
