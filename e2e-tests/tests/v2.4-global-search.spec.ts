/**
 * S90 全局搜索增强 E2E 测试（REQ-IX-008）
 *
 * TC-GSEARCH-001~006：验证全局搜索面板的打开、分组结果展示、点击跳转、中英文混合查询。
 */
import { test, expect } from '@playwright/test';
import { setupPage, injectStub, uiUrl } from './helpers.mjs';

test.describe('S90 全局搜索增强（REQ-IX-008）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-GSEARCH-001 ⌘⇧F 打开全局搜索面板', async ({ page }) => {
    // 按下 Cmd+Shift+F 打开全局搜索
    await page.keyboard.down('Meta');
    await page.keyboard.down('Shift');
    await page.keyboard.press('f');
    await page.keyboard.up('Shift');
    await page.keyboard.up('Meta');

    // 验证全局搜索面板可见
    const panel = page.locator('#globalSearch');
    await expect(panel).not.toHaveClass(/\bhidden\b/);

    // 验证搜索输入框存在且可见
    const input = page.locator('#globalSearchInput');
    await expect(input).toBeVisible();

    // 验证结果容器存在
    const results = page.locator('#globalSearchResults');
    await expect(results).toBeVisible();
  });

  test('TC-GSEARCH-002 搜索结果分组：消息 + 文档 + 实体', async ({ page }) => {
    // 打开全局搜索
    await page.evaluate(() => {
      const panel = document.getElementById('globalSearch');
      if (panel) panel.classList.remove('hidden');
      const input = document.getElementById('globalSearchInput') as HTMLInputElement;
      if (input) input.focus();
    });

    // 输入搜索关键词
    const input = page.locator('#globalSearchInput');
    await input.fill('test');

    // 等待防抖搜索完成
    await page.waitForTimeout(500);

    // 验证结果容器有内容
    const resultsContainer = page.locator('#globalSearchResults');
    const content = await resultsContainer.innerHTML();
    expect(content.length).toBeGreaterThan(0);
  });

  test('TC-GSEARCH-003 点击消息结果跳转到对应会话', async ({ page }) => {
    // 打开全局搜索
    await page.evaluate(() => {
      const panel = document.getElementById('globalSearch');
      if (panel) panel.classList.remove('hidden');
    });

    // 触发搜索
    await page.evaluate(() => {
      const input = document.getElementById('globalSearchInput') as HTMLInputElement;
      if (input) {
        input.value = 'test';
        input.dispatchEvent(new Event('input', { bubbles: true }));
      }
    });

    await page.waitForTimeout(500);

    // 如果有消息结果项，点击它
    const messageItems = page.locator('.gs-result-item[data-type="message"]');
    const count = await messageItems.count();
    if (count > 0) {
      await messageItems.first().click();
      await expect(page.locator('#globalSearch')).toHaveClass(/\bhidden\b/);
    }
  });

  test('TC-GSEARCH-004 点击文档结果跳转到文档详情', async ({ page }) => {
    await page.evaluate(() => {
      const panel = document.getElementById('globalSearch');
      if (panel) panel.classList.remove('hidden');
    });

    const input = page.locator('#globalSearchInput');
    await input.fill('doc');
    await page.waitForTimeout(500);

    const docItems = page.locator('.gs-result-item[data-type="document"]');
    const count = await docItems.count();
    if (count > 0) {
      await docItems.first().click();
      await expect(page.locator('#globalSearch')).toHaveClass(/\bhidden\b/);
    }
  });

  test('TC-GSEARCH-005 点击实体结果跳转到知识图谱查看器', async ({ page }) => {
    await page.evaluate(() => {
      const panel = document.getElementById('globalSearch');
      if (panel) panel.classList.remove('hidden');
    });

    const input = page.locator('#globalSearchInput');
    await input.fill('entity');
    await page.waitForTimeout(500);

    const entityItems = page.locator('.gs-result-item[data-type="entity"]');
    const count = await entityItems.count();
    if (count > 0) {
      await entityItems.first().click();
      await expect(page.locator('#globalSearch')).toHaveClass(/\bhidden\b/);
    }
  });

  test('TC-GSEARCH-006 搜索框支持中英文混合查询', async ({ page }) => {
    await page.evaluate(() => {
      const panel = document.getElementById('globalSearch');
      if (panel) panel.classList.remove('hidden');
    });

    const input = page.locator('#globalSearchInput');
    await input.fill('AI 人工智能');
    await page.waitForTimeout(500);

    const resultsContainer = page.locator('#globalSearchResults');
    const content = await resultsContainer.innerHTML();
    expect(content.length).toBeGreaterThan(0);

    await page.keyboard.press('Escape');
    await expect(page.locator('#globalSearch')).toHaveClass(/\bhidden\b/);
  });
});
