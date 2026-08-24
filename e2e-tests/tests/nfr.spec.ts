// REQ-NFR-001 冷启动性能 E2E 计时测试（L3-lite 桥接层）。
// 测量从页面导航到主界面可交互的耗时，断言 ≤ 2s（Apple Silicon 基准机）。
// 注：E2E bridge 环境下 tauri-stub 模拟后端，测量的是前端渲染+初始化时间，
// 作为冷启动的代理指标。真实 Tauri 冷启动由 L3 GUI E2E 覆盖。
import { test, expect } from '@playwright/test';
import { setupPageWizard, enterApp } from './helpers.mjs';

test.describe('REQ-NFR-001 冷启动性能', () => {
  test('从启动到主界面可交互 ≤ 2s', async ({ page }) => {
    // 记录开始时间（导航前）
    const start = Date.now();

    // 注入 tauri-stub 并导航到 UI
    await setupPageWizard(page);

    // 经向导快速进入主界面（模拟首次启动配置）
    await enterApp(page);

    // 计算耗时
    const elapsed = Date.now() - start;

    // AC-1：从启动到主界面可交互 ≤ 2s（2000ms）
    // 注：CI 环境可能比本地慢，使用 3000ms 作为 CI 容忍上限
    expect(elapsed).toBeLessThan(3000);
    // 本地基准断言（更严格）
    if (process.env.CI !== 'true') {
      expect(elapsed).toBeLessThan(2000);
    }
  });
});
