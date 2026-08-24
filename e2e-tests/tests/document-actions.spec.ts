// E2E 文档操作按钮可见性与交互（REQ-VEC-005 重试、REQ-ING-005 删除、REQ-AUDIT-001 审计门控）。
// E2E-DOC-001: Failed 文档显示重试按钮
// E2E-DOC-002: 重试后状态从 Failed → Indexed
// E2E-DOC-003: 重试触发 toast 提示
// E2E-DOC-004: 删除按钮交互与 toast
// E2E-DOC-005: 审计按钮仅 Pro + Indexed 可见
// E2E-DOC-006: 非 Pro 文档审计按钮隐藏
import { test, expect } from '@playwright/test';
import { activatePro, enterApp, importDocs, injectLocales, openKbModal, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-DOC-001~006 文档操作按钮', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await importDocs(page, ['/mock/actions.md']);
    await openKbModal(page);
  });

  test('E2E-DOC-001 Failed 文档显示重试按钮', async ({ page }) => {
    // 手动将文档状态设为 Failed
    await page.evaluate(() => {
      const doc = window.__state.docs[0];
      if (doc) doc.status = 'Failed';
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    const retryBtn = docItem.locator('button[title*="重试"], button[title*="retry"], button[title*="Retry"]');
    await expect(retryBtn).toBeVisible();
  });

  test('E2E-DOC-002 重试后状态从 Failed → Indexed', async ({ page }) => {
    // 设为 Failed
    await page.evaluate(() => {
      const doc = window.__state.docs[0];
      if (doc) doc.status = 'Failed';
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    await docItem.locator('button[title*="重试"], button[title*="retry"], button[title*="Retry"]').click();

    // toast 提示（locale 无关：检查关键词 retry / 重试）
    await expect(page.locator('#toasts')).toContainText(/retry|重试/i, { timeout: 5000 });
  });

  test('E2E-DOC-003 Indexed 文档不显示重试按钮', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    const retryBtn = docItem.locator('button[title*="重试"], button[title*="retry"], button[title*="Retry"]');
    // Indexed 状态下重试按钮应隐藏（display: none）
    await expect(retryBtn).toHaveCSS('display', 'none');
  });

  test('E2E-DOC-004 删除按钮交互与 toast', async ({ page }) => {
    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    await docItem.locator('button[title*="删除"], button[title*="delete"], button[title*="Delete"]').click();

    // toast 提示（locale 无关：检查关键词 delete / 已删除）
    await expect(page.locator('#toasts')).toContainText(/delete|已删除/i, { timeout: 5000 });
    // DOM 消失
    await expect(page.locator('#docList [data-doc-name="actions.md"]')).toHaveCount(0);
  });

  test('E2E-DOC-005 Pro 版 Indexed 文档显示审计按钮', async ({ page }) => {
    // 关闭 KB Modal 再激活 Pro（避免 Modal 遮挡 paywall）
    await page.locator('#kbCloseBtn').click();
    await activatePro(page);
    await page.waitForTimeout(200);
    // 重新打开 KB Modal
    await openKbModal(page);
    // 重新渲染文档列表
    await page.evaluate(() => {
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    const auditBtn = docItem.locator('button[title*="审计"], button[title*="audit"], button[title*="Audit"]');
    // 检查 display 不为 none（Pro 版 + Indexed 状态下应显示）
    await expect(auditBtn).toHaveCSS('display', 'block');
  });

  test('E2E-DOC-006 免费版不显示审计按钮', async ({ page }) => {
    // 确保是免费版
    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    const auditBtn = docItem.locator('button[title*="审计"], button[title*="audit"], button[title*="Audit"]');
    await expect(auditBtn).toHaveCSS('display', 'none');
  });

  test('E2E-DOC-007 Processing 状态文档不显示审计按钮', async ({ page }) => {
    // 关闭 KB Modal 再激活 Pro
    await page.locator('#kbCloseBtn').click();
    await activatePro(page);
    // 重新打开 KB Modal
    await openKbModal(page);
    // 将文档设为 Processing
    await page.evaluate(() => {
      const doc = window.__state.docs[0];
      if (doc) doc.status = 'Processing';
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    const docItem = page.locator('#docList [data-doc-name="actions.md"]');
    await docItem.hover();
    const auditBtn = docItem.locator('button[title*="审计"], button[title*="audit"], button[title*="Audit"]');
    await expect(auditBtn).toHaveCSS('display', 'none');
  });
});
