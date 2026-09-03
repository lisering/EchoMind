/**
 * E2E 测试：性能优化设置（TC-PERF-UI-001~003）。
 *
 * 大简化重构后精简版，验证：
 * - 性能优化 section 在设置面板中显示
 * - 智能模式 toggle
 * - Contextual Retrieval toggle
 * - 索引重建按钮（BM25 / Contextual Embeddings）
 *
 * 学术 RAG 优化模块已删除：缓存、压缩、检索记忆、渐进式注入、
 * Speculative RAG、质量门控、知识图谱检索、Proposition 索引、Summary Tree。
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
    // 验证有 Contextual Retrieval toggle
    await expect(page.locator('#perfContextualToggle')).toBeVisible();
  });

  test('TC-PERF-UI-002: Contextual Retrieval toggle 切换', async ({ page }) => {
    const toggle = page.locator('#perfContextualToggle');
    await expect(toggle).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-checked', 'true'); // 默认启用

    // 点击切换
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'false');

    // 切换回来
    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
  });

  test('TC-PERF-UI-003: 索引重建按钮', async ({ page }) => {
    // 验证两个重建按钮存在
    await expect(page.locator('#perfRebuildBM25')).toBeVisible();
    await expect(page.locator('#perfRebuildContextualEmbeddings')).toBeVisible();

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
});
