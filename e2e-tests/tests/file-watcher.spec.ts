// E2E 文件监听 + 增量同步（REQ-SYNC-001~003）：
// E2E-SYNC-001: 添加监听文件夹
// E2E-SYNC-002: 查询已监听文件夹列表
// E2E-SYNC-003: 移除监听文件夹
// E2E-SYNC-004: 重复添加同一文件夹——不重复
// E2E-SYNC-005: 移除不存在的文件夹——无错误
// E2E-SYNC-006: 添加多个文件夹
// E2E-SYNC-007: 逐个移除文件夹
// E2E-SYNC-008: 文件夹路径验证
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-SYNC 文件监听 + 增量同步（REQ-SYNC-001~003）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  // ─── 基本文件监听操作 ───

  test('E2E-SYNC-001 添加监听文件夹', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('add_watched_folder', { path: '/mock/watched/docs' })
    );
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).toContain('/mock/watched/docs');
  });

  test('E2E-SYNC-002 查询已监听文件夹列表', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('add_watched_folder', { path: '/mock/folder1' })
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('add_watched_folder', { path: '/mock/folder2' })
    );
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).toHaveLength(2);
    expect(folders).toContain('/mock/folder1');
    expect(folders).toContain('/mock/folder2');
  });

  test('E2E-SYNC-003 移除监听文件夹', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('add_watched_folder', { path: '/mock/toRemove' })
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('remove_watched_folder', { path: '/mock/toRemove' })
    );
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).not.toContain('/mock/toRemove');
  });

  // ─── 边界情况 ───

  test('E2E-SYNC-004 重复添加同一文件夹——不重复', async ({ page }) => {
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('add_watched_folder', { path: '/mock/duplicate' })
    );
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('add_watched_folder', { path: '/mock/duplicate' })
    );
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    const count = folders.filter((f) => f === '/mock/duplicate').length;
    expect(count).toBe(1);
  });

  test('E2E-SYNC-005 移除不存在的文件夹——无错误', async ({ page }) => {
    // 不应抛出异常
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('remove_watched_folder', { path: '/mock/nonexistent' })
    );
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).toHaveLength(0);
  });

  test('E2E-SYNC-006 添加多个文件夹', async ({ page }) => {
    for (let i = 0; i < 5; i++) {
      await page.evaluate((p) =>
        window.__TAURI__.core.invoke('add_watched_folder', { path: p })
      , `/mock/folder${i}`);
    }
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).toHaveLength(5);
  });

  test('E2E-SYNC-007 逐个移除文件夹', async ({ page }) => {
    for (let i = 0; i < 3; i++) {
      await page.evaluate((p) =>
        window.__TAURI__.core.invoke('add_watched_folder', { path: p })
      , `/mock/f${i}`);
    }
    // 移除中间一个
    await page.evaluate(() =>
      window.__TAURI__.core.invoke('remove_watched_folder', { path: '/mock/f1' })
    );
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).toHaveLength(2);
    expect(folders).toContain('/mock/f0');
    expect(folders).toContain('/mock/f2');
    expect(folders).not.toContain('/mock/f1');
  });

  test('E2E-SYNC-008 文件夹路径验证——空列表初始状态', async ({ page }) => {
    const folders = await page.evaluate(() =>
      window.__TAURI__.core.invoke('get_watched_folders')
    );
    expect(folders).toHaveLength(0);
    expect(Array.isArray(folders)).toBe(true);
  });
});
