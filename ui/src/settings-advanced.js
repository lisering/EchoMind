/**
 * EchoMind 设置面板 — 高级设置模块（从 settings.js 拆分）。
 *
 * 职责：
 * 1. 嵌入模型选择器（REQ-VEC-012）
 * 2. 分块参数配置（REQ-VEC-011）
 * 3. 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）
 * 4. 文件监听 + 增量同步（REQ-SYNC-001~003）
 * 5. 对话上下文长度管理（REQ-RAG-017）
 * 6. Token 用量追踪与预算
 * 7. 采样参数配置（S11）
 * 8. 多代理协调模式 toggle（REQ-RAG-025）
 * 9. 自定义快捷指令模板管理（S56）
 * 10. 窗口管理设置（REQ-WIN-003）
 * 11. 错误日志导出（REQ-ERR-005）
 * 12. 开机自启 + 应用更新检查（REQ-WIN-004 + REQ-HELP-004）
 * 13. RAG/LLM 参数设置（REQ-RAG-014/015 v1.10）
 */

import { setState, get } from './state.js';
import { $, formatBytes } from './utils.js';
import { invoke, embedModelApi, syncApi, localLlmApi, windowSettingsApi, errorLogsApi, ragParamsApi, generationParamsApi, chunkParamsApi, autostartApi, updateCheckApi, saveDialog, openDialog } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { t } from './i18n.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { resetCustomTemplates, loadCustomTemplates } from './slash-commands.js';

// ============================================================
// 嵌入模型选择器（REQ-VEC-012）
// ============================================================

export async function onEmbeddingModelChange() {
  const select = $('embeddingModelSelect');
  if (!select) return;
  const newModel = select.value;
  try {
    await embedModelApi.setModel(newModel);
    const warning = $('embeddingModelWarning');
    if (warning) {
      warning.classList.remove('hidden');
    }
    toast(t('settings.embedding_model_switched', { model: newModel }), 'success');
    // loadModelCacheInfo 在 settings.js 中导入
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 嵌入模型下载镜像源选择器（REQ-VEC-017 v2.3）
// ============================================================

/**
 * 初始化镜像源选择器。
 * 从后端读取当前配置，填充到下拉框。切换时调用 `set_mirror_source` IPC。
 */
export async function initMirrorSourceSelector() {
  const select = $('mirrorSourceSelect');
  if (!select) return;
  try {
    const { mirrorSourceApi } = await import('./ipc.js');
    const current = await mirrorSourceApi.get();
    select.value = current || 'auto';
    select.onchange = async () => {
      const source = select.value;
      try {
        await mirrorSourceApi.set(source);
        toast(t('settings.mirror_source_switched', { source }), 'success');
      } catch (err) {
        toastError(err);
      }
    };
  } catch (err) {
    // E2E mock 环境可能无此 IPC，静默降级
  }
}

// ============================================================
// 分块参数配置（REQ-VEC-011）
// ============================================================

export async function initChunkParams() {
  const sizeSlider = $('chunkSizeSlider');
  const overlapSlider = $('chunkOverlapSlider');
  if (!sizeSlider || !overlapSlider) return;

  try {
    const params = await chunkParamsApi.get();
    sizeSlider.value = String(params.chunk_size);
    overlapSlider.value = String(params.overlap);
    _updateChunkParamsDisplay();
  } catch (_) {}

  sizeSlider.addEventListener('input', _updateChunkParamsDisplay);
  overlapSlider.addEventListener('input', _updateChunkParamsDisplay);
  sizeSlider.addEventListener('change', _saveChunkParams);
  overlapSlider.addEventListener('change', _saveChunkParams);
}

function _updateChunkParamsDisplay() {
  const sizeSlider = $('chunkSizeSlider');
  const overlapSlider = $('chunkOverlapSlider');
  const sizeValue = $('chunkSizeValue');
  const overlapValue = $('chunkOverlapValue');
  if (sizeSlider && sizeValue) sizeValue.textContent = sizeSlider.value;
  if (overlapSlider && overlapValue) overlapValue.textContent = overlapSlider.value;
}

async function _saveChunkParams() {
  const sizeSlider = $('chunkSizeSlider');
  const overlapSlider = $('chunkOverlapSlider');
  if (!sizeSlider || !overlapSlider) return;
  const chunkSize = parseInt(sizeSlider.value, 10);
  const overlap = parseInt(overlapSlider.value, 10);
  try {
    await chunkParamsApi.set(chunkSize, overlap);
    toast(t('settings.chunk_params_saved'), 'success');
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 自定义 ONNX 嵌入模型上传（REQ-VEC-014，Pro 门控）
// ============================================================

export async function loadCustomModels() {
  const container = $('customModelList');
  if (!container) return;
  try {
    const models = await embedModelApi.listCustomModels();
    if (models.length === 0) {
      container.innerHTML = `<p class="text-text-secondary text-sm py-2">${t('settings.custom_model_empty')}</p>`;
      return;
    }
    container.innerHTML = models
      .map(
        (m) => `
      <div class="flex items-center justify-between gap-2 py-2 px-3 rounded-lg bg-surface-2 border border-border-default" data-custom-model="${m.name}">
        <div class="flex-1 min-w-0">
          <span class="text-text-primary text-sm font-medium truncate">${m.name}</span>
          ${m.is_valid ? '' : `<span class="ml-2 text-xs text-warning">${t('settings.custom_model_invalid')}</span>`}
          <span class="ml-2 text-xs text-text-secondary">${formatBytes(m.size_bytes)}</span>
        </div>
        <div class="flex gap-1">
          <button class="btn-icon text-xs px-2 py-1 rounded text-accent hover:bg-surface-3" onclick="switchToCustomModel('${m.name}')" title="${t('settings.custom_model_switch')}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4"/></svg>
          </button>
          <button class="btn-icon text-xs px-2 py-1 rounded text-error hover:bg-surface-3" onclick="deleteCustomModel('${m.name}')" title="${t('settings.custom_model_delete')}">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
          </button>
        </div>
      </div>`
      )
      .join('');
  } catch (err) {
    const msg = String(err);
    if (msg.includes('PRO_REQUIRED')) {
      container.innerHTML = `<p class="text-text-secondary text-sm py-2">${t('settings.custom_model_pro_required')}</p>`;
    } else {
      container.innerHTML = `<p class="text-error text-sm py-2">${msg}</p>`;
    }
  }
}

export async function onUploadCustomModel() {
  try {
    const onnxPath = await window.__TAURI__.dialog.open({
      multiple: false,
      title: t('settings.custom_model_select_onnx'),
      filters: [{ name: 'ONNX Model', extensions: ['onnx'] }],
    });
    if (!onnxPath) return;

    const tokenizerPaths = await window.__TAURI__.dialog.open({
      multiple: true,
      title: t('settings.custom_model_select_tokenizer'),
      filters: [{ name: 'Tokenizer Files', extensions: ['json', 'txt'] }],
    });
    if (!tokenizerPaths || (Array.isArray(tokenizerPaths) && tokenizerPaths.length === 0)) return;

    const defaultName = typeof onnxPath === 'string'
      ? onnxPath.split('/').pop()?.replace('.onnx', '') || 'custom-model'
      : 'custom-model';
    const name = await window.__TAURI__.dialog
      ? prompt(t('settings.custom_model_name'), defaultName)
      : defaultName;
    if (!name || !name.trim()) return;

    const tokenizerFiles = Array.isArray(tokenizerPaths) ? tokenizerPaths : [tokenizerPaths];
    const info = await embedModelApi.uploadCustomModel(
      name.trim(),
      typeof onnxPath === 'string' ? onnxPath : onnxPath[0],
      tokenizerFiles
    );
    toast(t('settings.custom_model_upload_success', { name: info.name }), 'success');
    await loadCustomModels();
  } catch (err) {
    const msg = String(err);
    if (msg.includes('PRO_REQUIRED')) {
      toast(t('settings.custom_model_pro_required'), 'warning');
    } else {
      toast(t('settings.custom_model_upload_failed', { error: msg }), 'error');
    }
  }
}

export async function switchToCustomModel(name) {
  try {
    await embedModelApi.setModel(`custom:${name}`);
    toast(t('settings.custom_model_switched', { name }), 'success');
  } catch (err) {
    toastError(err);
  }
}

export async function deleteCustomModel(name) {
  const confirmed = confirm(t('settings.custom_model_delete_confirm', { name }));
  if (!confirmed) return;
  try {
    await embedModelApi.deleteCustomModel(name);
    toast(t('settings.custom_model_deleted', { name }), 'success');
    await loadCustomModels();
  } catch (err) {
    toastError(err);
  }
}

// ============================================================
// 文件监听 + 增量同步（REQ-SYNC-001~003）
// ============================================================

export async function onAddWatchedFolder() {
  try {
    const selected = await window.__TAURI__.dialog.open({
      directory: true,
      multiple: false,
      title: t('settings.sync_select_folder'),
    });
    if (!selected) return;
    const folderPath = typeof selected === 'string' ? selected : selected[0];
    if (!folderPath) return;

    toast(t('settings.sync_starting'), 'info');
    const result = await syncApi.add(folderPath);
    toast(
      t('settings.sync_complete', {
        added: result.added,
        updated: result.updated,
        deleted: result.deleted,
      }),
      'success'
    );
    await loadWatchedFolders();
  } catch (err) {
    toastError(err);
  }
}

export async function onRemoveWatchedFolder(folderPath) {
  try {
    await syncApi.remove(folderPath);
    toast(t('settings.sync_removed'), 'success');
    await loadWatchedFolders();
  } catch (err) {
    toastError(err);
  }
}

export async function loadWatchedFolders() {
  const container = $('syncFolderList');
  if (!container) return;

  try {
    const folders = await syncApi.list();
    if (folders.length === 0) {
      container.innerHTML = `<p class="text-sm text-text-quaternary py-2">${t('settings.sync_empty')}</p>`;
      return;
    }

    container.innerHTML = folders
      .map(
        (f) => `
      <div class="flex items-center justify-between gap-2 py-2 px-3 rounded-lg bg-slate-800/50">
        <div class="flex-1 min-w-0">
          <p class="text-sm text-slate-200 truncate">${f.name}</p>
          <p class="text-xs text-text-quaternary truncate">${f.path}</p>
          <p class="text-xs ${f.sync_status === 'idle' ? 'text-emerald-400' : 'text-text-quaternary'}">
            ${f.sync_status === 'idle' ? t('settings.sync_status_idle') : t('settings.sync_status_stopped')}
            ${f.last_synced_at ? ' · ' + new Date(f.last_synced_at * 1000).toLocaleString() : ''}
          </p>
        </div>
        <button class="shrink-0 px-2 py-1 text-xs rounded-lg bg-red-500/15 text-red-300 border border-red-400/40 hover:bg-red-500/25 transition-colors"
          onclick="onRemoveWatchedFolder('${f.path.replace(/'/g, "\\'")}')"
        >${t('settings.sync_remove')}</button>
      </div>`
      )
      .join('');
  } catch (err) {
    container.innerHTML = `<p class="text-sm text-red-400 py-2">${t('settings.sync_load_error')}</p>`;
  }
}

// ============================================================
// 对话上下文长度管理（REQ-RAG-017）
// ============================================================

export function loadContextTokenLimit(settings) {
  const slider = $('contextTokenSlider');
  const valueEl = $('contextTokenValue');
  if (!slider || !valueEl) return;

  const limit = settings.context_token_limit || 4096;
  slider.value = limit;
  valueEl.textContent = t('settings.context_token_value', { value: limit });

  slider.oninput = () => {
    valueEl.textContent = t('settings.context_token_value', { value: slider.value });
  };
  slider.onchange = async () => {
    const limit = parseInt(slider.value, 10);
    if (limit < 2048 || limit > 32768) return;
    try {
      await invoke('update_setting', { key: 'rag.context_token_limit', value: String(limit) });
      toastSuccess(t('settings.context_token_saved'));
    } catch (err) {
      toastError(err);
    }
  };
}

// ============================================================
// Token 用量追踪与预算
// ============================================================

export function loadTokenCostSettings(settings) {
  const budgetInput = $('tokenBudgetInput');
  if (budgetInput) {
    budgetInput.value = settings.token_budget || 0;
    let saveTimer = null;
    budgetInput.oninput = () => {
      if (saveTimer) clearTimeout(saveTimer);
      saveTimer = setTimeout(async () => {
        const budget = parseInt(budgetInput.value, 10) || 0;
        if (budget < 0) return;
        try {
          await invoke('set_token_budget', { budget });
          toastSuccess(t('settings.cost_budget_saved'));
          await refreshCostUsage();
        } catch (err) {
          toastError(err);
        }
      }, 600);
    };
  }

  refreshCostUsage();
}

async function refreshCostUsage() {
  const promptEl = $('costPromptTokens');
  const completionEl = $('costCompletionTokens');
  const totalEl = $('costTotalTokens');
  const exchangesEl = $('costExchanges');
  const budgetBar = $('costBudgetBar');
  const budgetFill = $('costBudgetFill');
  const budgetUsed = $('costBudgetUsed');
  const budgetLimit = $('costBudgetLimit');

  if (!promptEl) return;

  try {
    const cost = await invoke('get_conversation_cost');
    promptEl.textContent = formatNumber(cost.total_prompt_tokens);
    completionEl.textContent = formatNumber(cost.total_completion_tokens);
    totalEl.textContent = formatNumber(cost.total_tokens);
    if (exchangesEl) {
      exchangesEl.textContent = t('settings.cost_exchanges', { count: cost.exchange_count });
    }

    if (cost.token_budget > 0) {
      if (budgetBar) budgetBar.classList.remove('hidden');
      const pct = Math.min(100, (cost.total_tokens / cost.token_budget) * 100);
      if (budgetFill) {
        budgetFill.style.width = pct + '%';
        budgetFill.classList.toggle('bg-red-500', pct >= 100);
        budgetFill.classList.toggle('bg-accent', pct < 100);
      }
      if (budgetUsed) budgetUsed.textContent = formatNumber(cost.total_tokens);
      if (budgetLimit) budgetLimit.textContent = formatNumber(cost.token_budget);
    } else {
      if (budgetBar) budgetBar.classList.add('hidden');
    }
  } catch (err) {
    // 忽略错误
  }
}

function formatNumber(n) {
  if (n >= 1000000) {
    return (n / 1000000).toFixed(1) + 'M';
  } else if (n >= 1000) {
    return (n / 1000).toFixed(1) + 'K';
  }
  return String(n);
}

// ============================================================
// 采样参数配置（S11）
// ============================================================

let _samplingParams = {
  temperature: null,
  top_p: null,
  top_k: null,
  max_tokens: null,
  frequency_penalty: null,
  presence_penalty: null,
};

export async function loadSamplingParams(settings) {
  const params = settings.llm_sampling;
  if (params) {
    _samplingParams = { ..._samplingParams, ...params };
  }

  const tempInput = $('samplingTemperature');
  if (tempInput) tempInput.value = params?.temperature ?? '';

  const topPInput = $('samplingTopP');
  if (topPInput) topPInput.value = params?.top_p ?? '';

  const topKInput = $('samplingTopK');
  if (topKInput) topKInput.value = params?.top_k ?? '';

  const maxTokensInput = $('samplingMaxTokens');
  if (maxTokensInput) maxTokensInput.value = params?.max_tokens ?? '';

  const freqPenaltyInput = $('samplingFreqPenalty');
  if (freqPenaltyInput) freqPenaltyInput.value = params?.frequency_penalty ?? '';

  const presPenaltyInput = $('samplingPresPenalty');
  if (presPenaltyInput) presPenaltyInput.value = params?.presence_penalty ?? '';
}

export async function saveSamplingParams() {
  const params = collectSamplingParams();
  try {
    await localLlmApi.setSamplingParams(params);
    _samplingParams = params;
    toastSuccess(t('settings.llm_sampling_saved'));
  } catch (err) {
    toastError(err);
  }
}

export async function resetSamplingParams() {
  const tempInput = $('samplingTemperature');
  if (tempInput) tempInput.value = '';
  const topPInput = $('samplingTopP');
  if (topPInput) topPInput.value = '';
  const topKInput = $('samplingTopK');
  if (topKInput) topKInput.value = '';
  const maxTokensInput = $('samplingMaxTokens');
  if (maxTokensInput) maxTokensInput.value = '';
  const freqPenaltyInput = $('samplingFreqPenalty');
  if (freqPenaltyInput) freqPenaltyInput.value = '';
  const presPenaltyInput = $('samplingPresPenalty');
  if (presPenaltyInput) presPenaltyInput.value = '';

  const params = {
    temperature: null,
    top_p: null,
    top_k: null,
    max_tokens: null,
    frequency_penalty: null,
    presence_penalty: null,
  };
  try {
    await localLlmApi.setSamplingParams(params);
    _samplingParams = params;
    toastSuccess(t('settings.llm_sampling_reset_done'));
  } catch (err) {
    toastError(err);
  }
}

function collectSamplingParams() {
  const parse = (id) => {
    const el = $(id);
    if (!el || el.value.trim() === '') return null;
    return parseFloat(el.value);
  };
  const parseIntVal = (id) => {
    const el = $(id);
    if (!el || el.value.trim() === '') return null;
    return parseInt(el.value, 10);
  };

  return {
    temperature: parse('samplingTemperature'),
    top_p: parse('samplingTopP'),
    top_k: parseIntVal('samplingTopK'),
    max_tokens: parseIntVal('samplingMaxTokens'),
    frequency_penalty: parse('samplingFreqPenalty'),
    presence_penalty: parse('samplingPresPenalty'),
  };
}

// ============================================================
// 多代理协调模式 toggle（REQ-RAG-025）
// ============================================================

export async function onCoordinatorToggle() {
  const toggle = $('coordinatorToggle');
  const enabled = toggle.getAttribute('aria-checked') === 'true';
  const newEnabled = !enabled;
  toggle.setAttribute('aria-checked', String(newEnabled));
  toggle.classList.toggle('bg-accent', newEnabled);
  toggle.classList.toggle('bg-slate-600', !newEnabled);
  toggle.querySelector('span').classList.toggle('translate-x-5', newEnabled);
  try {
    await invoke('update_setting', { key: 'rag.coordinator_enabled', value: String(newEnabled) });
    toast(newEnabled ? t('settings.coordinator_enabled') : t('settings.coordinator_disabled'), 'success');
  } catch (err) {
    toastError(err);
    toggle.setAttribute('aria-checked', String(enabled));
    toggle.classList.toggle('bg-accent', enabled);
    toggle.classList.toggle('bg-slate-600', !enabled);
    toggle.querySelector('span').classList.toggle('translate-x-5', enabled);
  }
}

export async function onSubAgentToggle() {
  const toggle = $('subAgentToggle');
  if (!toggle) return;
  const currentEnabled = get('subAgentEnabled');
  const newEnabled = !currentEnabled;

  setState({ subAgentEnabled: newEnabled });

  toggle.setAttribute('aria-checked', String(newEnabled));
  toggle.classList.toggle('bg-accent', newEnabled);
  toggle.classList.toggle('bg-slate-600', !newEnabled);
  toggle.querySelector('span').classList.toggle('translate-x-5', newEnabled);

  try {
    await invoke('update_setting', { key: 'rag.sub_agent_enabled', value: String(newEnabled) });
    toast(newEnabled ? t('settings.sub_agent_enabled') : t('settings.sub_agent_disabled'), 'success');
  } catch (err) {
    toastError(err);
    setState({ subAgentEnabled: currentEnabled });
    toggle.setAttribute('aria-checked', String(currentEnabled));
    toggle.classList.toggle('bg-accent', currentEnabled);
    toggle.classList.toggle('bg-slate-600', !currentEnabled);
    toggle.querySelector('span').classList.toggle('translate-x-5', currentEnabled);
  }
}

// ============================================================
// 自定义快捷指令模板管理（S56）
// ============================================================

export async function renderPromptTemplateSettings(container) {
  const header = document.createElement('div');
  header.className = 'flex items-center justify-between mb-3';

  const title = document.createElement('h4');
  title.className = 'side-label text-[11px] uppercase tracking-wider text-text-quaternary';
  title.textContent = t('settings.prompt_templates') || '快捷指令模板';
  header.appendChild(title);

  const btnGroup = document.createElement('div');
  btnGroup.className = 'flex items-center gap-1';

  // S88: 导入按钮
  const importBtn = document.createElement('button');
  importBtn.className = 'text-xs px-2 py-1 rounded-md border border-border-default text-text-secondary hover:bg-surface-3 transition-colors';
  importBtn.textContent = t('settings.template_import') || '导入';
  importBtn.onclick = () => importTemplates(container);
  btnGroup.appendChild(importBtn);

  // S88: 导出全部按钮
  const exportAllBtn = document.createElement('button');
  exportAllBtn.className = 'text-xs px-2 py-1 rounded-md border border-border-default text-text-secondary hover:bg-surface-3 transition-colors';
  exportAllBtn.textContent = t('settings.template_export_all') || '导出全部';
  exportAllBtn.onclick = () => exportAllTemplates(container);
  btnGroup.appendChild(exportAllBtn);

  // 新增按钮
  const addBtn = document.createElement('button');
  addBtn.className = 'text-xs px-2 py-1 rounded-md bg-accent text-ink hover:opacity-90 transition-opacity';
  addBtn.textContent = t('settings.template_add') || '+ 新增';
  addBtn.onclick = () => showTemplateForm(container, null);
  btnGroup.appendChild(addBtn);

  header.appendChild(btnGroup);
  container.appendChild(header);

  try {
    const templates = await invoke('list_prompt_templates');
    if (!templates || templates.length === 0) {
      const empty = document.createElement('p');
      empty.className = 'text-xs text-text-quaternary';
      empty.textContent = t('settings.template_empty') || '暂无自定义模板，点击「新增」创建';
      container.appendChild(empty);
    } else {
      const list = document.createElement('div');
      list.className = 'space-y-2';

      templates.forEach((tmpl) => {
        const item = document.createElement('div');
        item.className = 'flex items-center gap-2 p-2 rounded-lg bg-surface-2 border border-border-subtle';

        const icon = document.createElement('span');
        icon.className = 'text-lg shrink-0';
        icon.textContent = tmpl.icon || '⚡';

        const info = document.createElement('div');
        info.className = 'flex-1 min-w-0';

        const name = document.createElement('div');
        name.className = 'text-sm text-text-primary font-medium truncate';
        name.textContent = `/${tmpl.name}`;

        const desc = document.createElement('div');
        desc.className = 'text-xs text-text-tertiary truncate';
        desc.textContent = tmpl.description || tmpl.label;

        info.appendChild(name);
        info.appendChild(desc);

        const editBtn = document.createElement('button');
        editBtn.className = 'text-xs px-2 py-1 rounded text-text-secondary hover:text-text-primary hover:bg-surface-3 transition-colors shrink-0';
        editBtn.textContent = t('settings.template_edit') || '编辑';
        editBtn.onclick = () => showTemplateForm(container, tmpl);

        // S88: 导出单条模板按钮
        const exportBtn = document.createElement('button');
        exportBtn.className = 'text-xs px-2 py-1 rounded text-text-secondary hover:text-text-primary hover:bg-surface-3 transition-colors shrink-0';
        exportBtn.textContent = t('settings.template_export_single') || '导出';
        exportBtn.setAttribute('data-export-template', tmpl.id);
        exportBtn.onclick = () => exportSingleTemplate(container, tmpl);

        const delBtn = document.createElement('button');
        delBtn.className = 'text-xs px-2 py-1 rounded text-red-400 hover:text-red-300 hover:bg-surface-3 transition-colors shrink-0';
        delBtn.textContent = t('settings.template_delete') || '删除';
        delBtn.onclick = async () => {
          const confirmed = await showConfirmDialog({
            body: t('settings.template_delete_confirm') || '确定删除此模板？',
            title: t('settings.template_delete_title') || '删除模板',
          });
          if (!confirmed) return;
          try {
            await invoke('delete_prompt_template', { templateId: tmpl.id });
            toastSuccess(t('settings.template_deleted') || '模板已删除');
            resetCustomTemplates();
            await loadCustomTemplates();
            await refreshTemplateList(container);
          } catch (err) {
            toastError(err);
          }
        };

        item.appendChild(icon);
        item.appendChild(info);
        item.appendChild(editBtn);
        item.appendChild(exportBtn);
        item.appendChild(delBtn);
        list.appendChild(item);
      });

      container.appendChild(list);
    }
  } catch (err) {
    const errorP = document.createElement('p');
    errorP.className = 'text-xs text-red-400';
    errorP.textContent = t('settings.template_load_error') || '加载模板失败';
    container.appendChild(errorP);
  }
}

// ============================================================
// S88: 对话模板导入/导出（REQ-RAG-054）
// ============================================================

/**
 * 导出全部自定义模板为 JSON 文件。
 *
 * 调用 `list_prompt_templates` 获取全部模板 → 组装 JSON →
 * `saveDialog` 选择保存路径 → `save_text_file` 写入。
 *
 * @param {HTMLElement} container - 模板区域容器（用于刷新）
 */
export async function exportAllTemplates(container) {
  try {
    const templates = await invoke('list_prompt_templates');
    if (!templates || templates.length === 0) {
      toast(t('settings.template_empty') || '暂无自定义模板可导出', 'info');
      return;
    }

    const exportData = {
      version: '1.0',
      exported_at: new Date().toISOString(),
      templates: templates.map((tmpl) => ({
        name: tmpl.name,
        label: tmpl.label,
        description: tmpl.description || '',
        icon: tmpl.icon || '⚡',
        prompt_template: tmpl.prompt_template,
      })),
    };

    const jsonStr = JSON.stringify(exportData, null, 2);
    const defaultName = `echomind-templates-${new Date().toISOString().slice(0, 10)}.json`;

    const savePath = await saveDialog({
      defaultPath: defaultName,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (!savePath) return; // 用户取消

    await invoke('save_text_file', { path: savePath, content: jsonStr });
    toastSuccess(t('settings.template_exported') || '模板已导出');
  } catch (err) {
    toastError(err);
  }
}

/**
 * 导出单条模板为 JSON 文件。
 *
 * @param {HTMLElement} container - 模板区域容器
 * @param {Object} template - 要导出的模板对象
 */
export async function exportSingleTemplate(container, template) {
  try {
    const exportData = {
      version: '1.0',
      exported_at: new Date().toISOString(),
      templates: [{
        name: template.name,
        label: template.label,
        description: template.description || '',
        icon: template.icon || '⚡',
        prompt_template: template.prompt_template,
      }],
    };

    const jsonStr = JSON.stringify(exportData, null, 2);
    const defaultName = `echomind-template-${template.name}.json`;

    const savePath = await saveDialog({
      defaultPath: defaultName,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (!savePath) return; // 用户取消

    await invoke('save_text_file', { path: savePath, content: jsonStr });
    toastSuccess(t('settings.template_exported') || '模板已导出');
  } catch (err) {
    toastError(err);
  }
}

/**
 * 从 JSON 文件导入模板。
 *
 * `openDialog` 选择 JSON 文件 → `read_text_file` 读取 →
 * JSON.parse → 遍历 templates[] 调用 `save_prompt_template` →
 * 名冲突时自动追加 `_2`/`_3` 后缀。
 *
 * @param {HTMLElement} container - 模板区域容器（用于刷新）
 */
export async function importTemplates(container) {
  try {
    const filePath = await openDialog({
      multiple: false,
      title: t('settings.template_import') || '导入模板',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (!filePath) return; // 用户取消

    const path = typeof filePath === 'string' ? filePath : filePath[0];
    if (!path) return;

    const content = await invoke('read_text_file', { path });
    if (!content) {
      toast(t('settings.template_import') + ': ' + (t('settings.template_empty') || '文件为空'), 'error');
      return;
    }

    const data = JSON.parse(content);
    if (!data.templates || !Array.isArray(data.templates) || data.templates.length === 0) {
      toast(t('settings.template_empty') || '文件中无模板数据', 'error');
      return;
    }

    // 获取已有模板名列表用于冲突检测
    const existingTemplates = await invoke('list_prompt_templates');
    const existingNames = new Set((existingTemplates || []).map((tmpl) => tmpl.name));

    let importedCount = 0;
    let conflictCount = 0;

    for (const tmpl of data.templates) {
      if (!tmpl.name || !tmpl.label || !tmpl.prompt_template) {
        continue; // 跳过无效模板
      }

      // 名冲突检测：追加 _2、_3… 后缀
      let resolvedName = tmpl.name;
      let suffix = 2;
      while (existingNames.has(resolvedName)) {
        resolvedName = `${tmpl.name}_${suffix}`;
        suffix++;
      }

      if (resolvedName !== tmpl.name) {
        conflictCount++;
      }

      try {
        await invoke('save_prompt_template', {
          name: resolvedName,
          label: tmpl.label,
          description: tmpl.description || '',
          icon: tmpl.icon || '⚡',
          promptTemplate: tmpl.prompt_template,
        });
        existingNames.add(resolvedName);
        importedCount++;
      } catch (err) {
        // 单条导入失败不影响其他模板
        const errMsg = String(err);
        if (errMsg.includes('VALIDATION')) {
          // 验证失败（如系统指令名冲突或缺少 {query} 占位符），跳过
          continue;
        }
        throw err;
      }
    }

    // 刷新模板列表
    resetCustomTemplates();
    await loadCustomTemplates();
    await refreshTemplateList(container);

    if (conflictCount > 0) {
      toastSuccess(
        t('settings.template_imported', { count: importedCount }) ||
        `成功导入 ${importedCount} 个模板（${conflictCount} 个名称冲突已自动重命名）`
      );
    } else {
      toastSuccess(
        t('settings.template_imported', { count: importedCount }) ||
        `成功导入 ${importedCount} 个模板`
      );
    }
  } catch (err) {
    toastError(err);
  }
}

async function refreshTemplateList(container) {
  const form = container.querySelector('.template-form');
  container.innerHTML = '';
  await renderPromptTemplateSettings(container);
  if (form) container.appendChild(form);
}

function showTemplateForm(container, existing) {
  const oldForm = container.querySelector('.template-form');
  if (oldForm) oldForm.remove();

  const form = document.createElement('div');
  form.className = 'template-form mt-3 p-3 rounded-lg bg-surface-2 border border-border-default space-y-2';

  const nameRow = document.createElement('div');
  nameRow.className = 'flex flex-col gap-1';
  const nameLabel = document.createElement('label');
  nameLabel.className = 'text-xs text-text-tertiary';
  nameLabel.textContent = t('settings.template_name') || '指令名称（小写字母/数字/下划线）';
  const nameInput = document.createElement('input');
  nameInput.type = 'text';
  nameInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent';
  nameInput.placeholder = 'my_commands';
  nameInput.value = existing?.name || '';
  if (existing) nameInput.disabled = true;
  nameRow.appendChild(nameLabel);
  nameRow.appendChild(nameInput);
  form.appendChild(nameRow);

  const labelRow = document.createElement('div');
  labelRow.className = 'flex flex-col gap-1';
  const labelLabel = document.createElement('label');
  labelLabel.className = 'text-xs text-text-tertiary';
  labelLabel.textContent = t('settings.template_label') || '显示标签';
  const labelInput = document.createElement('input');
  labelInput.type = 'text';
  labelInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent';
  labelInput.placeholder = t('settings.template_label_placeholder') || '我的指令';
  labelInput.value = existing?.label || '';
  labelRow.appendChild(labelLabel);
  labelRow.appendChild(labelInput);
  form.appendChild(labelRow);

  const descRow = document.createElement('div');
  descRow.className = 'flex flex-col gap-1';
  const descLabel = document.createElement('label');
  descLabel.className = 'text-xs text-text-tertiary';
  descLabel.textContent = t('settings.template_description') || '描述';
  const descInput = document.createElement('input');
  descInput.type = 'text';
  descInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent';
  descInput.placeholder = t('settings.template_description_placeholder') || '指令用途说明';
  descInput.value = existing?.description || '';
  descRow.appendChild(descLabel);
  descRow.appendChild(descInput);
  form.appendChild(descRow);

  const iconRow = document.createElement('div');
  iconRow.className = 'flex flex-col gap-1';
  const iconLabel = document.createElement('label');
  iconLabel.className = 'text-xs text-text-tertiary';
  iconLabel.textContent = t('settings.template_icon') || '图标（emoji）';
  const iconInput = document.createElement('input');
  iconInput.type = 'text';
  iconInput.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent w-20';
  iconInput.placeholder = '⚡';
  iconInput.value = existing?.icon || '⚡';
  iconRow.appendChild(iconLabel);
  iconRow.appendChild(iconInput);
  form.appendChild(iconRow);

  const tmplRow = document.createElement('div');
  tmplRow.className = 'flex flex-col gap-1';
  const tmplLabel = document.createElement('label');
  tmplLabel.className = 'text-xs text-text-tertiary';
  tmplLabel.textContent = t('settings.template_content') || 'Prompt 模板（使用 {query} 作为占位符）';
  const tmplTextarea = document.createElement('textarea');
  tmplTextarea.className = 'bg-surface-3 border border-border-default rounded-lg px-3 py-2 text-sm text-text-primary outline-none focus:border-accent resize-y min-h-[80px]';
  tmplTextarea.placeholder = '请执行以下操作：{query}';
  tmplTextarea.value = existing?.prompt_template || '';
  tmplRow.appendChild(tmplLabel);
  tmplRow.appendChild(tmplTextarea);
  form.appendChild(tmplRow);

  const btnRow = document.createElement('div');
  btnRow.className = 'flex gap-2 justify-end';

  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'text-xs px-3 py-1.5 rounded-lg border border-border-default text-text-secondary hover:bg-surface-3 transition-colors';
  cancelBtn.textContent = t('settings.template_cancel') || '取消';
  cancelBtn.onclick = () => form.remove();

  const saveBtn = document.createElement('button');
  saveBtn.className = 'text-xs px-3 py-1.5 rounded-lg bg-accent text-ink hover:opacity-90 transition-opacity';
  saveBtn.textContent = existing
    ? (t('settings.template_update') || '更新')
    : (t('settings.template_create') || '创建');
  saveBtn.onclick = async () => {
    const name = nameInput.value.trim();
    const label = labelInput.value.trim();
    const description = descInput.value.trim();
    const icon = iconInput.value.trim() || '⚡';
    const promptTemplate = tmplTextarea.value.trim();

    if (!name || !label || !promptTemplate) {
      toast(t('settings.template_required') || '名称、标签和模板内容不能为空', 'error');
      return;
    }

    if (!promptTemplate.includes('{query}')) {
      toast(t('settings.template_query_required') || '模板内容必须包含 {query} 占位符', 'error');
      return;
    }

    try {
      await invoke('save_prompt_template', {
        name, label, description, icon, promptTemplate,
      });
      toastSuccess(existing
        ? (t('settings.template_updated') || '模板已更新')
        : (t('settings.template_created') || '模板已创建'));
      resetCustomTemplates();
      await loadCustomTemplates();
      form.remove();
      await refreshTemplateList(container);
    } catch (err) {
      toastError(err);
    }
  };

  btnRow.appendChild(cancelBtn);
  btnRow.appendChild(saveBtn);
  form.appendChild(btnRow);

  container.appendChild(form);
  nameInput.focus();
}

// ============================================================
// 窗口管理设置（REQ-WIN-003 — close-to-tray）
// ============================================================

export async function renderWindowSettings(container) {
  container.innerHTML = '';

  const title = document.createElement('h4');
  title.className = 'side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-3';
  title.textContent = t('window_settings.section_title', 'Window Management');
  container.appendChild(title);

  const row = document.createElement('div');
  row.className = 'flex items-center justify-between p-3 rounded-lg bg-surface-2 border border-border-subtle';

  const labelGroup = document.createElement('div');
  labelGroup.className = 'flex-1 min-w-0';

  const label = document.createElement('div');
  label.className = 'text-sm text-text-primary';
  label.textContent = t('window_settings.close_to_tray', 'Minimize to tray on close');
  labelGroup.appendChild(label);

  const desc = document.createElement('div');
  desc.className = 'text-xs text-text-quaternary mt-0.5';
  desc.textContent = t('window_settings.close_to_tray_desc', 'When enabled, closing the window hides it instead of quitting');
  labelGroup.appendChild(desc);

  const toggle = document.createElement('button');
  toggle.id = 'closeToTrayToggle';
  toggle.className = 'relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0';
  toggle.setAttribute('role', 'switch');
  toggle.setAttribute('aria-checked', 'false');

  const knob = document.createElement('span');
  knob.className = 'inline-block h-4 w-4 transform rounded-full bg-white transition-transform translate-x-1';
  toggle.appendChild(knob);

  let enabled = false;
  try {
    enabled = await windowSettingsApi.getCloseToTray();
  } catch (_) {}

  if (enabled) {
    toggle.classList.remove('bg-slate-600');
    toggle.classList.add('bg-accent');
    toggle.setAttribute('aria-checked', 'true');
    knob.classList.add('translate-x-5');
    knob.classList.remove('translate-x-1');
  } else {
    toggle.classList.add('bg-slate-600');
    toggle.classList.remove('bg-accent');
    toggle.setAttribute('aria-checked', 'false');
    knob.classList.add('translate-x-1');
    knob.classList.remove('translate-x-5');
  }

  toggle.onclick = async () => {
    const newEnabled = !enabled;
    try {
      await windowSettingsApi.setCloseToTray(newEnabled);
      enabled = newEnabled;
      if (newEnabled) {
        toggle.classList.remove('bg-slate-600');
        toggle.classList.add('bg-accent');
        toggle.setAttribute('aria-checked', 'true');
        knob.classList.add('translate-x-5');
        knob.classList.remove('translate-x-1');
        toastSuccess(t('window_settings.close_to_tray_on', 'Minimize to tray enabled'));
      } else {
        toggle.classList.add('bg-slate-600');
        toggle.classList.remove('bg-accent');
        toggle.setAttribute('aria-checked', 'false');
        knob.classList.add('translate-x-1');
        knob.classList.remove('translate-x-5');
        toastSuccess(t('window_settings.close_to_tray_off', 'Minimize to tray disabled'));
      }
    } catch (err) {
      toastError(err);
    }
  };

  row.appendChild(labelGroup);
  row.appendChild(toggle);
  container.appendChild(row);
}

// ============================================================
// 错误日志导出（REQ-ERR-005）
// ============================================================

export function renderErrorLogsSettings(container) {
  container.innerHTML = '';

  const title = document.createElement('h4');
  title.className = 'side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-3';
  title.textContent = t('error_logs.title', 'Error Logs');
  container.appendChild(title);

  const row = document.createElement('div');
  row.className = 'flex items-center justify-between p-3 rounded-lg bg-surface-2 border border-border-subtle';

  const label = document.createElement('div');
  label.className = 'text-sm text-text-secondary';
  label.textContent = t('error_logs.export', 'Export Error Logs');
  row.appendChild(label);

  const btn = document.createElement('button');
  btn.className = 'text-xs px-3 py-1.5 rounded-lg bg-accent text-ink hover:opacity-90 transition-opacity';
  btn.textContent = t('error_logs.export', 'Export Error Logs');
  btn.onclick = async () => {
    try {
      toast(t('error_logs.exporting', 'Exporting error logs…'), 'info');
      const logs = await errorLogsApi.export();

      if (!logs || logs.trim().length === 0) {
        toast(t('error_logs.empty', 'No error logs found'), 'info');
        return;
      }

      const filename = 'echomind-error-logs-' + new Date().toISOString().slice(0, 10) + '.log';
      await invoke('save_text_file', { content: logs, filename });
      toastSuccess(t('error_logs.exported', 'Error logs exported'));
    } catch (err) {
      toastError(err);
    }
  };
  row.appendChild(btn);
  container.appendChild(row);
}

// ============================================================
// 开机自启 + 应用更新检查设置（REQ-WIN-004 + REQ-HELP-004）
// ============================================================

export async function renderStartupSettings(container) {
  container.innerHTML = '';

  const title = document.createElement('h4');
  title.className = 'side-label text-[11px] uppercase tracking-wider text-text-quaternary mb-3';
  title.textContent = t('startup_settings.section_title', 'Startup & Updates');
  container.appendChild(title);

  // --- 开机自启 toggle ---
  const autostartRow = document.createElement('div');
  autostartRow.className = 'flex items-center justify-between p-3 rounded-lg bg-surface-2 border border-border-subtle mb-2';

  const autostartLabel = document.createElement('div');
  autostartLabel.className = 'flex-1 min-w-0';
  const autostartLabelTitle = document.createElement('div');
  autostartLabelTitle.className = 'text-sm text-text-primary';
  autostartLabelTitle.textContent = t('startup_settings.autostart', 'Launch at startup');
  autostartLabel.appendChild(autostartLabelTitle);
  const autostartDesc = document.createElement('div');
  autostartDesc.className = 'text-xs text-text-quaternary mt-0.5';
  autostartDesc.textContent = t('startup_settings.autostart_desc', 'Start EchoMind automatically when you log in');
  autostartLabel.appendChild(autostartDesc);

  const autostartToggle = document.createElement('button');
  autostartToggle.id = 'autostartToggle';
  autostartToggle.className = 'relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0';
  autostartToggle.setAttribute('role', 'switch');
  autostartToggle.setAttribute('aria-checked', 'false');
  const autostartKnob = document.createElement('span');
  autostartKnob.className = 'inline-block h-4 w-4 transform rounded-full bg-white transition-transform translate-x-1';
  autostartToggle.appendChild(autostartKnob);

  let autostartEnabled = false;
  try {
    autostartEnabled = await autostartApi.get();
  } catch (_) {}

  if (autostartEnabled) {
    autostartToggle.classList.add('bg-accent');
    autostartToggle.classList.remove('bg-slate-600');
    autostartToggle.setAttribute('aria-checked', 'true');
    autostartKnob.classList.add('translate-x-5');
    autostartKnob.classList.remove('translate-x-1');
  } else {
    autostartToggle.classList.add('bg-slate-600');
    autostartToggle.classList.remove('bg-accent');
    autostartToggle.setAttribute('aria-checked', 'false');
    autostartKnob.classList.add('translate-x-1');
    autostartKnob.classList.remove('translate-x-5');
  }

  autostartToggle.onclick = async () => {
    const newEnabled = !autostartEnabled;
    try {
      await autostartApi.set(newEnabled);
      autostartEnabled = newEnabled;
      if (newEnabled) {
        autostartToggle.classList.add('bg-accent');
        autostartToggle.classList.remove('bg-slate-600');
        autostartToggle.setAttribute('aria-checked', 'true');
        autostartKnob.classList.add('translate-x-5');
        autostartKnob.classList.remove('translate-x-1');
        toastSuccess(t('startup_settings.autostart_on', 'Launch at startup enabled'));
      } else {
        autostartToggle.classList.add('bg-slate-600');
        autostartToggle.classList.remove('bg-accent');
        autostartToggle.setAttribute('aria-checked', 'false');
        autostartKnob.classList.add('translate-x-1');
        autostartKnob.classList.remove('translate-x-5');
        toastSuccess(t('startup_settings.autostart_off', 'Launch at startup disabled'));
      }
    } catch (err) {
      toastError(err);
    }
  };

  autostartRow.appendChild(autostartLabel);
  autostartRow.appendChild(autostartToggle);
  container.appendChild(autostartRow);

  // --- 自动检查更新 toggle ---
  const updateRow = document.createElement('div');
  updateRow.className = 'flex items-center justify-between p-3 rounded-lg bg-surface-2 border border-border-subtle';

  const updateLabel = document.createElement('div');
  updateLabel.className = 'flex-1 min-w-0';
  const updateLabelTitle = document.createElement('div');
  updateLabelTitle.className = 'text-sm text-text-primary';
  updateLabelTitle.textContent = t('startup_settings.auto_update', 'Check for updates automatically');
  updateLabel.appendChild(updateLabelTitle);
  const updateDesc = document.createElement('div');
  updateDesc.className = 'text-xs text-text-quaternary mt-0.5';
  updateDesc.textContent = t('startup_settings.auto_update_desc', 'Check GitHub Releases for new versions every 24 hours');
  updateLabel.appendChild(updateDesc);

  const updateToggle = document.createElement('button');
  updateToggle.id = 'autoUpdateToggle';
  updateToggle.className = 'relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0';
  updateToggle.setAttribute('role', 'switch');
  updateToggle.setAttribute('aria-checked', 'false');
  const updateKnob = document.createElement('span');
  updateKnob.className = 'inline-block h-4 w-4 transform rounded-full bg-white transition-transform translate-x-1';
  updateToggle.appendChild(updateKnob);

  let updateEnabled = true;
  try {
    const config = await updateCheckApi.getConfig();
    updateEnabled = config.auto_check;
  } catch (_) {}

  if (updateEnabled) {
    updateToggle.classList.add('bg-accent');
    updateToggle.classList.remove('bg-slate-600');
    updateToggle.setAttribute('aria-checked', 'true');
    updateKnob.classList.add('translate-x-5');
    updateKnob.classList.remove('translate-x-1');
  } else {
    updateToggle.classList.add('bg-slate-600');
    updateToggle.classList.remove('bg-accent');
    updateToggle.setAttribute('aria-checked', 'false');
    updateKnob.classList.add('translate-x-1');
    updateKnob.classList.remove('translate-x-5');
  }

  updateToggle.onclick = async () => {
    const newEnabled = !updateEnabled;
    try {
      await updateCheckApi.setEnabled(newEnabled);
      updateEnabled = newEnabled;
      if (newEnabled) {
        updateToggle.classList.add('bg-accent');
        updateToggle.classList.remove('bg-slate-600');
        updateToggle.setAttribute('aria-checked', 'true');
        updateKnob.classList.add('translate-x-5');
        updateKnob.classList.remove('translate-x-1');
      } else {
        updateToggle.classList.add('bg-slate-600');
        updateToggle.classList.remove('bg-accent');
        updateToggle.setAttribute('aria-checked', 'false');
        updateKnob.classList.add('translate-x-1');
        updateKnob.classList.remove('translate-x-5');
      }
    } catch (err) {
      toastError(err);
    }
  };

  updateRow.appendChild(updateLabel);
  updateRow.appendChild(updateToggle);
  container.appendChild(updateRow);
}

// ============================================================
// RAG/LLM 参数设置（REQ-RAG-014/015 v1.10）
// ============================================================

export async function renderRagLlmParams(container) {
  container.innerHTML = '';

  let ragParams = null;
  let llmParams = null;
  try {
    [ragParams, llmParams] = await Promise.all([
      ragParamsApi.get(),
      generationParamsApi.get(),
    ]);
  } catch {
    container.innerHTML = '<p class="text-xs text-text-quaternary">' + t('params_load_error') + '</p>';
    return;
  }

  // --- RAG 检索参数区 ---
  const ragTitle = document.createElement('div');
  ragTitle.className = 'text-xs uppercase tracking-wider text-text-quaternary mb-2';
  ragTitle.textContent = t('rag_params_section');
  container.appendChild(ragTitle);

  const ragBox = document.createElement('div');
  ragBox.className = 'bg-surface-2 border border-border-default rounded-xl px-4 py-3 space-y-4';

  ragBox.appendChild(_makeSlider({
    id: 'ragTopKSlider',
    label: t('rag_params_top_k'),
    desc: t('rag_params_top_k_desc'),
    min: 1, max: 20, step: 1,
    value: ragParams.top_k,
    display: (v) => String(v),
  }));

  ragBox.appendChild(_makeSlider({
    id: 'ragThresholdSlider',
    label: t('rag_params_threshold'),
    desc: t('rag_params_threshold_desc'),
    min: 0, max: 1, step: 0.05,
    value: ragParams.score_threshold,
    display: (v) => v.toFixed(2),
  }));

  const expansionToggle = _makeToggle(
    t('rag_params_expansion'),
    t('rag_params_expansion_desc'),
    ragParams.chunk_expansion_enabled,
  );
  expansionToggle.querySelector('button').id = 'ragExpansionToggle';
  ragBox.appendChild(expansionToggle);

  ragBox.appendChild(_makeSlider({
    id: 'ragExpansionWindowSlider',
    label: t('rag_params_expansion_window'),
    desc: t('rag_params_expansion_window_desc'),
    min: 0, max: 3, step: 1,
    value: ragParams.chunk_expansion_window,
    display: (v) => String(v),
  }));

  container.appendChild(ragBox);

  // --- LLM 生成参数区 ---
  const llmTitle = document.createElement('div');
  llmTitle.className = 'text-xs uppercase tracking-wider text-text-quaternary mb-2 mt-5';
  llmTitle.textContent = t('llm_params_section');
  container.appendChild(llmTitle);

  const llmBox = document.createElement('div');
  llmBox.className = 'bg-surface-2 border border-border-default rounded-xl px-4 py-3 space-y-4';

  llmBox.appendChild(_makeSlider({
    id: 'llmTemperatureSlider',
    label: t('llm_params_temperature'),
    desc: t('llm_params_temperature_desc'),
    min: 0, max: 2, step: 0.1,
    value: llmParams.temperature,
    display: (v) => v.toFixed(1),
  }));

  llmBox.appendChild(_makeNumberInput({
    id: 'llmMaxTokensInput',
    label: t('llm_params_max_tokens'),
    desc: t('llm_params_max_tokens_desc'),
    min: 256, max: 8192, step: 64,
    value: llmParams.max_tokens,
  }));

  llmBox.appendChild(_makeSlider({
    id: 'llmTopPSlider',
    label: t('llm_params_top_p'),
    desc: t('llm_params_top_p_desc'),
    min: 0, max: 1, step: 0.05,
    value: llmParams.top_p,
    display: (v) => v.toFixed(2),
  }));

  container.appendChild(llmBox);

  // --- 保存按钮 ---
  const saveBtn = document.createElement('button');
  saveBtn.className = 'w-full mt-3 border border-accent/40 rounded-lg px-3 py-2 text-sm text-accent hover:bg-accent/10 transition-colors';
  saveBtn.textContent = t('settings.params_save');
  saveBtn.id = 'ragLlmParamsSaveBtn';
  saveBtn.onclick = async () => {
    try {
      const topK = parseInt(document.getElementById('ragTopKSlider')?.value || '8', 10);
      const threshold = parseFloat(document.getElementById('ragThresholdSlider')?.value || '0');
      const expansionEnabled = document.getElementById('ragExpansionToggle')?.getAttribute('aria-checked') === 'true';
      const expansionWindow = parseInt(document.getElementById('ragExpansionWindowSlider')?.value || '1', 10);
      const temperature = parseFloat(document.getElementById('llmTemperatureSlider')?.value || '0.7');
      const maxTokens = parseInt(document.getElementById('llmMaxTokensInput')?.value || '4096', 10);
      const topP = parseFloat(document.getElementById('llmTopPSlider')?.value || '1.0');

      await ragParamsApi.set({
        top_k: topK,
        score_threshold: threshold,
        chunk_expansion_enabled: expansionEnabled,
        chunk_expansion_window: expansionWindow,
      });
      await generationParamsApi.set({
        temperature,
        max_tokens: maxTokens,
        top_p: topP,
      });
      toastSuccess(t('params_save_success'));
    } catch (err) {
      toastError(t('params_save_error') + ': ' + String(err));
    }
  };
  container.appendChild(saveBtn);
}

// ============================================================
// 控件工厂函数
// ============================================================

function _makeSlider({ id, label, desc, min, max, step, value, display }) {
  const wrapper = document.createElement('div');
  wrapper.className = 'space-y-1';

  const header = document.createElement('div');
  header.className = 'flex items-center justify-between';

  const labelEl = document.createElement('span');
  labelEl.className = 'text-sm text-text-secondary';
  labelEl.textContent = label;
  header.appendChild(labelEl);

  const valueEl = document.createElement('span');
  valueEl.className = 'text-xs text-text-quaternary font-mono';
  valueEl.textContent = display(value);
  valueEl.id = id + 'Value';
  header.appendChild(valueEl);

  wrapper.appendChild(header);

  const slider = document.createElement('input');
  slider.type = 'range';
  slider.id = id;
  slider.min = String(min);
  slider.max = String(max);
  slider.step = String(step);
  slider.value = String(value);
  slider.className = 'w-full accent-sky-400 cursor-pointer';
  slider.oninput = () => {
    valueEl.textContent = display(parseFloat(slider.value));
  };
  wrapper.appendChild(slider);

  if (desc) {
    const descEl = document.createElement('p');
    descEl.className = 'text-[11px] text-text-quaternary';
    descEl.textContent = desc;
    wrapper.appendChild(descEl);
  }

  return wrapper;
}

function _makeNumberInput({ id, label, desc, min, max, step, value }) {
  const wrapper = document.createElement('div');
  wrapper.className = 'space-y-1';

  const labelEl = document.createElement('span');
  labelEl.className = 'text-sm text-text-secondary';
  labelEl.textContent = label;
  wrapper.appendChild(labelEl);

  const input = document.createElement('input');
  input.type = 'number';
  input.id = id;
  input.min = String(min);
  input.max = String(max);
  input.step = String(step);
  input.value = String(value);
  input.className = 'w-full bg-surface-1 border border-border-default rounded-lg px-3 py-1.5 text-sm text-text-primary outline-none focus:border-accent transition-colors';
  wrapper.appendChild(input);

  if (desc) {
    const descEl = document.createElement('p');
    descEl.className = 'text-[11px] text-text-quaternary';
    descEl.textContent = desc;
    wrapper.appendChild(descEl);
  }

  return wrapper;
}

function _makeToggle(label, desc, checked) {
  const wrapper = document.createElement('div');
  wrapper.className = 'flex items-center justify-between';

  const left = document.createElement('div');
  left.className = 'space-y-0.5';

  const labelEl = document.createElement('span');
  labelEl.className = 'text-sm text-text-secondary';
  labelEl.textContent = label;
  left.appendChild(labelEl);

  if (desc) {
    const descEl = document.createElement('p');
    descEl.className = 'text-[11px] text-text-quaternary';
    descEl.textContent = desc;
    left.appendChild(descEl);
  }

  wrapper.appendChild(left);

  const toggle = document.createElement('button');
  toggle.className = 'relative inline-flex h-6 w-11 items-center rounded-full transition-colors shrink-0';
  toggle.setAttribute('role', 'switch');
  toggle.setAttribute('aria-checked', String(checked));

  const knob = document.createElement('span');
  knob.className = 'inline-block h-4 w-4 transform rounded-full bg-white transition-transform';
  if (checked) {
    toggle.classList.add('bg-accent');
    knob.classList.add('translate-x-5');
  } else {
    toggle.classList.add('bg-slate-600');
    knob.classList.add('translate-x-1');
  }
  toggle.appendChild(knob);

  toggle.onclick = () => {
    const newChecked = toggle.getAttribute('aria-checked') !== 'true';
    toggle.setAttribute('aria-checked', String(newChecked));
    if (newChecked) {
      toggle.classList.add('bg-accent');
      toggle.classList.remove('bg-slate-600');
      knob.classList.add('translate-x-5');
      knob.classList.remove('translate-x-1');
    } else {
      toggle.classList.add('bg-slate-600');
      toggle.classList.remove('bg-accent');
      knob.classList.add('translate-x-1');
      knob.classList.remove('translate-x-5');
    }
  };

  wrapper.appendChild(toggle);

  return wrapper;
}
