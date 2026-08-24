// E2E 资源回收与内存测试：
// 验证长时间操作后无内存泄漏、DOM 节点不过度增长、事件监听器正确清理
// E2E-MEM-001: 连续 20 轮对话后 DOM 节点数不暴增
// E2E-MEM-002: 连续删除文档后 DOM 节点数回收
// E2E-MEM-003: 连续切换会话后事件监听器数稳定
// E2E-MEM-004: 关闭设置面板后 DOM 节点回收
// E2E-MEM-005: 长时间运行后应用不卡顿
// E2E-MEM-006: 取消流式后 DOM 节点清理
// E2E-MEM-007: 批量导入后文档列表渲染不重复
// E2E-MEM-008: 多次打开/关闭知识库弹框无 DOM 泄漏
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, openKbModal, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('E2E-MEM 资源回收与内存', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-MEM-001 连续 20 轮对话后 DOM 节点数不暴增', async ({ page }) => {
    await importDocs(page, ['/mock/mem-test.md']);

    // 记录初始 DOM 节点数
    const initialCount = await page.evaluate(() => document.querySelectorAll('*').length);

    // 连续 20 轮对话
    for (let i = 0; i < 20; i++) {
      await page.locator('#queryInput').fill(`内存测试 ${i}`);
      await page.locator('#sendBtn').click();
      await page.waitForTimeout(100);
    }
    await waitForStreamDone(page, 30000);

    // 检查最终 DOM 节点数
    const finalCount = await page.evaluate(() => document.querySelectorAll('*').length);

    // 节点数增长不应超过初始的 10 倍（每轮消息约增加 10-20 个节点）
    expect(finalCount).toBeLessThan(initialCount * 10);
  });

  test('E2E-MEM-002 连续删除文档后 DOM 节点数回收', async ({ page }) => {
    await openKbModal(page);
    // 导入 10 个文档
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: Array.from({ length: 10 }, (_, i) => `/mock/mem-del-${i}.md`) })
    );
    await page.waitForTimeout(1000);

    const beforeCount = await page.evaluate(() => document.querySelectorAll('#docList *').length);

    // 删除全部
    const docs = await page.evaluate(() => window.__mock.state.docs.map(d => d.id));
    for (const id of docs) {
      await page.evaluate((docId) =>
        window.__TAURI__.core.invoke('delete_document', { id: docId })
      , id);
    }
    await page.waitForTimeout(500);

    // 刷新列表
    await page.evaluate(() => {
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: 'refresh' } }));
    });
    await page.waitForTimeout(300);

    const afterCount = await page.evaluate(() => document.querySelectorAll('#docList *').length);

    // 删除后节点数应减少
    expect(afterCount).toBeLessThan(beforeCount);
  });

  test('E2E-MEM-003 连续切换会话后事件监听器数稳定', async ({ page }) => {
    // 创建 5 个会话
    for (let i = 0; i < 5; i++) {
      await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    }
    await page.waitForTimeout(300);

    // 初始监听器数
    const initialListeners = await page.evaluate(() =>
      Object.keys(window.__state.listeners).reduce((sum, k) => sum + window.__state.listeners[k].length, 0)
    );

    // 连续切换会话（使用 [data-conv-id] 而非 [data-conv-title]）
    const convs = page.locator('#convList [data-conv-id]');
    const count = await convs.count();
    if (count > 0) {
      for (let i = 0; i < Math.min(20, count > 1 ? count : 1); i++) {
        await convs.nth(i % count).click();
        await page.waitForTimeout(50);
      }
    }

    // 最终监听器数
    const finalListeners = await page.evaluate(() =>
      Object.keys(window.__state.listeners).reduce((sum, k) => sum + window.__state.listeners[k].length, 0)
    );

    // 监听器数不应暴增（允许较多增长）
    expect(finalListeners).toBeLessThan(initialListeners + 100);
  });

  test('E2E-MEM-004 关闭设置面板后 DOM 节点回收', async ({ page }) => {
    const beforeCount = await page.evaluate(() => document.querySelectorAll('*').length);

    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });
    await page.waitForTimeout(300);

    const openCount = await page.evaluate(() => document.querySelectorAll('*').length);
    expect(openCount).toBeGreaterThan(beforeCount);

    // 关闭设置面板
    await page.locator('#settingsClose').click();
    await page.waitForTimeout(500);

    const afterCount = await page.evaluate(() => document.querySelectorAll('*').length);
    // 关闭后节点数应减少（允许少量残留）
    expect(afterCount).toBeLessThanOrEqual(openCount);
  });

  test('E2E-MEM-005 长时间运行后应用不卡顿', async ({ page }) => {
    await importDocs(page, ['/mock/mem-perf.md']);

    // 连续发送多条消息
    for (let i = 0; i < 5; i++) {
      await page.locator('#queryInput').fill(`性能测试 ${i}`);
      await page.locator('#sendBtn').click();
      await page.waitForTimeout(200);
    }

    // 测量输入响应时间
    const startTime = Date.now();
    await page.locator('#queryInput').fill('延迟测试');
    const elapsed = Date.now() - startTime;

    // 输入应在 100ms 内完成
    expect(elapsed).toBeLessThan(100);
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-MEM-006 取消流式后 DOM 节点清理', async ({ page }) => {
    await importDocs(page, ['/mock/mem-cancel.md']);

    // 开始流式输出
    await page.locator('#queryInput').fill('取消测试');
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(500);

    const beforeCancel = await page.evaluate(() => document.querySelectorAll('#chatArea *').length);

    // 取消
    const stopBtn = page.locator('#sendBtn.stop-mode');
    if (await stopBtn.isVisible()) {
      await stopBtn.click();
      await page.waitForTimeout(1000);
    }

    const afterCancel = await page.evaluate(() => document.querySelectorAll('#chatArea *').length);

    // 取消后不应增加大量节点
    expect(afterCancel).toBeLessThanOrEqual(beforeCancel + 10);
  });

  test('E2E-MEM-007 批量导入后文档列表不重复渲染', async ({ page }) => {
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/mem-dup-1.md', '/mock/mem-dup-2.md'] })
    );
    await page.waitForTimeout(500);

    // 检查文档列表不重复
    const docItems = page.locator('#docList [data-doc-name]');
    const count = await docItems.count();

    // 获取所有文档名
    const names = await docItems.evaluateAll((els) => els.map((e) => e.getAttribute('data-doc-name')));

    // 不应有重复
    const uniqueNames = new Set(names);
    expect(uniqueNames.size).toBe(names.length);
  });

  test('E2E-MEM-008 多次打开关闭知识库弹框无泄漏', async ({ page }) => {
    const beforeCount = await page.evaluate(() => document.querySelectorAll('*').length);

    for (let i = 0; i < 10; i++) {
      await page.locator('#kbBtn').click();
      await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
      await page.locator('#kbCloseBtn').click();
      await page.waitForTimeout(100);
    }

    const afterCount = await page.evaluate(() => document.querySelectorAll('*').length);

    // DOM 节点数不应暴增
    expect(afterCount).toBeLessThan(beforeCount + 20);
  });
});
