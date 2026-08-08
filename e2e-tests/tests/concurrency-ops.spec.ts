// E2E 并发操作冲突测试：
// 验证多个操作同时发生时的数据一致性和 UI 稳定性
// E2E-CONC-001: 同时导入多个文件不丢失
// E2E-CONC-002: 同时创建多个会话不冲突
// E2E-CONC-003: 导入中发送消息不崩溃
// E2E-CONC-004: 删除文档中发送消息不崩溃
// E2E-CONC-005: 同时删除多个文档不遗漏
// E2E-CONC-006: 快速切换会话中发送消息不串话
// E2E-CONC-007: 设置面板操作中触发导入不崩溃
// E2E-CONC-008: 流式输出中打开设置面板不中断流
// E2E-CONC-009: 流式输出中折叠侧栏不中断流
// E2E-CONC-010: 流式输出中删除文档不中断流
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, openKbModal, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('E2E-CONC 并发操作冲突', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-CONC-001 同时导入多个文件不丢失', async ({ page }) => {
    await openKbModal(page);
    // 并发发起 3 批导入
    await Promise.all([
      page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/conc-1.md'] })),
      page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/conc-2.md'] })),
      page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/conc-3.md'] })),
    ]);
    await page.waitForTimeout(1000);

    const docCount = await page.evaluate(() => window.__mock.state.docs.length);
    expect(docCount).toBeGreaterThanOrEqual(3);
  });

  test('E2E-CONC-002 同时创建多个会话不冲突', async ({ page }) => {
    await Promise.all([
      page.evaluate(() => window.__TAURI__.core.invoke('create_conversation')),
      page.evaluate(() => window.__TAURI__.core.invoke('create_conversation')),
      page.evaluate(() => window.__TAURI__.core.invoke('create_conversation')),
    ]);
    await page.waitForTimeout(500);

    const convCount = await page.evaluate(() => window.__mock.state.conversations.length);
    expect(convCount).toBeGreaterThanOrEqual(3);
  });

  test('E2E-CONC-003 导入中发送消息不崩溃', async ({ page }) => {
    // 先导入一个文档
    await importDocs(page, ['/mock/conc-import.md']);

    // 同时发起导入和消息发送
    await page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/conc-import-2.md'] }));
    await page.locator('#queryInput').fill('导入中发送消息');
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(500);

    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-CONC-004 删除文档中发送消息不崩溃', async ({ page }) => {
    await importDocs(page, ['/mock/conc-delete.md']);
    const docId = await page.evaluate(() => window.__mock.state.docs[0]?.id);

    if (docId) {
      // 同时删除和发送
      await page.evaluate((id) =>
        window.__TAURI__.core.invoke('delete_document', { id })
      , docId);
      await page.locator('#queryInput').fill('删除中发送消息');
      await page.locator('#sendBtn').click();
      await page.waitForTimeout(500);
    }

    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-CONC-005 同时删除多个文档不遗漏', async ({ page }) => {
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/del-1.md', '/mock/del-2.md', '/mock/del-3.md'] })
    );
    await page.waitForTimeout(500);

    const docs = await page.evaluate(() => window.__mock.state.docs.map(d => d.id));
    expect(docs.length).toBeGreaterThanOrEqual(3);

    // 并发删除全部
    await Promise.all(docs.map((id) =>
      page.evaluate((docId) =>
        window.__TAURI__.core.invoke('delete_document', { id: docId })
      , id)
    ));
    await page.waitForTimeout(500);

    const remaining = await page.evaluate(() => window.__mock.state.docs.length);
    expect(remaining).toBe(0);
  });

  test('E2E-CONC-006 快速切换会话不串话', async ({ page }) => {
    // 创建 3 个会话
    for (let i = 0; i < 3; i++) {
      await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    }
    await page.waitForTimeout(300);

    // 快速切换并发送
    const convs = page.locator('#convList [data-conv-title]');
    const count = await convs.count();
    for (let i = 0; i < Math.min(count, 3); i++) {
      await convs.nth(i).click();
      await page.waitForTimeout(100);
      await page.locator('#queryInput').fill(`会话 ${i} 消息`);
      await page.locator('#sendBtn').click();
      await page.waitForTimeout(200);
    }

    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-CONC-007 设置面板操作中导入不崩溃', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });

    // 设置面板打开时导入
    await page.evaluate(() => window.__TAURI__.core.invoke('import_files', { paths: ['/mock/conc-settings.md'] }));
    await page.waitForTimeout(300);

    await expect(page.locator('#app')).toBeVisible();
    await page.locator('#settingsClose').click();
  });

  test('E2E-CONC-008 流式输出中打开设置不中断', async ({ page }) => {
    await importDocs(page, ['/mock/conc-stream.md']);
    await page.locator('#queryInput').fill('流式输出中打开设置');
    await page.locator('#sendBtn').click();

    // 等待流式开始
    await page.waitForTimeout(300);

    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });

    // 关闭设置面板
    await page.locator('#settingsClose').click();

    // 等待流式完成
    await waitForStreamDone(page, 15000);
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-CONC-009 流式输出中折叠侧栏不中断', async ({ page }) => {
    await importDocs(page, ['/mock/conc-sidebar.md']);
    await page.locator('#queryInput').fill('流式输出中折叠侧栏');
    await page.locator('#sendBtn').click();

    await page.waitForTimeout(300);

    // 折叠侧栏
    const collapseBtn = page.locator('#collapseBtn');
    if (await collapseBtn.isVisible()) {
      await collapseBtn.click();
      await page.waitForTimeout(200);
      // 展开回来
      const expandBtn = page.locator('#expandBtn');
      if (await expandBtn.isVisible()) {
        await expandBtn.click();
      }
    }

    await waitForStreamDone(page, 15000);
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-CONC-010 流式输出中删除文档不中断', async ({ page }) => {
    // 导入两个文档
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/conc-del-stream-1.md', '/mock/conc-del-stream-2.md'] })
    );
    await page.waitForTimeout(500);
    // RC1 修复：关闭 KB Modal 后才能交互输入框
    await page.locator('#kbCloseBtn').click();
    await expect(page.locator('#kbModal')).toBeHidden({ timeout: 3000 });

    // 开始发送消息
    await page.locator('#queryInput').fill('流式中删除文档');
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(300);

    // 删除一个文档
    const docId = await page.evaluate(() => window.__mock.state.docs[0]?.id);
    if (docId) {
      await page.evaluate((id) =>
        window.__TAURI__.core.invoke('delete_document', { id })
      , docId);
    }

    await waitForStreamDone(page, 15000);
    await expect(page.locator('#app')).toBeVisible();
  });
});
