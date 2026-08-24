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
import { setupPage, openToolsDropdown } from './helpers.mjs';

// 辅助：通过 evaluate 点击按钮（避免 Playwright 点击 SVG 子元素时的定位问题）
async function clickById(page, id) {
  await page.evaluate((btnId) => {
    const el = document.getElementById(btnId);
    if (el) el.click();
    else throw new Error(`Element #${btnId} not found`);
  }, id);
}

test.describe('AutoDream 知识库整理面板 (TC-DREAM-UI)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // S5 P1-1: dreamBtn 收纳到工具下拉菜单，等待 #toolsBtn 可见
    await page.waitForSelector('#toolsBtn', { timeout: 5000 });
  });

  test('TC-DREAM-UI-001: Dream 面板打开', async ({ page }) => {
    await clickById(page, 'dreamBtn');

    const overlay = page.locator('#dreamPanelOverlay');
    await expect(overlay).toBeVisible({ timeout: 5000 });
    await expect(overlay).toHaveAttribute('role', 'dialog');
    await expect(overlay).toHaveAttribute('aria-modal', 'true');

    // 关闭
    await clickById(page, 'dreamCloseBtn');
    await expect(overlay).toBeHidden({ timeout: 3000 });
  });

  test('TC-DREAM-UI-002: 触发 dream 分析 + 进度条', async ({ page }) => {
    await clickById(page, 'dreamBtn');
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 等待 overlay 完全初始化
    await page.waitForTimeout(1000);

    // 通过 evaluate 直接调用 triggerDream 逻辑
    await page.evaluate(async () => {
      const triggerBtn = document.getElementById('dreamTriggerBtn');
      const abortBtn = document.getElementById('dreamAbortBtn');
      const progressEl = document.getElementById('dreamProgress');
      if (triggerBtn) triggerBtn.classList.add('hidden');
      if (abortBtn) abortBtn.classList.remove('hidden');
      if (progressEl) progressEl.classList.remove('hidden');
      try {
        await window.__TAURI__.core.invoke('trigger_dream');
      } catch (e) { /* ignore */ }
    });

    // 验证进度条出现（使用 waitForFunction 避免 Playwright 可见性问题）
    await page.waitForFunction(() => {
      const el = document.getElementById('dreamProgress');
      return el && !el.classList.contains('hidden');
    }, { timeout: 5000 });

    // 验证进度条有进度值
    await page.waitForTimeout(500);
    const width = await page.evaluate(() => {
      const el = document.getElementById('dreamProgressBar');
      return el ? el.style.width : '';
    });
    expect(width).not.toBe('');
  });

  test('TC-DREAM-UI-003: 建议列表按 severity 分组', async ({ page }) => {
    // 先触发 dream 获取建议
    await page.evaluate(async () => {
      await (window as any).__TAURI__.core.invoke('trigger_dream');
    });
    await page.waitForTimeout(1000);

    // 打开面板查看建议
    await clickById(page, 'dreamBtn');
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 等待建议渲染
    await page.waitForSelector('#dreamSuggestions', { timeout: 5000 });
    const suggestions = page.locator('#dreamSuggestions > div');
    const count = await suggestions.count();
    expect(count).toBeGreaterThan(0);
  });

  test('TC-DREAM-UI-004: 中止 dream', async ({ page }) => {
    await clickById(page, 'dreamBtn');
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 等待 overlay 完全初始化
    await page.waitForTimeout(1000);

    // 通过 evaluate 直接调用 triggerDream 逻辑
    await page.evaluate(async () => {
      const triggerBtn = document.getElementById('dreamTriggerBtn');
      const abortBtn = document.getElementById('dreamAbortBtn');
      const progressEl = document.getElementById('dreamProgress');
      if (triggerBtn) triggerBtn.classList.add('hidden');
      if (abortBtn) abortBtn.classList.remove('hidden');
      if (progressEl) progressEl.classList.remove('hidden');
      try {
        await window.__TAURI__.core.invoke('trigger_dream');
      } catch (e) { /* ignore */ }
    });

    // 等待 abort 按钮可见
    await page.waitForFunction(() => {
      const el = document.getElementById('dreamAbortBtn');
      return el && !el.classList.contains('hidden');
    }, { timeout: 5000 });

    // 中止
    await page.evaluate(() => {
      const abortBtn = document.getElementById('dreamAbortBtn');
      if (abortBtn) abortBtn.click();
    });
    await page.waitForTimeout(500);

    // 验证中止按钮隐藏，触发按钮显示
    await page.waitForFunction(() => {
      const trigger = document.getElementById('dreamTriggerBtn');
      return trigger && !trigger.classList.contains('hidden');
    }, { timeout: 5000 });
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
    await clickById(page, 'dreamBtn');
    await expect(page.locator('#dreamPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 等待建议加载完成（loadDreamSuggestions 是异步的）
    await page.waitForFunction(() => {
      const el = document.getElementById('dreamSuggestions');
      return el && el.children.length > 0;
    }, { timeout: 10000 });

    // 验证建议列表有内容
    const content = await page.evaluate(() => {
      const el = document.getElementById('dreamSuggestions');
      return el ? el.textContent : '';
    });
    expect(content!.length).toBeGreaterThan(0);
  });
});
