/**
 * EchoMind 输入区快速 Toggle - DeepSeek 风格输入框旁快速开关。
 *
 * 职责：
 * 1. 创建可点击的 toggle 按钮（混合搜索 / Agent 模式）
 * 2. 点击切换 active 状态并调用后端 IPC 命令
 * 3. 暴露 getToggleState() 查询当前状态
 *
 * 设计参考：DeepSeek 输入框旁的「深度思考」「联网搜索」快速 toggle
 * - 默认不激活：灰色文字 + 透明背景
 * - 激活态：accent 色 + 微染背景
 * - 点击即时切换，无需打开设置面板
 */

import { t } from './i18n.js';
import { settingsApi } from './ipc.js';
import { get, setState, subscribe } from './state.js';

/** SVG 图标 */
const ICON_HYBRID = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>`;
const ICON_AGENT = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2a4 4 0 0 0-4 4v1a4 4 0 0 0 8 0V6a4 4 0 0 0-4-4z"/><path d="M5 12h14M12 8v8"/></svg>`;

/**
 * Toggle 配置表 - 每个 toggle 的设置键（S09 update_setting 白名单）+ 图标 + i18n key。
 */
const TOGGLE_CONFIG = {
  hybrid: {
    icon: ICON_HYBRID,
    labelKey: 'chat.toggle_hybrid',
    tooltipKey: 'chat.toggle_hybrid_tooltip',
    settingKey: 'rag.hybrid_search',
  },
  agent: {
    icon: ICON_AGENT,
    labelKey: 'chat.toggle_agent',
    tooltipKey: 'chat.toggle_agent_tooltip',
    settingKey: 'rag.agent_enabled',
  },
};

/**
 * 将 settingKey 转换为 state.js 中的字段名
 * @param {string} settingKey - 'hybrid' | 'agent'
 * @returns {string}
 */
function settingKeyToStateKey(settingKey) {
  switch (settingKey) {
    case 'hybrid': return 'hybridEnabled';
    case 'agent': return 'agentEnabled';
    default: return settingKey;
  }
}

/**
 * 创建输入框旁的快速 toggle 按钮。
 *
 * @param {string} settingKey - 'hybrid' | 'agent'
 * @param {boolean} [initialActive=false] - 初始激活状态
 * @returns {HTMLDivElement} toggle 按钮元素
 */
export function createInputToggle(settingKey, initialActive = false) {
  const config = TOGGLE_CONFIG[settingKey];
  if (!config) {
    console.warn(`[input-toggles] 未知 settingKey: ${settingKey}（S94 精简后仅支持 hybrid/agent）`);
    return document.createElement('div');
  }

  // 初始化时将状态同步到 state.js（无论 true/false 都要同步，确保覆盖上一轮残留状态）
  const stateKey = settingKeyToStateKey(settingKey);
  setState({ [stateKey]: initialActive });

  const toggle = document.createElement('div');
  toggle.className = 'input-toggle flex items-center gap-1.5 px-3 py-1.5 rounded-lg cursor-pointer text-sm transition-colors duration-150 select-none';
  toggle.dataset.settingKey = settingKey;
  toggle.setAttribute('role', 'switch');
  toggle.setAttribute('aria-checked', String(initialActive));
  toggle.tabIndex = 0;
  toggle.innerHTML = `${config.icon}<span>${t(config.labelKey)}</span>`;
  // P1-4: 添加 tooltip 提升发现性
  if (config.tooltipKey) {
    const tooltipText = t(config.tooltipKey);
    if (tooltipText && tooltipText !== config.tooltipKey) {
      toggle.title = tooltipText;
    }
  }

  /** 更新视觉状态 */
  function updateVisual() {
    const stateKey = settingKeyToStateKey(settingKey);
    const active = get(stateKey);
    toggle.classList.toggle('text-accent', active);
    toggle.classList.toggle('bg-accent/10', active);
    toggle.classList.toggle('text-text-tertiary', !active);
    toggle.classList.toggle('hover:text-text-secondary', !active);
    toggle.classList.toggle('hover:bg-surface-3', !active);
    toggle.setAttribute('aria-checked', String(active));
  }

  updateVisual();

  // 订阅状态变化以更新UI
  const unsubscribe = subscribe(stateKey, updateVisual);

  /** 切换状态 */
  async function doToggle() {
    const stateKey = settingKeyToStateKey(settingKey);
    const currentState = get(stateKey);
    const newState = !currentState;
    
    // 立即更新UI状态（乐观更新）
    setState({ [stateKey]: newState });
    updateVisual();
    
    try {
      await settingsApi.setBool(config.settingKey, newState);
    } catch (err) {
      // 回滚状态
      setState({ [stateKey]: currentState });
      updateVisual();
      console.error(`[input-toggles] ${settingKey} 切换失败:`, err);
    }
  }

  toggle.onclick = doToggle;
  toggle.onkeydown = (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      doToggle();
    }
  };

  // 清理订阅（避免内存泄漏）
  toggle.addEventListener('remove', () => {
    unsubscribe();
  });

  return toggle;
}

/**
 * 获取指定 toggle 的当前状态。
 * @param {string} settingKey - 'hybrid' | 'agent'
 * @returns {boolean}
 */
export function getToggleState(settingKey) {
  const stateKey = settingKeyToStateKey(settingKey);
  return get(stateKey);
}

/**
 * 设置 toggle 状态（外部同步用，不触发 IPC）。
 * @param {string} settingKey - 'hybrid' | 'agent'
 * @param {boolean} value
 */
export function setToggleState(settingKey, value) {
  const stateKey = settingKeyToStateKey(settingKey);
  setState({ [stateKey]: value });
}
