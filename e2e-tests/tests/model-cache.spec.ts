// E2E 模型缓存管理 UI（REQ-VEC-008）。
// E2E-VEC-001: 设置面板显示模型缓存大小
// E2E-VEC-002: 下载模型按钮触发 init_embedder
// E2E-VEC-003: 模型下载进度事件驱动 toast
// E2E-VEC-004: 下载完成后 toast「模型就绪」
// E2E-VEC-005: 清理缓存按钮触发 clear_model_cache
// E2E-VEC-006: 清理后缓存大小归零
// E2E-VEC-007: 空缓存状态展示
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl, showAllSettingsSections } from './helpers.mjs';
test.describe('E2E-VEC-001~007 模型缓存管理 UI', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await showAllSettingsSections(page);
  });

  test('E2E-VEC-001 设置面板显示模型缓存大小', async ({ page }) => {
    // 缓存信息区应显示总大小（30MB）
    const cacheInfo = page.locator('#settingsCacheInfo');
    await expect(cacheInfo).toBeVisible();
    const text = await cacheInfo.innerText();
    expect(text, '应显示模型大小 MB').toMatch(/\d+(\.\d+)?\s*MB/);
    expect(text, '应显示模型名称').toContain('all-MiniLM-L6-v2');
  });

  test('E2E-VEC-002 下载模型按钮触发 init_embedder', async ({ page }) => {
    // 点击下载模型
    await page.locator('#settingsInitEmbedder').click();

    // 应调用 init_embedder（mock 发射 model_download_progress 事件）
    // toast 应出现下载进度
    await expect(page.locator('#toasts')).toContainText('下载', { timeout: 5000 });
  });

  test('E2E-VEC-003 模型下载进度事件驱动 toast', async ({ page }) => {
    await page.locator('#settingsInitEmbedder').click();
    // 应出现下载相关 toast（可能含 % 或 下载关键词）
    await expect(page.locator('#toasts')).toContainText(/%|下载|模型/, { timeout: 5000 });
  });

  test('E2E-VEC-004 下载完成后 toast 模型就绪', async ({ page }) => {
    await page.locator('#settingsInitEmbedder').click();
    // 等待下载完成事件（done: true → toastSuccess「模型就绪」）
    await expect(page.locator('#toasts')).toContainText('模型下载完成', { timeout: 8000 });
  });

  test('E2E-VEC-005 清理缓存按钮触发 clear_model_cache', async ({ page }) => {
    // 点击清理缓存
    await page.locator('#settingsClearCache').click();

    // toast 应显示「已清理」+ 大小
    await expect(page.locator('#toasts')).toContainText('已清理', { timeout: 5000 });
    const toastText = await page.locator('#toasts').innerText();
    expect(toastText, '应含清理大小').toMatch(/\d+(\.\d+)?\s*(MB|KB|B)/);
  });

  test('E2E-VEC-006 清理后缓存大小归零', async ({ page }) => {
    await page.locator('#settingsClearCache').click();
    await page.waitForTimeout(1000);

    // 重新打开设置面板刷新缓存信息
    await page.locator('#settingsClose').click();
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await showAllSettingsSections(page);

    // 缓存信息应显示「未安装模型」
    const cacheInfo = page.locator('#settingsCacheInfo');
    await expect(cacheInfo).toContainText('未安装模型', { timeout: 5000 });
  });

  test('E2E-VEC-007 空缓存状态展示', async ({ page }) => {
    // 预置空缓存
    await page.evaluate(() => {
      window.__state.modelCacheInfo = { models: [], total_size_bytes: 0 };
    });
    // 关闭后重新打开设置面板触发刷新
    await page.locator('#settingsClose').click();
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await showAllSettingsSections(page);

    await expect(page.locator('#settingsCacheInfo')).toContainText('未安装模型', { timeout: 5000 });
  });
});
