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
 * @property {string} [id] - 自定义模板 ID（S56）
 */

/** @type {SlashCommand[]} */
export const SLASH_COMMANDS = [
  {
    name: 'summary',
    label: 'chat.slash_summary',
    desc: 'chat.slash_summary_desc',
    icon: '📝',
    promptTemplate: '请总结以下内容的核心要点：{query}',
  },
  {
    name: 'compare',
    label: 'chat.slash_compare',
    desc: 'chat.slash_compare_desc',
    icon: '⚖️',
    promptTemplate: '请对比以下内容的异同：{query}',
  },
  {
    name: 'extract',
    label: 'chat.slash_extract',
    desc: 'chat.slash_extract_desc',
    icon: '🔍',
    promptTemplate: '请从以下内容中提取关键信息/条款：{query}',
  },
  {
    name: 'translate',
    label: 'chat.slash_translate',
    desc: 'chat.slash_translate_desc',
    icon: '🌐',
    promptTemplate: '请将以下内容翻译：{query}',
  },
  {
    name: 'timeline',
    label: 'chat.slash_timeline',
    desc: 'chat.slash_timeline_desc',
    icon: '📅',
    promptTemplate: '请从以下内容中生成时间线：{query}',
  },
  {
    name: 'mindmap',
    label: 'chat.slash_mindmap',
    desc: 'chat.slash_mindmap_desc',
    icon: '🧠',
    promptTemplate: '请从以下内容中生成思维导图（Mermaid 格式）：{query}',
  },
];

// ============================================================
// 自定义模板缓存（S56）
// ============================================================

/** @type {SlashCommand[]} 自定义模板缓存 */
let _customCommands = [];

/** @type {boolean} 是否已加载过自定义模板 */
let _customLoaded = false;

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
  return [...SLASH_COMMANDS, ..._customCommands];
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
 * 键盘导航选中项（上/下）。
 *
 * - 'down' → 选中下一项，末项时回绕到首项
 * - 'up' → 选中上一项，首项时回绕到末项
 *
 * @param {SlashCommand[]} filtered - 过滤后的指令列表
 * @param {'up'|'down'} direction - 导航方向
 * @returns {void}
 */
export function navigateSlashCommand(filtered, direction) {
  if (!filtered || filtered.length === 0) return;

  if (filtered.length === 1) return;

  if (direction === 'down') {
    _selectedIndex = (_selectedIndex + 1) % filtered.length;
  } else {
    _selectedIndex = (_selectedIndex - 1 + filtered.length) % filtered.length;
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
