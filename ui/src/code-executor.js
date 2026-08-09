/**
 * EchoMind 代码执行沙箱面板（REQ-RAG-032，Pro feature）。
 *
 * 职责：
 * 1. 提供代码执行 UI 面板（语言选择 + 代码编辑区 + stdin + 结果展示）
 * 2. 通过 IPC 调用 `execute_code_snippet` 执行代码
 * 3. 展示 stdout / stderr / exit_code / duration / timed_out
 * 4. Free 版本调用时显示 Pro 升级提示
 *
 * 安全限制（后端强制）：
 * - 超时 10s（CodeExecutorConfig::timeout_secs）
 * - 内存 64MB（CodeExecutorConfig::memory_limit_mb）
 * - 无网络访问（CodeExecutorConfig::allow_network = false）
 *
 * 设计参考：CodeForge 多语言执行器 + DeepSeek 风格 UI
 */

import { t } from './i18n.js';
import { invoke } from './ipc.js';
import { toastError } from './toast.js';
import { createFocusTrap } from './focus-trap.js';

/** @type {HTMLElement|null} 当前活动面板的 DOM 根节点 */
let _panel = null;

/** @type {{ activate: () => void, deactivate: () => void }|null} Focus Trap 实例 */
let _trap = null;

/**
 * 打开代码执行面板。
 *
 * 创建一个模态面板，包含语言选择、代码编辑区、stdin 输入、执行按钮和结果展示区。
 * 面板关闭时自动清理 DOM 和事件监听器。
 */
export function openCodeExecutorPanel() {
  // 如果面板已存在，聚焦到代码编辑区
  if (_panel) {
    const textarea = _panel.querySelector('[data-role="code-input"]');
    if (textarea) textarea.focus();
    return;
  }

  _panel = document.createElement('div');
  _panel.className = 'fixed inset-0 z-50 flex items-center justify-center bg-black/50';
  _panel.dataset.role = 'code-executor-overlay';

  _panel.innerHTML = `
    <div class="bg-surface-1 border border-border-default rounded-2xl shadow-2xl w-full max-w-3xl mx-4 flex flex-col max-h-[85vh]" data-role="code-executor-panel">
      <!-- 头部 -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-border-default">
        <h2 class="text-lg font-medium text-text-primary" data-role="ce-title">${t('code_executor.title')}</h2>
        <button class="text-text-tertiary hover:text-text-primary transition-colors p-1 rounded-lg" data-role="ce-close" aria-label="Close">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
        </button>
      </div>

      <!-- 内容区 -->
      <div class="flex-1 overflow-y-auto px-6 py-4 space-y-4">
        <!-- 语言选择 -->
        <div class="flex items-center gap-3">
          <label class="text-sm text-text-secondary shrink-0" data-role="ce-lang-label">${t('code_executor.language')}</label>
          <select class="bg-surface-2 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary focus:outline-none focus:ring-2 focus:ring-accent" data-role="ce-language">
            <option value="python">${t('code_executor.python')}</option>
            <option value="javascript">${t('code_executor.javascript')}</option>
          </select>
        </div>

        <!-- 代码编辑区 -->
        <div>
          <textarea
            class="w-full bg-surface-2 border border-border-default rounded-lg px-4 py-3 text-sm text-text-primary font-mono resize-none focus:outline-none focus:ring-2 focus:ring-accent"
            rows="8"
            placeholder="${t('code_executor.code_placeholder')}"
            data-role="code-input"
            spellcheck="false"
          ></textarea>
        </div>

        <!-- stdin 输入（可折叠） -->
        <details class="group">
          <summary class="cursor-pointer text-sm text-text-tertiary hover:text-text-secondary transition-colors flex items-center gap-1">
            <svg class="transition-transform group-open:rotate-90" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
            ${t('code_executor.stdin_label')}
          </summary>
          <textarea
            class="w-full mt-2 bg-surface-2 border border-border-default rounded-lg px-4 py-3 text-sm text-text-primary font-mono resize-none focus:outline-none focus:ring-2 focus:ring-accent"
            rows="3"
            placeholder="${t('code_executor.stdin_placeholder')}"
            data-role="ce-stdin"
            spellcheck="false"
          ></textarea>
        </details>

        <!-- 结果区域 -->
        <div class="hidden" data-role="ce-result-area">
          <div class="text-sm font-medium text-text-secondary mb-2">${t('code_executor.result')}</div>

          <!-- 状态行 -->
          <div class="flex items-center gap-4 mb-3 text-xs">
            <span data-role="ce-status-line"></span>
          </div>

          <!-- stdout -->
          <div class="mb-3">
            <div class="text-xs text-text-tertiary mb-1">${t('code_executor.stdout')}</div>
            <pre class="bg-surface-0 border border-border-default rounded-lg p-3 text-xs text-text-primary font-mono overflow-x-auto max-h-40 overflow-y-auto" data-role="ce-stdout"></pre>
          </div>

          <!-- stderr -->
          <div>
            <div class="text-xs text-text-tertiary mb-1">${t('code_executor.stderr')}</div>
            <pre class="bg-surface-0 border border-border-default rounded-lg p-3 text-xs text-red-400 font-mono overflow-x-auto max-h-40 overflow-y-auto" data-role="ce-stderr"></pre>
          </div>
        </div>
      </div>

      <!-- 底部按钮 -->
      <div class="flex items-center justify-end gap-3 px-6 py-4 border-t border-border-default">
        <button class="px-4 py-2 text-sm text-text-secondary hover:text-text-primary transition-colors rounded-lg" data-role="ce-cancel">${t('code_executor.run') === 'Run' ? 'Close' : '关闭'}</button>
        <button class="px-5 py-2 text-sm font-medium text-white bg-accent hover:bg-accent/90 rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed" data-role="ce-run">
          ${t('code_executor.run')}
        </button>
      </div>
    </div>
  `;

  document.body.appendChild(_panel);

  // 激活 Focus Trap
  const panelEl = _panel.querySelector('[data-role="code-executor-panel"]');
  // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
  _trap = createFocusTrap(panelEl);
  _trap.activate();

  // 事件绑定
  const closeBtn = _panel.querySelector('[data-role="ce-close"]');
  const cancelBtn = _panel.querySelector('[data-role="ce-cancel"]');
  const runBtn = _panel.querySelector('[data-role="ce-run"]');
  const codeInput = _panel.querySelector('[data-role="code-input"]');

  closeBtn.addEventListener('click', closeCodeExecutorPanel);
  cancelBtn.addEventListener('click', closeCodeExecutorPanel);
  _panel.addEventListener('click', (e) => {
    if (e.target === _panel) closeCodeExecutorPanel();
  });

  runBtn.addEventListener('click', handleRun);
  codeInput.focus();

  // 键盘快捷键：Ctrl/Cmd + Enter 执行
  codeInput.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault();
      handleRun();
    }
  });
}

/**
 * 关闭代码执行面板。
 */
export function closeCodeExecutorPanel() {
  if (_trap) {
    _trap.deactivate();
    _trap = null;
  }
  if (_panel) {
    _panel.remove();
    _panel = null;
  }
}

/**
 * 处理执行按钮点击。
 *
 * 从 UI 读取代码、语言、stdin，调用 IPC 执行，
 * 然后在结果区域展示输出。
 */
async function handleRun() {
  if (!_panel) return;

  const codeInput = _panel.querySelector('[data-role="code-input"]');
  const langSelect = _panel.querySelector('[data-role="ce-language"]');
  const stdinInput = _panel.querySelector('[data-role="ce-stdin"]');
  const runBtn = _panel.querySelector('[data-role="ce-run"]');
  const resultArea = _panel.querySelector('[data-role="ce-result-area"]');

  const code = codeInput.value.trim();
  if (!code) {
    toastError(t('code_executor.empty_code'));
    return;
  }

  const language = langSelect.value;
  const stdin = stdinInput.value.trim() || null;

  // 禁用按钮，显示执行中状态
  runBtn.disabled = true;
  const originalText = runBtn.textContent;
  runBtn.textContent = t('code_executor.running');

  try {
    const result = await invoke('execute_code_snippet', {
      code,
      language,
      stdin,
    });

    // 展示结果
    resultArea.classList.remove('hidden');

    const statusLine = _panel.querySelector('[data-role="ce-status-line"]');
    const stdoutEl = _panel.querySelector('[data-role="ce-stdout"]');
    const stderrEl = _panel.querySelector('[data-role="ce-stderr"]');

    // 状态行
    if (result.timed_out) {
      statusLine.innerHTML = `<span class="text-red-400">${t('code_executor.timed_out')}</span>`;
    } else {
      const exitColor = result.exit_code === 0 ? 'text-green-400' : 'text-red-400';
      statusLine.innerHTML = `<span class="${exitColor}">${t('code_executor.exit_code')}: ${result.exit_code}</span> · <span class="text-text-tertiary">${t('code_executor.duration')}: ${result.duration_ms}ms</span>`;
    }

    // stdout
    stdoutEl.textContent = result.stdout || t('code_executor.no_output');

    // stderr
    stderrEl.textContent = result.stderr || t('code_executor.no_output');
  } catch (err) {
    const errMsg = String(err);
    if (errMsg.includes('Pro') || errMsg.includes('pro')) {
      toastError(t('code_executor.pro_required'));
    } else {
      toastError(`${t('code_executor.execution_failed')}: ${errMsg}`);
    }
  } finally {
    runBtn.disabled = false;
    runBtn.textContent = originalText;
  }
}

/**
 * 初始化代码执行器（注册命令面板入口等）。
 *
 * 当前为空操作 — 面板按需创建，无需预初始化。
 * 保留此函数以备未来扩展（如注册命令面板快捷入口）。
 */
export function initCodeExecutor() {
  // 预留：未来可注册命令面板入口
}
