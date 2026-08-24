// E2E License 停用与状态展示 UI（REQ-LIC-004）。
// E2E-LIC-005: 设置面板 Pro 版授权状态展示
// E2E-LIC-006: 停用按钮可见
// E2E-LIC-007: 停用后状态回落为免费版
// E2E-LIC-008: 停用后 toast 提示
// E2E-LIC-009: 停用后审计按钮消失
// E2E-LIC-010: 停用后侧栏状态更新
// E2E-LIC-011: 停用后重新激活可用
import { test, expect } from '@playwright/test';
import { activatePro, enterApp, importDocs, injectLocales, injectStub, uiUrl } from './helpers.mjs';
test.describe('E2E-LIC-005~011 License 停用与状态展示', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
    await activatePro(page);
    await importDocs(page, ['/mock/lic-test.md']);
  });

  test('E2E-LIC-005 设置面板 Pro 版授权状态展示', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 授权状态区应显示 Pro 版已激活
    const licenseInfo = page.locator('#settingsLicenseInfo');
    await expect(licenseInfo).toContainText('Pro 版');
    await expect(licenseInfo).toContainText('已激活');
  });

  test('E2E-LIC-006 停用按钮可见', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 停用按钮应存在
    const deactivateBtn = page.locator('#deactivateBtn');
    await expect(deactivateBtn).toBeVisible();
    await expect(deactivateBtn).toContainText('停用');
  });

  test('E2E-LIC-007 停用后状态回落为免费版', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 点击停用
    await page.locator('#deactivateBtn').click();
    await page.waitForTimeout(500);

    // 后端 isPro 应为 false
    const isPro = await page.evaluate(() => window.__state.isPro);
    expect(isPro, '停用后 isPro 应为 false').toBe(false);

    // 重新打开设置面板（deactivatePro 内部会调用 openSettings）
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    // 授权状态应显示免费版
    const licenseInfo = page.locator('#settingsLicenseInfo');
    await expect(licenseInfo).toContainText('免费版', { timeout: 5000 });
  });

  test('E2E-LIC-008 停用后 toast 提示', async ({ page }) => {
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });

    await page.locator('#deactivateBtn').click();

    // 应出现 toast 提示停用
    await expect(page.locator('#toasts')).toContainText('停用', { timeout: 5000 });
    await expect(page.locator('#toasts')).toContainText('免费版', { timeout: 5000 });
  });

  test('E2E-LIC-009 停用后审计按钮消失', async ({ page }) => {
    // RC4 修复：#docList 在 KB Modal 内，需先打开
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });

    // 停用前：审计按钮可见
    let docItem = page.locator('#docList [data-doc-name="lic-test.md"]');
    await docItem.hover();
    let auditBtn = docItem.locator('button[title="审计文档一致性"]');
    await expect(auditBtn).toBeVisible();

    // RC6 修复：先关闭 KB Modal 再打开设置面板，避免 z-index 冲突
    await page.locator('#kbCloseBtn').click();
    await expect(page.locator('#kbModal')).toBeHidden({ timeout: 3000 });

    // 通过设置面板停用 Pro
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.locator('#deactivateBtn').click();
    await page.waitForTimeout(500);
    // 关闭设置面板
    await page.locator('#settingsClose').click();
    await expect(page.locator('#settingsModal')).toBeHidden({ timeout: 3000 });

    // 停用后：审计按钮应隐藏
    // RC4 修复：需重新打开 KB Modal 才能访问 #docList
    await page.locator('#kbBtn').click();
    await expect(page.locator('#kbModal')).toBeVisible({ timeout: 3000 });
    docItem = page.locator('#docList [data-doc-name="lic-test.md"]');
    await docItem.hover();
    auditBtn = docItem.locator('button[title="审计文档一致性"]');
    await expect(auditBtn).toBeHidden({ timeout: 3000 });
  });

  test('E2E-LIC-010 停用后侧栏状态更新', async ({ page }) => {
    // 停用前：侧栏显示 Pro
    // RC4 修复：i18n pro_badge_pro = "Pro"（非 "Pro 版"），匹配实际文案
    await expect(page.locator('#proStatus')).toContainText('Pro');

    // 通过设置面板停用 Pro
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.locator('#deactivateBtn').click();
    await page.waitForTimeout(500);

    // 侧栏应显示免费版
    await expect(page.locator('#proStatus')).toContainText('免费版');
  });

  test('E2E-LIC-011 停用后重新激活可用', async ({ page }) => {
    // 先通过设置面板停用
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.locator('#deactivateBtn').click();
    await page.waitForTimeout(500);
    // 关闭设置面板
    await page.locator('#settingsClose').click();
    await expect(page.locator('#settingsModal')).toBeHidden();

    // 确认已停用
    // RC4 修复：i18n pro_badge_free = "免费版"
    await expect(page.locator('#proStatus')).toContainText('免费版');

    // 再通过付费墙流程重新激活
    await page.evaluate(() => window.__mock.simulateDragDrop(['/mock/reactivate.pdf']));
    await expect(page.locator('#paywall')).toBeVisible({ timeout: 5000 });
    await page.locator('#licenseInput').fill('new-pro-key');
    await page.locator('#paywallActivate').click();
    await expect(page.locator('#paywall')).toBeHidden({ timeout: 5000 });

    // 应恢复 Pro 状态
    const isPro = await page.evaluate(() => window.__state.isPro);
    expect(isPro, '重新激活后 isPro 应为 true').toBe(true);

    // 侧栏应恢复 Pro
    // RC4 修复：i18n pro_badge_pro = "Pro"（非 "Pro 版"）
    await expect(page.locator('#proStatus')).toContainText('Pro');
  });
});
