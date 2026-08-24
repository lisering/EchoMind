/**
 * EchoMind MCP 服务器配置面板（REQ-ARCH-016 前端 UI）。
 *
 * 功能：
 * - 添加/删除/启用/禁用 MCP 服务器
 * - 支持 stdio + SSE 两种传输类型
 * - 服务器连接状态实时显示（connected/disconnected/error）
 * - MCP 工具列表展示
 * - 工具调用确认对话框
 *
 * 使用方式：
 * 1. 设置面板「高级」区域点击「MCP 服务器」按钮打开面板
 * 2. 添加服务器 → 选择传输类型 → 填写配置
 * 3. 查看工具列表 → 确认调用
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { pushPanel, removePanel } from './panel-stack.js';

/** 面板 ID 常量 */
const PANEL_ID = 'mcp-settings-panel';

/** 面板 DOM 引用 */
let _overlay = null;

/**
 * 打开 MCP 服务器配置面板。
 *
 * 创建全屏覆盖层 + 面板 DOM，加载服务器列表。
 */
export function openMcpSettingsPanel() {
  if (_overlay) {
    _overlay.remove();
    _overlay = null;
  }

  _overlay = document.createElement('div');
  _overlay.id = PANEL_ID;
  _overlay.className = 'fixed inset-0 z-50 flex items-center justify-center bg-black/60';
  _overlay.innerHTML = _buildPanelHTML();

  document.body.appendChild(_overlay);

  // 绑定关闭事件
  const closeBtn = _overlay.querySelector('#mcpCloseBtn');
  if (closeBtn) {
    closeBtn.addEventListener('click', closePanel);
  }
  _overlay.addEventListener('click', (e) => {
    if (e.target === _overlay) closePanel();
  });

  // 绑定传输类型切换
  const transportSelect = _overlay.querySelector('#mcpTransportType');
  if (transportSelect) {
    transportSelect.addEventListener('change', onTransportChange);
  }

  // 绑定添加按钮
  const addBtn = _overlay.querySelector('#mcpAddBtn');
  if (addBtn) {
    addBtn.addEventListener('click', onAddServer);
  }

  // 加载服务器列表
  loadServerList();
}

/**
 * 关闭面板。
 */
function closePanel() {
  if (_overlay) {
    _overlay.remove();
    _overlay = null;
  }
}

/**
 * 构建面板 HTML。
 */
function _buildPanelHTML() {
  return `
    <div class="relative w-full max-w-3xl max-h-[90vh] overflow-y-auto bg-surface-1 rounded-2xl shadow-2xl border border-border-default">
      <!-- Header -->
      <div class="sticky top-0 z-10 flex items-center justify-between px-6 py-4 bg-surface-1 border-b border-border-default">
        <h2 class="text-lg font-semibold text-text-primary">${t('mcp.title') || 'MCP 服务器'}</h2>
        <button id="mcpCloseBtn" class="p-2 rounded-lg hover:bg-surface-2 transition-colors">
          <svg class="w-5 h-5 text-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
          </svg>
        </button>
      </div>

      <!-- Body -->
      <div class="px-6 py-4 space-y-6">
        <!-- 添加服务器表单 -->
        <div class="p-4 bg-surface-2 rounded-xl border border-border-default">
          <h3 class="text-sm font-medium text-text-primary mb-3">${t('mcp.add_server') || '添加服务器'}</h3>
          <div class="space-y-3">
            <input id="mcpServerName" type="text" placeholder="${t('mcp.server_name') || '服务器名称'}"
              class="w-full px-3 py-2 text-sm rounded-lg bg-surface-1 border border-border-default text-text-primary placeholder-text-quaternary focus:border-accent focus:outline-none" />

            <select id="mcpTransportType" class="w-full px-3 py-2 text-sm rounded-lg bg-surface-1 border border-border-default text-text-primary focus:border-accent focus:outline-none">
              <option value="stdio">${t('mcp.transport_stdio') || 'stdio（子进程）'}</option>
              <option value="sse">${t('mcp.transport_sse') || 'SSE（HTTP）'}</option>
            </select>

            <!-- stdio 配置 -->
            <div id="mcpStdioConfig" class="space-y-2">
              <input id="mcpCommand" type="text" placeholder="${t('mcp.command') || '可执行文件路径（如 npx, node, python3）'}"
                class="w-full px-3 py-2 text-sm rounded-lg bg-surface-1 border border-border-default text-text-primary placeholder-text-quaternary focus:border-accent focus:outline-none" />
              <input id="mcpArgs" type="text" placeholder="${t('mcp.args') || '参数（空格分隔）'}"
                class="w-full px-3 py-2 text-sm rounded-lg bg-surface-1 border border-border-default text-text-primary placeholder-text-quaternary focus:border-accent focus:outline-none" />
            </div>

            <!-- SSE 配置 -->
            <div id="mcpSseConfig" class="space-y-2 hidden">
              <input id="mcpUrl" type="text" placeholder="${t('mcp.url') || '服务器 URL（如 https://example.com/mcp）'}"
                class="w-full px-3 py-2 text-sm rounded-lg bg-surface-1 border border-border-default text-text-primary placeholder-text-quaternary focus:border-accent focus:outline-none" />
            </div>

            <button id="mcpAddBtn" class="px-4 py-2 text-sm rounded-lg bg-accent text-white hover:opacity-90 transition-opacity">
              ${t('mcp.add') || '添加'}
            </button>
          </div>
        </div>

        <!-- 服务器列表 -->
        <div>
          <h3 class="text-sm font-medium text-text-primary mb-3">${t('mcp.server_list') || '已配置服务器'}</h3>
          <div id="mcpServerList" class="space-y-2">
            <p class="text-sm text-text-quaternary py-4 text-center">${t('mcp.loading') || '加载中...'}</p>
          </div>
        </div>

        <!-- 工具列表 -->
        <div>
          <h3 class="text-sm font-medium text-text-primary mb-3">${t('mcp.tools') || '可用工具'}</h3>
          <div id="mcpToolList" class="space-y-2">
          </div>
        </div>
      </div>
    </div>
  `;
}

/**
 * 传输类型切换事件。
 */
function onTransportChange(e) {
  const type = e.target.value;
  const stdioConfig = _overlay?.querySelector('#mcpStdioConfig');
  const sseConfig = _overlay?.querySelector('#mcpSseConfig');
  if (stdioConfig && sseConfig) {
    stdioConfig.classList.toggle('hidden', type !== 'stdio');
    sseConfig.classList.toggle('hidden', type !== 'sse');
  }
}

/**
 * 加载服务器列表。
 */
async function loadServerList() {
  const listEl = _overlay?.querySelector('#mcpServerList');
  if (!listEl) return;

  try {
    const servers = await invoke('list_mcp_servers');
    if (!servers || servers.length === 0) {
      listEl.innerHTML = `<p class="text-sm text-text-quaternary py-4 text-center">${t('mcp.no_servers') || '暂无服务器'}</p>`;
      return;
    }

    listEl.innerHTML = servers.map(s => _buildServerItemHTML(s)).join('');

    // 绑定按钮事件
    servers.forEach(s => {
      const toggleBtn = listEl.querySelector(`#mcpToggle-${s.config.id}`);
      if (toggleBtn) {
        toggleBtn.addEventListener('click', () => onToggleServer(s.config.id, !s.config.enabled));
      }
      const removeBtn = listEl.querySelector(`#mcpRemove-${s.config.id}`);
      if (removeBtn) {
        removeBtn.addEventListener('click', () => onRemoveServer(s.config.id));
      }
    });

    // 加载工具列表
    loadToolList();
  } catch (e) {
    listEl.innerHTML = `<p class="text-sm text-red-400 py-4 text-center">${e}</p>`;
  }
}

/**
 * 构建单个服务器项 HTML。
 */
function _buildServerItemHTML(s) {
  const statusColor = s.status === 'connected' ? 'bg-green-500' :
                      s.status === 'error' ? 'bg-red-500' : 'bg-slate-500';
  const statusText = s.status === 'connected' ? (t('mcp.status_connected') || '已连接') :
                     s.status === 'error' ? (t('mcp.status_error') || '错误') :
                     (t('mcp.status_disconnected') || '未连接');

  return `
    <div class="flex items-center justify-between p-3 bg-surface-2 rounded-lg border border-border-default">
      <div class="flex items-center gap-3">
        <span class="w-2 h-2 rounded-full ${statusColor}"></span>
        <div>
          <p class="text-sm text-text-primary">${s.config.name}</p>
          <p class="text-xs text-text-quaternary">
            ${s.config.transport} · ${s.tool_count} ${t('mcp.tools_count') || '工具'}
            ${s.error_message ? ' · ' + s.error_message : ''}
          </p>
        </div>
      </div>
      <div class="flex items-center gap-2">
        <button id="mcpToggle-${s.config.id}" class="px-2 py-1 text-xs rounded-lg ${s.config.enabled ? 'bg-surface-1 text-text-secondary' : 'bg-accent text-white'} hover:opacity-90 transition-opacity">
          ${s.config.enabled ? (t('mcp.disable') || '禁用') : (t('mcp.enable') || '启用')}
        </button>
        <button id="mcpRemove-${s.config.id}" class="px-2 py-1 text-xs rounded-lg bg-red-500/15 text-red-300 hover:bg-red-500/25 transition-colors">
          ${t('mcp.remove') || '删除'}
        </button>
      </div>
    </div>
  `;
}

/**
 * 加载工具列表。
 */
async function loadToolList() {
  const toolEl = _overlay?.querySelector('#mcpToolList');
  if (!toolEl) return;

  try {
    const tools = await invoke('get_mcp_tools');
    if (!tools || tools.length === 0) {
      toolEl.innerHTML = `<p class="text-sm text-text-quaternary py-2">${t('mcp.no_tools') || '暂无可用工具'}</p>`;
      return;
    }

    toolEl.innerHTML = tools.map(tool => `
      <div class="p-3 bg-surface-2 rounded-lg border border-border-default">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-sm text-text-primary">${tool.name}</p>
            <p class="text-xs text-text-quaternary">${tool.server_name} · ${tool.description || ''}</p>
          </div>
          <button class="px-2 py-1 text-xs rounded-lg bg-accent text-white hover:opacity-90 transition-opacity"
            data-server-id="${tool.server_id}" data-tool-name="${tool.name}">
            ${t('mcp.call_tool') || '调用'}
          </button>
        </div>
      </div>
    `).join('');

    // 暴露调用函数
    (/** @type {any} */ (window)).__mcpCallTool = async (serverId, toolName) => {
      const confirmed = confirm(`${t('mcp.confirm_call') || '确认调用工具'}: ${toolName}?\n\n${t('mcp.confirm_warning') || 'MCP 工具具有任意执行能力，请确保信任此服务器。'}`);
      if (!confirmed) return;

      try {
        const result = await invoke('call_mcp_tool', {
          serverId,
          toolName,
          arguments: {}
        });
        if (result.success) {
          toastSuccess(result.content || t('mcp.tool_success') || '工具调用成功');
        } else {
          toastError(result.content || t('mcp.tool_error') || '工具调用失败');
        }
      } catch (e) {
        toastError(String(e));
      }
    };
  } catch (e) {
    toolEl.innerHTML = `<p class="text-sm text-red-400 py-2">${e}</p>`;
  }
}

/**
 * 添加服务器。
 */
async function onAddServer() {
  const name = _overlay?.querySelector('#mcpServerName')?.value?.trim();
  const transport = _overlay?.querySelector('#mcpTransportType')?.value;
  const command = _overlay?.querySelector('#mcpCommand')?.value?.trim();
  const args = _overlay?.querySelector('#mcpArgs')?.value?.trim();
  const url = _overlay?.querySelector('#mcpUrl')?.value?.trim();

  if (!name) {
    toastError(t('mcp.name_required') || '请输入服务器名称');
    return;
  }

  const config = {
    id: crypto.randomUUID(),
    name,
    transport,
    enabled: true,
    args: args ? args.split(/\s+/).filter(Boolean) : [],
    env: [],
    headers: [],
  };

  if (transport === 'stdio') {
    if (!command) {
      toastError(t('mcp.command_required') || '请输入可执行文件路径');
      return;
    }
    config.command = command;
  } else {
    if (!url) {
      toastError(t('mcp.url_required') || '请输入服务器 URL');
      return;
    }
    config.url = url;
  }

  try {
    await invoke('add_mcp_server', { config });
    toastSuccess(t('mcp.added') || '服务器已添加');
    // 清空表单
    _overlay.querySelector('#mcpServerName').value = '';
    _overlay.querySelector('#mcpCommand').value = '';
    _overlay.querySelector('#mcpArgs').value = '';
    _overlay.querySelector('#mcpUrl').value = '';
    // 刷新列表
    loadServerList();
  } catch (e) {
    toastError(String(e));
  }
}

/**
 * 切换服务器启用/禁用。
 */
async function onToggleServer(id, enabled) {
  try {
    await invoke('toggle_mcp_server', { id, enabled });
    toast(enabled ? (t('mcp.enabled') || '已启用') : (t('mcp.disabled') || '已禁用'));
    loadServerList();
  } catch (e) {
    toastError(String(e));
  }
}

/**
 * 删除服务器。
 */
async function onRemoveServer(id) {
  const confirmed = confirm(t('mcp.confirm_remove') || '确认删除此服务器？');
  if (!confirmed) return;

  try {
    await invoke('remove_mcp_server', { id });
    toastSuccess(t('mcp.removed') || '服务器已删除');
    loadServerList();
  } catch (e) {
    toastError(String(e));
  }
}
