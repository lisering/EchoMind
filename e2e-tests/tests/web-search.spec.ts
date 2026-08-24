// E2E 网页搜索集成测试（REQ-RAG-036）：
// S6 P1-3 变更：网页搜索从输入区 toggle 改为 /web 斜杠命令临时启用。
// 验证 /web 斜杠命令启用网页搜索 + IPC 调用 + 状态同步。
//
// TC-WEB-SEARCH-001: /web 斜杠命令可用
// TC-WEB-SEARCH-002: /web 命令临时启用网页搜索
// TC-WEB-SEARCH-003: 网页搜索发送后恢复为关闭
// TC-WEB-SEARCH-004: 网页搜索独立于混合搜索
// TC-WEB-SEARCH-005: 启用网页搜索后 IPC web_search 命令可调用
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, importDocs, sendMessage, injectStub, uiUrl, waitForStreamDone, setupPage } from './helpers.mjs';

test.describe('TC-WEB-SEARCH 网页搜索集成（REQ-RAG-036）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await importDocs(page, ['/mock/test.md']);
  });

  // ─── /web 斜杠命令可用 ───

  test('TC-WEB-SEARCH-001 /web 斜杠命令可用', async ({ page }) => {
    const input = page.locator('#queryInput');
    await input.fill('/web');
    await page.waitForTimeout(300);

    // 斜杠命令面板应显示（class-based, not ID）
    const slashPanel = page.locator('.slash-command-panel');
    await expect(slashPanel).toBeVisible({ timeout: 5000 });

    // 应找到 web 命令项
    const webCmd = page.locator('.slash-command-item:has-text("web"), .slash-command-item:has-text("Web"), .slash-command-item:has-text("搜索")');
    const count = await webCmd.count();
    expect(count).toBeGreaterThan(0);
  });

  // ─── /web 命令临时启用网页搜索 ───

  test('TC-WEB-SEARCH-002 /web 命令临时启用网页搜索', async ({ page }) => {
    // 使用 /web 命令 + 查询
    const input = page.locator('#queryInput');
    await input.fill('/web test query');
    await page.waitForTimeout(200);

    // 提交
    await page.locator('#sendBtn').click();

    // mock 状态应同步为 web search 启用
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // 等待流完成
    await waitForStreamDone(page);
  });

  // ─── 网页搜索发送后恢复为关闭 ───

  test('TC-WEB-SEARCH-003 网页搜索发送后恢复为关闭', async ({ page }) => {
    const input = page.locator('#queryInput');
    await input.fill('/web test query');
    await page.waitForTimeout(200);
    await page.locator('#sendBtn').click();

    // 发送时 web search 应启用
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // 等待流完成后应恢复为 false
    await waitForStreamDone(page);
    await page.waitForTimeout(1000);
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(false);
  });

  // ─── 网页搜索独立于混合搜索 ───

  test('TC-WEB-SEARCH-004 网页搜索独立于混合搜索', async ({ page }) => {
    // 记录 hybrid 初始状态
    const hybridInitial = await page.evaluate(() => window.__mock.state.hybridSearch);

    // 使用 /web 命令
    const input = page.locator('#queryInput');
    await input.fill('/web test query');
    await page.waitForTimeout(200);
    await page.locator('#sendBtn').click();

    // web search 应启用
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // hybrid 应保持其初始状态不变
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(hybridInitial);

    await waitForStreamDone(page);
    await page.waitForTimeout(1000);
    // 流完成后 web search 恢复为 false
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(false);
    // hybrid 仍保持
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(hybridInitial);
  });

  // ─── 启用后 IPC 不崩溃 ───

  test('TC-WEB-SEARCH-005 启用网页搜索后 IPC web_search 命令可调用', async ({ page }) => {
    // 通过 /web 启用网页搜索
    const input = page.locator('#queryInput');
    await input.fill('/web test query');
    await page.waitForTimeout(200);
    await page.locator('#sendBtn').click();

    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // 直接调用 web_search IPC 命令验证不崩溃
    const results = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('web_search', { query: 'test query' });
    });
    expect(results).toBeDefined();
    expect(Array.isArray(results)).toBe(true);

    await waitForStreamDone(page);
  });
});
