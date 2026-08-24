// E2E Toast 系统行为验收（REQ-UI-005）。
// E2E-TOAST-001: 成功 toast 样式正确
// E2E-TOAST-002: 错误 toast 含红色边框
// E2E-TOAST-003: info toast 默认样式
// E2E-TOAST-004: toast 4.2 秒后自动消失
// E2E-TOAST-005: 多 toast 堆叠不覆盖
// E2E-TOAST-006: toastError 自动脱敏 API Key
// E2E-TOAST-007: toastError 自动脱敏用户路径
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-TOAST-001~007 Toast 系统行为', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-TOAST-001 成功 toast 样式正确', async ({ page }) => {
    // 触发成功 toast（导入成功）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/success.md']));
    // RC1 修复：#docList 在 KB Modal 内，需先打开才能检查可见性
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await expect(page.locator('#docList [data-doc-name="success.md"]')).toBeVisible({ timeout: 5000 });

    // 成功 toast 应含 accent 颜色类
    const toast = page.locator('#toasts > div').last();
    await expect(toast).toBeVisible();
    await expect(toast).toHaveClass(/text-accent/);
  });

  test('E2E-TOAST-002 错误 toast 含红色边框', async ({ page }) => {
    // 触发错误 toast（导入不支持格式）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/bad.exe']));
    await expect(page.locator('#toasts')).toContainText('不支持', { timeout: 5000 });

    // 错误 toast 应含红色样式
    const toast = page.locator('#toasts > div').last();
    await expect(toast).toHaveClass(/text-red-300/);
    await expect(toast).toHaveClass(/border-red-400/);
  });

  test('E2E-TOAST-003 info toast 默认样式', async ({ page }) => {
    // RC1 修复：#docList 在 KB Modal 内，需先打开
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });

    // 触发 info toast（删除文档的「正在重试索引」是 info）
    await page.evaluate(() => {
      window.__state.docs.push({
        id: 'doc-fail-test',
        file_path: '/mock/fail.md',
        file_hash: 'hash-fail',
        status: 'Failed',
        created_at: Math.floor(Date.now() / 1000),
      });
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    const docItem = page.locator('#docList [data-doc-name="fail.md"]');
    await docItem.hover();
    await docItem.locator('button[title="重试索引"]').click();

    // info toast 应含默认 slate 样式
    await expect(page.locator('#toasts')).toContainText('正在重试', { timeout: 5000 });
    const toast = page.locator('#toasts > div').last();
    await expect(toast).toHaveClass(/text-slate-300/);
  });

  test('E2E-TOAST-004 toast 自动消失（~4.2 秒）', async ({ page }) => {
    // 触发一个 toast
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/autofade.md']));
    await expect(page.locator('#toasts')).toContainText('导入完成', { timeout: 5000 });

    // 等待 5 秒后 toast 应自动消失
    await page.waitForTimeout(5500);
    const toastCount = await page.locator('#toasts > div').count();
    // 导入完成 toast 应已消失（可能有其他残留 toast，但至少这个应消失）
    const hasImportToast = await page.locator('#toasts').textContent();
    expect(hasImportToast, '导入完成 toast 应在 4.2s 后自动消失').not.toContain('导入完成：autofade.md');
  });

  test('E2E-TOAST-005 多 toast 堆叠不覆盖', async ({ page }) => {
    // 快速触发多个 toast
    await page.evaluate(() => {
      window.__mock.simulateDragDrop(['/mock/stack1.md']);
    });
    await page.waitForTimeout(200);
    await page.evaluate(() => {
      window.__mock.simulateDragDrop(['/mock/stack2.md']);
    });
    await page.waitForTimeout(200);

    // 应有多个 toast 同时存在
    const count = await page.locator('#toasts > div').count();
    expect(count, '多 toast 应堆叠存在').toBeGreaterThanOrEqual(2);
  });

  test('E2E-TOAST-006 toastError 自动脱敏 API Key', async ({ page }) => {
    // 模拟后端错误含 API Key（通过 chatError 状态触发错误）
    await page.evaluate(() => {
      window.__state.chatError = 'LLM API 错误 (HTTP 401): sk-abcdefghijklmnop123456 无效';
    });
    // 导入文档并发送消息触发 chat 错误
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/toast-test.md'] })
    );
    await page.waitForTimeout(300);
    await page.locator('#kbCloseBtn').click();
    await page.locator('#queryInput').fill('触发错误');
    await page.locator('#sendBtn').click();

    await expect(page.locator('#toasts')).toContainText('401', { timeout: 5000 });
    // 检查最后一条 toast（错误 toast）
    const toast = page.locator('#toasts > div').last();
    const toastText = await toast.innerText();
    // 不应含完整 API Key
    expect(toastText, '不应含完整 API Key').not.toMatch(/sk-[a-zA-Z0-9]{8,}/);
    // 应含脱敏后的 Key
    expect(toastText, '应含脱敏 Key').toContain('sk-****');
  });

  test('E2E-TOAST-007 toastError 自动脱敏用户路径', async ({ page }) => {
    // 模拟错误含用户路径（通过 chatError 状态触发错误）
    await page.evaluate(() => {
      window.__state.chatError = '文件读取失败：/Users/john/Desktop/file.md 不存在';
    });
    // 导入文档并发送消息触发 chat 错误
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/toast-test2.md'] })
    );
    await page.waitForTimeout(300);
    await page.locator('#kbCloseBtn').click();
    await page.locator('#queryInput').fill('触发路径错误');
    await page.locator('#sendBtn').click();

    await expect(page.locator('#toasts')).toContainText('文件读取失败', { timeout: 5000 });
    // 检查最后一条 toast（错误 toast）
    const toast = page.locator('#toasts > div').last();
    const toastText = await toast.innerText();
    // 不应含用户名
    expect(toastText, '不应含用户名').not.toContain('/Users/john/');
    // 应含脱敏路径
    expect(toastText, '应含脱敏路径').toContain('/Users/****/');
  });
});
