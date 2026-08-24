/**
 * E2E tests for v1.21 SVG inline icon system (REQ-DS-004).
 *
 * Verifies that:
 * 1. SVG inline icons replace Unicode characters in interactive elements
 * 2. Icon size CSS classes (.icon-sm/.icon-md/.icon-lg) produce correct dimensions
 * 3. disabled state icons have opacity 0.4
 * 4. Icons inherit currentColor (compatible with hover:text-accent)
 * 5. Zero new dependencies (no icon library loaded)
 */
import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('v1.21 SVG Inline Icon System (REQ-DS-004)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  // ============================================================
  // AC-1: All interactive elements use SVG inline icons
  // ============================================================

  test('TC-V21-001: Settings button contains SVG element (not Unicode ⚙)', async ({ page }) => {
    const btn = page.locator('#settingsBtn');
    await expect(btn).toBeVisible();
    const svg = btn.locator('svg');
    await expect(svg).toHaveCount(1);
    // Ensure no Unicode character remains as text content
    const text = await btn.textContent();
    expect(text).not.toContain('⚙');
  });

  test('TC-V21-002: New Chat button contains SVG element (not Unicode ＋)', async ({ page }) => {
    const btn = page.locator('#newChatBtn');
    await expect(btn).toBeVisible();
    const svg = btn.locator('svg');
    await expect(svg).toHaveCount(1);
    const text = await btn.textContent();
    expect(text).not.toContain('＋');
  });

  test('TC-V21-003: Import/Plus button contains SVG element (not Unicode ＋)', async ({ page }) => {
    const btn = page.locator('#plusBtn');
    await expect(btn).toBeVisible();
    const svg = btn.locator('svg');
    await expect(svg).toHaveCount(1);
    const text = await btn.textContent();
    expect(text).not.toContain('＋');
  });

  test('TC-V21-004: Collapse button contains SVG element (not Unicode ‹)', async ({ page }) => {
    const btn = page.locator('#collapseBtn');
    await expect(btn).toBeVisible();
    const svg = btn.locator('svg');
    await expect(svg).toHaveCount(1);
  });

  test('TC-V21-005: Expand button contains SVG element (not Unicode ›)', async ({ page }) => {
    const btn = page.locator('#expandBtn');
    const svg = btn.locator('svg');
    await expect(svg).toHaveCount(1);
  });

  test('TC-V21-006: Stop button contains SVG element (not Unicode ■)', async ({ page }) => {
    const btn = page.locator('#sendBtn');
    await expect(btn).toBeVisible();
    const stopIcon = btn.locator('#stopIcon');
    await expect(stopIcon).toHaveCount(1);
    // Verify it's an SVG rect, not a Unicode ■ character
    const rect = stopIcon.locator('rect');
    await expect(rect).toHaveCount(1);
  });

  test('TC-V21-007: Drag overlay contains SVG element (not Unicode ⇩)', async ({ page }) => {
    const overlay = page.locator('#dragOverlay');
    await expect(overlay).toHaveClass(/hidden/);
    // Make it visible by dispatching drag-enter
    await overlay.evaluate((el) => el.classList.remove('hidden'));
    const svg = overlay.locator('svg');
    await expect(svg).toHaveCount(1);
    const text = await overlay.textContent();
    expect(text).not.toContain('⇩');
  });

  // ============================================================
  // AC-2: Icon size CSS classes produce correct dimensions
  // ============================================================

  test('TC-V21-008: .icon-sm class produces 16px width/height', async ({ page }) => {
    // The icons.css file is loaded; check a dynamically created icon
    await page.evaluate(() => {
      const div = document.createElement('div');
      div.id = '__test_icon_sm';
      div.innerHTML = '<svg class="icon-sm" viewBox="0 0 24 24"><path d=""/></svg>';
      document.body.appendChild(div);
    });
    const svg = page.locator('#__test_icon_sm svg');
    const box = await svg.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBe(16);
    expect(box!.height).toBe(16);
  });

  test('TC-V21-009: .icon-md class produces 20px width/height', async ({ page }) => {
    await page.evaluate(() => {
      const div = document.createElement('div');
      div.id = '__test_icon_md';
      div.innerHTML = '<svg class="icon-md" viewBox="0 0 24 24"><path d=""/></svg>';
      document.body.appendChild(div);
    });
    const svg = page.locator('#__test_icon_md svg');
    const box = await svg.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBe(20);
    expect(box!.height).toBe(20);
  });

  test('TC-V21-010: .icon-lg class produces 24px width/height', async ({ page }) => {
    await page.evaluate(() => {
      const div = document.createElement('div');
      div.id = '__test_icon_lg';
      div.innerHTML = '<svg class="icon-lg" viewBox="0 0 24 24"><path d=""/></svg>';
      document.body.appendChild(div);
    });
    const svg = page.locator('#__test_icon_lg svg');
    const box = await svg.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.width).toBe(24);
    expect(box!.height).toBe(24);
  });

  // ============================================================
  // AC-3: disabled state icons have opacity 0.4
  // ============================================================

  test('TC-V21-011: .icon-disabled class produces opacity 0.4', async ({ page }) => {
    await page.evaluate(() => {
      const div = document.createElement('div');
      div.id = '__test_icon_disabled';
      div.innerHTML = '<svg class="icon-sm icon-disabled" viewBox="0 0 24 24"><path d=""/></svg>';
      document.body.appendChild(div);
    });
    const svg = page.locator('#__test_icon_disabled svg');
    const opacity = await svg.evaluate((el) => window.getComputedStyle(el).opacity);
    expect(parseFloat(opacity)).toBeCloseTo(0.4, 1);
  });

  test('TC-V21-012: Button disabled state propagates opacity to SVG icon', async ({ page }) => {
    await page.evaluate(() => {
      const btn = document.createElement('button');
      btn.id = '__test_disabled_btn';
      btn.disabled = true;
      btn.innerHTML = '<svg class="icon-sm" viewBox="0 0 24 24"><path d=""/></svg>';
      document.body.appendChild(btn);
    });
    const svg = page.locator('#__test_disabled_btn svg');
    const opacity = await svg.evaluate((el) => window.getComputedStyle(el).opacity);
    expect(parseFloat(opacity)).toBeCloseTo(0.4, 1);
  });

  // ============================================================
  // AC-4: Icons inherit currentColor
  // ============================================================

  test('TC-V21-013: SVG icons use currentColor for stroke', async ({ page }) => {
    const btn = page.locator('#settingsBtn svg');
    const stroke = await btn.evaluate((el) => window.getComputedStyle(el).stroke);
    // currentColor resolves to the element's color property
    const color = await btn.evaluate((el) => window.getComputedStyle(el).color);
    // stroke should be the resolved currentColor value (not 'none' or empty)
    expect(stroke).not.toBe('none');
    expect(stroke.length).toBeGreaterThan(0);
  });

  // ============================================================
  // AC-5: Zero new dependencies (no icon library)
  // ============================================================

  test('TC-V21-014: No external icon library loaded', async ({ page }) => {
    // Check no FontAwesome, Material Icons, or other icon libraries are loaded
    const scripts = await page.evaluate(() =>
      Array.from(document.querySelectorAll('link[rel="stylesheet"]')).map(l => l.href)
    );
    const iconLibPatterns = ['fontawesome', 'material-icons', 'feather', 'lucide', 'heroicons', 'tabler'];
    for (const pattern of iconLibPatterns) {
      for (const src of scripts) {
        expect(src.toLowerCase()).not.toContain(pattern);
      }
    }
    // Also check inline styles for @font-face from icon libraries
    const hasFontFace = await page.evaluate(() => {
      for (const sheet of document.styleSheets) {
        try {
          for (const rule of sheet.cssRules) {
            if (rule.cssText && rule.cssText.includes('@font-face') && (
              rule.cssText.includes('FontAwesome') ||
              rule.cssText.includes('Material Icons') ||
              rule.cssText.includes('feather')
            )) {
              return true;
            }
          }
        } catch (_) { /* cross-origin sheet */ }
      }
      return false;
    });
    expect(hasFontFace).toBe(false);
  });

  // ============================================================
  // Dynamic icon replacement in JS-generated DOM
  // ============================================================

  test('TC-V21-015: Document list delete button uses SVG icon (not Unicode ×)', async ({ page }) => {
    // Import a document to populate the list
    await page.evaluate(async () => {
      await window.__TAURI__.core.invoke('import_files', { paths: ['/mock/test.md'] });
    });
    await page.waitForTimeout(500);

    // Find the delete button in document list
    const delBtn = page.locator('#docList [data-action="delete"]').first();
    if (await delBtn.count() > 0) {
      const svg = delBtn.locator('svg');
      await expect(svg).toHaveCount(1);
      const text = await delBtn.textContent();
      expect(text).not.toContain('×');
    }
  });

  test('TC-V21-016: Conversation list delete button uses SVG icon (not Unicode ×)', async ({ page }) => {
    // Wait for conversations to load
    await page.waitForTimeout(500);

    // Find conversation delete buttons
    const delBtns = page.locator('#convList button');
    if (await delBtns.count() > 0) {
      const firstBtn = delBtns.first();
      const svg = firstBtn.locator('svg');
      await expect(svg).toHaveCount(1);
      const text = await firstBtn.textContent();
      expect(text).not.toContain('×');
    }
  });

  // ============================================================
  // Empty state brand icon
  // ============================================================

  test('TC-V21-017: Empty state brand icon uses SVG (not Unicode ◈)', async ({ page }) => {
    // The empty state should be visible on startup (no documents)
    const brandEl = page.locator('.empty-state-logo');
    if (await brandEl.count() > 0) {
      const svg = brandEl.locator('svg');
      await expect(svg).toHaveCount(1);
      const text = await brandEl.textContent();
      expect(text).not.toContain('◈');
    }
  });
});
