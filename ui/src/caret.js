/**
 * EchoMind 流式光标管理模块 — caret.js
 *
 * 职责：
 * 1. 在流式生成期间，于 assistant 消息末尾显示闪烁光标（▋）
 * 2. 生成完成后移除光标
 * 3. 支持幂等操作（重复调用不重复插入）
 *
 * 设计参考：
 * - Cursor: 细竖条光标
 * - Claude.ai: 小实心方块
 * - ChatGPT: 脉冲圆点
 *
 * EchoMind 选择细竖条（▋），与文本底部对齐，视觉干扰最小。
 *
 * 对应 AC: TC-QA-001 (设计提案 §4.4.1)
 */

// ============================================================
// 常量
// ============================================================

/** 光标 Tailwind 工具类集合。 */
const CARET_CLASS = 'inline-block w-[2px] h-[1.1em] bg-accent ml-[2px] animate-caret-blink align-text-bottom rounded-[1px] motion-reduce:animate-none motion-reduce:opacity-50';

// ============================================================
// 公共 API
// ============================================================

/**
 * 在指定容器末尾显示流式光标。
 *
 * 如果容器中已存在光标，则跳过（幂等）。
 * 光标是一个 `<span class="inline-block w-[2px] h-[1.1em] ...">▋</span>` 元素，
 * 通过 Tailwind animate-caret-blink 动画实现 600ms 间隔闪烁。
 *
 * @param {HTMLElement} container - Markdown 渲染容器（.md 元素）
 * @returns {void}
 *
 * @example
 * // 流式 token 到达时
 * showCaret(assistantEl.querySelector('.md'));
 */
export function showCaret(container) {
  if (!container) return;
  // 幂等：已有光标时不重复插入
  if (container.querySelector('[data-stream-caret]')) return;

  const caret = document.createElement('span');
  caret.className = CARET_CLASS;
  caret.setAttribute('data-stream-caret', '');
  caret.setAttribute('aria-hidden', 'true');
  caret.textContent = '▋';
  container.appendChild(caret);
}

/**
 * 从指定容器移除流式光标。
 *
 * 如果容器中没有光标，则安全无操作（幂等）。
 *
 * @param {HTMLElement} container - Markdown 渲染容器
 * @returns {void}
 *
 * @example
 * // 流式结束后
 * removeCaret(assistantEl.querySelector('.md'));
 */
export function removeCaret(container) {
  if (!container) return;
  const caret = container.querySelector('[data-stream-caret]');
  if (caret) caret.remove();
}

/**
 * 检查指定容器中是否活跃着流式光标。
 *
 * @param {HTMLElement} container - Markdown 渲染容器
 * @returns {boolean} true=光标存在；false=不存在
 *
 * @example
 * if (isCaretActive(mdEl)) { /* 仍在流式中 *\/ }
 */
export function isCaretActive(container) {
  if (!container) return false;
  return container.querySelector('[data-stream-caret]') !== null;
}
