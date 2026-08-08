/**
 * EchoMind DAG 工作流可视化面板（REQ-RAG-030 前端 UI）。
 *
 * 功能：
 * 1. 全屏 overlay 面板（类似 graph-viewer.js 模式）
 * 2. 左侧：工作流模板列表 + 新建按钮
 * 3. 右侧：JSON 编辑器 + 运行按钮 + 结果面板
 * 4. 底部：节点状态时间线（运行后显示）
 * 5. workflow_progress 事件监听
 * 6. 删除工作流模板
 * 7. Focus Trap + ESC 关闭 + i18n
 *
 * 入口：设置面板新增「工作流」按钮 或 命令面板
 */

import { $ } from './utils.js';
import { invoke, listen } from './ipc.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { toast, toastError, toastSuccess } from './toast.js';

/** 工作流面板 Focus Trap 实例 */
let _workflowTrap = null;

/** workflow_progress 事件取消监听器 */
let _unlistenWorkflowProgress = null;

/**
 * 打开工作流面板。
 */
export async function openWorkflowPanel() {
  let overlay = $('workflowPanelOverlay');
  if (!overlay) {
    overlay = document.createElement('div');
    overlay.id = 'workflowPanelOverlay';
    overlay.className = 'hidden fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-start justify-center pt-[8vh]';
    overlay.setAttribute('role', 'dialog');
    overlay.setAttribute('aria-modal', 'true');
    overlay.innerHTML = `
      <div class="w-full max-w-3xl bg-surface-1 border border-border-strong rounded-lg shadow-modal scale-in overflow-hidden flex flex-col" style="max-height: 84vh">
        <div class="flex items-center justify-between px-5 h-12 border-b border-border-subtle shrink-0">
          <h2 class="text-sm font-semibold text-text-primary" data-i18n="workflow.title"></h2>
          <button id="workflowCloseBtn" class="text-text-quaternary hover:text-text-secondary transition-colors">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="flex flex-1 overflow-hidden">
          <!-- 左侧：模板列表 -->
          <div class="w-64 border-r border-border-subtle flex flex-col">
            <div class="p-3 border-b border-border-subtle">
              <button id="workflowNewBtn" class="w-full px-3 py-2 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent/90 transition-colors" data-i18n="workflow.new"></button>
            </div>
            <div id="workflowList" class="flex-1 overflow-y-auto p-2 space-y-1"></div>
          </div>
          <!-- 右侧：编辑器 + 结果 -->
          <div class="flex-1 flex flex-col overflow-hidden">
            <div class="p-3 border-b border-border-subtle">
              <p class="text-xs text-text-quaternary mb-2" data-i18n="workflow.json_editor"></p>
              <textarea id="workflowJsonEditor" class="w-full h-48 px-3 py-2 bg-surface-2 border border-border-default rounded-lg text-xs text-text-primary font-mono resize-none focus:outline-none focus:border-accent transition-colors" placeholder=""></textarea>
              <div class="flex items-center gap-2 mt-2">
                <button id="workflowSaveBtn" class="px-3 py-1.5 rounded-lg border border-border-default text-sm text-text-secondary hover:bg-surface-2 transition-colors" data-i18n="workflow.save"></button>
                <button id="workflowRunBtn" class="px-3 py-1.5 rounded-lg bg-accent text-white text-sm font-medium hover:bg-accent/90 transition-colors" data-i18n="workflow.run"></button>
              </div>
              <p class="text-[10px] text-text-quaternary mt-1" data-i18n="workflow.node_types_help"></p>
            </div>
            <div id="workflowResults" class="flex-1 overflow-y-auto p-3 space-y-2"></div>
          </div>
        </div>
      </div>
    `;
    document.body.appendChild(overlay);

    // 绑定事件
    $('workflowCloseBtn').onclick = closeWorkflowPanel;
    $('workflowNewBtn').onclick = newWorkflow;
    $('workflowSaveBtn').onclick = saveWorkflow;
    $('workflowRunBtn').onclick = runWorkflow;
  }

  // 更新 i18n
  overlay.querySelectorAll('[data-i18n]').forEach((el) => {
    el.textContent = t(el.dataset.i18n);
  });
  const editor = $('workflowJsonEditor');
  if (editor) editor.placeholder = t('workflow.json_placeholder');

  overlay.classList.remove('hidden');

  // 加载工作流列表
  await loadWorkflowList();

  // 激活 Focus Trap
  if (_workflowTrap) _workflowTrap.deactivate();
  _workflowTrap = createFocusTrap(overlay);
  _workflowTrap.activate();

  // 监听 workflow_progress 事件
  if (_unlistenWorkflowProgress) {
    _unlistenWorkflowProgress();
  }
  _unlistenWorkflowProgress = await listen('workflow_progress', (event) => {
    const data = event.payload || {};
    const resultsEl = $('workflowResults');
    if (!resultsEl) return;
    // 更新节点状态
    const nodeEl = resultsEl.querySelector(`[data-node-id="${data.node_id}"]`);
    if (nodeEl) {
      const statusEl = nodeEl.querySelector('.node-status');
      if (statusEl) {
        statusEl.textContent = data.status || '';
        statusEl.className = 'node-status text-[10px] ' + (data.status === 'Completed' ? 'text-emerald-400' : data.status === 'Running' ? 'text-accent' : 'text-text-quaternary');
      }
    }
  });
}

/**
 * 关闭工作流面板。
 */
export function closeWorkflowPanel() {
  const overlay = $('workflowPanelOverlay');
  if (overlay) overlay.classList.add('hidden');
  if (_workflowTrap) {
    _workflowTrap.deactivate();
    _workflowTrap = null;
  }
  if (_unlistenWorkflowProgress) {
    _unlistenWorkflowProgress();
    _unlistenWorkflowProgress = null;
  }
}

/**
 * 加载工作流模板列表。
 */
async function loadWorkflowList() {
  const container = $('workflowList');
  if (!container) return;

  try {
    const workflows = await invoke('list_workflows');
    if (!workflows || workflows.length === 0) {
      container.innerHTML = `<p class="text-xs text-text-quaternary text-center py-4">${t('workflow.empty')}</p>`;
      return;
    }

    container.innerHTML = workflows.map((wf) => `
      <div class="px-2.5 py-2 rounded-lg hover:bg-surface-2 cursor-pointer transition-colors group" data-wf-id="${wf.id}">
        <div class="flex items-center justify-between">
          <p class="text-sm text-text-primary truncate flex-1">${wf.name || 'Unnamed'}</p>
          <button class="wf-delete-btn opacity-0 group-hover:opacity-100 text-text-quaternary hover:text-red-300 transition-all text-[10px]" data-wf-id="${wf.id}">✕</button>
        </div>
        <p class="text-[10px] text-text-quaternary truncate">${wf.description || ''}</p>
      </div>
    `).join('');

    // 点击加载到编辑器
    container.querySelectorAll('[data-wf-id]').forEach((el) => {
      el.onclick = (e) => {
        if (e.target.classList.contains('wf-delete-btn')) return;
        const wf = workflows.find((w) => w.id === el.dataset.wfId);
        if (wf) {
          const editor = $('workflowJsonEditor');
          if (editor) {
            editor.value = JSON.stringify(wf, null, 2);
            editor.dataset.wfId = wf.id;
          }
        }
      };
    });

    // 删除按钮
    container.querySelectorAll('.wf-delete-btn').forEach((btn) => {
      btn.onclick = async (e) => {
        e.stopPropagation();
        const wfId = btn.dataset.wfId;
        try {
          await invoke('delete_workflow', { workflow_id: wfId });
          toastSuccess(t('workflow.deleted'));
          await loadWorkflowList();
        } catch (err) {
          toastError(err);
        }
      };
    });
  } catch (err) {
    container.innerHTML = `<p class="text-xs text-red-400 text-center py-4">${err}</p>`;
  }
}

/**
 * 新建工作流：填充编辑器为默认模板。
 */
function newWorkflow() {
  const editor = $('workflowJsonEditor');
  if (editor) {
    const defaultTemplate = {
      name: 'New Workflow',
      description: '',
      nodes: [
        { id: 'node_1', label: 'Retrieve', node_type: { kind: 'Retrieval', top_k: 5, retrieval_mode: 'hybrid' } },
        { id: 'node_2', label: 'Generate', node_type: { kind: 'Generation', system_prompt: 'Answer based on: {input}', model: null } },
        { id: 'node_3', label: 'Output', node_type: { kind: 'Output', format: 'text' } },
      ],
      edges: [
        { source: 'node_1', target: 'node_2', mapping: null },
        { source: 'node_2', target: 'node_3', mapping: null },
      ],
    };
    editor.value = JSON.stringify(defaultTemplate, null, 2);
    delete editor.dataset.wfId;
  }
}

/**
 * 保存工作流模板。
 */
async function saveWorkflow() {
  const editor = $('workflowJsonEditor');
  if (!editor || !editor.value.trim()) {
    toastError(t('workflow.invalid_json'));
    return;
  }

  try {
    const wf = JSON.parse(editor.value);
    if (!wf.name || !wf.nodes) {
      toastError(t('workflow.invalid_json'));
      return;
    }

    const wfId = await invoke('save_workflow_template', { workflow: wf });
    toastSuccess(t('workflow.saved'));
    await loadWorkflowList();

    // 保存后更新编辑器关联的 ID
    editor.dataset.wfId = wfId;
  } catch (err) {
    toastError(t('workflow.invalid_json') + ': ' + err);
  }
}

/**
 * 运行工作流。
 */
async function runWorkflow() {
  const editor = $('workflowJsonEditor');
  if (!editor || !editor.value.trim()) {
    toastError(t('workflow.invalid_json'));
    return;
  }

  const resultsEl = $('workflowResults');
  try {
    const wf = JSON.parse(editor.value);
    const wfId = editor.dataset.wfId;
    if (!wfId) {
      // 先保存再运行
      const newId = await invoke('save_workflow_template', { workflow: wf });
      editor.dataset.wfId = newId;
      await runWorkflowWithId(newId, wf, resultsEl);
    } else {
      await runWorkflowWithId(wfId, wf, resultsEl);
    }
  } catch (err) {
    toastError(err);
  }
}

/**
 * 使用工作流 ID 运行工作流。
 * @param {string} wfId - 工作流 ID
 * @param {Object} wf - 工作流定义
 * @param {HTMLElement} resultsEl - 结果容器
 */
async function runWorkflowWithId(wfId, wf, resultsEl) {
  // 渲染节点状态时间线
  if (resultsEl) {
    resultsEl.innerHTML = `
      <div class="space-y-1">
        ${wf.nodes.map(node => `
          <div class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-surface-2" data-node-id="${node.id}">
            <span class="text-xs text-text-secondary font-mono">${node.id}</span>
            <span class="text-xs text-text-quaternary">${node.label}</span>
            <span class="ml-auto node-status text-[10px] text-text-quaternary">Pending</span>
          </div>
        `).join('')}
      </div>
      <div id="workflowFinalOutput" class="mt-3"></div>
    `;
  }

  try {
    const result = await invoke('run_workflow', { workflow_id: wfId });
    // 渲染最终输出
    const finalEl = $('workflowFinalOutput');
    if (finalEl && result) {
      finalEl.innerHTML = `
        <div class="bg-surface-2 rounded-lg p-3 space-y-1">
          <p class="text-xs font-medium text-text-secondary">${t('workflow.final_output')}</p>
          <p class="text-sm text-text-primary whitespace-pre-wrap">${result.final_output || ''}</p>
          <p class="text-[10px] text-text-quaternary">${t('workflow.duration')}: ${result.duration_ms || 0}</p>
        </div>
      `;
    }
    toastSuccess(t('workflow.completed'));
  } catch (err) {
    toastError(err);
  }
}

/**
 * 初始化工作流面板事件（ESC 关闭）。
 */
export function initWorkflowPanel() {
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      const overlay = $('workflowPanelOverlay');
      if (overlay && !overlay.classList.contains('hidden')) {
        closeWorkflowPanel();
      }
    }
  });
}
