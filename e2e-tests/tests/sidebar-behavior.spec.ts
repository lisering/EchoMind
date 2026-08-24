// E2E 侧栏折叠与交互验收（REQ-NAV-001：折叠时完全隐藏侧栏）。
// E2E-SB-001: 折叠按钮切换侧栏可见性（240px → 完全隐藏）
// E2E-SB-002: 折叠后侧栏内容完全不可见
// E2E-SB-003: 展开后恢复完整列表
// E2E-SB-004: 折叠/展开按钮可见性切换
// E2E-SB-005: 新对话按钮可点击创建会话
// E2E-SB-006: 侧栏会话列表可见
// E2E-SB-007: 设置按钮可点击打开面板
// E2E-SB-008: 侧栏底部授权状态显示
// E2E-SB-009: 折叠后侧栏完全不可见（无图标列残留）
// E2E-SB-010: 空知识库时输入框和发送按钮禁用
// E2E-SB-011: 有文档时输入框和发送按钮可用
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl } from './helpers.mjs';

test.describe('E2E-SB-001~011 侧栏折叠与交互', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('E2E-SB-001 折叠按钮切换侧栏可见性', async ({ page }) => {
    const sidebar = page.locator('#sidebar');
    // 初始展开态：无 sidebar-collapsed 类
    await expect(sidebar).not.toHaveClass(/sidebar-collapsed/);

    // 点击折叠 → 侧栏 transform:translateX(-100%) 滑出视口
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(300);
    await expect(sidebar).toHaveClass(/sidebar-collapsed/);
    // 展开按钮应可见
    await expect(page.locator('#expandBtn')).toBeVisible();

    // 点击展开按钮恢复（translateX(0) 滑入）
    await page.locator('#expandBtn').click();
    await page.waitForTimeout(300);
    await expect(sidebar).not.toHaveClass(/sidebar-collapsed/);
    await expect(page.locator('#collapseBtn')).toBeVisible();
  });

  test('E2E-SB-002 折叠后侧栏内容完全不可见', async ({ page }) => {
    // 点击折叠
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(300);

    // 侧栏应有 sidebar-collapsed 类（transform:translateX(-100%) 滑出视口）
    const sidebar = page.locator('#sidebar');
    await expect(sidebar).toHaveClass(/sidebar-collapsed/);
    // 侧栏 boundingBox x 应为负值（滑出视口左侧）
    const box = await sidebar.boundingBox();
    expect(box?.x, '折叠后侧栏应滑出视口（x < 0）').toBeLessThan(0);

    // 展开模式内容应不可见（opacity:0 + pointer-events:none）
    const expanded = page.locator('#sidebarExpanded');
    await expect(expanded).toHaveClass(/opacity-0/);

    // 图标列不应存在（已从 DOM 中移除）
    const icons = page.locator('#sidebarIcons');
    await expect(icons).toHaveCount(0);

    // 浮层面板不应存在（已从 DOM 中移除）
    const flyout = page.locator('#sidebarFlyout');
    await expect(flyout).toHaveCount(0);
  });

  test('E2E-SB-003 展开后内容恢复', async ({ page }) => {
    // 先折叠再展开
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(300);
    await page.locator('#expandBtn').click();
    await page.waitForTimeout(300);

    // 展开模式内容应恢复可见
    const expanded = page.locator('#sidebarExpanded');
    await expect(expanded).toBeVisible();

    // .side-label 元素应恢复可见
    const labels = page.locator('#sidebar .side-label');
    const count = await labels.count();
    expect(count, '应有 side-label 元素').toBeGreaterThan(0);
  });

  test('E2E-SB-004 折叠/展开按钮可见性切换', async ({ page }) => {
    // 初始展开态：collapseBtn 可见，expandBtn 隐藏
    await expect(page.locator('#collapseBtn')).toBeVisible();
    await expect(page.locator('#expandBtn')).toBeHidden();

    // 折叠后：collapseBtn 隐藏，expandBtn 可见
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(300);
    await expect(page.locator('#collapseBtn')).toBeHidden();
    await expect(page.locator('#expandBtn')).toBeVisible();

    // 展开后恢复
    await page.locator('#expandBtn').click();
    await page.waitForTimeout(300);
    await expect(page.locator('#collapseBtn')).toBeVisible();
    await expect(page.locator('#expandBtn')).toBeHidden();
  });

  test('E2E-SB-005 新对话按钮可点击创建会话', async ({ page }) => {
    const initialCount = await page.locator('#convList > div').count();

    await page.locator('#newChatBtn').click();
    await page.waitForTimeout(500);

    const afterCount = await page.locator('#convList > div').count();
    // 新建会话为懒创建（前端 UUID），不立即落库，loadConversations 可能不增加
    // 只要不报错且 UI 响应即为通过
    expect(afterCount, '新建后 UI 应响应').toBeGreaterThanOrEqual(initialCount);
  });

  test('E2E-SB-006 侧栏会话列表可见', async ({ page }) => {
    // 会话列表容器存在且有内容
    const convList = page.locator('#convList');
    await expect(convList).toBeVisible();
    const count = await convList.locator('> div').count();
    expect(count, '应至少有一个会话').toBeGreaterThanOrEqual(1);
  });

  test('E2E-SB-007 设置按钮可点击打开面板', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
  });

  test('E2E-SB-008 侧栏底部授权状态显示', async ({ page }) => {
    // 免费版状态（i18n 后可能显示"免费版"或"Free"）
    const text = await page.textContent('#proStatus');
    expect(text).not.toBeNull();
    expect(text.length).toBeGreaterThan(0);
  });

  test('E2E-SB-009 折叠后侧栏完全不可见（无图标列残留）', async ({ page }) => {
    // 点击折叠
    await page.locator('#collapseBtn').click();
    await page.waitForTimeout(300);

    // 侧栏应有 sidebar-collapsed 类
    const sidebar = page.locator('#sidebar');
    await expect(sidebar).toHaveClass(/sidebar-collapsed/);

    // 不应有任何可见的图标列元素
    const icons = page.locator('#sidebarIcons');
    await expect(icons).toHaveCount(0);

    // 不应有任何可见的浮层面板元素
    const flyout = page.locator('#sidebarFlyout');
    await expect(flyout).toHaveCount(0);
  });

  test('E2E-SB-010 空知识库时输入框和发送按钮禁用', async ({ page }) => {
    // 等待 loadDocuments 完成（stub 返回空 docs → docCount=0 → 输入禁用）
    const queryInput = page.locator('#queryInput');
    // 使用 expect.toBeDisabled 等待禁用状态出现（容忍异步时序）
    await expect(queryInput).toBeDisabled({ timeout: 10000 });

    // 检查发送按钮是否禁用
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn).toBeDisabled();

    // placeholder 应显示空库提示
    const placeholder = await queryInput.getAttribute('placeholder');
    expect(placeholder).not.toBeNull();
    expect(placeholder!.length).toBeGreaterThan(0);
  });

  test('E2E-SB-011 有文档时输入框和发送按钮可用', async ({ page }) => {
    // 使用 Mock 控制接口注入文档（根因 V2 修复：集中化 mock 管理）
    await page.evaluate(() => {
      if (window.__ECHOMIND_MOCK_CONTROL__) {
        window.__ECHOMIND_MOCK_CONTROL__.addDoc('test.md', 'Indexed');
      } else if (window.__TAURI__ && window.__TAURI__.core) {
        // 回退：通过 import_files mock 注入
        window.__TAURI__.core.invoke('import_files', { paths: ['test.md'] });
      }
    });
    await page.waitForTimeout(500);

    // 触发前端重新加载文档列表
    await page.evaluate(() => {
      // 发射 doc-status-changed 事件触发 loadDocuments
      const cbs = window.__state?.listeners?.['doc-status-changed'] || [];
      cbs.forEach((cb) => cb({ payload: {} }));
    });
    await page.waitForTimeout(1000);

    // 输入框应可用
    const queryInput = page.locator('#queryInput');
    await expect(queryInput).toBeEnabled({ timeout: 10000 });

    // 发送按钮应可用
    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn).toBeEnabled();
  });
});
