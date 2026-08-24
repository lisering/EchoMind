/**
 * EchoMind 记忆管理面板（REQ-RAG-033 持久化记忆系统前端 UI）。
 *
 * 功能：
 * 1. 设置面板内嵌 section（与 security.js / perf-settings.js 风格统一）
 * 2. Tab 切换式布局（Wing / Hall / Room），每个 Tab 显示记忆数量
 * 3. 每条记忆卡片：内容 + 重要性进度条 + 操作按钮
 * 4. 记忆开关 toggle（set_memory_enabled）
 * 5. 提升（promote_memory）/ 删除（delete_memory）
 * 6. 批量操作：清空指定 tier（clear_memories）
 *
 * 设计遵循现有 settings.js 模式：Tailwind 工具类 + 设计令牌 + i18n。
 */

import { $ } from './utils.js';
import { invoke } from './ipc.js';
import { memoryExtApi } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { showConfirmDialog } from './confirm-dialog.js';

/** 记忆层级配置 */
const TIER_CONFIG = [
  { key: 'wing', labelKey: 'memory.tier_wing', descKey: 'memory.tier_wing_desc', color: '#38bdf8', icon: 'M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5' },
  { key: 'hall', labelKey: 'memory.tier_hall', descKey: 'memory.tier_hall_desc', color: '#a78bfa', icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6' },
  { key: 'room', labelKey: 'memory.tier_room', descKey: 'memory.tier_room_desc', color: '#4ade80', icon: 'M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 20.571M10 4l8 0M4 11h7m-4 4l4 0' },
];

/** 当前选中的 tier tab */
let _activeTier = 'wing';

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

  const tierCounts = {
    wing: allMemories.filter(m => m.tier === 'wing').length,
    hall: allMemories.filter(m => m.tier === 'hall').length,
    room: allMemories.filter(m => m.tier === 'room').length,
  };

  // 确保 _activeTier 有效
  if (!TIER_CONFIG.some(tg => tg.key === _activeTier)) {
    _activeTier = 'wing';
  }

  container.innerHTML = `
    <div class="border-t border-border-default pt-5 mt-5" id="memorySettingsSection">
      <div class="flex items-start justify-between mb-4">
        <div class="flex items-center gap-1.5">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
               stroke-linecap="round" stroke-linejoin="round" class="text-text-tertiary shrink-0">
            <path d="M9.5 2A2.5 2.5 0 0112 4.5v15a2.5 2.5 0 01-5 0V4.5A2.5 2.5 0 019.5 2z"/>
            <path d="M14.5 2A2.5 2.5 0 0012 4.5v15a2.5 2.5 0 005 0V4.5A2.5 2.5 0 0014.5 2z"/>
          </svg>
          <div>
            <h3 class="text-sm font-semibold m-0">${t('memory.title')}</h3>
            <p class="text-xs text-text-tertiary leading-tight m-0 mt-0.5">${t('memory.description')}</p>
          </div>
        </div>
        <div id="memToggle" class="w-10 h-5 rounded-full bg-slate-600 cursor-pointer transition-colors shrink-0 mt-0.5" role="switch" aria-checked="${memoryEnabled}">
          <span class="block w-4 h-4 mt-0.5 ml-0.5 bg-white rounded-full transition-transform"></span>
        </div>
      </div>

      <!-- Tier Tab Bar -->
      <div class="flex gap-1.5 mb-3">
        ${TIER_CONFIG.map(tier => {
          const count = tierCounts[tier.key];
          const isActive = _activeTier === tier.key;
          return `
            <button class="mem-tab-btn flex-1 px-3 py-2 rounded-lg text-xs font-medium transition-all duration-150 flex items-center justify-center gap-1.5
              ${isActive
                ? 'bg-surface-3 text-text-primary border border-border-default'
                : 'bg-transparent text-text-tertiary border border-transparent hover:bg-surface-2 hover:text-text-secondary'
              }"
              data-tier="${tier.key}">
              <span class="w-2 h-2 rounded-full shrink-0" style="background:${tier.color}"></span>
              <span>${t(tier.labelKey)}</span>
              <span class="px-1.5 py-0.5 rounded-md text-[10px] font-semibold ${isActive ? 'bg-accent/15 text-accent' : 'bg-surface-2 text-text-quaternary'}">${count}</span>
            </button>
          `;
        }).join('')}
      </div>

      <!-- Active Tier Content -->
      <div id="memTierContent">
        ${renderTierContent(TIER_CONFIG.find(tg => tg.key === _activeTier), allMemories.filter(m => m.tier === _activeTier))}
      </div>

      <!-- Scratch 层整合 + Burst Buffer（S2 复盘接线） -->
      <div class="border-t border-border-subtle pt-3 mt-3 space-y-2" id="memExtSection">
        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span class="text-xs font-medium" data-i18n="memory.consolidation">Scratch 整合</span>
            <span class="text-[11px] text-text-tertiary" data-i18n="memory.consolidation_desc">手动触发 Scratch 层 LLM 整合</span>
          </div>
          <button id="btnTriggerConsolidation" class="shrink-0 ml-3 px-2.5 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-surface-3 transition-colors" data-i18n="memory.trigger">触发</button>
        </div>

        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span class="text-xs font-medium" data-i18n="memory.scratch_logs">Scratch 日志</span>
            <span class="text-[11px] text-text-tertiary" data-i18n="memory.scratch_logs_desc">查看临时记忆日志</span>
          </div>
          <button id="btnViewScratchLogs" class="shrink-0 ml-3 px-2.5 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-surface-3 transition-colors" data-i18n="memory.view_logs">查看</button>
        </div>

        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span class="text-xs font-medium" data-i18n="memory.burst_buffer">Burst Buffer</span>
            <span class="text-[11px] text-text-tertiary" id="burstBufferStatus" data-i18n="memory.burst_buffer_idle">空闲</span>
          </div>
          <button id="btnFlushBurstBuffer" class="shrink-0 ml-3 px-2.5 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-surface-3 transition-colors" data-i18n="memory.flush">Flush</button>
        </div>
      </div>
    </div>
  `;

  initMemoryHandlers(memoryEnabled, allMemories);
}

/**
 * 渲染当前选中 tier 的内容区域。
 * @param {Object} tier - tier 配置
 * @param {Array} memories - 该 tier 的记忆列表
 * @returns {string} HTML
 */
function renderTierContent(tier, memories) {
  return `
    <div class="space-y-2" data-tier="${tier.key}">
      <!-- Tier Description -->
      <div class="flex items-center justify-between mb-2">
        <p class="text-xs text-text-tertiary leading-relaxed m-0">${t(tier.descKey)}</p>
        ${tier.key !== 'room'
          ? `<button class="mem-clear-btn text-xs px-2.5 py-1 rounded-md border border-border-default text-text-tertiary hover:text-red-400 hover:border-red-400/40 transition-colors shrink-0 ml-3" data-tier="${tier.key}">${t('memory.clear_tier')}</button>`
          : ''
        }
      </div>

      <!-- Memory Cards -->
      <div class="space-y-2 max-h-56 overflow-y-auto">
        ${memories.length === 0
          ? renderEmptyState(tier)
          : memories.map(m => renderMemoryCard(m, tier)).join('')
        }
      </div>
    </div>
  `;
}

/**
 * 渲染空状态。
 * @param {Object} tier - tier 配置
 * @returns {string} HTML
 */
function renderEmptyState(tier) {
  return `
    <div class="flex flex-col items-center justify-center py-8 text-center">
      <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"
           stroke-linecap="round" stroke-linejoin="round" class="text-text-quaternary opacity-50 mb-2">
        <path d="${tier.icon}"/>
      </svg>
      <p class="text-sm text-text-quaternary m-0">${t('memory.empty')}</p>
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
  const createdStr = mem.created_at
    ? new Date(mem.created_at).toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
    : '';

  return `
    <div class="bg-surface-2 border border-border-subtle rounded-lg px-3 py-2.5 space-y-2" data-mem-id="${mem.id}">
      <p class="text-sm text-text-primary leading-relaxed m-0 line-clamp-3" data-selectable>${mem.content || ''}</p>
      <div class="flex items-center gap-2">
        <div class="flex-1 h-1.5 bg-surface-3 rounded-full overflow-hidden">
          <div class="h-full rounded-full transition-all" style="width:${importancePct}%;background:${tier.color}"></div>
        </div>
        <span class="text-xs text-text-quaternary font-medium tabular-nums">${importancePct}%</span>
        ${createdStr ? `<span class="text-xs text-text-quaternary">·</span><span class="text-xs text-text-quaternary">${createdStr}</span>` : ''}
      </div>
      <div class="flex items-center gap-2 pt-0.5">
        ${tier.key !== 'room'
          ? `<button class="mem-promote-btn text-xs px-2 py-0.5 rounded text-accent hover:bg-accent/10 transition-colors" data-mem-id="${mem.id}">${t('memory.promote')}</button>`
          : ''
        }
        <button class="mem-pin-btn text-xs px-2 py-0.5 rounded text-text-tertiary hover:text-accent hover:bg-accent/10 transition-colors" data-mem-id="${mem.id}" data-i18n="memory.pin">📌</button>
        <button class="mem-delete-btn text-xs px-2 py-0.5 rounded text-text-quaternary hover:text-red-400 hover:bg-red-400/10 transition-colors" data-mem-id="${mem.id}">${t('memory.delete')}</button>
      </div>
    </div>
  `;
}

/**
 * 初始化记忆管理事件处理器。
 * @param {boolean} memoryEnabled - 初始启用状态
 * @param {Array} allMemories - 全部记忆列表
 */
function initMemoryHandlers(memoryEnabled, allMemories) {
  // 记忆开关 toggle
  const toggle = $('memToggle');
  if (toggle) {
    updateMemToggle(toggle, memoryEnabled);
    toggle.onclick = async () => {
      const enabled = toggle.getAttribute('aria-checked') === 'true';
      const newEnabled = !enabled;
      updateMemToggle(toggle, newEnabled);
      try {
        await invoke('update_setting', { key: 'memory.enabled', value: String(newEnabled) });
        toast(newEnabled ? t('memory.toggle') + ': ON' : t('memory.toggle') + ': OFF', 'success');
      } catch (err) {
        updateMemToggle(toggle, !newEnabled);
        toastError(err);
      }
    };
  }

  // Tab 切换
  containerQuerySelectorAll('.mem-tab-btn').forEach((btn) => {
    btn.onclick = async () => {
      _activeTier = btn.dataset.tier || 'wing';
      // 重新加载记忆数据并刷新内容
      try {
        const memories = await invoke('get_memories');
        await renderMemorySettings($('memorySettingsContainer'));
      } catch (err) {
        // 静默失败，仍刷新 UI
        await renderMemorySettings($('memorySettingsContainer'));
      }
    };
  });

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

  // 固定记忆（S2 复盘接线：pin_memory）
  containerQuerySelectorAll('.mem-pin-btn').forEach((btn) => {
    btn.onclick = async () => {
      const memId = btn.dataset.memId;
      try {
        await memoryExtApi.pin(memId);
        toastSuccess(t('memory.pinned', '已固定'));
      } catch (err) {
        toastError(err);
      }
    };
  });

  // 触发 Scratch 整合（S2 复盘接线：trigger_memory_consolidation）
  const btnConsolidate = $('btnTriggerConsolidation');
  if (btnConsolidate) {
    btnConsolidate.onclick = async () => {
      btnConsolidate.textContent = t('memory.consolidating', '整合中…');
      btnConsolidate.disabled = true;
      try {
        const result = await memoryExtApi.triggerConsolidation();
        toastSuccess(t('memory.consolidation_done', '整合完成') + (result ? `: ${JSON.stringify(result)}` : ''));
        await renderMemorySettings($('memorySettingsContainer'));
      } catch (err) {
        toastError(err);
      } finally {
        btnConsolidate.textContent = t('memory.trigger', '触发');
        btnConsolidate.disabled = false;
      }
    };
  }

  // 查看 Scratch 日志（S2 复盘接线：get_scratch_logs）
  const btnScratchLogs = $('btnViewScratchLogs');
  if (btnScratchLogs) {
    btnScratchLogs.onclick = async () => {
      try {
        const logs = await memoryExtApi.getScratchLogs(50);
        showScratchLogsDialog(logs);
      } catch (err) {
        toastError(err);
      }
    };
  }

  // Burst Buffer 状态 + Flush（S2 复盘接线：get_burst_buffer_status / flush_memory_burst_buffer）
  const burstStatusEl = $('burstBufferStatus');
  const btnFlush = $('btnFlushBurstBuffer');
  const loadBurstStatus = async () => {
    if (!burstStatusEl) return;
    try {
      const status = await memoryExtApi.getBurstBufferStatus();
      const pending = status?.pending_count || 0;
      const total = status?.total_turns || 0;
      burstStatusEl.textContent = pending > 0
        ? `${t('memory.burst_pending', '待处理')}: ${pending} / ${total}`
        : t('memory.burst_buffer_idle', '空闲');
    } catch (_) {
      // 静默降级
    }
  };
  loadBurstStatus();
  if (btnFlush) {
    btnFlush.onclick = async () => {
      btnFlush.textContent = t('memory.flushing', '处理中…');
      btnFlush.disabled = true;
      try {
        await memoryExtApi.flushBurstBuffer();
        toastSuccess(t('memory.flush_done', 'Burst Buffer 已清空'));
        await loadBurstStatus();
      } catch (err) {
        toastError(err);
      } finally {
        btnFlush.textContent = t('memory.flush', 'Flush');
        btnFlush.disabled = false;
      }
    };
  }

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

/**
 * 显示 Scratch 日志对话框（S2 复盘接线）。
 * @param {Array} logs - scratch 日志列表
 */
function showScratchLogsDialog(logs) {
  const dialog = document.createElement('div');
  dialog.className = 'fixed inset-0 flex items-center justify-center bg-black/50 backdrop-blur-[4px]';
  dialog.style.zIndex = '9000';
  dialog.innerHTML = `
    <div class="bg-bg-primary rounded-2xl shadow-modal max-w-[600px] w-[90%] max-h-[80vh] overflow-y-auto">
      <h3 class="text-lg font-semibold px-6 pt-6 m-0 flex items-center gap-2"><svg class="icon-md shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="9" y1="13" x2="15" y2="13"/><line x1="9" y1="17" x2="13" y2="17"/></svg> Scratch 日志</h3>
      <div class="px-6 py-4">
        ${(!logs || logs.length === 0) ? `
          <p class="text-sm text-text-quaternary text-center py-8">暂无 Scratch 日志</p>
        ` : logs.map(log => {
          const time = log.created_at ? new Date(log.created_at).toLocaleString() : '-';
          return `
            <div class="bg-surface-2 rounded-lg p-3 mb-2">
              <div class="flex items-center justify-between mb-1">
                <span class="text-xs text-text-quaternary">${time}</span>
                <span class="text-xs text-text-tertiary">${log.date || '-'}</span>
              </div>
              <p class="text-sm text-text-primary m-0">${log.content || '-'}</p>
            </div>
          `;
        }).join('')}
      </div>
      <div class="flex justify-end px-6 pb-6">
        <button class="px-3.5 py-2.5 text-sm font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" id="scratchLogsCloseBtn">关闭</button>
      </div>
    </div>
  `;
  document.body.appendChild(dialog);
  dialog.querySelector('#scratchLogsCloseBtn').onclick = () => dialog.remove();
  dialog.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') dialog.remove();
  });
}
