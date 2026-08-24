// E2E 边界与防御性测试（REQ-UI-005、REQ-LIC-003-AC-4、REQ-SEC-003）。
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E 边界与防御性', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
  });

  test('E2E-UI-011 空知识库时主界面展示导入引导', async ({ page }) => {
    await enterApp(page);
    await expect(page.locator('#chatArea')).toBeVisible();
    const hasGuide = await page.locator('#chatArea .empty-state-wrapper, #chatArea .h-full').count();
    expect(hasGuide, '空知识库应展示引导空状态').toBeGreaterThanOrEqual(1);
    await expect(page.locator('#kbDocCount')).toContainText('0/50');
  });

  test('E2E-UI-012 后端错误经 toast 展示，不含敏感信息', async ({ page }) => {
    await enterApp(page);
    // 通过拖拽触发不支持格式错误（前端 importPaths 捕获并 toast）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/bad.exe']));
    await expect(page.locator('#toasts')).toContainText('不支持', { timeout: 5000 });
    const toastText = await page.locator('#toasts').innerText();
    expect(toastText, 'toast 不得含 sk- 前缀').not.toMatch(/sk-[a-zA-Z0-9]/);
  });

  test('E2E-LIC-004 关闭付费墙无副作用', async ({ page }) => {
    await enterApp(page);
    // 确保是免费版（stub 默认 isPro=true）
    await page.evaluate(() => { window.__state.isPro = false; });
    // 触发付费墙（免费版拖拽导入 PDF）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/paper.pdf']));
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });

    // 点击「稍后再说」关闭
    await page.locator('#paywallClose').click();
    await expect(page.locator('#paywall')).toBeHidden();

    // 原有数据不受影响：知识库仍为空，可正常操作
    await expect(page.locator('#kbDocCount')).toContainText('0/50');
    await expect(page.locator('#queryInput')).toBeVisible();
  });

  test('E2E-SEC-002 默认配置下不发起外网请求', async ({ page }) => {
    const requests = [];
    page.on('request', (req) => {
      const url = req.url();
      if (url.startsWith('http://') || url.startsWith('https://')) {
        requests.push(url);
      }
    });

    await enterApp(page);
    await page.waitForTimeout(1000);
    expect(requests, '不应有任何 http(s) 外网请求').toHaveLength(0);
  });

  test('E2E-UI-011b 配额触顶弹出付费墙', async ({ page }) => {
    await enterApp(page);
    // 确保是免费版（stub 默认 isPro=true）
    await page.evaluate(() => { window.__state.isPro = false; });
    // 模拟已有 50 个文档
    await page.evaluate(() => {
      for (let i = 0; i < 50; i++) {
        window.__state.docs.push({
          id: 'doc-fill-' + i,
          file_path: '/mock/fill-' + i + '.md',
          file_hash: 'hash-' + i,
          status: 'Indexed',
          created_at: Math.floor(Date.now() / 1000),
        });
      }
    });
    // 刷新 UI（发射 doc-status-changed 事件驱动 loadDocuments）
    await page.evaluate(() => {
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(500);

    // 通过拖拽尝试导入第 51 个文件（触发 LIMIT_REACHED）
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/extra.md']));
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });
  });
});
