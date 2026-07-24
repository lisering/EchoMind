import { defineConfig } from '@playwright/test';

// L3-lite 桥接层配置（Chromium + 契约级 Mock __TAURI__ 注入，全平台 headless）。
// 注：真实 GUI 层（tauri-driver）走 WebDriver 协议（127.0.0.1:4444），
// Playwright 仅讲 CDP、无法直连 WebDriver 服务器，故真实层由 WebdriverIO 承担（见 wdio.conf.mjs）。
export default defineConfig({
  testDir: './tests',
  timeout: 120_000,
  retries: 0,
  workers: 1,
  reporter: [['list']],
  use: {
    headless: true,
    actionTimeout: 30_000,
  },
});
