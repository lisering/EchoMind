import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';

test.describe('Wiki 双向链接功能（REQ-ING-020）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('TC-ING-WIKI-E2E-001: IPC get_forward_links 命令正常调用', async ({ page }) => {
    // Set up mock data
    await page.evaluate(() => {
      window.__mock.state.wikiLinks = [
        { id: 'wl-1', source_doc_id: 'doc-1', target: '设计文档', chunk_id: 'chunk-1', created_at: 1700000000 },
        { id: 'wl-2', source_doc_id: 'doc-2', target: 'API文档', chunk_id: 'chunk-2', created_at: 1700000001 },
      ];
    });

    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_forward_links', { docId: 'doc-1' });
    });

    expect(result).toHaveLength(1);
    expect(result[0].target).toBe('设计文档');
    expect(result[0].source_doc_id).toBe('doc-1');
  });

  test('TC-ING-WIKI-E2E-002: IPC get_backlinks 命令正常调用', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.wikiLinks = [
        { id: 'wl-1', source_doc_id: 'doc-1', target: '设计文档', chunk_id: 'chunk-1', created_at: 1700000000 },
        { id: 'wl-2', source_doc_id: 'doc-2', target: '设计文档', chunk_id: 'chunk-2', created_at: 1700000001 },
        { id: 'wl-3', source_doc_id: 'doc-3', target: 'API文档', chunk_id: 'chunk-3', created_at: 1700000002 },
      ];
    });

    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_backlinks', { docName: '设计文档' });
    });

    expect(result).toHaveLength(2);
    expect(result[0].target).toBe('设计文档');
    expect(result[1].target).toBe('设计文档');
  });

  test('TC-ING-WIKI-E2E-003: IPC rebuild_wiki_links 命令正常调用', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.wikiLinks = [
        { id: 'wl-1', source_doc_id: 'doc-1', target: 'A', chunk_id: 'c1', created_at: 1700000000 },
        { id: 'wl-2', source_doc_id: 'doc-1', target: 'B', chunk_id: 'c2', created_at: 1700000001 },
      ];
    });

    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('rebuild_wiki_links');
    });

    expect(result).toBe(2);
  });

  test('TC-ING-WIKI-E2E-004: wiki-link mock 数据正确存储和读取', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.wikiLinks = [
        { id: 'wl-1', source_doc_id: 'doc-1', target: '测试文档', chunk_id: 'c1', created_at: 1700000000 },
      ];
    });

    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_forward_links', { docId: 'doc-1' });
    });

    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('wl-1');
    expect(result[0].target).toBe('测试文档');
  });

  test('TC-ING-WIKI-E2E-005: 反向链接模糊匹配正确', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.wikiLinks = [
        { id: 'wl-1', source_doc_id: 'doc-1', target: '设计文档', chunk_id: 'c1', created_at: 1700000000 },
        { id: 'wl-2', source_doc_id: 'doc-2', target: '设计文档v2', chunk_id: 'c2', created_at: 1700000001 },
        { id: 'wl-3', source_doc_id: 'doc-3', target: 'API文档', chunk_id: 'c3', created_at: 1700000002 },
      ];
    });

    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_backlinks', { docName: '设计' });
    });

    expect(result).toHaveLength(2);
    const targets = result.map((r: any) => r.target);
    expect(targets).toContain('设计文档');
    expect(targets).toContain('设计文档v2');
  });

  test('TC-ING-WIKI-E2E-006: 无链接时返回空列表', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.wikiLinks = [];
    });

    const forwardResult = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_forward_links', { docId: 'nonexistent' });
    });
    expect(forwardResult).toHaveLength(0);

    const backResult = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_backlinks', { docName: 'nonexistent' });
    });
    expect(backResult).toHaveLength(0);
  });

  test('TC-ING-WIKI-E2E-007: wiki-links state 初始为空数组', async ({ page }) => {
    const initial = await page.evaluate(() => {
      return window.__mock.state.wikiLinks;
    });
    expect(Array.isArray(initial)).toBe(true);
  });

  test('TC-ING-WIKI-E2E-008: 应用加载后 UI 正常显示', async ({ page }) => {
    // Basic sanity check that the app loaded
    const sidebar = page.locator('#sidebar');
    await expect(sidebar).toBeVisible();
  });
});
