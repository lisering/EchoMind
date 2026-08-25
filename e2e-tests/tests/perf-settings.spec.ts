/**
 * E2E 测试：性能优化设置（TC-PERF-UI-001~005）。
 *
 * 验证性能优化设置区块：
 * - 性能优化 section 在设置面板中显示
 * - 语义缓存 toggle + 统计展示
 * - Prompt 压缩比滑块
 * - 检索记忆 toggle + 统计
 * - 索引重建按钮（BM25 / Proposition / Summary Tree）
 */

import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('性能优化设置 (TC-PERF-UI)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // S5 P0-1: 智能模式默认隐藏所有性能设置，需先关闭智能模式
    await page.waitForSelector('#settingsBtn', { timeout: 5000 });
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    // V3.1 阶段二：S94 Tab 化后 perf 设置在「高级」分区，先切 Tab 再等待容器可见
    const advancedTab = page.locator('#settingsTabBar [data-tab-id="advanced"]');
    if (await advancedTab.count()) {
      await advancedTab.click();
    }
    // 等待性能设置容器渲染
    await page.waitForSelector('#perfSettingsContainer', { timeout: 5000 });
    // 关闭智能模式以显示所有性能设置 toggle
    // 直接通过 evaluate 调用 smartModeApi.set(false) 并重新打开设置面板触发重新渲染
    await page.evaluate(async () => {
      try {
        await window.__TAURI__.core.invoke('set_smart_mode', { enabled: false });
      } catch (e) { /* ignore */ }
    });
    // 关闭并重新打开设置面板以触发 perf-settings 重新渲染
    await page.evaluate(() => {
      const modal = document.querySelector('#settingsModal');
      if (modal) modal.classList.add('hidden');
    });
    await page.waitForTimeout(300);
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.waitForSelector('#perfSettingsContainer', { timeout: 5000 });
    await page.waitForTimeout(500);
  });

  test('TC-PERF-UI-001: 性能优化 section 显示', async ({ page }) => {
    const container = page.locator('#perfSettingsContainer');
    await expect(container).toBeVisible();
    // 验证有缓存 toggle
    await expect(page.locator('#perfCacheToggle')).toBeVisible();
    // 验证有压缩比滑块
    await expect(page.locator('#perfCompressionSlider')).toBeVisible();
  });

  test('TC-PERF-UI-002: 缓存 toggle + 统计', async ({ page }) => {
    const toggle = page.locator('#perfCacheToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'true'); // 默认启用

    // 验证统计信息存在
    const container = page.locator('#perfSettingsContainer');
    const text = await container.textContent();
    expect(text).not.toBe('');

    // 点击切换
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');

    // 切换回来
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  test('TC-PERF-UI-003: 压缩 ratio 滑块', async ({ page }) => {
    const slider = page.locator('#perfCompressionSlider');
    await expect(slider).toBeVisible();

    // 验证初始值为 1.0
    const initialValue = await slider.inputValue();
    expect(initialValue).toBe('1');

    // 拖动滑块到 3
    await slider.fill('3');
    const newValue = await slider.inputValue();
    expect(newValue).toBe('3');

    // 验证值标签更新
    const valueLabel = page.locator('#perfCompressionValue');
    const labelText = await valueLabel.textContent();
    expect(labelText).toContain('3');
  });

  test('TC-PERF-UI-004: 检索记忆 toggle + 统计', async ({ page }) => {
    const toggle = page.locator('#perfMemoryToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false'); // 默认禁用

    // 点击启用
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');

    // 验证重置按钮存在
    await expect(page.locator('#perfMemoryReset')).toBeVisible();
  });

  test('TC-PERF-UI-005: 索引重建按钮', async ({ page }) => {
    // 验证三个重建按钮存在
    await expect(page.locator('#perfRebuildBM25')).toBeVisible();
    await expect(page.locator('#perfRebuildProposition')).toBeVisible();
    await expect(page.locator('#perfBuildSummaryTree')).toBeVisible();

    // 点击 BM25 重建
    const btn = page.locator('#perfRebuildBM25');
    const originalText = await btn.textContent();
    await btn.click();
    await page.waitForTimeout(500);

    // 按钮应该恢复原文本
    await page.waitForTimeout(500);
    const finalText = await btn.textContent();
    expect(finalText).not.toBe('');
  });

  test('TC-PERF-UI-006: 渐进式注入 toggle 切换', async ({ page }) => {
    const toggle = page.locator('#perfProgressiveToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false'); // 默认禁用

    // 点击启用
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');

    // 切换回来
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  test('TC-PERF-UI-007: Speculative RAG toggle 切换', async ({ page }) => {
    const toggle = page.locator('#perfSpeculativeToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false'); // 默认禁用

    // 点击启用
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');

    // 切换回来
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  test('TC-PERF-UI-008: 质量门控 toggle 切换', async ({ page }) => {
    const toggle = page.locator('#perfQualityGateToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false'); // 默认禁用

    // 点击启用
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');

    // 切换回来
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });

  test('TC-PERF-UI-009: 知识图谱检索 toggle 切换', async ({ page }) => {
    const toggle = page.locator('#perfGraphRetrieverToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'false'); // 默认禁用

    // 点击启用
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');

    // 切换回来
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
  });
});
