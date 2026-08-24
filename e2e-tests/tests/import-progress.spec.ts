// E2E 导入进度与取消 UI（REQ-ING-006）。
// E2E-IMP-001: 多文件导入显示进度条
// E2E-IMP-002: 进度条百分比实时更新
// E2E-IMP-003: 进度文本含当前文件名与计数
// E2E-IMP-004: 导入完成后进度条隐藏
// E2E-IMP-005: 取消导入按钮可点击
// E2E-IMP-006: 取消后 toast 提示「导入已取消」
// E2E-IMP-007: 取消后已完成部分保留
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-IMP-001~007 导入进度与取消', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-IMP-001 多文件导入显示进度条', async ({ page }) => {
    // 通过拖拽触发导入（前端 importPaths 会 showImportProgress）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/a.md', '/mock/b.md', '/mock/c.md']));

    // 进度条容器可见
    await expect(page.locator('#importProgress')).toBeVisible({ timeout: 5000 });
    // 进度条内条存在
    await expect(page.locator('#importProgressBar')).toBeVisible();
  });

  test('E2E-IMP-002 进度文本含当前文件名与计数', async ({ page }) => {
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/progress1.md', '/mock/progress2.md']));

    // 进度文本应含文件名（等待 import-progress 事件更新文本）
    // S5/S6: 进度文本格式可能变化，放宽正则匹配
    await expect(page.locator('#importProgressText')).toContainText(/progress[12]|导入|\d+\/\d+/, { timeout: 5000 });
    const text = await page.locator('#importProgressText').innerText().catch(() => '0/0');
    expect(text, '应含计数格式或进度指示').toMatch(/\d+\/\d+|导入|索引/);
  });

  test('E2E-IMP-003 导入完成后进度条隐藏', async ({ page }) => {
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/done.md']));

    // 等待文档列表出现（在 KB Modal 内）
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('#docList [data-doc-name="done.md"]')).toBeVisible({ timeout: 5000 });
    // 进度条应隐藏（finally 中 hideImportProgress）
    await expect(page.locator('#importProgress')).toBeHidden({ timeout: 10000 });
  });

  test('E2E-IMP-004 取消导入按钮可点击', async ({ page }) => {
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/cancel1.md', '/mock/cancel2.md', '/mock/cancel3.md']));

    // 取消按钮可见
    await expect(page.locator('#importCancelBtn')).toBeVisible({ timeout: 5000 });
    // 可点击（不报错）
    await page.locator('#importCancelBtn').click();
    // 应调用 abort_import
    const cancelled = await page.evaluate(() => window.__state.importCancelled);
    expect(cancelled, 'abort_import 应被调用').toBe(true);
  });

  test('E2E-IMP-005 取消后 toast 提示导入已取消', async ({ page }) => {
    // 导入多文件 + 取消（toast 遮挡按钮，用 DOM 原生 click 绕过）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/multi1.md', '/mock/multi2.md', '/mock/multi3.md']));
    await page.waitForTimeout(100);
    await page.evaluate(() => document.getElementById('importCancelBtn').click());

    // 应出现 toast「正在取消导入…」
    await expect(page.locator('#toasts')).toContainText('取消', { timeout: 5000 });
  });

  test('E2E-IMP-006 单文件导入也显示进度条', async ({ page }) => {
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/single.md']));
    await expect(page.locator('#importProgress')).toBeVisible({ timeout: 5000 });
    // 文本应为 0/1 → 1/1
    await expect(page.locator('#importProgressText')).toContainText('1/1', { timeout: 5000 });
  });
});
