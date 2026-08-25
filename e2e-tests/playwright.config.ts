import { defineConfig } from '@playwright/test';
import os from 'node:os';

// L3-lite 桥接层配置（Chromium + 契约级 Mock __TAURI__ 注入，全平台 headless）。
// 注：真实 GUI 层（tauri-driver）走 WebDriver 协议（127.0.0.1:4444），
// Playwright 仅讲 CDP、无法直连 WebDriver 服务器，故真实层由 WebdriverIO 承担（见 wdio.conf.mjs）。
//
// 真实 LLM E2E 测试（real-llm.spec.ts）默认排除，仅在特定 CI job 中运行：
//   ECHOMIND_E2E_REAL_LLM=1 npx playwright test tests/real-llm.spec.ts
//
// 性能优化说明：
// - workers: CI 固定 4 进程，本地使用 CPU 核心数的 50%（上限 8）
// - fullyParallel: 文件内+文件间全量并行（每个测试独立 BrowserContext，无状态共享）
// - shard: CI 中通过 --shard=I/N 拆分到多个 runner 并行执行
// - 超时从 120s 降至 60s（并行后单个测试不再需要等前面排队完成）
// - E2E_SPEED 环境变量控制 Mock 延迟倍率（默认 1.0，CI 设 0.2 加速 5 倍）
export default defineConfig({
  testDir: './tests',
  // V3.1 阶段二：CI 环境敏感 spec 排除（像素对比/渲染时序/性能阈值断言
  // 在 CI 慢机不可靠，本地全量验证）。设 CI_E2E_SKIP_UNSTABLE=1 启用。
  testIgnore: process.env.CI_E2E_SKIP_UNSTABLE === '1'
    ? [
        '**/visual-regression.spec.ts',
        '**/viz.spec.ts',
        '**/viz-advanced.spec.ts',
        '**/memory-leak.spec.ts',
        '**/navigation-advanced.spec.ts',
      ]
    : [],
  // 排除 vitest 单元测试目录（由 vitest.config.ts 单独管理）
  testMatch: /.*\.spec\.ts$/,
  // 默认排除真实 LLM 测试（需真实 API Key + cargo tauri dev 运行中）
  // 通过命令行 --grep 或 ECHOMIND_E2E_REAL_LLM=1 显式启用
  exclude: /real-llm\.spec\.ts$/,
  // 全量并行：文件间 + 文件内并行（每个测试独立 BrowserContext，Mock state 天然隔离）
  fullyParallel: true,
  // 超时从 120s 降至 60s（并行后不再排队等待，单个测试不应超过 60s）
  timeout: 60_000,
  // CI 禁止 test.only，本地允许
  forbidOnly: !!process.env.CI,
  // 重试策略（V3.1 阶段一）：本地默认 0 快速失败不掩盖问题；
  // CI 设 PLAYWRIGHT_RETRIES=1 收敛环境抖动（真实回归在重试后仍会红）。
  // real-data job 单独用 --retries=1（外部 LLM/网络依赖抖动更大）。
  retries: process.env.PLAYWRIGHT_RETRIES ? Number(process.env.PLAYWRIGHT_RETRIES) : 0,
  // 并行 worker 数：CI 固定 4，本地用 CPU 核心数的 50%（上限 8）
  workers: process.env.CI ? 4 : Math.min(8, Math.ceil(os.cpus().length / 2)),
  reporter: process.env.CI ? [['list'], ['blob', { outputDir: 'blob-report' }]] : [['list']],
  use: {
    headless: true,
    // 单个操作超时从 30s 降至 15s（并行后响应更快）
    actionTimeout: 15_000,
    // 失败时保留 trace 用于调试
    trace: 'on-first-retry',
  },
  // CI 分片支持：通过 --shard=I/N 拆分测试到多个 runner
  // GitHub Actions 示例见 .github/workflows/e2e.yml e2e-bridge job
});
