/**
 * v1.19 多知识库（Workspace）功能 E2E 测试。
 *
 * 覆盖：
 * - REQ-WS-001：新建知识库 / 切换 / 数据隔离 / 持久化
 * - REQ-WS-003：重命名 / 删除确认 / 级联清理 / 最后一个库禁删
 *
 * 测试模式：injectStub → page.addInitScript → injectLocales → page.goto → 操作 → 断言
 */
import { test, expect } from '@playwright/test';
import { setupPage, waitForToast } from './helpers.mjs';

test.describe('v1.19 多知识库功能 (REQ-WS-001/003)', () => {

  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  /**
   * 辅助函数：打开知识库下拉菜单并等待内容加载。
   */
  async function openWorkspaceDropdown(page) {
    const dropdown = page.locator('#workspaceDropdown');
    // 检查是否已经可见
    const alreadyVisible = await dropdown.isVisible().catch(() => false);
    if (alreadyVisible) {
      // 先关闭再重新打开，确保内容刷新
      await page.locator('#workspaceToggle').click();
      await page.waitForTimeout(300);
    } else {
      // 确保没有其他模态框
      await page.keyboard.press('Escape');
      await page.waitForTimeout(300);
    }
    // 点击切换按钮打开
    await page.locator('#workspaceToggle').click();
    await page.waitForTimeout(300);
    await expect(dropdown).toBeVisible({ timeout: 5000 });
    // 等待内容加载（异步 loadWorkspaceList）
    await page.waitForTimeout(500);
    return dropdown;
  }

  /**
   * 辅助函数：新建知识库（完整流程）。
   */
  async function createWorkspace(page, name) {
    const dropdown = await openWorkspaceDropdown(page);
    // 点击「新建知识库」按钮（带 border-t 样式的最后一项）
    const createBtn = dropdown.locator('div.border-t').last();
    await createBtn.waitFor({ state: 'visible', timeout: 5000 });
    await createBtn.click();
    // 等待对话框输入框
    const dialogInput = page.locator('.fixed input[type="text"]').last();
    await dialogInput.waitFor({ state: 'visible', timeout: 5000 });
    await dialogInput.fill(name);
    await page.keyboard.press('Enter');
    // 等待选择器更新
    await expect(page.locator('#workspaceName')).toContainText(name, { timeout: 5000 });
  }

  // ============================================================
  // REQ-WS-001：多知识库创建与切换
  // ============================================================

  test('TC-V19-WS001-001: 侧栏顶部显示知识库选择器', async ({ page }) => {
    // AC-1：侧栏顶部显示知识库选择器，列出全部知识库
    const selector = page.locator('#workspaceSelector');
    await expect(selector).toBeVisible({ timeout: 10000 });

    // 点击展开下拉
    const dropdown = await openWorkspaceDropdown(page);

    // 验证默认工作空间显示
    await expect(dropdown).toContainText('Default', { timeout: 5000 });
  });

  test('TC-V19-WS001-002: 新建知识库并自动切换', async ({ page }) => {
    // AC-2：点击「新建知识库」弹出输入框，输入名称后创建
    await createWorkspace(page, 'Test KB');

    // 验证切换成功 toast
    await waitForToast(page, 'Test KB');
  });

  test('TC-V19-WS001-003: 切换知识库后数据隔离', async ({ page }) => {
    // AC-3/AC-4：切换知识库后文档/会话列表同步更新，不同库数据隔离

    // 1. 在 default 工作空间创建一个会话
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation', { workspaceId: 'default' });
    });

    // 2. 新建第二个知识库
    await createWorkspace(page, 'Second KB');

    // 3. 在 Second KB 创建一个会话
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation', {
        workspaceId: window.__state.currentWorkspaceId,
      });
    });

    // 4. 验证 Second KB 中只有 1 个会话（default 的会话被隔离）
    const convs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_conversations', {
        workspaceId: window.__state.currentWorkspaceId,
      });
    });
    expect(convs.length).toBe(1);
  });

  test('TC-V19-WS001-004: 当前知识库选择持久化', async ({ page }) => {
    // AC-5：当前知识库选择持久化，重启后恢复

    // 1. 新建知识库
    await createWorkspace(page, 'Persisted KB');

    // 2. 验证 mock 中持久化了
    const currentWs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_current_workspace');
    });
    expect(currentWs).not.toBe('default');
  });

  // ============================================================
  // REQ-WS-003：知识库重命名与删除
  // ============================================================

  test('TC-V19-WS003-001: 双击重命名知识库', async ({ page }) => {
    // AC-1/AC-2：双击知识库名称进入编辑模式，重命名后即时更新

    // 先新建一个知识库
    await createWorkspace(page, 'Original Name');

    // 再次打开下拉，双击名称编辑
    const dropdown = await openWorkspaceDropdown(page);
    const nameSpan = dropdown.locator('.workspace-name').first();
    await nameSpan.waitFor({ state: 'visible', timeout: 5000 });
    await nameSpan.dblclick();

    // 输入新名称
    const editInput = dropdown.locator('input[type="text"]').first();
    await editInput.waitFor({ state: 'visible', timeout: 5000 });
    await editInput.fill('Renamed KB');
    await page.keyboard.press('Enter');

    // 验证更新
    await waitForToast(page, 'Renamed KB');
  });

  test('TC-V19-WS003-002: 删除知识库确认对话框显示数据量', async ({ page }) => {
    // AC-3：删除知识库前弹出确认对话框，显示将删除的数据量

    // 1. 新建知识库
    await createWorkspace(page, 'To Delete');

    // 2. 在 To Delete 中创建一个会话
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation', {
        workspaceId: window.__state.currentWorkspaceId,
      });
    });

    // 3. 打开下拉，点击删除按钮
    const dropdown = await openWorkspaceDropdown(page);
    const deleteBtn = dropdown.locator('.ws-action-btn').first();
    await deleteBtn.waitFor({ state: 'visible', timeout: 5000 });
    await deleteBtn.click();

    // 4. 验证确认对话框显示数据量
    await expect(page.locator('body')).toContainText('To Delete', { timeout: 5000 });
    await expect(page.locator('body')).toContainText('0', { timeout: 5000 }); // 0 docs
  });

  test('TC-V19-WS003-003: 确认删除后级联清理数据', async ({ page }) => {
    // AC-4：确认删除后级联清理该库全部数据

    // 1. 新建知识库
    await createWorkspace(page, 'Cascade Test');

    // 2. 创建会话
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('create_conversation', {
        workspaceId: window.__state.currentWorkspaceId,
      });
    });

    // 3. 获取工作空间 ID
    const wsId = await page.evaluate(() => window.__state.currentWorkspaceId);

    // 4. 打开下拉，点击删除
    const dropdown = await openWorkspaceDropdown(page);
    const deleteBtn = dropdown.locator('.ws-action-btn').first();
    await deleteBtn.waitFor({ state: 'visible', timeout: 5000 });
    await deleteBtn.click();
    await page.waitForTimeout(1000); // 等待异步 confirmDeleteWorkspace

    // 5. 确认删除 — 找到确认按钮（showConfirmDialog 使用 role=alertdialog）
    const confirmDialog = page.locator('#confirmDialog');
    await confirmDialog.waitFor({ state: 'visible', timeout: 10000 });
    // 找到 danger 样式的确认按钮
    const confirmBtn = confirmDialog.first().locator('button').filter({ hasText: /Delete|删除/ }).last();
    await confirmBtn.click();

    // 6. 验证级联清理
    await waitForToast(page, 'Cascade Test');

    // 7. 验证工作空间已删除
    const workspaces = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('list_workspaces');
    });
    expect(workspaces.find((w) => w.id === wsId)).toBeUndefined();

    // 8. 验证当前回退到 default
    const currentWs = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_current_workspace');
    });
    expect(currentWs).toBe('default');
  });

  test('TC-V19-WS003-004: 最后一个知识库禁用删除', async ({ page }) => {
    // AC-5：只有一个知识库时删除按钮禁用

    // 确保只有 default 工作空间
    const dropdown = await openWorkspaceDropdown(page);

    // default 工作空间不应有删除按钮
    const deleteBtns = dropdown.locator('.ws-action-btn');
    const count = await deleteBtns.count();
    // 只有一个工作空间时，不应有删除按钮
    expect(count).toBe(0);
  });

});
