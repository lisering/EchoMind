/**
 * EchoMind 付费墙模块 — 配额触顶 / PDF 付费门 / License 激活 / 停用。
 *
 * 职责：
 * 1. 显示付费墙 Modal（设置升级原因文案）
 * 2. 激活 Pro License（验证签名 → 刷新 UI）
 * 3. 停用 Pro 授权
 * 4. 更新侧栏授权状态文案
 */

import { setState, get } from './state.js';
import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { t } from './i18n.js';
import { createFocusTrap } from './focus-trap.js';
import { pushPanel, removePanel } from './panel-stack.js';

/** 付费墙的 Focus Trap 实例（REQ-A11Y-002） */
let _paywallTrap = null;

/**
 * 显示付费墙 Modal，设置升级原因文案。
 * @param {string} reason - 触发付费墙的原因
 */
export function showPaywall(reason) {
  $('paywallReason').textContent = reason || t('paywall.reason_default');
  $('paywallError').classList.add('hidden');
  $('licenseInput').value = '';
  $('paywall').classList.remove('hidden');

  // 激活 Focus Trap（REQ-A11Y-002）：Tab 键锁定在付费墙内
  if (_paywallTrap) {
    _paywallTrap.deactivate();
  }
  _paywallTrap = createFocusTrap($('paywall'));
  _paywallTrap.activate();

  // 注册到面板栈
  pushPanel({ id: 'paywall', close: hidePaywall, element: $('paywall'), label: 'Paywall' });
}

/** 隐藏付费墙 Modal。 */
export function hidePaywall() {
  // 从面板栈移除
  removePanel('paywall');
  // 停用 Focus Trap（恢复焦点到触发元素，REQ-A11Y-002）
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
  if (get('isPro')) {
    el.textContent = t('sidebar.pro_badge_pro');
    badge.className = 'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium bg-accent/15 text-accent border border-accent/20';
    badge.querySelector('.badge-icon').textContent = '⭐';
  } else {
    el.textContent = t('sidebar.pro_badge_free');
    badge.className = 'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium bg-surface-3 text-text-tertiary border border-border-default';
    badge.querySelector('.badge-icon').textContent = '🔒';
  }
}

/**
 * 激活 Pro License。
 */
export async function activatePro() {
  const key = $('licenseInput').value.trim();
  if (!key) {
    $('paywallError').textContent = t('paywall.error_no_key');
    $('paywallError').classList.remove('hidden');
    return;
  }
  $('paywallActivate').disabled = true;
  $('paywallActivate').textContent = t('paywall.activating');
  try {
    const result = await invoke('activate_pro', { licenseKey: key });
    if (result) {
      setState({ isPro: true });
      updateProStatus();
      hidePaywall();
      toastSuccess(t('paywall.activated'));
    }
  } catch (err) {
    $('paywallError').textContent = String(err);
    $('paywallError').classList.remove('hidden');
  }
  $('paywallActivate').disabled = false;
  $('paywallActivate').textContent = t('paywall.activate');
}

/**
 * 停用 Pro 授权（REQ-LIC-004）。
 */
export async function deactivatePro() {
  try {
    await invoke('deactivate_pro');
    setState({ isPro: false });
    updateProStatus();
    // 若设置面板打开，同步更新授权状态显示
    const settingsModal = $('settingsModal');
    if (settingsModal && !settingsModal.classList.contains('hidden')) {
      $('settingsLicenseInfo').innerHTML =
        '<span class="text-slate-400">' + t('settings.license_free') + '</span>';
    }
    toast(t('paywall.deactivated'), 'info');
  } catch (err) {
    toastError(err);
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
    if (e.key === 'Enter') activatePro();
  });
}
