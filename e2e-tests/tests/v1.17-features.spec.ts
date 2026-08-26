/**
 * EchoMind v1.17 E2E 测试 — REQ-A11Y-005 高对比度 + REQ-RAG-018 用户建议 + REQ-EXP-003 数据恢复
 *
 * 测试覆盖：
 * - TC-V17-HC-001~004: 高对比度模式（REQ-A11Y-005）
 * - TC-V17-SUG-001~003: 用户提问建议（REQ-RAG-018）
 * - TC-V17-BACKUP-001~003: 数据备份与恢复（REQ-EXP-002/003）
 */
import { test, expect } from '@playwright/test';
import { injectStub, injectLocales, uiUrl, showAllSettingsSections } from './helpers.mjs';

/**
 * 设置带文档数据的测试页面（用于建议卡片测试）。
 * 在 setupPage 之前注入 mock 文档数据。
 */
async function setupPageWithDocs(page, docs) {
  await injectStub(page);
  await page.addInitScript((d) => {
    window.__state.configured = true;
    window.__state.docs = d;
    window.__state.docCount = d.length;
  }, docs);
  await injectLocales(page);
  await page.goto(uiUrl);
  await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
}

// ============================================================
// REQ-A11Y-005 高对比度模式
// ============================================================

test.describe('v1.17 REQ-A11Y-005 High Contrast Mode', () => {
  test('TC-V17-HC-001: high-contrast theme button exists in settings', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#settingsBtn').click();
    await page.locator('#settingsModal').waitFor({ state: 'visible', timeout: 10000 });
    await showAllSettingsSections(page);

    const hcBtn = page.locator('[data-theme-value="high-contrast"]');
    await expect(hcBtn).toBeVisible();
  });

  test('TC-V17-HC-002: clicking high-contrast sets data-theme on root', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#settingsBtn').click();
    await page.locator('#settingsModal').waitFor({ state: 'visible', timeout: 10000 });
    await showAllSettingsSections(page);

    await page.locator('[data-theme-value="high-contrast"]').click();

    const theme = await page.evaluate(() => document.documentElement.dataset.theme);
    expect(theme).toBe('high-contrast');
  });

  test('TC-V17-HC-003: high-contrast uses pure black surface', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'high-contrast';
    });

    const surface0 = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return cs.getPropertyValue('--surface-0').trim();
    });
    expect(surface0).toBe('#000000' as any);
  });

  test('TC-V17-HC-004: high-contrast text primary is pure white', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'high-contrast';
    });

    const textPrimary = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return cs.getPropertyValue('--text-primary').trim();
    });
    expect(textPrimary).toBe('#FFFFFF' as any);
  });
});

// ============================================================
// REQ-RAG-018 用户提问建议
// ============================================================

test.describe('v1.17 REQ-RAG-018 User Question Suggestions', () => {
  test('TC-V17-SUG-001: empty state shows suggestion cards when docs exist', async ({ page }) => {
    const mockDocs = [
      { id: 'd1', file_path: '实验设计.md', name: '实验设计.md', title: '实验设计', file_hash: 'h1', status: 'Ready', created_at: 0 },
      { id: 'd2', file_path: '数据分析.pdf', name: '数据分析.pdf', title: '数据分析', file_hash: 'h2', status: 'Ready', created_at: 0 },
      { id: 'd3', file_path: '报告.docx', name: '报告.docx', title: '报告', file_hash: 'h3', status: 'Ready', created_at: 0 },
    ];
    await setupPageWithDocs(page, mockDocs);

    // 点击新对话按钮触发空状态
    await page.locator('#newChatBtn').click();
    await page.waitForTimeout(500);

    // 验证建议卡片存在
    const cards = page.locator('.empty-state-suggestion-card');
    await cards.first().waitFor({ state: 'visible', timeout: 10000 });
    const count = await cards.count();
    expect(count).toBeGreaterThanOrEqual(3);
  });

  test('TC-V17-SUG-002: suggestion cards include document-name-based questions', async ({ page }) => {
    const mockDocs = [
      { id: 'd1', file_path: '实验设计.md', name: '实验设计.md', title: '实验设计', file_hash: 'h1', status: 'Ready', created_at: 0 },
      { id: 'd2', file_path: '数据分析.pdf', name: '数据分析.pdf', title: '数据分析', file_hash: 'h2', status: 'Ready', created_at: 0 },
    ];
    await setupPageWithDocs(page, mockDocs);

    await page.locator('#newChatBtn').click();
    await page.waitForTimeout(500);

    // 验证第一张卡片包含文档名
    const cards = page.locator('.empty-state-suggestion-card');
    await cards.first().waitFor({ state: 'visible', timeout: 10000 });
    const text = await cards.first().textContent();
    expect(text).toContain('实验设计');
  });

  test('TC-V17-SUG-003: empty KB shows import guide instead of suggestions', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => {
      window.__state.configured = true;
      window.__state.docs = [];
      window.__state.docCount = 0;
    });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });

    await page.locator('#newChatBtn').click();

    // 验证导入引导按钮存在
    const importBtn = page.locator('.empty-state-import-btn');
    await expect(importBtn).toBeVisible();

    // 验证没有建议卡片
    const cards = page.locator('.empty-state-suggestion-card');
    const count = await cards.count();
    expect(count).toBe(0);
  });
});

// ============================================================
// REQ-EXP-002/003 数据备份与恢复
// ============================================================

test.describe('v1.17 REQ-EXP-002/003 Data Backup & Restore', () => {
  test('TC-V17-BACKUP-001: export backup button exists in settings', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#settingsBtn').click();
    await page.locator('#settingsModal').waitFor({ state: 'visible', timeout: 10000 });
    await showAllSettingsSections(page);

    const exportBtn = page.locator('#exportBackupBtn');
    await expect(exportBtn).toBeVisible();
  });

  test('TC-V17-BACKUP-002: import backup button exists in settings', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#settingsBtn').click();
    await page.locator('#settingsModal').waitFor({ state: 'visible', timeout: 10000 });
    await showAllSettingsSections(page);

    const importBtn = page.locator('#importBackupBtn');
    await expect(importBtn).toBeVisible();
  });

  test('TC-V17-BACKUP-003: export_backup IPC returns valid JSON', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => { window.__state.configured = true; });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });

    const result = await page.evaluate(async () => {
      const json = await window.__TAURI__.core.invoke('export_backup');
      const data = JSON.parse(json);
      return {
        version: data.version,
        hasConversations: Array.isArray(data.conversations),
        hasMessages: typeof data.messages === 'object',
        hasDocuments: Array.isArray(data.documents),
        hasSettings: typeof data.settings === 'object',
        hasExportedAt: typeof data.exported_at === 'string',
      };
    });

    expect(result.version).toBe(1);
    expect(result.hasConversations).toBe(true);
    expect(result.hasMessages).toBe(true);
    expect(result.hasDocuments).toBe(true);
    expect(result.hasSettings).toBe(true);
    expect(result.hasExportedAt).toBe(true);
  });
});
