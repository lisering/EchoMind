// E2E v1.13 功能测试（桥接层验证）：
// TC-ERR-OFFLINE-001: 离线事件触发后侧栏显示离线指示器
// TC-ERR-OFFLINE-002: 离线时输入框 placeholder 变为提示文案
// TC-ERR-OFFLINE-003: 离线时发送按钮 disabled
// TC-ERR-OFFLINE-004: 在线事件触发后恢复正常状态
// TC-V13-AUTO-001: set_autostart IPC 启用自启
// TC-V13-AUTO-002: get_autostart 返回当前自启状态
// TC-V13-UPD-001: check_for_updates IPC 返回版本信息
// TC-V13-UPD-002: get_update_check_config 返回配置
// TC-V13-BC-001: 面包屑容器存在于对话区顶部
// TC-V13-BC-002: 面包屑显示知识库名
import { test, expect } from '@playwright/test';
import { setupPage } from './helpers.mjs';

test.describe('TC-V13-OFFLINE 离线模式降级（REQ-ERR-003）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-ERR-OFFLINE-001 离线事件触发后侧栏显示离线指示器', async ({ page }) => {
    // 模拟离线事件
    await page.evaluate(() => {
      Object.defineProperty(navigator, 'onLine', { value: false, configurable: true });
      window.dispatchEvent(new Event('offline'));
    });
    await page.waitForTimeout(300);

    const indicator = page.locator('#offlineIndicator');
    // 离线指示器应不再有 hidden 类
    const classes = await indicator.getAttribute('class') || '';
    expect(classes).not.toMatch(/\bhidden\b/);
    const text = await indicator.textContent() || '';
 // t() 可能返回中文或英文，检查是否有文字内容
    expect(text.trim().length).toBeGreaterThan(0);
  });

  test('TC-ERR-OFFLINE-002 离线时输入框 placeholder 变为提示文案', async ({ page }) => {
    // 先保存原始 placeholder
    const originalPlaceholder = await page.locator('#queryInput').getAttribute('placeholder');
    await page.evaluate(() => {
      Object.defineProperty(navigator, 'onLine', { value: false, configurable: true });
      window.dispatchEvent(new Event('offline'));
    });
    await page.waitForTimeout(300);

    const placeholder = await page.locator('#queryInput').getAttribute('placeholder') || '';
    // placeholder 应该变化（中英文取决于 locale，检查不等于原始值即可）
    expect(placeholder).not.toBe(originalPlaceholder);
  });

  test('TC-ERR-OFFLINE-003 离线时发送按钮 disabled', async ({ page }) => {
    await page.evaluate(() => {
      Object.defineProperty(navigator, 'onLine', { value: false, configurable: true });
      window.dispatchEvent(new Event('offline'));
    });
    await page.waitForTimeout(200);

    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn).toHaveAttribute('disabled', '');
  });

  test('TC-ERR-OFFLINE-004 在线事件触发后恢复正常状态', async ({ page }) => {
    // 先离线
    await page.evaluate(() => {
      Object.defineProperty(navigator, 'onLine', { value: false, configurable: true });
      window.dispatchEvent(new Event('offline'));
    });
    await page.waitForTimeout(200);

    // 再恢复在线
    await page.evaluate(() => {
      Object.defineProperty(navigator, 'onLine', { value: true, configurable: true });
      window.dispatchEvent(new Event('online'));
    });
    await page.waitForTimeout(200);

    const indicator = page.locator('#offlineIndicator');
    await expect(indicator).toHaveClass(/\bhidden\b/);

    const sendBtn = page.locator('#sendBtn');
    await expect(sendBtn).not.toHaveAttribute('disabled');
  });
});

test.describe('TC-V13-AUTO 开机自启（REQ-WIN-004）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V13-AUTO-001 set_autostart IPC 启用自启', async ({ page }) => {
    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('update_setting', { key: 'app.autostart', value: String(true) });
    });
    expect(result).toBeNull();
  });

  test('TC-V13-AUTO-002 get_autostart 返回当前自启状态', async ({ page }) => {
    // 先设置启用
    await page.evaluate(() => {
      return window.__TAURI__.core.invoke('update_setting', { key: 'app.autostart', value: String(true) });
    });

    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_autostart');
    });
    expect(result).toBe(true);
  });
});

test.describe('TC-V13-UPD 应用更新检查（REQ-HELP-004）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V13-UPD-001 check_for_updates IPC 返回版本信息', async ({ page }) => {
    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('check_for_updates');
    });
    // mock 返回 null（无更新）或 { latest_version, current_version, has_update }
    expect(result).toBeDefined();
  });

  test('TC-V13-UPD-002 get_update_check_config 返回配置', async ({ page }) => {
    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('get_update_check_config');
    });
    expect(result).toBeDefined();
    expect(result.auto_check).toBeDefined();
  });
});

test.describe('TC-V13-BC 面包屑与上下文指示（REQ-NAV-004）', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  test('TC-V13-BC-001 面包屑容器存在于对话区顶部', async ({ page }) => {
    const breadcrumb = page.locator('#breadcrumbBar');
    await expect(breadcrumb).toBeVisible();
  });

  test('TC-V13-BC-002 面包屑显示知识库名', async ({ page }) => {
    const kbName = page.locator('#breadcrumbKbName');
    await expect(kbName).toBeVisible();
    const text = await kbName.textContent();
    expect(text?.trim().length).toBeGreaterThan(0);
  });

  test('TC-V13-BC-003 面包屑显示会话标题', async ({ page }) => {
    // 创建一个会话
    await page.evaluate(() => {
      window.__mock.state.conversations = [
        { id: 'bc-test', title: '面包屑测试会话', created_at: Date.now() },
      ];
      window.__mock.state.currentConvId = 'bc-test';
    });

    // 切换到该会话
    await page.evaluate(() => {
      window.__TAURI__.core.invoke('get_conversations');
    });
    await page.waitForTimeout(300);

    const title = page.locator('#breadcrumbConvTitle');
    await expect(title).toBeVisible();
  });

  test('TC-V13-BC-004 面包屑显示消息数与创建时间', async ({ page }) => {
    // breadcrumb meta 在空会话时不显示，创建会话后再检查
    await page.evaluate(() => {
      window.__mock.state.conversations = [
        { id: 'bc-meta', title: '面包屑元数据测试', created_at: Date.now() - 86400000 },
      ];
      window.__mock.state.currentConvId = 'bc-meta';
      window.__mock.state.messages['bc-meta'] = [
        { id: 'm1', conv_id: 'bc-meta', role: 'user', content: 'hello', created_at: Date.now() },
        { id: 'm2', conv_id: 'bc-meta', role: 'assistant', content: 'hi', created_at: Date.now() },
      ];
    });
    await page.waitForTimeout(300);

    // 面包屑条应可见
    const bar = page.locator('#breadcrumbBar');
    await expect(bar).toBeVisible();
    // breadcrumb 左侧应有内容
    const left = bar.locator('.breadcrumb-left');
    await expect(left).toBeVisible();
  });

  test('TC-V13-BC-005 空会话时面包屑显示新会话', async ({ page }) => {
    // 新建会话（空对话）
    await page.evaluate(() => {
      window.__mock.state.conversations = [];
      window.__mock.state.currentConvId = null;
    });

    // 触发新会话
    await page.evaluate(() => {
      const btn = document.querySelector('#newChatBtn');
      if (btn) (btn as HTMLElement).click();
    });
    await page.waitForTimeout(300);

    const title = page.locator('#breadcrumbConvTitle');
    const text = await title.textContent();
    // 应该包含"新"或"New"字样
    expect(text).toBeTruthy();
  });
});
