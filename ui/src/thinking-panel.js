/**
 * EchoMind 思维链折叠面板 — DeepSeek 风格可折叠思考过程展示。
 *
 * 职责：
 * 1. 创建可折叠的思维链面板（header + content）
 * 2. 点击 header 切换展开/折叠
 * 3. update() 实时更新思考文本
 * 4. collapse() 强制折叠
 * 5. setComplete() 标记思考完成（改变 header 文案 + 显示思考耗时）
 * 6. startThinking() 记录思考开始时间（用于耗时计算）
 * 7. ensureMinLoadingDelay() 保证最小加载时间防闪烁
 *
 * 设计参考：DeepSeek 聊天页面的思维链折叠交互
 * - 默认折叠，灰色小字号
 * - 展开时显示完整思考过程
 * - chevron 图标旋转动画
 * - 思考完成后显示思考耗时（"思考了 X 秒"）
 * - ensureMinLoadingDelay 防止极快响应导致闪烁
 */

/** 最小 loading 显示时间（ms）— 防止极快响应时思考面板闪烁 */
const MIN_LOADING_DELAY_MS = 600;

import { t } from './i18n.js';
import { renderMarkdown } from './markdown.js';

/** localStorage 存储键前缀：思考面板展开/折叠状态（按消息 ID 持久化，每条消息独立） */
const THINKING_EXPANDED_PREFIX = 'echomind_thinking_expanded_';

/**
 * 从 localStorage 读取指定消息的展开状态。
 * 无 msgId 时默认折叠（不使用全局变量，确保每条消息状态独立）。
 * @param {string|null} msgId - 消息 ID
 * @returns {boolean}
 */
function loadExpandedState(msgId) {
  if (msgId) {
    try {
      const val = localStorage.getItem(THINKING_EXPANDED_PREFIX + msgId);
      if (val !== null) return val === 'true';
    } catch (_) { /* 隐私模式 */ }
  }
  return false; // 默认折叠
}

/**
 * 持久化指定消息的展开状态到 localStorage。
 * 无 msgId 时不持久化（流式期间 msgId 尚未确定时，状态仅在内存中）。
 * @param {string|null} msgId - 消息 ID
 * @param {boolean} expanded
 */
function saveExpandedState(msgId, expanded) {
  if (msgId) {
    try {
      localStorage.setItem(THINKING_EXPANDED_PREFIX + msgId, String(expanded));
    } catch (_) { /* 隐私模式 */ }
  }
}

/**
 * SVG 图标常量。
 */
const ICON_BULB = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18h6M10 22h4M12 2a7 7 0 0 0-4 12.7V17h8v-2.3A7 7 0 0 0 12 2z"/></svg>`;
const ICON_CHEVRON = `<svg class="thinking-panel-chevron shrink-0 transition-transform duration-150" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>`;

// REQ-RAG-052: Agent 步骤类型图标
const ICON_AGENT_THOUGHT = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18h6M10 22h4M12 2a7 7 0 0 0-4 12.7V17h8v-2.3A7 7 0 0 0 12 2z"/></svg>`;
const ICON_AGENT_ACTION = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/></svg>`;
const ICON_AGENT_OBSERVATION = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>`;
const ICON_AGENT_ANSWER = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`;

/**
 * 阶段图标库（需求 5：思考生动动画 — 图标流转）。
 * 按 chat_phase 阶段切换图标，视觉呈现「准备→检索→生成」的思考流转。
 */
const STAGE_ICONS = {
  preparing: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>`,
  retrieving: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>`,
  generating: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`,
};

/**
 * 打字点动画 HTML（已废弃 — 图标动画已足够表达思考状态，保留空字符串避免引用错误）。
 */
const TYPING_DOTS = '';

/**
 * 创建可折叠的思维链面板。
 *
 * @param {string} [initialText=''] - 初始思考文本
 * @returns {ThinkingPanelHandle}
 */
export function createThinkingPanel(initialText = '') {
  const container = document.createElement('div');
  container.className = 'thinking-panel';

  // Header：图标 + 文本 + chevron，可点击切换
  const header = document.createElement('div');
  header.className = 'thinking-panel-header';
  header.innerHTML = `
    <span class="thinking-panel-icon shrink-0 thinking-stage-icon">${ICON_BULB}</span>
    <span class="thinking-panel-text flex-1 truncate">${initialText || t('chat.thinking_preparing')}</span>
    ${ICON_CHEVRON}
  `;

  // Content：折叠状态隐藏，展开时显示完整思考过程
  const content = document.createElement('div');
  content.className = 'thinking-panel-content hidden';

  // 推理内容容器（reasoning_content 流式追加；无推理内容时不存在）
  let reasoningEl = null;

  container.appendChild(header);
  container.appendChild(content);

  let expanded = false;
  /** 关联的消息 ID（用于持久化展开/折叠状态） */
  let _msgId = null;
  /** 思考开始时间戳（ms）— 用于计算思考耗时 */
  let _thinkStartTime = null;
  /** 思考完成时间戳（ms）— 用于计算思考耗时 */
  let _thinkEndTime = null;
  /** 是否已收到首个 token（用于 AWAITING_FIRST_CHANGE 状态判断） */
  let _firstTokenReceived = false;
  /** ensureMinLoadingDelay 的延迟完成回调（在最小延迟到期后调用） */
  let _minDelayPromise = null;

  /** 同步展开/折叠的 DOM 表现 */
  function applyExpanded() {
    const wasHidden = content.classList.contains('hidden');
    content.classList.toggle('hidden', !expanded);
    const chevron = header.querySelector('.thinking-panel-chevron');
    if (chevron) {
      chevron.style.transform = expanded ? 'rotate(180deg)' : '';
    }
    // 展开时添加 fade-in-zoom-expand 动画（DeepSeek 风格）
    if (expanded && wasHidden) {
      content.classList.add('fade-in-zoom-expand');
      // 动画结束后移除类，避免重复触发
      setTimeout(() => {
        content.classList.remove('fade-in-zoom-expand');
      }, 300);
    }
  }

  // 点击 header 切换展开/折叠，并持久化状态
  header.onclick = () => {
    expanded = !expanded;
    applyExpanded();
    saveExpandedState(_msgId, expanded);
  };

  return {
    container,
    /**
     * 更新 header 文本；可附带阶段标识切换图标（需求 5 图标流转）。
     * 展开内容由 appendReasoning/appendStage 管理。
     * @param {string} text - 阶段文本
     * @param {string} [phase] - 阶段标识（preparing / retrieving / generating）
     */
    update(text, phase) {
      const textEl = container.querySelector('.thinking-panel-text');
      if (textEl) {
        textEl.textContent = text;
      }
      // 图标流转：按阶段切换图标（preparing→时钟 / retrieving→放大镜 / generating→星星）
      if (phase && STAGE_ICONS[phase]) {
        const iconEl = container.querySelector('.thinking-stage-icon');
        if (iconEl) {
          iconEl.innerHTML = STAGE_ICONS[phase];
          // 添加旋转动画表示「正在处理」
          iconEl.classList.add('thinking-icon-active');
        }
      }
    },
    /**
     * 追加模型推理内容（reasoning_content）到展开内容。
     * DeepSeek R1 / Qwen 等推理模型的思考过程经 chat_reasoning 事件流式到达，
     * 逐段追加显示（纯文本，避免流式期间逐帧 markdown 渲染的性能开销）；
     * 流完成后由 finalizeReasoning() 做一次性 markdown 渲染。
     * @param {string} text - 推理内容增量
     */
    appendReasoning(text) {
      if (!text) return;
      if (!reasoningEl) {
        reasoningEl = document.createElement('div');
        reasoningEl.className = 'thinking-reasoning';
        content.appendChild(reasoningEl);
      }
      reasoningEl.textContent += text;
    },
    /**
     * 渲染完整推理内容（版本切换/历史加载场景）：
     * 清空并以 markdown 渲染目标版本的思考过程（对齐 DeepSeek ds-think-content）。
     * @param {string} text - 完整推理内容
     */
    renderReasoning(text) {
      if (!text) return;
      if (!reasoningEl) {
        reasoningEl = document.createElement('div');
        reasoningEl.className = 'thinking-reasoning';
        content.appendChild(reasoningEl);
      }
      // renderMarkdown 会写入 dataset.rawMarkdown 作为已渲染标记
      renderMarkdown(reasoningEl, text, null, true);
    },
    /**
     * 流完成后把已累加的纯文本思考内容做一次性 markdown 渲染。
     * 幂等：renderMarkdown 已写入 dataset.rawMarkdown 表示已渲染。
     */
    finalizeReasoning() {
      if (!reasoningEl || reasoningEl.dataset.rawMarkdown) return;
      const raw = reasoningEl.textContent || '';
      if (!raw.trim()) return;
      renderMarkdown(reasoningEl, raw, null, true);
    },
    /**
     * 追加一条处理阶段记录到展开内容（时间线形式）。
     * 后端 chat_phase 只推送阶段状态（如「正在检索…」），逐条追加后
     * 展开面板能完整看到处理流程，而不是一片空白。
     * @param {string} text - 阶段消息
     */
    appendStage(text) {
      if (!text) return;
      const line = document.createElement('div');
      line.className = 'thinking-stage';
      line.textContent = text;
      content.appendChild(line);
    },
    /**
     * REQ-RAG-052: 追加 Agent 步骤卡片（Thought/Action/Observation）。
     *
     * 每个步骤渲染为独立卡片，含步骤编号 + 类型图标 + 内容。
     * 卡片默认折叠，点击展开查看详情。
     *
     * @param {object} step - AgentStepPayload
     * @param {string} step.step_type - thought/action/observation/answer
     * @param {string} step.content - 步骤内容
     * @param {string} [step.tool] - 工具名称（仅 action）
     * @param {string} [step.input] - 工具输入（仅 action）
     * @param {number} step.iteration - 迭代轮次（从 1 开始）
     */
    appendAgentStep(step) {
      if (!step || !step.step_type) return;
      const card = document.createElement('div');
      card.className = `agent-step-card agent-step-${step.step_type}`;

      const header = document.createElement('div');
      header.className = 'agent-step-card-header';

      const iconMap = {
        thought: ICON_AGENT_THOUGHT,
        action: ICON_AGENT_ACTION,
        observation: ICON_AGENT_OBSERVATION,
        answer: ICON_AGENT_ANSWER,
      };
      const labelMap = {
        thought: t('chat.agent_thought', { default: 'Thought' }),
        action: t('chat.agent_action', { default: 'Action' }),
        observation: t('chat.agent_observation', { default: 'Observation' }),
        answer: t('chat.agent_answer', { default: 'Answer' }),
      };

      const icon = iconMap[step.step_type] || ICON_BULB;
      const label = labelMap[step.step_type] || step.step_type;

      header.innerHTML = `
        <span class="agent-step-icon shrink-0">${icon}</span>
        <span class="agent-step-label flex-1 truncate">
          <span class="agent-step-iteration">[${step.iteration}]</span>
          ${label}
          ${step.tool ? `<span class="agent-step-tool">· ${step.tool}</span>` : ''}
        </span>
        <span class="agent-step-chevron shrink-0 transition-transform duration-150">
          <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>
        </span>
      `;

      const body = document.createElement('div');
      body.className = 'agent-step-card-body hidden';
      body.textContent = step.content;

      // 点击 header 切换展开/折叠
      header.onclick = (e) => {
        e.stopPropagation();
        const wasHidden = body.classList.contains('hidden');
        body.classList.toggle('hidden');
        const chevron = header.querySelector('.agent-step-chevron svg');
        if (chevron) {
          chevron.style.transform = wasHidden ? 'rotate(180deg)' : '';
        }
      };

      card.appendChild(header);
      card.appendChild(body);
      content.appendChild(card);

      // 自动展开 thought 和 answer，折叠 action 和 observation
      if (step.step_type === 'thought' || step.step_type === 'answer') {
        body.classList.remove('hidden');
        const chevron = header.querySelector('.agent-step-chevron svg');
        if (chevron) chevron.style.transform = 'rotate(180deg)';
      }
    },
    /**
     * REQ-RAG-052: 设置 Agent 进度条（第 N/最大轮次）。
     * @param {number} current - 当前轮次
     * @param {number} max - 最大轮次（通常 5）
     */
    setAgentProgress(current, max) {
      let progressEl = container.querySelector('.agent-progress-bar');
      if (!progressEl) {
        progressEl = document.createElement('div');
        progressEl.className = 'agent-progress-bar';
        progressEl.innerHTML = `
          <div class="agent-progress-track">
            <div class="agent-progress-fill"></div>
          </div>
          <span class="agent-progress-text"></span>
        `;
        container.appendChild(progressEl);
      }
      const fill = progressEl.querySelector('.agent-progress-fill');
      const text = progressEl.querySelector('.agent-progress-text');
      const pct = Math.min(100, Math.round((current / max) * 100));
      if (fill) fill.style.width = pct + '%';
      if (text) text.textContent = t('chat.agent_progress', { current, max, default: `${current}/${max}` });
    },
    /**
     * 设置关联的消息 ID（用于持久化展开/折叠状态）。
     * 加载历史消息时调用，设置后立即恢复该消息的展开状态。
     * @param {string} msgId
     */
    setMsgId(msgId) {
      _msgId = msgId || null;
      // 恢复持久化的展开状态
      expanded = loadExpandedState(_msgId);
      applyExpanded();
    },
    /**
     * 查询当前展开状态。
     * @returns {boolean}
     */
    isExpanded() {
      return expanded;
    },
    /**
     * 折叠面板（不改变持久化状态，仅视觉折叠）。
     * 流式生成中收到首个 token 时调用：如果用户未展开过，保持折叠。
     */
    collapse() {
      expanded = false;
      applyExpanded();
    },
    /**
     * 展开面板（不改变持久化状态，仅视觉展开）。
     */
    expand() {
      expanded = true;
      applyExpanded();
    },
    /**
     * 记录思考开始时间（在 chat_phase 首次触发时调用）。
     * 用于在思考完成时计算并显示思考耗时。
     */
    startThinking() {
      _thinkStartTime = Date.now();
      _firstTokenReceived = false;
    },
    /**
     * 标记已收到首个 token（AWAITING_FIRST_CHANGE → GENERATING 过渡）。
     * 在 chat_token 首次触发时调用，用于消除空白等待状态。
     */
    markFirstTokenReceived() {
      _firstTokenReceived = true;
    },
    /**
     * 查询是否已收到首个 token。
     * @returns {boolean}
     */
    isFirstTokenReceived() {
      return _firstTokenReceived;
    },
    /**
     * 计算思考耗时（秒）。
     * @returns {number|null} 耗时秒数（无开始时间时返回 null）
     */
    getThinkDuration() {
      if (!_thinkStartTime) return null;
      const end = _thinkEndTime || Date.now();
      return Math.round((end - _thinkStartTime) / 1000);
    },
    /**
     * 确保最小 loading 显示时间，防止极快响应导致闪烁。
     * 在思考完成时调用：如果思考耗时 < MIN_LOADING_DELAY_MS，
     * 延迟 MIN_LOADING_DELAY_MS - elapsed 后才真正标记完成。
     * @returns {Promise<void>}
     */
    ensureMinLoadingDelay() {
      if (!_thinkStartTime) return Promise.resolve();
      const elapsed = Date.now() - _thinkStartTime;
      if (elapsed >= MIN_LOADING_DELAY_MS) return Promise.resolve();
      const remaining = MIN_LOADING_DELAY_MS - elapsed;
      return new Promise((resolve) => {
        setTimeout(resolve, remaining);
      });
    },
    /**
     * 标记思考完成，并显示思考耗时。
     * @param {string} [text] - 自定义完成文案（不传时自动生成带耗时的文案）
     */
    async setComplete(text) {
      _thinkEndTime = Date.now();
      // 确保最小 loading 时间
      await this.ensureMinLoadingDelay();
      const textEl = container.querySelector('.thinking-panel-text');
      if (textEl) {
        if (text) {
          textEl.textContent = text;
        } else {
          // 自动生成带耗时的完成文案
          const duration = this.getThinkDuration();
          if (duration !== null && duration > 0) {
            textEl.textContent = t('chat.thinking_duration', { seconds: duration });
          } else {
            textEl.textContent = t('chat.thinking_complete');
          }
        }
      }
      // 移除图标旋转动画（思考完成）
      const iconEl = container.querySelector('.thinking-stage-icon');
      if (iconEl) iconEl.classList.remove('thinking-icon-active');
    },
    /**
     * 重置面板到初始状态（清空内容、重置文本）。
     * 用于编辑重发时重用 assistant 块。
     */
    reset() {
      reasoningEl = null;
      content.innerHTML = '';
      content.classList.add('hidden');
      expanded = false;
      // 重置消息 ID 关联（新消息未持久化，使用全局默认值）
      _msgId = null;
      // 重置思考时间状态
      _thinkStartTime = null;
      _thinkEndTime = null;
      _firstTokenReceived = false;
      // REQ-RAG-052: 移除 Agent 进度条（如有）
      const progressEl = container.querySelector('.agent-progress-bar');
      if (progressEl) progressEl.remove();
      const textEl = container.querySelector('.thinking-panel-text');
      if (textEl) textEl.textContent = t('chat.thinking_preparing');
      // 重置图标（回到初始「思考中」状态）
      const iconEl = container.querySelector('.thinking-stage-icon');
      if (iconEl) {
        iconEl.innerHTML = ICON_BULB;
        iconEl.classList.add('thinking-icon-active');
      }
      const chevron = container.querySelector('.thinking-panel-chevron');
      if (chevron) chevron.style.transform = '';
    },
    /**
     * 清空展开内容（reasoning + stages），不改变折叠/完成状态。
     * 用于分支版本切换：先清空旧版本内容，再追加目标版本的推理。
     */
    clearContent() {
      reasoningEl = null;
      content.innerHTML = '';
    },
    /**
     * 获取累积的推理内容文本。
     * @returns {string|null}
     */
    getReasoning() {
      return reasoningEl ? reasoningEl.textContent || null : null;
    },
  };
}
