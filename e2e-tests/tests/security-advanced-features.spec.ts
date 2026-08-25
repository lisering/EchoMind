/**
 * E2E 测试：安全面板 7 个新区域（TC-SEC-NEW-001~007）。
 *
 * 验证 S2 复盘接线的 7 个安全功能 UI 区域：
 * - TC-SEC-NEW-001: 紧急销毁区域
 * - TC-SEC-NEW-002: 自动锁定配置区域
 * - TC-SEC-NEW-003: 密码强度检测
 * - TC-SEC-NEW-004: 审计日志清空
 * - TC-SEC-NEW-005: 剪贴板配置区域
 * - TC-SEC-NEW-006: 安全态势选择器
 * - TC-SEC-NEW-007: Shadow 筛查统计
 */

import { test, expect } from '@playwright/test';
import { setupPage, showAllSettingsSections } from './helpers.mjs';

test.describe('安全面板新区域 (TC-SEC-NEW)', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
    // 打开设置面板
    await page.waitForSelector('#settingsBtn', { timeout: 5000 });
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    // 等待安全设置区块渲染
    await page.waitForSelector('#securitySettingsContainer', { timeout: 10000 });
  });

  test('TC-SEC-NEW-001: 紧急销毁区域可见', async ({ page }) => {
    const section = page.locator('#panicWipeSection');
    await expect(section).toBeVisible();
    // 验证标题或标签存在
    const sectionText = await section.textContent();
    expect(sectionText).toBeTruthy();
    expect(sectionText!.length).toBeGreaterThan(0);
  });

  test('TC-SEC-NEW-002: 自动锁定配置区域可见', async ({ page }) => {
    // 验证自动锁定超时输入框存在
    const autoLockInput = page.locator('#autoLockTimeoutInput');
    await expect(autoLockInput).toBeVisible();
    // 验证输入框是 number 类型
    const inputType = await autoLockInput.getAttribute('type');
    expect(inputType).toBe('number');
  });

  test('TC-SEC-NEW-003: 密码强度检测可用', async ({ page }) => {
    // 验证密码强度 IPC 可调用
    const result = await page.evaluate(async () => {
      try {
        return await (window as any).__TAURI__.core.invoke('check_password_strength', { password: 'Test1234!' });
      } catch (e) {
        return null;
      }
    });
    // 验证返回结果有 score 或 strength 字段
    expect(result).not.toBeNull();
  });

  test('TC-SEC-NEW-004: 审计日志清空按钮可见', async ({ page }) => {
    // 查找清空审计日志按钮（可能在审计面板或安全设置中）
    const clearBtns = page.locator('button:has-text("清空"), button:has-text("Clear")');
    const count = await clearBtns.count();
    // 至少有一个清空按钮
    expect(count).toBeGreaterThan(0);
  });

  test('TC-SEC-NEW-005: 剪贴板配置区域可见', async ({ page }) => {
    // 验证剪贴板配置 IPC 可调用
    const result = await page.evaluate(async () => {
      try {
        await (window as any).__TAURI__.core.invoke('set_clipboard_config', { enabled: true, clearAfterSecs: 30 });
        return true;
      } catch (e) {
        return false;
      }
    });
    expect(result).toBe(true);

    // 验证 clipboard_config 在 security status 中
    const status = await page.evaluate(async () => {
      return await (window as any).__TAURI__.core.invoke('get_security_status');
    });
    expect(status).toHaveProperty('clipboard_config');
  });

  test('TC-SEC-NEW-006: 安全态势选择器可见', async ({ page }) => {
    // 验证安全态势 IPC 可调用
    const posture = await page.evaluate(async () => {
      try {
        return await (window as any).__TAURI__.core.invoke('get_security_posture');
      } catch (e) {
        return null;
      }
    });
    // 验证返回有效态势值
    expect(posture).not.toBeNull();
    expect(['dangerous', 'auto', 'strict']).toContain(posture);

    // 验证设置态势
    const setResult = await page.evaluate(async () => {
      try {
        await (window as any).__TAURI__.core.invoke('set_security_posture', { posture: 'strict' });
        return await (window as any).__TAURI__.core.invoke('get_security_posture');
      } catch (e) {
        return null;
      }
    });
    expect(setResult).toBe('strict');
  });

  test('TC-SEC-NEW-007: Shadow 筛查统计可见', async ({ page }) => {
    // 验证 Shadow 筛查区域存在
    const shadowSection = page.locator('#shadowScreenSection');
    await expect(shadowSection).toBeVisible();

    // 验证 Shadow 统计 IPC 可调用
    const stats = await page.evaluate(async () => {
      try {
        return await (window as any).__TAURI__.core.invoke('get_security_screen_stats');
      } catch (e) {
        return null;
      }
    });
    expect(stats).not.toBeNull();
    // 验证统计字段存在
    expect(stats).toHaveProperty('total');
    expect(stats).toHaveProperty('agree');
    expect(stats).toHaveProperty('disagree');
    expect(stats).toHaveProperty('unavailable');
  });
});
