/**
 * EchoMind 轮次版本树模块 — 从 DB 消息列表重建编辑版本树。
 *
 * 职责：
 * 1. 从 list_messages 返回的平坦 ChatMessage[] 重建 turn_group → versions 树
 * 2. 提供「获取 turn_group 的活跃版本」/「获取所有版本」查询接口
 * 3. 生成新 turn_group ID（UUID）
 * 4. 对话分支树面板（REQ-RAG-039）：树形可视化 + 分支切换
 *
 * 设计参考：rs-pro chatTurns 树结构 + DB 持久化
 * AC-QA-006：用户消息编辑后创建新版本，旧版本保留在 DB 中
 */

import { convApi } from './ipc.js';
import { get } from './state.js';
import { t } from './i18n.js';
import { pushPanel, removePanel } from './panel-stack.js';

// ============================================================
// 类型定义（JSDoc）
// ============================================================

/**
 * @typedef {Object} TurnVersion
 * @property {string} userContent - 用户消息内容
 * @property {string|null} userMessageId - 用户消息 DB ID（用于思考面板状态持久化）
 * @property {string|null} assistantContent - AI 回答内容（流完成前为 null）
 * @property {Array|null} sources - 引用来源
 * @property {string|null} reasoning - 推理思考过程
 * @property {number} version - 版本号（从 1 递增）
 */

/**
 * @typedef {Object} ChatTurn
 * @property {string} turnGroup - 轮次分组 ID
 * @property {TurnVersion[]} versions - 所有版本（按 version 升序）
 * @property {number} activeVersion - 当前活跃版本号（默认最新）
 */

// ============================================================
// 状态管理
// ============================================================

/**
 * 全局轮次树：conversationId → ChatTurn[]
 * 每次切换会话时重建。
 */
let _turnTree = [];

// ============================================================
// 树构建
// ============================================================

/**
 * 从 DB 返回的平坦消息列表重建轮次版本树。
 *
 * 消息排列规则（DB rowid ASC）：
 * - 无 turn_group 的消息：视为单版本轮次（turn_group = 消息 id 的哈希）
 * - 有 turn_group 的消息：按 turn_group 分组，同组内按 version 排序
 * - 每个 turn_group 内 user + assistant 交替出现
 *
 * @param {Array<{id?: string, role: string, content: string, sources?: Array, reasoning?: string, turn_group?: string, version?: number}>} messages
 * @returns {ChatTurn[]} 重建的轮次版本树
 */
export function buildTurnTree(messages) {
  if (!messages || messages.length === 0) return [];

  // 无 turn_group 的消息按 user→assistant 两两配对（user + assistant = 1 turn），
  // 有 turn_group 的消息按组收集并按 version 建版本
  const turns = [];
  const groupedTurns = new Map();
  let pendingUngroupedUser = null;

  for (const msg of messages) {
    if (msg.turn_group) {
      // 有 turn_group 的消息：按组收集
      if (!groupedTurns.has(msg.turn_group)) {
        groupedTurns.set(msg.turn_group, {
          turnGroup: msg.turn_group,
          versions: [],
        });
      }
      const turn = groupedTurns.get(msg.turn_group);
      const ver = msg.version || 1;

      // 查找或创建版本
      let versionEntry = turn.versions.find((v) => v.version === ver);
      if (!versionEntry) {
        versionEntry = {
          userContent: '',
          userMessageId: null,
          assistantContent: null,
          sources: null,
          reasoning: null,
          version: ver,
        };
        turn.versions.push(versionEntry);
      }

      if (msg.role === 'user') {
        versionEntry.userContent = msg.content;
        versionEntry.userMessageId = msg.id || null;
      } else if (msg.role === 'assistant') {
        versionEntry.assistantContent = msg.content;
        versionEntry.sources = msg.sources || null;
        versionEntry.reasoning = msg.reasoning || null;
      }
    } else {
      // 无 turn_group 的消息：按 user→assistant 配对
      if (msg.role === 'user') {
        if (pendingUngroupedUser) {
          // 前一个 user 没有 assistant 回答，单独成 turn
          turns.push({
            turnGroup: `ungrouped-${turns.length}`,
            versions: [{
              userContent: pendingUngroupedUser.content,
              assistantContent: null,
              sources: null,
              reasoning: null,
              version: 1,
            }],
            activeVersion: 1,
          });
        }
        pendingUngroupedUser = msg;
      } else if (msg.role === 'assistant') {
        const tg = `ungrouped-${turns.length}`;
        turns.push({
          turnGroup: tg,
          versions: [{
            userContent: pendingUngroupedUser ? pendingUngroupedUser.content : '',
            assistantContent: msg.content,
            sources: msg.sources || null,
            reasoning: msg.reasoning || null,
            version: 1,
          }],
          activeVersion: 1,
        });
        pendingUngroupedUser = null;
      }
    }
  }

  // 处理末尾未配对的 user
  if (pendingUngroupedUser) {
    turns.push({
      turnGroup: `ungrouped-${turns.length}`,
      versions: [{
        userContent: pendingUngroupedUser.content,
        assistantContent: null,
        sources: null,
        reasoning: null,
        version: 1,
      }],
      activeVersion: 1,
    });
  }

  // 将分组的 turns 加入（按 turn_group 在原始消息中首次出现的顺序）
  for (const [, turn] of groupedTurns) {
    turn.versions.sort((a, b) => a.version - b.version);
    turn.activeVersion = turn.versions[turn.versions.length - 1].version;
    turns.push(turn);
  }

  // 按 turnGroup 中首个消息在原始 messages 中的位置排序
  const firstIdx = new Map();
  for (let i = 0; i < messages.length; i++) {
    const msg = messages[i];
    const tg = msg.turn_group || `ungrouped-${i}`;
    if (!firstIdx.has(tg)) firstIdx.set(tg, i);
  }
  // 为 ungrouped turns 赋正确的 firstIdx
  // ungrouped turns 的 turnGroup 是 `ungrouped-N`，但 firstIdx 用的 key 是 `ungrouped-i`
  // 这里直接按原始顺序排：先按 firstIdx 排序
  turns.sort((a, b) => {
    const aIdx = getFirstIdx(firstIdx, a.turnGroup, messages);
    const bIdx = getFirstIdx(firstIdx, b.turnGroup, messages);
    return aIdx - bIdx;
  });

  _turnTree = turns;
  return turns;
}

/**
 * 获取 turnGroup 在 firstIdx 中的索引（处理 ungrouped 的特殊映射）。
 */
function getFirstIdx(firstIdx, turnGroup, messages) {
  if (firstIdx.has(turnGroup)) return firstIdx.get(turnGroup);
  // ungrouped turns：turnGroup 格式 `ungrouped-N`，N 是在 turns 中的序号
  // 需要找到对应的原始消息位置
  for (let i = 0; i < messages.length; i++) {
    if (!messages[i].turn_group) {
      const tg = `ungrouped-${i}`;
      if (firstIdx.has(tg)) return firstIdx.get(tg);
    }
  }
  return 0;
}

// ============================================================
// 查询接口
// ============================================================

/**
 * 获取全局轮次树。
 * @returns {ChatTurn[]}
 */
export function getTurnTree() {
  return _turnTree;
}

/**
 * 设置全局轮次树（切换会话时调用）。
 * @param {ChatTurn[]} turns
 */
export function setTurnTree(turns) {
  _turnTree = turns || [];
}

/**
 * 根据 turnGroup 获取对应的 ChatTurn。
 * @param {string} turnGroup
 * @returns {ChatTurn|null}
 */
export function getTurn(turnGroup) {
  return _turnTree.find((t) => t.turnGroup === turnGroup) || null;
}

/**
 * 获取 turnGroup 的活跃版本。
 * @param {string} turnGroup
 * @returns {TurnVersion|null}
 */
export function getActiveVersion(turnGroup) {
  const turn = getTurn(turnGroup);
  if (!turn) return null;
  return turn.versions.find((v) => v.version === turn.activeVersion) || null;
}

/**
 * 获取 turnGroup 的版本总数。
 * @param {string} turnGroup
 * @returns {number}
 */
export function getVersionCount(turnGroup) {
  const turn = getTurn(turnGroup);
  return turn ? turn.versions.length : 0;
}

/**
 * 设置 turnGroup 的活跃版本号。
 * @param {string} turnGroup
 * @param {number} version
 * @returns {boolean} 是否设置成功
 */
export function setActiveVersion(turnGroup, version) {
  const turn = getTurn(turnGroup);
  if (!turn) return false;
  const exists = turn.versions.some((v) => v.version === version);
  if (!exists) return false;
  turn.activeVersion = version;
  return true;
}

/**
 * 向轮次树中添加新版本（编辑发送后调用）。
 * @param {string} turnGroup
 * @param {number} version
 * @param {string} userContent
 * @returns {void}
 */
export function addVersion(turnGroup, version, userContent) {
  let turn = getTurn(turnGroup);
  if (!turn) {
    turn = { turnGroup, versions: [], activeVersion: version };
    _turnTree.push(turn);
  }
  turn.versions.push({
    userContent,
    userMessageId: null,
    assistantContent: null,
    sources: null,
    reasoning: null,
    version,
  });
  turn.versions.sort((a, b) => a.version - b.version);
  turn.activeVersion = version;
}

/**
 * 更新版本的助手回答内容（流完成后调用）。
 * @param {string} turnGroup
 * @param {number} version
 * @param {string} assistantContent
 * @param {Array|null} sources
 * @param {string|null} reasoning
 * @returns {void}
 */
export function updateVersionAssistant(turnGroup, version, assistantContent, sources, reasoning) {
  const turn = getTurn(turnGroup);
  if (!turn) return;
  const ver = turn.versions.find((v) => v.version === version);
  if (!ver) return;
  ver.assistantContent = assistantContent;
  ver.sources = sources;
  ver.reasoning = reasoning;
}

/**
 * 从 DB 活跃版本列表应用到轮次树（加载会话时调用）。
 * @param {Array<{turn_group: string, active_version: number}>} activeVersions
 * @returns {void}
 */
export function applyActiveVersions(activeVersions) {
  if (!activeVersions || activeVersions.length === 0) return;
  for (const { turn_group, active_version } of activeVersions) {
    setActiveVersion(turn_group, active_version);
  }
}

/**
 * 获取所有 turnGroup 的活跃版本映射（用于持久化）。
 * @returns {Array<{turn_group: string, active_version: number}>}
 */
export function getActiveVersionMap() {
  return _turnTree.map((t) => ({
    turn_group: t.turnGroup,
    active_version: t.activeVersion,
  }));
}

/**
 * 生成新的 turn_group ID。
 * @returns {string} UUID 格式的 turn_group ID
 */
export function generateTurnGroupId() {
  if (typeof crypto !== 'undefined' && crypto.randomUUID) {
    return `turn-${crypto.randomUUID()}`;
  }
  // Fallback
  return `turn-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

/**
 * 从轮次树构建平坦的历史消息数组（仅活跃版本），供 LLM 多轮上下文使用。
 * @returns {Array<{role: string, content: string}>}
 */
export function buildHistoryFromTurns() {
  const history = [];
  for (const turn of _turnTree) {
    const ver = turn.versions.find((v) => v.version === turn.activeVersion);
    if (!ver) continue;
    history.push({ role: 'user', content: ver.userContent });
    if (ver.assistantContent) {
      history.push({ role: 'assistant', content: ver.assistantContent });
    }
  }
  return history;
}

// ============================================================
// 对话分支树视图（REQ-RAG-039）
// ============================================================

/** 树视图面板元素 */
let _treePanel = null;

/**
 * 打开对话分支树面板（REQ-RAG-039）。
 *
 * 调用 get_conversation_tree IPC 获取整棵分支树，
 * 渲染为垂直树形列表。活跃路径高亮显示。
 *
 * @returns {Promise<void>}
 */
export async function openConversationTreePanel() {
  const conversationId = get('currentConversationId');
  if (!conversationId) return;

  // 移除已存在的面板
  closeConversationTreePanel();

  // 创建面板容器
  _treePanel = document.createElement('div');
  _treePanel.id = 'conversationTreePanel';
  _treePanel.className = 'conversation-tree-panel';
  _treePanel.setAttribute('role', 'dialog');
  _treePanel.setAttribute('aria-modal', 'true');

  // 面板内容
  _treePanel.innerHTML = `
    <div class="conversation-tree-content">
      <div class="conversation-tree-header">
        <h3 class="conversation-tree-title"></h3>
        <button class="conversation-tree-close" aria-label="${t('common.close') || 'close'}"><svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg></button>
      </div>
      <div class="conversation-tree-body">
        <div class="conversation-tree-loading"></div>
      </div>
    </div>
  `;

  document.body.appendChild(_treePanel);

  // 绑定关闭事件
  const closeBtn = _treePanel.querySelector('.conversation-tree-close');
  if (closeBtn) closeBtn.onclick = () => closeConversationTreePanel();

  // 点击遮罩关闭
  _treePanel.addEventListener('click', (e) => {
    if (e.target === _treePanel) closeConversationTreePanel();
  });

  // ESC 关闭（面板自身处理）
  _treePanel.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeConversationTreePanel();
    }
  });

  // 注册到面板栈（ESC 关闭 + 生命周期追踪）
  pushPanel({ id: 'turn-tree', close: closeConversationTreePanel, element: _treePanel, label: 'Conversation Tree' });

  // 加载树数据
  const titleEl = _treePanel.querySelector('.conversation-tree-title');
  const bodyEl = _treePanel.querySelector('.conversation-tree-body');
  const loadingEl = _treePanel.querySelector('.conversation-tree-loading');

  if (titleEl) titleEl.textContent = t('tree.title') || 'Conversation Branch Tree';

  try {
    const tree = await convApi.getConversationTree(conversationId);

    if (!tree || !tree.nodes || tree.nodes.length === 0) {
      if (bodyEl) {
        bodyEl.innerHTML = `<div class="conversation-tree-empty">${
          t('tree.empty') || 'No branches yet. Edit a message to create a branch.'
        }</div>`;
      }
      return;
    }

    // 渲染树
    if (bodyEl) {
      bodyEl.innerHTML = '';
      const treeContainer = document.createElement('div');
      treeContainer.className = 'conversation-tree-container';
      bodyEl.appendChild(treeContainer);

      // 构建 node_id → node 映射
      const nodeMap = new Map();
      for (const node of tree.nodes) {
        nodeMap.set(node.node_id, node);
      }

      // 递归渲染树节点
      for (const rootId of tree.root_ids) {
        const rootNode = nodeMap.get(rootId);
        if (rootNode) {
          const rootEl = renderTreeNode(rootNode, nodeMap, tree.active_path || []);
          treeContainer.appendChild(rootEl);
        }
      }
    }
  } catch (err) {
    if (loadingEl) {
      loadingEl.textContent = String(err.message || err);
      loadingEl.className = 'conversation-tree-error';
    }
  }
}

/**
 * 关闭对话分支树面板。
 *
 * @returns {void}
 */
export function closeConversationTreePanel() {
  removePanel('turn-tree');
  if (_treePanel) {
    _treePanel.remove();
    _treePanel = null;
  }
}

/**
 * 渲染单个树节点（递归）。
 *
 * @param {Object} node - ConversationTreeNode
 * @param {Map<string, Object>} nodeMap - node_id → node 映射
 * @param {string[]} activePath - 活跃路径的 node_id 列表
 * @returns {HTMLElement} 节点元素
 */
function renderTreeNode(node, nodeMap, activePath) {
  if (!node) return document.createElement('div');

  const isActive = activePath.includes(node.node_id);
  const hasChildren = node.child_message_ids && node.child_message_ids.length > 0;

  const nodeEl = document.createElement('div');
  nodeEl.className = 'conversation-tree-node';
  if (isActive) nodeEl.classList.add('active');
  nodeEl.dataset.nodeId = node.node_id;

  // 节点内容
  const contentEl = document.createElement('div');
  contentEl.className = 'conversation-tree-node-content';

  const versionSpan = document.createElement('span');
  versionSpan.className = 'conversation-tree-node-version';
  versionSpan.textContent = `v${node.version}`;

  const previewSpan = document.createElement('span');
  previewSpan.className = 'conversation-tree-node-preview';
  previewSpan.textContent = node.preview || '(empty)';

  contentEl.appendChild(versionSpan);
  contentEl.appendChild(previewSpan);

  if (isActive) {
    const badge = document.createElement('span');
    badge.className = 'conversation-tree-node-badge';
    badge.textContent = t('tree.active') || 'Active';
    contentEl.appendChild(badge);
  }

  // 点击节点：切换到该版本
  contentEl.addEventListener('click', () => {
    switchToBranch(node);
  });

  nodeEl.appendChild(contentEl);

  // 递归渲染子节点
  if (hasChildren) {
    const childrenEl = document.createElement('div');
    childrenEl.className = 'conversation-tree-children';
    for (const childId of node.child_message_ids) {
      const childNode = nodeMap.get(childId);
      if (childNode) {
        childrenEl.appendChild(renderTreeNode(childNode, nodeMap, activePath));
      }
    }
    nodeEl.appendChild(childrenEl);
  }

  return nodeEl;
}

/**
 * 切换到指定分支版本。
 *
 * 点击树节点时调用：设置活跃版本并重新加载对话消息。
 *
 * @param {Object} node - 目标 ConversationTreeNode
 * @returns {void}
 */
function switchToBranch(node) {
  const conversationId = get('currentConversationId');
  if (!conversationId) return;

  // 设置活跃版本
  convApi.setTurnActiveVersion(conversationId, node.turn_group, node.version).then(() => {
    // 更新本地轮次树
    setActiveVersion(node.turn_group, node.version);

    // 关闭面板
    closeConversationTreePanel();

    // 触发对话重新加载（通过全局事件）
    window.dispatchEvent(new CustomEvent('echomind-branch-switched', {
      detail: { turnGroup: node.turn_group, version: node.version },
    }));
  }).catch((err) => {
    console.warn('切换分支失败:', err);
  });
}

/**
 * 检查分支树面板是否已打开。
 *
 * @returns {boolean}
 */
export function isTreePanelOpen() {
  return _treePanel !== null;
}