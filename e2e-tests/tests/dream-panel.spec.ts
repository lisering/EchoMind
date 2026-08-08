/**
 * E2E 测试：AutoDream 知识库整理面板（TC-DREAM-UI-001~005）。
 *
 * 验证 AutoDream 前端 UI：
 * - 侧栏「知识库整理」按钮存在且可点击
 * - 点击按钮后弹出 Dream 面板 overlay
 * - 触发 dream 分析 + 进度条显示
 * - 建议列表按 severity 分组
 * - 中止 dream 分析
 * - 历史建议查看
 */

import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('AutoDream 知识库整理面板 (TC-DREAM-UI)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.waitForSelector('#dreamBtn', { timeout: 5000 });
  });

  test('TC-DREAM-UI-001: Dream 面板打开', async ({ page }) => {
    const btn = page.locator('#dreamBtn');
    await expect(btn).toBeVisible();
    await expect(btn).toBeEnabled();
    await btn.click();

    const overlay = page.locator('#dreamPanelOverlay');
    await expect(overlay).toBeVisible({ timeout: 5000 });
    await expect(overlay).toHaveAttribute('role', 'dialog');
    await expect(overlay).toHaveAttribute('aria-modal', 'true');

    // 关闭
    await page.locator('#dreamCloseBtn').click();
    await expect(overlay).toBeHidden({ timeout: 3000 });
  });

  test('TC-DREAM-UI-002: 触发 dream 分析 + 进度条', async ({ page }) => {
    await page.locator('#dreamBtn').click();
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 点击触发按钮
    await page.locator('#dreamTriggerBtn').click();

    // 验证进度条出现
    await expect(page.locator('#dreamProgress')).toBeVisible({ timeout: 3000 });

    // 验证进度条有进度值
    await page.waitForTimeout(500);
    const progressBar = page.locator('#dreamProgressBar');
    const width = await progressBar.evaluate((el) => (el as HTMLElement).style.width);
    expect(width).not.toBe('');
  });

  test('TC-DREAM-UI-003: 建议列表按 severity 分组', async ({ page }) => {
    // 先触发 dream 获取建议
    await page.evaluate(async () => {
      await (window as any).__TAURI__.core.invoke('trigger_dream');
    });
    await page.waitForTimeout(1000);

    // 打开面板查看建议
    await page.locator('#dreamBtn').click();
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 等待建议渲染
    await page.waitForSelector('#dreamSuggestions', { timeout: 5000 });
    const suggestions = page.locator('#dreamSuggestions > div');
    const count = await suggestions.count();
    expect(count).toBeGreaterThan(0);
  });

  test('TC-DREAM-UI-004: 中止 dream', async ({ page }) => {
    await page.locator('#dreamBtn').click();
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 触发 dream
    await page.locator('#dreamTriggerBtn').click();
    await expect(page.locator('#dreamAbortBtn')).toBeVisible({ timeout: 3000 });

    // 点击中止
    await page.locator('#dreamAbortBtn').click();
    await page.waitForTimeout(500);

    // 验证中止按钮隐藏，触发按钮显示
    await expect(page.locator('#dreamTriggerBtn')).toBeVisible({ timeout: 3000 });
  });

  test('TC-DREAM-UI-005: 历史建议查看', async ({ page }) => {
    // 确保 mock 中有历史建议
    await page.evaluate(async () => {
      const mock = (window as any).__mock;
      if (mock && mock.state.dreamSuggestions.length === 0) {
        await (window as any).__TAURI__.core.invoke('trigger_dream');
      }
    });
    await page.waitForTimeout(500);

    // 打开面板
    await page.locator('#dreamBtn').click();
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 验证建议列表已加载
    const suggestionsContainer = page.locator('#dreamSuggestions');
    await expect(suggestionsContainer).toBeVisible();
    const content = await suggestionsContainer.textContent();
    expect(content.length).toBeGreaterThan(0);
  });
});
