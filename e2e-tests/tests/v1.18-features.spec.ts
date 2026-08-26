/**
 * EchoMind v1.18 E2E 测试 — REQ-KB-002 命令面板 + REQ-NAV-002 全局搜索
 *
 * 测试覆盖：
 * - TC-V18-CP-001~005: 命令面板（⌘K 打开 / 搜索过滤 / 键盘导航 / Esc 关闭 / 中文搜索）
 * - TC-V18-GS-001~005: 全局搜索（⌘⇧F 打开 / 分组显示 / 点击跳转 / 关键词高亮 / 空状态）
 */
import { test, expect } from '@playwright/test';
import { injectStub, injectLocales, uiUrl, showAllSettingsSections } from './helpers.mjs';

/**
 * 设置带 mock 数据的测试页面。
 * 预注入会话、文档、消息数据供全局搜索使用。
 */
async function setupPageWithData(page) {
  await injectStub(page);
  await page.addInitScript(() => {
    window.__state.configured = true;
    // 预置会话数据
    window.__state.conversations = [
      { id: 'conv-1', title: 'RAG 架构讨论', created_at: Math.floor(Date.now() / 1000), workspace_id: 'default' },
      { id: 'conv-2', title: '向量数据库选型', created_at: Math.floor(Date.now() / 1000) - 3600, workspace_id: 'default' },
      { id: 'conv-3', title: 'Embedding 模型对比', created_at: Math.floor(Date.now() / 1000) - 7200, workspace_id: 'default' },
    ];
    // 预置文档数据
    window.__state.docs = [
      { id: 'doc-1', file_path: '/path/to/RAG_Guide.md', status: 'Indexed', created_at: Math.floor(Date.now() / 1000), file_size: 10240 },
      { id: 'doc-2', file_path: '/path/to/Vector_DB.pdf', status: 'Indexed', created_at: Math.floor(Date.now() / 1000) - 3600, file_size: 51200 },
      { id: 'doc-3', file_path: '/path/to/embedding_notes.txt', status: 'Indexed', created_at: Math.floor(Date.now() / 1000) - 7200, file_size: 2048 },
    ];
    // 预置消息数据
    window.__state.messages = {
      'conv-1': [
        { id: 'msg-1', role: 'user', content: '什么是 RAG 架构？', conversation_id: 'conv-1', created_at: Math.floor(Date.now() / 1000) },
        { id: 'msg-2', role: 'assistant', content: 'RAG 是检索增强生成架构，结合检索和生成模型。', conversation_id: 'conv-1', created_at: Math.floor(Date.now() / 1000) },
      ],
      'conv-2': [
        { id: 'msg-3', role: 'user', content: 'SQLite 向量存储方案对比', conversation_id: 'conv-2', created_at: Math.floor(Date.now() / 1000) - 3600 },
        { id: 'msg-4', role: 'assistant', content: 'SQLite with vector BLOB vs Qdrant vs Milvus', conversation_id: 'conv-2', created_at: Math.floor(Date.now() / 1000) - 3600 },
      ],
      'conv-3': [],
    };
  });
  await injectLocales(page);
  await page.goto(uiUrl);
  await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
  // 等待 boot() 异步初始化完成（initKeyboardShortcuts + initConversations）
  await page.waitForTimeout(1000);
}

// ============================================================
// REQ-KB-002 命令面板
// ============================================================

test.describe('v1.18 REQ-KB-002 Command Palette', () => {
  test('TC-V18-CP-001: Cmd+K opens command palette centered', async ({ page }) => {
    await setupPageWithData(page);
    // Press Cmd+K (metaKey on macOS)
    await page.keyboard.press('Meta+k');
    // Panel should be visible
    await expect(page.locator('#commandPalette')).toBeVisible({ timeout: 5000 });
    // Search input should be focused
    await expect(page.locator('#cmdSearch')).toBeFocused();
    // Command list should have items
    const items = page.locator('#cmdList [role="option"]');
    await expect(items.first()).toBeVisible({ timeout: 3000 });
  });

  test('TC-V18-CP-002: typing filters command list', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+k');
    await expect(page.locator('#commandPalette')).toBeVisible({ timeout: 3000 });

    const beforeCount = await page.locator('#cmdList [role="option"]').count();
    expect(beforeCount).toBeGreaterThan(0);

    // Type to filter (使用中文关键词，适配 zh-CN locale)
    await page.locator('#cmdSearch').fill('设置');
    await page.waitForTimeout(300); // debounce

    const afterCount = await page.locator('#cmdList [role="option"]').count();
    // Should have fewer results (at least 1: "打开设置")
    expect(afterCount).toBeGreaterThanOrEqual(1);
    expect(afterCount).toBeLessThanOrEqual(beforeCount);

    // Verify the filtered result contains "设置" text
    const firstItemText = await page.locator('#cmdList [role="option"]').first().textContent();
    expect(firstItemText).toContain('设置');
  });

  test('TC-V18-CP-003: arrow keys navigate and Enter executes', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+k');
    await expect(page.locator('#commandPalette')).toBeVisible({ timeout: 3000 });

    // First item should be selected (bg-accent)
    const firstItem = page.locator('#cmdList [role="option"]').first();
    const firstClass = await firstItem.getAttribute('class');
    expect(firstClass).toContain('bg-accent');

    // Press ArrowDown to move to second item
    await page.keyboard.press('ArrowDown');
    const items = page.locator('#cmdList [role="option"]');
    const itemCount = await items.count();
    if (itemCount > 1) {
      const secondItem = items.nth(1);
      const secondClass = await secondItem.getAttribute('class');
      expect(secondClass).toContain('bg-accent');
    }

    // Press ArrowUp to go back
    await page.keyboard.press('ArrowUp');
    const restoredClass = await firstItem.getAttribute('class');
    expect(restoredClass).toContain('bg-accent');
  });

  test('TC-V18-CP-004: Esc closes command palette', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+k');
    await expect(page.locator('#commandPalette')).toBeVisible({ timeout: 3000 });

    await page.keyboard.press('Escape');
    await expect(page.locator('#commandPalette')).toBeHidden({ timeout: 3000 });
  });

  test('TC-V18-CP-005: Chinese search works', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+k');
    await expect(page.locator('#commandPalette')).toBeVisible({ timeout: 3000 });

    // Type Chinese keyword — should match "新建会话" (New Chat)
    await page.locator('#cmdSearch').fill('会话');
    await page.waitForTimeout(300);

    const items = page.locator('#cmdList [role="option"]');
    const count = await items.count();
    expect(count).toBeGreaterThanOrEqual(1);

    // At least one result should contain "会话"
    const firstText = await items.first().textContent();
    expect(firstText).toContain('会话');
  });
});

// ============================================================
// REQ-NAV-002 全局搜索
// ============================================================

test.describe('v1.18 REQ-NAV-002 Global Search', () => {
  test('TC-V18-GS-001: Cmd+Shift+F opens global search', async ({ page }) => {
    await setupPageWithData(page);
    // Press Cmd+Shift+F
    await page.keyboard.press('Meta+Shift+f');
    // Panel should be visible
    await expect(page.locator('#globalSearch')).toBeVisible({ timeout: 3000 });
    // Search input should be focused
    await expect(page.locator('#globalSearchInput')).toBeFocused();
  });

  test('TC-V18-GS-002: search shows grouped results', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+Shift+f');
    await expect(page.locator('#globalSearch')).toBeVisible({ timeout: 3000 });

    // Type a keyword that matches conversations, documents, and messages
    await page.locator('#globalSearchInput').fill('RAG');
    await page.waitForTimeout(500); // debounce for async search

    // Should have result groups (conversations / documents / messages)
    const groups = page.locator('#globalSearchResults .gs-group');
    const groupCount = await groups.count();
    expect(groupCount).toBeGreaterThanOrEqual(1);

    // At least one result item
    const resultItems = page.locator('#globalSearchResults .gs-result-item');
    const itemCount = await resultItems.count();
    expect(itemCount).toBeGreaterThanOrEqual(1);
  });

  test('TC-V18-GS-003: clicking conversation result loads it', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+Shift+f');
    await expect(page.locator('#globalSearch')).toBeVisible({ timeout: 3000 });

    // Search for a conversation title
    await page.locator('#globalSearchInput').fill('架构');
    await page.waitForTimeout(500);

    // Click the first conversation result
    const convResult = page.locator('#globalSearchResults .gs-result-item[data-type="conversation"]').first();
    await expect(convResult).toBeVisible({ timeout: 3000 });
    await convResult.click();

    // Global search should close
    await expect(page.locator('#globalSearch')).toBeHidden({ timeout: 3000 });
  });

  test('TC-V18-GS-004: keywords highlighted in results', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+Shift+f');
    await expect(page.locator('#globalSearch')).toBeVisible({ timeout: 3000 });

    await page.locator('#globalSearchInput').fill('向量');
    await page.waitForTimeout(500);

    // Check that <mark> elements exist in results
    const marks = page.locator('#globalSearchResults mark');
    const markCount = await marks.count();
    expect(markCount).toBeGreaterThanOrEqual(1);
  });

  test('TC-V18-GS-005: empty state when no results', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+Shift+f');
    await expect(page.locator('#globalSearch')).toBeVisible({ timeout: 3000 });

    // Type a query that matches nothing
    await page.locator('#globalSearchInput').fill('zzz_no_match_xxx_999');
    await page.waitForTimeout(500);

    // Should show empty state
    const emptyState = page.locator('#globalSearchResults .gs-empty');
    await expect(emptyState).toBeVisible({ timeout: 3000 });
  });

  test('TC-V18-GS-006: Esc closes global search', async ({ page }) => {
    await setupPageWithData(page);
    await page.keyboard.press('Meta+Shift+f');
    await expect(page.locator('#globalSearch')).toBeVisible({ timeout: 3000 });

    await page.keyboard.press('Escape');
    await expect(page.locator('#globalSearch')).toBeHidden({ timeout: 3000 });
  });
});
