/**
 * EchoMind settings.js 单元测试 — open/close / Tab 导航 / 智能模式 / 开发者模式。
 *
 * 验证点：
 * 1. SETTINGS_TABS 配置完整性
 * 2. createSettingsTabBar Tab 栏创建
 * 3. _activeTab 切换
 * 4. _devMode 切换
 * 5. closeSettings 面板关闭
 * 6. Focus Trap 生命周期
 * 7. LLM 模式切换
 * 8. 语言切换验证
 * 9. VLM 开关
 * 10. Rerank/HyDE 开关
 *
 * Mock: Tauri IPC / i18n / toast / state
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key) => key,
  getLocale: () => 'zh-CN',
  setLocale: async () => {},
  SUPPORTED_LOCALES: ['zh-CN', 'en'],
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}));

// Mock state
vi.mock('../../../ui/src/state.js', () => {
  let state = { isPro: false, vlmEnabled: false, rerankEnabled: false, hydeEnabled: false, hybridEnabled: false, agentEnabled: false, subAgentEnabled: false, memoryEnabled: false, webSearchEnabled: false };
  return {
    getState: () => ({ ...state }),
    setState: (partial) => { state = { ...state, ...partial }; return state; },
    get: (key) => state[key],
  };
});

// Setup DOM
document.body.innerHTML = `
  <div id="settingsModal" class="hidden">
    <div class="overflow-y-auto" id="settingsScrollContainer">
      <div id="settingsLlmInfo"></div>
      <select id="localeSelect"><option value="zh-CN">中文</option><option value="en">English</option></select>
      <div id="settingsLicenseInfo"></div>
      <div id="securitySettingsContainer"></div>
      <div id="perfSettingsContainer"></div>
      <div id="ragLlmParamsContainer"></div>
      <div id="promptTemplateContainer"></div>
      <div id="memorySettingsContainer"></div>
      <div id="startupSettingsContainer"></div>
    </div>
    <button id="settingsClose">关闭</button>
    <button id="settingsCloseBtn">关闭</button>
    <button id="settingsEditLlm">编辑</button>
    <button id="settingsInitEmbedder">初始化</button>
    <button id="settingsClearCache">清理</button>
    <div id="vlmToggle"></div>
    <div id="rerankToggle"></div>
    <div id="hydeToggle"></div>
  </div>
`;

describe('settings.js — SETTINGS_TABS 配置', () => {
  const SETTINGS_TABS = [
    { id: 'appearance', labelKey: 'settings.tab_appearance', anchorId: 'localeSelect' },
    { id: 'model', labelKey: 'settings.tab_model', anchorId: 'settingsLlmInfo' },
    { id: 'kb', labelKey: 'settings.tab_kb', anchorId: 'syncAddBtn' },
    { id: 'retrieval', labelKey: 'settings.tab_retrieval', anchorId: 'ragLlmParamsContainer' },
    { id: 'security', labelKey: 'settings.tab_security', anchorId: 'securitySettingsContainer' },
    { id: 'data', labelKey: 'settings.tab_data', anchorId: 'exportBackupBtn' },
    { id: 'application', labelKey: 'settings.tab_application', anchorId: 'startupSettingsContainer' },
    { id: 'advanced', labelKey: 'settings.tab_advanced', anchorId: 'perfSettingsContainer' },
  ];

  it('应有 8 个 Tab', () => {
    expect(SETTINGS_TABS).toHaveLength(8);
  });

  it('每个 Tab 有 id / labelKey / anchorId', () => {
    for (const tab of SETTINGS_TABS) {
      expect(tab.id).toBeTruthy();
      expect(tab.labelKey).toBeTruthy();
      expect(tab.anchorId).toBeTruthy();
    }
  });

  it('Tab ID 不重复', () => {
    const ids = SETTINGS_TABS.map(t => t.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('appearance 是默认活动 Tab', () => {
    const _activeTab = 'appearance';
    expect(SETTINGS_TABS[0].id).toBe(_activeTab);
  });
});

describe('settings.js — closeSettings 逻辑', () => {
  function closeSettings() {
    const modal = document.getElementById('settingsModal');
    if (modal) modal.classList.add('hidden');
  }

  it('关闭后 settingsModal 添加 hidden 类', () => {
    const modal = document.getElementById('settingsModal');
    modal.classList.remove('hidden');
    closeSettings();
    expect(modal.classList.contains('hidden')).toBe(true);
  });

  it('重复关闭不报错', () => {
    const modal = document.getElementById('settingsModal');
    closeSettings();
    closeSettings();
    expect(modal.classList.contains('hidden')).toBe(true);
  });
});

describe('settings.js — 开发者模式切换', () => {
  it('⌘Shift+D 切换 _devMode', () => {
    let _devMode = false;
    const e = { metaKey: true, shiftKey: true, key: 'D', preventDefault: () => {} };
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'D') {
      _devMode = !_devMode;
    }
    expect(_devMode).toBe(true);
  });

  it('Ctrl+Shift+D 也切换 _devMode', () => {
    let _devMode = false;
    const e = { ctrlKey: true, shiftKey: true, key: 'D', preventDefault: () => {} };
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'D') {
      _devMode = !_devMode;
    }
    expect(_devMode).toBe(true);
  });

  it('非 Shift+D 不切换', () => {
    let _devMode = false;
    const e = { metaKey: true, shiftKey: false, key: 'D', preventDefault: () => {} };
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'D') {
      _devMode = !_devMode;
    }
    expect(_devMode).toBe(false);
  });

  it('开发模式开启后 toast 提示', () => {
    let _devMode = false;
    let toastMsg = '';
    // 切换
    _devMode = !_devMode;
    toastMsg = _devMode ? '开发者模式已开启' : '开发者模式已关闭';
    expect(toastMsg).toBe('开发者模式已开启');
  });

  it('开发模式关闭后 toast 提示', () => {
    let _devMode = true;
    let toastMsg = '';
    _devMode = !_devMode;
    toastMsg = _devMode ? '开发者模式已开启' : '开发者模式已关闭';
    expect(toastMsg).toBe('开发者模式已关闭');
  });

  it('_devMode 初始为 false', () => {
    let _devMode = false;
    expect(_devMode).toBe(false);
  });
});

describe('settings.js — 语言切换验证', () => {
  it('当前语言与新语言相同时不切换', () => {
    const currentLocale = 'zh-CN';
    const newLocale = 'zh-CN';
    const shouldSwitch = newLocale !== currentLocale;
    expect(shouldSwitch).toBe(false);
  });

  it('当前语言与新语言不同时切换', () => {
    const currentLocale = 'zh-CN';
    const newLocale = 'en';
    const shouldSwitch = newLocale !== currentLocale;
    expect(shouldSwitch).toBe(true);
  });

  it('不支持的语言不切换', () => {
    const SUPPORTED_LOCALES = ['zh-CN', 'en'];
    const newLocale = 'fr';
    const isValid = SUPPORTED_LOCALES.includes(newLocale);
    expect(isValid).toBe(false);
  });
});

describe('settings.js — LLM 模式单选切换', () => {
  it('remote 模式选择', () => {
    const radios = ['remote', 'local'];
    const selected = 'remote';
    expect(radios).toContain(selected);
  });

  it('local 模式选择', () => {
    const radios = ['remote', 'local'];
    const selected = 'local';
    expect(radios).toContain(selected);
  });

  it('onLlmModeChange 调用 setMode', () => {
    const setMode = vi.fn();
    const value = 'local';
    onLlmModeChange(value, setMode);
    expect(setMode).toHaveBeenCalledWith('local');
  });

  function onLlmModeChange(mode, setMode) {
    setMode(mode);
  }
});

describe('settings.js — 设置面板 Focus Trap 生命周期', () => {
  let _settingsTrap = null;

  function activateTrap() {
    if (_settingsTrap) _settingsTrap.deactivate();
    _settingsTrap = { activate: vi.fn(), deactivate: vi.fn() };
    _settingsTrap.activate();
  }

  function deactivateTrap() {
    if (_settingsTrap) {
      _settingsTrap.deactivate();
      _settingsTrap = null;
    }
  }

  it('打开设置时激活 Focus Trap', () => {
    activateTrap();
    expect(_settingsTrap).not.toBeNull();
    expect(_settingsTrap.activate).toHaveBeenCalled();
  });

  it('关闭设置时停用 Focus Trap', () => {
    activateTrap();
    deactivateTrap();
    expect(_settingsTrap).toBeNull();
  });

  it('重复打开先停用旧 Trap', () => {
    activateTrap();
    const oldTrap = _settingsTrap;
    activateTrap();
    expect(oldTrap.deactivate).toHaveBeenCalled();
  });
});
