/**
 * EchoMind 统一 Z-index 分层体系。
 *
 * 设计原则：
 * 1. 所有 overlay/modal 面板的 z-index 从此常量获取，禁止硬编码
 * 2. 按"用户交互层级"从低到高排列，同层不允许共存
 * 3. 安全相关面板（锁屏）始终最高优先级
 * 4. 全屏沉浸式面板（图谱）独立层级，不与普通 modal 混用
 *
 * 层级图（从底到顶）：
 *
 *   0          BASE            主界面（侧栏 + 聊天区 + 输入区）
 *   40         WIZARD          启动向导
 *   50         PANEL_1         用户主动打开的主面板（设置/付费墙/代码执行/Dream/符号/工作流/分支树）
 *   55         PANEL_2         叠加在 PANEL_1 之上的确认弹窗（confirm-dialog/VLM 确认）
 *   60         TOAST            Toast 通知（需在所有面板之上但不遮挡安全面板）
 *   65         PANEL_3         下载管理器
 *   70         PANEL_4         DB 错误 Modal
 *   75         PANEL_5         搜索弹框 / 下载恢复
 *   80         COMMAND_PALETTE  命令面板（最高用户交互层）
 *   90         GRAPH_VIEWER     知识图谱全屏查看器
 *   95         AUDIT_LOG        审计日志面板
 *   99999      LOCK_OVERLAY     锁屏遮罩（最高安全优先级）
 */

export const Z_INDEX = {
  /** 主界面（侧栏 + 聊天区 + 输入区） */
  BASE: 0,

  /** 启动向导 */
  WIZARD: 40,

  /** 用户主动打开的主面板：设置 / 付费墙 / 代码执行 / Dream / 符号搜索 / 工作流 / 分支树 */
  PANEL_1: 50,

  /** 叠加在 PANEL_1 之上的确认弹窗：confirm-dialog / VLM 确认 */
  PANEL_2: 55,

  /** Toast 通知 */
  TOAST: 60,

  /** 下载管理器 */
  PANEL_3: 65,

  /** DB 错误 Modal */
  PANEL_4: 70,

  /** 搜索弹框 / 下载恢复 */
  PANEL_5: 75,

  /** 命令面板（最高用户交互层） */
  COMMAND_PALETTE: 80,

  /** 知识图谱全屏查看器（沉浸式，独立层级） */
  GRAPH_VIEWER: 90,

  /** 审计日志面板（安全相关，独立层级） */
  AUDIT_LOG: 95,

  /** 锁屏遮罩（最高安全优先级，不可被任何面板遮挡） */
  LOCK_OVERLAY: 99999,
};

/**
 * 将 Z_INDEX 值转为 Tailwind 任意值类名。
 * 例如：Z_INDEX.PANEL_1 (50) → 'z-50'
 *      Z_INDEX.PANEL_2 (55) → 'z-[55]'
 *
 * @param {number} value - Z_INDEX 常量值
 * @returns {string} Tailwind class 字符串
 */
export function zClass(value) {
  // Tailwind 内置 z-0/10/20/30/40/50，非标准值用任意值语法
  if ([0, 10, 20, 30, 40, 50].includes(value)) {
    return `z-${value}`;
  }
  return `z-[${value}]`;
}
