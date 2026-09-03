/**
 * EchoMind 快捷指令面板模块 — 输入 `/` 触发命令面板。
 *
 * 职责：
 * 1. 定义 6 个系统快捷指令（summary/compare/extract/translate/timeline/mindmap）
 * 2. 加载并合并用户自定义模板（S56，从后端 IPC 获取）
 * 3. 按输入前缀过滤指令（系统 + 自定义）
 * 4. 渲染浮动指令面板（键盘导航 + 鼠标点击）
 * 5. 选中指令后替换输入框内容
 *
 * 设计参考：QA_UI_DESIGN_PROPOSAL.md §4.10 快捷指令
 * AC-QA-010：输入 `/` 触发命令面板，支持 6 个快捷指令
 * S56：自定义快捷指令模板合并展示
 */

import { t } from './i18n.js';
import { invoke } from './ipc.js';

// ============================================================
// 系统指令定义
// ============================================================

/**
 * @typedef {Object} SlashCommand
 * @property {string} name - 指令名（不含 /）
 * @property {string} label - 显示标签（i18n key: chat.slash_<name>）
 * @property {string} desc - 描述（i18n key: chat.slash_<name>_desc）
 * @property {string} icon - 图标 emoji
 * @property {string} promptTemplate - 发送给 LLM 的 prompt 模板（{query} 为占位符）
 * @property {boolean} [isCustom] - 是否为自定义模板（S56）
 * @property {boolean} [isSkill] - 是否为 Skill 技能（B09 v1.8）
 * @property {string} [id] - 自定义模板 ID（S56）
 */

/** @type {SlashCommand[]} */
export const SLASH_COMMANDS = [
  {
    name: 'summary',
    label: 'chat.slash_summary',
    desc: 'chat.slash_summary_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="9" y1="13" x2="15" y2="13"/><line x1="9" y1="17" x2="13" y2="17"/></svg>',
    promptTemplate: '请总结以下内容的核心要点：{query}',
  },
  {
    name: 'compare',
    label: 'chat.slash_compare',
    desc: 'chat.slash_compare_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v18"/><path d="M5 21h14"/><path d="M3 8h4l2-5"/><path d="M17 8h4l-2-5"/><circle cx="7" cy="8" r="1.5"/><circle cx="17" cy="8" r="1.5"/></svg>',
    promptTemplate: '请对比以下内容的异同：{query}',
  },
  {
    name: 'extract',
    label: 'chat.slash_extract',
    desc: 'chat.slash_extract_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>',
    promptTemplate: '请从以下内容中提取关键信息/条款：{query}',
  },
  {
    name: 'translate',
    label: 'chat.slash_translate',
    desc: 'chat.slash_translate_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>',
    promptTemplate: '请将以下内容翻译：{query}',
  },
  {
    name: 'timeline',
    label: 'chat.slash_timeline',
    desc: 'chat.slash_timeline_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"/><line x1="16" y1="2" x2="16" y2="6"/><line x1="8" y1="2" x2="8" y2="6"/><line x1="3" y1="10" x2="21" y2="10"/></svg>',
    promptTemplate: '请从以下内容中生成时间线：{query}',
  },
  {
    name: 'mindmap',
    label: 'chat.slash_mindmap',
    desc: 'chat.slash_mindmap_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9.5 2A2.5 2.5 0 0 1 12 4.5v15a2.5 2.5 0 0 1-4.96.44 2.5 2.5 0 0 1-2.96-3.08 3 3 0 0 1-.34-5.58 2.5 2.5 0 0 1 1.32-4.24 2.5 2.5 0 0 1 1.98-3A2.5 2.5 0 0 1 9.5 2Z"/><path d="M14.5 2A2.5 2.5 0 0 0 12 4.5v15a2.5 2.5 0 0 0 4.96.44 2.5 2.5 0 0 0 2.96-3.08 3 3 0 0 0 .34-5.58 2.5 2.5 0 0 0-1.32-4.24 2.5 2.5 0 0 0-1.98-3A2.5 2.5 0 0 0 14.5 2Z"/></svg>',
    promptTemplate: '请从以下内容中生成思维导图（Mermaid 格式）：{query}',
  },
  {
    name: 'web',
    label: 'chat.slash_web',
    desc: 'chat.slash_web_desc',
    icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>',
    promptTemplate: '请结合网页搜索结果回答以下问题：{query}',
  },
];

// ============================================================
// 自定义模板缓存（S56）
// ============================================================

/** @type {SlashCommand[]} 自定义模板缓存 */
let _customCommands = [];

/** @type {boolean} 是否已加载过自定义模板 */
let _customLoaded = false;

// ============================================================
// B09 Skill 加载（v1.8：从 {data_dir}/skills/ 发现 slash: true 的技能）
// ============================================================

/** @type {SlashCommand[]} Skill 缓存 */
let _skillCommands = [];

/** @type {boolean} 是否已加载过 Skill */
let _skillsLoaded = false;

/**
 * 从后端加载 Skill 文件（B09 v1.8）。
 *
 * 调用 `discover_skills` IPC 扫描 `{data_dir}/skills/` 目录中的 `.md` 文件，
 * 仅返回 `slash: true` 的技能。转换为 SlashCommand 格式并缓存。
 * 失败时静默降级为空列表（不影响系统指令和自定义模板）。
 *
 * @returns {Promise<SlashCommand[]>} Skill 指令列表
 */
export async function loadSkills() {
  try {
    const skills = await invoke('discover_skills');
    _skillCommands = (skills || []).map((skill) => ({
      name: skill.name,
      label: skill.description || `chat.slash_${skill.name}`,
      desc: skill.description || 'chat.slash_skill_desc',
      icon: '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>',
      promptTemplate: skill.content + '\n\n---\n用户问题: {query}',
      isSkill: true,
    }));
    _skillsLoaded = true;
  } catch (_) {
    // 静默降级：后端不可用时仅使用系统指令和自定义模板
    _skillCommands = [];
    _skillsLoaded = true;
  }
  return _skillCommands;
}

/**
 * 重置 Skill 缓存。
 */
export function resetSkills() {
  _skillCommands = [];
  _skillsLoaded = false;
}

/**
 * 从后端加载自定义快捷指令模板（S56）。
 *
 * 调用 `list_prompt_templates` IPC 获取用户创建的模板，
 * 转换为 SlashCommand 格式并缓存。
 * 失败时静默降级为空列表（不影响系统指令）。
 *
 * @returns {Promise<SlashCommand[]>} 自定义指令列表
 */
export async function loadCustomTemplates() {
  try {
    const templates = await invoke('list_prompt_templates');
    _customCommands = (templates || []).map((tmpl) => ({
      name: tmpl.name,
      label: tmpl.label,
      desc: tmpl.description,
      icon: tmpl.icon || '⚡',
      promptTemplate: tmpl.prompt_template,
      isCustom: true,
      id: tmpl.id,
    }));
    _customLoaded = true;
  } catch (_) {
    // 静默降级：后端不可用时仅使用系统指令
    _customCommands = [];
    _customLoaded = true;
  }
  return _customCommands;
}

/**
 * 重置自定义模板缓存（设置面板修改后调用）。
 */
export function resetCustomTemplates() {
  _customCommands = [];
  _customLoaded = false;
}

/**
 * 获取合并后的全部指令（系统 + 自定义）。
 *
 * @returns {SlashCommand[]} 合并后的指令列表
 */
export function getAllCommands() {
  return [...SLASH_COMMANDS, ..._customCommands, ..._skillCommands];
}

// ============================================================
// 模块级状态（选中索引）
// ============================================================

/** 当前选中索引（0-based）。 */
let _selectedIndex = 0;

/** 当前活跃的面板 DOM 引用（renderSlashCommandPanel 时设置）。 */
let _currentPanel = null;

// ============================================================
// 纯函数：过滤与选择
// ============================================================

/**
 * 按输入前缀过滤指令（系统 + 自定义合并）。
 *
 * - 输入 `/` → 返回全部指令（系统在前，自定义在后）
 * - 输入 `/sum` → 返回匹配前缀 `sum` 的指令
 * - 输入不以 `/` 开头 → 返回空数组
 *
 * @param {string} input - 输入框当前值
 * @returns {SlashCommand[]} 过滤后的指令列表
 */
export function filterSlashCommands(input) {
  // 必须以 / 开头
  if (!input || !input.startsWith('/')) return [];

  // 仅 / 时返回全部（合并系统 + 自定义）
  const query = input.slice(1).toLowerCase();

  // 空查询（仅 /）→ 返回全部
  if (!query) return getAllCommands();

  // 按前缀匹配（name 或 label 均可匹配）
  const all = getAllCommands();
  return all.filter((cmd) => {
    const name = cmd.name.toLowerCase();
    const label = t(cmd.label).toLowerCase();
    return name.startsWith(query) || label.startsWith(query);
  });
}

// ============================================================
// DOM 渲染
// ============================================================

/**
 * 渲染快捷指令面板。
 *
 * 在容器中创建 `.slash-command-panel` 元素，包含指令列表项。
 * 首项默认选中。点击指令项触发 onSelect 回调。
 * 自定义模板项显示 ⚡ 角标以区分（S56）。
 *
 * @param {HTMLElement} container - 挂载容器
 * @param {SlashCommand[]} filtered - 过滤后的指令列表
 * @param {(cmd: SlashCommand) => void} [onSelect] - 选中指令时的回调
 * @returns {HTMLElement} 创建的面板根元素
 */
export function renderSlashCommandPanel(container, filtered, onSelect) {
  // 清除已有面板
  const existing = container.querySelector('.slash-command-panel');
  if (existing) existing.remove();

  // 重置选中索引
  _selectedIndex = 0;

  const panel = document.createElement('div');
  panel.className = 'slash-command-panel';

  if (filtered.length === 0) {
    const empty = document.createElement('div');
    empty.className = 'slash-command-empty';
    empty.textContent = t('chat.slash_no_match') || '无匹配指令';
    panel.appendChild(empty);
  } else {
    filtered.forEach((cmd, index) => {
      const item = document.createElement('div');
      item.className = 'slash-command-item';
      if (index === 0) item.classList.add('slash-command-selected');
      item.dataset.cmdName = cmd.name;

      const icon = document.createElement('span');
      icon.className = 'slash-command-icon';
      icon.textContent = cmd.icon;

      const text = document.createElement('div');
      text.className = 'slash-command-text';

      const name = document.createElement('span');
      name.className = 'slash-command-name';
      name.textContent = `/${cmd.name}`;

      // 自定义模板显示角标（S56）
      if (cmd.isCustom) {
        const badge = document.createElement('span');
        badge.className = 'slash-command-badge';
        badge.textContent = '⚡';
        badge.title = t('chat.slash_custom') || '自定义';
        name.appendChild(badge);
      }
      // Skill 显示角标（B09 v1.8）
      if (cmd.isSkill) {
        const badge = document.createElement('span');
        badge.className = 'slash-command-badge';
        badge.innerHTML = '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/></svg>';
        badge.title = t('chat.slash_skill') || 'Skill';
        name.appendChild(badge);
      }

      const desc = document.createElement('span');
      desc.className = 'slash-command-desc';
      desc.textContent = t(cmd.desc);

      text.appendChild(name);
      text.appendChild(desc);
      item.appendChild(icon);
      item.appendChild(text);

      item.onclick = () => {
        _selectedIndex = index;
        if (onSelect) onSelect(cmd);
      };

      // 鼠标悬停更新选中
      item.onmouseenter = () => {
        _selectedIndex = index;
        updateSelectedVisual(panel, filtered);
      };

      panel.appendChild(item);
    });
  }

  container.appendChild(panel);

  // 保存当前面板引用，供 navigateSlashCommand 使用
  _currentPanel = panel;

  return panel;
}

/**
 * 更新面板中选中项的视觉状态。
 * @param {HTMLElement} panel - 面板根元素
 * @param {SlashCommand[]} filtered - 过滤后的指令列表
 */
function updateSelectedVisual(panel, filtered) {
  const items = panel.querySelectorAll('.slash-command-item');
  items.forEach((item, index) => {
    item.classList.toggle('slash-command-selected', index === _selectedIndex);
  });
}

// ============================================================
// 键盘导航
// ============================================================

/**
 * 键盘导航选中项（上/下/首/末）。
 *
 * - 'down' → 选中下一项，末项时回绕到首项
 * - 'up' → 选中上一项，首项时回绕到末项
 * - 'home' → 跳转到首项（P2-4）
 * - 'end' → 跳转到末项（P2-4）
 *
 * @param {SlashCommand[]} filtered - 过滤后的指令列表
 * @param {'up'|'down'|'home'|'end'} direction - 导航方向
 * @returns {void}
 */
export function navigateSlashCommand(filtered, direction) {
  if (!filtered || filtered.length === 0) return;

  if (filtered.length === 1 && direction !== 'home' && direction !== 'end') return;

  if (direction === 'down') {
    _selectedIndex = (_selectedIndex + 1) % filtered.length;
  } else if (direction === 'up') {
    _selectedIndex = (_selectedIndex - 1 + filtered.length) % filtered.length;
  } else if (direction === 'home') {
    _selectedIndex = 0;
  } else if (direction === 'end') {
    _selectedIndex = filtered.length - 1;
  } else {
    return;
  }

  // 更新视觉（使用模块级引用，避免多面板冲突）
  const panel = _currentPanel;
  if (panel) {
    updateSelectedVisual(panel, filtered);
    // 滚动到选中项
    const items = panel.querySelectorAll('.slash-command-item');
    if (items[_selectedIndex] && typeof items[_selectedIndex].scrollIntoView === 'function') {
      items[_selectedIndex].scrollIntoView({ block: 'nearest' });
    }
  }
}

/**
 * 获取当前选中的指令。
 * @param {SlashCommand[]} filtered - 过滤后的指令列表
 * @returns {SlashCommand|null} 当前选中指令
 */
export function getSelectedSlashCommand(filtered) {
  if (!filtered || filtered.length === 0) return null;
  return filtered[_selectedIndex] || null;
}

/**
 * 将选中的指令应用到输入框。
 *
 * 替换输入框内容为 `/command `（含尾部空格，便于用户继续输入参数）。
 *
 * @param {SlashCommand} cmd - 要应用的指令
 * @param {HTMLInputElement|HTMLTextAreaElement} inputEl - 输入框元素
 */
export function applySlashCommand(cmd, inputEl) {
  inputEl.value = `/${cmd.name} `;
  inputEl.focus();
  // 光标置于末尾
  inputEl.setSelectionRange(inputEl.value.length, inputEl.value.length);
}

/**
 * 处理输入中的快捷指令，将 `/command query` 展开为完整 prompt。
 *
 * 如果输入以 `/command ` 开头且找到匹配的指令（系统或自定义），
 * 则用指令的 `promptTemplate` 替换 `{query}` 占位符生成完整 prompt。
 * 如果不匹配任何指令，原样返回。
 *
 * @param {string} text - 用户输入的完整文本
 * @returns {{text: string, matched: boolean, command: SlashCommand|null}} 处理结果
 */
export function processSlashCommand(text) {
  if (!text || !text.startsWith('/')) {
    return { text, matched: false, command: null };
  }

  // 提取指令名（/ 后到第一个空格）
  const spaceIdx = text.indexOf(' ');
  if (spaceIdx === -1) {
    // 只有 /command 没有空格，不处理
    return { text, matched: false, command: null };
  }

  const cmdName = text.slice(1, spaceIdx);
  const userQuery = text.slice(spaceIdx + 1).trim();

  // 在系统 + 自定义指令中查找匹配
  const all = getAllCommands();
  const cmd = all.find((c) => c.name === cmdName);
  if (!cmd) {
    return { text, matched: false, command: null };
  }

  // 用户未输入查询内容时，使用空字符串
  const finalQuery = userQuery || '';

  // 用 promptTemplate 替换 {query} 占位符
  const expanded = cmd.promptTemplate.replace('{query}', finalQuery);
  return { text: expanded, matched: true, command: cmd };
}

/**
 * 重置选中索引（面板关闭时调用）。
 */
export function resetSlashSelection() {
  _selectedIndex = 0;
}

/**
 * 移除快捷指令面板（如果存在）。
 * @param {HTMLElement} container - 面板挂载容器
 */
export function removeSlashCommandPanel(container) {
  const panel = container?.querySelector('.slash-command-panel');
  if (panel) panel.remove();
  _currentPanel = null;
  resetSlashSelection();
}
