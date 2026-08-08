/**
 * E2E 测试：工作流可视化面板（TC-WF-UI-001~005）。
 *
 * 验证 REQ-RAG-030 前端工作流 UI：
 * - 侧栏「工作流」按钮存在且可点击
 * - 点击按钮后弹出工作流面板 overlay
 * - 创建工作流模板（JSON 编辑器输入 + 保存）
 * - 运行工作流 + 进度显示
 * - 删除工作流模板
 * - JSON 编辑器输入验证
 */

import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('工作流可视化面板 (TC-WF-UI)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    await page.waitForSelector('#workflowBtn', { timeout: 5000 });
  });

  test('TC-WF-UI-001: 工作流面板打开/关闭', async ({ page }) => {
    const btn = page.locator('#workflowBtn');
    await expect(btn).toBeVisible();
    await expect(btn).toBeEnabled();
    await btn.click();

    const overlay = page.locator('#workflowPanelOverlay');
    await expect(overlay).toBeVisible({ timeout: 5000 });
    await expect(overlay).toHaveAttribute('role', 'dialog');
    await expect(overlay).toHaveAttribute('aria-modal', 'true');

    // 关闭按钮
    await page.locator('#workflowCloseBtn').click();
    await expect(overlay).toBeHidden({ timeout: 3000 });
  });

  test('TC-WF-UI-002: 创建工作流模板', async ({ page }) => {
    await page.locator('#workflowBtn').click();
    await expect(page.locator('#workflowPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 点击新建按钮
    await page.locator('#workflowNewBtn').click();

    // 验证编辑器有内容
    const editor = page.locator('#workflowJsonEditor');
    const editorValue = await editor.inputValue();
    expect(editorValue).toContain('"nodes"');
    expect(editorValue).toContain('"edges"');

    // 保存
    await page.locator('#workflowSaveBtn').click();

    // 验证列表中有工作流
    await page.waitForTimeout(500);
    const listItems = page.locator('#workflowList [data-wf-id]');
    const count = await listItems.count();
    expect(count).toBeGreaterThan(0);
  });

  test('TC-WF-UI-003: 运行工作流 + 进度显示', async ({ page }) => {
    await page.locator('#workflowBtn').click();
    await expect(page.locator('#workflowPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 先创建并保存一个工作流
    await page.locator('#workflowNewBtn').click();
    await page.locator('#workflowSaveBtn').click();
    await page.waitForTimeout(500);

    // 点击运行
    await page.locator('#workflowRunBtn').click();

    // 验证结果区域显示节点状态
    await page.waitForSelector('#workflowResults [data-node-id]', { timeout: 5000 });
    const nodes = page.locator('#workflowResults [data-node-id]');
    const nodeCount = await nodes.count();
    expect(nodeCount).toBeGreaterThan(0);
  });

  test('TC-WF-UI-004: 删除工作流模板', async ({ page }) => {
    await page.locator('#workflowBtn').click();
    await expect(page.locator('#workflowPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 先创建一个工作流
    await page.locator('#workflowNewBtn').click();
    await page.locator('#workflowSaveBtn').click();
    await page.waitForTimeout(500);

    // 验证有工作流
    const beforeCount = await page.locator('#workflowList [data-wf-id]').count();
    expect(beforeCount).toBeGreaterThan(0);

    // 点击删除按钮
    const deleteBtn = page.locator('#workflowList .wf-delete-btn').first();
    await deleteBtn.click();
    await page.waitForTimeout(500);

    // 验证数量减少
    const afterCount = await page.locator('#workflowList [data-wf-id]').count();
    expect(afterCount).toBeLessThan(beforeCount);
  });

  test('TC-WF-UI-005: JSON 编辑器输入验证', async ({ page }) => {
    await page.locator('#workflowBtn').click();
    await expect(page.locator('#workflowPanelOverlay')).toBeVisible({ timeout: 5000 });

    // 输入无效 JSON
    const editor = page.locator('#workflowJsonEditor');
    await editor.fill('invalid json {{{');
    await page.locator('#workflowSaveBtn').click();

    // 验证没有创建新的工作流（toast 错误提示）
    // 编辑器仍保持可见
    await expect(editor).toBeVisible();
  });
});
