/**
 * EchoMind 检索质量自学习反馈采集模块（REQ-PERF-012 扩展）。
 *
 * 借鉴 StoryMoss 持续学习机制：通过观察用户行为自动蒸馏检索效果，无需显式标注。
 *
 * 职责：
 * 1. 跟踪上一轮查询文本 + 推断的检索方法
 * 2. 检测隐式反馈信号：
 *    - 用户重新提问（相似度 > 0.6）→ RetryWithDifferentMethod（负信号）
 *    - 用户编辑后重发 → EditAndResend（负信号）
 *    - 用户继续新话题（相似度 < 0.3）→ Accepted（正信号，针对上一轮）
 * 3. 显式反馈信号：
 *    - 用户点赞 → ThumbsUp（强正信号）
 *    - 用户点踩 → ThumbsDown（强负信号）
 * 4. 通过 IPC 上报到后端 record_retrieval_feedback 命令
 *
 * 检索方法推断逻辑（与 chat_inner 中一致）：
 * - hybrid=false → vector_only
 * - hybrid=true + rerank=true → hybrid_rerank
 * - hybrid=true + rerank=false → hybrid
 */

import { invoke } from './ipc.js';
import { getState } from './state.js';
import { getToggleState } from './input-toggles.js';

// ============================================================
// 状态跟踪
// ============================================================

/** 上一轮查询文本（用于相似度比较） */
let _lastQuery = null;

/** 上一轮是否为编辑重发模式 */
let _lastWasEdit = false;

// ============================================================
// 检索方法推断
// ============================================================

/**
 * 推断当前使用的检索方法（与后端 chat_inner 逻辑一致）。
 *
 * @returns {string} 检索方法标识：'vector_only' | 'hybrid' | 'hybrid_rerank'
 */
export function inferRetrievalMethod() {
  const hybridEnabled = getToggleState('hybrid');
  const rerankEnabled = getState().rerankEnabled || false;

  if (!hybridEnabled) return 'vector_only';
  if (rerankEnabled) return 'hybrid_rerank';
  return 'hybrid';
}

// ============================================================
// 文本相似度（简单 Jaccard，零依赖）
// ============================================================

/**
 * 计算两个查询文本的相似度（Jaccard 词重叠率）。
 *
 * @param {string} a - 查询 A
 * @param {string} b - 查询 B
 * @returns {number} 相似度 0.0-1.0
 */
export function textSimilarity(a, b) {
  if (!a || !b) return 0.0;
  const tokensA = new Set(a.toLowerCase().split(/\s+|(?<=[\u4e00-\u9fff])|(?=[\u4e00-\u9fff])/).filter(Boolean));
  const tokensB = new Set(b.toLowerCase().split(/\s+|(?<=[\u4e00-\u9fff])|(?=[\u4e00-\u9fff])/).filter(Boolean));
  if (tokensA.size === 0 || tokensB.size === 0) return 0.0;
  let intersection = 0;
  for (const t of tokensA) {
    if (tokensB.has(t)) intersection++;
  }
  const union = tokensA.size + tokensB.size - intersection;
  return union > 0 ? intersection / union : 0.0;
}

// ============================================================
// IPC 上报
// ============================================================

/**
 * 上报用户反馈信号到后端。
 *
 * @param {string} query - 用户查询文本
 * @param {string} method - 检索方法标识
 * @param {string} feedbackType - 反馈类型：'retry_with_different_method' | 'edit_and_resend' | 'accepted' | 'thumbs_up' | 'thumbs_down'
 * @returns {Promise<void>}
 */
export async function reportFeedback(query, method, feedbackType) {
  try {
    await invoke('record_retrieval_feedback', {
      signal: {
        query,
        method,
        feedback: feedbackType,
        timestamp: Math.floor(Date.now() / 1000),
      },
    });
  } catch (e) {
    // 静默失败：反馈采集不应影响用户正常使用
    console.debug('[retrieval-feedback] 上报失败（静默）:', e);
  }
}

// ============================================================
// 隐式反馈检测（在 send() 调用前调用）
// ============================================================

/**
 * 检测并上报隐式反馈信号。
 *
 * 在用户发送新查询时调用，根据与上一轮查询的相似度推断反馈类型：
 * - 相似度 > 0.6 → RetryWithDifferentMethod（负信号，上一轮不满意）
 * - 相似度 < 0.3 → Accepted（正信号，上一轮满意）
 * - 0.3-0.6 之间 → 无信号（中性，不上报）
 *
 * @param {string} newQuery - 新查询文本
 * @param {boolean} isEdit - 是否为编辑重发模式
 */
export function detectAndReportImplicit(newQuery, isEdit = false) {
  if (!_lastQuery) {
    // 首次查询，无上一轮可比
    _lastQuery = newQuery;
    _lastWasEdit = isEdit;
    return;
  }

  const method = inferRetrievalMethod();
  const similarity = textSimilarity(_lastQuery, newQuery);

  if (isEdit || _lastWasEdit) {
    // 编辑重发 → 强负信号
    reportFeedback(_lastQuery, method, 'edit_and_resend');
  } else if (similarity > 0.6) {
    // 相似查询重新提问 → 负信号
    reportFeedback(_lastQuery, method, 'retry_with_different_method');
  } else if (similarity < 0.3) {
    // 完全不同的话题 → 上一轮被接受（正信号）
    reportFeedback(_lastQuery, method, 'accepted');
  }
  // 0.3-0.6 之间：中性，不上报

  // 更新跟踪状态
  _lastQuery = newQuery;
  _lastWasEdit = isEdit;
}

/**
 * 显式反馈：用户点赞。
 *
 * @param {string} query - 被点赞回答对应的查询文本
 */
export function reportThumbsUp(query) {
  const method = inferRetrievalMethod();
  reportFeedback(query, method, 'thumbs_up');
}

/**
 * 显式反馈：用户点踩。
 *
 * @param {string} query - 被点踩回答对应的查询文本
 */
export function reportThumbsDown(query) {
  const method = inferRetrievalMethod();
  reportFeedback(query, method, 'thumbs_down');
}

/**
 * 重置反馈跟踪状态（新对话时调用）。
 */
export function resetFeedbackTracking() {
  _lastQuery = null;
  _lastWasEdit = false;
}
