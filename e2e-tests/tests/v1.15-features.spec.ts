import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('v1.15 Keyboard Help Search (REQ-KB-005)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V15-001: Cmd+/ opens keyboard help panel with search input', async ({ page }) => {
    // Press Cmd+/ (or Ctrl+/ on non-Mac)
    const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${mod}+/`);

    // Panel should be visible
    const panel = page.locator('#keyboardHelpPanel');
    await expect(panel).not.toHaveClass(/hidden/);

    // Search input should exist and be focused
    const searchInput = page.locator('#keyboardHelpSearch');
    await expect(searchInput).toBeVisible();
    await expect(searchInput).toBeFocused();

    // Content should have shortcut groups
    const content = page.locator('#keyboardHelpContent');
    await expect(content).not.toBeEmpty();
  });

  test('TC-V15-002: Search filter narrows shortcut results', async ({ page }) => {
    const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${mod}+/`);

    const searchInput = page.locator('#keyboardHelpSearch');
    await expect(searchInput).toBeVisible();

    // Get initial row count
    const content = page.locator('#keyboardHelpContent');
    const initialRows = await content.locator('kbd').count();
    expect(initialRows).toBeGreaterThan(2);

    // Type a search query that matches only a few shortcuts (use key name for locale-independence)
    await searchInput.fill('enter');
    await page.waitForTimeout(300); // Wait for debounce

    // Should have fewer results
    const filteredRows = await content.locator('kbd').count();
    expect(filteredRows).toBeLessThan(initialRows);
    expect(filteredRows).toBeGreaterThan(0);
  });

  test('TC-V15-003: No results message when search has no matches', async ({ page }) => {
    const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${mod}+/`);

    const searchInput = page.locator('#keyboardHelpSearch');
    await searchInput.fill('zzznonexistent12345');
    await page.waitForTimeout(300);

    const content = page.locator('#keyboardHelpContent');
    await expect(content).toContainText(/No matching|未找到/);
  });

  test('TC-V15-004: Esc closes keyboard help panel', async ({ page }) => {
    const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${mod}+/`);

    const panel = page.locator('#keyboardHelpPanel');
    await expect(panel).not.toHaveClass(/hidden/);

    await page.keyboard.press('Escape');
    await expect(panel).toHaveClass(/hidden/);
  });

  test('TC-V15-005: Edit group (cut/copy/paste/select all) is shown', async ({ page }) => {
    const mod = process.platform === 'darwin' ? 'Meta' : 'Control';
    await page.keyboard.press(`${mod}+/`);

    const content = page.locator('#keyboardHelpContent');
    // The edit group should be visible with cut/copy/paste items
    await expect(content).toContainText(/Cut|剪切/);
    await expect(content).toContainText(/Copy|复制/);
    await expect(content).toContainText(/Paste|粘贴/);
    await expect(content).toContainText(/Select all|全选/);
  });
});

test.describe('v1.15 Design Tokens (REQ-DS-006 / REQ-IX-004 / REQ-DS-007)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V15-006: REQ-DS-006 transition tokens are defined', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        durationMicro: cs.getPropertyValue('--duration-micro').trim(),
        durationFast: cs.getPropertyValue('--duration-fast').trim(),
        durationNormal: cs.getPropertyValue('--duration-normal').trim(),
        durationSlow: cs.getPropertyValue('--duration-slow').trim(),
        easeInOut: cs.getPropertyValue('--ease-in-out').trim(),
        easeSpring: cs.getPropertyValue('--ease-spring').trim(),
      };
    });
    expect(tokens.durationMicro).toBe('100ms');
    expect(tokens.durationFast).toBe('150ms');
    expect(tokens.durationNormal).toBe('250ms');
    expect(tokens.durationSlow).toBe('400ms');
    expect(tokens.easeInOut).toContain('cubic-bezier');
    expect(tokens.easeSpring).toContain('cubic-bezier');
  });

  test('TC-V15-007: REQ-DS-006 animation utility classes exist', async ({ page }) => {
    const hasModalAnim = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'animate-modal-in';
      document.body.appendChild(el);
      const cs = getComputedStyle(el);
      const anim = cs.animationName;
      el.remove();
      return anim;
    });
    expect(hasModalAnim).toBe('modalIn');

    const hasToastAnim = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'animate-toast-in';
      document.body.appendChild(el);
      const cs = getComputedStyle(el);
      const anim = cs.animationName;
      el.remove();
      return anim;
    });
    expect(hasToastAnim).toBe('toastIn');
  });

  test('TC-V15-008: REQ-IX-004 hover tokens are defined', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        hoverBg: cs.getPropertyValue('--hover-bg').trim(),
        hoverBgSubtle: cs.getPropertyValue('--hover-bg-subtle').trim(),
        hoverColor: cs.getPropertyValue('--hover-color').trim(),
        hoverTransition: cs.getPropertyValue('--hover-transition').trim(),
        hoverOpacity: cs.getPropertyValue('--hover-opacity').trim(),
      };
    });
    expect(tokens.hoverBg).not.toBe('');
    expect(tokens.hoverBgSubtle).toContain('rgba');
    expect(tokens.hoverColor).not.toBe('');
    expect(tokens.hoverTransition).toContain('150ms');
    expect(tokens.hoverOpacity).toBe('0.9');
  });

  test('TC-V15-009: REQ-DS-007 radius tokens are defined', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        sm: cs.getPropertyValue('--radius-sm').trim(),
        md: cs.getPropertyValue('--radius-md').trim(),
        lg: cs.getPropertyValue('--radius-lg').trim(),
        xl: cs.getPropertyValue('--radius-xl').trim(),
        full: cs.getPropertyValue('--radius-full').trim(),
      };
    });
    expect(tokens.sm).toBe('4px');
    expect(tokens.md).toBe('8px');
    expect(tokens.lg).toBe('12px');
    expect(tokens.xl).toBe('16px');
    expect(tokens.full).toBe('9999px');
  });

  test('TC-V15-010: REQ-DS-007 shadow tokens are defined', async ({ page }) => {
    const tokens = await page.evaluate(() => {
      const cs = getComputedStyle(document.documentElement);
      return {
        sm: cs.getPropertyValue('--shadow-sm').trim(),
        md: cs.getPropertyValue('--shadow-md').trim(),
        lg: cs.getPropertyValue('--shadow-lg').trim(),
        focus: cs.getPropertyValue('--shadow-focus').trim(),
      };
    });
    expect(tokens.sm).toContain('0 4px 12px');
    expect(tokens.md).toContain('0 8px 24px');
    expect(tokens.lg).toContain('0 16px 48px');
    expect(tokens.focus).toContain('0 0 0');
  });

  test('TC-V15-011: REQ-DS-006 prefers-reduced-motion disables new animations', async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });

    const animDuration = await page.evaluate(() => {
      const el = document.createElement('div');
      el.className = 'animate-modal-in';
      document.body.appendChild(el);
      const cs = getComputedStyle(el);
      const dur = cs.animationDuration;
      el.remove();
      return dur;
    });
    // With prefers-reduced-motion: reduce, animation should be near-zero (0.01ms or 1e-05s)
    expect(parseFloat(animDuration)).toBeLessThan(0.1);
  });
});
