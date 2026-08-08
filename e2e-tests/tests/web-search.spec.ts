// E2E 网页搜索集成测试（REQ-RAG-036）：
// 验证 web_search toggle 开关 UI 交互 + IPC 调用 + 状态同步。
//
// TC-WEB-SEARCH-001: 输入区网页搜索 toggle 可见
// TC-WEB-SEARCH-002: 启用网页搜索——UI 交互 + IPC 状态同步
// TC-WEB-SEARCH-003: 禁用网页搜索——UI 交互 + IPC 状态同步
// TC-WEB-SEARCH-004: 网页搜索 toggle 独立于其他 toggle
// TC-WEB-SEARCH-005: 启用网页搜索后对话不崩溃
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, importDocs, sendMessage, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('TC-WEB-SEARCH 网页搜索集成（REQ-RAG-036）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── Toggle 可见性 ───

  test('TC-WEB-SEARCH-001 输入区网页搜索 toggle 可见', async ({ page }) => {
    const togglesContainer = page.locator('#inputToggles');
    await expect(togglesContainer).toBeVisible({ timeout: 5000 });

    // 查找包含 web_search settingKey 的 toggle
    const webSearchToggle = page.locator('[data-setting-key="web_search"]');
    await expect(webSearchToggle).toBeVisible({ timeout: 5000 });

    // 验证 toggle 有 role="switch" 属性
    await expect(webSearchToggle).toHaveAttribute('role', 'switch');
    // 默认关闭
    await expect(webSearchToggle).toHaveAttribute('aria-checked', 'false');
  });

  // ─── 启用网页搜索 ───

  test('TC-WEB-SEARCH-002 启用网页搜索——UI 交互 + IPC 状态同步', async ({ page }) => {
    const webSearchToggle = page.locator('[data-setting-key="web_search"]');
    await expect(webSearchToggle).toHaveAttribute('aria-checked', 'false');

    // 点击启用
    await webSearchToggle.click();

    // mock 状态应同步
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);
    // UI 选中态
    await expect(webSearchToggle).toHaveAttribute('aria-checked', 'true');
  });

  // ─── 禁用网页搜索 ───

  test('TC-WEB-SEARCH-003 禁用网页搜索——UI 交互 + IPC 状态同步', async ({ page }) => {
    const webSearchToggle = page.locator('[data-setting-key="web_search"]');

    // 先启用
    await webSearchToggle.click();
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // 再禁用
    await webSearchToggle.click();
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(false);
    await expect(webSearchToggle).toHaveAttribute('aria-checked', 'false');
  });

  // ─── Toggle 独立性 ───

  test('TC-WEB-SEARCH-004 网页搜索 toggle 独立于其他 toggle', async ({ page }) => {
    const webSearchToggle = page.locator('[data-setting-key="web_search"]');

    // 记录 hybrid 初始状态
    const hybridInitial = await page.evaluate(() => window.__mock.state.hybridSearch);

    // 启用 web search
    await webSearchToggle.click();
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // hybrid 应保持其初始状态不变
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(hybridInitial);

    // 禁用 web search，hybrid 仍保持
    await webSearchToggle.click();
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(false);
    expect(await page.evaluate(() => window.__mock.state.hybridSearch)).toBe(hybridInitial);
  });

  // ─── 启用后对话不崩溃 ───

  test('TC-WEB-SEARCH-005 启用网页搜索后 IPC web_search 命令可调用', async ({ page }) => {
    // 启用网页搜索
    const webSearchToggle = page.locator('[data-setting-key="web_search"]');
    await webSearchToggle.click();
    expect(await page.evaluate(() => window.__mock.state.webSearchEnabled)).toBe(true);

    // 直接调用 web_search IPC 命令验证不崩溃
    const results = await page.evaluate(async () => {
      return await window.__TAURI__.core.invoke('web_search', { query: 'test query' });
    });
    expect(results).toBeDefined();
    expect(Array.isArray(results)).toBe(true);
    expect(results.length).toBeGreaterThan(0);
    // 验证搜索结果格式
    expect(results[0]).toHaveProperty('title');
    expect(results[0]).toHaveProperty('url');
    expect(results[0]).toHaveProperty('snippet');
    expect(results[0]).toHaveProperty('source');
  });
});
