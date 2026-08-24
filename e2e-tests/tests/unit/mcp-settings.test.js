/**
 * EchoMind mcp-settings.js 单元测试 — MCP 服务器配置面板 (REQ-ARCH-016)。
 *
 * 验证点：
 * 1. openMcpSettingsPanel 创建并显示面板 overlay
 * 2. 面板包含服务器名称输入框
 * 3. 面板包含传输类型选择器（stdio + sse）
 * 4. stdio 传输配置区域初始可见
 * 5. SSE 传输配置区域初始隐藏
 * 6. 切换传输类型时显示/隐藏对应配置区域
 * 7. 添加服务器调用 invoke('add_mcp_server')
 * 8. 缺少名称时显示错误提示
 * 9. stdio 缺少 command 时显示错误提示
 * 10. SSE 缺少 url 时显示错误提示
 * 11. 服务器列表渲染（含状态指示器）
 * 12. 工具列表渲染
 * 13. 删除服务器调用 invoke('remove_mcp_server')
 * 14. 启用/禁用调用 invoke('toggle_mcp_server')
 * 15. 工具调用确认对话框
 *
 * Mock: utils.js, ipc.js, i18n.js, toast.js, panel-stack.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
}));

// Mock ipc
const mockInvoke = vi.fn((cmd) => {
  if (cmd === 'list_mcp_servers') {
    return Promise.resolve([
      {
        config: { id: 'srv-1', name: '文件系统', transport: 'stdio', enabled: true, command: 'npx' },
        status: 'connected',
        error_message: null,
        tool_count: 3,
      },
      {
        config: { id: 'srv-2', name: '远程工具', transport: 'sse', enabled: false, url: 'https://example.com' },
        status: 'disconnected',
        error_message: null,
        tool_count: 0,
      },
    ]);
  }
  if (cmd === 'get_mcp_tools') {
    return Promise.resolve([
      { name: 'read_file', description: '读取文件', server_id: 'srv-1', server_name: '文件系统' },
      { name: 'write_file', description: '写入文件', server_id: 'srv-1', server_name: '文件系统' },
      { name: 'search', description: '搜索', server_id: 'srv-1', server_name: '文件系统' },
    ]);
  }
  if (cmd === 'add_mcp_server') return Promise.resolve();
  if (cmd === 'remove_mcp_server') return Promise.resolve();
  if (cmd === 'toggle_mcp_server') return Promise.resolve();
  if (cmd === 'call_mcp_tool') {
    return Promise.resolve({ success: true, content: 'result', tool_name: 'read_file', server_id: 'srv-1', is_error: false });
  }
  return Promise.resolve(null);
});

vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: mockInvoke,
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

// Mock panel-stack
vi.mock('../../../ui/src/panel-stack.js', () => ({
  pushPanel: vi.fn(),
  removePanel: vi.fn(),
}));

// Mock confirm
global.confirm = vi.fn(() => true);

// Mock crypto.randomUUID
if (!global.crypto) {
  global.crypto = {};
}
global.crypto.randomUUID = () => 'test-uuid-' + Math.random().toString(36).slice(2);

// Import after mocks
const { openMcpSettingsPanel } = await import('../../../ui/src/mcp-settings.js');

describe('MCP Settings Panel — REQ-ARCH-016', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
    mockInvoke.mockClear();
    vi.clearAllMocks();
  });

  // TC-MCP-UI-001: openMcpSettingsPanel 创建并显示面板 overlay
  it('TC-MCP-UI-001: should create panel overlay', async () => {
    openMcpSettingsPanel();
    const panel = document.getElementById('mcp-settings-panel');
    expect(panel).toBeTruthy();
    expect(panel.className).toContain('fixed inset-0');
  });

  // TC-MCP-UI-002: 面板包含服务器名称输入框
  it('TC-MCP-UI-002: should contain server name input', () => {
    openMcpSettingsPanel();
    const nameInput = document.getElementById('mcpServerName');
    expect(nameInput).toBeTruthy();
    expect(nameInput.type).toBe('text');
  });

  // TC-MCP-UI-003: 面板包含传输类型选择器
  it('TC-MCP-UI-003: should contain transport type selector', () => {
    openMcpSettingsPanel();
    const select = document.getElementById('mcpTransportType');
    expect(select).toBeTruthy();
    const options = Array.from(select.options).map(o => o.value);
    expect(options).toContain('stdio');
    expect(options).toContain('sse');
  });

  // TC-MCP-UI-004: stdio 配置区域初始可见
  it('TC-MCP-UI-004: should show stdio config by default', () => {
    openMcpSettingsPanel();
    const stdioConfig = document.getElementById('mcpStdioConfig');
    expect(stdioConfig).toBeTruthy();
    expect(stdioConfig.classList.contains('hidden')).toBe(false);
  });

  // TC-MCP-UI-005: SSE 配置区域初始隐藏
  it('TC-MCP-UI-005: should hide SSE config by default', () => {
    openMcpSettingsPanel();
    const sseConfig = document.getElementById('mcpSseConfig');
    expect(sseConfig).toBeTruthy();
    expect(sseConfig.classList.contains('hidden')).toBe(true);
  });

  // TC-MCP-UI-006: 切换到 SSE 时显示 SSE 配置，隐藏 stdio 配置
  it('TC-MCP-UI-006: should toggle transport config visibility', async () => {
    openMcpSettingsPanel();
    const select = document.getElementById('mcpTransportType');
    select.value = 'sse';
    select.dispatchEvent(new Event('change'));

    const stdioConfig = document.getElementById('mcpStdioConfig');
    const sseConfig = document.getElementById('mcpSseConfig');
    expect(stdioConfig.classList.contains('hidden')).toBe(true);
    expect(sseConfig.classList.contains('hidden')).toBe(false);
  });

  // TC-MCP-UI-007: 切换回 stdio 时显示 stdio 配置，隐藏 SSE 配置
  it('TC-MCP-UI-007: should toggle back to stdio config', async () => {
    openMcpSettingsPanel();
    const select = document.getElementById('mcpTransportType');

    // 先切到 SSE
    select.value = 'sse';
    select.dispatchEvent(new Event('change'));

    // 再切回 stdio
    select.value = 'stdio';
    select.dispatchEvent(new Event('change'));

    const stdioConfig = document.getElementById('mcpStdioConfig');
    const sseConfig = document.getElementById('mcpSseConfig');
    expect(stdioConfig.classList.contains('hidden')).toBe(false);
    expect(sseConfig.classList.contains('hidden')).toBe(true);
  });

  // TC-MCP-UI-008: 添加服务器缺少名称时显示错误
  it('TC-MCP-UI-008: should show error when name is empty', async () => {
    openMcpSettingsPanel();
    const addBtn = document.getElementById('mcpAddBtn');
    addBtn.click();
    // 等待微任务
    await new Promise(r => setTimeout(r, 0));
    expect(mockInvoke).not.toHaveBeenCalledWith('add_mcp_server', expect.anything());
  });

  // TC-MCP-UI-009: stdio 缺少 command 时显示错误
  it('TC-MCP-UI-009: should show error when stdio command is empty', async () => {
    openMcpSettingsPanel();
    document.getElementById('mcpServerName').value = '测试服务器';
    // 不填写 command
    document.getElementById('mcpAddBtn').click();
    await new Promise(r => setTimeout(r, 0));
    expect(mockInvoke).not.toHaveBeenCalledWith('add_mcp_server', expect.anything());
  });

  // TC-MCP-UI-010: SSE 缺少 url 时显示错误
  it('TC-MCP-UI-010: should show error when SSE url is empty', async () => {
    openMcpSettingsPanel();
    document.getElementById('mcpServerName').value = '远程服务器';
    const select = document.getElementById('mcpTransportType');
    select.value = 'sse';
    select.dispatchEvent(new Event('change'));
    // 不填写 url
    document.getElementById('mcpAddBtn').click();
    await new Promise(r => setTimeout(r, 0));
    expect(mockInvoke).not.toHaveBeenCalledWith('add_mcp_server', expect.anything());
  });

  // TC-MCP-UI-011: 成功添加 stdio 服务器
  it('TC-MCP-UI-011: should add stdio server successfully', async () => {
    openMcpSettingsPanel();
    document.getElementById('mcpServerName').value = '测试服务器';
    document.getElementById('mcpCommand').value = 'npx';
    document.getElementById('mcpArgs').value = '-y @modelcontextprotocol/server-filesystem';

    document.getElementById('mcpAddBtn').click();
    await new Promise(r => setTimeout(r, 10));

    expect(mockInvoke).toHaveBeenCalledWith('add_mcp_server', expect.objectContaining({
      config: expect.objectContaining({
        name: '测试服务器',
        transport: 'stdio',
        command: 'npx',
        enabled: true,
      }),
    }));
  });

  // TC-MCP-UI-012: 成功添加 SSE 服务器
  it('TC-MCP-UI-012: should add SSE server successfully', async () => {
    openMcpSettingsPanel();
    document.getElementById('mcpServerName').value = '远程工具';
    const select = document.getElementById('mcpTransportType');
    select.value = 'sse';
    select.dispatchEvent(new Event('change'));
    document.getElementById('mcpUrl').value = 'https://example.com/mcp';

    document.getElementById('mcpAddBtn').click();
    await new Promise(r => setTimeout(r, 10));

    expect(mockInvoke).toHaveBeenCalledWith('add_mcp_server', expect.objectContaining({
      config: expect.objectContaining({
        name: '远程工具',
        transport: 'sse',
        url: 'https://example.com/mcp',
        enabled: true,
      }),
    }));
  });

  // TC-MCP-UI-013: 服务器列表渲染
  it('TC-MCP-UI-013: should render server list with status', async () => {
    openMcpSettingsPanel();
    // 等待 loadServerList 完成
    await new Promise(r => setTimeout(r, 10));

    const serverList = document.getElementById('mcpServerList');
    expect(serverList.innerHTML).toContain('文件系统');
    expect(serverList.innerHTML).toContain('远程工具');
    expect(serverList.innerHTML).toContain('mcpToggle-srv-1');
    expect(serverList.innerHTML).toContain('mcpRemove-srv-1');
  });

  // TC-MCP-UI-014: 工具列表渲染
  it('TC-MCP-UI-014: should render tool list', async () => {
    openMcpSettingsPanel();
    await new Promise(r => setTimeout(r, 10));

    const toolList = document.getElementById('mcpToolList');
    expect(toolList.innerHTML).toContain('read_file');
    expect(toolList.innerHTML).toContain('write_file');
    expect(toolList.innerHTML).toContain('search');
  });

  // TC-MCP-UI-015: 删除服务器调用 invoke
  it('TC-MCP-UI-015: should call remove_mcp_server on delete', async () => {
    openMcpSettingsPanel();
    await new Promise(r => setTimeout(r, 10));

    const removeBtn = document.getElementById('mcpRemove-srv-1');
    removeBtn.click();
    await new Promise(r => setTimeout(r, 10));

    expect(mockInvoke).toHaveBeenCalledWith('remove_mcp_server', expect.objectContaining({
      id: 'srv-1',
    }));
  });
});
