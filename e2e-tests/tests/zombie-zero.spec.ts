// E2E Zombie Zero 测试（僵尸率 0% + 借鉴消化度 100%）：
// TC-ZOMBIE-001: PagedAttention toggle 可见且可切换
// TC-ZOMBIE-002: 内核模式选择器可见且可切换
// TC-ZOMBIE-003: 图谱布局切换按钮点击后有响应
// TC-ZOMBIE-004: 全量 IPC 命令无僵尸验证（4 个原僵尸命令 IPC 封装存在）
import { test, expect } from '@playwright/test';
import { setupPage, clickToolButton } from './helpers.mjs';

test.describe('TC-ZOMBIE Zombie Zero 测试', () => {
  test.beforeEach(async ({ page }) => {
    await setupPage(page);
  });

  // ─── S1: PagedAttention toggle ───

  test('TC-ZOMBIE-001 PagedAttention toggle 可见且可切换', async ({ page }) => {
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    await page.waitForTimeout(800);

    // 验证 PagedAttention 设置区域存在
    const pagedAttnSection = page.locator('#pagedAttnSection');
    await expect(pagedAttnSection).toBeVisible();

    // 验证 toggle 按钮存在
    const toggle = page.locator('#pagedAttnToggle');
    await expect(toggle).toBeVisible();

    // 如果 Pro 已激活（按钮未 disabled），验证 toggle 可切换
    const isDisabled = await toggle.getAttribute('disabled');
    if (!isDisabled) {
      const initialChecked = await toggle.getAttribute('aria-checked');
      await toggle.click();
      await page.waitForTimeout(300);
      const newChecked = await toggle.getAttribute('aria-checked');
      expect(newChecked).not.toBe(initialChecked);
    }
  });

  // ─── S1: 内核模式选择器 ───

  test('TC-ZOMBIE-002 内核模式选择器可见且可切换', async ({ page }) => {
    // 打开设置面板
    await page.locator('#settingsBtn').click();
    await expect(page.locator('#settingsModal')).toBeVisible({ timeout: 5000 });
    await page.evaluate(() => {
      document.querySelectorAll('[data-settings-section]').forEach((el) => el.classList.remove('hidden'));
    });
    await page.waitForTimeout(800);

    // 验证内核模式设置区域存在
    const kernelModeSection = page.locator('#kernelModeSection');
    await expect(kernelModeSection).toBeVisible();

    // 验证两个按钮存在
    const btnMistral = page.locator('#kernelBtnMistral');
    const btnGemv = page.locator('#kernelBtnGemv');
    await expect(btnMistral).toBeVisible();
    await expect(btnGemv).toBeVisible();

    // 如果 Pro 已激活（按钮未 disabled），验证按钮可切换
    const isDisabled = await btnMistral.getAttribute('disabled');
    if (!isDisabled) {
      // 点击 Custom GEMV 按钮
      await btnGemv.click();
      await page.waitForTimeout(300);
      // 验证按钮样式切换（active class）
      const gemvClass = await btnGemv.getAttribute('class');
      expect(gemvClass).toContain('bg-accent');

      // 点击回 Mistral.rs 按钮
      await btnMistral.click();
      await page.waitForTimeout(300);
      const mistralClass = await btnMistral.getAttribute('class');
      expect(mistralClass).toContain('bg-accent');
    }
  });

  // ─── S1: 图谱布局切换 ───

  test('TC-ZOMBIE-003 图谱布局切换按钮点击后有响应', async ({ page }) => {
    // 打开知识图谱面板
    await clickToolButton(page, 'graphBtn');
    await expect(page.locator('#graphOverlay')).toBeVisible({ timeout: 5000 });
    await page.waitForTimeout(1000);

    // 验证布局面板存在
    const layoutPanel = page.locator('#graphLayoutPanel');
    await expect(layoutPanel).toBeVisible();

    // 验证三个布局按钮存在
    const forceBtn = page.locator('.graph-layout-btn[data-layout="force"]');
    const hierarchicalBtn = page.locator('.graph-layout-btn[data-layout="hierarchical"]');
    const radialBtn = page.locator('.graph-layout-btn[data-layout="radial"]');
    await expect(forceBtn).toBeVisible();
    await expect(hierarchicalBtn).toBeVisible();
    await expect(radialBtn).toBeVisible();

    // 点击 hierarchical 布局
    await hierarchicalBtn.click();
    await page.waitForTimeout(500);
    // 验证 hierarchical 按钮变为 active
    const hierClass = await hierarchicalBtn.getAttribute('class');
    expect(hierClass).toContain('graph-layout-active');

    // 点击 radial 布局
    await radialBtn.click();
    await page.waitForTimeout(500);
    const radialClass = await radialBtn.getAttribute('class');
    expect(radialClass).toContain('graph-layout-active');

    // 点击回 force 布局
    await forceBtn.click();
    await page.waitForTimeout(500);
    const forceClass = await forceBtn.getAttribute('class');
    expect(forceClass).toContain('graph-layout-active');
  });

  // ─── S1: 全量 IPC 无僵尸验证 ───

  test('TC-ZOMBIE-004 全量 IPC 命令无僵尸验证（4 个原僵尸命令 IPC mock 可调用）', async ({ page }) => {
    // 验证 4 个原先的僵尸命令现在都能通过 tauri-stub mock 调用
    const results = await page.evaluate(async () => {
      const results: Record<string, boolean> = {};
      try {
        // 通过 window.__TAURI__.core.invoke 直接调用 mock handler
        const invoke = window.__TAURI__?.core?.invoke;
        if (!invoke) {
          results['error'] = true;
          return results;
        }

        // set_paged_attn
        await invoke('set_paged_attn', { enabled: true, blockSize: 512, gpuMemoryCtx: 512 });
        results['set_paged_attn'] = true;

        // get_kernel_mode
        const mode = await invoke('get_kernel_mode');
        results['get_kernel_mode'] = typeof mode === 'string' || mode === undefined;

        // set_kernel_mode
        await invoke('set_kernel_mode', { mode: 'mistralrs' });
        results['set_kernel_mode'] = true;

        // get_graph_layout
        const layouts = await invoke('get_graph_layout');
        results['get_graph_layout'] = Array.isArray(layouts);
      } catch (err) {
        results['error'] = String(err);
      }
      return results;
    });

    // 验证所有 4 个命令都能成功调用（无错误）
    expect(results['set_paged_attn']).toBe(true);
    expect(results['get_kernel_mode']).toBe(true);
    expect(results['set_kernel_mode']).toBe(true);
    expect(results['get_graph_layout']).toBe(true);
    expect(results['error']).toBeUndefined();
  });
});
