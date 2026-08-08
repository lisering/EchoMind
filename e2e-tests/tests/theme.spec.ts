/**
 * 浅色主题切换 E2E 测试（REQ-UI-011）
 *
 * TC-UI-LIGHT-001: 切换到浅色主题 — data-theme="light"
 * TC-UI-LIGHT-002: 浅色主题颜色生效 — backgroundColor 为浅色
 * TC-UI-LIGHT-003: 主题持久化 — 刷新后保持
 * TC-UI-LIGHT-004: 跟随系统模式 — data-theme="system" + prefers-color-scheme: light
 * TC-UI-LIGHT-005: 浅色主题对比度 — axe-core color-contrast 零 violation
 */
import { test, expect } from '@playwright/test';
import { AxeBuilder } from '@axe-core/playwright';
import { setupPage } from './helpers.mjs';

test.describe('浅色主题切换 (REQ-UI-011)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 等待应用初始化完成
    await page.waitForSelector('#sendBtn', { state: 'visible', timeout: 10000 });
  });

  test('TC-UI-LIGHT-001 切换到浅色主题后 data-theme 为 light', async ({ page }) => {
    // 打开设置面板
    await page.click('#settingsBtn');
    await page.waitForSelector('#settingsModal', { state: 'visible' });

    // 等待主题切换器出现
    await page.waitForSelector('#themeSwitcher', { state: 'visible' });

    // 点击「浅色」主题按钮
    await page.click('[data-theme-value="light"]');

    // 验证 data-theme 属性
    const theme = await page.evaluate(() => document.documentElement.dataset.theme);
    expect(theme).toBe('light');
  });

  test('TC-UI-LIGHT-002 浅色主题下 backgroundColor 为浅色', async ({ page }) => {
    // 直接通过 JS 设置浅色主题（绕过 UI 交互，专注于 CSS 变量验证）
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'light';
    });

    // 等待 CSS 变量生效（浏览器重绘）
    await page.waitForTimeout(100);

    // 获取 body 的背景色
    const bgColor = await page.evaluate(() => {
      return getComputedStyle(document.body).backgroundColor;
    });

    // 浅色主题 --surface-0: #FFFFFF → rgb(255, 255, 255)
    // 或 --surface-1: #F8FAFC → rgb(248, 250, 252)
    // 验证 RGB 值接近白色（每个分量 > 230）
    const match = bgColor.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
    expect(match).toBeTruthy();
    if (match) {
      const r = parseInt(match[1], 10);
      const g = parseInt(match[2], 10);
      const b = parseInt(match[3], 10);
      // 浅色主题背景应为白色或接近白色
      expect(r).toBeGreaterThan(230);
      expect(g).toBeGreaterThan(230);
      expect(b).toBeGreaterThan(230);
    }
  });

  test('TC-UI-LIGHT-003 主题选择持久化（刷新后保持）', async ({ page }) => {
    // 打开设置面板
    await page.click('#settingsBtn');
    await page.waitForSelector('#settingsModal', { state: 'visible' });

    // 点击「浅色」主题按钮
    await page.waitForSelector('#themeSwitcher', { state: 'visible' });
    await page.click('[data-theme-value="light"]');

    // 验证当前主题
    const themeBefore = await page.evaluate(() => document.documentElement.dataset.theme);
    expect(themeBefore).toBe('light');

    // 关闭设置面板
    await page.click('#settingsClose');

    // 刷新页面
    await page.reload();

    // 等待应用重新初始化
    await page.waitForSelector('#sendBtn', { state: 'visible', timeout: 10000 });

    // 等待 initTheme 执行（从 localStorage 读取）
    await page.waitForFunction(() => {
      const t = document.documentElement.dataset.theme;
      return t && t.length > 0;
    }, { timeout: 5000 });

    // 验证刷新后主题仍为 light
    const themeAfter = await page.evaluate(() => document.documentElement.dataset.theme);
    expect(themeAfter).toBe('light');
  });

  test('TC-UI-LIGHT-004 跟随系统模式自动切换', async ({ page, browser }) => {
    // 使用浏览器上下文模拟 prefers-color-scheme: light
    // 先设置 system 主题
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'system';
    });

    // 验证 data-theme 为 system
    const theme = await page.evaluate(() => document.documentElement.dataset.theme);
    expect(theme).toBe('system');

    // 验证 CSS 变量在系统浅色模式下生效
    // 创建新上下文模拟 prefers-color-scheme: light
    const lightContext = await browser.newContext({
      colorScheme: 'light',
    });
    const lightPage = await lightContext.newPage();
    await setupPage(lightPage);
    await lightPage.waitForSelector('#sendBtn', { state: 'visible', timeout: 10000 });

    // 设置 system 主题
    await lightPage.evaluate(() => {
      document.documentElement.dataset.theme = 'system';
    });

    // 等待 CSS 变量生效
    await lightPage.waitForTimeout(100);

    // 验证在 prefers-color-scheme: light 下背景色为浅色
    const bgColor = await lightPage.evaluate(() => {
      return getComputedStyle(document.body).backgroundColor;
    });

    const match = bgColor.match(/rgb\((\d+),\s*(\d+),\s*(\d+)\)/);
    expect(match).toBeTruthy();
    if (match) {
      const r = parseInt(match[1], 10);
      const g = parseInt(match[2], 10);
      const b = parseInt(match[3], 10);
      expect(r).toBeGreaterThan(230);
      expect(g).toBeGreaterThan(230);
      expect(b).toBeGreaterThan(230);
    }

    await lightContext.close();
  });

  test('TC-UI-LIGHT-005 浅色主题对比度达 WCAG AA 4.5:1', async ({ page }) => {
    // 设置浅色主题
    await page.evaluate(() => {
      document.documentElement.dataset.theme = 'light';
    });

    // 等待 CSS 变量完全生效
    await page.waitForTimeout(300);

    // 使用 axe-core 扫描颜色对比度（仅扫描主界面，不含设置面板）
    const results = await new AxeBuilder({ page })
      .withTags(['wcag2aa'])
      .withRules(['color-contrast'])
      .include('#app')
      .exclude('#settingsModal')
      .exclude('#wizard')
      .analyze();

    // 零 color-contrast violation
    const contrastViolations = results.violations.filter(v => v.id === 'color-contrast');
    expect(contrastViolations).toHaveLength(0);
  });
});
