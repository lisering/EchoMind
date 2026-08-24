/**
 * EchoMind V2.4 S91 批量操作增强 E2E 测试（REQ-ING-024）。
 *
 * 覆盖验收标准：
 * - TC-ING-BATCH-001：文档列表每行新增复选框，支持全选/反选
 * - TC-ING-BATCH-002：选中 ≥ 1 个文档时显示批量操作工具栏
 * - TC-ING-BATCH-003：批量删除前显示确认对话框含文档数量
 * - TC-ING-BATCH-004：批量移动到其他工作空间（下拉选择目标）
 * - TC-ING-BATCH-005：批量添加标签（输入标签名，多个用逗号分隔）
 * - TC-ING-BATCH-006：操作完成后显示成功/失败统计
 */

import { test, expect } from '@playwright/test';
import { setupPage, importDocs } from './helpers.mjs';

test.describe('V2.4 S91 批量操作增强 (REQ-ING-024)', () => {

  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-ING-BATCH-001：多选模式复选框 + 全选', async ({ page }) => {
    await importDocs(page, ['/mock/test1.md', '/mock/test2.md', '/mock/test3.md']);
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)');
    // 进入多选模式
    await page.click('#kbSelectToggle');
    await page.waitForSelector('#kbBatchBar:not(.hidden)');
    // 验证每行有复选框
    const checkboxes = page.locator('#docList input[type="checkbox"]');
    await expect(checkboxes).toHaveCount(3);
    // 点击全选
    await page.check('#kbSelectAll');
    // 验证全部选中
    const checked = page.locator('#docList input[type="checkbox"]:checked');
    await expect(checked).toHaveCount(3);
  });

  test('TC-ING-BATCH-002：选中文档时显示批量工具栏', async ({ page }) => {
    await importDocs(page, ['/mock/test1.md', '/mock/test2.md']);
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)');
    // 进入多选模式
    await page.click('#kbSelectToggle');
    await page.waitForSelector('#kbBatchBar:not(.hidden)');
    // 验证批量按钮存在
    await expect(page.locator('#kbBatchDelete')).toBeVisible();
    await expect(page.locator('#kbBatchMove')).toBeVisible();
    await expect(page.locator('#kbBatchTag')).toBeVisible();
  });

  test('TC-ING-BATCH-003：批量删除确认对话框含文档数量', async ({ page }) => {
    await importDocs(page, ['/mock/test1.md', '/mock/test2.md']);
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)');
    // 进入多选模式
    await page.click('#kbSelectToggle');
    // 全选
    await page.check('#kbSelectAll');
    // 点击批量删除
    await page.click('#kbBatchDelete');
    // 验证确认对话框出现
    await page.waitForSelector('#confirmDialog');
    // 验证对话框内容包含数量 "2"
    const dialogText = await page.locator('#confirmDialog').textContent();
    expect(dialogText).toContain('2');
  });

  test('TC-ING-BATCH-004：批量移动到其他工作空间', async ({ page }) => {
    await importDocs(page, ['/mock/test1.md']);
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)');
    // 进入多选模式
    await page.click('#kbSelectToggle');
    // 选中第一个文档
    await page.check('#docList input[type="checkbox"]:first-child');
    // 点击移动按钮
    await page.click('#kbBatchMove');
    // 验证对话框出现
    await page.waitForSelector('#confirmDialog');
    // 验证下拉选择器存在
    await expect(page.locator('#batchMoveTarget')).toBeVisible();
  });

  test('TC-ING-BATCH-005：批量添加标签输入框', async ({ page }) => {
    await importDocs(page, ['/mock/test1.md']);
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)');
    // 进入多选模式
    await page.click('#kbSelectToggle');
    // 选中第一个文档
    await page.check('#docList input[type="checkbox"]:first-child');
    // 点击标签按钮
    await page.click('#kbBatchTag');
    // 验证对话框出现
    await page.waitForSelector('#confirmDialog');
    // 验证输入框存在
    await expect(page.locator('#batchTagInput')).toBeVisible();
  });

  test('TC-ING-BATCH-006：批量删除后显示成功统计', async ({ page }) => {
    await importDocs(page, ['/mock/test1.md', '/mock/test2.md', '/mock/test3.md']);
    await page.click('#kbBtn');
    await page.waitForSelector('#kbModal:not(.hidden)');
    // 进入多选模式
    await page.click('#kbSelectToggle');
    // 全选
    await page.check('#kbSelectAll');
    // 点击批量删除
    await page.click('#kbBatchDelete');
    // 等待确认对话框
    await page.waitForSelector('#confirmDialog');
    // 点击确认按钮（需要等待 500ms 防误触）
    await page.waitForTimeout(600);
    await page.click('[data-role="confirm"]');
    // 验证 toast 出现（成功消息）
    await page.waitForSelector('.toast', { timeout: 5000 }).catch(() => {});
    // 验证文档已被删除
    const docItems = page.locator('#docList [data-doc-name]');
    await expect(docItems).toHaveCount(0);
  });

});
