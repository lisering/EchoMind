import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test('debug: 检查 embedEvalBtn onclick 绑定', async ({ page }) => {
  await setupPage(page);
  await page.waitForSelector('#settingsBtn', { timeout: 5000 });
  await page.locator('#settingsBtn').click();
  await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
  await page.waitForSelector('#settingsTabBar', { timeout: 5000 });
  await page.waitForTimeout(300);

  // 切换到高级 Tab
  await page.locator('[data-tab-id="advanced"]').click();
  await page.waitForTimeout(500);

  // 检查按钮是否有 onclick
  const onclickInfo = await page.evaluate(() => {
    const btn = document.getElementById('embedEvalBtn');
    if (!btn) return { found: false };
    return {
      found: true,
      hasOnclick: typeof btn.onclick === 'function',
      onclickType: typeof btn.onclick,
    };
  });
  console.log('Button info:', JSON.stringify(onclickInfo));

  // 尝试直接调用
  const result = await page.evaluate(async () => {
    try {
      const btn = document.getElementById('embedEvalBtn');
      if (btn && btn.onclick) {
        btn.onclick();
        await new Promise(r => setTimeout(r, 500));
        return { overlayExists: !!document.getElementById('embedEvalOverlay') };
      }
      return { noOnclick: true };
    } catch (e) {
      return { error: e.message };
    }
  });
  console.log('Click result:', JSON.stringify(result));
});
