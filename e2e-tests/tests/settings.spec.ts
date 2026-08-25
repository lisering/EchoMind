// E2E 设置面板 + VLM 开关 + 隐私弹窗（REQ-MM-003 前端接线）。
// E2E-MM-001: 设置面板打开与 LLM 配置展示
// E2E-MM-002: 设置面板关闭
// E2E-MM-003: VLM 开关初始关闭状态
// E2E-MM-004: VLM 开启 — 隐私确认弹窗弹出
// E2E-MM-005: VLM 开启 — 取消确认（不持久化）
// E2E-MM-006: VLM 开启 — 确认开启持久化
// E2E-MM-007: VLM 关闭 — 直接生效无需确认
// E2E-MM-008: VLM 状态持久化与恢复
// E2E-MM-009: 设置面板修改 LLM 配置入口
import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl, showAllSettingsSections } from './helpers.mjs';
test.describe('E2E-MM-001~009 设置面板 + VLM 开关 + 隐私弹窗（REQ-MM-003）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    // V3.1 阶段二：S94 Tab 化——测试专用视图显示全部分区（幂等）
    await showAllSettingsSections(page);
  });

  // ─── 设置面板基础交互 ───

  test('E2E-MM-001 设置面板打开与 LLM 配置展示', async ({ page }) => {
    // 点击侧栏设置按钮
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }

    // 设置 Modal 可见
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // LLM 配置信息区展示已配置的端点、模型、脱敏 Key
    const info = page.locator('#settingsLlmInfo');
    await expect(info).toContainText('http://mock.local');
    await expect(info).toContainText('mock-llm');
    await expect(info).toContainText('****-e2e');
  });

  test('E2E-MM-002 设置面板关闭', async ({ page }) => {
    // 打开设置面板
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 点击「完成」关闭
    await page.locator('#settingsClose').click();
    await expect(page.locator('#settingsModal')).toBeHidden();
  });

  // ─── VLM 开关初始状态 ───

  test('E2E-MM-003 VLM 开关初始关闭状态', async ({ page }) => {
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // VLM 开关初始为关闭
    const toggle = page.locator('#vlmToggle');
    await expect(toggle).toHaveAttribute('aria-checked', 'false');
    await expect(toggle).toHaveClass(/bg-slate-600/);

    // 隐私提示隐藏
    await expect(page.locator('#vlmPrivacy')).toBeHidden();
  });

  // ─── VLM 开启流程：隐私确认弹窗 ───

  test('E2E-MM-004 VLM 开启 — 隐私确认弹窗弹出', async ({ page }) => {
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 点击 VLM 开关（off → on）
    await page.locator('#vlmToggle').click();

    // 隐私确认弹窗出现
    await expect(page.locator('#vlmConfirm')).toBeVisible({ timeout: 5000 });

    // 弹窗含 BYOK 隐私提示文案
    await expect(page.locator('#vlmConfirm')).toContainText('BYOK');
    await expect(page.locator('#vlmConfirm')).toContainText('图片数据将离开本地');

    // 弹窗含确认和取消按钮
    await expect(page.locator('#vlmConfirmOk')).toBeVisible();
    await expect(page.locator('#vlmConfirmCancel')).toBeVisible();
  });

  test('E2E-MM-005 VLM 开启 — 取消确认（不持久化）', async ({ page }) => {
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 确认初始 VLM 关闭
    const vlmBefore = await page.evaluate(() => window.__mock.state.vlmEnabled);
    expect(vlmBefore).toBe(false);

    // 点击开关 → 弹出确认弹窗
    await page.locator('#vlmToggle').click();
    await expect(page.locator('#vlmConfirm')).toBeVisible({ timeout: 5000 });

    // 点击取消
    await page.locator('#vlmConfirmCancel').click();

    // 弹窗关闭
    await expect(page.locator('#vlmConfirm')).toBeHidden();

    // 开关保持关闭状态
    await expect(page.locator('#vlmToggle')).toHaveAttribute('aria-checked', 'false');
    await expect(page.locator('#vlmPrivacy')).toBeHidden();

    // 后端 set_vlm_enabled 未被调用（mock state 仍为 false）
    const vlmAfter = await page.evaluate(() => window.__mock.state.vlmEnabled);
    expect(vlmAfter).toBe(false);
  });

  test('E2E-MM-006 VLM 开启 — 确认开启持久化', async ({ page }) => {
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 点击开关 → 弹出确认弹窗
    await page.locator('#vlmToggle').click();
    await expect(page.locator('#vlmConfirm')).toBeVisible({ timeout: 5000 });

    // 点击确认开启
    await page.locator('#vlmConfirmOk').click();

    // 弹窗关闭
    await expect(page.locator('#vlmConfirm')).toBeHidden();

    // 开关变为开启状态
    const toggle = page.locator('#vlmToggle');
    await expect(toggle).toHaveAttribute('aria-checked', 'true');
    await expect(toggle).toHaveClass(/bg-accent/);

    // 隐私提示可见
    await expect(page.locator('#vlmPrivacy')).toBeVisible();
    await expect(page.locator('#vlmPrivacy')).toContainText('BYOK');

    // 后端 set_vlm_enabled(true) 已调用（mock state 更新为 true）
    const vlmState = await page.evaluate(() => window.__mock.state.vlmEnabled);
    expect(vlmState).toBe(true);

    // Toast 提示「VLM 增强已开启」
    await expect(page.locator('#toasts')).toContainText('VLM 增强已开启', { timeout: 5000 });
  });

  // ─── VLM 关闭流程 ───

  test('E2E-MM-007 VLM 关闭 — 直接生效无需确认', async ({ page }) => {
    // 预置 VLM 已开启状态（模拟后端已持久化）
    // V3.1：beforeEach 已打开设置——预置 stub 状态后需重开设置触发 UI 重同步
    await page.keyboard.press('Escape');
    await page.evaluate(() => {
      window.__mock.state.vlmEnabled = true;
    });
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    await expect(page.locator('#vlmToggle')).toHaveAttribute('aria-checked', 'true');
    await expect(page.locator('#vlmPrivacy')).toBeVisible();

    // 点击开关（on → off）
    await page.locator('#vlmToggle').click();

    // 不弹出确认弹窗（关闭无需确认）
    await expect(page.locator('#vlmConfirm')).toBeHidden();

    // 开关变为关闭状态
    await expect(page.locator('#vlmToggle')).toHaveAttribute('aria-checked', 'false');
    await expect(page.locator('#vlmPrivacy')).toBeHidden();

    // 后端 set_vlm_enabled(false) 已调用
    const vlmState = await page.evaluate(() => window.__mock.state.vlmEnabled);
    expect(vlmState).toBe(false);

    // Toast 提示「VLM 增强已关闭」
    await expect(page.locator('#toasts')).toContainText('VLM 增强已关闭', { timeout: 5000 });
  });

  // ─── VLM 状态持久化与恢复 ───

  test('E2E-MM-008 VLM 状态持久化与恢复', async ({ page }) => {
    // 第一次打开设置面板，开启 VLM
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    await page.locator('#vlmToggle').click();
    await expect(page.locator('#vlmConfirm')).toBeVisible({ timeout: 5000 });
    await page.locator('#vlmConfirmOk').click();

    // 确认开关已开启 + 后端已持久化
    await expect(page.locator('#vlmToggle')).toHaveAttribute('aria-checked', 'true');
    const vlmAfterEnable = await page.evaluate(() => window.__mock.state.vlmEnabled);
    expect(vlmAfterEnable).toBe(true);

    // 关闭设置面板
    await page.locator('#settingsClose').click();
    await expect(page.locator('#settingsModal')).toBeHidden();

    // 再次打开设置面板 — VLM 开关应恢复为开启状态（从后端 get_settings 读取）
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    await expect(page.locator('#vlmToggle')).toHaveAttribute('aria-checked', 'true');
    await expect(page.locator('#vlmToggle')).toHaveClass(/bg-accent/);
    await expect(page.locator('#vlmPrivacy')).toBeVisible();
  });

  // ─── 设置面板 LLM 配置修改入口 ───

  test('E2E-MM-009 设置面板修改 LLM 配置入口', async ({ page }) => {
    if (!(await page.locator('#settingsModal').isVisible().catch(() => false))) {
      await page.locator('#settingsBtn').click();
    }
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 点击「修改 LLM 配置」
    await page.locator('#settingsEditLlm').click();

    // 设置面板关闭，向导重新出现
    await expect(page.locator('#settingsModal')).toBeHidden();
    await expect(page.locator('#wizard')).toBeVisible();
  });
});
