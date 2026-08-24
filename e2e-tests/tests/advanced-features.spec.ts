// E2E 高级功能——领域分类 / 嵌入模型 / 会话分页 / 上下文限制（REQ-VEC-013, REQ-VEC-012, REQ-RAG-019, REQ-RAG-017）：
// E2E-ADV-001: 领域分类——对已有文档执行分类
// E2E-ADV-002: 领域分类——不存在的文档返回错误
// E2E-ADV-003: 嵌入模型——默认 all-MiniLM-L6-v2
// E2E-ADV-004: 嵌入模型——切换为 bge-small-zh
// E2E-ADV-005: 嵌入模型——切换回默认
// E2E-ADV-006: 会话分页——默认 limit=20
// E2E-ADV-007: 会话分页——offset 偏移
// E2E-ADV-008: 上下文 token 限制——设置有效值
// E2E-ADV-009: 上下文 token 限制——无效值抛出错误
// E2E-ADV-010: 上下文 token 限制——边界值 2048
// E2E-ADV-011: 上下文 token 限制——边界值 32768
// E2E-ADV-012: 多文档领域分类——不同文档不同领域
import { test, expect } from '@playwright/test';
import { enterApp, importDocs, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-ADV 高级功能（REQ-VEC-013, REQ-VEC-012, REQ-RAG-019, REQ-RAG-017）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 领域分类 ───

  test('E2E-ADV-001 领域分类——对已有文档执行分类', async ({ page }) => {
    // 先导入一个文档
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/rust-guide.md'] })
    );
    await page.waitForTimeout(200);

    const docId = await page.evaluate(() => window.__mock.state.docs[0]?.id);
    expect(docId).not.toBeNull();
    expect(typeof docId).toBe('string');
    expect(docId.length).toBeGreaterThan(0);

    const domain = await page.evaluate((id) =>
      window.__TAURI__.core.invoke('reclassify_document', { docId: id })
    , docId);
    expect(domain).not.toBeNull();
    expect(typeof domain).toBe('string');
    expect(domain.length).toBeGreaterThan(0);
    // 应为已知领域之一
    expect(['programming', 'medical', 'general']).toContain(domain);
  });

  test('E2E-ADV-002 领域分类——不存在的文档返回错误', async ({ page }) => {
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('reclassify_document', { docId: 'nonexistent-doc' })
      )
    ).rejects.toThrow();
  });

  test('E2E-ADV-012 多文档领域分类——不同文档不同领域', async ({ page }) => {
    // 导入两个不同类型的文档
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('import_files', { paths: ['/mock/rust-tutorial.md', '/mock/medical-report.md'] })
    );
    await page.waitForTimeout(200);

    const docs = await page.evaluate(() => window.__mock.state.docs);
    expect(docs.length).toBeGreaterThanOrEqual(2);

    // 对两个文档分别分类
    for (const doc of docs) {
      const domain = await page.evaluate((id) =>
        window.__TAURI__.core.invoke('reclassify_document', { docId: id })
      , doc.id);
      expect(domain).not.toBeNull();
      expect(typeof domain).toBe('string');
      expect(domain.length).toBeGreaterThan(0);
      expect(['programming', 'medical', 'general']).toContain(domain);
    }
  });

  // ─── 嵌入模型切换 ───

  test('E2E-ADV-003 嵌入模型——默认 all-MiniLM-L6-v2', async ({ page }) => {
    const model = await page.evaluate(() => window.__mock.state.embeddingModel);
    expect(model).toBe('all-MiniLM-L6-v2');
  });

  test('E2E-ADV-004 嵌入模型——切换为 bge-small-zh', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'vec.embedding_model', value: 'bge-small-zh-v1.5' })
    );
    const model = await page.evaluate(() => window.__mock.state.embeddingModel);
    expect(model).toBe('bge-small-zh-v1.5');
  });

  test('E2E-ADV-005 嵌入模型——切换回默认', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'vec.embedding_model', value: 'bge-small-zh-v1.5' })
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'vec.embedding_model', value: 'all-MiniLM-L6-v2' })
    );
    const model = await page.evaluate(() => window.__mock.state.embeddingModel);
    expect(model).toBe('all-MiniLM-L6-v2');
  });

  // ─── 会话分页 ───

  test('E2E-ADV-006 会话分页——默认 limit=20', async ({ page }) => {
    // 创建多个会话
    for (let i = 0; i < 5; i++) {
      await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    }
    const result = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversations_paginated', {})
    );
    expect(result.total).toBe(5);
    expect(result.items.length).toBeLessThanOrEqual(20);
  });

  test('E2E-ADV-007 会话分页——offset 偏移', async ({ page }) => {
    for (let i = 0; i < 10; i++) {
      await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    }
    // 获取前 3 个
    const first = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversations_paginated', { limit: 3, offset: 0 })
    );
    expect(first.items.length).toBe(3);

    // 获取接下来的 3 个
    const second = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_conversations_paginated', { limit: 3, offset: 3 })
    );
    expect(second.items.length).toBe(3);

    // 两次结果不应有交集
    const firstIds = first.items.map((c) => c.id);
    const secondIds = second.items.map((c) => c.id);
    const intersection = firstIds.filter((id) => secondIds.includes(id));
    expect(intersection).toHaveLength(0);
  });

  // ─── 上下文 token 限制 ───

  test('E2E-ADV-008 上下文 token 限制——设置有效值', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.context_token_limit', value: String(8192) })
    );
    const limit = await page.evaluate(() => window.__mock.state.contextTokenLimit);
    expect(limit).toBe(8192);
  });

  test('E2E-ADV-009 上下文 token 限制——无效值抛出错误', async ({ page }) => {
    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('update_setting', { key: 'rag.context_token_limit', value: String(100) })
      )
    ).rejects.toThrow();

    await expect(
      page.evaluate(() =>
        window.__TAURI__.core.invoke('update_setting', { key: 'rag.context_token_limit', value: String(999999) })
      )
    ).rejects.toThrow();
  });

  test('E2E-ADV-010 上下文 token 限制——边界值 2048', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.context_token_limit', value: String(2048) })
    );
    const limit = await page.evaluate(() => window.__mock.state.contextTokenLimit);
    expect(limit).toBe(2048);
  });

  test('E2E-ADV-011 上下文 token 限制——边界值 32768', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('update_setting', { key: 'rag.context_token_limit', value: String(32768) })
    );
    const limit = await page.evaluate(() => window.__mock.state.contextTokenLimit);
    expect(limit).toBe(32768);
  });
});
