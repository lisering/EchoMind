/**
 * E2E 测试：嵌入模型对比评估面板（TC-VEC-EVAL-E2E-001~005，REQ-VEC-018）。
 *
 * 验证：
 * - 设置面板中「嵌入模型评估」按钮存在
 * - 点击按钮打开面板 overlay
 * - 面板包含 6 个模型 checkbox
 * - 面板包含数据集选择 + Top-K + 开始评估按钮
 * - 评估完成后显示结果表格 + 柱状图
 */

import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('嵌入模型对比评估面板 (TC-VEC-EVAL-E2E)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  // 辅助：打开设置面板并切换到高级 Tab
  async function openSettingsAdvancedTab(page) {
    await page.waitForSelector('#settingsBtn', { timeout: 5000 });
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.waitForSelector('#settingsTabBar', { timeout: 5000 });
    await page.waitForTimeout(300);

    // 切换到「高级」Tab
    const advancedTab = page.locator('[data-tab-id="advanced"]');
    await advancedTab.click();
    await page.waitForTimeout(300);

    // 等待高级设置区域可见
    const advancedSection = page.locator('[data-settings-section="advanced"]');
    await expect(advancedSection).not.toHaveClass(/hidden/);

    // 等待嵌入评估按钮出现
    await page.waitForSelector('#embedEvalBtn', { timeout: 5000 });
    return page.locator('#embedEvalBtn');
  }

  test('TC-VEC-EVAL-E2E-001: 设置面板中嵌入模型评估按钮存在', async ({ page }) => {
    const btn = await openSettingsAdvancedTab(page);
    await expect(btn).toBeVisible({ timeout: 5000 });
  });

  test('TC-VEC-EVAL-E2E-002: 点击按钮打开面板 overlay', async ({ page }) => {
    const btn = await openSettingsAdvancedTab(page);
    await btn.click();
    await expect(page.locator('#embedEvalOverlay')).toBeVisible({ timeout: 5000 });
  });

  test('TC-VEC-EVAL-E2E-003: 面板包含 6 个模型 checkbox', async ({ page }) => {
    const btn = await openSettingsAdvancedTab(page);
    await btn.click();
    await expect(page.locator('#embedEvalOverlay')).toBeVisible({ timeout: 5000 });

    const checkboxes = page.locator('.embed-model-cb');
    await expect(checkboxes).toHaveCount(6);
  });

  test('TC-VEC-EVAL-E2E-004: 面板包含数据集选择 + Top-K + 开始评估按钮', async ({ page }) => {
    const btn = await openSettingsAdvancedTab(page);
    await btn.click();
    await expect(page.locator('#embedEvalOverlay')).toBeVisible({ timeout: 5000 });

    const radios = page.locator('input[name="embedDataset"]');
    await expect(radios).toHaveCount(2);

    await expect(page.locator('#embedEvalTopK')).toBeVisible();
    await expect(page.locator('#embedEvalStart')).toBeVisible();
  });

  test('TC-VEC-EVAL-E2E-005: 评估完成后显示结果表格 + 柱状图', async ({ page }) => {
    const btn = await openSettingsAdvancedTab(page);
    await btn.click();
    await expect(page.locator('#embedEvalOverlay')).toBeVisible({ timeout: 5000 });

    const checkboxes = page.locator('.embed-model-cb');
    await checkboxes.nth(0).check();
    await checkboxes.nth(1).check();

    await page.locator('#embedEvalStart').click();

    await expect(page.locator('#embedEvalResults')).not.toHaveClass(/hidden/, { timeout: 10000 });

    const table = page.locator('#embedEvalResults table');
    await expect(table).toBeVisible({ timeout: 10000 });

    const svg = page.locator('#embedEvalResults svg');
    await expect(svg).toBeVisible({ timeout: 10000 });
  });
});
