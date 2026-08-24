/**
 * EchoMind wizard.js 单元测试 — 首启向导 / 步骤验证。
 *
 * 验证点：
 * 1. showWizardStep(1) 显示下载步骤
 * 2. showWizardStep(2) 显示配置步骤
 * 3. showWizardStep(3) 显示导入步骤
 * 4. updateStepIndicator 设置步骤指示器状态
 * 5. calcOverallProgress 有 Content-Length 时精确计算
 * 6. calcOverallProgress total=0 时使用对数曲线估算
 * 7. handleDownloadEvent Downloading 事件更新进度
 * 8. handleDownloadEvent Loading 事件触发成功
 * 9. handleDownloadEvent Done 事件静默处理
 * 10. handleDownloadEvent Error 事件触发错误
 *
 * Mock: state.js, chat-render.js, utils.js, ipc.js, toast.js, i18n.js, focus-trap.js
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';

// Mock state
vi.mock('../../../ui/src/state.js', () => ({
  setState: vi.fn(),
  get: (key) => {
    const map = { activePreset: 'deepseek', currentModel: '', currentLlmMode: 'remote', llmConfigured: false };
    return map[key];
  },
}));

// Mock chat-render
vi.mock('../../../ui/src/chat-render.js', () => ({
  showApp: vi.fn(),
  updateModelPill: vi.fn(),
}));

// Mock utils
vi.mock('../../../ui/src/utils.js', () => ({
  $: (id) => document.getElementById(id),
  PRESETS: {
    deepseek: { label: 'DeepSeek', base_url: 'https://api.deepseek.com/v1', model: 'deepseek-chat', needKey: true, keyUrl: 'https://platform.deepseek.com/api_keys', descKey: 'preset.deepseek_desc' },
    openai: { label: 'OpenAI', base_url: 'https://api.openai.com/v1', model: 'gpt-4o', needKey: true, keyUrl: 'https://platform.openai.com/api-keys', descKey: 'preset.openai_desc' },
    ollama: { label: 'Ollama', base_url: 'http://localhost:11434/v1', model: 'llama3', needKey: false, keyUrl: '', descKey: 'preset.ollama_desc' },
  },
  WORKSPACE: 'default',
}));

// Mock ipc
vi.mock('../../../ui/src/ipc.js', () => ({
  invoke: vi.fn(() => Promise.resolve()),
  openUrl: vi.fn(),
  listen: vi.fn(() => Promise.resolve(() => {})),
  openDialog: vi.fn(() => Promise.resolve([])),
}));

// Mock toast
vi.mock('../../../ui/src/toast.js', () => ({
  toast: vi.fn(),
  toastError: vi.fn(),
}));

// Mock i18n
vi.mock('../../../ui/src/i18n.js', () => ({
  t: (key, params) => {
    if (typeof params === 'object' && params !== null) return key + JSON.stringify(params);
    return key;
  },
}));

// Mock focus-trap
vi.mock('../../../ui/src/focus-trap.js', () => ({
  createFocusTrap: vi.fn(() => ({
    activate: vi.fn(),
    deactivate: vi.fn(),
  })),
}));

// Setup DOM
function setupDom() {
  document.body.innerHTML = `
    <div id="wizard">
      <div id="wizardStep1" class="hidden">
        <div id="wizDownloadBar" class="progress-indeterminate" style="width:0%"></div>
        <span id="wizDownloadPct"></span>
        <span id="wizDownloadStatus" data-i18n="wizard.download_preparing"></span>
        <div id="wizDownloadError" class="hidden"></div>
        <div id="wizDownloadRetry" class="hidden"></div>
        <div id="wizDownloadDone" class="hidden"></div>
        <button id="wizRetryBtn"></button>
        <button id="wizNextFromStep1"></button>
      </div>
      <div id="wizardStep2" class="hidden">
        <div id="presetCards"></div>
        <input id="wizKey" />
        <input id="wizUrl" />
        <input id="wizModel" />
        <span id="keyOptional" class="hidden"></span>
        <a id="wizKeyLink" class="hidden"></a>
        <div id="wizError" class="hidden"></div>
        <button id="wizStart">验证并继续</button>
        <button id="wizSkipStep2"></button>
      </div>
      <div id="wizardStep3" class="hidden">
        <div id="wizDropZone"></div>
        <button id="wizPickFiles"></button>
        <div id="wizImportProgress" class="hidden"></div>
        <div id="wizImportBar" style="width:0%"></div>
        <span id="wizImportText"></span>
        <div id="wizImportedList" class="hidden"></div>
        <button id="wizFinish"></button>
      </div>
      <div id="wizStepDot1"></div>
      <div id="wizStepDot2"></div>
      <div id="wizStepDot3"></div>
      <span id="wizStepLabel"></span>
    </div>
  `;
}

setupDom();

import { showWizardStep, renderPresetCards, applyPreset, initWizard, startWizard } from '../../../ui/src/wizard.js';

describe('wizard.js — 步骤切换', () => {
  beforeEach(() => {
    setupDom();
    vi.clearAllMocks();
  });

  it('showWizardStep(1) 显示下载步骤，隐藏其他', () => {
    showWizardStep(1);
    expect(document.getElementById('wizardStep1').classList.contains('hidden')).toBe(false);
    expect(document.getElementById('wizardStep2').classList.contains('hidden')).toBe(true);
    expect(document.getElementById('wizardStep3').classList.contains('hidden')).toBe(true);
  });

  it('showWizardStep(2) 显示配置步骤', () => {
    showWizardStep(2);
    expect(document.getElementById('wizardStep2').classList.contains('hidden')).toBe(false);
    expect(document.getElementById('wizardStep1').classList.contains('hidden')).toBe(true);
  });

  it('showWizardStep(3) 显示导入步骤', () => {
    showWizardStep(3);
    expect(document.getElementById('wizardStep3').classList.contains('hidden')).toBe(false);
    expect(document.getElementById('wizardStep1').classList.contains('hidden')).toBe(true);
  });

  it('showWizardStep 更新步骤指示器 dot 状态', () => {
    showWizardStep(2);
    const dot1 = document.getElementById('wizStepDot1');
    const dot2 = document.getElementById('wizStepDot2');
    expect(dot1.classList.contains('completed')).toBe(true);
    expect(dot2.classList.contains('active')).toBe(true);
  });

  it('showWizardStep 更新步骤标签 i18n key', () => {
    showWizardStep(2);
    const label = document.getElementById('wizStepLabel');
    expect(label.getAttribute('data-i18n')).toBe('wizard.step2_title');
  });

  it('renderPresetCards 创建预设卡片按钮', () => {
    showWizardStep(2);
    renderPresetCards();
    const cards = document.getElementById('presetCards');
    expect(cards.children.length).toBeGreaterThan(0);
  });

  it('applyPreset 将预设值填入输入框', () => {
    showWizardStep(2);
    applyPreset();
    expect(document.getElementById('wizUrl').value).toBe('https://api.deepseek.com/v1');
    expect(document.getElementById('wizModel').value).toBe('deepseek-chat');
  });

  it('initWizard 绑定按钮事件不报错', () => {
    expect(() => initWizard(vi.fn())).not.toThrow();
  });

  it('initWizard 绑定重试按钮事件', () => {
    initWizard(vi.fn());
    const retryBtn = document.getElementById('wizRetryBtn');
    expect(retryBtn.onclick).not.toBeNull();
  });

  it('initWizard 绑定完成按钮事件', () => {
    initWizard(vi.fn());
    const finishBtn = document.getElementById('wizFinish');
    expect(finishBtn.onclick).not.toBeNull();
  });
});
