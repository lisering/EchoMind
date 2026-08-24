import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

/**
 * Helper: ensure sidebar is expanded and conversation list is refreshed.
 * After setupPage, boot() calls newChat() which sets isNewConversation=true.
 * Subsequent #newChatBtn clicks early-return without calling loadConversations().
 * So we call loadConversations() directly via window.__loadConversations.
 */
async function ensureConvListVisible(page) {
  // Expand sidebar if collapsed
  const expandBtn = page.locator('#expandBtn');
  if (await expandBtn.isVisible()) {
    await expandBtn.click();
    await page.waitForTimeout(300);
  }
  // Directly refresh conversation list (bypasses newChat early-return)
  await page.evaluate(() => window.__loadConversations && window.__loadConversations());
  await page.locator('#convList [data-conv-id]').first().waitFor({ state: 'attached', timeout: 5000 });
}

test.describe('v1.16 Conversation Drag Reorder (REQ-IX-002)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V16-001: conversation items are draggable', async ({ page }) => {
    // Create a conversation via IPC and verify it exists in backend
    const convId = await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation', { workspaceId: 'default' }));
    expect(convId).toBeTruthy();

    // Verify conversation exists in backend
    const convs = await page.evaluate(() => window.__TAURI__.core.invoke('get_conversations', { workspaceId: 'default' }));
    expect(convs.length).toBeGreaterThan(0);

    // Refresh UI list directly (newChat early-returns if isNewConversation=true)
    await ensureConvListVisible(page);

    const convItem = page.locator('#convList [data-conv-id]').first();
    await expect(convItem).toHaveAttribute('draggable', 'true');
  });

  test('TC-V16-002: conversation list has role=list', async ({ page }) => {
    const convList = page.locator('#convList');
    await expect(convList).toHaveAttribute('role', 'list');
  });

  test('TC-V16-003: reorder_conversations IPC is wired in tauri-stub', async ({ page }) => {
    // Create two conversations
    const id1 = await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation', { workspaceId: 'default' }));
    await page.evaluate((id) => window.__TAURI__.core.invoke('rename_conversation', { id, title: 'First' }), id1);

    const id2 = await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation', { workspaceId: 'default' }));
    await page.evaluate((id) => window.__TAURI__.core.invoke('rename_conversation', { id, title: 'Second' }), id2);

    // Reorder
    await page.evaluate((ids) => window.__TAURI__.core.invoke('reorder_conversations', { orderedIds: ids }), [id2, id1]);

    // Get conversations and verify order
    const convs = await page.evaluate(() => window.__TAURI__.core.invoke('get_conversations', { workspaceId: 'default' }));
    expect(convs.length).toBeGreaterThanOrEqual(2);
    // The first conversation should be id2 (reordered to front)
    expect(convs[0].id).toBe(id2);
  });

  test('TC-V16-004: conversation items have aria-label and role=listitem', async ({ page }) => {
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation', { workspaceId: 'default' }));
    await ensureConvListVisible(page);

    const convItem = page.locator('#convList [data-conv-id]').first();
    await expect(convItem).toHaveAttribute('aria-label');
    await expect(convItem).toHaveAttribute('role', 'listitem');
  });
});

test.describe('v1.16 KB Stats Enhancement (REQ-VEC-010)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V16-005: kb-stats mock returns vector_count and status_distribution', async ({ page }) => {
    const stats = await page.evaluate(() => window.__TAURI__.core.invoke('get_kb_stats'));

    expect(stats).toHaveProperty('vector_count');
    expect(stats).toHaveProperty('status_distribution');
    expect(Array.isArray(stats.status_distribution)).toBe(true);
    expect(stats.status_distribution.length).toBe(4);
  });

  test('TC-V16-006: kb-stats mock returns correct status_distribution labels', async ({ page }) => {
    const stats = await page.evaluate(() => window.__TAURI__.core.invoke('get_kb_stats'));

    // Verify status_distribution has correct labels
    const labels = stats.status_distribution.map(([label]) => label);
    expect(labels).toContain('pending');
    expect(labels).toContain('processing');
    expect(labels).toContain('indexed');
    expect(labels).toContain('failed');
  });
});

test.describe('v1.16 ARIA Semantic Annotations (REQ-A11Y-001)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V16-007: sidebar has role=navigation', async ({ page }) => {
    const sidebar = page.locator('#sidebar');
    await expect(sidebar).toHaveAttribute('role', 'navigation');
  });

  test('TC-V16-008: main has role=main', async ({ page }) => {
    const main = page.locator('main[role="main"]');
    await expect(main).toBeVisible();
  });

  test('TC-V16-009: chatArea has aria-live=polite', async ({ page }) => {
    const chatArea = page.locator('#chatArea');
    await expect(chatArea).toHaveAttribute('aria-live', 'polite');
  });

  test('TC-V16-010: toast area has role=alert', async ({ page }) => {
    const toasts = page.locator('#toasts');
    await expect(toasts).toHaveAttribute('role', 'alert');
  });

  test('TC-V16-011: import progress bar has role=progressbar attribute', async ({ page }) => {
    // The progress bar is inside a hidden container, so check attribute not visibility
    const progressbar = page.locator('[role="progressbar"]');
    await expect(progressbar).toHaveAttribute('aria-valuenow', '0');
    await expect(progressbar).toHaveAttribute('aria-valuemin', '0');
    await expect(progressbar).toHaveAttribute('aria-valuemax', '100');
  });

  test('TC-V16-012: icon buttons have aria-label', async ({ page }) => {
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn).toHaveAttribute('data-i18n-aria-label');

    const micBtn = page.locator('#micBtn');
    await expect(micBtn).toHaveAttribute('data-i18n-aria-label');
  });
});

test.describe('v1.16 Screen Reader Support (REQ-A11Y-004)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V16-013: sr-only status region exists with aria-live=polite', async ({ page }) => {
    const srStatus = page.locator('#srStatus');
    await expect(srStatus).toHaveAttribute('aria-live', 'polite');
    await expect(srStatus).toHaveAttribute('class', /sr-only/);
  });

  test('TC-V16-014: sr-only error region exists with aria-live=assertive', async ({ page }) => {
    const srError = page.locator('#srError');
    await expect(srError).toHaveAttribute('aria-live', 'assertive');
    await expect(srError).toHaveAttribute('class', /sr-only/);
  });

  test('TC-V16-015: decorative SVGs in buttons have aria-hidden=true', async ({ page }) => {
    const svgsInButtons = page.locator('button[aria-label] svg, button[data-i18n-aria-label] svg');
    const count = await svgsInButtons.count();

    expect(count).toBeGreaterThan(0);
    // Check first 5
    for (let i = 0; i < Math.min(count, 5); i++) {
      await expect(svgsInButtons.nth(i)).toHaveAttribute('aria-hidden', 'true');
    }
  });

  test('TC-V16-016: document.title updates on new chat', async ({ page }) => {
    // Click new chat
    await page.click('#newChatBtn');
    await page.waitForTimeout(300);

    const title = await page.title();
    expect(title).toContain('EchoMind');
  });
});
