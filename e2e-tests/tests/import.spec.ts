// E2E 导入管线全场景（REQ-ING-001~005、REQ-UI-006/014、REQ-LIC-002）。
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, openKbModal, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-ING-001~008 导入管线', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-ING-002 不支持格式被拦截，toast 说明原因', async ({ page }) => {
    // 通过拖拽路径触发 importPaths（前端错误处理链路）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/file.exe']));
    await expect(page.locator('#toasts')).toContainText('不支持', { timeout: 5000 });
    const docCount = await page.locator('#docList [data-doc-name]').count();
    expect(docCount).toBe(0);
  });

  test('E2E-ING-004 .docx 被拒绝并提示支持格式', async ({ page }) => {
    // 确保 Free 模式（Alpha 阶段 mock 默认 isPro=true，需手动设为 false）
    await page.evaluate(() => { window.__state.isPro = false; });
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/report.docx']));
    // Free 版 .docx 为 Pro 门控格式，应触发付费墙
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });
  });

  test('E2E-ING-003 Pro 版导入 PDF 正常入库', async ({ page }) => {
    // 先激活 Pro
    await page.evaluate(() => window.__TAURI__.core.invoke('activate_pro', { licenseKey: 'test-key' }));
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/paper.pdf'] })
    );
    await expect(page.locator('#docList [data-doc-name="paper.pdf"]')).toBeVisible({ timeout: 5000 });
  });

  test('E2E-ING-006 相同内容重复导入跳过', async ({ page }) => {
    // 第一次导入
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/doc.md'] })
    );
    await expect(page.locator('#docList [data-doc-name="doc.md"]')).toBeVisible({ timeout: 5000 });
    const count1 = await page.locator('#docList [data-doc-name]').count();

    // 第二次导入相同文件
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/doc.md'] })
    );
    await page.waitForTimeout(500);
    const count2 = await page.locator('#docList [data-doc-name]').count();
    expect(count2, '重复导入不应增加文档数').toBe(count1);
  });

  test('E2E-ING-007 同名不同内容正常入库', async ({ page }) => {
    // 设置不同内容
    await page.evaluate(() => {
      window.__mock.setFileContent('/mock/a/data.md', '内容 A');
      window.__mock.setFileContent('/mock/b/data.md', '内容 B');
    });
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/a/data.md', '/mock/b/data.md'] })
    );
    // 两个同名文件都应入库
    const docs = await page.locator('#docList [data-doc-name="data.md"]').count();
    expect(docs, '同名不同内容文件应各自入库').toBe(2);
  });

  test('E2E-UI-013 索引状态徽标与事件一致', async ({ page }) => {
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/status.md'] })
    );
    const item = page.locator('#docList [data-doc-name="status.md"]');
    await expect(item).toBeVisible({ timeout: 5000 });
    // 最终状态应为「已索引」（检查 data-doc-status 属性，非文本内容）
    await expect(item).toHaveAttribute('data-doc-status', 'Indexed', { timeout: 10000 });
  });

  test('E2E-UI-014 免费版配额 n/50 实时刷新', async ({ page }) => {
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/quota.md'] })
    );
    await expect(page.locator('#docList [data-doc-name="quota.md"]')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#kbDocCount')).toContainText('1/50');
  });

  test('E2E-ING-008 删除后配额计数释放', async ({ page }) => {
    // 确保 Free 模式（Alpha 阶段 mock 默认 isPro=true，需手动设为 false）
    await page.evaluate(() => { window.__state.isPro = false; });
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/del.md'] })
    );
    await expect(page.locator('#docList [data-doc-name="del.md"]')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#kbDocCount')).toContainText('1/50');

    // 删除文档（headless 模式下 group-hover:visible 不触发，用 evaluate 直接点击）
    await page.evaluate(() => {
      const item = document.querySelector('#docList [data-doc-name="del.md"]');
      const delBtn = item?.querySelector('button[data-action="delete"]');
      if (delBtn) delBtn.click();
    });
    await expect(page.locator('#docList [data-doc-name="del.md"]')).toHaveCount(0);
    await expect(page.locator('#kbDocCount')).toContainText('0/50');
  });

  test('E2E-ING-005 文件选择器多选后批量入库', async ({ page }) => {
    // mock dialog.open 返回多文件
    await page.evaluate(() => {
      window.__TAURI__.dialog.open = async () => ['/mock/multi1.md', '/mock/multi2.md'];
    });
    await page.locator('#plusBtn').click();
    await openKbModal(page);
    await expect(page.locator('#docList [data-doc-name="multi1.md"]')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#docList [data-doc-name="multi2.md"]')).toBeVisible({ timeout: 5000 });
  });
});
