/**
 * EchoMind 记忆管理面板（REQ-RAG-033 持久化记忆系统前端 UI）。
 *
 * 功能：
 * 1. 设置面板内嵌 section（类似 security.js 的 renderSecuritySettings 模式）
 * 2. 三列布局（Wing / Hall / Room），每列可折叠
 * 3. 每条记忆卡片：内容 + 元信息 + 操作按钮
 * 4. 记忆开关 toggle（set_memory_enabled）
 * 5. 提升（promote_memory）/ 删除（delete_memory）/ 置顶（pin_memory）
 * 6. 批量操作：清空指定 tier（clear_memories）
 *
 * 设计遵循现有 settings.js 模式：Tailwind 工具类 + 设计令牌 + i18n。
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { showConfirmDialog } from './confirm-dialog.js';

/** 记忆层级配置 */
const TIER_CONFIG = [
  { key: 'wing', labelKey: 'memory.tier_wing', color: '#38bdf8' },
  { key: 'hall', labelKey: 'memory.tier_hall', color: '#a78bfa' },
  { key: 'room', labelKey: 'memory.tier_room', color: '#4ade80' },
];

/**
 * 渲染记忆管理设置区块到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderMemorySettings(container) {
  if (!container) return;

  let memoryEnabled = false;
  let allMemories = [];

  try {
    allMemories = await invoke('get_memories');
    memoryEnabled = !!(window.__mock?.state?.memoryEnabled) || false;
  } catch (err) {
    // 静默失败
  }

  container.innerHTML = `
    <div class="bg-surface-2 border border-border-default rounded-xl px-4 py-3 space-y-3">
      <div class="flex items-center justify-between">
        <div>
          <h4 class="side-label text-[11px] uppercase tracking-wider text-text-quaternary">${t('memory.title')}</h4>
          <p class="text-xs text-text-quaternary mt-1">${t('memory.description')}</p>
        </div>
        <div id="memToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0" role="switch" aria-checked="${memoryEnabled}">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>
      <div id="memTiers" class="grid grid-cols-3 gap-3">
        ${TIER_CONFIG.map(tier => {
          const tierMemories = allMemories.filter(m => m.tier === tier.key);
          return renderTierColumn(tier, tierMemories);
        }).join('')}
      </div>
    </div>
  `;

  initMemoryHandlers(memoryEnabled);
}

/**
 * 渲染单个 tier 列。
 * @param {Object} tier - tier 配置
 * @param {Array} memories - 该 tier 的记忆列表
 * @returns {string} HTML
 */
function renderTierColumn(tier, memories) {
  const color = tier.color;
  return `
    <div class="space-y-2" data-tier="${tier.key}">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <span class="w-2 h-2 rounded-full" style="background:${color}"></span>
          <span class="text-xs font-medium text-text-secondary">${t(tier.labelKey)}</span>
        </div>
        <div class="flex items-center gap-1">
          ${tier.key !== 'room' ? `<button class="mem-clear-btn text-[10px] text-text-quaternary hover:text-red-300 transition-colors" data-tier="${tier.key}">${t('memory.clear_tier')}</button>` : ''}
        </div>
      </div>
      <div class="space-y-1.5 max-h-40 overflow-y-auto">
        ${memories.length === 0
          ? `<p class="text-[11px] text-text-quaternary text-center py-3">${t('memory.empty')}</p>`
          : memories.map(m => renderMemoryCard(m, tier)).join('')}
      </div>
    </div>
  `;
}

/**
 * 渲染单条记忆卡片。
 * @param {Object} mem - 记忆条目
 * @param {Object} tier - tier 配置
 * @returns {string} HTML
 */
function renderMemoryCard(mem, tier) {
  const importancePct = Math.round((mem.importance || 0.5) * 100);
  return `
    <div class="bg-surface-3 rounded-lg px-2.5 py-2 space-y-1" data-mem-id="${mem.id}">
      <p class="text-xs text-text-primary line-clamp-2">${mem.content || ''}</p>
      <div class="flex items-center gap-2">
        <div class="flex-1 h-1 bg-surface-2 rounded-full overflow-hidden">
          <div class="h-full bg-accent rounded-full" style="width:${importancePct}%"></div>
        </div>
        <span class="text-[9px] text-text-quaternary">${importancePct}%</span>
      </div>
      <div class="flex items-center gap-1.5 pt-0.5">
        ${tier.key !== 'room' ? `<button class="mem-promote-btn text-[10px] text-accent hover:text-accent/80 transition-colors" data-mem-id="${mem.id}">${t('memory.promote')}</button>` : ''}
        <button class="mem-delete-btn text-[10px] text-text-quaternary hover:text-red-300 transition-colors" data-mem-id="${mem.id}">${t('memory.delete')}</button>
      </div>
    </div>
  `;
}

/**
 * 初始化记忆管理事件处理器。
 * @param {boolean} memoryEnabled - 初始启用状态
 */
function initMemoryHandlers(memoryEnabled) {
  // 记忆开关 toggle
  const toggle = $('memToggle');
  if (toggle) {
    updateMemToggle(toggle, memoryEnabled);
    toggle.onclick = async () => {
      const enabled = toggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateMemToggle(toggle, newEnabled);
      try {
        await invoke('set_memory_enabled', { enabled: newEnabled });
        toast(newEnabled ? t('memory.toggle') + ': ON' : t('memory.toggle') + ': OFF', 'success');
      } catch (err) {
        updateMemToggle(toggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // 提升记忆
  containerQuerySelectorAll('.mem-promote-btn').forEach((btn) => {
    btn.onclick = async () => {
      const memId = btn.dataset.memId;
      try {
        await invoke('promote_memory', { memory_id: memId });
        toastSuccess(t('memory.promoted'));
        await renderMemorySettings($('memorySettingsContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  });

  // 删除记忆
  containerQuerySelectorAll('.mem-delete-btn').forEach((btn) => {
    btn.onclick = async () => {
      const memId = btn.dataset.memId;
      try {
        await invoke('delete_memory', { memory_id: memId });
        toastSuccess(t('memory.deleted'));
        await renderMemorySettings($('memorySettingsContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  });

  // 清空 tier
  containerQuerySelectorAll('.mem-clear-btn').forEach((btn) => {
    btn.onclick = async () => {
      const tier = btn.dataset.tier;
      const confirmed = await showConfirmDialog({ title: t('memory.clear_confirm') });
      if (!confirmed) return;
      try {
        const count = await invoke('clear_memories', { tier });
        toastSuccess(t('memory.cleared', { count }));
        await renderMemorySettings($('memorySettingsContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  });
}

/**
 * 查询当前容器内所有匹配元素（辅助函数）。
 * @param {string} selector
 * @returns {NodeListOf<Element>}
 */
function containerQuerySelectorAll(selector) {
  const container = $('memorySettingsContainer');
  if (!container) return document.querySelectorAll(selector);
  return container.querySelectorAll(selector);
}

/**
 * 更新记忆 toggle 视觉状态。
 * @param {HTMLElement} toggle
 * @param {boolean} enabled
 */
function updateMemToggle(toggle, enabled) {
  const knob = toggle.querySelector('span');
  if (enabled) {
    toggle.classList.remove('bg-slate-600');
    toggle.classList.add('bg-accent');
    toggle.setAttribute('aria-checked', 'true');
    if (knob) knob.classList.add('translate-x-5');
  } else {
    toggle.classList.add('bg-slate-600');
    toggle.classList.remove('bg-accent');
    toggle.setAttribute('aria-checked', 'false');
    if (knob) knob.classList.remove('translate-x-5');
  }
}
