/**
 * EchoMind 工具函数模块 — 纯函数集合，无副作用，无 DOM 依赖。
 *
 * 设计原则：
 * 1. 纯函数 — 输入 → 输出，无副作用，可独立单元测试
 * 2. 无 DOM 依赖 — 不直接操作 document，便于 jsdom 测试
 * 3. 单一职责 — 每个函数只做一件事
 */

/**
 * 按 id 获取 DOM 元素简写助手。
 *
 * 返回 any 类型以兼容 vanilla JS 中对 .value / .disabled / .dataset
 * 等属性的访问（渐进式类型迁移：后续可逐步替换为具体类型）。
 *
 * @param {string} id - 元素 ID（不含 # 前缀）
 * @returns {any} 对应的 DOM 元素
 */
export function $(id) {
  return document.getElementById(id);
}

/**
 * 脱敏错误消息：过滤 API Key、文件路径中的用户名等敏感信息（REQ-UI-005-AC-2）。
 * @param {string|Error} err - 原始错误对象或字符串
 * @returns {string} 脱敏后的安全错误消息
 */
export function sanitizeError(err) {
  let msg = String(err);
  // 过滤 API Key（sk-xxx 格式，保留前缀 + 末四位）
  msg = msg.replace(/sk-[a-zA-Z0-9]{8,}/g, 'sk-****');
  // 过滤 Windows 用户路径
  msg = msg.replace(/\\Users\\[^\\]+?\\/g, '\\Users\\****\\');
  // 过滤 Unix 用户路径
  msg = msg.replace(/\/Users\/[^/]+?\//g, '/Users/****/');
  return msg;
}

/**
 * 从入库副本路径中剥离 MD5 哈希前缀，返回展示用文件名。
 * 入库副本命名规则 {md5}-{原始文件名}（REQ-ING-001）。
 * @param {string} filePath - 入库后的本地路径
 * @returns {string} 去除哈希前缀后的文件名
 */
export function displayDocName(filePath) {
  const base = filePath.split('/').pop() || filePath;
  return base.length > 33 && base[32] === '-' ? base.slice(33) : base;
}

/**
 * 从文档对象安全提取状态字符串（后端枚举或字符串形式均兼容）。
 * @param {Object} d - 文档对象
 * @returns {string} 状态标识（Pending/Processing/Indexed/Failed）
 */
export function docStatusOf(d) {
  return typeof d.status === 'string' ? d.status : 'Failed';
}

/**
 * 格式化字节大小（REQ-VEC-008）。
 * @param {number} bytes - 字节数
 * @returns {string} 人类可读的大小字符串（如 "32.5 MB"）
 */
export function formatBytes(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / 1024 / 1024).toFixed(1) + ' MB';
}

/**
 * 从路径提取文件名。
 * @param {string} p - 文件路径
 * @returns {string} 文件名（最后一段）
 */
export function basename(p) {
  return p.split('/').pop() || p;
}

/**
 * 从文件名提取扩展名（小写）。
 * @param {string} p - 文件路径
 * @returns {string} 扩展名（不含点），如 "md"
 */
export function extname(p) {
  const name = basename(p);
  const dot = name.lastIndexOf('.');
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : '';
}

/**
 * 检查当前焦点是否在输入元素上（用于全局快捷键不触发判断）。
 * @returns {boolean} 当前焦点是否在 input/textarea/contenteditable 上
 */
export function isInputFocused() {
  const el = document.activeElement;
  return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable);
}

/**
 * 将多模态管线子阶段标识转换为可读文案（REQ-MM-004）。
 * @param {string} phase - 子阶段标识
 * @returns {string} 可读文案
 */
export function getSubPhaseLabel(phase) {
  const labelKeys = {
    text_extracting: 'doc_phases.text_extracting',
    image_rendering: 'doc_phases.image_rendering',
    ocr: 'doc_phases.ocr',
    vlm_enhancing: 'doc_phases.vlm_enhancing',
  };
  return labelKeys[phase] || phase;
}

/**
 * 文档状态 → 标签 + 样式映射表。
 */
export const DOC_STATUS_STYLE = {
  Pending: ['status_pending', 'text-slate-400 border-slate-500/40'],
  Processing: ['status_processing', 'text-amber-300 border-amber-400/40'],
  Indexed: ['status_indexed', 'text-accent border-accent/40'],
  Failed: ['status_failed', 'text-red-400 border-red-400/40'],
};

/**
 * Provider 预设配置表（REQ-LLM-003）。
 *
 * 所有预设均为 OpenAI 兼容端点。base_url 路径格式由后端
 * `resolve_chat_completions_url()` 智能拼接，兼容标准格式、
 * 版本路径（如 /v4）和完整端点路径。
 */
export const PRESETS = {
  deepseek: { label: 'DeepSeek', base_url: 'https://api.deepseek.com', model: 'deepseek-chat', keyUrl: 'https://platform.deepseek.com/api_keys', needKey: true, descKey: 'presets.deepseek_desc' },
  openai: { label: 'OpenAI', base_url: 'https://api.openai.com', model: 'gpt-4o-mini', keyUrl: 'https://platform.openai.com/api-keys', needKey: true, descKey: 'presets.openai_desc' },
  qwen: { label: 'Qwen', base_url: 'https://dashscope.aliyuncs.com/compatible-mode', model: 'qwen-plus', keyUrl: 'https://dashscope.console.aliyun.com/apiKey', needKey: true, descKey: 'presets.qwen_desc' },
  kimi: { label: 'Kimi', base_url: 'https://api.moonshot.cn', model: 'moonshot-v1-8k', keyUrl: 'https://platform.moonshot.cn/console/api-keys', needKey: true, descKey: 'presets.kimi_desc' },
  glm: { label: 'GLM', base_url: 'https://open.bigmodel.cn/api/paas/v4', model: 'glm-4-flash', keyUrl: 'https://open.bigmodel.cn/usercenter/apikeys', needKey: true, descKey: 'presets.glm_desc' },
  minimax: { label: 'MiniMax', base_url: 'https://api.minimax.chat', model: 'MiniMax-Text-01', keyUrl: 'https://platform.minimaxi.com/user-center/basic-information/interface-key', needKey: true, descKey: 'presets.minimax_desc' },
  mistral: { label: 'Mistral', base_url: 'https://api.mistral.ai', model: 'mistral-large-latest', keyUrl: 'https://console.mistral.ai/api-keys', needKey: true, descKey: 'presets.mistral_desc' },
  grok: { label: 'Grok', base_url: 'https://api.x.ai', model: 'grok-3', keyUrl: 'https://console.x.ai', needKey: true, descKey: 'presets.grok_desc' },
  ollama: { label: 'Ollama', base_url: 'http://localhost:11434', model: 'llama3.1', keyUrl: 'https://ollama.com/library', needKey: false, descKey: 'presets.ollama_desc' },
  custom: { label: 'Custom', base_url: '', model: '', keyUrl: '', needKey: true, descKey: 'presets.custom_desc' },
};

/** 默认工作空间 ID */
export const WORKSPACE = 'default';

/**
 * 统一剪贴板复制工具（REQ-IX-003 AC-5）。
 *
 * 处理非安全上下文检测（`window.isSecureContext === false`）和
 * `navigator.clipboard` API 不可用时的 fallback。
 *
 * 调用方负责根据返回值显示 toast（成功 → toast「已复制」，失败 → toastError「复制失败」）。
 *
 * @param {string} text - 要复制的文本
 * @returns {Promise<boolean>} 是否复制成功
 */
export async function copyToClipboard(text) {
  // 非安全上下文或 clipboard API 不可用 → 使用 execCommand fallback
  if (!window.isSecureContext || !navigator.clipboard || !navigator.clipboard.writeText) {
    try {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.top = '-9999px';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      const ok = document.execCommand('copy');
      document.body.removeChild(textarea);
      return ok;
    } catch (_) {
      return false;
    }
  }

  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (_) {
    return false;
  }
}
