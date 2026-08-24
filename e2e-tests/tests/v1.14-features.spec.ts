import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('v1.14 Context Menu Enhancement (REQ-IX-001)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 创建对话供右键菜单测试使用
    await page.evaluate(() => window.__TAURI__.core.invoke('create_conversation'));
    await page.waitForTimeout(200);
    await page.evaluate(() => window.__loadConversations && window.__loadConversations());
    await page.waitForTimeout(300);
  });

  test('TC-V14-MENU-001: Conversation list right-click shows context menu', async ({ page }) => {
    // Wait for conversation list to load
    await page.waitForSelector('[data-conv-id]', { timeout: 5000 });

    // Right-click on a conversation item
    const convItem = page.locator('[data-conv-id]').first();
    await convItem.click({ button: 'right' });

    // Verify context menu is visible
    const ctxMenu = page.locator('#ctxMenu.visible');
    await expect(ctxMenu).toBeVisible();

    // Verify menu items for conversation
    const renameItem = ctxMenu.locator('.ctx-item[data-action="convRename"]');
    await expect(renameItem).toBeVisible();
    await expect(renameItem).toContainText(/重命名|Rename/);

    const exportItem = ctxMenu.locator('.ctx-item[data-action="convExport"]');
    await expect(exportItem).toBeVisible();

    const deleteItem = ctxMenu.locator('.ctx-item[data-action="convDelete"]');
    await expect(deleteItem).toBeVisible();
  });

  test('TC-V14-MENU-002: Message block right-click shows context menu', async ({ page }) => {
    // Import a document and send a message to generate a response
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] });
    });
    await page.waitForTimeout(500);

    // Type and send a message
    await page.locator('#queryInput').fill('What is this document about?');
    await page.locator('#sendBtn').click();

    // Wait for response
    await page.waitForSelector('.msg-block.msg-assistant', { timeout: 10000 });

    // Right-click on the assistant message
    const msgBlock = page.locator('.msg-block.msg-assistant').first();
    await msgBlock.click({ button: 'right' });

    // Verify context menu is visible
    const ctxMenu = page.locator('#ctxMenu.visible');
    await expect(ctxMenu).toBeVisible();

    // Verify menu items for message
    const copyFullItem = ctxMenu.locator('.ctx-item[data-action="msgCopyFull"]');
    await expect(copyFullItem).toBeVisible();

    const copyPlainItem = ctxMenu.locator('.ctx-item[data-action="msgCopyPlain"]');
    await expect(copyPlainItem).toBeVisible();
  });

  test('TC-V14-MENU-003: Document list right-click shows delete option', async ({ page }) => {
    // Import a document
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] });
    });
    await page.waitForTimeout(500);

    // Open KB Modal to ensure document list is visible
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });

    // Wait for document to appear and be visible
    await page.locator('[data-doc-name]').first().waitFor({ state: 'visible', timeout: 5000 });

    // Right-click on a document item
    const docItem = page.locator('[data-doc-name]').first();
    await docItem.click({ button: 'right' });

    // Verify context menu is visible
    const ctxMenu = page.locator('#ctxMenu.visible');
    await expect(ctxMenu).toBeVisible();

    // Verify delete document menu item exists
    const deleteItem = ctxMenu.locator('.ctx-item[data-action="deleteDoc"]');
    await expect(deleteItem).toBeVisible();
  });

  test('TC-V14-MENU-004: Context menu boundary detection prevents overflow', async ({ page }) => {
    // Wait for conversation list to be visible
    await page.locator('[data-conv-id]').first().waitFor({ state: 'visible', timeout: 5000 });

    // Right-click on conversation item (use same method as TC-V14-MENU-001 for reliability)
    const convItem = page.locator('[data-conv-id]').first();
    await convItem.click({ button: 'right' });

    // Verify menu is visible
    const ctxMenu = page.locator('#ctxMenu.visible');
    await expect(ctxMenu).toBeVisible({ timeout: 5000 });

    // Verify menu is within viewport bounds
    const menuBox = await ctxMenu.boundingBox();
    const vp = page.viewportSize();
    if (menuBox && vp) {
      // Menu should not overflow right edge
      expect(menuBox.x + menuBox.width).toBeLessThanOrEqual(vp.width);
      // Menu should not overflow bottom edge
      expect(menuBox.y + menuBox.height).toBeLessThanOrEqual(vp.height);
    }
  });

  test('TC-V14-MENU-005: Conversation rename via context menu', async ({ page }) => {
    await page.waitForSelector('[data-conv-id]', { timeout: 5000 });

    const convItem = page.locator('[data-conv-id]').first();
    const convId = await convItem.getAttribute('data-conv-id');
    expect(convId).toBeTruthy();

    // Right-click and select rename
    await convItem.click({ button: 'right' });
    const ctxMenu = page.locator('#ctxMenu.visible');
    await ctxMenu.locator('.ctx-item[data-action="convRename"]').click();

    // Title span should become editable
    const titleSpan = convItem.locator('span.truncate');
    await expect(titleSpan).toHaveAttribute('contenteditable', 'true');

    // Type new name and press Enter
    await titleSpan.fill('Renamed Conversation');
    await page.keyboard.press('Enter');

    // Editable mode should be exited
    await expect(titleSpan).not.toHaveAttribute('contenteditable', 'true');
  });
});

test.describe('v1.14 Document List Keyboard Shortcuts (REQ-KB-004)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // Import a document for testing
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test1.md', '/mock/test2.md'] });
    });
    await page.waitForTimeout(500);
    // Open KB Modal to ensure document list is visible and interactive
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    // Wait for doc list items to be visible
    await page.locator('[data-doc-name]').first().waitFor({ state: 'visible', timeout: 5000 });
    // Re-initialize doc keyboard in case it was called before #docList existed
    await page.evaluate(() => {
      if (typeof window.__reinitDocKeyboard === 'function') {
        window.__reinitDocKeyboard();
      }
    });
  });

  test('TC-V14-KB-DOC-001: Arrow Down navigates to next document', async ({ page }) => {
    // Focus the document list area and dispatch keydown directly on #docList
    await page.evaluate(() => {
      const el = document.getElementById('kbDocScroll');
      if (el) { el.setAttribute('tabindex', '0'); el.focus(); }
      // Also dispatch keydown on #docList directly as fallback
      const docList = document.getElementById('docList');
      if (docList) docList.focus();
    });
    await page.waitForTimeout(300);
    // Dispatch ArrowDown keydown directly on docList
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    });
    await page.waitForTimeout(200);

    // First document should be selected
    const firstDoc = page.locator('[data-doc-id]').first();
    await expect(firstDoc).toHaveClass(/kb-keyboard-selected/);
  });

  test('TC-V14-KB-DOC-002: Arrow Up/Down navigates between documents', async ({ page }) => {
    // Focus the document list and dispatch events directly
    await page.evaluate(() => {
      const el = document.getElementById('kbDocScroll');
      if (el) { el.setAttribute('tabindex', '0'); el.focus(); }
      const docList = document.getElementById('docList');
      if (docList) docList.focus();
    });
    await page.waitForTimeout(300);

    // Navigate down twice via direct dispatch
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      }
    });
    await page.waitForTimeout(100);
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      }
    });
    await page.waitForTimeout(100);

    const secondDoc = page.locator('[data-doc-id]').nth(1);
    await expect(secondDoc).toHaveClass(/kb-keyboard-selected/);

    // Navigate back up
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
      }
    });
    await page.waitForTimeout(100);

    const firstDoc = page.locator('[data-doc-id]').first();
    await expect(firstDoc).toHaveClass(/kb-keyboard-selected/);
  });

  test('TC-V14-KB-DOC-003: Delete shows confirmation dialog', async ({ page }) => {
    // Focus the document list and select first item
    await page.evaluate(() => {
      const el = document.getElementById('kbDocScroll');
      if (el) { el.setAttribute('tabindex', '0'); el.focus(); }
      const docList = document.getElementById('docList');
      if (docList) docList.focus();
    });
    await page.waitForTimeout(300);
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      }
    });
    await page.waitForTimeout(100);

    // Press Delete
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
      }
    });

    // Confirmation dialog should appear
    await expect(page.locator('#confirmDialog')).toBeVisible({ timeout: 5000 });

    // Cancel deletion
    const cancelBtn = page.locator('#confirmCancel');
    if (await cancelBtn.isVisible()) {
      await cancelBtn.click();
    }
  });

  test('TC-V14-KB-DOC-004: Escape clears selection', async ({ page }) => {
    // Focus the document list and select first item
    await page.evaluate(() => {
      const el = document.getElementById('kbDocScroll');
      if (el) { el.setAttribute('tabindex', '0'); el.focus(); }
      const docList = document.getElementById('docList');
      if (docList) docList.focus();
    });
    await page.waitForTimeout(300);
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      }
    });
    await page.waitForTimeout(100);

    const firstDoc = page.locator('[data-doc-id]').first();
    await expect(firstDoc).toHaveClass(/kb-keyboard-selected/);

    // Press Escape
    await page.evaluate(() => {
      const docList = document.getElementById('docList');
      if (docList) {
        docList.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
      }
    });

    // Selection should be cleared
    await expect(firstDoc).not.toHaveClass(/kb-keyboard-selected/);
  });
});
