// E2E 压力与边界场景：
// E2E-STRESS-001: 大量文档导入——50 个文件批量导入
// E2E-STRESS-002: 大量会话创建——100 个会话
// E2E-STRESS-003: 长文本消息——超长输入不溢出
// E2E-STRESS-004: 特殊字符——Emoji/Unicode 正常
// E2E-STRESS-005: 特殊字符——中日韩混排正常
// E2E-STRESS-006: 特殊字符——HTML 实体转义
// E2E-STRESS-007: 连续快速发送——不崩溃
// E2E-STRESS-008: 连续切换会话——不崩溃
// E2E-STRESS-009: 连续打开/关闭设置——不崩溃
// E2E-STRESS-010: 连续折叠/展开侧栏——不崩溃
// E2E-STRESS-011: 空文档导入——空文件处理
// E2E-STRESS-012: 重复导入相同文件——去重
// E2E-STRESS-013: 删除全部文档——配额归零
// E2E-STRESS-014: 删除全部会话——空状态
// E2E-STRESS-015: 快速切换语言——不崩溃
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, openKbModal, injectStub, uiUrl, waitForStreamDone } from './helpers.mjs';

test.describe('E2E-STRESS 压力与边界场景', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 大量数据 ───

  test('E2E-STRESS-001 大量文档导入——50 个文件批量导入', async ({ page }) => {
    const paths = [];
    for (let i = 0; i < 50; i++) {
      paths.push(`/mock/batch-${i}.md`);
    }

    // 打开知识库弹框，然后批量导入
    await openKbModal(page);
    await page.evaluate((ps) =>
      window.__TAURI__.core.invoke('import_files', { paths: ps })
    , paths);
    await page.waitForTimeout(2000);

    // 文档列表应更新
    const docCount = await page.evaluate(() => window.__mock.state.docs.length);
    expect(docCount).toBeGreaterThanOrEqual(50);

    // 配额计数应更新
    await expect(page.locator('#kbDocCount')).toContainText('50');
  });

  test('E2E-STRESS-002 大量会话创建——100 个会话', async ({ page }) => {
    for (let i = 0; i < 100; i++) {
      await page.evaluate(() =>
        window.__TAURI__.core.invoke('create_conversation')
      );
    }
    await page.waitForTimeout(500);

    // 会话列表应显示（可能分页）
    const convCount = await page.evaluate(() => window.__mock.state.conversations.length);
    expect(convCount).toBeGreaterThanOrEqual(100);

    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  // ─── 长文本 ───

  test('E2E-STRESS-003 长文本消息——不溢出', async ({ page }) => {
    // 导入文档
    await importDocs(page, ['/mock/rust-guide.md']);

    // 发送超长消息
    const longMsg = '这是一条很长的消息。'.repeat(100);
    await page.locator('#queryInput').fill(longMsg);
    await page.locator('#sendBtn').click();
    await page.waitForTimeout(500);

    // 用户消息 block 应存在
    const userBlock = page.locator('#chatArea [class*="justify-end"]').first();
    if (await userBlock.count() > 0) {
      // 不应溢出（overflow hidden 或 scroll）
      const chatArea = page.locator('#chatArea');
      const areaWidth = await chatArea.evaluate((el) => el.scrollWidth);
      const clientWidth = await chatArea.evaluate((el) => el.clientWidth);
      // scrollWidth 不应远超 clientWidth
      expect(areaWidth).toBeLessThanOrEqual(clientWidth + 50);
    }
  });

  // ─── 特殊字符 ───

  test('E2E-STRESS-004 特殊字符——Emoji 正常', async ({ page }) => {
    // RC1 修复：空 KB 时 queryInput 被禁用，需先导入文档
    await importDocs(page, ['/mock/test.md']);
    await page.locator('#queryInput').fill('测试 Emoji：😀🎉🚀💻📝');
    await expect(page.locator('#queryInput')).toHaveValue('测试 Emoji：😀🎉🚀💻📝');
  });

  test('E2E-STRESS-005 特殊字符——中日韩混排', async ({ page }) => {
    // RC1 修复：空 KB 时 queryInput 被禁用，需先导入文档
    await importDocs(page, ['/mock/test.md']);
    const text = '中文 English 日本語 한국어';
    await page.locator('#queryInput').fill(text);
    await expect(page.locator('#queryInput')).toHaveValue(text);
  });

  test('E2E-STRESS-006 特殊字符——HTML 实体转义', async ({ page }) => {
    // 导入文档
    await importDocs(page, ['/mock/rust-guide.md']);

    // 设置含 HTML 的 token
    const xssTokens = ['<div>', 'alert(1)', '</div>', '<script>', 'evil()', '</script>'];
    await page.evaluate((tokens) => window.__mock.setCustomTokens(tokens), xssTokens);

    await page.locator('#queryInput').fill('安全测试');
    await page.locator('#sendBtn').click();
    await waitForStreamDone(page, 15000);

    // 不应执行脚本
    const scripts = page.locator('#chatArea script:not([src])');
    expect(await scripts.count()).toBe(0);
  });

  // ─── 连续操作 ───

  test('E2E-STRESS-007 连续快速发送——不崩溃', async ({ page }) => {
    // 导入文档
    await importDocs(page, ['/mock/rust-guide.md']);

    // 快速连续发送 3 条消息
    for (let i = 0; i < 3; i++) {
      await page.locator('#queryInput').fill(`快速消息 ${i}`);
      await page.locator('#sendBtn').click();
      await page.waitForTimeout(200);
    }

    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-STRESS-008 连续切换会话——不崩溃', async ({ page }) => {
    // 创建 5 个会话
    for (let i = 0; i < 5; i++) {
      await page.evaluate(() =>
        window.__TAURI__.core.invoke('create_conversation')
      );
    }
    await page.waitForTimeout(300);

    // 连续切换会话
    const convs = page.locator('#convList [data-conv-title]');
    const count = await convs.count();
    for (let i = 0; i < Math.min(count, 5); i++) {
      await convs.nth(i).click();
      await page.waitForTimeout(100);
    }

    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-STRESS-009 连续打开/关闭设置——不崩溃', async ({ page }) => {
    for (let i = 0; i < 5; i++) {
      await page.locator('#settingsBtn').click();
      await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 3000 });
      await page.locator('#settingsClose').click();
      await page.waitForTimeout(200);
    }

    // 应用不应崩溃
    await expect(page.locator('#settingsBtn')).toBeVisible();
  });

  test('E2E-STRESS-010 连续折叠/展开侧栏——不崩溃', async ({ page }) => {
    for (let i = 0; i < 10; i++) {
      await page.locator('#collapseBtn').click();
      await page.waitForTimeout(100);
      await page.locator('#expandBtn').click();
      await page.waitForTimeout(100);
    }

    // 应用不应崩溃
    await expect(page.locator('#sidebar')).toBeVisible();
  });

  // ─── 空文件/重复 ───

  test('E2E-STRESS-011 空文档导入——处理', async ({ page }) => {
    // mock 环境下空文件导入可能成功或失败
    try {
      await openKbModal(page);
      await page.evaluate(() =>
        window.__TAURI__.core.invoke('import_files', { paths: ['/mock/empty.md'] })
      );
    } catch {
      // 空文件可能被拒绝
    }

    // 应用不应崩溃
    await expect(page.locator('#app')).toBeVisible();
  });

  test('E2E-STRESS-012 重复导入相同文件——去重', async ({ page }) => {
    // 首次导入
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/duplicate-test.md'] })
    );
    await page.waitForTimeout(300);
    const count1 = await page.evaluate(() => window.__mock.state.docs.length);

    // 再次导入同一文件
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/duplicate-test.md'] })
    );
    await page.waitForTimeout(300);
    const count2 = await page.evaluate(() => window.__mock.state.docs.length);

    // 文档数不应增加（去重）
    expect(count2).toBe(count1);
  });

  // ─── 清空操作 ───

  test('E2E-STRESS-013 删除全部文档——配额归零', async ({ page }) => {
    // 导入文档
    await openKbModal(page);
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/doc1.md', '/mock/doc2.md', '/mock/doc3.md'] })
    );
    await page.waitForTimeout(500);

    // 删除所有文档
    const docs = await page.evaluate(() => window.__mock.state.docs.map(d => d.id));
    for (const docId of docs) {
      await page.evaluate((id) =>
        window.__TAURI__.core.invoke('delete_document', { id })
      , docId);
    }

    // 刷新
    await page.evaluate(() => {
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: 'refresh' } }));
    });
    await page.waitForTimeout(300);

    // 配额应归零
    await expect(page.locator('#kbDocCount')).toContainText('0/50');
  });

  test('E2E-STRESS-014 删除全部会话——空状态', async ({ page }) => {
    // 创建会话
    for (let i = 0; i < 3; i++) {
      await page.evaluate(() =>
        window.__TAURI__.core.invoke('create_conversation')
      );
    }
    await page.waitForTimeout(200);

    // 删除所有会话
    const convs = await page.evaluate(() => window.__mock.state.conversations.map(c => c.id));
    for (const convId of convs) {
      await page.evaluate((id) =>
        window.__TAURI__.core.invoke('delete_conversation', { id })
      , convId);
    }
    await page.waitForTimeout(300);

    // 会话列表应为空
    const convCount = await page.locator('#convList [data-conv-title]').count();
    expect(convCount).toBe(0);
  });

  // ─── 语言快速切换 ───

  test('E2E-STRESS-015 快速切换语言——不崩溃', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 快速切换语言 5 次
    for (let i = 0; i < 5; i++) {
      await page.locator('#localeSelect').selectOption(i % 2 === 0 ? 'en' : 'zh-CN');
      await page.waitForTimeout(100);
    }

    // 应用不应崩溃
    await expect(page.locator('#settingsModal')).toBeVisible();
    await page.locator('#settingsClose').click();
    await expect(page.locator('#app')).toBeVisible();
  });
});
