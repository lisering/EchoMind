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
