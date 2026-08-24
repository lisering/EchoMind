/**
 * E2E 测试：Trace + Budget 面板（TC-TRACE-UI-001~008）。
 *
 * 验证 S2 复盘接线的 Trace 链路追踪 + Token 预算面板：
 * - 面板在设置中可见
 * - Trace 记数显示
 * - 查看最近 trace 列表
 * - Trace 详情对话框
 * - 清空 trace 操作
 * - Token 预算统计显示
 * - 预算配置保存
 * - 日限额设置
 */

import { test, expect } from '@playwright/test';
import { setupPage, enableDevMode } from './helpers.mjs';

test.describe('Trace + Budget 面板 (TC-TRACE-UI)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // S5 P0-6: Trace + Budget 面板仅在开发者模式下可见
    await page.waitForSelector('#settingsBtn', { timeout: 5000 });
    // 先启用开发者模式（⌘Shift+D 切换 _devMode）
    await enableDevMode(page);
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    // 等待 Trace + Budget 面板渲染完成（异步渲染，需轮询等待）
    await page.waitForSelector('#traceBudgetSection', { timeout: 15000 }).catch(async () => {
      // 如果未出现，重试：关闭设置再重新打开
      await page.evaluate(() => {
        const modal = document.querySelector('#settingsModal');
        if (modal) modal.classList.add('hidden');
      });
      await page.waitForTimeout(500);
      await page.locator('#settingsBtn').click();
      await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
      await page.waitForSelector('#traceBudgetSection', { timeout: 15000 });
    });
  });

  test('TC-TRACE-UI-001: Trace 面板可见', async ({ page }) => {
    const section = page.locator('#traceBudgetSection');
    await expect(section).toBeVisible();
    // 验证标题存在
    await expect(section.locator('span[data-i18n="trace.title"]')).toBeVisible();
  });

  test('TC-TRACE-UI-002: Trace 计数显示', async ({ page }) => {
    // 注入 mock trace 数据
    await page.evaluate(() => {
      const mock = (window as any).__mock;
      if (mock) {
        mock.state.traces = [
          { id: 'tr1', query: 'test query 1', total_duration_ms: 100, total_tokens: 50, created_at: Date.now() },
          { id: 'tr2', query: 'test query 2', total_duration_ms: 200, total_tokens: 80, created_at: Date.now() },
        ];
      }
    });
    // 重新打开设置面板刷新数据
    await page.evaluate(() => {
      const modal = document.querySelector('#settingsModal');
      if (modal) modal.classList.add('hidden');
    });
    await page.waitForTimeout(300);
    await page.locator('#settingsBtn').click();
    await page.waitForSelector('#traceBudgetSection', { timeout: 10000 });
    await page.waitForTimeout(500);

    // 验证计数显示包含 "2"
    const countText = page.locator('#traceBudgetSection').textContent();
    expect(await countText).toContain('2');
  });

  test('TC-TRACE-UI-003: 查看最近 trace 列表', async ({ page }) => {
    // 注入 mock trace 数据
    await page.evaluate(() => {
      const mock = (window as any).__mock;
      if (mock) {
        mock.state.traces = [
          { id: 'tr1', query: 'recent query 1', total_duration_ms: 150, total_tokens: 60, created_at: Date.now() },
        ];
      }
    });
    // 重新打开设置面板
    await page.evaluate(() => {
      const modal = document.querySelector('#settingsModal');
      if (modal) modal.classList.add('hidden');
    });
    await page.waitForTimeout(300);
    await page.locator('#settingsBtn').click();
    await page.waitForSelector('#traceBudgetSection', { timeout: 10000 });
    await page.waitForTimeout(500);

    // 点击 "查看最近" 按钮
    const btnView = page.locator('#btnViewTraces');
    await expect(btnView).toBeVisible({ timeout: 5000 });
    await btnView.click();
    await page.waitForTimeout(1000);

    // 验证 trace 列表对话框出现（使用 .trace-list-item 限定到对话框内的列表项）
    const dialogItem = page.locator('.trace-list-item').filter({ hasText: 'recent query 1' });
    await expect(dialogItem).toBeVisible({ timeout: 5000 });
  });

  test('TC-TRACE-UI-004: 清空 trace 操作', async ({ page }) => {
    // 注入 mock trace 数据
    await page.evaluate(() => {
      const mock = (window as any).__mock;
      if (mock) {
        mock.state.traces = [
          { id: 'tr1', query: 'to be cleared', total_duration_ms: 100, total_tokens: 50, created_at: Date.now() },
        ];
      }
    });
    // 重新打开设置面板
    await page.evaluate(() => {
      const modal = document.querySelector('#settingsModal');
      if (modal) modal.classList.add('hidden');
    });
    await page.waitForTimeout(300);
    await page.locator('#settingsBtn').click();
    await page.waitForSelector('#traceBudgetSection', { timeout: 10000 });
    await page.waitForTimeout(500);

    // 点击 "清空" 按钮
    const btnClear = page.locator('#btnClearTraces');
    await expect(btnClear).toBeVisible({ timeout: 5000 });
    await btnClear.click();
    await page.waitForTimeout(800);

    // 确认对话框 → 点击确认（使用 data-role 定位，等待防误触延迟后启用）
    const confirmBtn = page.locator('button[data-role="confirm"]');
    await expect(confirmBtn).toBeVisible({ timeout: 3000 });
    // 等待防误触延迟结束（按钮变为 enabled）
    await expect(confirmBtn).toBeEnabled({ timeout: 2000 });
    await confirmBtn.click();
    await page.waitForTimeout(500);

    // 验证 trace 被清空
    const mockState = await page.evaluate(() => (window as any).__mock?.state?.traces?.length || 0);
    expect(mockState).toBe(0);
  });

  test('TC-TRACE-UI-005: Token 预算统计显示', async ({ page }) => {
    // 验证日用量进度条存在
    const budgetTitle = page.locator('span[data-i18n="trace.budget_title"]');
    await expect(budgetTitle).toBeVisible();
    // 验证今日用量标签存在
    const dailyUsage = page.locator('span[data-i18n="trace.daily_usage"]');
    await expect(dailyUsage).toBeVisible();
  });

  test('TC-TRACE-UI-006: 预算配置输入框存在', async ({ page }) => {
    // 验证上下文窗口限制输入框
    const maxTokensInput = page.locator('#budgetMaxTokensInput');
    await expect(maxTokensInput).toBeVisible();
    // 验证压缩阈值输入框
    const thresholdInput = page.locator('#budgetThresholdInput');
    await expect(thresholdInput).toBeVisible();
    // 验证保留比例输入框
    const keepRatioInput = page.locator('#budgetKeepRatioInput');
    await expect(keepRatioInput).toBeVisible();
    // 验证保存配置按钮
    const btnSave = page.locator('#btnSaveBudgetConfig');
    await expect(btnSave).toBeVisible();
  });

  test('TC-TRACE-UI-007: 保存预算配置', async ({ page }) => {
    // 修改上下文窗口限制
    const maxTokensInput = page.locator('#budgetMaxTokensInput');
    await maxTokensInput.fill('16384');

    // 点击保存配置
    const btnSave = page.locator('#btnSaveBudgetConfig');
    await btnSave.click();
    await page.waitForTimeout(500);

    // 验证后端状态更新
    const savedMaxTokens = await page.evaluate(() => (window as any).__mock?.state?.tokenBudgetMaxTokens);
    expect(savedMaxTokens).toBe(16384);
  });

  test('TC-TRACE-UI-008: 日限额设置', async ({ page }) => {
    // 修改日限额
    const limitInput = page.locator('#budgetDailyLimitInput');
    await limitInput.fill('5.0');
    await limitInput.dispatchEvent('change');
    await page.waitForTimeout(500);

    // 验证后端状态更新
    const savedLimit = await page.evaluate(() => (window as any).__mock?.state?.budgetDailyLimit);
    expect(savedLimit).toBe(5.0);
  });
});
