/**
 * EchoMind Trace + Budget 面板（S2 复盘 — 僵尸命令接线）。
 *
 * 功能：
 * 1. RAG 链路追踪面板 — get_recent_traces / get_trace_detail / clear_traces / get_trace_count
 * 2. Token 预算面板 — get_budget_stats / set_budget_limit / get_token_budget_config / set_token_budget_config
 *
 * 在设置面板中渲染为 section，与其他设置模块风格统一。
 */

import { $ } from './utils.js';
import { traceApi, budgetApi } from './ipc.js';
import { t } from './i18n.js';
import { toast, toastError, toastSuccess } from './toast.js';
import { showConfirmDialog } from './confirm-dialog.js';

/**
 * 渲染 Trace + Budget 设置区块到指定容器。
 * @param {HTMLElement} container - 目标容器元素
 */
export async function renderTraceBudgetSettings(container) {
  if (!container) return;

  // 立即清空旧内容，避免重新打开设置时 waitForSelector 命中旧元素
  container.innerHTML = '';

  // 加载 trace 和 budget 数据
  let traceCount = 0;
  let recentTraces = [];
  let budgetStats = null;
  let budgetConfig = null;

  try {
    traceCount = await traceApi.getCount();
    recentTraces = await traceApi.getRecent(5);
  } catch (_) {
    // 静默降级
  }

  try {
    budgetStats = await budgetApi.getStats();
    budgetConfig = await budgetApi.getConfig();
  } catch (_) {
    // 静默降级
  }

  const dailySpent = budgetStats?.daily_spent_usd || 0;
  const dailyLimit = budgetStats?.daily_limit_usd || 0;
  const monthlySpent = budgetStats?.monthly_spent_usd || 0;
  const usagePct = dailyLimit > 0 ? Math.min(100, Math.round((dailySpent / dailyLimit) * 100)) : 0;

  const maxTokens = budgetConfig?.max_tokens || 32768;
  const threshold = budgetConfig?.compaction_threshold || 0.8;
  const keepRatio = budgetConfig?.recent_keep_ratio || 0.67;
  const minMsgs = budgetConfig?.min_messages_to_compact || 3;

  container.innerHTML = `
    <div class="border-t border-border-default pt-5 mt-5" id="traceBudgetSection">
      <!-- Trace 追踪 -->
      <h3 class="text-sm font-semibold m-0 mb-4 flex items-center gap-1">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" style="display:inline; vertical-align:middle; margin-right:6px;">
          <path d="M22 12h-4l-3 9L9 3l-3 9H2"/>
        </svg>
        <span data-i18n="trace.title">RAG 链路追踪</span>
      </h3>

      <div class="flex items-center justify-between py-2">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="trace.count">追踪记录数</span>
          <span class="text-xs text-text-tertiary leading-tight">${traceCount} ${t('trace.records', '条记录')}</span>
        </div>
        <div class="shrink-0 ml-4 flex gap-2">
          <button id="btnViewTraces" class="px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" data-i18n="trace.view_recent">查看最近</button>
          <button id="btnClearTraces" class="px-3 py-1 text-xs font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:text-red-400 hover:border-red-400/40 transition-colors" data-i18n="trace.clear">清空</button>
        </div>
      </div>

      <!-- 最近 5 条 trace 预览 -->
      ${recentTraces.length > 0 ? `
        <div class="bg-surface-2 rounded-lg p-3 mb-3 max-h-40 overflow-y-auto">
          ${recentTraces.map(tr => {
            const time = tr.created_at ? new Date(tr.created_at).toLocaleString() : '-';
            const duration = tr.total_duration_ms ? `${tr.total_duration_ms}ms` : '-';
            const tokenCount = tr.total_tokens || 0;
            return `
              <div class="flex items-center gap-3 py-1 text-xs border-b border-border-subtle last:border-0">
                <span class="text-text-quaternary tabular-nums shrink-0">${time}</span>
                <span class="text-text-secondary truncate flex-1">${tr.query || '-'}</span>
                <span class="text-text-tertiary shrink-0">${duration}</span>
                <span class="text-text-quaternary shrink-0">${tokenCount} tok</span>
                <button class="trace-detail-btn text-accent hover:underline shrink-0" data-trace-id="${tr.id || ''}" data-i18n="trace.detail">详情</button>
              </div>
            `;
          }).join('')}
        </div>
      ` : ''}

      <!-- Token 预算 -->
      <h3 class="text-sm font-semibold m-0 mb-4 mt-5 flex items-center gap-1">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
             stroke-linecap="round" stroke-linejoin="round" style="display:inline; vertical-align:middle; margin-right:6px;">
          <rect x="2" y="5" width="20" height="14" rx="2"/>
          <line x1="2" y1="10" x2="22" y2="10"/>
        </svg>
        <span data-i18n="trace.budget_title">Token 预算</span>
      </h3>

      <!-- 日用量进度条 -->
      <div class="py-2">
        <div class="flex items-center justify-between mb-1">
          <span class="text-sm text-text-secondary" data-i18n="trace.daily_usage">今日用量</span>
          <span class="text-xs text-text-quaternary">$${dailySpent.toFixed(4)} / $${dailyLimit.toFixed(2)}</span>
        </div>
        <div class="h-2 bg-surface-3 rounded-full overflow-hidden">
          <div class="h-full rounded-full transition-all ${usagePct > 80 ? 'bg-danger' : usagePct > 50 ? 'bg-warning' : 'bg-success'}" style="width:${usagePct}%"></div>
        </div>
        <div class="flex justify-between text-[10px] text-text-quaternary mt-1">
          <span>${usagePct}%</span>
          <span data-i18n="trace.monthly_usage">月累计</span>: $${monthlySpent.toFixed(4)}
        </div>
      </div>

      <!-- 日限额设置 -->
      <div class="flex items-center justify-between py-2 border-t border-border-default">
        <div class="flex flex-col gap-0.5 flex-1">
          <span data-i18n="trace.daily_limit">日限额 (USD)</span>
          <span class="text-xs text-text-tertiary leading-tight" data-i18n="trace.daily_limit_desc">超过后拒绝 LLM 调用</span>
        </div>
        <div class="shrink-0 ml-4">
          <input type="number" id="budgetDailyLimitInput" class="px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                 min="0" max="100" step="0.5"
                 value="${dailyLimit}"
                 style="width: 80px;"/>
        </div>
      </div>

      <!-- Token 预算配置 -->
      <div class="py-2 border-t border-border-default space-y-3">
        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="trace.max_tokens">上下文窗口限制</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="trace.max_tokens_desc">超出触发上下文压缩</span>
          </div>
          <input type="number" id="budgetMaxTokensInput" class="shrink-0 ml-4 px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                 min="4096" max="131072" step="1024"
                 value="${maxTokens}"
                 style="width: 100px;"/>
        </div>

        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="trace.threshold">压缩阈值</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="trace.threshold_desc">达到窗口的多少比例时触发压缩</span>
          </div>
          <input type="number" id="budgetThresholdInput" class="shrink-0 ml-4 px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                 min="0.5" max="0.95" step="0.05"
                 value="${threshold}"
                 style="width: 70px;"/>
        </div>

        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="trace.keep_ratio">保留比例</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="trace.keep_ratio_desc">压缩后保留最近消息的比例</span>
          </div>
          <input type="number" id="budgetKeepRatioInput" class="shrink-0 ml-4 px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                 min="0.2" max="0.9" step="0.05"
                 value="${keepRatio}"
                 style="width: 70px;"/>
        </div>

        <div class="flex items-center justify-between">
          <div class="flex flex-col gap-0.5 flex-1">
            <span data-i18n="trace.min_msgs">最小压缩消息数</span>
            <span class="text-xs text-text-tertiary leading-tight" data-i18n="trace.min_msgs_desc">消息数不足时不触发压缩</span>
          </div>
          <input type="number" id="budgetMinMsgsInput" class="shrink-0 ml-4 px-2.5 py-1 text-[13px] border border-border-default rounded-md bg-bg-input text-text-primary outline-none transition-colors focus:border-primary focus:shadow-[0_0_0_3px_rgba(var(--info-rgb),0.15)]"
                 min="2" max="50" step="1"
                 value="${minMsgs}"
                 style="width: 60px;"/>
        </div>

        <button id="btnSaveBudgetConfig" class="w-full text-xs px-3 py-2 rounded-lg border border-border-default text-text-tertiary hover:text-text-secondary hover:bg-surface-3 transition-colors" data-i18n="trace.save_config">保存配置</button>
      </div>
    </div>
  `;

  initTraceBudgetHandlers();
}

/**
 * 初始化 Trace + Budget 事件处理器。
 */
function initTraceBudgetHandlers() {
  // 查看最近 trace
  const btnView = $('btnViewTraces');
  if (btnView) {
    btnView.onclick = async () => {
      try {
        const traces = await traceApi.getRecent(20);
        showTraceListDialog(traces);
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 清空 trace
  const btnClear = $('btnClearTraces');
  if (btnClear) {
    btnClear.onclick = async () => {
      const confirmed = await showConfirmDialog({
        title: t('trace.clear_confirm', '确认清空所有追踪记录？'),
      });
      if (!confirmed) return;
      try {
        await traceApi.clear();
        toastSuccess(t('trace.cleared', '追踪记录已清空'));
        await renderTraceBudgetSettings($('traceBudgetContainer'));
      } catch (err) {
        toastError(err);
      }
    };
  }

  // Trace 详情按钮
  document.querySelectorAll('.trace-detail-btn').forEach((btn) => {
    btn.onclick = async () => {
      const id = btn.dataset.traceId;
      if (!id) return;
      try {
        const detail = await traceApi.getDetail(id);
        if (detail) showTraceDetailDialog(detail);
      } catch (err) {
        toastError(err);
      }
    };
  });

  // 保存预算配置
  const btnSave = $('btnSaveBudgetConfig');
  if (btnSave) {
    btnSave.onclick = async () => {
      const maxTokens = parseInt($('budgetMaxTokensInput')?.value, 10) || 32768;
      const threshold = parseFloat($('budgetThresholdInput')?.value) || 0.8;
      const keepRatio = parseFloat($('budgetKeepRatioInput')?.value) || 0.67;
      const minMsgs = parseInt($('budgetMinMsgsInput')?.value, 10) || 3;
      try {
        await budgetApi.setConfig({
          max_tokens: maxTokens,
          compaction_threshold: threshold,
          recent_keep_ratio: keepRatio,
          min_messages_to_compact: minMsgs,
        });
        toastSuccess(t('trace.config_saved', '配置已保存'));
      } catch (err) {
        toastError(err);
      }
    };
  }

  // 日限额设置
  const limitInput = $('budgetDailyLimitInput');
  if (limitInput) {
    limitInput.onchange = async () => {
      const limit = parseFloat(limitInput.value) || 0;
      try {
        await budgetApi.setLimit(limit);
        toast(t('trace.daily_limit_set', '日限额已设置') + ': $' + limit.toFixed(2), 'success');
      } catch (err) {
        toastError(err);
      }
    };
  }
}

/**
 * 显示 trace 列表对话框。
 * @param {Array} traces - trace 记录列表
 */
function showTraceListDialog(traces) {
  const dialog = document.createElement('div');
  dialog.className = 'fixed inset-0 flex items-center justify-center bg-black/50 backdrop-blur-[4px]';
  dialog.style.zIndex = '9000';
  dialog.innerHTML = `
    <div class="bg-bg-primary rounded-2xl shadow-modal max-w-[700px] w-[90%] max-h-[85vh] overflow-y-auto">
      <h3 class="text-lg font-semibold px-6 pt-6 m-0 flex items-center gap-2" data-i18n="trace.list_title"><svg class="icon-md shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg> RAG 链路追踪</h3>
      <div class="px-6 py-4">
        ${traces.length === 0 ? `
          <p class="text-sm text-text-quaternary text-center py-8" data-i18n="trace.no_records">暂无追踪记录</p>
        ` : traces.map(tr => {
          const time = tr.created_at ? new Date(tr.created_at).toLocaleString() : '-';
          return `
            <div class="bg-surface-2 rounded-lg p-3 mb-2 cursor-pointer hover:bg-surface-3 transition-colors trace-list-item" data-trace-id="${tr.id || ''}">
              <div class="flex items-center justify-between mb-1">
                <span class="text-xs text-text-quaternary tabular-nums">${time}</span>
                <span class="text-xs text-text-tertiary">${tr.total_duration_ms || 0}ms · ${tr.total_tokens || 0} tokens</span>
              </div>
              <p class="text-sm text-text-primary m-0 truncate">${tr.query || '-'}</p>
            </div>
          `;
        }).join('')}
      </div>
      <div class="flex justify-end px-6 pb-6">
        <button class="px-3.5 py-2.5 text-sm font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" id="traceListCloseBtn" data-i18n="common.close">关闭</button>
      </div>
    </div>
  `;
  document.body.appendChild(dialog);

  dialog.querySelector('#traceListCloseBtn').onclick = () => dialog.remove();
  dialog.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') dialog.remove();
  });

  dialog.querySelectorAll('.trace-list-item').forEach((item) => {
    item.onclick = async () => {
      const id = item.dataset.traceId;
      if (!id) return;
      try {
        const detail = await traceApi.getDetail(id);
        if (detail) {
          dialog.remove();
          showTraceDetailDialog(detail);
        }
      } catch (err) {
        toastError(err);
      }
    };
  });
}

/**
 * 显示单条 trace 详情对话框。
 * @param {Object} detail - trace 记录详情
 */
function showTraceDetailDialog(detail) {
  const dialog = document.createElement('div');
  dialog.className = 'fixed inset-0 flex items-center justify-center bg-black/50 backdrop-blur-[4px]';
  dialog.style.zIndex = '9000';
  const time = detail.created_at ? new Date(detail.created_at).toLocaleString() : '-';

  dialog.innerHTML = `
    <div class="bg-bg-primary rounded-2xl shadow-modal max-w-[600px] w-[90%] max-h-[85vh] overflow-y-auto">
      <h3 class="text-lg font-semibold px-6 pt-6 m-0 flex items-center gap-2" data-i18n="trace.detail_title"><svg class="icon-md shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg> 追踪详情</h3>
      <div class="px-6 py-4 space-y-3">
        <div>
          <span class="text-xs text-text-quaternary" data-i18n="trace.time">时间</span>
          <p class="text-sm text-text-primary m-0">${time}</p>
        </div>
        <div>
          <span class="text-xs text-text-quaternary" data-i18n="trace.query">查询</span>
          <p class="text-sm text-text-primary m-0">${detail.query || '-'}</p>
        </div>
        <div class="flex gap-4">
          <div>
            <span class="text-xs text-text-quaternary" data-i18n="trace.duration">耗时</span>
            <p class="text-sm text-text-primary m-0">${detail.total_duration_ms || 0}ms</p>
          </div>
          <div>
            <span class="text-xs text-text-quaternary" data-i18n="trace.tokens">Token 数</span>
            <p class="text-sm text-text-primary m-0">${detail.total_tokens || 0}</p>
          </div>
          <div>
            <span class="text-xs text-text-quaternary" data-i18n="trace.retrieval_count">检索结果数</span>
            <p class="text-sm text-text-primary m-0">${detail.retrieval_count || 0}</p>
          </div>
        </div>
        ${detail.stages ? `
          <div>
            <span class="text-xs text-text-quaternary" data-i18n="trace.stages">阶段</span>
            <div class="mt-1 space-y-1">
              ${detail.stages.map(s => `
                <div class="flex items-center gap-2 text-xs">
                  <span class="text-text-secondary">${s.name || '-'}</span>
                  <span class="text-text-tertiary">${s.duration_ms || 0}ms</span>
                </div>
              `).join('')}
            </div>
          </div>
        ` : ''}
        ${detail.error ? `
          <div class="text-sm text-danger bg-[rgba(var(--danger-rgb),0.08)] px-3 py-2 rounded-lg">
            <span data-i18n="trace.error">错误</span>: ${detail.error}
          </div>
        ` : ''}
      </div>
      <div class="flex justify-end px-6 pb-6">
        <button class="px-3.5 py-2.5 text-sm font-medium rounded-md bg-bg-secondary text-text-primary border border-border-default cursor-pointer hover:bg-bg-hover transition-colors" id="traceDetailCloseBtn" data-i18n="common.close">关闭</button>
      </div>
    </div>
  `;
  document.body.appendChild(dialog);

  dialog.querySelector('#traceDetailCloseBtn').onclick = () => dialog.remove();
  dialog.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') dialog.remove();
  });
}
