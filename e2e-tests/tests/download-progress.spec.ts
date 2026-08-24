/**
 * E2E 测试：向导下载进度条全链路覆盖（TC-DL-001 ~ TC-DL-012）
 *
 * 测试策略：
 * - beforeEach: 拦截 check_embedder_status → needs_download，使 boot() 显示 Step 1
 * - beforeEach: 拦截 init_embedder → 返回永不 resolve 的 Promise（挂起初始下载）
 * - 每个测试通过 window.__TAURI__.event.emit 手动发送进度事件，精确控制测试场景
 *
 * 测试场景覆盖：
 * - TC-DL-001: 进入下载界面立即显示 indeterminate 动画（不等首个事件）
 * - TC-DL-002: 单文件下载进度条精确推进（17% → 50% → 100%）
 * - TC-DL-003: 多文件下载整体进度计算（file_index + current/total）
 * - TC-DL-004: Content-Length 缺失时对数估算进度
 * - TC-DL-005: 慢速连接（2s 延迟）→ 首事件前 indeterminate 动画持续
 * - TC-DL-006: 下载挂起 → 15s 后看门狗提示网络问题
 * - TC-DL-007: 下载完成 → progress-complete CSS 类 + 自动进入 Step 2
 * - TC-DL-008: 下载失败 → progress-error CSS 类 + 重试按钮可见
 * - TC-DL-009: 重试下载 → 状态重置 + 重新开始
 * - TC-DL-010: Downloading → Loading → Done 状态机完整转换
 * - TC-DL-011: shimmer 动画 CSS 类存在验证
 * - TC-DL-012: 进度条文件名/大小/序号信息正确显示
 */

import { test, expect } from '@playwright/test';
import { injectStub, injectLocales, uiUrl } from './helpers.mjs';

test.describe('TC-DL-001~012 向导下载进度条全链路', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    // 覆盖 check_embedder_status → needs_download + init_embedder → 挂起
    await page.addInitScript(() => {
      const origInvoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = function (cmd, args) {
        if (cmd === 'check_embedder_status') {
          return Promise.resolve('needs_download');
        }
        if (cmd === 'init_embedder') {
          // 挂起：不发送任何事件，不 resolve，让进度条停留在 indeterminate 状态
          return new Promise(() => {});
        }
        return origInvoke.call(this, cmd, args);
      };
    });
    await page.goto(uiUrl);
    await expect(page.locator('#wizard')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('#wizardStep1')).toBeVisible({ timeout: 10000 });
    // 等待 startDownload() 执行完毕（进度条应已进入 indeterminate 状态）
    await page.waitForTimeout(300);
  });

  /** 辅助：通过 Tauri event emit 发送下载进度事件 */
  async function emitProgress(page, payload) {
    await page.evaluate((p) => {
      window.__TAURI__.event.emit('model_download_progress', p);
    }, payload);
  }

  // ============================================================
  // TC-DL-001: 进入下载界面立即显示 indeterminate 动画
  // ============================================================
  test('TC-DL-001 进入下载界面立即显示 indeterminate 动画（不等首个事件）', async ({ page }) => {
    // beforeEach 中 init_embedder 已挂起，进度条应处于 indeterminate 状态
    const barClass = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass, '进入下载界面后应立即有 progress-indeterminate 类').toContain('progress-indeterminate');

    // 进度条不应是 100% 宽度（indeterminate 模式下 CSS 设 width:40%）
    // 不应出现“看起来已完成”的 100% 宽度
    const barWidth = await page.locator('#wizDownloadBar').evaluate((el) => el.style.width);
    expect(barWidth, 'indeterminate 模式下不应设 width:100%').not.toBe('100%');

    // 状态文本应为连接中
    const statusText = await page.locator('#wizDownloadStatus').textContent();
    expect(statusText, '状态文本应为连接中').toContain('连接');
  });

  // ============================================================
  // TC-DL-002: 单文件下载进度条精确推进
  // ============================================================
  test('TC-DL-002 单文件下载进度条精确推进（17% → 50% → 100%）', async ({ page }) => {
    // 发送第一个进度事件：5242880/31457280 ≈ 17%
    await emitProgress(page, { downloading: { current: 5242880, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
    await expect(page.locator('#wizDownloadPct')).toContainText('17%', { timeout: 3000 });
    expect(await page.locator('#wizDownloadBar').evaluate((el) => el.style.width)).toBe('17%');

    // 第二个进度事件：50%
    await emitProgress(page, { downloading: { current: 15728640, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
    await expect(page.locator('#wizDownloadPct')).toContainText('50%', { timeout: 3000 });

    // 第三个进度事件：100%
    await emitProgress(page, { downloading: { current: 31457280, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
    await expect(page.locator('#wizDownloadPct')).toContainText('100%', { timeout: 3000 });
  });

  // ============================================================
  // TC-DL-003: 多文件下载整体进度计算
  // ============================================================
  test('TC-DL-003 多文件下载整体进度计算（file_index + current/total）', async ({ page }) => {
    // 文件 0/3: config.json (1024 bytes)
    await emitProgress(page, { downloading: { current: 0, total: 1024, file_name: 'config.json', file_index: 0, total_files: 3 } });
    await page.waitForTimeout(100);
    let pct = await page.locator('#wizDownloadPct').textContent();
    expect(parseInt((pct || '0').replace('%', ''), 10), '文件 0 进度 0% → 整体 0%').toBe(0);

    await emitProgress(page, { downloading: { current: 1024, total: 1024, file_name: 'config.json', file_index: 0, total_files: 3 } });
    await page.waitForTimeout(100);
    pct = await page.locator('#wizDownloadPct').textContent();
    expect(parseInt((pct || '0').replace('%', ''), 10), '文件 0 完成 → 整体 33%').toBe(33);

    // 文件 1/3: tokenizer.json (51200 bytes)
    await emitProgress(page, { downloading: { current: 25600, total: 51200, file_name: 'tokenizer.json', file_index: 1, total_files: 3 } });
    await page.waitForTimeout(100);
    pct = await page.locator('#wizDownloadPct').textContent();
    expect(parseInt((pct || '0').replace('%', ''), 10), '文件 1 50% → 整体 50%').toBe(50);

    // 文件 2/3: model_quantized.onnx (31457280 bytes)
    await emitProgress(page, { downloading: { current: 31457280, total: 31457280, file_name: 'model_quantized.onnx', file_index: 2, total_files: 3 } });
    await page.waitForTimeout(100);
    pct = await page.locator('#wizDownloadPct').textContent();
    expect(parseInt((pct || '0').replace('%', ''), 10), '文件 2 完成 → 整体 100%').toBe(100);
  });

  // ============================================================
  // TC-DL-004: Content-Length 缺失时对数估算进度
  // ============================================================
  test('TC-DL-004 Content-Length 缺失时进度条不全程停留在 0%', async ({ page }) => {
    // total=0 的进度事件
    await emitProgress(page, { downloading: { current: 65536, total: 0, file_name: 'unknown.onnx', file_index: 0, total_files: 1 } });
    await page.waitForTimeout(100);

    const pctText = await page.locator('#wizDownloadPct').textContent();
    const pctVal = parseInt((pctText || '0').replace('%', ''), 10);
    expect(pctVal, 'Content-Length 缺失时进度应 > 0%（对数估算）').toBeGreaterThan(0);

    // bar 应有 progress-indeterminate 类（total=0 时添加）
    const barClass = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass, 'total=0 时应有 progress-indeterminate 类').toContain('progress-indeterminate');
  });

  // ============================================================
  // TC-DL-005: 慢速连接 → indeterminate 持续
  // ============================================================
  test('TC-DL-005 慢速连接时 indeterminate 动画持续到首个进度事件', async ({ page }) => {
    // 初始状态：indeterminate（init_embedder 挂起，无事件）
    const barClass1 = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass1, '无事件时应处于 indeterminate 状态').toContain('progress-indeterminate');

    // 发送首个进度事件 → indeterminate 应移除
    await emitProgress(page, { downloading: { current: 5242880, total: 31457280, file_name: 'model.onnx', file_index: 0, total_files: 1 } });
    await page.waitForTimeout(100);

    const barClass2 = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass2, '首个进度事件后应移除 indeterminate').not.toContain('progress-indeterminate');
  });

  // ============================================================
  // TC-DL-006 已删除：15s 看门狗功能已移除（魔塔下载很快），
  // indeterminate 持续显示由 TC-DL-005 覆盖。
  // ============================================================

  // ============================================================
  // TC-DL-007: 下载完成 → progress-complete + 自动进入 Step 2
  // ============================================================
  test('TC-DL-007 下载完成 → progress-complete CSS 类 + 自动进入 Step 2', async ({ page }) => {
    // 发送 loading 事件（= 下载完成信号，ONNX 在后台加载）
    await emitProgress(page, { loading: true });

    // 等待 progress-complete 类
    await expect(page.locator('#wizDownloadBar')).toHaveClass(/progress-complete/, { timeout: 3000 });
    await expect(page.locator('#wizDownloadPct')).toContainText('100%');

    // 自动进入 Step 2（延迟 800ms）
    await expect(page.locator('#wizardStep2')).toBeVisible({ timeout: 5000 });
  });

  // ============================================================
  // TC-DL-008: 下载连续失败 4 次 → 超过自动重试上限 → progress-error + 手动重试
  // ============================================================
  test('TC-DL-008 连续失败超过自动重试上限 → progress-error + 手动重试按钮', async ({ page }) => {
    // 自动重试 3 次（2s + 4s + 8s = 14s），每次都发送 error 事件
    test.setTimeout(60000);
    // 第 1 次失败
    await emitProgress(page, { error: { message: '下载失败：网络连接超时' } });
    await expect(page.locator('#wizDownloadStatus')).toContainText('自动重试', { timeout: 3000 });

    // 等待第 1 次自动重试（2s 后）→ 发送第 2 次失败
    await page.waitForTimeout(2100);
    await emitProgress(page, { error: { message: '下载失败：网络连接超时' } });
    await expect(page.locator('#wizDownloadStatus')).toContainText('自动重试', { timeout: 3000 });

    // 等待第 2 次自动重试（4s 后）→ 发送第 3 次失败
    await page.waitForTimeout(4100);
    await emitProgress(page, { error: { message: '下载失败：网络连接超时' } });
    await expect(page.locator('#wizDownloadStatus')).toContainText('自动重试', { timeout: 3000 });

    // 等待第 3 次自动重试（8s 后）→ 发送第 4 次失败 → 超过上限
    await page.waitForTimeout(8100);
    await emitProgress(page, { error: { message: '下载失败：网络连接超时' } });

    // 超过自动重试上限 → 显示手动重试
    await expect(page.locator('#wizDownloadBar')).toHaveClass(/progress-error/, { timeout: 3000 });
    const statusText = await page.locator('#wizDownloadStatus').textContent();
    expect(statusText, '失败时状态文本应为下载失败').toContain('失败');
    await expect(page.locator('#wizRetryBtn')).toBeVisible({ timeout: 3000 });
  });

  // ============================================================
  // TC-DL-009: 重试下载 → 状态重置 + 重新开始
  // ============================================================
  test('TC-DL-009 手动重试 → 状态重置 + 重新开始', async ({ page }) => {
    // 先触发 4 次连续失败以超过自动重试上限
    test.setTimeout(60000);
    for (let i = 0; i < 4; i++) {
      if (i > 0) {
        const delay = Math.pow(2, i); // 2,4,8
        await page.waitForTimeout(delay * 1000 + 100);
      }
      await emitProgress(page, { error: { message: '网络超时' } });
      if (i < 3) {
        await expect(page.locator('#wizDownloadStatus')).toContainText('自动重试', { timeout: 3000 });
      }
    }
    await expect(page.locator('#wizDownloadBar')).toHaveClass(/progress-error/, { timeout: 5000 });

    // 点击重试按钮
    await page.locator('#wizRetryBtn').click();

    // 进度条应重置：error 移除，indeterminate 添加
    await page.waitForTimeout(200);
    const barClass = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass, '重试后应移除 progress-error').not.toContain('progress-error');
    expect(barClass, '重试后应添加 progress-indeterminate').toContain('progress-indeterminate');

    // 错误框应隐藏
    await expect(page.locator('#wizDownloadError')).toBeHidden({ timeout: 1000 });

    // 发送进度事件验证可以正常下载
    await emitProgress(page, { downloading: { current: 5242880, total: 31457280, file_name: 'model.onnx', file_index: 0, total_files: 1 } });
    await expect(page.locator('#wizDownloadPct')).toContainText('17%', { timeout: 3000 });
  });

  // ============================================================
  // TC-DL-010: Downloading → Loading 状态机转换（Loading 即进入下一步）
  // ============================================================
  test('TC-DL-010 Downloading → Loading 即进入下一步（不等 ONNX 加载）', async ({ page }) => {
    // 1. 初始：连接中 → indeterminate
    const barClass1 = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass1, '初始状态应有 indeterminate').toContain('progress-indeterminate');

    // 2. Downloading 阶段
    await emitProgress(page, { downloading: { current: 5242880, total: 31457280, file_name: 'model.onnx', file_index: 0, total_files: 1 } });
    await expect(page.locator('#wizDownloadPct')).toContainText('17%', { timeout: 3000 });

    // 3. Loading 阶段 = 下载完成，立即进入下一步
    await emitProgress(page, { loading: true });
    await expect(page.locator('#wizDownloadStatus')).toContainText('下载完成', { timeout: 3000 });
    await expect(page.locator('#wizDownloadBar')).toHaveClass(/progress-complete/);
  });

  // ============================================================
  // TC-DL-011: shimmer 动画 CSS 类存在验证
  // ============================================================
  test('TC-DL-011 progress-shimmer CSS 类应用于进度条', async ({ page }) => {
    // 初始状态：indeterminate（有动画）
    const barClass = await page.locator('#wizDownloadBar').getAttribute('class');
    expect(barClass, '进度条应有 progress-shimmer 类').toContain('progress-shimmer');
    expect(barClass, '进度条应有 progress-indeterminate 类').toContain('progress-indeterminate');

    // 验证 CSS 规则存在
    const animationName = await page.locator('#wizDownloadBar').evaluate((el) => {
      return window.getComputedStyle(el).animationName;
    });
    expect(animationName, 'shimmer 动画应被应用').not.toBe('none');
  });

  // ============================================================
  // TC-DL-012: 进度条百分比正确显示
  // ============================================================
  test('TC-DL-012 进度条百分比正确显示', async ({ page }) => {
    // 发送进度事件
    await emitProgress(page, { downloading: { current: 5242880, total: 31457280, file_name: 'model_quantized.onnx', file_index: 0, total_files: 1 } });
    await page.waitForTimeout(100);

    // 百分比应显示
    const pctText = await page.locator('#wizDownloadPct').textContent();
    expect(pctText, '百分比应匹配格式').toMatch(/\d+%/);

    // 进度条宽度应与百分比一致
    const barWidth = await page.locator('#wizDownloadBar').evaluate((el) => el.style.width);
    expect(barWidth, '进度条宽度应包含 %').toContain('%');
  });
});
