/**
 * 3 步启动向导全面 E2E 测试。
 *
 * 测试覆盖：
 * 1. 全新安装：模型未下载 + LLM 未配置 → Step 1 下载界面
 * 2. 模型已下载 + LLM 未配置 → Step 2 配置界面
 * 3. 全部就绪 → 直接进入主界面
 * 4. Step 1 下载进度条 + 自动进入 Step 2
 * 5. Step 1 下载失败 + 重试
 * 6. Step 2 LLM 配置验证 → Step 3
 * 7. Step 2 跳过 → Step 3
 * 8. Step 3 文档导入 → 完成向导
 * 9. 完整 3 步流程端到端
 * 10. 步骤标签随步骤变化
 * 11. 下载进度显示文件名和大小
 * 12. 镜像源指示器在下载时显示
 */
import { test, expect } from '@playwright/test';
import { injectStub, injectLocales, uiUrl } from './helpers.mjs';

/**
 * 注入测试配置到页面（在 stub 之前执行，stub 会读取 window.__TEST_OPTS）。
 */
async function setupTestEnv(page, opts: Record<string, unknown> = {}) {
  // 先注入测试配置（addInitScript 按顺序执行，先执行的先运行）
  await page.addInitScript((testOpts) => {
    (window as any).__TEST_OPTS = testOpts;
  }, opts);
  // 再注入 stub（stub 会读取 window.__TEST_OPTS 初始化状态）
  await injectStub(page);
  await injectLocales(page);
}

test.describe('3 步启动向导', () => {

  // ============================================================
  // TC-ONBOARD-001: 全新安装 → Step 1 下载界面
  // ============================================================
  test('TC-ONBOARD-001 全新安装显示 Step 1 下载界面', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'needs_download', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizard')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#wizardStep2')).toBeHidden();
    await expect(page.locator('#wizardStep3')).toBeHidden();
    await expect(page.locator('#wizDownloadBar')).toBeVisible();
    await expect(page.locator('#wizDownloadStatus')).toBeVisible();
    await expect(page.locator('#wizStepDot1')).toHaveClass(/active/);
    await expect(page.locator('#wizStepDot2')).not.toHaveClass(/active/);
  });

  // ============================================================
  // TC-ONBOARD-002: 模型已下载 + LLM 未配置 → Step 2
  // ============================================================
  test('TC-ONBOARD-002 模型就绪但 LLM 未配置 → Step 2', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'ready', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizard')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#wizardStep1')).toBeHidden();
    await expect(page.locator('#wizStepDot1')).toHaveClass(/completed/);
    await expect(page.locator('#wizStepDot2')).toHaveClass(/active/);
  });

  // ============================================================
  // TC-ONBOARD-003: 全部就绪 → 直接进入主界面
  // ============================================================
  test('TC-ONBOARD-003 模型就绪 + LLM 已配置 → 直接进入主界面', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'ready', configured: true });
    await page.goto(uiUrl);

    await expect(page.locator('#app')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizard')).toBeHidden();
  });

  // ============================================================
  // TC-ONBOARD-004: Step 1 下载进度更新 + 自动进入 Step 2
  // ============================================================
  test('TC-ONBOARD-004 下载进度条更新并自动进入 Step 2', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'needs_download', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 10000 });
    // 等待进度条 > 0
    await expect(page.locator('#wizDownloadBar')).not.toHaveCSS('width', '0px', { timeout: 5000 });
    // 等待自动进入 Step 2
    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizardStep1')).toBeHidden();
    await expect(page.locator('#wizStepDot1')).toHaveClass(/completed/);
    await expect(page.locator('#wizStepDot2')).toHaveClass(/active/);
  });

  // ============================================================
  // TC-ONBOARD-005: Step 1 下载失败 → 自动重试 → 成功
  // ============================================================
  test('TC-ONBOARD-005 下载失败自动重试后成功', async ({ page }) => {
    await setupTestEnv(page, {
      embedderStatus: 'needs_download',
      embedderDownloadFail: true,
      configured: false,
    });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 10000 });

    // 第一次失败 → 自动重试（2s 后）→ 第二次成功（stub 中 fail 标志已重置）
    // 自动重试成功后进入 Step 2
    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 15000 });
  });

  // ============================================================
  // TC-ONBOARD-006: Step 2 LLM 配置验证 → Step 3
  // ============================================================
  test('TC-ONBOARD-006 LLM 配置验证成功后进入 Step 3', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'ready', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 10000 });
    await page.locator('#wizKey').fill('sk-e2e-mock');
    await page.locator('#wizStart').click();

    await expect(page.locator('#wizardStep3')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#wizardStep2')).toBeHidden();
    await expect(page.locator('#wizStepDot1')).toHaveClass(/completed/);
    await expect(page.locator('#wizStepDot2')).toHaveClass(/completed/);
    await expect(page.locator('#wizStepDot3')).toHaveClass(/active/);
  });

  // ============================================================
  // TC-ONBOARD-007: Step 2 跳过 → Step 3
  // ============================================================
  test('TC-ONBOARD-007 跳过 LLM 配置直接进入 Step 3', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'ready', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 10000 });
    await page.locator('#wizSkipStep2').click();

    await expect(page.locator('#wizardStep3')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#wizardStep2')).toBeHidden();
  });

  // ============================================================
  // TC-ONBOARD-008: Step 3 完成向导 → 主界面
  // ============================================================
  test('TC-ONBOARD-008 完成向导进入主界面', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'ready', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 10000 });
    await page.locator('#wizSkipStep2').click();
    await expect(page.locator('#wizardStep3')).toBeVisible({ timeout: 5000 });

    await expect(page.locator('#wizDropZone')).toBeVisible();
    await expect(page.locator('#wizFinish')).toBeVisible();
    await page.locator('#wizFinish').click();

    await expect(page.locator('#app')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#wizard')).toBeHidden();
  });

  // ============================================================
  // TC-ONBOARD-009: 完整 3 步流程（下载 → 配置 → 导入 → 主界面）
  // ============================================================
  test('TC-ONBOARD-009 完整 3 步流程端到端', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'needs_download', configured: false });
    await page.goto(uiUrl);

    // Step 1: 等待下载完成 → 自动进入 Step 2
    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 15000 });

    // Step 2: 配置 LLM → 进入 Step 3
    await page.locator('#wizKey').fill('sk-e2e-full-flow');
    await page.locator('#wizStart').click();
    await expect(page.locator('#wizardStep3')).toBeVisible({ timeout: 5000 });

    // Step 3: 完成向导
    await page.locator('#wizFinish').click();
    await expect(page.locator('#app')).toBeVisible({ timeout: 5000 });

    // 验证 LLM 配置已保存
    const settings = await page.evaluate(() =>
      (window as any).__TAURI__.core.invoke('get_settings'),
    );
    expect(settings.has_llm_config).toBe(true);
    // mock 的 get_settings 在 configured=true 时返回 mock-llm
    expect(settings.model).toBe('mock-llm');
  });

  // ============================================================
  // TC-ONBOARD-010: 步骤标签随步骤变化
  // ============================================================
  test('TC-ONBOARD-010 步骤标签随步骤变化更新', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'needs_download', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizStepLabel')).toBeVisible({ timeout: 10000 });

    // 等待进入 Step 2
    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 15000 });
    const step2Label = await page.locator('#wizStepLabel').getAttribute('data-i18n');
    expect(step2Label).toBe('wizard.step2_title');

    // 进入 Step 3
    await page.locator('#wizSkipStep2').click();
    await expect(page.locator('#wizardStep3')).toBeVisible({ timeout: 5000 });
    const step3Label = await page.locator('#wizStepLabel').getAttribute('data-i18n');
    expect(step3Label).toBe('wizard.step3_title');
  });

  // ============================================================
  // TC-ONBOARD-011: 下载进度显示百分比
  // ============================================================
  test('TC-ONBOARD-011 下载进度显示百分比', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'needs_download', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 10000 });

    const pct = await page.locator('#wizDownloadPct').textContent();
    expect(pct).toMatch(/\d+%/);
  });

  // ============================================================
  // TC-ONBOARD-012: 下载过程中状态文案更新
  // ============================================================
  test('TC-ONBOARD-012 下载过程中状态文案更新', async ({ page }) => {
    await setupTestEnv(page, { embedderStatus: 'needs_download', configured: false });
    await page.goto(uiUrl);

    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizDownloadStatus')).not.toBeEmpty({ timeout: 5000 });
  });
});
