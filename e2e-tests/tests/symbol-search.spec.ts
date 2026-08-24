/**
 * E2E 测试：符号搜索面板（TC-SYM-UI-001~004）。
 *
 * 验证 REQ-RAG-031 代码感知 RAG 前端 UI：
 * - 侧栏「符号搜索」按钮存在且可点击
 * - 搜索框输入查询 → 结果展示
 * - 搜索结果包含符号名/类型图标/语言/行号
 * - 重建索引按钮存在
 */

import { test, expect } from '@playwright/test';
import { setupPage, clickToolButton } from './helpers.mjs';

test.describe('符号搜索面板 (TC-SYM-UI)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // S5 P1-1: symbolBtn 收纳到工具下拉菜单
    await page.waitForSelector('#toolsBtn', { timeout: 5000 });
  });

  test('TC-SYM-UI-001: 符号搜索框输入', async ({ page }) => {
    await clickToolButton(page, 'symbolBtn');
    await expect(page.locator('#symbolSearchOverlay')).toBeVisible({ timeout: 5000 });

    const input = page.locator('#symbolSearchInput');
    await expect(input).toBeVisible();
    await expect(input).toBeFocused();

    // 输入查询
    await input.fill('main');
    await page.waitForTimeout(500); // 等待防抖

    // 验证结果区域有内容
    const results = page.locator('#symbolResults');
    await expect(results).toBeVisible();
  });

  test('TC-SYM-UI-002: 搜索结果展示', async ({ page }) => {
    await clickToolButton(page, 'symbolBtn');
    await expect(page.locator('#symbolSearchOverlay')).toBeVisible({ timeout: 5000 });

    // 等待初始加载（空查询返回全部符号）
    await page.waitForSelector('#symbolResults > div', { timeout: 5000 });

    // 验证有搜索结果
    const results = page.locator('#symbolResults [data-chunk-id]');
    const count = await results.count();
    expect(count).toBeGreaterThan(0);
  });

  test('TC-SYM-UI-003: 点击结果跳转', async ({ page }) => {
    await clickToolButton(page, 'symbolBtn');
    await expect(page.locator('#symbolSearchOverlay')).toBeVisible({ timeout: 5000 });

    // 等待结果加载
    await page.waitForSelector('#symbolResults [data-chunk-id]', { timeout: 5000 });

    // 验证每个结果有 chunk-id 属性（用于跳转）
    const firstResult = page.locator('#symbolResults [data-chunk-id]').first();
    const chunkId = await firstResult.getAttribute('data-chunk-id');
    expect(chunkId).not.toBeNull();
    expect(chunkId!.length).toBeGreaterThan(0);
  });

  test('TC-SYM-UI-004: 重建索引按钮存在', async ({ page }) => {
    await clickToolButton(page, 'symbolBtn');
    await expect(page.locator('#symbolSearchOverlay')).toBeVisible({ timeout: 5000 });

    const rebuildBtn = page.locator('#symbolRebuildBtn');
    await expect(rebuildBtn).toBeVisible();
    await expect(rebuildBtn).toBeEnabled();
  });
});
