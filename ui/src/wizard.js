/**
 * EchoMind 3 步配置向导模块。
 *
 * 流程：向量模型下载 → 配置大模型 → 导入文档
 *
 * 职责：
 * 1. Step 1: 下载向量模型（镜像/断点续传/进度/重试）
 * 2. Step 2: 配置 LLM（预设卡片 + API Key + Base URL + Model）
 * 3. Step 3: 导入文档（拖拽 + 文件选择）
 * 4. 步骤指示器状态同步
 */

import { setState, get } from './state.js';
import { showApp, updateModelPill } from './chat-render.js';
import { $, PRESETS, WORKSPACE } from './utils.js';
import { invoke, openUrl, listen, openDialog, demoModeApi } from './ipc.js';
import { toast, toastError } from './toast.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';

/** 向导面板的 Focus Trap 实例（REQ-A11Y-002） */
let _wizardTrap = null;

/** 当前向导步骤（1/2/3） */
let _currentStep = 1;

/** 向导完成回调（由 main.js 传入） */
let _onComplete = null;

/** model_download_progress 事件取消监听器 */
let _unlistenDownload = null;

/** 标记是否已收到首个进度事件（用于区分"连接中"和"下载中"阶段） */
let _firstProgressReceived = false;

/** 标记是否已进入下一步（防重入：Loading 事件和 invoke 返回都可能触发） */
let _downloadAdvanced = false;

/** 自动重试计数器 */
let _retryCount = 0;

/** 最大自动重试次数 */
const MAX_AUTO_RETRIES = 3;

/** 已导入的文件列表（Step 3） */
let _importedFiles = [];

// ============================================================
// 步骤指示器
// ============================================================

/**
 * 更新步骤指示器UI（点+线状态）。
 * @param {number} step - 当前步骤（1/2/3）
 */
function updateStepIndicator(step) {
  _currentStep = step;
  for (let i = 1; i <= 3; i++) {
    const dot = $(`wizStepDot${i}`);
    if (!dot) continue;
    dot.classList.remove('active', 'completed');
    if (i < step) {
      dot.classList.add('completed');
    } else if (i === step) {
      dot.classList.add('active');
    }
  }
  // 更新连接线
  const lines = document.querySelectorAll('.wizard-step-line');
  lines.forEach((line, idx) => {
    line.classList.toggle('completed', idx < step - 1);
  });
  // 更新步骤标签
  const labelEl = $('wizStepLabel');
  if (labelEl) {
    labelEl.setAttribute('data-i18n', `wizard.step${step}_title`);
    labelEl.textContent = t(`wizard.step${step}_title`);
  }
}

/**
 * 显示指定步骤，隐藏其他步骤。
 * @param {number} step - 1=下载, 2=配置, 3=导入
 */
export function showWizardStep(step) {
  updateStepIndicator(step);
  for (let i = 1; i <= 3; i++) {
    const panel = $(`wizardStep${i}`);
    if (panel) {
      panel.classList.toggle('hidden', i !== step);
    }
  }
  // 步骤特定的初始化
  if (step === 1) {
    startDownload();
  } else if (step === 3) {
    _importedFiles = [];
    updateImportedList();
  }
}

// ============================================================
// Step 1: 向量模型下载
// ============================================================


/**
 * 启动向量模型下载。
 *
 * 调用 `init_embedder` IPC 命令，后端会：
 * 1. 检查模型文件是否已齐备 → 如已齐备，推送 Done 事件
 * 2. 否则下载缺失文件（支持断点续传），推送 Downloading 事件
 * 3. 下载完成后加载 ONNX 模型，推送 Loading → Done 事件
 *
 * 前端通过 `model_download_progress` 事件实时更新进度条。
 */
export async function startDownload() {
  // 重置标记（手动重试 = 全新开始）
  _firstProgressReceived = false;
  _downloadAdvanced = false;
  _retryCount = 0;
  await startDownloadInternal();
}

/**
 * 下载内部实现（自动重试时调用，不重置计数器）。
 */
async function startDownloadInternal() {
  const bar = $('wizDownloadBar');
  const pct = $('wizDownloadPct');
  const status = $('wizDownloadStatus');
  const errBox = $('wizDownloadError');
  const retryBox = $('wizDownloadRetry');
  const doneBox = $('wizDownloadDone');

  // 重置 UI（保留重试计数）
  _firstProgressReceived = false;

  if (bar) {
    bar.style.width = '';
    bar.classList.remove('progress-complete', 'progress-error');
    bar.classList.add('progress-indeterminate');
  }
  if (pct) pct.textContent = '';
  if (errBox) errBox.classList.add('hidden');
  if (retryBox) retryBox.classList.add('hidden');
  if (doneBox) doneBox.classList.add('hidden');
  if (status) {
    status.textContent = t('wizard.download_preparing');
    status.setAttribute('data-i18n', 'wizard.download_preparing');
  }

  // 清理旧的监听器
  if (_unlistenDownload) {
    _unlistenDownload();
    _unlistenDownload = null;
  }

  // 先发起 init_embedder（后端 spawn_blocking 开始 HTTP 连接）
  // 再注册监听器——两者并行，不串行等待
  const invokePromise = invoke('init_embedder').catch((err) => {
    if (!_downloadAdvanced) {
      onDownloadError(String(err));
    }
  });

  // 注册进度事件监听（与后端 HTTP 连接并行）
  try {
    _unlistenDownload = await listen('model_download_progress', (event) => {
      const p = event.payload;
      if (!_firstProgressReceived) {
        _firstProgressReceived = true;
        if (bar) {
          bar.classList.remove('progress-indeterminate');
          bar.style.transform = '';
        }
      }
      handleDownloadEvent(p);
    });
  } catch (_) {
    // 测试环境或事件系统不可用时静默降级
  }

  // 等待 invoke 完成（fire-and-forget，仅捕获错误）
  await invokePromise;
}

/**
 * 计算多文件下载的整体进度百分比。
 *
 * 后端 DownloadEvent::Downloading 携带 file_index / total_files / current / total。
 * 前端需要将其折算为整体进度，避免进度条在每个文件间 0→100 跳跃。
 *
 * - 当 total > 0 时：整体 = (file_index + current/total) / total_files * 100
 * - 当 total = 0 时（Content-Length 缺失）：基于已下载字节数估算进度，
 *   使用对数曲线让前期有可见反馈而非停留在 0%
 *
 * @param {object} downloading - downloading payload
 * @returns {number} 0-100 的整体百分比
 */
function calcOverallProgress(downloading) {
  const fileIndex = downloading.file_index || 0;
  const totalFiles = downloading.total_files || 1;
  const current = downloading.current || 0;
  const total = downloading.total || 0;

  if (total > 0) {
    // 有 Content-Length：精确计算
    const fileProgress = current / total; // 0.0 ~ 1.0
    return Math.min(100, Math.round(((fileIndex + fileProgress) / totalFiles) * 100));
  }

  // total=0（Content-Length 缺失）：使用对数曲线估算
  // 前期快速上升到 ~30%，然后缓慢推进，避免全程 0% 的观感
  if (current <= 0) return Math.round((fileIndex / totalFiles) * 100);
  const estimatedPct = Math.min(90, 30 + 15 * Math.log10(1 + current / 65536));
  return Math.min(95, Math.round(((fileIndex + estimatedPct / 100) / totalFiles) * 100));
}

/**
 * 处理下载进度事件。
 *
 * 支持两种序列化格式：
 * - 真实 Tauri serde: `"loading"` / `"done"` (字符串) / `{ "downloading": {...} }` / `{ "error": {...} }`
 * - E2E stub: `{ loading: true }` / `{ done: true }` / `{ downloading: {...} }` / `{ error: {...} }`
 *
 * @param {object|string} event - DownloadEvent payload
 */
function handleDownloadEvent(event) {
  // DownloadEvent::Downloading { current, total, file_index, total_files }
  const downloading = (typeof event === 'object' && event !== null)
    ? (event.downloading || event.Downloading)
    : null;
  if (downloading) {
    const pctVal = calcOverallProgress(downloading);
    const bar = $('wizDownloadBar');
    const pctEl = $('wizDownloadPct');
    const statusEl = $('wizDownloadStatus');

    if (bar) {
      bar.style.width = pctVal + '%';
      // total=0 时添加 indeterminate 动画类
      const isUnknown = !downloading.total || downloading.total <= 0;
      bar.classList.toggle('progress-indeterminate', isUnknown && pctVal < 95);
    }
    if (pctEl) pctEl.textContent = pctVal + '%';
    if (statusEl) {
      if (pctVal >= 100) {
        statusEl.textContent = t('wizard.download_complete_loading');
      } else {
        statusEl.textContent = t('wizard.download_progress');
      }
    }
    return;
  }

  // DownloadEvent::Loading = 下载完成，ONNX 正在后台加载
  // 立即进入下一步，不等 ONNX 加载完成
  if (event === 'loading' || (typeof event === 'object' && event !== null && (event.loading || event.Loading))) {
    onDownloadSuccess();
    return;
  }

  // DownloadEvent::Done = ONNX 加载也完成了（静默忽略，用户已进入下一步）
  if (event === 'done' || (typeof event === 'object' && event !== null && (event.done || event.Done !== undefined))) {
    return;
  }

  // DownloadEvent::Error { message }
  const errorData = (typeof event === 'object' && event !== null)
    ? (event.error || event.Error)
    : null;
  if (errorData) {
    const msg = errorData.message || 'Unknown error';
    onDownloadError(msg);
    return;
  }
}

/**
 * 下载成功回调：更新 UI，自动进入下一步。
 * 防重入：多处可能触发（Loading 事件 / invoke 返回），只执行一次。
 */
function onDownloadSuccess() {
  if (_downloadAdvanced) return;
  _downloadAdvanced = true;
  const bar = $('wizDownloadBar');
  const pct = $('wizDownloadPct');
  const status = $('wizDownloadStatus');
  const doneBox = $('wizDownloadDone');

  if (bar) {
    bar.style.width = '100%';
    bar.style.transform = '';
    bar.classList.remove('progress-indeterminate');
    bar.classList.add('progress-complete');
  }
  if (pct) pct.textContent = '100%';
  if (status) {
    status.textContent = t('wizard.download_complete');
    status.setAttribute('data-i18n', 'wizard.download_complete');
  }

  // 清理监听器
  if (_unlistenDownload) {
    _unlistenDownload();
    _unlistenDownload = null;
  }

  // 自动进入下一步（延迟 800ms 让用户看到完成状态）
  setTimeout(() => {
    showWizardStep(2);
  }, 800);
}

/**
 * 下载失败回调：自动重试（指数退避），超过最大次数后显示手动重试。
 * @param {string} message - 错误信息
 */
function onDownloadError(message) {
  // 已进入下一步的错误忽略
  if (_downloadAdvanced) return;

  _retryCount++;
  const status = $('wizDownloadStatus');
  const bar = $('wizDownloadBar');

  if (_retryCount <= MAX_AUTO_RETRIES) {
    // 自动重试：指数退避（2s → 4s → 8s）
    const delaySec = Math.pow(2, _retryCount);
    if (status) {
      status.textContent = t('wizard.download_retrying', { sec: delaySec, n: _retryCount, max: MAX_AUTO_RETRIES });
    }
    console.warn(`[Wizard] 下载失败（第 ${_retryCount} 次），${delaySec} 秒后自动重试：${message}`);
    setTimeout(() => {
      if (!_downloadAdvanced) startDownloadInternal();
    }, delaySec * 1000);
    return;
  }

  // 超过最大重试次数：显示错误 + 手动重试按钮
  if (bar) {
    bar.style.transform = '';
    bar.classList.remove('progress-indeterminate');
    bar.classList.add('progress-error');
  }
  if (status) {
    status.textContent = t('wizard.download_failed');
    status.setAttribute('data-i18n', 'wizard.download_failed');
  }
  const errBox = $('wizDownloadError');
  if (errBox) {
    errBox.textContent = message;
    errBox.classList.remove('hidden');
  }
  const retryBox = $('wizDownloadRetry');
  if (retryBox) {
    retryBox.classList.remove('hidden');
  }

  // 清理监听器
  if (_unlistenDownload) {
    _unlistenDownload();
    _unlistenDownload = null;
  }
}

// ============================================================
// Step 2: 配置大模型
// ============================================================

/**
 * 渲染 Provider 预设卡片（DeepSeek / OpenAI / Ollama），高亮当前选中项。
 */
export function renderPresetCards() {
  const box = $('presetCards');
  if (!box) return;
  box.innerHTML = '';
  for (const [key, p] of Object.entries(PRESETS)) {
    const btn = document.createElement('button');
    const active = key === get('activePreset');
    btn.className = `rounded-xl border px-4 py-4 text-left transition-colors ${active ? 'border-accent bg-accent/10' : 'border-border-default bg-surface-1 hover:border-slate-500'}`;
    btn.innerHTML = `<div class="text-sm font-medium ${active ? 'text-accent' : ''}">${p.label}</div><div class="mt-1 text-[11px] text-text-quaternary">${t(p.descKey)}</div>`;
    btn.onclick = () => { setState({ activePreset: key }); applyPreset(); renderPresetCards(); };
    box.appendChild(btn);
  }
}

/**
 * 将当前选中的预设值填入向导输入框，并切换 API Key 可选性提示。
 */
export function applyPreset() {
  const p = PRESETS[get('activePreset')];
  const urlEl = $('wizUrl');
  const modelEl = $('wizModel');
  if (urlEl) urlEl.value = p.base_url;
  if (modelEl) modelEl.value = p.model;
  const keyOpt = $('keyOptional');
  if (keyOpt) keyOpt.classList.toggle('hidden', p.needKey);
  const errBox = $('wizError');
  if (errBox) errBox.classList.add('hidden');
  // Hide "Get API Key" link for custom preset (no key URL)
  const keyLink = $('wizKeyLink');
  if (keyLink) keyLink.classList.toggle('hidden', !p.keyUrl);
}

/**
 * Step 2「验证并继续」流程：先 test_llm_connection 验证配置，成功后进入 Step 3。
 */
async function validateAndContinue() {
  const p = PRESETS[get('activePreset')];
  const api_key = $('wizKey').value.trim();
  const base_url = $('wizUrl').value.trim();
  const model = $('wizModel').value.trim();
  const errBox = $('wizError');
  if (errBox) errBox.classList.add('hidden');

  if (p.needKey && !api_key) { if (errBox) { errBox.textContent = t('wizard.error_no_key'); errBox.classList.remove('hidden'); } return; }
  if (!base_url || !model) { if (errBox) { errBox.textContent = t('wizard.error_no_url'); errBox.classList.remove('hidden'); } return; }

  const btn = $('wizStart');
  if (btn) { btn.disabled = true; btn.textContent = t('wizard.validating'); }
  try {
    await invoke('test_llm_connection', { apiKey: api_key, baseUrl: base_url, model });
  } catch (err) {
    if (errBox) { errBox.textContent = String(err); errBox.classList.remove('hidden'); }
    if (btn) { btn.disabled = false; btn.textContent = t('wizard.validate_and_continue'); }
    return;
  }
  try {
    await invoke('update_llm_config', { config: { api_key, base_url, model } });
  } catch (err) {
    if (errBox) { errBox.textContent = t('wizard.error_save_failed') + String(err); errBox.classList.remove('hidden'); }
    if (btn) { btn.disabled = false; btn.textContent = t('wizard.validate_and_continue'); }
    return;
  }
  if (btn) { btn.disabled = false; btn.textContent = t('wizard.validate_and_continue'); }
  setState({ currentModel: model, currentLlmMode: 'remote', llmConfigured: true, demoMode: false });
  updateModelPill();
  toast(t('wizard.llm_configured'), 'success');
  // REQ-RAG-051 AC-5: 配置 API Key 后自动退出演示模式
  if (window.exitDemoModeIfActive) {
    await window.exitDemoModeIfActive();
  }
  // 进入 Step 3
  showWizardStep(3);
}

// ============================================================
// Step 3: 导入文档
// ============================================================

/**
 * 处理文件选择/拖拽的文件列表，调用 import_files IPC。
 * @param {string[]} filePaths - 文件路径数组
 */
async function importWizardFiles(filePaths) {
  if (!filePaths || filePaths.length === 0) return;

  const progressEl = $('wizImportProgress');
  const barEl = $('wizImportBar');
  const textEl = $('wizImportText');
  const listEl = $('wizImportedList');

  if (progressEl) progressEl.classList.remove('hidden');
  if (barEl) barEl.style.width = '0%';
  if (textEl) textEl.textContent = t('wizard.importing');

  try {
    const result = await invoke('import_files', { paths: filePaths });
    // 显示导入结果
    if (listEl) {
      listEl.classList.remove('hidden');
      const items = (result || []).map(r => {
        const status = r.status === 'ok' ? '✓' : r.status === 'duplicate' ? '⊙' : '✗';
        const color = r.status === 'ok' ? 'text-green-400' : r.status === 'duplicate' ? 'text-text-quaternary' : 'text-red-400';
        return `<div class="flex items-center gap-2 text-xs"><span class="${color}">${status}</span><span class="text-text-secondary truncate">${r.name || r.path}</span></div>`;
      });
      listEl.innerHTML = items.join('');
    }
    _importedFiles = _importedFiles.concat(filePaths);
    if (barEl) barEl.style.width = '100%';
    if (textEl) textEl.textContent = t('wizard.import_complete');
  } catch (err) {
    if (textEl) textEl.textContent = t('wizard.import_failed') + ': ' + String(err);
  }
}

/**
 * 更新已导入文件列表 UI。
 */
function updateImportedList() {
  const listEl = $('wizImportedList');
  if (!listEl) return;
  if (_importedFiles.length === 0) {
    listEl.classList.add('hidden');
    listEl.innerHTML = '';
    return;
  }
  listEl.classList.remove('hidden');
  listEl.innerHTML = _importedFiles.map(f => {
    const name = f.split('/').pop() || f;
    return `<div class="flex items-center gap-2 text-xs"><span class="text-green-400">✓</span><span class="text-text-secondary truncate">${name}</span></div>`;
  }).join('');
}

/**
 * 完成向导：显示主界面。
 */
async function finishWizard() {
  // 确保清理下载监听器
  if (_unlistenDownload) {
    _unlistenDownload();
    _unlistenDownload = null;
  }
  showApp();
  updateModelPill();
  toast(t('wizard.welcome_to_echomind'), 'success');
  if (_onComplete) await _onComplete();
}

// ============================================================
// 初始化
// ============================================================

/**
 * 初始化向导事件绑定。
 * @param {() => Promise} [onComplete] - 向导完成后的回调（通常为 initConversations）
 */
export function initWizard(onComplete) {
  _onComplete = onComplete;

  // Step 1: 重试按钮
  const retryBtn = $('wizRetryBtn');
  if (retryBtn) retryBtn.onclick = () => startDownload();

  // Step 1: 下载完成后手动进入下一步
  const nextBtn = $('wizNextFromStep1');
  if (nextBtn) nextBtn.onclick = () => showWizardStep(2);

  // Step 2: 验证并继续
  const startBtn = $('wizStart');
  if (startBtn) startBtn.onclick = () => validateAndContinue();

  // Step 2: 跳过（不配置 LLM，直接进入 Step 3）
  const skipBtn = $('wizSkipStep2');
  if (skipBtn) skipBtn.onclick = () => showWizardStep(3);

  // REQ-RAG-051 AC-1: 跳过配置，使用演示模式
  const demoBtn = $('wizDemoMode');
  if (demoBtn) demoBtn.onclick = async () => {
    try {
      // 加载示例文档 + 设置演示模式标志
      await demoModeApi.loadDemoDocuments();
      toast(t('wizard.demo_mode_activated'), 'success');
      // 演示模式无需 LLM 配置，直接进入主界面
      setState({ demoMode: true, llmConfigured: true });
      if (_onComplete) await _onComplete();
      showApp();
      updateModelPill();
    } catch (err) {
      toastError(String(err));
    }
  };

  // Step 2: 获取 API Key 链接
  const keyLink = $('wizKeyLink');
  if (keyLink) keyLink.onclick = async () => {
    const url = PRESETS[get('activePreset')].keyUrl;
    try { await openUrl(url); }
    catch (_) { toast(t('wizard.manual_url') + url, 'info'); }
  };

  // Step 3: 文件选择按钮
  const pickBtn = $('wizPickFiles');
  if (pickBtn) pickBtn.onclick = async () => {
    try {
      const selected = await openDialog({
        multiple: true,
        filters: [{ name: 'Documents', extensions: ['md', 'txt', 'pdf', 'markdown', 'docx', 'html', 'htm', 'pptx', 'epub'] }],
      });
      if (selected && selected.length > 0) {
        await importWizardFiles(Array.isArray(selected) ? selected : [selected]);
      }
    } catch (_) { /* 用户取消选择 */ }
  };

  // Step 3: 拖拽区点击也打开文件选择
  const dropZone = $('wizDropZone');
  if (dropZone) {
    dropZone.onclick = async () => {
      try {
        const selected = await openDialog({
          multiple: true,
          filters: [{ name: 'Documents', extensions: ['md', 'txt', 'pdf', 'markdown', 'docx', 'html', 'htm', 'pptx', 'epub'] }],
        });
        if (selected && selected.length > 0) {
          await importWizardFiles(Array.isArray(selected) ? selected : [selected]);
        }
      } catch (_) { /* 用户取消选择 */ }
    };

    // 拖拽事件
    dropZone.addEventListener('dragover', (e) => {
      e.preventDefault();
      dropZone.classList.add('border-accent');
    });
    dropZone.addEventListener('dragleave', () => {
      dropZone.classList.remove('border-accent');
    });
    dropZone.addEventListener('drop', async (e) => {
      e.preventDefault();
      dropZone.classList.remove('border-accent');
      const files = Array.from(e.dataTransfer?.files || []);
      // Tauri 环境下拖拽获取的是文件路径
      const paths = files.map(f => f.path).filter(Boolean);
      if (paths.length > 0) {
        await importWizardFiles(paths);
      }
    });
  }

  // Step 3: 完成按钮
  const finishBtn = $('wizFinish');
  if (finishBtn) finishBtn.onclick = () => finishWizard();

  // 设置向导 Focus Trap（REQ-A11Y-002）
  _wizardTrap = createFocusTrap($('wizard'));
  if (!$('wizard').classList.contains('hidden')) {
    _wizardTrap.activate();
  }
  const observer = new MutationObserver(() => {
    const isVisible = !$('wizard').classList.contains('hidden');
    if (isVisible) {
      _wizardTrap.activate();
    } else {
      _wizardTrap.deactivate();
    }
  });
  observer.observe($('wizard'), { attributes: true, attributeFilter: ['class'] });
}

/**
 * 兼容旧接口：startWizard（现在由 validateAndContinue 替代）。
 * 保留导出以避免 main.js 中的 import 报错。
 */
export async function startWizard(onComplete) {
  // 旧接口：直接验证并继续
  _onComplete = onComplete;
  await validateAndContinue();
}

// ============================================================
// 付费墙（paywall.js 已合并到此）
// ============================================================

/**
 * EchoMind 付费墙模块 — 配额触顶 / PDF 付费门 / License 激活 / 停用。
 *
 * 职责：
 * 1. 显示付费墙 Modal（设置升级原因文案）
 * 2. 激活 Pro License（验证签名 → 刷新 UI）
 * 3. 停用 Pro 授权
 * 4. 更新侧栏授权状态文案
 */

import { setState as _setState, get as _get } from './state.js';
import { invoke as _invoke } from './ipc.js';
import { toast as _toast, toastError as _toastError, toastSuccess as _toastSuccess } from './toast.js';
import { t as _t2 } from './i18n.js';
import { createFocusTrap as _createFocusTrap } from './focus-trap.js';
import { pushPanel as _pushPanel, removePanel as _removePanel } from './panel-stack.js';
import { isComposingEvent as _isComposingEvent } from './input-utils.js';

/** 付费墙的 Focus Trap 实例（REQ-A11Y-002） */
let _paywallTrap = null;

/**
 * 显示付费墙 Modal，设置升级原因文案。
 * @param {string} reason - 触发付费墙的原因
 */
export function showPaywall(reason) {
  $('paywallReason').textContent = reason || _t2('paywall.reason_default');
  $('paywallError').classList.add('hidden');
  $('licenseInput').value = '';
  $('paywall').classList.remove('hidden');

  if (_paywallTrap) {
    _paywallTrap.deactivate();
  }
  _paywallTrap = _createFocusTrap($('paywall'));
  _paywallTrap.activate();

  _pushPanel({ id: 'paywall', close: hidePaywall, element: $('paywall'), label: 'Paywall' });
}

/** 隐藏付费墙 Modal。 */
export function hidePaywall() {
  _removePanel('paywall');
  if (_paywallTrap) {
    _paywallTrap.deactivate();
    _paywallTrap = null;
  }
  $('paywall').classList.add('hidden');
}

/**
 * 根据 isPro 全局状态刷新侧栏授权状态文案与颜色。
 */
export function updateProStatus() {
  const el = $('proStatus');
  const badge = $('proStatusBadge');
  if (!el || !badge) return;
  if (_get('isPro')) {
    el.textContent = _t2('sidebar.pro_badge_pro');
    badge.className = 'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium bg-accent/15 text-accent border border-accent/20';
    badge.querySelector('.badge-icon').textContent = '⭐';
  } else {
    el.textContent = _t2('sidebar.pro_badge_free');
    badge.className = 'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium bg-surface-3 text-text-tertiary border border-border-default';
    badge.querySelector('.badge-icon').innerHTML = '<svg class="icon-sm" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>';
  }
}

/**
 * 激活 Pro License。
 */
export async function activatePro() {
  const key = $('licenseInput').value.trim();
  if (!key) {
    $('paywallError').textContent = _t2('paywall.error_no_key');
    $('paywallError').classList.remove('hidden');
    return;
  }
  $('paywallActivate').disabled = true;
  $('paywallActivate').textContent = _t2('paywall.activating');
  try {
    const result = await _invoke('activate_pro', { licenseKey: key });
    if (result) {
      _setState({ isPro: true });
      updateProStatus();
      hidePaywall();
      _toastSuccess(_t2('paywall.activated'));
    }
  } catch (err) {
    $('paywallError').textContent = String(err);
    $('paywallError').classList.remove('hidden');
  }
  $('paywallActivate').disabled = false;
  $('paywallActivate').textContent = _t2('paywall.activate');
}

/**
 * 停用 Pro 授权（REQ-LIC-004）。
 */
export async function deactivatePro() {
  try {
    await _invoke('deactivate_pro');
    _setState({ isPro: false });
    updateProStatus();
    const settingsModal = $('settingsModal');
    if (settingsModal && !settingsModal.classList.contains('hidden')) {
      $('settingsLicenseInfo').innerHTML =
        '<span class="text-slate-400">' + _t2('settings.license_free') + '</span>';
    }
    _toast(_t2('paywall.deactivated'), 'info');
  } catch (err) {
    _toastError(err);
  }
}

/**
 * 初始化付费墙事件绑定。
 */
export function initPaywall() {
  $('paywallClose').onclick = hidePaywall;
  if ($('paywallCloseBtn')) $('paywallCloseBtn').onclick = hidePaywall;
  $('paywallActivate').onclick = activatePro;
  $('licenseInput').addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !_isComposingEvent(e)) activatePro();
  });
}
