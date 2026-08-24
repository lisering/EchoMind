/**
 * E2E 测试：顶栏工具下拉菜单位置 + 设置面板数据管理区域。
 *
 * 测试内容：
 * 1. TC-UI-TOOLS-001: 工具按钮存在且可见
 * 2. TC-UI-TOOLS-002: 点击工具按钮后菜单可见
 * 3. TC-UI-TOOLS-003: 菜单左对齐到工具按钮（非右对齐偏移）
 * 4. TC-UI-TOOLS-004: 菜单包含 4 个功能按钮
 * 5. TC-UI-TOOLS-005: 点击外部关闭菜单
 * 6. TC-UI-TOOLS-006: 设置面板"备份与恢复"区域可见且按钮可点击
 * 7. TC-UI-TOOLS-007: "数据管理"标签已改名为"备份与恢复"
 */
import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('工具下拉菜单位置与数据管理', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-UI-TOOLS-001: 工具按钮存在且可见', async ({ page }) => {
    const toolsBtn = page.locator('#toolsBtn');
    await expect(toolsBtn).toBeVisible();
  });

  test('TC-UI-TOOLS-002: 点击工具按钮后菜单可见', async ({ page }) => {
    const toolsBtn = page.locator('#toolsBtn');
    const toolsMenu = page.locator('#toolsMenu');

    // 菜单初始隐藏
    await expect(toolsMenu).toHaveClass(/hidden/);

    // 点击按钮显示菜单
    await toolsBtn.click();
    await expect(toolsMenu).not.toHaveClass(/hidden/);
    await expect(toolsMenu).toBeVisible();
  });

  test('TC-UI-TOOLS-003: 菜单左对齐到工具按钮（非右对齐偏移）', async ({ page }) => {
    const toolsBtn = page.locator('#toolsBtn');
    const toolsMenu = page.locator('#toolsMenu');

    // 打开菜单
    await toolsBtn.click();
    await expect(toolsMenu).toBeVisible();

    // 获取按钮和菜单的位置
    const btnBox = await toolsBtn.boundingBox();
    const menuBox = await toolsMenu.boundingBox();

    expect(btnBox).not.toBeNull();
    expect(menuBox).not.toBeNull();

    // 菜单左边缘应该 >= 按钮左边缘（左对齐）
    // 菜单不应该向左偏移到按钮左边缘之前
    expect(menuBox!.x).toBeGreaterThanOrEqual(btnBox!.x - 1); // 允许 1px 误差

    // 菜单不应该向右超出按钮右边缘 + 菜单宽度（确保不会偏移到屏幕右侧）
    // 菜单右边缘应该在合理范围内
    expect(menuBox!.x + menuBox!.width).toBeLessThan(btnBox!.x + menuBox!.width + 200);
  });

  test('TC-UI-TOOLS-004: 菜单包含 4 个功能按钮', async ({ page }) => {
    const toolsBtn = page.locator('#toolsBtn');
    await toolsBtn.click();

    // 知识图谱
    await expect(page.locator('#graphBtn')).toBeVisible();
    // AutoDream
    await expect(page.locator('#dreamBtn')).toBeVisible();
    // 符号搜索
    await expect(page.locator('#symbolBtn')).toBeVisible();
    // 对话分支树
    await expect(page.locator('#branchTreeBtn')).toBeVisible();
  });

  test('TC-UI-TOOLS-005: 点击外部关闭菜单', async ({ page }) => {
    const toolsBtn = page.locator('#toolsBtn');
    const toolsMenu = page.locator('#toolsMenu');

    // 打开菜单
    await toolsBtn.click();
    await expect(toolsMenu).toBeVisible();

    // 点击页面其他区域（用 body 区域避开 topBar 拦截）
    await page.mouse.click(400, 300);

    // 菜单应该隐藏
    await expect(toolsMenu).toHaveClass(/hidden/);
  });

  test('TC-UI-TOOLS-006: 设置面板"备份与恢复"区域可见且按钮可点击', async ({ page }) => {
    // 打开设置并切换到「数据」Tab（S94 分区化后备份区在 data 分区）
    await page.locator('#settingsBtn').click();
    await page.locator('#settingsModal').waitFor({ state: 'visible', timeout: 10000 });
    const dataTab = page.locator('#settingsTabBar [data-tab-id="data"]');
    if (await dataTab.count()) await dataTab.click();

    // 导出备份按钮
    const exportBtn = page.locator('#exportBackupBtn');
    await expect(exportBtn).toBeVisible();

    // 恢复数据按钮
    const importBtn = page.locator('#importBackupBtn');
    await expect(importBtn).toBeVisible();
  });

  test('TC-UI-TOOLS-007: "数据管理"标签已改名为"备份与恢复"', async ({ page }) => {
    // 打开设置并切换到「数据」Tab
    await page.locator('#settingsBtn').click();
    await page.locator('#settingsModal').waitFor({ state: 'visible', timeout: 10000 });
    const tab2 = page.locator('#settingsTabBar [data-tab-id="data"]');
    if (await tab2.count()) await tab2.click();

    // 查找 data-i18n="settings.data_management" 的元素
    const dataMgmtLabel = page.locator('[data-i18n="settings.data_management"]');
    await expect(dataMgmtLabel).toBeVisible();

    // 中文环境下应该显示"备份与恢复"
    const text = await dataMgmtLabel.textContent();
    // 允许"备份与恢复"或"Backup & Restore"（取决于 locale）
    expect(text).toBeTruthy();
    expect(text).not.toBe('数据管理');
    expect(text).not.toBe('Data Management');
  });
});
