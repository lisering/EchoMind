// E2E 文档列表搜索与筛选（REQ-ING-007）。
// AC-1: 搜索框输入关键词后，列表仅显示文件名包含该关键词的文档（大小写不敏感）
// AC-2: 选择「失败」筛选后，列表仅显示 Failed 状态文档
// AC-3: 搜索与筛选条件可组合使用，结果取交集
// AC-4: 无匹配结果时显示「未找到匹配文档」空状态
import { test, expect } from '@playwright/test';
import { setupPage, importDocs, injectLocales, openKbModal, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-ING-007 文档列表搜索与筛选', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 导入多个不同格式和名称的文档
    await importDocs(page, [
      '/mock/rust-guide.md',
      '/mock/python-tutorial.txt',
      '/mock/rust-async.md',
      '/mock/failed-doc.txt',
    ]);
    await openKbModal(page);
  });

  test('AC-1 搜索框按文件名模糊搜索（大小写不敏感）', async ({ page }) => {
    const searchInput = page.locator('#docSearchInput');
    await searchInput.fill('rust');
    await page.waitForTimeout(300);

    const items = page.locator('#docList [data-doc-name]');
    const visibleItems = await items.filter({ hasNot: page.locator('[style*="display: none"]') }).count();
    // 应显示 rust-guide.md 和 rust-async.md
    const visibleNames = await items.evaluateAll((els) =>
      els.filter((el) => el.style.display !== 'none').map((el) => el.dataset.docName)
    );
    expect(visibleNames).toContain('rust-guide.md');
    expect(visibleNames).toContain('rust-async.md');
    expect(visibleNames).not.toContain('python-tutorial.txt');
  });

  test('AC-1b 搜索大小写不敏感', async ({ page }) => {
    const searchInput = page.locator('#docSearchInput');
    await searchInput.fill('RUST');
    await page.waitForTimeout(300);

    const visibleNames = await page.locator('#docList [data-doc-name]').evaluateAll((els) =>
      els.filter((el) => el.style.display !== 'none').map((el) => el.dataset.docName)
    );
    expect(visibleNames).toContain('rust-guide.md');
    expect(visibleNames).toContain('rust-async.md');
  });

  test('AC-2 按索引状态筛选（Failed）', async ({ page }) => {
    // 将 failed-doc.txt 设为 Failed 状态
    await page.evaluate(() => {
      const doc = window.__state.docs.find((d) => d.file_path.includes('failed-doc'));
      if (doc) doc.status = 'Failed';
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    // 展开筛选面板（#kbFilterPanel 默认隐藏）
    await page.locator('#kbFilterToggle').click();
    await page.waitForTimeout(200);

    // 选择「失败」筛选
    const statusFilter = page.locator('#docStatusFilter');
    await statusFilter.waitFor({ state: 'visible', timeout: 5000 });
    await statusFilter.selectOption('Failed');
    await page.waitForTimeout(300);

    const visibleNames = await page.locator('#docList [data-doc-name]').evaluateAll((els) =>
      els.filter((el) => el.style.display !== 'none').map((el) => el.dataset.docName)
    );
    expect(visibleNames).toContain('failed-doc.txt');
    expect(visibleNames).not.toContain('rust-guide.md');
  });

  test('AC-2b 按索引状态筛选（Indexed）', async ({ page }) => {
    // 将 failed-doc.txt 设为 Failed
    await page.evaluate(() => {
      const doc = window.__state.docs.find((d) => d.file_path.includes('failed-doc'));
      if (doc) doc.status = 'Failed';
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    // 展开筛选面板
    await page.locator('#kbFilterToggle').click();
    await page.waitForTimeout(200);

    const statusFilter = page.locator('#docStatusFilter');
    await statusFilter.waitFor({ state: 'visible', timeout: 5000 });
    await statusFilter.selectOption('Indexed');
    await page.waitForTimeout(300);

    const visibleNames = await page.locator('#docList [data-doc-name]').evaluateAll((els) =>
      els.filter((el) => el.style.display !== 'none').map((el) => el.dataset.docName)
    );
    expect(visibleNames).toContain('rust-guide.md');
    expect(visibleNames).not.toContain('failed-doc.txt');
  });

  test('AC-3 搜索与状态筛选组合（取交集）', async ({ page }) => {
    // 将 failed-doc.txt 设为 Failed
    await page.evaluate(() => {
      const doc = window.__state.docs.find((d) => d.file_path.includes('failed-doc'));
      if (doc) doc.status = 'Failed';
      const listeners = window.__state.listeners['doc-status-changed'] || [];
      listeners.forEach((cb) => cb({ payload: { status: 'done', message: '刷新' } }));
    });
    await page.waitForTimeout(300);

    // 展开筛选面板
    await page.locator('#kbFilterToggle').click();
    await page.waitForTimeout(200);

    // 搜索 "rust" + 状态 Indexed
    await page.locator('#docSearchInput').fill('rust');
    const filterEl = page.locator('#docStatusFilter');
    await filterEl.waitFor({ state: 'visible', timeout: 5000 });
    await filterEl.selectOption('Indexed');
    await page.waitForTimeout(300);

    const visibleNames = await page.locator('#docList [data-doc-name]').evaluateAll((els) =>
      els.filter((el) => el.style.display !== 'none').map((el) => el.dataset.docName)
    );
    // 交集：rust-guide.md 和 rust-async.md（都是 Indexed 且包含 "rust"）
    expect(visibleNames).toContain('rust-guide.md');
    expect(visibleNames).toContain('rust-async.md');
    expect(visibleNames).not.toContain('failed-doc.txt');
    expect(visibleNames).not.toContain('python-tutorial.txt');
  });

  test('AC-3b 搜索与格式筛选组合（取交集）', async ({ page }) => {
    // 展开筛选面板
    await page.locator('#kbFilterToggle').click();
    await page.waitForTimeout(200);

    // 搜索 "rust" + 格式 .md
    await page.locator('#docSearchInput').fill('rust');
    const formatFilter = page.locator('#docFormatFilter');
    await formatFilter.waitFor({ state: 'visible', timeout: 5000 });
    await formatFilter.selectOption('md');
    await page.waitForTimeout(300);

    const visibleNames = await page.locator('#docList [data-doc-name]').evaluateAll((els) =>
      els.filter((el) => el.style.display !== 'none').map((el) => el.dataset.docName)
    );
    expect(visibleNames).toContain('rust-guide.md');
    expect(visibleNames).toContain('rust-async.md');
    expect(visibleNames).not.toContain('python-tutorial.txt');
  });

  test('AC-4 无匹配结果时显示空状态', async ({ page }) => {
    await page.locator('#docSearchInput').fill('nonexistent-file-xyz');
    await page.waitForTimeout(300);

    // 应显示「未找到匹配文档」空状态
    await expect(page.locator('#docList')).toContainText(/未找到|no.*match|no.*result/i, { timeout: 3000 });
  });
});
