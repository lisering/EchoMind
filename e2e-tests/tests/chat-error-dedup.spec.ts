// E2E 测试：聊天错误双重 toast 消除（Bug 修复验证）
//
// Bug 根因：chat 命令包装器同时 emit chat_error 事件 + return Err，
// 前端 chat_error 监听器和 invoke().catch() 各显示一次 toast → 用户看到两个重复错误。
//
// 修复方案：
// 1. 后端：chat 命令不再 emit chat_error（Err 返回值由 invoke().catch() 处理）
// 2. 前端：_chatErrorHandled 去重标志（安全网，防止任何残余双重报告）
// 3. 后端：chat_inner 所有错误路径补全前缀（STORAGE/EMBED/LLM/VALIDATION），
//    消除「未知错误」问题
//
// 测试矩阵：
// TC-DEDUP-001: LLM 错误 → 只显示一个 toast
// TC-DEDUP-002: EMBED 错误 → 只显示一个 toast
// TC-DEDUP-003: NETWORK 错误 → 只显示一个 toast + 正确显示「网络连接异常」
// TC-DEDUP-004: 错误后 UI 正确恢复（输入框可用、停止按钮隐藏）
// TC-DEDUP-005: 空知识库错误 → 显示具体原因而非「未知错误」

import { test, expect } from '@playwright/test';
import { setupPage, importDocs, sendMessage, waitForStreamDone, waitForToastsClear } from './helpers.mjs';

test.describe('聊天错误双重 toast 消除', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 导入文档（chat 前置条件）
    await importDocs(page, ['/mock/test.md']);
    // 等待导入 toast 消失，避免干扰错误 toast 计数
    await waitForToastsClear(page);
  });

  test('TC-DEDUP-001 LLM 错误只显示一个 toast', async ({ page }) => {
    // 设置 chat 返回 LLM 错误
    await page.evaluate(() => window.__mock.setChatError('LLM: 模型调用失败（HTTP 500）'));

    // 发送消息
    await sendMessage(page, '请解释一下Lisp的原理');

    // 等待错误 toast 出现
    await expect(page.locator('#toasts')).toContainText('LLM 服务异常', { timeout: 5000 });

    // 等待 UI 恢复
    await waitForStreamDone(page, 5000);

    // 关键断言：只应有 1 个错误 toast（修复前会有 2 个）
    const errorToasts = await page.locator('#toasts > div').count();
    expect(errorToasts, 'LLM 错误应只显示一个 toast（修复前会显示两个）').toBe(1);
  });

  test('TC-DEDUP-002 EMBED 错误只显示一个 toast', async ({ page }) => {
    // 设置 chat 模拟 embedder 初始化失败
    await page.evaluate(() => window.__mock.setChatEmbedderError());

    // 发送消息
    await sendMessage(page, '请解释一下Lisp的原理');

    // 等待 UI 恢复
    await waitForStreamDone(page, 8000);

    // 关键断言：只应有 1 个错误 toast
    const errorToasts = await page.locator('#toasts > div').count();
    expect(errorToasts, 'EMBED 错误应只显示一个 toast（修复前会显示两个）').toBe(1);
  });

  test('TC-DEDUP-003 NETWORK 错误只显示一个 toast 且文案正确', async ({ page }) => {
    // 设置 chat 返回 NETWORK 错误
    await page.evaluate(() => window.__mock.setChatError('NETWORK: connection refused'));

    // 发送消息
    await sendMessage(page, '测试网络错误');

    // 等待 toast 出现
    await expect(page.locator('#toasts')).toContainText('网络连接异常', { timeout: 5000 });

    // 等待 UI 恢复
    await waitForStreamDone(page, 5000);

    // 关键断言：只应有 1 个错误 toast
    const errorToasts = await page.locator('#toasts > div').count();
    expect(errorToasts, 'NETWORK 错误应只显示一个 toast').toBe(1);
  });

  test('TC-DEDUP-004 错误后 UI 正确恢复', async ({ page }) => {
    // 设置 chat 返回错误
    await page.evaluate(() => window.__mock.setChatError('AUTH: API Key 无效'));

    // 发送消息
    await sendMessage(page, '测试错误恢复');

    // 等待 UI 恢复
    await waitForStreamDone(page, 5000);

    // 输入框应恢复可用
    await expect(page.locator('#queryInput')).not.toBeDisabled();
    // 发送按钮应可见
    await expect(page.locator('#sendBtn')).toBeVisible();
    // 发送按钮应退出停止形态（发送/停止合二为一）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/);
  });

  test('TC-DEDUP-005 STORAGE 错误显示「存储异常」而非「未知错误」', async ({ page }) => {
    // 设置 chat 返回 STORAGE 错误（修复前这类错误无前缀，会显示「未知错误」）
    await page.evaluate(() => window.__mock.setChatError('STORAGE: 数据库写入失败'));

    // 发送消息
    await sendMessage(page, '测试存储错误');

    // 等待 toast 出现
    await expect(page.locator('#toasts')).toContainText('存储异常', { timeout: 5000 });

    // 等待 UI 恢复
    await waitForStreamDone(page, 5000);

    // 关键断言：不应显示「未知错误」
    const toastText = await page.locator('#toasts').textContent();
    expect(toastText, 'STORAGE 错误不应显示「未知错误」').not.toContain('未知错误');

    // 只应有 1 个 toast
    const errorToasts = await page.locator('#toasts > div').count();
    expect(errorToasts, 'STORAGE 错误应只显示一个 toast').toBe(1);
  });
});
