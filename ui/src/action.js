/**
 * Action 系统 - 类型安全键盘快捷键注册与调度（借鉴 Zed `gpui::Action`/`actions!` 宏）。
 *
 * 设计原则：
 * 1. 类型安全 - 每个 Action 有唯一 id，编译期（JSDoc）保证不重复
 * 2. 声明式注册 - 快捷键绑定通过 ActionRegistry 注册，而非 if-else 硬编码
 * 3. 统一调度 - 命令面板和键盘快捷键共用同一注册表
 * 4. 可扩展 - 新增快捷键只需注册一个 Action，无需修改调度逻辑
 *
 * 借鉴 Zed 的 Action 系统：
 * - Zed: `actions!(Scope, [Action1, Action2])` 宏 → 类型安全 Action struct
 * - EchoMind: `Action` 类 + `ActionRegistry` → 运行时注册 + 调度
 *
 * 用法：
 *   const registry = new ActionRegistry();
 *   registry.register({
 *     id: 'new-chat',
 *     keybinding: { mod: true, key: 'n' },
 *     description: '新建会话',
 *     handler: () => createNewConversation(),
 *   });
 *   registry.dispatchKeydown(event); // 自动匹配并执行
 */

// ============================================================
// 类型定义
// ============================================================

/**
 * @typedef {Object} KeyBinding
 * @property {boolean} [mod] - Cmd (macOS) / Ctrl (Windows/Linux)
 * @property {boolean} [shift]
 * @property {boolean} [alt]
 * @property {string} key - 按键名（小写，如 'k', 'n', ','）
 */

/**
 * @typedef {Object} Action
 * @property {string} id - 唯一标识（如 'new-chat'）
 * @property {KeyBinding} [keybinding] - 快捷键绑定
 * @property {string} description - 人类可读描述
 * @property {string} [icon] - 图标名称（供命令面板显示）
 * @property {() => void} handler - 执行函数
 * @property {() => boolean} [condition] - 是否可用（返回 false 时跳过）
 */

// ============================================================
// ActionRegistry - 注册表 + 调度
// ============================================================

/**
 * Action 注册表 - 统一管理所有 Action 和快捷键绑定。
 *
 * 借鉴 Zed 的 `Keymap` + `ActionRegistry` 模式：
 * - register() 注册 Action（含快捷键绑定）
 * - dispatchKeydown() 匹配按键事件并执行对应 handler
 * - listActions() 获取所有已注册 Action（供命令面板使用）
 */
export class ActionRegistry {
  constructor() {
    /** @type {Map<string, Action>} */
    this._actions = new Map();
    /** @type {Map<string, string>} - keybinding signature → action id */
    this._keymap = new Map();
  }

  /**
   * 注册一个 Action。
   * @param {Action} action
   * @throws {Error} id 重复时抛出
   */
  register(action) {
    if (this._actions.has(action.id)) {
      throw new Error(`Action id 已存在: ${action.id}`);
    }
    this._actions.set(action.id, action);
    if (action.keybinding) {
      const sig = this._keybindingSignature(action.keybinding);
      this._keymap.set(sig, action.id);
    }
  }

  /**
   * 注销一个 Action。
   * @param {string} id
   */
  unregister(id) {
    const action = this._actions.get(id);
    if (action?.keybinding) {
      const sig = this._keybindingSignature(action.keybinding);
      this._keymap.delete(sig);
    }
    this._actions.delete(id);
  }

  /**
   * 根据 ID 获取 Action。
   * @param {string} id
   * @returns {Action | undefined}
   */
  get(id) {
    return this._actions.get(id);
  }

  /**
   * 列出所有已注册 Action（供命令面板使用）。
   * @returns {Action[]}
   */
  listActions() {
    return Array.from(this._actions.values());
  }

  /**
   * 调度键盘事件 - 匹配快捷键并执行对应 handler。
   *
   * @param {KeyboardEvent} event
   * @param {Object} [context] - 上下文信息
   * @param {boolean} [context.appVisible] - 应用是否可见
   * @param {boolean} [context.inputFocused] - 是否有输入框聚焦
   * @returns {boolean} - 是否匹配并执行了某个 Action
   */
  dispatchKeydown(event, context = {}) {
    const sig = this._eventSignature(event);
    const actionId = this._keymap.get(sig);
    if (!actionId) return false;

    const action = this._actions.get(actionId);
    if (!action) return false;

    // 检查条件
    if (action.condition && !action.condition()) return false;

    // 执行 handler
    event.preventDefault();
    action.handler();
    return true;
  }

  /**
   * 执行指定 ID 的 Action。
   * @param {string} id
   * @returns {boolean} - 是否找到并执行了
   */
  execute(id) {
    const action = this._actions.get(id);
    if (!action) return false;
    if (action.condition && !action.condition()) return false;
    action.handler();
    return true;
  }

  /**
   * 生成快捷键签名字符串（用于 keymap 查找）。
   * @param {KeyBinding} kb
   * @returns {string}
   * @private
   */
  _keybindingSignature(kb) {
    const parts = [];
    if (kb.mod) parts.push('mod');
    if (kb.shift) parts.push('shift');
    if (kb.alt) parts.push('alt');
    parts.push(kb.key.toLowerCase());
    return parts.join('+');
  }

  /**
   * 从 KeyboardEvent 生成签名。
   * @param {KeyboardEvent} event
   * @returns {string}
   * @private
   */
  _eventSignature(event) {
    const parts = [];
    if (event.metaKey || event.ctrlKey) parts.push('mod');
    if (event.shiftKey) parts.push('shift');
    if (event.altKey) parts.push('alt');
    parts.push(event.key.toLowerCase());
    return parts.join('+');
  }
}

// ============================================================
// 默认 Action 定义 - 与现有快捷键保持一致
// ============================================================

/**
 * 创建默认 Action 注册表（与现有 keyboard.js 快捷键一致）。
 *
 * @param {Object} handlers - 各 action 的 handler 函数
 * @param {() => void} handlers.onNewChat
 * @param {() => void} handlers.onImport
 * @param {() => void} handlers.onSettings
 * @param {() => void} [handlers.onToggleSidebar] - 折叠/展开侧栏（⌘B）
 * @param {() => void} [handlers.onAbort]
 * @param {() => void} [handlers.onCommandPalette]
 * @param {() => void} [handlers.onKeyboardHelp] - 显示快捷键帮助（⌘/）
 * @param {() => void} [handlers.onGlobalSearch] - 全局搜索（⌘⇧F）
 * @param {() => void} [handlers.onExport] - 导出当前对话（⌘E）
 * @param {() => boolean} [handlers.isInputFocused] - 判断是否有输入框聚焦
 * @param {() => boolean} [handlers.isAppVisible] - 判断应用是否可见
 * @returns {ActionRegistry}
 */
export function createDefaultRegistry(handlers) {
  const registry = new ActionRegistry();

  const isInputFocused = handlers.isInputFocused || (() => false);
  const isAppVisible = handlers.isAppVisible || (() => true);

  // ⌘K 打开/关闭命令面板
  registry.register({
    id: 'command-palette',
    keybinding: { mod: true, key: 'k' },
    description: '打开命令面板',
    handler: handlers.onCommandPalette || (() => {}),
    condition: () => isAppVisible(),
  });

  // ⌘N 新建会话
  registry.register({
    id: 'new-chat',
    keybinding: { mod: true, key: 'n' },
    description: '新建会话',
    handler: handlers.onNewChat,
    condition: () => isAppVisible() && !isInputFocused(),
  });

  // ⌘O 导入文件
  registry.register({
    id: 'import-files',
    keybinding: { mod: true, key: 'o' },
    description: '导入文件',
    handler: handlers.onImport,
    condition: () => isAppVisible() && !isInputFocused(),
  });

  // ⌘, 打开设置
  registry.register({
    id: 'open-settings',
    keybinding: { mod: true, key: ',' },
    description: '打开设置',
    handler: handlers.onSettings,
    condition: () => isAppVisible() && !isInputFocused(),
  });

  // ⌘B 折叠/展开侧栏
  registry.register({
    id: 'toggle-sidebar',
    keybinding: { mod: true, key: 'b' },
    description: '折叠/展开侧栏',
    handler: handlers.onToggleSidebar || (() => {}),
    condition: () => isAppVisible(),
  });

  // ⌘/ 打开快捷键帮助面板（REQ-KB-005）
  registry.register({
    id: 'keyboard-help',
    keybinding: { mod: true, key: '/' },
    description: '显示快捷键帮助',
    handler: handlers.onKeyboardHelp || (() => {}),
    condition: () => isAppVisible(),
  });

  // ⌘⇧F 全局搜索（REQ-NAV-002）
  registry.register({
    id: 'global-search',
    keybinding: { mod: true, shift: true, key: 'f' },
    description: '全局搜索',
    handler: handlers.onGlobalSearch || (() => {}),
    condition: () => isAppVisible(),
  });

  // ⌘E 导出当前对话（P1-6: N1 导出发现性）
  registry.register({
    id: 'export-conversation',
    keybinding: { mod: true, key: 'e' },
    description: '导出当前对话',
    handler: handlers.onExport || (() => {}),
    condition: () => isAppVisible() && !isInputFocused(),
  });

  return registry;
}

// ============================================================
// 单例（供全局使用）
// ============================================================

/** 全局 Action 注册表单例 */
let _globalRegistry = null;

/**
 * 获取全局 Action 注册表（懒初始化）。
 * @returns {ActionRegistry}
 */
export function getGlobalRegistry() {
  if (!_globalRegistry) {
    _globalRegistry = new ActionRegistry();
  }
  return _globalRegistry;
}

/**
 * 初始化全局 Action 注册表（替换单例）。
 * @param {ActionRegistry} registry
 */
export function setGlobalRegistry(registry) {
  _globalRegistry = registry;
}

// ============================================================
// 防御性守卫（guards.js 已合并到此）
// ============================================================

/**
 * 前端防御性编程守卫（Defensive Guards）+ 统一 UI 状态管理。
 *
 * 设计原则：
 * 1. 每个依赖后端前置条件的用户操作都应有前端层 UX 防御
 * 2. 防御分两层：UI 禁用（prevent input）+ 函数拦截（early return）
 * 3. 防御守卫集中管理，避免散落在各模块中
 * 4. updateInputUI() 是唯一的输入区 UI 状态更新入口
 */

import { get as _getState, isLocked } from './state.js';
import { $ } from './utils.js';
import { t as _t } from './i18n.js';
import { toastError as _toastError } from './toast.js';
import { getQueueSize } from './chat-utils.js';

/**
 * @typedef {Object} GuardResult
 * @property {boolean} passed - 是否通过守卫检查
 * @property {string} [reason] - 未通过时的原因（i18n key）
 */

/**
 * 检查知识库是否有文档。
 * @returns {GuardResult}
 */
export function requireDocuments() {
  if (_getState('docCount') === 0) {
    return { passed: false, reason: 'chat.empty_kb_error' };
  }
  return { passed: true };
}

/**
 * 检查 LLM 是否已配置。
 * @returns {GuardResult}
 */
export function requireLlmConfig() {
  if (!_getState('llmConfigured')) {
    return { passed: false, reason: 'chat.llm_not_configured' };
  }
  return { passed: true };
}

/**
 * 检查数据库是否已解锁。
 * @returns {GuardResult}
 */
export function requireUnlocked() {
  if (isLocked()) {
    return { passed: false, reason: 'chat.security_locked' };
  }
  return { passed: true };
}

/**
 * 检查当前是否可以发送消息（非流式 + 有文档 + LLM 已配置 + 未锁定）。
 * @returns {GuardResult}
 */
export function canSend() {
  const idle = requireIdle();
  if (!idle.passed) return idle;
  const docs = requireDocuments();
  if (!docs.passed) return docs;
  const llm = requireLlmConfig();
  if (!llm.passed) return llm;
  const unlocked = requireUnlocked();
  if (!unlocked.passed) return unlocked;
  return { passed: true };
}

/**
 * 检查是否为 Pro 用户。
 * @returns {GuardResult}
 */
export function requirePro() {
  if (!_getState('isPro')) {
    return { passed: false, reason: 'paywall.reason_default' };
  }
  return { passed: true };
}

/**
 * 检查当前是否可以进行新操作（非流式、非审计中、未锁定）。
 * @returns {GuardResult}
 */
export function requireIdle() {
  if (isLocked()) {
    return { passed: false, reason: 'chat.security_locked' };
  }
  if (_getState('streaming')) {
    return { passed: false, reason: 'chat.streaming_hint' };
  }
  if (_getState('auditingDocId')) {
    return { passed: false, reason: 'chat.thinking_auditing' };
  }
  return { passed: true };
}

/**
 * 执行守卫检查：通过返回 true，未通过显示 toast 并返回 false。
 * @param {GuardResult} result - 守卫检查结果
 * @returns {boolean} 是否通过
 */
export function runGuard(result) {
  if (!result.passed) {
    if (result.reason) {
      _toastError(_t(result.reason));
    }
    return false;
  }
  return true;
}

/**
 * 根据全局 state 计算并更新聊天输入框和发送按钮的 UI 状态。
 *
 * 状态矩阵（7 种）：
 * | 状态 | 触发条件 | queryInput | sendBtn | placeholder |
 * |---|---|---|---|---|
 * | locked       | 数据库锁定 | disabled | disabled | "数据库已锁定" |
 * | empty-kb     | docCount===0 && !streaming | disabled | disabled | "请先导入文档" |
 * | streaming    | streaming && queue 空 | enabled | stop-mode | "输入下一条问题排队发送…" |
 * | queued       | streaming && queue 非空 | enabled | stop-mode+badge | 同上 |
 * | ready-empty  | !streaming && input 空 | enabled | enabled+视觉降级 | 正常 placeholder |
 * | ready-active | !streaming && input 非空 | enabled | enabled+accent | 正常 placeholder |
 */
export function updateInputUI() {
  const docCount = _getState('docCount');
  const streaming = _getState('streaming');
  const queueSize = getQueueSize();
  const locked = isLocked();
  const input = $('queryInput');
  const sendBtn = $('sendBtn');
  if (!input || !sendBtn) return;

  if (locked) {
    input.disabled = true;
    sendBtn.disabled = true;
    input.setAttribute('placeholder', _t('chat.locked_placeholder') || '数据库已锁定，请先解锁');
    sendBtn.classList.add('opacity-30', 'cursor-not-allowed');
    return;
  }

  if (docCount === 0 && !streaming) {
    input.disabled = true;
    sendBtn.disabled = true;
    input.setAttribute('placeholder', _t('chat.empty_kb_placeholder'));
    sendBtn.classList.add('opacity-30', 'cursor-not-allowed');
    return;
  }

  if (streaming) {
    input.disabled = false;
    sendBtn.disabled = false;
    sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');
    input.setAttribute('placeholder', _t('chat.queue_placeholder') || '输入下一条问题排队发送…');
    return;
  }

  input.disabled = false;
  sendBtn.disabled = false;
  input.setAttribute('placeholder', _t('chat.input_placeholder'));
  sendBtn.classList.remove('opacity-30', 'cursor-not-allowed');

  const inputEmpty = !input.value.trim();
  if (inputEmpty) {
    sendBtn.classList.add('opacity-40', 'cursor-default');
    sendBtn.classList.remove('hover:opacity-90');
  } else {
    sendBtn.classList.remove('opacity-40', 'cursor-default');
    sendBtn.classList.add('hover:opacity-90');
  }
}

/**
 * 向后兼容：syncChatInputState 委托给 updateInputUI。
 * @deprecated 使用 updateInputUI() 替代。
 */
export function syncChatInputState() {
  updateInputUI();
}
