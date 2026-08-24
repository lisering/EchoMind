/**
 * EchoMind 安全防御 UI 模块 — 锁屏遮罩、加密设置、PII 检测、审计日志面板。
 *
 * 功能：
 * 1. 锁屏遮罩 — 当 securityState === 'locked' 时全屏遮罩，需输入密码解锁
 * 2. 数据库加密 — 在设置面板提供加密入口，输入密码后启用 SQLCipher AES-256
 * 3. 自动锁屏 — 可配置空闲超时（分钟），超时后自动锁定
 * 4. PII 检测 — 开关 PII 自动检测/脱敏，8 类个人身份信息
 * 5. 审计日志 — 查看安全操作日志，验证哈希链完整性
 * 6. 剪贴板清除 — 配置剪贴板自动清除超时
 *
 * 依赖：state.js, ipc.js, i18n.js, toast.js
 */

import { getState, setState, subscribe, isLocked, isEncrypted } from './state.js';
import { securityApi } from './ipc.js';
import { t } from './i18n.js';
import { toast as showToast } from './toast.js';
import { createFocusTrap } from './focus-trap.js';
import { Z_INDEX, zClass } from './panel-stack.js';
import { pushPanel, removePanel } from './panel-stack.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { icon } from './utils.js';
import { isComposingEvent } from './input-utils.js';

// ============================================================
// 锁屏遮罩
// ============================================================

let _lockOverlayEl = null;
let _lockPasswordInputEl = null;
let _lockErrorEl = null;
let _lockUnlockBtnEl = null;

/** 锁屏遮罩的 Focus Trap 实例（REQ-A11Y-002） */
let _lockTrap = null;

/**
 * 创建锁屏遮罩 DOM 元素（惰性创建，首次锁定时调用）。
 */
function ensureLockOverlay() {
  if (_lockOverlayEl) return;

  _lockOverlayEl = document.createElement('div');
  _lockOverlayEl.id = 'lockOverlay';
  _lockOverlayEl.className = `fixed inset-0 ${zClass(Z_INDEX.LOCK_OVERLAY)} flex items-center justify-center bg-[rgba(15,17,21,0.92)] backdrop-blur-[12px] opacity-0 pointer-events-none transition-opacity duration-300`;
  _lockOverlayEl.innerHTML = `
    <div class="flex flex-col items-center px-10 py-12 max-w-[400px] text-center text-text-secondary">
      <div class="text-info mb-6 opacity-90">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
        </svg>
      </div>
      <h2 class="text-xl font-semibold m-0 mb-2 text-text-primary" data-i18n="security.lock_title">EchoMind 已锁定</h2>
      <p class="text-sm text-text-tertiary m-0 mb-8 leading-normal" data-i18n="security.lock_subtitle">输入密码以解锁并访问你的知识库</p>
      <div class="w-full mb-3">
        <input type="password" id="lockPasswordInput" class="w-full px-4 py-3 text-[15px] border border-border-strong rounded-[10px] bg-bg-input text-text-primary outline-none transition-colors box-border focus:border-info focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
               data-i18n-placeholder="security.password_placeholder"
               placeholder="输入数据库密码…"
               autocomplete="off"/>
      </div>
      <p id="lockError" class="text-[13px] text-danger min-h-[20px] m-0 mb-4"></p>
      <button id="lockUnlockBtn" class="w-full py-3 text-[15px] font-semibold border-none rounded-[10px] bg-primary text-surface-0 cursor-pointer transition-colors hover:bg-primary-hover disabled:opacity-60 disabled:cursor-not-allowed" data-i18n="security.unlock">解锁</button>
      <p class="text-xs text-text-quaternary mt-6 leading-normal" data-i18n="security.lock_hint">密码仅用于本地数据库解密，不会发送到任何服务器</p>
    </div>
  `;
  document.body.appendChild(_lockOverlayEl);

  _lockPasswordInputEl = _lockOverlayEl.querySelector('#lockPasswordInput');
  _lockErrorEl = _lockOverlayEl.querySelector('#lockError');
  _lockUnlockBtnEl = _lockOverlayEl.querySelector('#lockUnlockBtn');

  // 解锁按钮点击
  _lockUnlockBtnEl.addEventListener('click', handleUnlock);

  // Enter 键提交
  _lockPasswordInputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !isComposingEvent(e)) {
      handleUnlock();
    }
  });
}

/**
 * 显示锁屏遮罩。
 */
function showLockOverlay() {
  ensureLockOverlay();
  _lockOverlayEl.classList.add('opacity-100', 'pointer-events-auto');
  // 激活 Focus Trap（REQ-A11Y-002）：Tab 键锁定在锁屏遮罩内
  if (_lockTrap) {
    _lockTrap.deactivate();
  }
  _lockTrap = createFocusTrap(_lockOverlayEl);
  _lockTrap.activate();
  // 注册到面板栈（锁屏遮罩不可被 ESC 关闭，但需追踪生命周期）
  pushPanel({ id: 'lock-overlay', close: () => {}, element: _lockOverlayEl, label: 'Lock Overlay' });
  // 清除上一次的错误信息
  if (_lockErrorEl) _lockErrorEl.textContent = '';
  if (_lockPasswordInputEl) _lockPasswordInputEl.value = '';
}

/**
 * 隐藏锁屏遮罩。
 */
function hideLockOverlay() {
  if (_lockOverlayEl) {
    _lockOverlayEl.classList.remove('opacity-100', 'pointer-events-auto');
  }
  // 停用 Focus Trap（恢复焦点到触发元素，REQ-A11Y-002）
  if (_lockTrap) {
    _lockTrap.deactivate();
    _lockTrap = null;
  }
}

/**
 * 处理解锁请求。
 */
async function handleUnlock() {
  const password = _lockPasswordInputEl?.value?.trim();
  if (!password) {
    if (_lockErrorEl) _lockErrorEl.textContent = t('security.error_no_password', '请输入密码');
    return;
  }

  _lockUnlockBtnEl.disabled = true;
  _lockUnlockBtnEl.textContent = t('security.unlocking', '解锁中…');

  try {
    const result = await securityApi.unlock(password);
    if (result.success) {
      setState({ securityState: 'encrypted_unlocked' });
      hideLockOverlay();
      showToast(t('security.unlocked', '已解锁'), 'success');
    } else {
      // 解锁失败（密码错误或暴力破解锁定）
      const errorMsg = result.message || t('security.unlock_failed', '解锁失败');
      const waitSecs = result.wait_seconds || 0;
      if (_lockErrorEl) {
        _lockErrorEl.textContent = waitSecs > 0
          ? `${errorMsg}（${t('security.please_wait', '请等待')} ${waitSecs}s）`
          : errorMsg;
      }
    }
  } catch (err) {
    if (_lockErrorEl) _lockErrorEl.textContent = String(err);
  } finally {
    _lockUnlockBtnEl.disabled = false;
    _lockUnlockBtnEl.textContent = t('security.unlock', '解锁');
  }
}

/**
 * 锁定应用（手动或自动）。
 */
export async function lockApp() {
  try {
    await securityApi.lock();
    setState({ securityState: 'locked' });
    showLockOverlay();
  } catch (err) {
    showToast(t('security.lock_failed', '锁定失败') + ': ' + String(err), 'error');
  }
}

// ============================================================
// 安全状态同步
// ============================================================

/**
 * 从后端同步安全状态到前端。
 * 在应用启动时调用。
 */
export async function syncSecurityStatus() {
  try {
    const status = await securityApi.getStatus();
    setState({
      securityState: status.state || 'unencrypted',
      piiDetectionEnabled: status.pii_detection_enabled || false,
      autoLockTimeout: status.auto_lock_timeout || 0,
    });

    // 如果已锁定，显示锁屏遮罩
    if (status.state === 'locked') {
      showLockOverlay();
    }
  } catch (err) {
    // 安全状态获取失败（可能后端未启用），静默降级
    console.warn('Security status sync failed:', err);
  }
}

/**
 * 监听 security-state-changed Tauri 事件，实时同步锁屏状态。
 */
export async function listenSecurityEvents() {
  try {
    const { listen } = await import('./ipc.js');
    await listen('security-state-changed', (event) => {
      const newState = event.payload?.state;
      if (newState) {
        setState({ securityState: newState });
        if (newState === 'locked') {
          showLockOverlay();
        } else if (newState === 'encrypted_unlocked') {
          hideLockOverlay();
        }
      }
    });
  } catch (_) {
    // 事件监听失败，静默降级
  }
}

/**
 * 记录用户活动（重置自动锁屏计时器）。
 * 在鼠标移动、键盘按下等事件中调用。
 */
let _activityThrottle = 0;
export function recordActivity() {
  const now = Date.now();
  if (now - _activityThrottle < 5000) return; // 5 秒内不重复上报
  _activityThrottle = now;
  // 异步通知后端 record_activity 命令（S2 复盘接线），降级 checkStatus
  securityApi.recordActivity().catch(() => {
    securityApi.checkStatus().catch(() => {});
  });
}

// ============================================================
// 加密设置面板
// ============================================================

/**
 * 在设置面板中渲染安全设置区块。
 * @param {HTMLElement} container - 设置面板容器
 */
export function renderSecuritySettings(container) {
  const state = getState();
  const isEnc = isEncrypted();

  const securityHtml = `
    <div class="border-t border-border-default pt-5 mt-5" id="securitySettingsSection">
      <h3 class="text-sm font-semibold m-0 mb-4 flex items-center gap-1">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" style="display:inline; vertical-align:middle; margin-right:6px;">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
        </svg>
        <span data-i18n="security.section_title">安全防御</span>
      </h3>

      <!-- 加密状态 -->
      <div class="flex items-center justify-between py-3 border-b border-border-default">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="security.encryption_status">数据库加密</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.encryption_desc">SQLCipher AES-256 透明加密，保护文档和向量数据</span>
        </div>
        <div class="shrink-0 ml-4">
          ${isEnc
            ? `<span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-md text-xs font-medium bg-[rgba(var(--success-rgb),0.12)] text-success">${icon('lock', 'sm')} ${t('security.encrypted', '已加密')}</span>`
            : `<span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-md text-xs font-medium bg-[rgba(100,116,139,0.12)] text-text-quaternary">${t('security.not_encrypted', '未加密')}</span>`
          }
        </div>
      </div>

      ${!isEnc ? `
        <!-- 加密入口 -->
        <div class="flex items-center justify-between py-3 border-b border-border-default" id="encryptRow">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="security.enable_encryption">启用加密</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.enable_encryption_desc">设置密码后，所有数据库内容将使用 AES-256 加密</span>
          </div>
          <div class="shrink-0 ml-4">
            <button id="btnEnableEncryption" class="px-3 py-1 text-xs font-medium border-none rounded-md bg-primary text-surface-0 cursor-pointer hover:bg-primary-hover transition-colors" data-i18n="security.enable">启用</button>
          </div>
        </div>
      ` : ''}

      ${isEnc ? `
        <!-- 手动锁定 -->
        <div class="flex items-center justify-between py-3 border-b border-border-default">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="security.manual_lock">手动锁定</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.manual_lock_desc">立即锁定应用，需要密码才能解锁</span>
          </div>
          <div class="shrink-0 ml-4">
            <button id="btnLockApp" class="px-3 py-1 text-xs font-medium border-none rounded-md bg-warning text-surface-0 cursor-pointer transition-colors" data-i18n="security.lock_now">立即锁定</button>
          </div>
        </div>

        <!-- 自动锁屏超时 -->
        <div class="flex items-center justify-between py-3 border-b border-border-default">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="security.auto_lock">自动锁屏</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.auto_lock_desc">空闲超时后自动锁定（分钟，0=禁用）</span>
          </div>
          <div class="shrink-0 ml-4">
            <input type="number" id="autoLockTimeoutInput" class="px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                   min="0" max="1440" step="1"
                   value="${state.autoLockTimeout || 0}"
                   style="width: 80px;"/>
          </div>
        </div>
      ` : ''}

      <!-- PII 检测 -->
      <div class="flex items-center justify-between py-3 border-b border-border-default">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="security.pii_detection">PII 自动检测</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.pii_detection_desc">检测并脱敏邮箱、手机号、身份证等 8 类个人信息</span>
        </div>
        <div class="shrink-0 ml-4">
          <label class="relative inline-block w-10 h-[22px]">
            <input type="checkbox" id="piiDetectionToggle" class="peer opacity-0 w-0 h-0" ${state.piiDetectionEnabled ? 'checked' : ''}/>
            <span class="absolute cursor-pointer inset-0 bg-border-default rounded-[11px] transition-[background] duration-300 before:content-[''] before:absolute before:h-4 before:w-4 before:left-[3px] before:bottom-[3px] before:bg-white before:rounded-full before:transition-transform before:duration-300 peer-checked:bg-primary peer-checked:before:translate-x-[18px]"></span>
          </label>
        </div>
      </div>

      <!-- 审计日志 -->
      <div class="flex items-center justify-between py-3">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="security.audit_log">审计日志</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.audit_log_desc">查看安全操作记录（哈希链防篡改）</span>
        </div>
        <div class="shrink-0 ml-4 flex gap-2">
          <button id="btnViewAuditLog" class="px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" data-i18n="security.view_log">查看日志</button>
          <button id="btnVerifyAudit" class="px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" data-i18n="security.verify_integrity">验证完整性</button>
          <button id="btnClearAuditLogs" class="px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:text-red-400 hover:border-red-400/40 transition-colors" data-i18n="security.clear_logs">清空日志</button>
        </div>
      </div>

      <!-- 剪贴板自动清除配置（S2 复盘接线） -->
      <div class="flex items-center justify-between py-3 border-t border-border-default">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="security.clipboard_clear">剪贴板自动清除</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.clipboard_clear_desc">复制敏感数据后自动清除剪贴板（秒，0=禁用）</span>
        </div>
        <div class="shrink-0 ml-4 flex items-center gap-2">
          <input type="number" id="clipboardClearTimeoutInput" class="px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                 min="0" max="300" step="5"
                 value="${state.clipboardClearTimeout || 30}"
                 style="width: 70px;"/>
          <label class="relative inline-block w-10 h-[22px]">
            <input type="checkbox" id="clipboardClearToggle" class="peer opacity-0 w-0 h-0" ${state.clipboardClearEnabled ? 'checked' : ''}/>
            <span class="absolute cursor-pointer inset-0 bg-border-default rounded-[11px] transition-[background] duration-300 before:content-[''] before:absolute before:h-4 before:w-4 before:left-[3px] before:bottom-[3px] before:bg-white before:rounded-full before:transition-transform before:duration-300 peer-checked:bg-primary peer-checked:before:translate-x-[18px]"></span>
          </label>
        </div>
      </div>

      <!-- 安全态势选择器（S2 复盘接线） -->
      <div class="flex items-center justify-between py-3 border-t border-border-default">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="security.posture">安全态势</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.posture_desc">控制安全筛查严格程度</span>
        </div>
        <div class="shrink-0 ml-4 flex gap-1" id="postureSelector">
          <button class="posture-btn px-2.5 py-1 text-xs font-medium rounded-md border border-border-default text-text-tertiary hover:bg-surface-3 transition-colors" data-posture="dangerous" data-i18n="security.posture_dangerous">宽松</button>
          <button class="posture-btn px-2.5 py-1 text-xs font-medium rounded-md border border-border-default text-text-tertiary hover:bg-surface-3 transition-colors" data-posture="auto" data-i18n="security.posture_auto">自动</button>
          <button class="posture-btn px-2.5 py-1 text-xs font-medium rounded-md border border-border-default text-text-tertiary hover:bg-surface-3 transition-colors" data-posture="strict" data-i18n="security.posture_strict">严格</button>
        </div>
      </div>

      <!-- Shadow 筛查统计（S2 复盘接线） -->
      <div class="py-3 border-t border-border-default" id="shadowScreenSection">
        <div class="flex items-center justify-between mb-2">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="security.shadow_screen">Shadow 筛查统计</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.shadow_screen_desc">并行安全筛查对比统计</span>
          </div>
          <button id="btnResetShadowStats" class="shrink-0 ml-4 px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" data-i18n="security.reset_stats">重置</button>
        </div>
        <div class="flex gap-3 text-sm" id="shadowScreenStats">
          <span class="text-text-tertiary"><span data-i18n="security.shadow_total">总计</span>: <span class="font-semibold text-text-primary" id="shadowTotal">-</span></span>
          <span class="text-success"><span data-i18n="security.shadow_agree">一致</span>: <span class="font-semibold" id="shadowAgree">-</span></span>
          <span class="text-danger"><span data-i18n="security.shadow_disagree">分歧</span>: <span class="font-semibold" id="shadowDisagree">-</span></span>
          <span class="text-text-quaternary"><span data-i18n="security.shadow_unavailable">不可用</span>: <span class="font-semibold" id="shadowUnavailable">-</span></span>
        </div>
      </div>

      <!-- 紧急销毁（S2 复盘接线） -->
      <div class="py-3 border-t border-border-default" id="panicWipeSection">
        <div class="flex items-center justify-between mb-2">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="security.panic_wipe">紧急销毁</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="security.panic_wipe_desc">设置紧急密码，输入后立即销毁所有数据</span>
          </div>
          <button id="btnSetPanicWipe" class="shrink-0 ml-4 px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" data-i18n="security.set_panic_password">设置密码</button>
        </div>
        <div class="flex items-center gap-2" id="panicWipeStatus" style="display:none">
          <span class="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-md text-xs font-medium bg-[rgba(var(--success-rgb),0.12)] text-success" data-i18n="security.panic_enabled">已启用</span>
          <button id="btnClearPanicWipe" class="px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:text-red-400 hover:border-red-400/40 transition-colors" data-i18n="security.clear_panic">清除</button>
        </div>
      </div>
    </div>
  `;

  // 插入到设置面板（在 LLM 配置区块之后）
  const insertPoint = container.querySelector('#securitySettingsSection') ||
                      container.querySelector('.settings-section:last-of-type');
  if (!container.querySelector('#securitySettingsSection')) {
    container.insertAdjacentHTML('beforeend', securityHtml);
  }

  // 绑定事件
  bindSecuritySettingsEvents(container);
}

/**
 * 绑定安全设置区块中的事件。
 */
function bindSecuritySettingsEvents(container) {
  // 启用加密
  const btnEnable = container.querySelector('#btnEnableEncryption');
  if (btnEnable) {
    btnEnable.addEventListener('click', showEncryptDialog);
  }

  // 手动锁定
  const btnLock = container.querySelector('#btnLockApp');
  if (btnLock) {
    btnLock.addEventListener('click', lockApp);
  }

  // 自动锁屏超时
  const autoLockInput = container.querySelector('#autoLockTimeoutInput');
  if (autoLockInput) {
    autoLockInput.addEventListener('change', async (e) => {
      const minutes = parseInt(e.target.value, 10) || 0;
      try {
        await securityApi.setAutoLockTimeout(minutes * 60);
        setState({ autoLockTimeout: minutes });
        showToast(t('security.auto_lock_updated', '自动锁屏已更新'), 'success');
      } catch (err) {
        showToast(t('security.update_failed', '更新失败') + ': ' + String(err), 'error');
      }
    });
  }

  // PII 检测开关
  const piiToggle = container.querySelector('#piiDetectionToggle');
  if (piiToggle) {
    piiToggle.addEventListener('change', async (e) => {
      const enabled = e.target.checked;
      try {
        await securityApi.setPiiDetection(enabled);
        setState({ piiDetectionEnabled: enabled });
        showToast(
          enabled
            ? t('security.pii_enabled', 'PII 检测已开启')
            : t('security.pii_disabled', 'PII 检测已关闭'),
          'success'
        );
      } catch (err) {
        showToast(t('security.update_failed', '更新失败') + ': ' + String(err), 'error');
        e.target.checked = !enabled; // 恢复
      }
    });
  }

  // 查看审计日志
  const btnAuditLog = container.querySelector('#btnViewAuditLog');
  if (btnAuditLog) {
    btnAuditLog.addEventListener('click', showAuditLogPanel);
  }

  // 验证审计链
  const btnVerify = container.querySelector('#btnVerifyAudit');
  if (btnVerify) {
    btnVerify.addEventListener('click', verifyAuditChain);
  }

  // 清空审计日志（S2 复盘接线）
  const btnClearLogs = container.querySelector('#btnClearAuditLogs');
  if (btnClearLogs) {
    btnClearLogs.addEventListener('click', async () => {
      const confirmed = await showConfirmDialog({
        title: t('security.clear_logs_confirm', '确认清空所有审计日志？此操作不可撤销。'),
      });
      if (!confirmed) return;
      try {
        await securityApi.clearAuditLogs();
        showToast(t('security.logs_cleared', '审计日志已清空'), 'success');
      } catch (err) {
        showToast(t('security.clear_failed', '清空失败') + ': ' + String(err), 'error');
      }
    });
  }

  // 剪贴板自动清除配置（S2 复盘接线）
  const clipboardToggle = container.querySelector('#clipboardClearToggle');
  const clipboardInput = container.querySelector('#clipboardClearTimeoutInput');
  if (clipboardToggle && clipboardInput) {
    const updateClipboard = async () => {
      const enabled = clipboardToggle.checked;
      const secs = parseInt(clipboardInput.value, 10) || 0;
      try {
        await securityApi.setClipboardConfig(enabled, secs);
        setState({ clipboardClearEnabled: enabled, clipboardClearTimeout: secs });
      } catch (err) {
        showToast(t('security.update_failed', '更新失败') + ': ' + String(err), 'error');
      }
    };
    clipboardToggle.addEventListener('change', updateClipboard);
    clipboardInput.addEventListener('change', updateClipboard);
  }

  // 安全态势选择器（S2 复盘接线）
  const postureButtons = container.querySelectorAll('.posture-btn');
  if (postureButtons.length > 0) {
    // 异步加载当前态势
    securityApi.getSecurityPosture().then((posture) => {
      postureButtons.forEach((btn) => {
        const isActive = btn.dataset.posture === posture;
        btn.classList.toggle('bg-accent', isActive);
        btn.classList.toggle('text-surface-0', isActive);
        btn.classList.toggle('border-accent', isActive);
        btn.classList.toggle('text-text-tertiary', !isActive);
        btn.classList.toggle('border-border-default', !isActive);
      });
    }).catch(() => {});
    // 绑定点击
    postureButtons.forEach((btn) => {
      btn.addEventListener('click', async () => {
        const posture = btn.dataset.posture;
        try {
          await securityApi.setSecurityPosture(posture);
          postureButtons.forEach((b) => {
            const active = b.dataset.posture === posture;
            b.classList.toggle('bg-accent', active);
            b.classList.toggle('text-surface-0', active);
            b.classList.toggle('border-accent', active);
            b.classList.toggle('text-text-tertiary', !active);
            b.classList.toggle('border-border-default', !active);
          });
          showToast(t('security.posture_updated', '安全态势已更新'), 'success');
        } catch (err) {
          showToast(t('security.update_failed', '更新失败') + ': ' + String(err), 'error');
        }
      });
    });
  }

  // Shadow 筛查统计（S2 复盘接线）
  const shadowSection = container.querySelector('#shadowScreenSection');
  if (shadowSection) {
    const loadShadowStats = async () => {
      try {
        const stats = await securityApi.getSecurityScreenStats();
        const el = (id) => shadowSection.querySelector(id);
        if (el('#shadowTotal')) el('#shadowTotal').textContent = String(stats.total || 0);
        if (el('#shadowAgree')) el('#shadowAgree').textContent = String(stats.agree || 0);
        if (el('#shadowDisagree')) el('#shadowDisagree').textContent = String(stats.disagree || 0);
        if (el('#shadowUnavailable')) el('#shadowUnavailable').textContent = String(stats.unavailable || 0);
      } catch (_) {
        // 静默降级
      }
    };
    loadShadowStats();
    const resetBtn = container.querySelector('#btnResetShadowStats');
    if (resetBtn) {
      resetBtn.addEventListener('click', async () => {
        try {
          await securityApi.resetSecurityScreenStats();
          await loadShadowStats();
          showToast(t('security.stats_reset', '统计已重置'), 'success');
        } catch (err) {
          showToast(String(err), 'error');
        }
      });
    }
  }

  // 紧急销毁（S2 复盘接线）
  const btnSetPanic = container.querySelector('#btnSetPanicWipe');
  const panicStatus = container.querySelector('#panicWipeStatus');
  const btnClearPanic = container.querySelector('#btnClearPanicWipe');
  if (btnSetPanic) {
    // 异步检查是否已启用
    securityApi.isPanicWipeEnabled().then((enabled) => {
      if (enabled && panicStatus) {
        panicStatus.style.display = 'flex';
        btnSetPanic.textContent = t('security.update_panic_password', '更新密码');
      }
    }).catch(() => {});
    btnSetPanic.addEventListener('click', () => {
      showPanicWipeDialog(btnSetPanic, panicStatus);
    });
  }
  if (btnClearPanic) {
    btnClearPanic.addEventListener('click', async () => {
      const confirmed = await showConfirmDialog({
        title: t('security.clear_panic_confirm', '确认清除紧急销毁密码？'),
      });
      if (!confirmed) return;
      try {
        await securityApi.clearPanicWipePassword();
        if (panicStatus) panicStatus.style.display = 'none';
        if (btnSetPanic) btnSetPanic.textContent = t('security.set_panic_password', '设置密码');
        showToast(t('security.panic_cleared', '紧急销毁已清除'), 'success');
      } catch (err) {
        showToast(String(err), 'error');
      }
    });
  }
}

// ============================================================
// 加密对话框
// ============================================================

/**
 * 显示加密设置对话框（输入密码 + 确认密码）。
 */
function showEncryptDialog() {
  const dialog = document.createElement('div');
  dialog.className = `fixed inset-0 ${zClass(Z_INDEX.AUDIT_LOG)} flex items-center justify-center bg-black/50 backdrop-blur-[4px]`;
  dialog.innerHTML = `
    <div class="bg-bg-primary rounded-2xl shadow-modal max-w-[480px] w-[90%] max-h-[85vh] overflow-y-auto">
      <h3 class="text-lg font-semibold px-6 pt-6 m-0" data-i18n="security.encrypt_title">启用数据库加密</h3>
      <div class="px-7 py-5">
        <p class="text-sm text-text-secondary leading-normal m-0 mb-5" data-i18n="security.encrypt_desc">
          设置一个强密码，用于加密你的数据库。所有文档、向量、对话记录将使用 AES-256 加密。
        </p>
        <div class="mb-4">
          <label class="block text-[13px] font-medium mb-1.5" data-i18n="security.password">密码</label>
          <input type="password" id="encPwdInput" class="w-full px-3.5 py-2.5 text-sm border border-border-default rounded-md bg-bg-input text-text-primary outline-none box-border transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]" autocomplete="new-password"
                 data-i18n-placeholder="security.password_placeholder"
                 placeholder="至少 8 个字符…"/>
        </div>
        <div class="mb-4">
          <label class="block text-[13px] font-medium mb-1.5" data-i18n="security.confirm_password">确认密码</label>
          <input type="password" id="encPwdConfirmInput" class="w-full px-3.5 py-2.5 text-sm border border-border-default rounded-md bg-bg-input text-text-primary outline-none box-border transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]" autocomplete="new-password"
                 data-i18n-placeholder="security.confirm_password_placeholder"
                 placeholder="再次输入密码…"/>
        </div>
        <div id="encPwdStrength" class="text-xs font-medium mt-1.5 min-h-[18px]"></div>
        <p class="text-[13px] text-warning bg-[rgba(var(--warning-rgb),0.08)] px-3.5 py-2.5 rounded-lg mt-3 leading-normal" data-i18n="security.encrypt_warning">
          密码丢失将无法恢复数据。请妥善保管密码。
        </p>
        <p id="encError" class="text-[13px] text-danger min-h-[18px] mt-2"></p>
      </div>
      <div class="flex justify-end gap-3 px-7 pb-6">
        <button class="px-3.5 py-2.5 text-sm font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" id="encCancelBtn" data-i18n="common.cancel">取消</button>
        <button class="px-3.5 py-2.5 text-sm font-medium border-none rounded-md bg-primary text-surface-0 cursor-pointer hover:bg-primary-hover transition-colors" id="encConfirmBtn" data-i18n="security.encrypt_confirm">确认加密</button>
      </div>
    </div>
  `;
  document.body.appendChild(dialog);

  const pwdInput = dialog.querySelector('#encPwdInput');
  const pwdConfirm = dialog.querySelector('#encPwdConfirmInput');
  const strengthEl = dialog.querySelector('#encPwdStrength');
  const errorEl = dialog.querySelector('#encError');
  const cancelBtn = dialog.querySelector('#encCancelBtn');
  const confirmBtn = dialog.querySelector('#encConfirmBtn');

  // 密码强度实时检测（S2 复盘：调用后端 check_password_strength）
  pwdInput.addEventListener('input', async () => {
    const pwd = pwdInput.value;
    if (!pwd) {
      strengthEl.textContent = '';
      return;
    }
    // 先用前端即时评估快速响应
    const localStrength = assessPasswordStrength(pwd);
    strengthEl.textContent = localStrength.label;
    strengthEl.className = `text-xs font-medium mt-1.5 min-h-[18px] ${localStrength.level === 'weak' ? 'text-danger' : localStrength.level === 'medium' ? 'text-warning' : 'text-success'}`;
    // 再异步调用后端精确评估（含 Argon2id 建议）
    try {
      const result = await securityApi.checkPasswordStrength(pwd);
      if (result && result.level) {
        const pct = result.percentage || 0;
        const color = result.color || '';
        const suggestions = result.suggestions || [];
        const levelText = t('security.pwd_' + result.level, result.level);
        strengthEl.textContent = `${levelText} (${pct}%)`;
        strengthEl.className = `text-xs font-medium mt-1.5 min-h-[18px] ${color || (result.level === 'weak' ? 'text-danger' : result.level === 'medium' ? 'text-warning' : 'text-success')}`;
        if (suggestions.length > 0 && pwd.length < 12) {
          strengthEl.textContent += ' — ' + suggestions[0];
        }
      }
    } catch (_) {
      // 后端不可用时保留前端评估结果
    }
  });

  // 取消
  cancelBtn.addEventListener('click', () => {
    if (encTrap) encTrap.deactivate();
    dialog.remove();
  });

  // 确认加密
  confirmBtn.addEventListener('click', async () => {
    const pwd = pwdInput.value;
    const confirm = pwdConfirm.value;

    if (!pwd || pwd.length < 8) {
      errorEl.textContent = t('security.error_pwd_too_short', '密码至少 8 个字符');
      return;
    }
    if (pwd !== confirm) {
      errorEl.textContent = t('security.error_pwd_mismatch', '两次输入的密码不一致');
      return;
    }

    confirmBtn.disabled = true;
    confirmBtn.textContent = t('security.encrypting', '加密中…');

    try {
      const result = await securityApi.encrypt(pwd);
      if (result.success) {
        setState({ securityState: 'encrypted_unlocked' });
        showToast(t('security.encrypt_success', '数据库已加密'), 'success');
        if (encTrap) encTrap.deactivate();
        dialog.remove();
        // 刷新设置面板
        refreshSettingsPanel();
      } else {
        errorEl.textContent = result.message || t('security.encrypt_failed', '加密失败');
      }
    } catch (err) {
      errorEl.textContent = String(err);
    } finally {
      confirmBtn.disabled = false;
      confirmBtn.textContent = t('security.encrypt_confirm', '确认加密');
    }
  });

  // Esc 关闭
  dialog.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      if (encTrap) encTrap.deactivate();
      dialog.remove();
    }
  });

  // 激活 Focus Trap（REQ-A11Y-002）：Tab 键锁定在加密对话框内
  const encTrap = createFocusTrap(dialog);
  encTrap.activate();

  pwdInput.focus();
}

/**
 * 简易密码强度评估（前端预检，后端有 Argon2id 正式验证）。
 * @param {string} pwd
 * @returns {{level: string, label: string}}
 */
function assessPasswordStrength(pwd) {
  let score = 0;
  if (pwd.length >= 8) score++;
  if (pwd.length >= 12) score++;
  if (/[a-z]/.test(pwd) && /[A-Z]/.test(pwd)) score++;
  if (/\d/.test(pwd)) score++;
  if (/[^a-zA-Z0-9]/.test(pwd)) score++;

  if (score <= 2) return { level: 'weak', label: t('security.pwd_weak', '弱') };
  if (score <= 3) return { level: 'medium', label: t('security.pwd_medium', '中') };
  if (score <= 4) return { level: 'good', label: t('security.pwd_good', '良好') };
  return { level: 'strong', label: t('security.pwd_strong', '强') };
}

// ============================================================
// 审计日志面板
// ============================================================

/**
 * 显示审计日志面板。
 */
async function showAuditLogPanel() {
  const panel = document.createElement('div');
  panel.className = 'modal-overlay';
  panel.innerHTML = `
    <div class="modal-card modal-card--wide">
      <h3 class="modal-title" data-i18n="security.audit_log_title">安全审计日志</h3>
      <div class="modal-body">
        <div id="auditLogContent" class="audit-log-container">
          <p class="loading-text" data-i18n="security.loading">加载中…</p>
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn btn-secondary" id="auditCloseBtn" data-i18n="common.close">关闭</button>
        <button class="btn btn-secondary" id="auditClearBtn" data-i18n="security.clear_logs">清空日志</button>
        <button class="btn btn-secondary" id="auditVerifyBtn" data-i18n="security.verify_integrity">验证完整性</button>
      </div>
    </div>
  `;
  document.body.appendChild(panel);

  const contentEl = panel.querySelector('#auditLogContent');
  const closeBtn = panel.querySelector('#auditCloseBtn');
  const verifyBtn = panel.querySelector('#auditVerifyBtn');

  closeBtn.addEventListener('click', () => {
    if (auditTrap) auditTrap.deactivate();
    panel.remove();
  });
  verifyBtn.addEventListener('click', verifyAuditChain);

  // 清空审计日志（S2 复盘接线）
  const clearBtn = panel.querySelector('#auditClearBtn');
  if (clearBtn) {
    clearBtn.addEventListener('click', async () => {
      const confirmed = await showConfirmDialog({
        title: t('security.clear_logs_confirm', '确认清空所有审计日志？此操作不可撤销。'),
      });
      if (!confirmed) return;
      try {
        await securityApi.clearAuditLogs();
        showToast(t('security.logs_cleared', '审计日志已清空'), 'success');
        contentEl.innerHTML = `<p class="empty-text" data-i18n="security.no_audit_logs">暂无审计日志记录</p>`;
      } catch (err) {
        showToast(t('security.clear_failed', '清空失败') + ': ' + String(err), 'error');
      }
    });
  }

  // 激活 Focus Trap（REQ-A11Y-002）：Tab 键锁定在审计日志面板内
  const auditTrap = createFocusTrap(panel);
  auditTrap.activate();

  // 加载审计日志
  try {
    const logs = await securityApi.getAuditLogs(100);
    if (!logs || logs.length === 0) {
      contentEl.innerHTML = `<p class="empty-text" data-i18n="security.no_audit_logs">暂无审计日志记录</p>`;
      return;
    }

    const rowsHtml = logs.map(log => {
      const time = new Date(log.timestamp * 1000).toLocaleString();
      const hashShort = log.entry_hash ? log.entry_hash.substring(0, 12) + '…' : '-';
      return `
        <div class="audit-log-row">
          <span class="audit-log-time">${time}</span>
          <span class="audit-log-action audit-action--${log.action}">${log.action}</span>
          <span class="audit-log-target">${log.target || '-'}</span>
          <span class="audit-log-hash" title="${log.entry_hash || ''}">${hashShort}</span>
        </div>
      `;
    }).join('');

    contentEl.innerHTML = `
      <div class="audit-log-header">
        <span data-i18n="security.audit_time">时间</span>
        <span data-i18n="security.audit_action">操作</span>
        <span data-i18n="security.audit_target">目标</span>
        <span data-i18n="security.audit_hash">哈希</span>
      </div>
      ${rowsHtml}
    `;
  } catch (err) {
    contentEl.innerHTML = `<p class="error-text">${String(err)}</p>`;
  }
}

/**
 * 验证审计日志哈希链完整性。
 */
async function verifyAuditChain() {
  try {
    showToast(t('security.verifying', '正在验证…'), 'info');
    const result = await securityApi.verifyAuditChain();
    if (result.valid) {
      showToast(
        t('security.audit_valid', '审计日志完整，未检测到篡改') +
        ` (${result.count} ${t('security.audit_entries', '条记录')})`,
        'success'
      );
    } else {
      showToast(
        t('security.audit_invalid', '审计日志可能已被篡改') +
        ` (${t('security.audit_broken_at', '第')} ${result.broken_at} ${t('security.audit_entry', '条')})`,
        'error'
      );
    }
  } catch (err) {
    showToast(t('security.verify_failed', '验证失败') + ': ' + String(err), 'error');
  }
}

// ============================================================
// 刷新设置面板
// ============================================================

/**
 * 刷新设置面板（加密后重新渲染安全区块）。
 */
function refreshSettingsPanel() {
  const settingsContainer = document.querySelector('#securitySettingsContainer');
  if (!settingsContainer) return;

  // 移除旧的安全区块
  const oldSection = settingsContainer.querySelector('#securitySettingsSection');
  if (oldSection) oldSection.remove();

  // 重新渲染
  settingsContainer.innerHTML = '';
  // @ts-expect-error Element extended with HTMLElement properties via dom-ext.d.ts
  renderSecuritySettings(settingsContainer);
}

// ============================================================
// 初始化
// ============================================================

/**
 * 初始化安全模块。
 * 在应用启动时调用。
 */
export async function initSecurity() {
  // 1. 同步后端安全状态
  await syncSecurityStatus();

  // 2. 监听安全事件
  await listenSecurityEvents();

  // 3. 订阅状态变化，自动显示/隐藏锁屏遮罩
  subscribe('securityState', (newState) => {
    if (newState === 'locked') {
      showLockOverlay();
    } else if (newState === 'encrypted_unlocked') {
      hideLockOverlay();
    }
  });

  // 4. 绑定全局用户活动监听（重置自动锁屏计时器）
  let activityTimer = null;
  const activityHandler = () => {
    if (activityTimer) return;
    activityTimer = setTimeout(() => {
      activityTimer = null;
    }, 10000);
    recordActivity();
  };
  document.addEventListener('mousemove', activityHandler, { passive: true });
  document.addEventListener('keydown', activityHandler, { passive: true });
}

// ============================================================
// 命令面板集成
// ============================================================

/**
 * 返回安全相关的命令面板条目。
 * @returns {Array<{id: string, label: string, group: string, action: Function}>}
 */
export function getSecurityCommands() {
  const state = getState();
  const commands = [];

  if (isEncrypted() && !isLocked()) {
    commands.push({
      id: 'lock-app',
      label: t('security.cmd_lock', '锁定应用'),
      group: 'security',
      action: lockApp,
    });
  }

  commands.push({
    id: 'view-audit-log',
    label: t('security.cmd_audit_log', '查看审计日志'),
    group: 'security',
    action: showAuditLogPanel,
  });

  commands.push({
    id: 'verify-audit',
    label: t('security.cmd_verify_audit', '验证审计完整性'),
    group: 'security',
    action: verifyAuditChain,
  });

  return commands;
}

// ============================================================
// 紧急销毁对话框（S2 复盘接线）
// ============================================================

/**
 * 显示紧急销毁密码设置对话框。
 * @param {HTMLElement} btnSetPanic - 设置按钮
 * @param {HTMLElement} panicStatus - 状态显示区域
 */
function showPanicWipeDialog(btnSetPanic, panicStatus) {
  const dialog = document.createElement('div');
  dialog.className = `fixed inset-0 ${zClass(Z_INDEX.AUDIT_LOG)} flex items-center justify-center bg-black/50 backdrop-blur-[4px]`;
  dialog.innerHTML = `
    <div class="bg-bg-primary rounded-2xl shadow-modal max-w-[420px] w-[90%]">
      <h3 class="text-lg font-semibold px-6 pt-6 m-0" data-i18n="security.panic_wipe_title">🚨 紧急销毁密码</h3>
      <div class="px-7 py-5">
        <p class="text-sm text-text-secondary leading-normal m-0 mb-5" data-i18n="security.panic_wipe_warning">
          设置紧急销毁密码后，在锁屏界面输入此密码将立即永久删除所有数据库内容。此操作不可撤销。
        </p>
        <div class="mb-4">
          <label class="block text-[13px] font-medium mb-1.5" data-i18n="security.password">密码</label>
          <input type="password" id="panicPwdInput" class="w-full px-3.5 py-2.5 text-sm border border-border-default rounded-md bg-bg-input text-text-primary outline-none box-border transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]" autocomplete="new-password" placeholder="至少 8 个字符…"/>
        </div>
        <div class="mb-4">
          <label class="block text-[13px] font-medium mb-1.5" data-i18n="security.confirm_password">确认密码</label>
          <input type="password" id="panicPwdConfirm" class="w-full px-3.5 py-2.5 text-sm border border-border-default rounded-md bg-bg-input text-text-primary outline-none box-border transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]" autocomplete="new-password" placeholder="再次输入密码…"/>
        </div>
        <p id="panicError" class="text-[13px] text-danger min-h-[18px] mt-2"></p>
      </div>
      <div class="flex justify-end gap-3 px-7 pb-6">
        <button class="px-3.5 py-2.5 text-sm font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" id="panicCancelBtn" data-i18n="common.cancel">取消</button>
        <button class="px-3.5 py-2.5 text-sm font-medium border-none rounded-md bg-danger text-surface-0 cursor-pointer hover:opacity-90 transition-opacity" id="panicConfirmBtn" data-i18n="security.set_panic_confirm">确认设置</button>
      </div>
    </div>
  `;
  document.body.appendChild(dialog);

  const pwdInput = dialog.querySelector('#panicPwdInput');
  const pwdConfirm = dialog.querySelector('#panicPwdConfirm');
  const errorEl = dialog.querySelector('#panicError');
  const cancelBtn = dialog.querySelector('#panicCancelBtn');
  const confirmBtn = dialog.querySelector('#panicConfirmBtn');
  const trap = createFocusTrap(dialog);
  trap.activate();

  const closeDialog = () => {
    trap.deactivate();
    dialog.remove();
  };

  cancelBtn.addEventListener('click', closeDialog);
  dialog.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') closeDialog();
  });

  confirmBtn.addEventListener('click', async () => {
    const pwd = pwdInput.value;
    const confirm = pwdConfirm.value;
    if (!pwd || pwd.length < 8) {
      errorEl.textContent = t('security.error_pwd_too_short', '密码至少 8 个字符');
      return;
    }
    if (pwd !== confirm) {
      errorEl.textContent = t('security.error_pwd_mismatch', '两次输入的密码不一致');
      return;
    }
    try {
      await securityApi.setPanicWipePassword(pwd);
      if (panicStatus) panicStatus.style.display = 'flex';
      if (btnSetPanic) btnSetPanic.textContent = t('security.update_panic_password', '更新密码');
      showToast(t('security.panic_set', '紧急销毁密码已设置'), 'success');
      closeDialog();
    } catch (err) {
      errorEl.textContent = String(err);
    }
  });

  pwdInput.focus();
}
