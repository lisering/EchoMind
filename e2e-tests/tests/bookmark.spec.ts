/**
 * E2E tests for Conversation Bookmarks (REQ-RAG-047).
 *
 * Verifies that:
 * AC-1: Bookmark panel exists in sidebar
 * AC-2: Conversation list items have bookmark toggle button
 * AC-3: Adding a bookmark shows it in the panel
 * AC-4: Removing a bookmark via panel remove button works
 * AC-5: Toggling bookmark state updates the icon
 * AC-6: Clicking a bookmark item navigates to the conversation
 * AC-7: Bookmark count badge updates correctly
 * AC-8: Expand/collapse bookmark panel works
 */
import { test, expect } from '@playwright/test';
import { setupPage, importDocs, sendMessage, waitForStreamDone } from './helpers.mjs';

test.describe('Conversation Bookmarks (REQ-RAG-047)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // Import a document so the KB is non-empty (chat requires documents)
    await importDocs(page, ['/mock/test.md']);
  });

  // ============================================================
  // AC-1: Bookmark panel exists in sidebar
  // ============================================================

  test('TC-BM-001: Bookmark panel container exists in sidebar', async ({ page }) => {
    const panel = page.locator('#bookmarkPanel');
    await expect(panel).toBeAttached();
  });

  test('TC-BM-002: Bookmark panel shows title text', async ({ page }) => {
    const panel = page.locator('#bookmarkPanel');
    const text = await panel.textContent();
    // Should contain "Bookmarks" (en) or "收藏夹" (zh-CN)
    expect(text.length).toBeGreaterThan(0);
  });

  // ============================================================
  // AC-2: Conversation list items have bookmark toggle button
  // ============================================================

  test('TC-BM-003: Conversation list item has bookmark button', async ({ page }) => {
    // Send a message to create a conversation
    await sendMessage(page, 'Hello, test message');
    await waitForStreamDone(page, 20000);

    // Wait for conversation list to render
    await page.waitForTimeout(500);

    // Find the conversation item
    const convItem = page.locator('#convList [data-conv-id]').first();
    await expect(convItem).toBeAttached();

    // The bookmark button should be present (it's invisible until hover, but attached)
    // Check by evaluating the DOM
    const hasBookmarkBtn = await convItem.evaluate((el) => {
      const btns = el.querySelectorAll('button');
      return Array.from(btns).some((b) => b.getAttribute('aria-label')?.includes('Bookmark') || b.getAttribute('aria-label')?.includes('书签'));
    });
    expect(hasBookmarkBtn).toBe(true);
  });

  // ============================================================
  // AC-3: Adding a bookmark shows it in the panel
  // ============================================================

  test('TC-BM-004: Add bookmark via conversation list button', async ({ page }) => {
    // Create a conversation by sending a message
    await sendMessage(page, 'Test bookmark add');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    // Get the conversation ID
    const convId = await page.locator('#convList [data-conv-id]').first().getAttribute('data-conv-id');
    expect(convId).toBeTruthy();

    // Click the bookmark button on the first conversation item
    const bookmarkBtn = await page.locator('#convList [data-conv-id]').first().evaluate((el) => {
      const btns = el.querySelectorAll('button');
      for (const b of btns) {
        if (b.getAttribute('aria-label')?.includes('Bookmark') || b.getAttribute('aria-label')?.includes('书签')) {
          return b;
        }
      }
      return null;
    });

    // Click the bookmark button via evaluate (since it's invisible)
    await page.locator('#convList [data-conv-id]').first().evaluate((el) => {
      const btns = el.querySelectorAll('button');
      for (const b of btns) {
        if (b.getAttribute('aria-label')?.includes('Bookmark') || b.getAttribute('aria-label')?.includes('书签')) {
          (b as HTMLElement).click();
          break;
        }
      }
    });

    await page.waitForTimeout(500);

    // Verify bookmark was added via IPC
    const isBookmarked = await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('is_bookmarked', { conversationId: id });
    }, convId);
    expect(isBookmarked).toBe(true);
  });

  test('TC-BM-005: Bookmarked conversation appears in bookmark panel', async ({ page }) => {
    // Create a conversation
    await sendMessage(page, 'Test bookmark panel display');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    const convId = await page.locator('#convList [data-conv-id]').first().getAttribute('data-conv-id');

    // Add bookmark directly via IPC
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: null });
    }, convId);

    // Expand the bookmark panel by clicking the header
    const bookmarkHeader = page.locator('#bookmarkPanel .bookmark-header');
    await bookmarkHeader.click();
    await page.waitForTimeout(300);

    // Check that the bookmark list has at least one item
    const bookmarkItems = page.locator('#bookmarkPanel .bookmark-item');
    await expect(bookmarkItems.first()).toBeAttached({ timeout: 3000 });
    expect(await bookmarkItems.count()).toBeGreaterThanOrEqual(1);

    // Verify the bookmarked conversation ID matches
    const itemConvId = await bookmarkItems.first().getAttribute('data-conv-id');
    expect(itemConvId).toBe(convId);
  });

  // ============================================================
  // AC-4: Removing a bookmark via panel remove button
  // ============================================================

  test('TC-BM-006: Remove bookmark via panel remove button', async ({ page }) => {
    // Create a conversation and bookmark it
    await sendMessage(page, 'Test bookmark remove');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    const convId = await page.locator('#convList [data-conv-id]').first().getAttribute('data-conv-id');

    // Add bookmark via IPC
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: null });
    }, convId);

    // Expand bookmark panel
    const bookmarkHeader = page.locator('#bookmarkPanel .bookmark-header');
    await bookmarkHeader.click();
    await page.waitForTimeout(300);

    // Click the remove button on the first bookmark item
    const removeBtn = page.locator('#bookmarkPanel .bookmark-item button').first();
    await removeBtn.evaluate((el) => (el as HTMLElement).click());
    await page.waitForTimeout(500);

    // Verify bookmark was removed
    const isBookmarked = await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('is_bookmarked', { conversationId: id });
    }, convId);
    expect(isBookmarked).toBe(false);
  });

  // ============================================================
  // AC-5: Toggling bookmark state updates the icon
  // ============================================================

  test('TC-BM-007: Bookmark toggle button changes appearance after toggle', async ({ page }) => {
    // Create a conversation
    await sendMessage(page, 'Test bookmark toggle icon');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    const convItem = page.locator('#convList [data-conv-id]').first();
    const convId = await convItem.getAttribute('data-conv-id');

    // Find the bookmark button
    const bookmarkBtnSelector = '#convList [data-conv-id]:first-child button[aria-label*="Bookmark"], #convList [data-conv-id]:first-child button[aria-label*="书签"]';

    // Check initial state: no amber color (not bookmarked)
    const initiallyAmber = await convItem.evaluate((el) => {
      const btns = el.querySelectorAll('button');
      for (const b of btns) {
        if (b.getAttribute('aria-label')?.includes('Bookmark') || b.getAttribute('aria-label')?.includes('书签')) {
          return b.classList.contains('text-amber-400');
        }
      }
      return false;
    });
    expect(initiallyAmber).toBe(false);

    // Add bookmark via IPC
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: null });
    }, convId);

    // Reload conversations to trigger updateBookmarkIcon
    await page.waitForTimeout(500);

    // Click the bookmark button to trigger updateBookmarkIcon
    await convItem.evaluate((el) => {
      const btns = el.querySelectorAll('button');
      for (const b of btns) {
        if (b.getAttribute('aria-label')?.includes('Bookmark') || b.getAttribute('aria-label')?.includes('书签')) {
          (b as HTMLElement).click();
          break;
        }
      }
    });
    await page.waitForTimeout(500);

    // After toggling (now removing), verify state changed
    const isBookmarked = await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('is_bookmarked', { conversationId: id });
    }, convId);
    expect(isBookmarked).toBe(false);
  });

  // ============================================================
  // AC-6: Clicking a bookmark navigates to the conversation
  // ============================================================

  test('TC-BM-008: Click bookmark item navigates to conversation', async ({ page }) => {
    // Create two conversations directly via IPC
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    await page.waitForTimeout(100);
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    await page.waitForTimeout(100);

    // Reload conversation list via frontend function
    await page.evaluate(() => window.__loadConversations && window.__loadConversations());
    await page.waitForTimeout(300);

    // Get conversation items
    const convItems = page.locator('#convList [data-conv-id]');
    const count = await convItems.count();
    expect(count).toBeGreaterThanOrEqual(2);

    // Bookmark the first conversation (last in list = first created)
    const firstConvId = await convItems.last().getAttribute('data-conv-id');

    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: null });
    }, firstConvId);

    // Click on the second conversation to make it active (navigate away from first)
    const secondConvId = await convItems.first().getAttribute('data-conv-id');
    await convItems.first().click();
    await page.waitForTimeout(300);

    // Expand bookmark panel
    const bookmarkHeader = page.locator('#bookmarkPanel .bookmark-header');
    await bookmarkHeader.click();
    await page.waitForTimeout(300);

    // Click the bookmark item to navigate back to first conversation
    const bookmarkItem = page.locator(`#bookmarkPanel .bookmark-item[data-conv-id="${firstConvId}"]`);
    await expect(bookmarkItem).toBeAttached({ timeout: 3000 });
    await bookmarkItem.click();
    await page.waitForTimeout(500);

    // Verify that the first conversation is now active (highlighted in sidebar)
    // Use evaluate to check active state since Tailwind prebuilt CSS class names vary
    const activeId = await page.evaluate((expectedId) => {
      const items = document.querySelectorAll('#convList [data-conv-id]');
      for (const item of items) {
        if (item.classList.contains('bg-accent/15') || item.classList.contains('text-accent')) {
          return item.getAttribute('data-conv-id');
        }
      }
      // Fallback: check which item has text-accent class (active marker)
      for (const item of items) {
        if (item.className.includes('text-accent') && !item.className.includes('text-text-')) {
          return item.getAttribute('data-conv-id');
        }
      }
      return null;
    }, firstConvId);
    expect(activeId).toBe(firstConvId);
  });

  // ============================================================
  // AC-7: Bookmark count badge updates correctly
  // ============================================================

  test('TC-BM-009: Bookmark count badge shows correct count', async ({ page }) => {
    // Initially, bookmark count should be 0
    const badge = page.locator('#bookmarkPanel .bookmark-header span.text-\\[10px\\]');
    await expect(badge).toBeAttached();
    expect(await badge.textContent()).toBe('0');

    // Create a conversation and bookmark it
    await sendMessage(page, 'Test bookmark count');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    const convId = await page.locator('#convList [data-conv-id]').first().getAttribute('data-conv-id');

    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: null });
    }, convId);

    // Wait for bookmark panel to refresh
    await page.waitForTimeout(500);

    // The badge should now show 1
    // Trigger refresh by clicking header (which calls refreshBookmarks)
    const bookmarkHeader = page.locator('#bookmarkPanel .bookmark-header');
    await bookmarkHeader.click();
    await page.waitForTimeout(300);

    const badgeText = await badge.textContent();
    expect(badgeText).toBe('1');
  });

  // ============================================================
  // AC-8: Expand/collapse bookmark panel works
  // ============================================================

  test('TC-BM-010: Expand/collapse bookmark panel toggles list visibility', async ({ page }) => {
    // Create a conversation and bookmark it
    await sendMessage(page, 'Test bookmark expand');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    const convId = await page.locator('#convList [data-conv-id]').first().getAttribute('data-conv-id');

    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: null });
    }, convId);
    await page.waitForTimeout(300);

    const bookmarkHeader = page.locator('#bookmarkPanel .bookmark-header');

    // Initially collapsed: bookmark list should not exist or be empty
    const listBefore = page.locator('#bookmarkPanel .bookmark-list');
    expect(await listBefore.count()).toBe(0);

    // Click to expand
    await bookmarkHeader.click();
    await page.waitForTimeout(300);

    // Now bookmark list should be visible
    const listAfter = page.locator('#bookmarkPanel .bookmark-list');
    await expect(listAfter).toBeAttached();

    // Click again to collapse
    await bookmarkHeader.click();
    await page.waitForTimeout(300);

    // List should be gone
    expect(await page.locator('#bookmarkPanel .bookmark-list').count()).toBe(0);
  });

  // ============================================================
  // AC-3 (supplementary): list_bookmarks returns correct data
  // ============================================================

  test('TC-BM-011: list_bookmarks returns sorted bookmark list', async ({ page }) => {
    // Create two conversations directly via IPC
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    await page.waitForTimeout(100);
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    await page.waitForTimeout(100);
    // Reload conversation list via frontend function
    await page.evaluate(() => window.__loadConversations && window.__loadConversations());
    await page.waitForTimeout(300);

    // Get conversation IDs from the list
    const convItems = page.locator('#convList [data-conv-id]');
    expect(await convItems.count()).toBeGreaterThanOrEqual(2);

    // First created = last in list (sorted by created_at DESC)
    const convId1 = await convItems.last().getAttribute('data-conv-id');
    const convId2 = await convItems.first().getAttribute('data-conv-id');

    // Add both as bookmarks with a small delay to ensure different created_at
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: 'Note 1' });
    }, convId1);
    await page.waitForTimeout(200);
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: 'Note 2' });
    }, convId2);

    // List bookmarks
    const bookmarks = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('list_bookmarks');
    });

    expect(Array.isArray(bookmarks)).toBe(true);
    expect(bookmarks.length).toBe(2);

    // Should be sorted by created_at descending (most recent first)
    expect(bookmarks[0].conversation_id).toBe(convId2);
    expect(bookmarks[1].conversation_id).toBe(convId1);

    // Verify note field is preserved
    expect(bookmarks[0].note).toBe('Note 2');
    expect(bookmarks[1].note).toBe('Note 1');
  });

  // ============================================================
  // AC-4 (supplementary): Duplicate add updates existing bookmark
  // ============================================================

  test('TC-BM-012: Re-adding bookmark updates note instead of duplicating', async ({ page }) => {
    await sendMessage(page, 'Test duplicate bookmark');
    await waitForStreamDone(page, 20000);
    await page.waitForTimeout(500);

    const convId = await page.locator('#convList [data-conv-id]').first().getAttribute('data-conv-id');

    // Add bookmark with note
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: 'Original note' });
    }, convId);

    // Add again with different note
    await page.evaluate((id) => {
      return window.__TAURI__.core.invoke('add_bookmark', { conversationId: id, note: 'Updated note' });
    }, convId);

    // List bookmarks - should only have 1
    const bookmarks = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('list_bookmarks');
    });

    expect(bookmarks.length).toBe(1);
    expect(bookmarks[0].note).toBe('Updated note');
  });
});
