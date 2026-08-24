/**
 * EchoMind 状态容器 — Observable 模式，替代 15 个全局可变变量。
 *
 * 设计原则：
 * 1. 单一真相源 — 所有 UI 状态集中管理，不再散落在全局 let 变量中
 * 2. 不可变更新 — 通过 setState() 修改，始终返回新快照
 * 3. 订阅通知 — 组件可订阅状态变化，自动更新 UI（观察者模式）
 * 4. 可测试 — 导出 getState() / setState()，测试中可直接读写和断言
 *
 * 灵感来源：Redux（单一 store）+ MobX（细粒度订阅）+ Vue 3（响应式 ref）
 */

// ============================================================
// 类型定义（JSDoc）
// ============================================================

/**
 * @typedef {Object} EchoState
 * @property {Array} history               - 多轮上下文（REQ-RAG-004/006）
 * @property {string} currentRawMarkdown   - 流式累积原文（REQ-UI-003）
 * @property {Object|null} lastSources      - 最近一次引用来源
 * @property {boolean} streaming           - 是否正在流式生成
 * @property {HTMLElement|null} currentAssistantEl - 当前 assistant Block DOM
 * @property {boolean} renderScheduled     - rAF 渲染调度标志
 * @property {boolean} tempWebSearch       - /web 斜杠命令临时启用网页搜索标志（chat_done 后复位）
 * @property {boolean} tempAgent           - /agent 斜杠命令临时启用 Agent 模式标志（chat_done 后复位）
 * @property {boolean} regenForceSearch    - 强制检索重生成临时开混合搜索标志（chat_done 后复位）
 * @property {string|null} currentConversationId - 当前会话 ID
 * @property {boolean} isNewConversation   - 是否处于新建未持久化会话状态（懒创建标志）
 * @property {string} activePreset         - 当前选中的 Provider 预设
 * @property {boolean} isPro               - Pro 授权状态
 * @property {boolean} vlmEnabled          - VLM 图片理解增强开关
 * @property {boolean} rerankEnabled        - 重排序开关
 * @property {boolean} hydeEnabled          - HyDE 查询改写开关
 * @property {string|null} auditingDocId   - 审计中文档 ID（null=非审计模式）
 * @property {boolean} importing           - 导入进行中标志
 * @property {number} docCount             - 知识库文档总数
 * @property {number} chunkCount           - 知识库分块总数
 * @property {Array} documents             - 当前文档列表快照
 * @property {Array} kbAllDocs             - 全部文档缓存（用于建议生成）
 * @property {string} activeSidebarTab     - 当前侧栏 Tab（conversations/documents）
 * @property {number} cmdSelectedIndex    - 命令面板选中索引
 * @property {Array} cmdFiltered          - 当前过滤后的命令列表
 * @property {Element|null} cmdPrevFocus  - 命令面板打开前的焦点元素
 * @property {string} currentModel         - 当前 LLM 模型名
 * @property {string} currentLlmMode       - LLM 模式（remote/local）
 * @property {number} contextTokens       - 当前对话已用 token 数
 * @property {number} contextLimit         - 模型上下文窗口大小
 * @property {string} securityState        - 安全状态（unencrypted/encrypted_unlocked/locked）
 * @property {boolean} piiDetectionEnabled - PII 检测开关
 * @property {number} autoLockTimeout      - 自动锁屏超时（0=disabled）
 * @property {boolean} clipboardClearEnabled - 剪贴板自动清除开关
 * @property {number} clipboardClearTimeout  - 剪贴板清除超时（秒）
 * @property {string} theme                - 当前主题模式（dark/light/system，REQ-UI-011）
 * @property {boolean} llmConfigured       - LLM 是否已配置（API Key + Base URL + Model）
 * @property {boolean} demoMode            - REQ-RAG-051: 演示模式（无 Key 体验）
 * @property {Object<string, string>} drafts - 会话级草稿（conversationId → 未发送文本）
 * @property {boolean} hybridEnabled       - 混合搜索开关
 * @property {boolean} agentEnabled        - Agent 模式开关
 * @property {boolean} subAgentEnabled     - 子代理开关
 * @property {boolean} memoryEnabled       - 记忆开关
 * @property {boolean} webSearchEnabled    - 网页搜索开关
 * @property {Array} docList              - 文档提及列表
 */

// ============================================================
// 初始状态
// ============================================================

/** @type {EchoState} */
const initialState = {
  history: [],
  currentRawMarkdown: '',
  lastSources: null,
  streaming: false,
  currentAssistantEl: null,
  /** V3.1 P4-4：/web 斜杠命令的「单次会话临时启用」标志（chat_done 后复位） */
  tempWebSearch: false,
  /** V3.1 P4-4：/agent 斜杠命令的「单次会话临时启用」标志（chat_done 后复位） */
  tempAgent: false,
  /** V3.1 P4-4：强制检索重生成的「临时开混合搜索」标志（chat_done 后复位） */
  regenForceSearch: false,
  renderScheduled: false,
  currentConversationId: null,
  isNewConversation: false,
  activePreset: 'deepseek',
  isPro: false,
  vlmEnabled: false,
  rerankEnabled: false,
hydeEnabled: false,
  auditingDocId: null,
  importing: false,
docCount: 0,
chunkCount: 0,
documents: [],
kbAllDocs: [],
  activeSidebarTab: 'conversations',
  cmdSelectedIndex: 0,
  cmdFiltered: [],
  cmdPrevFocus: null,
  // 当前 LLM 模型信息（TC-QA-004：每条 assistant 消息标注模型）
  currentModel: '',              // 如 'claude-3.5-sonnet' 或 'qwen2.5-7b.gguf'
  currentLlmMode: 'remote',      // 'remote' 或 'local'
  // 上下文窗口用量（TC-QA-020~026：上下文窗口指示器）
  contextTokens: 0,              // 当前对话已用 token 数
  contextLimit: 8000,            // 模型上下文窗口大小（默认 8K）
  // 安全状态
  securityState: 'unencrypted',   // 'unencrypted' | 'encrypted_unlocked' | 'locked'
  piiDetectionEnabled: false,
  autoLockTimeout: 0,             // 0 = disabled
  clipboardClearEnabled: true,   // S2 复盘：剪贴板自动清除开关
  clipboardClearTimeout: 30,     // S2 复盘：剪贴板清除超时（秒）
  theme: 'dark',                  // REQ-UI-011：主题模式 dark/light/system
  llmConfigured: false,            // LLM 是否已配置
demoMode: false,                  // REQ-RAG-051: 演示模式（无 Key 体验）
  drafts: {},                      // 会话级草稿（conversationId → 未发送文本）
  
  // 功能开关状态（方案4：input-toggles.js 状态补全）
  hybridEnabled: false,            // 混合搜索开关
  agentEnabled: false,             // Agent 模式开关
  subAgentEnabled: false,          // 子代理开关
  memoryEnabled: false,            // 记忆开关
  webSearchEnabled: false,         // 网页搜索开关
  docList: [],                     // 文档提及列表（chat.js doc-mention 需要）
};

// ============================================================
// 状态存储（模块级单例）
// ============================================================

/** 当前状态快照（不可变引用，修改时创建新对象） */
let _state = { ...initialState };

/** 订阅者列表：key → Set<callback>（细粒度订阅） */
const _subscribers = new Map();

/** 全局订阅者（任意状态变化时通知） */
const _globalSubscribers = new Set();

// ============================================================
// 公共 API
// ============================================================

/**
 * 获取当前状态的只读快照。
 * 调用方不应直接修改返回值——使用 setState() 更新。
 * @returns {EchoState} 当前状态快照（浅拷贝）
 */
export function getState() {
  return { ..._state };
}

/**
 * 获取状态中某个字段的值（便捷访问器）。
 * @param {string} key - 状态字段名
 * @returns {*} 字段值
 */
export function get(key) {
  return _state[key];
}

/**
 * 更新状态（部分更新），并通知订阅者。
 *
 * @param {Partial<EchoState>} partial - 需要更新的字段
 * @returns {EchoState} 更新后的状态快照
 *
 * @example
 * setState({ streaming: true, currentRawMarkdown: '' });
 */
export function setState(partial) {
  const changed = [];
  for (const key of Object.keys(partial)) {
    if (_state[key] !== partial[key]) {
      changed.push(key);
    }
  }
  if (changed.length === 0) return _state;

  _state = { ..._state, ...partial };

  // 通知细粒度订阅者
  for (const key of changed) {
    const subs = _subscribers.get(key);
    if (subs) {
      for (const cb of subs) {
        try { cb(_state[key], key); } catch (_) { /* 订阅者异常不阻断 */ }
      }
    }
  }

  // 通知全局订阅者
  for (const cb of _globalSubscribers) {
    try { cb(_state, changed); } catch (_) { /* 同上 */ }
  }

  return _state;
}

/**
 * 订阅特定字段的变化（细粒度订阅）。
 * @param {string} key - 要订阅的状态字段名
 * @param {(value: *, key: string) => void} callback - 值变化时的回调
 * @returns {() => void} 取消订阅函数
 *
 * @example
 * const unsub = subscribe('streaming', (isStreaming) => {
 *   console.log('流式状态变化:', isStreaming);
 * });
 * // 不再需要时：unsub();
 */
export function subscribe(key, callback) {
  if (!_subscribers.has(key)) {
    _subscribers.set(key, new Set());
  }
  _subscribers.get(key).add(callback);
  return () => _subscribers.get(key)?.delete(callback);
}

/**
 * 订阅任意状态变化（全局订阅）。
 * @param {(state: EchoState, changed: string[]) => void} callback
 * @returns {() => void} 取消订阅函数
 */
export function subscribeAll(callback) {
  _globalSubscribers.add(callback);
  return () => _globalSubscribers.delete(callback);
}

/**
 * 重置状态到初始值（主要用于测试隔离）。
 * 清除所有订阅者。
 */
export function resetState() {
  _state = { ...initialState };
  _subscribers.clear();
  _globalSubscribers.clear();
}

// ============================================================
// 便捷访问器（语义化导出）
// ============================================================

/** 是否正在流式生成 */
export function isStreaming() {
  return _state.streaming;
}

/** 是否为 Pro 版 */
export function isProUser() {
  return _state.isPro;
}

/** 当前会话 ID */
export function currentConv() {
  return _state.currentConversationId;
}

/** 是否正在审计 */
export function isAuditing() {
  return _state.auditingDocId !== null;
}

/** 数据库是否已加密 */
export function isEncrypted() {
  return _state.securityState !== 'unencrypted';
}

/** 应用是否已锁定 */
export function isLocked() {
  return _state.securityState === 'locked';
}
