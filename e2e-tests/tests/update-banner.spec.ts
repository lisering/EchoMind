/**
 * REQ-HELP-004 S87 自动更新检查 E2E 测试。
 *
 * TC-UPD-001: 启动后 5s 异步检查更新（无新版本 → 不显示横幅）
 * TC-UPD-002: 发现新版本时顶部显示更新横幅
 * TC-UPD-003: 横幅含版本号 + 更新日志按钮 + 下载按钮
 * TC-UPD-004: 关闭按钮隐藏横幅
 * TC-UPD-005: 网络不可用时静默跳过（不报错）
 */
import { test, expect } from '@playwright/test';
import { setupPage, uiUrl, injectStub, injectLocales } from './helpers.mjs';

test.describe('REQ-HELP-004 S87 自动更新检查', () => {

  test('TC-UPD-001: 无新版本时不显示横幅', async ({ page }) => {
    await setupPage(page);
    // 等待 6s（启动后 5s 延迟 + 1s 缓冲）
    await page.waitForTimeout(6000);
    // 无新版本时不应有更新横幅
    const banner = page.locator('#updateBanner');
    await expect(banner).toHaveCount(0);
  });

  test('TC-UPD-002: 发现新版本时显示更新横幅', async ({ page }) => {
    // 注入 mock：有新版本
    await injectStub(page);
    await page.addInitScript(() => {
      window.__state.updateInfo = {
        has_update: true,
        current_version: '2.2.0',
        latest_version: '2.3.0',
        release_notes: 'Bug fixes and performance improvements',
        download_url: 'https://github.com/EchoMind/EchoMind/releases/tag/v2.3.0',
      };
    });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    // 等待 5s 更新检查延迟 + 横幅动画
    await page.locator('#updateBanner').waitFor({ state: 'visible', timeout: 10000 });
    await expect(page.locator('#updateBanner')).toBeVisible();
  });

  test('TC-UPD-003: 横幅含版本号 + 更新日志按钮 + 下载按钮', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => {
      window.__state.updateInfo = {
        has_update: true,
        current_version: '2.2.0',
        latest_version: '2.3.0',
        release_notes: 'New features and bug fixes',
        download_url: 'https://github.com/EchoMind/EchoMind/releases/tag/v2.3.0',
      };
    });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#updateBanner').waitFor({ state: 'visible', timeout: 10000 });

    // 验证版本号
    const versionEl = page.locator('.update-banner-version');
    await expect(versionEl).toBeVisible();
    await expect(versionEl).toContainText('2.3.0');

    // 验证更新日志按钮
    const notesBtn = page.locator('.update-banner-link');
    await expect(notesBtn).toBeVisible();

    // 验证下载按钮
    const downloadBtn = page.locator('.update-banner-download');
    await expect(downloadBtn).toBeVisible();

    // 验证关闭按钮
    const closeBtn = page.locator('.update-banner-close');
    await expect(closeBtn).toBeVisible();
  });

  test('TC-UPD-004: 关闭按钮隐藏横幅', async ({ page }) => {
    await injectStub(page);
    await page.addInitScript(() => {
      window.__state.updateInfo = {
        has_update: true,
        current_version: '2.2.0',
        latest_version: '2.3.0',
        release_notes: 'Bug fixes',
        download_url: 'https://github.com/EchoMind/EchoMind/releases/tag/v2.3.0',
      };
    });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    await page.locator('#updateBanner').waitFor({ state: 'visible', timeout: 10000 });

    // 点击关闭按钮
    await page.locator('.update-banner-close').click();
    // 横幅应消失
    await expect(page.locator('#updateBanner')).toHaveCount(0);
  });

  test('TC-UPD-005: 网络不可用时静默跳过', async ({ page }) => {
    await injectStub(page);
    // 注入一个让 check_for_updates 抛异常的 mock
    await page.addInitScript(() => {
      const origInvoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = async function(cmd: string, ...args: any[]) {
        if (cmd === 'check_for_updates') {
          throw new Error('Network unreachable');
        }
        return origInvoke(cmd, ...args);
      };
    });
    await injectLocales(page);
    await page.goto(uiUrl);
    await page.locator('#app').waitFor({ state: 'visible', timeout: 15000 });
    // 等待 6s（5s 延迟 + 1s 缓冲）
    await page.waitForTimeout(6000);
    // 不应有横幅（静默跳过）
    const banner = page.locator('#updateBanner');
    await expect(banner).toHaveCount(0);
    // 也不应有错误提示
    const toasts = page.locator('[role="alert"]');
    const errorToasts = page.locator('.toast-error');
    await expect(errorToasts).toHaveCount(0);
  });

});
