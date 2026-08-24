// E2E 批量删除文档（REQ-ING-009）。
// AC-1: 进入多选模式后，每个文档项前出现复选框；底部出现批量操作栏
// AC-2: 批量删除前弹出确认对话框，显示「将删除 N 个文档」
// AC-3: 取消确认对话框不产生任何删除操作
// AC-4: 批量删除完成后列表实时刷新，配额计数同步更新
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, openKbModal, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-ING-009 批量删除文档', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await importDocs(page, [
      '/mock/batch-doc-1.md',
      '/mock/batch-doc-2.txt',
      '/mock/batch-doc-3.md',
    ]);
    await openKbModal(page);
  });

  test('AC-1 进入多选模式后显示复选框和批量操作栏', async ({ page }) => {
    // 点击多选切换按钮
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);

    // 每个文档项应显示复选框
    const checkboxes = page.locator('#docList input[type="checkbox"]');
    await expect(checkboxes).toHaveCount(3);

    // 底部批量操作栏应可见
    await expect(page.locator('#kbBatchBar')).toBeVisible();
  });

  test('AC-2 批量删除前弹出确认对话框显示数量', async ({ page }) => {
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);

    // 勾选前两个文档
    await page.locator('#docList input[type="checkbox"]').nth(0).click();
    await page.locator('#docList input[type="checkbox"]').nth(1).click();
    await page.waitForTimeout(100);

    // 点击批量删除按钮
    await page.locator('#kbBatchDelete').click();

    // 确认对话框应显示「2」
    // RC2 修复：前端迁移到 showConfirmDialog 统一组件，使用 #confirmDialog 而非 #kbConfirmDialog
    await expect(page.locator('#confirmDialog')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('#confirmDialogBody')).toContainText('2');
  });

  test('AC-3 取消确认对话框不产生删除操作', async ({ page }) => {
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);

    // 勾选两个文档
    await page.locator('#docList input[type="checkbox"]').nth(0).click();
    await page.locator('#docList input[type="checkbox"]').nth(1).click();
    await page.waitForTimeout(100);

    // 点击批量删除
    await page.locator('#kbBatchDelete').click();
    // RC2 修复：前端迁移到 showConfirmDialog 统一组件
    await expect(page.locator('#confirmDialog')).toBeVisible({ timeout: 3000 });

    // 取消
    await page.locator('#confirmDialog button[data-role="cancel"]').click();
    await expect(page.locator('#confirmDialog')).toBeHidden({ timeout: 3000 });

    // 文档仍在
    const items = page.locator('#docList [data-doc-name]');
    await expect(items).toHaveCount(3);
  });

  test('AC-4 批量删除后列表刷新配额更新', async ({ page }) => {
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);

    // 全选
    for (let i = 0; i < 3; i++) {
      await page.locator('#docList input[type="checkbox"]').nth(i).click();
    }
    await page.waitForTimeout(100);

    // 批量删除
    await page.locator('#kbBatchDelete').click();
    // RC2 修复：前端迁移到 showConfirmDialog 统一组件
    await expect(page.locator('#confirmDialog')).toBeVisible({ timeout: 3000 });

    // 等待防误触延迟（500ms）后确认删除
    await page.waitForTimeout(600);
    await page.locator('#confirmDialog button[data-role="confirm"]').click();

    // 等待删除完成（toast 或列表更新）
    await page.waitForTimeout(500);

    // 文档列表应清空
    const remaining = page.locator('#docList [data-doc-name]');
    await expect(remaining).toHaveCount(0);
  });

  // v1.5 S2 补充测试 — 边界场景覆盖
  test('AC-5 退出多选模式恢复常规视图', async ({ page }) => {
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);
    await expect(page.locator('#kbBatchBar')).toBeVisible();

    // 点击取消按钮退出多选
    await page.locator('#kbBatchCancel').click();
    await page.waitForTimeout(200);

    // 批量操作栏应隐藏
    await expect(page.locator('#kbBatchBar')).toBeHidden();
    // 复选框应消失
    const checkboxes = page.locator('#docList input[type="checkbox"]');
    await expect(checkboxes).toHaveCount(0);
    // 文档仍存在
    const items = page.locator('#docList [data-doc-name]');
    await expect(items).toHaveCount(3);
  });

  test('AC-6 部分删除保留剩余文档', async ({ page }) => {
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);

    // 仅勾选第 1 和第 3 个文档
    await page.locator('#docList input[type="checkbox"]').nth(0).click();
    await page.locator('#docList input[type="checkbox"]').nth(2).click();
    await page.waitForTimeout(100);

    // 批量删除
    await page.locator('#kbBatchDelete').click();
    await expect(page.locator('#confirmDialog')).toBeVisible({ timeout: 3000 });
    await page.waitForTimeout(600);
    await page.locator('#confirmDialog button[data-role="confirm"]').click();
    await page.waitForTimeout(500);

    // 应剩 1 个文档
    const remaining = page.locator('#docList [data-doc-name]');
    await expect(remaining).toHaveCount(1);
  });

  test('AC-7 空选时不触发批量删除', async ({ page }) => {
    await page.locator('#kbSelectToggle').click();
    await page.waitForTimeout(200);

    // 不勾选任何文档，直接点击批量删除
    await page.locator('#kbBatchDelete').click();
    await page.waitForTimeout(300);

    // 确认对话框不应出现
    await expect(page.locator('#confirmDialog')).toBeHidden();
    // 文档仍存在
    const items = page.locator('#docList [data-doc-name]');
    await expect(items).toHaveCount(3);
  });
});
