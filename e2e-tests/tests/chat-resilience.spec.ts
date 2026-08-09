// E2E 对话韧性测试 — 验证前端在后端异常时的恢复能力
//
// 根因：chat_inner 中 embedder 初始化无超时保护，当 ONNX 模型下载因网络问题挂起时
// （如 HuggingFace 在境内不可达），chat 命令永不返回，前端永久停留在「初始化向量化引擎」。
// E2E 测试全量使用 tauri-stub.js mock 后端，从未测试此后端挂起场景。
//
// TC-RES-001: 后端永久挂起 → 客户端看门狗超时 → UI 恢复到 error 状态
// TC-RES-002: embedder 初始化失败 → chat_error 事件 → UI 恢复到 error 状态
// TC-RES-003: 看门狗恢复后正常对话仍可用
// TC-RES-004: 正常流式对话期间看门狗不误触发
import { test, expect } from '@playwright/test';
import { importDocs, injectLocales, injectStub, uiUrl, waitForStreamDone, sendMessage } from './helpers.mjs';

test.describe('E2E-RES 对话韧性 — 后端异常恢复', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    // 设置短看门狗超时（2 秒），使测试无需等待默认 300 秒
    // 同时预配置 LLM 跳过向导（与 setupPage 一致，更可靠）
    await page.addInitScript(() => {
      window.__ECHOMIND_WATCHDOG_TIMEOUT_MS__ = 2000;
      window.__state.configured = true;
    });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    // 导入文档（chat 命令需要非空知识库）
    await importDocs(page, ['/mock/resilience-test.md']);
  });

  // ─── 后端永久挂起 ───

  test('TC-RES-001 后端永久挂起时看门狗超时恢复', async ({ page }) => {
    // 设置下次 chat 挂起（模拟后端永久阻塞）
    await page.evaluate(() => window.__mock.setChatHang());

    // 发送消息
    await sendMessage(page, '请用图示帮忙解释一下递归');

    // 验证进入 streaming 状态（发送/停止合二为一：sendBtn 变为 stop-mode）
    await expect(page.locator('#sendBtn.stop-mode')).toBeVisible({ timeout: 3000 });

    // 等待看门狗超时（2 秒 + 缓冲）
    // 看门狗触发后应显示错误 toast + 恢复输入框
    // 等待 stop-mode 被移除（看门狗恢复后 finalizeStream 会移除 stop-mode）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 6000 });

    // 验证错误提示出现（toast 或 inputHint）
    const hasError = await page.evaluate(() => {
      const toasts = document.querySelector('#toasts')?.textContent || '';
      const hint = document.querySelector('#inputHint')?.textContent || '';
      return toasts.includes('超时') || toasts.includes('timed out') ||
             hint.includes('超时') || hint.includes('timed out') ||
             toasts.includes('无响应') || hint.includes('无响应');
    });
    expect(hasError, '看门狗超时后应显示包含「超时」或「无响应」的错误提示').toBe(true);

    // 验证输入框恢复可用
    await expect(page.locator('#queryInput')).not.toBeDisabled();
  });

  // ─── embedder 初始化失败 ───

  test('TC-RES-002 embedder 初始化失败时 UI 正确恢复', async ({ page }) => {
    // 设置下次 chat 模拟 embedder 初始化失败
    await page.evaluate(() => window.__mock.setChatEmbedderError());

    // 发送消息
    await sendMessage(page, '请用图示帮忙解释一下递归');

    // 等待 chat_error 事件到达 → UI 恢复
    await expect(page.locator('#sendBtn')).toBeVisible({ timeout: 6000 });

    // 验证错误消息包含 EMBED 前缀
    const hasEmbedError = await page.evaluate(() => {
      const toasts = document.querySelector('#toasts')?.textContent || '';
      const hint = document.querySelector('#inputHint')?.textContent || '';
      return toasts.includes('EMBED') || hint.includes('EMBED') ||
             toasts.includes('向量化引擎') || hint.includes('向量化引擎');
    });
    expect(hasEmbedError, '应显示包含 EMBED 或向量化引擎的错误提示').toBe(true);

    // 验证输入框恢复可用
    await expect(page.locator('#queryInput')).not.toBeDisabled();
  });

  // ─── 恢复后正常对话 ───

  test('TC-RES-003 看门狗恢复后正常对话仍可用', async ({ page }) => {
    // 第一次：触发挂起 → 看门狗恢复
    await page.evaluate(() => window.__mock.setChatHang());
    await sendMessage(page, '第一次问题（会挂起）');
    // 等待看门狗恢复（stop-mode 被移除）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 6000 });

    // 第二次：正常对话
    await sendMessage(page, '第二次问题（正常）');
    // 等待流式完成（stop-mode 被移除表示完成）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 10000 });

    // 验证正常收到回复
    const messages = await page.evaluate(() => {
      const blocks = document.querySelectorAll('.message-in .md');
      return Array.from(blocks).map(b => b.textContent?.trim() || '');
    });
    expect(messages.length, '应有至少 2 条 assistant 回复（第一次空 + 第二次有内容）').toBeGreaterThanOrEqual(2);
    const lastReply = messages[messages.length - 1];
    expect(lastReply.length, '第二次回复应有内容').toBeGreaterThan(0);
  });

  // ─── 正常对话不误触发看门狗 ───

  test('TC-RES-004 正常流式对话期间看门狗不误触发', async ({ page }) => {
    // 正常对话（不设置任何错误/hang 标志）
    await sendMessage(page, '正常对话测试');

    // 等待流式完成（stop-mode 被移除表示完成）
    await expect(page.locator('#sendBtn')).not.toHaveClass(/stop-mode/, { timeout: 15000 });

    // 验证没有超时错误
    const hasTimeoutError = await page.evaluate(() => {
      const toasts = document.querySelector('#toasts')?.textContent || '';
      const hint = document.querySelector('#inputHint')?.textContent || '';
      return toasts.includes('超时') || hint.includes('超时') ||
             toasts.includes('timed out') || hint.includes('timed out');
    });
    expect(hasTimeoutError, '正常对话不应触发看门狗超时').toBeFalsy();

    // 验证有正常回复内容
    const lastMd = await page.evaluate(() => {
      const blocks = document.querySelectorAll('.message-in .md');
      const last = blocks[blocks.length - 1];
      return last?.textContent?.trim() || '';
    });
    expect(lastMd.length, '应有非空的助手回复').toBeGreaterThan(0);
  });
});
